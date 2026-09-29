//! Accessibility writes for text fields: dictation, VoiceOver and UI automation
//! reach a text field through the accessibility tree, not through keystrokes.
//! A text node that only advertises a role accepts those writes and drops
//! them; the frame below routes them into the state instead.
//!
//! [`apply_a11y_text_action`] is the whole routing table (action + data →
//! state operation). The element builders ([`a11y_text_field`],
//! [`a11y_text_input`]) expose the role, label and live value and attach that
//! table to the node, and the composer wraps its text area in
//! [`a11y_text_field`]. Tests drive [`apply_a11y_text_action`] directly:
//! gpui's test harness cannot dispatch an [`accesskit::ActionRequest`] to a
//! window (`Window::handle_a11y_action` is `pub(crate)`, and listeners only
//! register while a11y is active).

use gpui::accesskit::ActionData;
use gpui::{
    div, prelude::*, AccessibleAction, App, Div, ElementId, Entity, Role,
    SharedString, Stateful, StatefulInteractiveElement, Window,
};
use gpui_kit::base::input::{Input, InputBaseState, InputModeKind, InputState};

/// Routes one accessibility action into a gpui-kit text state. Returns whether
/// the action was handled.
///
/// - `SetValue` with a text value replaces the whole value (undoable, and an
///   [`InputEvent::Change`][gpui_kit::base::input::InputEvent] fires, exactly
///   as for a programmatic `replace_all`).
/// - `ReplaceSelectedText` with a text value replaces the selection, or
///   inserts at the caret when the selection is collapsed — what dictation and
///   `AXSelectedText` sends mean.
/// - `Focus` moves keyboard focus into the field.
/// - Anything else (or a text action without its text payload) is ignored.
pub fn apply_a11y_text_action<M: InputModeKind>(
    state: &Entity<InputBaseState<M>>,
    action: AccessibleAction,
    data: Option<&ActionData>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    match action {
        AccessibleAction::SetValue => {
            let Some(ActionData::Value(value)) = data else {
                return false;
            };
            let value = value.to_string();
            state.update(cx, |state, cx| {
                state.replace_all(value, window, cx);
            });
            true
        }
        AccessibleAction::ReplaceSelectedText => {
            let Some(ActionData::Value(value)) = data else {
                return false;
            };
            let value = value.to_string();
            state.update(cx, |state, cx| {
                state.replace(value, window, cx);
            });
            true
        }
        AccessibleAction::Focus => {
            state.update(cx, |state, cx| {
                state.focus(window, cx);
            });
            true
        }
        _ => false,
    }
}

/// The accessibility frame for a text field: a [`Stateful<Div>`] carrying the id, role,
/// label, live value and the [`apply_a11y_text_action`] handlers for `state`.
/// Chain layout and the field element itself onto the return:
///
/// ```ignore
/// a11y_text_field(("search", "field"), "Search", &state, false, cx)
///     .w_full()
///     .child(gpui_kit::base::input::Input::new(&state))
/// ```
///
/// The value is withheld while the field masks it (passwords and the like
/// stay out of the accessibility tree, the way gpui-kit's own input does).
pub fn a11y_text_field<M: InputModeKind>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    state: &Entity<InputBaseState<M>>,
    multiline: bool,
    cx: &App,
) -> Stateful<Div> {
    let snapshot = state.read(cx);
    let value = (!snapshot.presentation().is_masked()).then(|| snapshot.value());
    let placeholder = {
        let placeholder = snapshot.presentation().placeholder().clone();
        (!placeholder.is_empty()).then_some(placeholder)
    };
    let role = if multiline {
        Role::MultilineTextInput
    } else {
        Role::TextInput
    };
    let for_set = state.clone();
    let for_replace = state.clone();
    let for_focus = state.clone();
    div()
        .id(id)
        .role(role)
        .aria_label(label)
        .when_some(placeholder, |this, placeholder| {
            this.aria_placeholder(placeholder)
        })
        .when_some(value, |this, value| this.aria_value(value))
        .on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
            apply_a11y_text_action(&for_set, AccessibleAction::SetValue, data, window, cx);
        })
        .on_a11y_action(
            AccessibleAction::ReplaceSelectedText,
            move |data, window, cx| {
                apply_a11y_text_action(
                    &for_replace,
                    AccessibleAction::ReplaceSelectedText,
                    data,
                    window,
                    cx,
                );
            },
        )
        .on_a11y_action(AccessibleAction::Focus, move |data, window, cx| {
            apply_a11y_text_action(&for_focus, AccessibleAction::Focus, data, window, cx);
        })
}

/// Any single-line gpui-kit [`Input`] with the full accessibility surface:
/// role, label, live value, and the set / insert-at-caret / focus actions.
/// What a consumer's search field wraps itself in.
///
/// Returns the [`Stateful`] frame itself (not an opaque element) so callers
/// can chain layout onto it — a flex-filling slot, for example — the way
/// [`a11y_text_field`]'s documentation shows.
pub fn a11y_text_input(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    state: &Entity<InputState>,
    cx: &App,
) -> Stateful<Div> {
    a11y_text_field(id, label, state, false, cx).child(Input::new(state))
}
