//! Shake: the ±4 px horizontal error shake (500 ms, std easing) that a
//! denied action or a failed send plays once.

use std::time::Duration;

use gpui::{px, App, ElementId, Pixels, Window};
use gpui_kit::base::{animate_keyframes, Keyframe, Keyframes, Timing};

/// One shake.
pub const DURATION: Duration = Duration::from_millis(500);

/// The horizontal offset to apply (as a relative `left`) for shake number
/// `generation` under `id`. Bump the generation to play again; the offset
/// returns to 0 when the shake ends. Reduced motion never moves.
pub fn shake_offset(id: impl Into<ElementId>, generation: u32, window: &mut Window, cx: &mut App) -> Pixels {
    if generation == 0 {
        return px(0.0);
    }
    let frames = Keyframes::try_new([
        Keyframe::new(0.0, 0.0f32),
        Keyframe::new(0.2, -4.0),
        Keyframe::new(0.4, 4.0),
        Keyframe::new(0.6, -3.0),
        Keyframe::new(0.8, 3.0),
        Keyframe::new(1.0, 0.0),
    ])
    .expect("shake keyframes are valid");
    let id: ElementId = id.into();
    let value = animate_keyframes((crate::child_id(id, generation as usize), "shake"), &frames, Timing::new(DURATION), window, cx).value;
    px(value)
}
