//! Enter / exit for elements that appear and disappear: cards, menus,
//! toasts, banners, hover toolbars. The element keeps rendering during its
//! exit (the "exit hold"), so it can fade instead of vanishing.

use std::time::Duration;

use aui_tokens::{durations, Easing};
use gpui::{px, App, Pixels, Window};
use gpui_kit::base::{Presence, PresencePhase, Transition, TransitionId};

pub use gpui_kit::base::PresenceSample;

/// Enter and exit timings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnterExit {
    /// Enter duration (design: 220 ms ease-out).
    pub enter: Duration,
    /// Exit duration (design: 160 ms).
    pub exit: Duration,
    /// Delay before entering (staggers).
    pub delay: Duration,
}

impl EnterExit {
    /// enter 220 · exit 160.
    pub const DEFAULT: EnterExit = EnterExit { enter: durations::ENTER, exit: durations::EXIT, delay: Duration::ZERO };
    /// base 180 both ways — streamed chunks, list rows.
    pub const BASE: EnterExit = EnterExit { enter: durations::BASE, exit: durations::BASE, delay: Duration::ZERO };
    /// quick 150 in / 120 out — composer menus (plus, pickers, caret
    /// popovers), the one presence tween every composer menu shares.
    pub const QUICK: EnterExit = EnterExit { enter: durations::QUICK_ENTER, exit: durations::QUICK_EXIT, delay: Duration::ZERO };

    /// Adds an enter delay.
    pub const fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

impl Default for EnterExit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Samples the presence of an element. `present` is the desired state; the
/// sample says whether to render at all and how far along the enter/exit is
/// (`progress` runs 0→1 on enter and 1→0 on exit).
///
/// gpui-base's presence has a single duration, so the exit is expressed as a
/// shorter, reversed enter: the reversal factor already scales the time by how
/// far the enter got, and we scale the policy again by `exit / enter`.
pub fn presence(id: impl Into<TransitionId>, present: bool, timing: EnterExit, window: &mut Window, cx: &mut App) -> PresenceSample {
    let duration = if present { timing.enter } else { timing.exit };
    let policy = Transition::new(duration)
        .delay(if present { timing.delay } else { Duration::ZERO })
        .ease(|t| Easing::OUT.sample(t));
    Presence::new(id, present).transition(policy).sample(window, cx)
}

/// The standard visual for an entering/leaving element, derived from a
/// [`PresenceSample`]: opacity, a vertical rise and a size scale.
///
/// gpui has no element transform, so `scale` is meant to be applied as a size
/// or padding change by the caller, and `offset_y` as a relative `top`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresenceStyle {
    /// 0 → 1 while entering.
    pub opacity: f32,
    /// Positive means "below its resting place" (rise on enter).
    pub offset_y: Pixels,
    /// 0.985 → 1 for palettes and dialogs, 1 for plain fades.
    pub scale: f32,
}

impl PresenceStyle {
    /// Fade + rise by `rise` px (cards: 6, toolbars: 4, chunks: 3).
    pub fn fade_rise(sample: PresenceSample, rise: f32) -> Self {
        let p = sample.progress.clamp(0.0, 1.0);
        Self { opacity: p, offset_y: px(rise * (1.0 - p)), scale: 1.0 }
    }

    /// Fade + rise + a slight scale (command palette: 6 px, .985).
    pub fn fade_rise_scale(sample: PresenceSample, rise: f32, from_scale: f32) -> Self {
        let mut s = Self::fade_rise(sample, rise);
        s.scale = from_scale + (1.0 - from_scale) * sample.progress.clamp(0.0, 1.0);
        s
    }

    /// Plain fade.
    pub fn fade(sample: PresenceSample) -> Self {
        Self { opacity: sample.progress.clamp(0.0, 1.0), offset_y: px(0.0), scale: 1.0 }
    }

    /// Whether the element is fully at rest and visible.
    pub fn settled(sample: PresenceSample) -> bool {
        matches!(sample.phase, PresencePhase::Present)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_is_the_snappy_menu_timing() {
        assert!(EnterExit::QUICK.enter.as_millis() <= 150, "enter {:?}", EnterExit::QUICK.enter);
        assert!(EnterExit::QUICK.exit.as_millis() <= 120, "exit {:?}", EnterExit::QUICK.exit);
        assert_eq!(EnterExit::QUICK.enter, durations::QUICK_ENTER);
        assert_eq!(EnterExit::QUICK.exit, durations::QUICK_EXIT);
        assert_eq!(EnterExit::QUICK.delay, Duration::ZERO);
    }
}
