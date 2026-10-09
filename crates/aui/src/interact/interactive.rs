//! [`Interactive`]: the bounded-control wrapper that owns all state paint.

use aui_motion::{tint_fade, SpringKind, Tween};
use aui_tokens::{ActiveAui, Easing, Tier};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, MouseButton, RenderOnce, Role, SharedString, Window};

use super::Shape;
use crate::data::record_ax_label;
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// Key fragment for the per-control focus handle (see [`Interactive`]).
/// The gallery's states card uses it to focus the focus-visible tile.
pub const INTERACT_FOCUS_TAG: &str = "aui-interact-focus";

/// A snapshot of one interactive control's live flags, for callers that
/// need them (a trailing icon that follows the hover, for example).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractState {
    /// The pointer is over the control.
    pub hovered: bool,
    /// The primary button is held down on the control.
    pub pressed: bool,
    /// The control holds keyboard focus.
    pub focused: bool,
}

/// Reads the live [`InteractState`] for `id`: the hover / press flags from
/// [`crate::util::interaction`] plus whether the control's focus handle is
/// focused. Never notifies; call it during render.
pub fn interactive_state(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> InteractState {
    let id: ElementId = id.into();
    let (_, flags) = interaction_flags(id.clone(), window, cx);
    let focused = window
        .use_keyed_state((id, INTERACT_FOCUS_TAG), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .is_focused(window);
    InteractState { hovered: flags.hovered, pressed: flags.pressed, focused }
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

/// A bounded control wrapper that owns ALL state paint: the hover overlay,
/// the selected overlay, the pressed overlay plus the 1 px press inset, the
/// focus-visible ring, and the disabled treatment.
///
/// It never sets a background colour beyond the overlays: the ground comes
/// from the [`Surface`](super::Surface) beneath it. Build with
/// [`interactive`].
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

    /// Whether a press insets the control 1 px per side (default true).
    /// A control that only navigates (a link-like row) can opt out.
    pub fn pressable(mut self, pressable: bool) -> Self {
        self.pressable = pressable;
        self
    }

    /// Whether the control is a tab stop that draws the focus-visible ring
    /// (default false). Rows inside a [`FluidList`](super::FluidList) stay
    /// non-focusable: the highlighted row is their focus indicator.
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
    /// and captures, where no pointer hovers and animations are frozen.
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

        let (state, live) = interaction_flags(id.clone(), window, cx);
        let (hovered, pressed, focus_pinned) = match self.preview {
            Some(fixed) => (fixed.hovered && !disabled, fixed.pressed && !disabled && self.pressable, fixed.focused),
            None => (live.hovered && !disabled, live.pressed && !disabled && self.pressable, false),
        };

        // One focus handle per control id; only focusable controls are tab
        // stops. The handle key matches `interactive_state`, so callers read
        // the same focus the ring draws from.
        let focus = window
            .use_keyed_state((id.clone(), INTERACT_FOCUS_TAG), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
            .tab_stop(self.focusable && !disabled);
        let show_ring = focus_pinned || (self.focusable && !disabled && focus.is_focused(window) && crate::keys::keyboard_nav(cx));

        // Overlays fade on the fast tier; the press inset springs back on
        // release. Weight never changes on hover.
        let enter = Tween::new(Tier::Fast.enter(), Easing::STD);
        let pinned = self.preview.is_some();
        let (hover_tint, selected_tint, pressed_tint, press) = if pinned {
            let on = |flag: bool, tint: gpui::Hsla| if flag { tint } else { tint.alpha(0.0) };
            (
                on(hovered, p.hover_overlay()),
                on(self.selected && !disabled, p.selected_overlay()),
                on(pressed, p.pressed_overlay()),
                if pressed { 1.0 } else { 0.0 },
            )
        } else {
            (
                tint_fade((id.clone(), "aui-hover"), hovered, p.hover_overlay(), enter, window, cx),
                tint_fade((id.clone(), "aui-selected"), self.selected && !disabled, p.selected_overlay(), enter, window, cx),
                tint_fade((id.clone(), "aui-pressed"), pressed, p.pressed_overlay(), Tween::exit(Tier::Fast), window, cx),
                aui_motion::spring_phase((id.clone(), "aui-press"), pressed, SpringKind::Fast, window, cx).clamp(0.0, 1.0),
            )
        };
        let inset = px(1.0) * press;

        let radius = self.shape.radius();
        let overlay = |tint: gpui::Hsla| div().absolute().inset(px(0.0)).rounded(px(radius)).bg(tint);

        let inner = div()
            .relative()
            .rounded(px(radius))
            .child(overlay(hover_tint))
            .child(overlay(selected_tint))
            .child(overlay(pressed_tint))
            .child(self.child);

        // The focus-visible ring: 2 px of accent-ring, 2 px outside the
        // control, only while the keyboard-nav flag is armed. An outline
        // overlay, never a shadow: a spread shadow would fill the control.
        let ring = show_ring.then(|| {
            div()
                .absolute()
                .top(px(-2.0))
                .left(px(-2.0))
                .right(px(-2.0))
                .bottom(px(-2.0))
                .rounded(px(radius + 2.0))
                .border_2()
                .border_color(p.accent_ring)
        });

        let mut outer = div()
            .id(id)
            .relative()
            .rounded(px(radius))
            .p(inset)
            .when(disabled, |d| d.text_color(p.fg_muted))
            .when(!disabled && self.on_click.is_some(), |d| d.cursor_pointer())
            .child(inner)
            .children(ring);
        if !disabled {
            outer = outer.track_focus(&focus);
            if !pinned {
                outer = outer.track_interaction(&state);
            }
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
