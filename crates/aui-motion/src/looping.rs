//! A repeating 0→1 phase for spinners, blinking carets and other ambient
//! loops. Requests a frame every render while the loop runs; under reduced
//! motion it holds at the resting phase.

use std::time::Duration;

use aui_tokens::Easing;
use gpui::{App, Window};
use gpui_kit::base::{animate_keyframes, IterationCount, Keyframe, Keyframes, TransitionId};
use gpui_kit::base::{PlaybackDirection, Timing};

/// Loop parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loop {
    /// One cycle.
    pub period: Duration,
    /// Easing per cycle (`None` = linear).
    pub easing: Option<Easing>,
    /// Ping-pong instead of restarting.
    pub alternate: bool,
    /// Phase reported under reduced motion.
    pub resting: f32,
}

impl Loop {
    /// A linear loop.
    pub const fn linear(period: Duration) -> Self {
        Self { period, easing: None, alternate: false, resting: 0.0 }
    }

    /// An eased loop.
    pub const fn eased(period: Duration, easing: Easing) -> Self {
        Self { period, easing: Some(easing), alternate: false, resting: 0.0 }
    }

    /// Ping-pong.
    pub const fn alternate(mut self) -> Self {
        self.alternate = true;
        self
    }

    /// The value to hold under reduced motion.
    pub const fn resting(mut self, resting: f32) -> Self {
        self.resting = resting;
        self
    }
}

/// The current phase in `0..=1` of a loop keyed by `id`.
pub fn looping(id: impl Into<TransitionId>, config: Loop, window: &mut Window, cx: &mut App) -> f32 {
    if cx.reduce_motion() {
        return config.resting;
    }
    let frames = Keyframes::try_new([Keyframe::new(0.0, 0.0f32), Keyframe::new(1.0, 1.0f32)])
        .expect("two endpoint keyframes are valid");
    let mut timing = Timing::new(config.period).iterations(IterationCount::Infinite);
    if config.alternate {
        timing = timing.direction(PlaybackDirection::Alternate);
    }
    if let Some(easing) = config.easing {
        timing = timing.ease(gpui_kit::base::Easing::Custom(std::rc::Rc::new(move |t| easing.sample(t))));
    }
    animate_keyframes(id, &frames, timing, window, cx).value
}
