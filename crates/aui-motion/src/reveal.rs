//! Stream reveal: each newly arrived chunk of assistant text (or each new
//! timeline row) fades in and rises 3 px over the base duration.

use gpui::{px, App, ElementId, Pixels, Window};

use crate::presence::{presence, EnterExit};

/// The style for one revealed chunk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RevealSample {
    /// 0 → 1.
    pub opacity: f32,
    /// 3 px → 0 (apply as a relative `top`).
    pub offset_y: Pixels,
}

/// Samples the reveal for chunk `index` under `id`. A chunk that was already
/// present when its element mounted (e.g. history) can pass `settled = true`
/// to skip the animation.
pub fn stream_reveal(id: impl Into<ElementId>, index: usize, settled: bool, window: &mut Window, cx: &mut App) -> RevealSample {
    if settled {
        return RevealSample { opacity: 1.0, offset_y: px(0.0) };
    }
    let id: ElementId = id.into();
    // Keyed state adopts the first value it sees, so the chunk mounts as
    // "absent" (progress 0) and is asked to be present on the same frame.
    let chunk = crate::child_id(id, index);
    let _ = presence((chunk.clone(), "reveal-seed"), false, EnterExit::BASE, window, cx);
    let sample = presence((chunk, "reveal"), true, EnterExit::BASE, window, cx);
    let p = sample.progress.clamp(0.0, 1.0);
    RevealSample { opacity: p, offset_y: px(3.0 * (1.0 - p)) }
}
