//! [`Interactive`]: the bounded-control wrapper for things that take the
//! keyboard. Its hover, press and selection are the one model
//! ([`super::paint`]); what it adds is the focus-visible ring and the tab
//! stop.

use aui_tokens::ActiveAui;
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, MouseButton, RenderOnce, Role, SharedString, Window};

use super::paint::{PaintStates, StatePaint};
use super::Shape;
use crate::data::record_ax_label;
use crate::util::ClickHandler;

/// Key fragment for the per-control focus handle (see [`Interactive`]).
pub const INTERACT_FOCUS_TAG: &str = "aui-interact-focus";

/// A pinned interaction state: what [`Interactive::preview`] and
/// [`StatePaint::preview`] paint instead of the live one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractState {
    /// The pointer is over the control.
    pub hovered: bool,
    /// The primary button is held down on the control.
    pub pressed: bool,
    /// The control holds keyboard focus.
    pub focused: bool,
}

impl InteractState {
    /// Nothing pinned: the rest look.
    pub const REST: Self = Self { hovered: false, pressed: false, focused: false };
    /// The pointer over the control.
    pub const HOVER: Self = Self { hovered: true, pressed: false, focused: false };
    /// The control held down.
    pub const PRESSED: Self = Self { hovered: true, pressed: true, focused: false };
    /// The control holding the keyboard (the focus-visible ring).
    pub const FOCUSED: Self = Self { hovered: false, pressed: false, focused: true };
}

/// The focus-visible ring every control draws: 2 px of `accent-ring`, 2 px
/// outside the control, as an outline overlay — never a spread shadow,
/// which shows through a see-through fill. Add it as a child of a
/// `relative()` control whose corner radius is `radius`.
pub fn focus_ring(radius: gpui::Pixels, p: &aui_tokens::Palette) -> gpui::Div {
    div()
        .absolute()
        .top(px(-2.0))
        .left(px(-2.0))
        .right(px(-2.0))
        .bottom(px(-2.0))
        .rounded(radius + px(2.0))
        .border_2()
        .border_color(p.accent_ring)
}

/// Maps the [`Interactive::role`] name to an accessibility role.
/// Unknown names fall back to `Button`: a control that wraps content and
/// takes clicks is a button unless the caller says otherwise.
fn parse_role(name: &str) -> Role {
    match name.to_ascii_lowercase().as_str() {
        "link" => Role::Link,
        "checkbox" => Role::CheckBox,
        "radio" => Role::RadioButton,
        "tab" => Role::Tab,
        "menuitem" | "menu-item" => Role::MenuItem,
        "menu" => Role::Menu,
        "list" => Role::List,
        "option" | "listitem" | "list-item" | "row" => Role::ListItem,
        _ => Role::Button,
    }
}

/// A bounded control wrapper: the one hover / press / selected paint
/// ([`StatePaint`]), the focus-visible ring and tab stop, and the disabled
/// treatment.
///
/// It never sets a background colour beyond the overlays: the ground comes
/// from whatever is beneath it. Build with [`interactive`].
#[derive(IntoElement)]
pub struct Interactive {
    id: ElementId,
    child: AnyElement,
    selected: bool,
    disabled: bool,
    pressable: bool,
    focusable: bool,
    shape: Shape,
    role: Role,
    label: Option<SharedString>,
    on_click: Option<ClickHandler>,
    preview: Option<InteractState>,
}

/// Wraps `child` in an [`Interactive`] control with `id`.
pub fn interactive(id: impl Into<ElementId>, child: impl IntoElement) -> Interactive {
    Interactive {
        id: id.into(),
        child: child.into_any_element(),
        selected: false,
        disabled: false,
        pressable: true,
        focusable: false,
        shape: Shape::Item,
        role: Role::Button,
        label: None,
        on_click: None,
        preview: None,
    }
}

impl Interactive {
    /// Whether the selected overlay paints.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Disabled controls wear `fg-muted` and never press or hover.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Whether a press tints the control (default true). A control that
    /// only navigates (a link-like row) can opt out.
    pub fn pressable(mut self, pressable: bool) -> Self {
        self.pressable = pressable;
        self
    }

    /// Whether the control is a tab stop that draws the focus-visible ring
    /// (default false). Rows in a list stay non-focusable: the highlighted
    /// row is their focus indicator.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// The corner shape (default [`Shape::Item`]).
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// The accessibility role name (`"button"`, `"link"`, `"checkbox"`,
    /// `"radio"`, `"tab"`, `"menuitem"`, `"menu"`, `"list"`, `"option"`).
    /// Unknown names fall back to `"button"`.
    pub fn role(mut self, role: &str) -> Self {
        self.role = parse_role(role);
        self
    }

    /// The human label: the accessible name and the AX-probe record.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Paints a fixed state instead of the live one — for gallery cards
    /// and captures, where no pointer hovers.
    /// The control still takes clicks; only the paint is pinned.
    pub fn preview(mut self, state: InteractState) -> Self {
        self.preview = Some(state);
        self
    }

    /// Click handler (mouse and keyboard activation alike: gpui turns
    /// enter / space on a focused element into a `ClickEvent`).
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Interactive {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let disabled = self.disabled;

        // One focus handle per control id; only focusable controls are tab
        // stops. The handle key matches `interactive_state`, so callers read
        // the same focus the ring draws from.
        let focus = window
            .use_keyed_state((id.clone(), INTERACT_FOCUS_TAG), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
            .tab_stop(self.focusable && !disabled);
        let focus_pinned = self.preview.is_some_and(|s| s.focused);
        let show_ring = focus_pinned || (self.focusable && !disabled && focus.is_focused(window) && crate::keys::keyboard_nav(cx));

        // Hover, press and selection are the one model (`paint`): gpui's own
        // refinements, no keyed state, no tween.
        let paint = StatePaint::new()
            .selected(self.selected)
            .disabled(disabled)
            .pressable(self.pressable)
            .preview(self.preview);
        let radius = self.shape.radius();

        // The focus-visible ring: 2 px of accent-ring, 2 px outside the
        // control, only while the keyboard-nav flag is armed. An outline
        // overlay, never a shadow: a spread shadow would fill the control.
        let ring = show_ring.then(|| focus_ring(px(radius), &p));

        let mut outer = div()
            .id(id)
            .relative()
            .rounded(px(radius))
            .paint_states(paint, cx)
            .when(disabled, |d| d.text_color(p.fg_muted))
            .when(!disabled && self.on_click.is_some(), |d| d.cursor_pointer())
            .child(self.child)
            .children(ring);
        if !disabled {
            outer = outer.track_focus(&focus);
            outer = outer.on_key_down(|_, window, cx| {
                if !crate::keys::keyboard_nav(cx) {
                    crate::keys::set_keyboard_nav(true, cx);
                    window.refresh();
                }
            });
            // A mouse press anywhere on the control disarms the ring: the
            // next keyboard press re-arms it.
            outer = outer.on_mouse_down(MouseButton::Left, |_, window, cx| {
                if crate::keys::keyboard_nav(cx) {
                    crate::keys::set_keyboard_nav(false, cx);
                    window.refresh();
                }
            });
            if let Some(on_click) = self.on_click {
                outer = outer.on_click(move |e, w, cx| on_click(e, w, cx));
            }
        }
        // Every control names itself for the accessibility tree and the
        // AX probe; an empty record is an unlabelled control.
        record_ax_label(self.label.as_deref().unwrap_or(""));
        outer = outer.role(self.role);
        if let Some(label) = self.label {
            outer = outer.aria_label(label);
        }
        outer
    }
}
