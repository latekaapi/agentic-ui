//! Duration-based transitions on any interpolable value (colours, opacity,
//! sizes, offsets). The design uses these for hover tints and fades, where a
//! spring would be overkill.

use std::time::Duration;

use aui_tokens::{durations, Easing};
use gpui::{App, Window};
use gpui_kit::base::{Interpolate, Transition, TransitionId};

/// A named timing policy: duration + easing (+ optional delay).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    /// How long the value takes to reach its target.
    pub duration: Duration,
    /// The easing curve.
    pub easing: Easing,
    /// Delay before the value starts moving.
    pub delay: Duration,
}

impl Tween {
    /// 120 ms, std easing — hover tints.
    pub const FAST: Tween = Tween::new(durations::FAST, Easing::STD);
    /// 180 ms, std easing — toggles, tabs.
    pub const BASE: Tween = Tween::new(durations::BASE, Easing::STD);
    /// 220 ms, ease-out — cards and menus entering.
    pub const ENTER: Tween = Tween::new(durations::ENTER, Easing::OUT);
    /// 160 ms, std easing — dismissals.
    pub const EXIT: Tween = Tween::new(durations::EXIT, Easing::STD);
    /// 280 ms, ease-out — panel width.
    pub const SLOW: Tween = Tween::new(durations::SLOW, Easing::OUT);

    /// A custom policy.
    pub const fn new(duration: Duration, easing: Easing) -> Self {
        Self { duration, easing, delay: Duration::ZERO }
    }

    /// Adds a start delay (used by staggers).
    pub const fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// The equivalent gpui-base policy.
    pub fn policy(self) -> Transition {
        let easing = self.easing;
        Transition::new(self.duration)
            .delay(self.delay)
            .ease(move |t| easing.sample(t))
    }
}

/// Transitions a value toward `target`. The first call adopts the target;
/// later target changes start from the value sampled at that moment, and a
/// reversal mid-flight takes proportionally less time. Reduced motion jumps.
pub fn tween<T>(id: impl Into<TransitionId>, target: T, policy: Tween, window: &mut Window, cx: &mut App) -> T
where
    T: Interpolate + PartialEq + 'static,
{
    gpui_kit::base::transition(id, target, policy.policy(), window, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match_the_design_durations() {
        assert_eq!(Tween::FAST.duration.as_millis(), 120);
        assert_eq!(Tween::BASE.duration.as_millis(), 180);
        assert_eq!(Tween::ENTER.duration.as_millis(), 220);
        assert_eq!(Tween::EXIT.duration.as_millis(), 160);
        assert_eq!(Tween::SLOW.duration.as_millis(), 280);
        assert_eq!(Tween::ENTER.easing, Easing::OUT);
    }
}
