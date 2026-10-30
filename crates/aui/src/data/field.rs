//! The text field's frame: the design system's one focus treatment for a
//! text field, the composer's (v3, owner 2026-10-14) — a hairline edge in
//! the foreground at 8 % that brightens to 22 % while the field holds the
//! keyboard, on an 80 ms ease-out. No accent border, no glow: the kit's
//! default focus (a 2 px accent ring plus its halo) read as a heavy blue
//! box inside a quiet card (owner 2026-10-31, the question card's
//! "Say what you would rather …" field).
//!
//! Wrap the kit's editor with `appearance(false)` and `bordered(false)` in
//! [`field_frame`], so the frame is the only edge drawn.

use aui_motion::{tween, Tween};
use aui_tokens::{scale, ActiveAui};
use gpui::{div, prelude::*, px, App, Div, ElementId, Entity, Hsla, Stateful, Window};
use gpui_kit::base::input::{InputBaseState, InputModeKind};

/// The edge at rest: the foreground at 8 % (the composer's field).
pub const FIELD_EDGE_REST: f32 = 0.08;
/// The edge while focused: the foreground at 22 % (the composer's field).
pub const FIELD_EDGE_FOCUSED: f32 = 0.22;
/// The edge's change between the two: the composer's 80 ms ease-out.
const FIELD_EDGE_FADE: Tween = Tween::new(std::time::Duration::from_millis(80), aui_tokens::Easing::OUT);
/// The text's inset from the edge.
const FIELD_PAD_X: f32 = 8.0;
const FIELD_PAD_Y: f32 = 4.0;

/// The edge colour for a field that is (or is not) `focused`, tweened.
pub fn field_edge(id: impl Into<ElementId>, focused: bool, window: &mut Window, cx: &mut App) -> Hsla {
    let p = cx.aui().colors;
    let target = p.ink.alpha(if focused { FIELD_EDGE_FOCUSED } else { FIELD_EDGE_REST });
    tween(id.into(), target, FIELD_EDGE_FADE, window, cx)
}

/// A text field's frame for `state`: radius 6, the inset ground, the
/// [`field_edge`] read from the state's own focus handle (no host reports
/// focus). Add the kit editor (`appearance(false)`, `bordered(false)`) as
/// its child.
pub fn field_frame<M: InputModeKind>(
    id: impl Into<ElementId>,
    state: &Entity<InputBaseState<M>>,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    use gpui::Focusable as _;
    let id: ElementId = id.into();
    let focused = state.read(cx).focus_handle(cx).contains_focused(window, cx);
    let edge = field_edge((id.clone(), "edge"), focused, window, cx);
    let p = cx.aui().colors;
    div()
        .id(id)
        .px(px(FIELD_PAD_X))
        .py(px(FIELD_PAD_Y))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(edge)
        .bg(p.surface_1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The edge is the composer's: foreground 8 % at rest, 22 % with the
    /// keyboard — never the accent (the kit's blue ring the owner flagged).
    /// Draws nothing; reads the two edges inside a render, where tweens live.
    struct Probe(std::rc::Rc<std::cell::Cell<Option<(Hsla, Hsla)>>>);

    impl gpui::Render for Probe {
        fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            self.0.set(Some((field_edge("rest", false, window, cx), field_edge("focused", true, window, cx))));
            div()
        }
    }

    #[gpui::test]
    fn the_edge_brightens_with_focus_and_is_never_the_accent(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            crate::init(aui_tokens::ThemeKind::Dark, cx);
            cx.set_reduce_motion(true);
        });
        let seen = std::rc::Rc::new(std::cell::Cell::new(None));
        let held = seen.clone();
        let (_view, vc) = cx.add_window_view(move |_, _| Probe(held));
        vc.run_until_parked();
        let (rest, focused) = seen.get().expect("the probe rendered");
        let p = vc.update(|_, cx| cx.aui().colors);
        assert!((rest.a - FIELD_EDGE_REST).abs() < 1e-3, "{rest:?}");
        assert!((focused.a - FIELD_EDGE_FOCUSED).abs() < 1e-3, "{focused:?}");
        assert_eq!((focused.h, focused.s, focused.l), (p.ink.h, p.ink.s, p.ink.l), "the foreground, not a hue");
        assert_ne!(focused, p.accent);
        assert_ne!(focused, p.accent_ring);
    }
}
