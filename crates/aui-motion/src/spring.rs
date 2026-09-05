//! The four design springs, as retargetable values.
//!
//! A spring carries velocity across a retarget, so a chevron that is asked to
//! rotate back mid-flight decelerates and turns instead of restarting. State
//! is keyed by id and lives in the window.

use std::time::Duration;

use aui_tokens::springs;
use gpui::{App, Pixels, SpringConfig, Window};
use gpui_kit::base::{Spring, TransitionId};

/// Which of the design's four springs to use (`motion.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpringKind {
    /// 500 / 30 / 0.6 — buttons, chips, rows on press.
    Press,
    /// 460 / 30 / 0.55 — icon morph, chevrons, tab indicator, checkbox.
    Swap,
    /// 360 / 32 / 1 — collapse/expand, panel width, card resize.
    Layout,
    /// 200 / 26 / 1 — popover morph, drop-zone overlay.
    Gentle,
}

impl SpringKind {
    /// The physical parameters from the design tokens.
    pub fn config(self) -> SpringConfig {
        match self {
            SpringKind::Press => springs::PRESS,
            SpringKind::Swap => springs::SWAP,
            SpringKind::Layout => springs::LAYOUT,
            SpringKind::Gentle => springs::GENTLE,
        }
    }

    /// Human-readable label, as printed on the motion card.
    pub fn label(self) -> &'static str {
        match self {
            SpringKind::Press => "press",
            SpringKind::Swap => "swap",
            SpringKind::Layout => "layout",
            SpringKind::Gentle => "gentle",
        }
    }

    /// `stiffness / damping / mass` as printed on the motion card.
    pub fn describe(self) -> String {
        let c = self.config();
        format!("{} / {} / {}", c.stiffness, c.damping, c.mass)
    }

    /// The equivalent gpui-base policy for a value in `0..=1`.
    ///
    /// gpui-base describes a spring by its undamped period (`response`) and
    /// damping ratio ζ, with unit mass; the design describes it by stiffness
    /// `k`, damping coefficient `c` and mass `m`. They are the same oscillator:
    /// `ω₀ = √(k/m)`, `response = 2π/ω₀`, `ζ = c / (2√(km))`.
    pub fn policy(self) -> Spring {
        let cfg = self.config();
        let (omega, zeta) = cfg.canonical();
        Spring::new(Duration::from_secs_f32(std::f32::consts::TAU / omega)).with_damping(zeta)
    }

    /// The policy for a value in pixels, with a coarser settling tolerance so
    /// sub-pixel motion stops requesting frames.
    pub fn policy_px(self) -> Spring {
        self.policy().with_epsilon(0.1)
    }

    /// How long the spring takes to settle from rest to a unit step, for
    /// display and for choosing exit holds.
    pub fn settle_time(self) -> Duration {
        let cfg = self.config();
        cfg.settle_time(gpui::SpringState { position: 0.0, velocity: 0.0 }, 1.0, 0.001)
    }
}

/// A spring-driven `f32`. The first call adopts `target`; later calls travel
/// toward it with the spring's velocity preserved. Returns the current value
/// and requests frames while moving. Reduced motion resolves instantly.
pub fn spring(id: impl Into<TransitionId>, target: f32, kind: SpringKind, window: &mut Window, cx: &mut App) -> f32 {
    gpui_kit::base::spring(id, target, kind.policy(), window, cx)
}

/// A spring-driven length in pixels.
pub fn spring_px(id: impl Into<TransitionId>, target: Pixels, kind: SpringKind, window: &mut Window, cx: &mut App) -> Pixels {
    gpui_kit::base::spring(id, target, kind.policy_px(), window, cx)
}

/// A spring-driven phase for a boolean state: `0.0` when `on` is false,
/// `1.0` when true, travelling between them. This is the building block for
/// chevron rotation, checkbox pops and the send/stop morph.
pub fn spring_phase(id: impl Into<TransitionId>, on: bool, kind: SpringKind, window: &mut Window, cx: &mut App) -> f32 {
    spring(id, if on { 1.0 } else { 0.0 }, kind, window, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_preserves_the_oscillator() {
        for kind in [SpringKind::Press, SpringKind::Swap, SpringKind::Layout, SpringKind::Gentle] {
            let (omega, zeta) = kind.config().canonical();
            assert!(omega > 0.0 && zeta > 0.0 && zeta < 1.0, "{kind:?} should be under-damped");
            // press: ω0 = √(500/0.6) ≈ 28.87, ζ ≈ 0.866
            if kind == SpringKind::Press {
                assert!((omega - 28.87).abs() < 0.05);
                assert!((zeta - 0.866).abs() < 0.005);
            }
        }
    }

    #[test]
    fn springs_settle_in_the_expected_windows() {
        // The CSS linear() approximations end around 220–300 ms for press/swap
        // and ~330–450 ms for layout/gentle.
        let press = SpringKind::Press.settle_time().as_millis();
        let gentle = SpringKind::Gentle.settle_time().as_millis();
        assert!((150..=450).contains(&press), "press settles in {press} ms");
        assert!(gentle > press, "gentle ({gentle} ms) is slower than press ({press} ms)");
    }

    #[test]
    fn describe_prints_the_card_numbers() {
        assert_eq!(SpringKind::Press.describe(), "500 / 30 / 0.6");
        assert_eq!(SpringKind::Layout.describe(), "360 / 32 / 1");
    }
}
