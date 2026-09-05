//! Height 0 ↔ auto on the layout spring: thinking traces, activity
//! timelines, tool-card bodies, sidebar groups.

use gpui::{AnyElement, App, ElementId, Window};
use gpui_kit::base::MotionReveal;

use crate::spring::{spring_phase, SpringKind};

/// Wraps `child` in a measured, clipped reveal whose height follows the
/// layout spring from 0 (closed) to the child's natural height (open). The
/// child stays mounted while closed so its state survives; the caller may
/// skip rendering it entirely once the reveal has settled closed.
///
/// Returns the element and the current progress (0..=1) so the caller can
/// fade content or rotate a chevron in step.
pub fn collapse(id: impl Into<ElementId>, open: bool, child: AnyElement, window: &mut Window, cx: &mut App) -> (MotionReveal, f32) {
    let id: ElementId = id.into();
    let progress = spring_phase((id.clone(), "collapse"), open, SpringKind::Layout, window, cx).clamp(0.0, 1.0);
    (MotionReveal::new((id, "reveal"), progress, child), progress)
}
