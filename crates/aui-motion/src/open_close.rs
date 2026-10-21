//! [`open_close`]: one 0 → 1 value that opens on a spring and closes on a
//! tween — Fluid Functionalism's pairing for panels (`spring.slow` in,
//! `spring.slow.exit` out; the peek on `moderate` / `moderate.exit`).
//!
//! gpui-base keys a spring and a tween as two separate transitions, so
//! switching policy with the direction would restart from the other key's
//! stale value and jump. This keeps one clock: a direction change records
//! the value it left from and starts the new curve there. The spring is
//! sampled analytically from rest (velocity is not carried across a
//! reversal; at these durations the difference is under a pixel).

use std::time::{Duration, Instant};

use aui_tokens::Easing;
use gpui::{App, ElementId, Window};

use crate::SpringKind;

/// The easing of a duration-only tween in motion (the FF source's exit
/// tweens set only `duration`, which motion runs as ease-in-out).
pub const EXIT_EASING: Easing = Easing([0.42, 0.0, 0.58, 1.0]);

/// How a value opens: on a spring (panels) or a timed curve (fades).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Enter {
    /// The spring's step response from wherever the value is.
    Spring(SpringKind),
    /// A tween of this length on this easing.
    Tween(Duration, Easing),
}

#[derive(Clone, Copy)]
struct Run {
    open: bool,
    from: f32,
    start: Instant,
}

/// Samples the current value: 1 = open, 0 = closed. Opening runs `spring`
/// from where the value is; closing runs a tween of `exit` on
/// [`EXIT_EASING`]. The first call adopts the state without animating;
/// frames are requested while the value moves. Reduced motion jumps.
pub fn open_close(id: impl Into<ElementId>, open: bool, spring: SpringKind, exit: Duration, window: &mut Window, cx: &mut App) -> f32 {
    open_close_with(id, open, Enter::Spring(spring), exit, EXIT_EASING, window, cx)
}

/// [`open_close`] with any enter curve and exit easing — the closed
/// shell's pills fade in on FF's popup timing (80 ms ease-out) and out in
/// 60 ms. Like [`open_close`], the first call adopts the state, so a
/// window that opens closed shows its pills without a fade.
pub fn open_close_with(id: impl Into<ElementId>, open: bool, enter: Enter, exit: Duration, exit_easing: Easing, window: &mut Window, cx: &mut App) -> f32 {
    let target = if open { 1.0 } else { 0.0 };
    let now = cx.background_executor().now();
    let state = window.use_keyed_state(id.into(), cx, |_, _| Run { open, from: target, start: now.checked_sub(Duration::from_secs(10)).unwrap_or(now) });
    let run = *state.read(cx);
    let spring = Curves { enter, exit, exit_easing };
    let run = if run.open != open {
        let from = sample(run, spring, now).0;
        let next = Run { open, from, start: now };
        state.update(cx, |s, _| *s = next);
        next
    } else {
        run
    };
    if cx.reduce_motion() {
        return target;
    }
    let (value, settled) = sample(run, spring, now);
    if !settled {
        crate::request_frame(window);
    }
    value
}

#[derive(Clone, Copy)]
struct Curves {
    enter: Enter,
    exit: Duration,
    exit_easing: Easing,
}

/// Puts an [`open_close`] value at rest at `open` with no motion — for a
/// host that turned the motion off for a while (an instant layout), so the
/// next animated change starts from where the value really is rather than
/// from a stale run.
pub fn open_close_jump(id: impl Into<ElementId>, open: bool, window: &mut Window, cx: &mut App) {
    let target = if open { 1.0 } else { 0.0 };
    let now = cx.background_executor().now();
    let rest = Run { open, from: target, start: now.checked_sub(Duration::from_secs(10)).unwrap_or(now) };
    let state = window.use_keyed_state(id.into(), cx, |_, _| rest);
    let run = *state.read(cx);
    if run.open != open || run.from != target {
        state.update(cx, |s, _| *s = rest);
    }
}

/// The value of `run` at `now`, and whether it has settled.
fn sample(run: Run, curves: Curves, now: Instant) -> (f32, bool) {
    let t = now.saturating_duration_since(run.start).as_secs_f32();
    if run.open {
        match curves.enter {
            Enter::Spring(spring) => {
                let x = spring_step(spring, t);
                let value = run.from + (1.0 - run.from) * x;
                let settled = t > 0.05 && (1.0 - x).abs() < 0.001 && spring_velocity_small(spring, t);
                (if settled { 1.0 } else { value }, settled)
            }
            Enter::Tween(d, easing) => {
                let p = (t / d.as_secs_f32().max(f32::EPSILON)).min(1.0);
                (run.from + (1.0 - run.from) * easing.sample(p), p >= 1.0)
            }
        }
    } else {
        let d = curves.exit.as_secs_f32().max(f32::EPSILON);
        let p = (t / d).min(1.0);
        (run.from * (1.0 - curves.exit_easing.sample(p)), p >= 1.0)
    }
}

/// The unit step response of a spring from rest: 0 at t = 0, settling at 1.
pub fn spring_step(kind: SpringKind, t: f32) -> f32 {
    let cfg = kind.config();
    let (omega, zeta) = cfg.canonical();
    if zeta < 1.0 {
        let wd = omega * (1.0 - zeta * zeta).sqrt();
        1.0 - (-zeta * omega * t).exp() * ((wd * t).cos() + (zeta * omega / wd) * (wd * t).sin())
    } else {
        // Critically damped (the moderate and fast tiers are exactly 1).
        1.0 - (-omega * t).exp() * (1.0 + omega * t)
    }
}

fn spring_velocity_small(kind: SpringKind, t: f32) -> bool {
    let dt = 0.002;
    ((spring_step(kind, t + dt) - spring_step(kind, t)) / dt).abs() < 0.01
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_spring_overshoots_slightly_and_settles() {
        // FF slow: 685 / 46.1 / 1, ζ ≈ 0.88 — a faint overshoot, settled by ~0.4 s.
        let peak = (0..400).map(|i| spring_step(SpringKind::Slow, i as f32 / 1000.0)).fold(0.0f32, f32::max);
        assert!(peak > 1.0 && peak < 1.01, "peak {peak}");
        assert!((spring_step(SpringKind::Slow, 0.6) - 1.0).abs() < 0.001);
        assert_eq!(spring_step(SpringKind::Slow, 0.0), 0.0);
    }

    #[test]
    fn moderate_spring_never_overshoots() {
        let peak = (0..400).map(|i| spring_step(SpringKind::Moderate, i as f32 / 1000.0)).fold(0.0f32, f32::max);
        assert!(peak <= 1.0 + 1e-6, "peak {peak}");
    }

    #[test]
    fn a_reversal_starts_from_where_the_value_was() {
        let start = Instant::now();
        let opening = Run { open: true, from: 0.0, start };
        let curves = Curves { enter: Enter::Spring(SpringKind::Slow), exit: Duration::from_millis(160), exit_easing: EXIT_EASING };
        let mid = sample(opening, curves, start + Duration::from_millis(60)).0;
        assert!(mid > 0.1 && mid < 0.9);
        let closing = Run { open: false, from: mid, start: start + Duration::from_millis(60) };
        let (at_start, _) = sample(closing, curves, start + Duration::from_millis(60));
        assert!((at_start - mid).abs() < 1e-6);
        let (end, settled) = sample(closing, curves, start + Duration::from_millis(230));
        assert!(settled && end == 0.0);
    }
}
