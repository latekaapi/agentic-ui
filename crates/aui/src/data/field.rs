//! The text field's frame — the one input look (docs/83 §7): `surface-4`
//! fill, a 1 px `line-strong` edge that turns `accent-ring` while the field
//! holds the keyboard, radius 8, at least 32 tall. No glow, no tween: the
//! edge is the whole focus treatment.
//!
//! Wrap the kit's editor with `appearance(false)` and `bordered(false)` in
//! [`field_frame`], so the frame is the only edge drawn. [`field`] is the
//! same frame for a host that knows the focus itself (a gallery card, a
//! static capture).

use aui_tokens::ActiveAui;
use gpui::{div, prelude::*, px, App, Div, ElementId, Entity, Stateful, Window};
use gpui_kit::base::input::{InputBaseState, InputModeKind};

use crate::interact::R_ITEM;

/// The field's minimum height (docs/83 §7: 32). A multi-line field grows.
/// No token yet; task 2.3 moves it into the metrics.
pub const FIELD_H: f32 = 32.0;
/// The text's inset from the edge.
const FIELD_PAD_X: f32 = 8.0;
const FIELD_PAD_Y: f32 = 4.0;

/// The input frame keyed `id`, its edge `accent-ring` when `focused`.
pub fn field(id: impl Into<ElementId>, focused: bool, cx: &App) -> Stateful<Div> {
    let p = cx.aui().colors;
    div()
        .id(id.into())
        .flex()
        .items_center()
        .min_h(px(FIELD_H))
        .px(px(FIELD_PAD_X))
        .py(px(FIELD_PAD_Y))
        .rounded(px(R_ITEM))
        .border_1()
        .border_color(if focused { p.accent_ring } else { p.line_strong })
        .bg(p.surface_4)
}

/// [`field`] for `state`, its focus read from the state's own focus handle
/// (no host reports focus). Add the kit editor (`appearance(false)`,
/// `bordered(false)`) as its child.
pub fn field_frame<M: InputModeKind>(
    id: impl Into<ElementId>,
    state: &Entity<InputBaseState<M>>,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    use gpui::Focusable as _;
    let focused = state.read(cx).focus_handle(cx).contains_focused(window, cx);
    field(id, focused, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The edge is `line-strong` at rest and `accent-ring` with the
    /// keyboard (docs/83 §7) — never the old tweened foreground hairline.
    type Edges = (Option<gpui::Hsla>, Option<gpui::Hsla>);
    struct Probe(std::rc::Rc<std::cell::Cell<Option<Edges>>>);

    impl gpui::Render for Probe {
        fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let mut rest = field("rest", false, cx);
            let mut focused = field("focused", true, cx);
            self.0.set(Some((rest.style().border_color, focused.style().border_color)));
            div()
        }
    }

    #[gpui::test]
    fn the_edge_is_line_strong_at_rest_and_the_accent_ring_with_focus(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(aui_tokens::ThemeKind::Dark, cx));
        let seen = std::rc::Rc::new(std::cell::Cell::new(None));
        let held = seen.clone();
        let (_view, vc) = cx.add_window_view(move |_, _| Probe(held));
        vc.run_until_parked();
        let (rest, focused) = seen.get().expect("the probe rendered");
        let p = vc.update(|_, cx| cx.aui().colors);
        assert_eq!(rest, Some(p.line_strong));
        assert_eq!(focused, Some(p.accent_ring));
    }
}
