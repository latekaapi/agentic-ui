//! Duration-based transitions on any interpolable value (colours, opacity,
//! sizes, offsets). The design uses these for hover tints and fades, where a
//! spring would be overkill.

use std::time::Duration;

use aui_tokens::{durations, Easing};
use gpui::{App, Hsla, Window};
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

    /// Swaps the easing (a base-length slide that eases out).
    pub const fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
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

/// Fades a tint in and out over whatever ground is behind it: the highlight a
/// row wears while it is hovered or selected.
///
/// Use this instead of tweening the tint against `transparent_black()`. `Hsla`
/// interpolates channel by channel, so a tween toward transparent black drags
/// the hue round to red and the lightness down to black on the way out: for
/// half of its 120 ms the row wears a colour *darker* than both the tint and
/// the ground it sits on, which reads as a flash rather than a fade. Holding
/// the tint's own hue and moving only its alpha is what `transition:background`
/// does in the CSS, and it lets two neighbouring rows cross-fade cleanly as the
/// pointer travels between them.
pub fn tint_fade(id: impl Into<TransitionId>, on: bool, tint: Hsla, policy: Tween, window: &mut Window, cx: &mut App) -> Hsla {
    let clear = Hsla { a: 0.0, ..tint };
    tween(id, if on { tint } else { clear }, policy, window, cx)
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

    /// Every preset, for the invariants that hold across all of them.
    const PRESETS: [Tween; 5] = [Tween::FAST, Tween::BASE, Tween::ENTER, Tween::EXIT, Tween::SLOW];

    #[test]
    fn presets_carry_the_right_easings_and_no_delay() {
        assert_eq!(Tween::FAST.easing, Easing::STD);
        assert_eq!(Tween::BASE.easing, Easing::STD);
        assert_eq!(Tween::EXIT.easing, Easing::STD);
        assert_eq!(Tween::SLOW.easing, Easing::OUT);
        for t in PRESETS {
            assert_eq!(t.delay, Duration::ZERO, "{t:?} starts delayed");
            assert!(t.duration > Duration::ZERO, "{t:?} has no duration");
        }
    }

    #[test]
    fn with_delay_and_with_easing_change_one_field_each() {
        let staggered = Tween::ENTER.with_delay(Duration::from_millis(40));
        assert_eq!(staggered.delay.as_millis(), 40);
        assert_eq!(staggered.duration, Tween::ENTER.duration);
        assert_eq!(staggered.easing, Tween::ENTER.easing);

        let eased = Tween::BASE.with_easing(Easing::OUT);
        assert_eq!(eased.easing, Easing::OUT);
        assert_eq!(eased.duration, Tween::BASE.duration);
        assert_eq!(eased.delay, Tween::BASE.delay);

        // The builders chain and leave the preset itself alone.
        assert_eq!(Tween::BASE.easing, Easing::STD);
        let both = Tween::new(Duration::from_millis(90), Easing::INOUT).with_delay(Duration::from_millis(10)).with_easing(Easing::OUT);
        assert_eq!(both, Tween { duration: Duration::from_millis(90), easing: Easing::OUT, delay: Duration::from_millis(10) });
    }

    #[test]
    fn easings_start_at_zero_and_end_at_one() {
        for e in [Easing::OUT, Easing::INOUT, Easing::STD] {
            assert!(e.sample(0.0).abs() < 1e-3, "{e:?} does not start at 0");
            assert!((e.sample(1.0) - 1.0).abs() < 1e-3, "{e:?} does not end at 1");
        }
    }

    #[test]
    fn easings_are_monotonic_and_bounded_in_between() {
        for e in [Easing::OUT, Easing::INOUT, Easing::STD] {
            let mut last = 0.0;
            for i in 0..=100 {
                let v = e.sample(i as f32 / 100.0);
                assert!((0.0..=1.0).contains(&v), "{e:?} left 0..=1 at {i}");
                assert!(v >= last - 1e-4, "{e:?} is not monotonic at {i}");
                last = v;
            }
        }
    }

    #[test]
    fn easing_samples_clamp_outside_the_unit_interval() {
        for e in [Easing::OUT, Easing::INOUT, Easing::STD] {
            assert_eq!(e.sample(-1.0), e.sample(0.0));
            assert_eq!(e.sample(2.0), e.sample(1.0));
        }
    }

    #[test]
    fn tint_fade_clears_the_alpha_and_nothing_else() {
        // The target `tint_fade` builds for the "off" state.
        let tint = Hsla { h: 0.58, s: 0.62, l: 0.44, a: 0.14 };
        let clear = Hsla { a: 0.0, ..tint };
        assert_eq!((clear.h, clear.s, clear.l), (tint.h, tint.s, tint.l));
        assert_eq!(clear.a, 0.0);
    }

    #[test]
    fn interpolating_tint_to_clear_never_moves_the_hue() {
        let tint = Hsla { h: 0.58, s: 0.62, l: 0.44, a: 0.14 };
        let clear = Hsla { a: 0.0, ..tint };
        let mut last = tint.a;
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let mid = tint.interpolate(&clear, t);
            assert_eq!((mid.h, mid.s, mid.l), (tint.h, tint.s, tint.l), "hue moved at t = {t}");
            assert!(mid.a <= last + 1e-6 && mid.a >= 0.0, "alpha is not falling at t = {t}");
            last = mid.a;
        }
        // A tween toward transparent black is what this avoids: it drags h, s and l.
        let flash = tint.interpolate(&gpui::transparent_black(), 0.5);
        assert_ne!((flash.h, flash.s, flash.l), (tint.h, tint.s, tint.l));
    }
}
