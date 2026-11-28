//! The window drag region: press-drag moves the window, double-click zooms
//! (card 10's header; mirrors gpui-component's `TitleBar`, `title_bar.rs`
//! ~318-360).
//!
//! ```ignore
//! drag_region(("app", "header-drag")).child(
//!     centre_header("hd-centre", "checkout-flow-v2") /* … */
//! )
//! ```
//!
//! A left press arms the region; the next move calls
//! `window.start_window_move()`, which hands the gesture to the window
//! manager. A double-click (a click whose `click_count` is 2, the same test
//! gpui-component's `on_double_click` uses) calls
//! `window.titlebar_double_click()` on macOS and `window.zoom_window()`
//! everywhere else. [`crate::shell::AppShell`] wraps its header row in one by default (see
//! [`crate::shell::AppShell::draggable`]).
//!
//! Interactive children keep their clicks: pointer events hit the innermost
//! element first, so a click on a button reaches the button — only a press
//! *followed by motion* moves the window, exactly like `TitleBar`. Children
//! that call `cx.stop_propagation()` on mouse-down opt out of arming entirely,
//! and so does a press a text field takes (it suppresses the window's text
//! selection for that press, `GlobalState::suppress_text_selection`): a drag
//! that starts in a field selects its text rather than moving the window
//! (live 2026-11-28, the maximised pane's address field).

use gpui::{div, prelude::*, App, ElementId, IntoElement, MouseButton, Window};
use gpui_kit::base::GlobalState;

/// A window drag region over its children. Build with [`drag_region`].
#[derive(IntoElement)]
pub struct DragRegion {
    id: ElementId,
    enabled: bool,
    children: Vec<gpui::AnyElement>,
}

/// Wraps a header row so press-drag moves the window and double-click zooms.
///
/// The wrapper is unstyled (`w_full`, height from content), so wrapping never
/// moves pixels: on/off screenshots are identical.
pub fn drag_region(id: impl Into<ElementId>) -> DragRegion {
    DragRegion { id: id.into(), enabled: true, children: Vec::new() }
}

impl DragRegion {
    /// Whether press-drag and double-click are armed. On by default; pass
    /// false to keep the (layout-identical) wrapper without the behaviour.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Adds wrapped content (usually one header row).
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.children.push(el.into_any_element());
        self
    }
}

impl RenderOnce for DragRegion {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // One armed flag per region, keyed by element id: `use_state` alone is
        // keyed by call site, which every region shares.
        let armed = window.use_keyed_state((self.id.clone(), "drag-armed"), cx, |_, _| false);
        let region = div().id(self.id).w_full().flex_none().children(self.children);
        if !self.enabled {
            return region;
        }
        let press = armed.clone();
        let release = armed.clone();
        let release_out = armed.clone();
        let motion = armed.clone();
        region
            .on_mouse_down_out(move |_, _, cx: &mut App| {
                release_out.update(cx, |armed, _| *armed = false);
            })
            // A field in front takes the press first and marks it as its own
            // (`suppress_text_selection`); the mark is reset here at capture
            // so one left by an earlier press never holds the region back.
            .capture_any_mouse_down(|_, _, cx: &mut App| {
                if cx.has_global::<GlobalState>() {
                    GlobalState::reset_text_selection_suppression(cx);
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx: &mut App| {
                let field_took_it = cx.has_global::<GlobalState>() && GlobalState::is_text_selection_suppressed(cx);
                press.update(cx, |armed, _| *armed = !field_took_it);
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx: &mut App| {
                release.update(cx, |armed, _| *armed = false);
            })
            .on_mouse_move(move |_, window: &mut Window, cx: &mut App| {
                if *motion.read(cx) {
                    motion.update(cx, |armed, _| *armed = false);
                    window.start_window_move();
                }
            })
            // No `on_double_click` on this gpui: a click with `click_count`
            // 2 is the double-click, the same test gpui-component's helper
            // uses. Single clicks (all the header buttons) fall through.
            .on_click(move |event, window: &mut Window, cx: &mut App| {
                // A double-click a field took selects a word there.
                let field_took_it = cx.has_global::<GlobalState>() && GlobalState::is_text_selection_suppressed(cx);
                if event.click_count() == 2 && !field_took_it {
                    if cfg!(target_os = "macos") {
                        window.titlebar_double_click();
                    } else {
                        window.zoom_window();
                    }
                }
            })
    }
}
