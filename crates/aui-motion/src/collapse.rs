//! Height 0 ↔ auto on the layout spring: thinking traces, activity
//! timelines, tool-card bodies, sidebar groups.
//!
//! The reveal below is gpui-base's `MotionReveal` with one change: on the
//! first frame, before any height has been measured, the child is laid out as
//! a real layout child with the node's height left auto, so the node opens at
//! the child's natural height instead of 0. A quiet window may never paint a
//! second frame, and the old element needed one before it drew any rows.

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels,
    Style, Window, px, relative, size,
};

use crate::spring::{spring_phase, SpringKind};

/// Wraps `child` in a measured, clipped reveal whose height follows the
/// layout spring from 0 (closed) to the child's natural height (open). The
/// child stays mounted while closed so its state survives; the caller may
/// skip rendering it entirely once the reveal has settled closed.
///
/// Returns the element and the current progress (0..=1) so the caller can
/// fade content or rotate a chevron in step.
pub fn collapse(id: impl Into<ElementId>, open: bool, child: AnyElement, window: &mut Window, cx: &mut App) -> (Reveal, f32) {
    let id: ElementId = id.into();
    let progress = spring_phase((id.clone(), "collapse"), open, SpringKind::Layout, window, cx).clamp(0.0, 1.0);
    (Reveal::new((id, "reveal"), progress, child), progress)
}

/// A measured, clipped vertical reveal driven by normalized progress.
///
/// Behaves like gpui-base's `MotionReveal`, except the unmeasured first frame
/// of an opening reveal lays the child out as a real layout child: the node
/// is the child's natural height on frame one (full height when open; when
/// opening mid-animation that one frame is still full height, the same
/// one-frame overshoot the old element had on close). A reveal that is closed
/// before its first measure stays height 0 without mounting the child, so a
/// closed collapse never flashes its content for a frame. Once a height is
/// stored, layout measures the child as a root and clips to
/// `height * progress`, exactly as before.
pub struct Reveal {
    id: ElementId,
    progress: f32,
    child: AnyElement,
}

#[derive(Clone, Copy, Default)]
struct RevealState {
    height: Option<Pixels>,
}

impl Reveal {
    /// A reveal of `child` at normalized `progress` (clamped to 0..=1).
    pub fn new(id: impl Into<ElementId>, progress: f32, child: AnyElement) -> Self {
        Self {
            id: id.into(),
            progress: progress.clamp(0.0, 1.0),
            child,
        }
    }
}

impl IntoElement for Reveal {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Reveal {
    type RequestLayoutState = Option<LayoutId>;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let height = window.with_element_state(
            global_id.expect("Reveal must have an id"),
            |state: Option<RevealState>, _| {
                let state = state.unwrap_or_default();
                (state.height, state)
            },
        );
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        match height {
            // Unmeasured and opening (or open): the child joins layout for
            // real and the node's height stays auto, so frame one is already
            // the child's natural height. The child id travels to prepaint in
            // the layout state.
            None if self.progress > 0.0 => {
                let child = self.child.request_layout(window, cx);
                return (window.request_layout(style, [child], cx), Some(child));
            }
            // Closed before the first measure: height 0, no child in layout.
            None => style.size.height = px(0.0).into(),
            Some(height) => style.size.height = (height * self.progress).into(),
        }
        (window.request_layout(style, None, cx), None)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // First frame of an opening reveal: the child's height is already in
        // the layout tree, so read it back, remember it, and paint the child
        // at its laid-out place with no mask — full height on frame one.
        if let Some(child_id) = *child_layout {
            let measured = window.layout_bounds(child_id).size.height;
            let changed = window.with_element_state(
                global_id.expect("Reveal must have an id"),
                |state: Option<RevealState>, _| {
                    let mut state = state.unwrap_or_default();
                    let changed = state.height != Some(measured);
                    state.height = Some(measured);
                    (changed, state)
                },
            );
            if changed {
                window.request_animation_frame();
            }
            self.child.prepaint(window, cx);
            return;
        }
        let measured = self.child.layout_as_root(
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::MinContent,
            ),
            window,
            cx,
        );
        let changed = window.with_element_state(
            global_id.expect("Reveal must have an id"),
            |state: Option<RevealState>, _| {
                let mut state = state.unwrap_or_default();
                let changed = state.height != Some(measured.height);
                state.height = Some(measured.height);
                (changed, state)
            },
        );
        if changed {
            window.request_animation_frame();
        }
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.child.prepaint_at(bounds.origin, window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.child.paint(window, cx);
        });
    }
}
