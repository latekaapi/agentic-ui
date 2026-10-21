//! Small helpers shared by the components: per-element interaction state
//! (hover / press) kept in the window, keyed by element id.

use gpui::{App, ElementId, Entity, Window};

/// Hover and press flags for one interactive element. Lives in the window's
/// element state, so stateless (`RenderOnce`) components can animate hover
/// tints and press springs without owning a view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Interaction {
    /// The pointer is over the element.
    pub hovered: bool,
    /// The primary button is down on the element.
    pub pressed: bool,
}

/// The interaction state for `id`, created on first use.
pub fn interaction(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Entity<Interaction> {
    let id: ElementId = id.into();
    window.use_keyed_state((id, "aui-interaction"), cx, |_, _| Interaction::default())
}

/// Reads the current flags without keeping the entity.
pub fn interaction_flags(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> (Entity<Interaction>, Interaction) {
    let state = interaction(id, window, cx);
    let flags = *state.read(cx);
    (state, flags)
}

/// Wires hover / press tracking into a stateful element.
pub trait TrackInteraction: gpui::StatefulInteractiveElement + gpui::InteractiveElement + Sized {
    /// Updates `state` from hover, mouse-down and mouse-up events.
    fn track_interaction(self, state: &Entity<Interaction>) -> Self {
        self.track_interaction_reported(state, |_, _, _| {})
    }

    /// Like [`Self::track_interaction`], plus a hover report: `report`
    /// fires on every hover enter/leave beside the state update, so a caller
    /// that owns hover-driven UI (a detail card's delay arm) learns about
    /// the pointer even when nothing re-renders. A gpui element carries a
    /// single `on_hover` slot, so the report must share it rather than add
    /// a second handler.
    fn track_interaction_reported(
        self,
        state: &Entity<Interaction>,
        report: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        let hover = state.clone();
        let down = state.clone();
        let up = state.clone();
        let up_out = state.clone();
        self.on_hover(move |hovered, window, cx| {
            report(*hovered, window, cx);
            hover.update(cx, |s, cx| {
                if s.hovered != *hovered {
                    s.hovered = *hovered;
                    if !*hovered {
                        s.pressed = false;
                    }
                    cx.notify();
                }
            })
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            down.update(cx, |s, cx| {
                s.pressed = true;
                cx.notify();
            })
        })
        .on_mouse_up(gpui::MouseButton::Left, move |_, _, cx| {
            up.update(cx, |s, cx| {
                s.pressed = false;
                cx.notify();
            })
        })
        .on_mouse_up_out(gpui::MouseButton::Left, move |_, _, cx| {
            up_out.update(cx, |s, cx| {
                if s.pressed {
                    s.pressed = false;
                    cx.notify();
                }
            })
        })
    }
}

impl<T: gpui::StatefulInteractiveElement + gpui::InteractiveElement + Sized> TrackInteraction for T {}

/// A click handler stored by a component builder.
pub type ClickHandler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;

// ------------------------------------------------------- element id building

/// Appends `n`'s decimal digits to `buf` without the formatting machinery.
///
/// Element ids are rebuilt for every child on every frame, so the components
/// build them from a single `String` with a known capacity instead of going
/// through `format!`.
pub(crate) fn push_usize(buf: &mut String, n: usize) {
    // 20 digits is `usize::MAX` on a 64-bit target.
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    let mut rest = n;
    loop {
        at -= 1;
        digits[at] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    // Every byte written is an ASCII digit.
    buf.push_str(std::str::from_utf8(&digits[at..]).unwrap_or_default());
}

/// `parent` / `{prefix}{index}`: the id of an indexed child, built without a
/// `format!`. The name is the same string `format!("{prefix}{index}")` would
/// produce, so element state keyed by it is unchanged.
pub(crate) fn indexed_child(parent: &ElementId, prefix: &str, index: usize) -> ElementId {
    let mut name = String::with_capacity(prefix.len() + 4);
    name.push_str(prefix);
    push_usize(&mut name, index);
    (parent.clone(), gpui::SharedString::from(name)).into()
}

/// `parent` / `{prefix}{name}`: the id of a child keyed by a string of its
/// own (a row id, a chip id), built without a `format!`.
pub(crate) fn named_child(parent: &ElementId, prefix: &str, name: &str) -> ElementId {
    let mut key = String::with_capacity(prefix.len() + name.len());
    key.push_str(prefix);
    key.push_str(name);
    (parent.clone(), gpui::SharedString::from(key)).into()
}

/// A block that sits *in* the sheet — a code block, a table, a tool card:
/// v3 (dark) paints it **Raised** with the one edge (canvas "Baaz v3",
/// docs/31a §0); light keeps FF's look, `light_bg` with the 1 px hairline,
/// until light is designed.
pub(crate) fn raised_block<E: gpui::Styled>(el: E, p: &aui_tokens::Palette, light_bg: gpui::Hsla) -> E {
    if p.is_dark() {
        // A transparent 1 px border keeps the box FF measured (content
        // 1 px in); the inset edge paints over it.
        el.border_1().border_color(gpui::transparent_black()).bg(p.surface_3).shadow(p.elevation_shadow(3))
    } else {
        el.border_1().border_color(p.line).bg(light_bg)
    }
}

/// The lit band of a raised block (a code block's header, a table's head):
/// v3's **Raised, lit** in dark; `light` in light.
pub(crate) fn raised_lit(p: &aui_tokens::Palette, light: gpui::Hsla) -> gpui::Hsla {
    if p.is_dark() { p.surface_4 } else { light }
}

/// Lock a scroll container's wheel to the axis it scrolls on, one axis per
/// gesture: gpui's opt-in `restrict_scroll_to_axis`.
///
/// Without it gpui moves a container that scrolls only vertically by a
/// wheel's HORIZONTAL delta whenever the vertical one is zero, and lets
/// both deltas of a diagonal gesture through. Owner 2026-10-21: "even moving
/// sideways on the trackpad causes the pane to slightly scroll up and
/// down" (the right pane's Markdown preview) — a sideways swipe (the pane's
/// Shift-swipe screens gesture without Shift) scrolled the content.
pub trait ScrollAxisLock: gpui::Styled + Sized {
    /// See the trait.
    fn scroll_axis_locked(mut self) -> Self {
        self.style().restrict_scroll_to_axis = Some(true);
        self
    }
}

impl<T: gpui::Styled> ScrollAxisLock for T {}

#[cfg(test)]
mod id_tests {
    use super::*;

    #[test]
    fn indexed_and_named_children_match_the_formatted_names() {
        let parent: ElementId = "p".into();
        assert_eq!(
            indexed_child(&parent, "row-", 12),
            (parent.clone(), gpui::SharedString::from(format!("row-{}", 12))).into()
        );
        assert_eq!(
            indexed_child(&parent, "li", 0),
            (parent.clone(), gpui::SharedString::from(format!("li{}", 0))).into()
        );
        assert_eq!(
            named_child(&parent, "chip-x-", "abc"),
            (parent.clone(), gpui::SharedString::from(format!("chip-x-{}", "abc"))).into()
        );
    }

    #[test]
    fn digits_round_trip() {
        for n in [0usize, 1, 9, 10, 99, 100, 1234, usize::MAX] {
            let mut buf = String::new();
            push_usize(&mut buf, n);
            assert_eq!(buf, n.to_string());
        }
    }

    mod axis_lock {
        use super::super::ScrollAxisLock;
        use gpui::{div, point, prelude::*, px, Context, ScrollDelta, ScrollHandle, ScrollWheelEvent, TouchPhase, Window};

        struct Host {
            scroll: ScrollHandle,
            locked: bool,
        }

        impl Render for Host {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let list = div().id("list").size_full().overflow_y_scroll().track_scroll(&self.scroll).child(div().h(px(4000.0)).w_full());
                let list = if self.locked { list.scroll_axis_locked() } else { list };
                div().size_full().child(list)
            }
        }

        fn sideways(vc: &mut gpui::VisualTestContext, dx: f32, dy: f32) {
            vc.simulate_event(ScrollWheelEvent {
                position: point(px(100.0), px(100.0)),
                delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            });
            vc.run_until_parked();
        }

        /// A sideways trackpad swipe must not move a vertical list.
        #[gpui::test]
        fn a_sideways_swipe_leaves_a_vertical_list_where_it_is(cx: &mut gpui::TestAppContext) {
            for locked in [false, true] {
                let scroll = ScrollHandle::new();
                let handle = scroll.clone();
                let (_view, vc) = cx.add_window_view(move |_, _| Host { scroll: handle, locked });
                vc.run_until_parked();
                sideways(vc, -60.0, 0.0);
                let moved = scroll.offset().y != px(0.0);
                // gpui's own default moves it — the defect — and the lock keeps it still.
                assert_eq!(moved, !locked, "locked {locked}: offset {:?}", scroll.offset());
            }
        }
    }
}
