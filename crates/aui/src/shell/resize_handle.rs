//! The sidebar resize handle: a 6 px transparent strip over the divider, plus
//! the full-window capture overlay used mid-drag (card 10, spec §1.1).
//!
//! The app owns the numbers (`width`, `resizing`, `grab_x`, `start_w` — keep
//! them in the view or in window state, and persist the settled width with the
//! same store pattern as the sessions file); the shell owns the strip:
//!
//! ```ignore
//! // State: `width`, `resizing`, `grab_x`, `start_w`.
//! div().relative().child(
//!     app_shell("app")
//!         .sidebar_width(px(width))
//!         .resizing(resizing)
//!         .sidebar(sidebar)
//!         .centre(centre),
//! ).child(
//!     // Pin the strip to the divider: 6 px wide, centred on the edge.
//!     div().absolute().top(px(0.0)).bottom(px(0.0)).left(px(width - 3.0)).child(
//!         resize_handle(("app", "sidebar-resize"))
//!             .on_drag_start(|x, _, _| { /* resizing = true; grab_x = x; start_w = width; */ })
//!             .on_drag(|x, _, _| { /* width = clamp_sidebar_width(start_w + (x - grab_x)); */ })
//!             .on_drag_end(|_, _| { /* resizing = false; persist width; */ }),
//!     ),
//! )
//! // While `resizing`, render the capture overlay at the root: the move event
//! // fires only while hovered, so once the pointer outruns the 6 px strip the
//! // handle alone would go silent and the drag would stall.
//! if resizing {
//!     drag_capture_overlay(("app", "resize-capture"))
//!         .on_drag(|x, _, _| { /* width = clamp_sidebar_width(start_w + (x - grab_x)); */ })
//!         .on_drag_end(|_, _| { /* resizing = false; persist width; */ })
//! }
//! ```
//!
//! While `resizing` is true [`crate::shell::AppShell`] feeds the width
//! straight through and skips the layout spring, so the divider tracks the
//! pointer; when the drag ends the spring re-arms from the current width and
//! the pane settles with no jump. Clamp every move to
//! [`crate::shell::SIDEBAR_MIN_WIDTH`]..=[`crate::shell::SIDEBAR_MAX_WIDTH`]
//! (see [`crate::shell::clamp_sidebar_width`]); also clear `resizing` on
//! window blur, in case the release happens outside the window.

use std::rc::Rc;

use gpui::{div, prelude::*, px, App, CursorStyle, ElementId, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, Window};

/// Width of the resize strip: wide enough to grab, transparent so the divider
/// beneath it keeps its own paint.
pub const RESIZE_HANDLE_W: f32 = 6.0;

type DragPositionHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type DragEndHandler = Rc<dyn Fn(&mut Window, &mut App)>;

fn drag_x(event: &MouseMoveEvent) -> f32 {
    f32::from(event.position.x)
}

fn down_x(event: &MouseDownEvent) -> f32 {
    f32::from(event.position.x)
}

/// The resize strip over the sidebar/centre divider. Build with
/// [`resize_handle`].
#[derive(IntoElement)]
pub struct ResizeHandle {
    id: ElementId,
    on_drag_start: Option<DragPositionHandler>,
    on_drag: Option<DragPositionHandler>,
    on_drag_end: Option<DragEndHandler>,
}

/// A 6 px transparent strip, full height, with the horizontal-resize cursor.
///
/// The caller positions it over the divider (see the module docs); the strip
/// itself only reports pointer intents.
pub fn resize_handle(id: impl Into<ElementId>) -> ResizeHandle {
    ResizeHandle { id: id.into(), on_drag_start: None, on_drag: None, on_drag_end: None }
}

impl ResizeHandle {
    /// The left button went down on the strip; the argument is the grab x in
    /// window pixels. Arm the drag and remember `grab_x` and `start_w`.
    pub fn on_drag_start(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_drag_start = Some(Rc::new(f));
        self
    }

    /// The pointer moved with the left button held; the argument is the
    /// current x in window pixels. Set `width = clamp(start_w + (x - grab_x))`.
    pub fn on_drag(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_drag = Some(Rc::new(f));
        self
    }

    /// The button was released (inside or outside the strip). Disarm the drag
    /// and persist the width.
    pub fn on_drag_end(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_drag_end = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ResizeHandle {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut strip = div()
            .id(self.id)
            .w(px(RESIZE_HANDLE_W))
            .h_full()
            .flex_none()
            .bg(gpui::transparent_black())
            .cursor(CursorStyle::ResizeLeftRight);
        if let Some(h) = self.on_drag_start {
            strip = strip.on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                h(down_x(event), window, cx);
                cx.stop_propagation();
            });
        }
        if let Some(h) = self.on_drag {
            strip = strip.on_mouse_move(move |event: &MouseMoveEvent, window: &mut Window, cx: &mut App| {
                if event.dragging() {
                    h(drag_x(event), window, cx);
                    cx.stop_propagation();
                }
            });
        }
        if let Some(h) = self.on_drag_end {
            let outside = h.clone();
            strip = strip
                .on_mouse_up(MouseButton::Left, move |_, window: &mut Window, cx: &mut App| {
                    h(window, cx);
                })
                .on_mouse_up_out(MouseButton::Left, move |_, window: &mut Window, cx: &mut App| {
                    outside(window, cx);
                });
        }
        strip
    }
}

/// The full-window capture layer for an in-flight resize drag. Build with
/// [`drag_capture_overlay`].
#[derive(IntoElement)]
pub struct DragCaptureOverlay {
    id: ElementId,
    on_drag: Option<DragPositionHandler>,
    on_drag_end: Option<DragEndHandler>,
}

/// A transparent layer over the window that forwards every move and the
/// release to the drag intents. Render it at the root while `resizing` (see
/// the module docs) so the drag survives the pointer leaving the strip; it
/// swallows clicks until the release ends the drag.
pub fn drag_capture_overlay(id: impl Into<ElementId>) -> DragCaptureOverlay {
    DragCaptureOverlay { id: id.into(), on_drag: None, on_drag_end: None }
}

impl DragCaptureOverlay {
    /// A move anywhere in the window; same update as the handle's `on_drag`.
    pub fn on_drag(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_drag = Some(Rc::new(f));
        self
    }

    /// The button was released; same teardown as the handle's `on_drag_end`.
    pub fn on_drag_end(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_drag_end = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for DragCaptureOverlay {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut layer = div()
            .id(self.id)
            .absolute()
            .inset_0()
            .bg(gpui::transparent_black())
            .occlude()
            .cursor(CursorStyle::ResizeLeftRight);
        if let Some(h) = self.on_drag {
            layer = layer.on_mouse_move(move |event: &MouseMoveEvent, window: &mut Window, cx: &mut App| {
                h(drag_x(event), window, cx);
                cx.stop_propagation();
            });
        }
        if let Some(h) = self.on_drag_end {
            let outside = h.clone();
            layer = layer
                .on_mouse_up(MouseButton::Left, move |_, window: &mut Window, cx: &mut App| {
                    h(window, cx);
                })
                .on_mouse_up_out(MouseButton::Left, move |_, window: &mut Window, cx: &mut App| {
                    outside(window, cx);
                });
        }
        layer
    }
}
