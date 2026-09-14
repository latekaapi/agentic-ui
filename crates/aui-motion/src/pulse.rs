//! Pulse: the ring that expands out of a running / waiting status dot every
//! 2 s (`.dot.pulse::after` in base.css: inset −4 px, 1.5 px border, scale
//! .6 → 1.5, opacity .7 → 0, ease-out).

use std::time::Duration;

use aui_tokens::Easing;
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, Pixels, Window};

use crate::looping::{looping, Loop};

/// One pulse cycle.
pub const PERIOD: Duration = Duration::from_millis(2000);

/// Samples the pulse phase for a cycle that started at `epoch`, without
/// asking for a frame: exactly what [`crate::looping::looping`]
/// reports for the pulse loop at `now`. A view that advances on its own
/// timer — rather than on `request_animation_frame`, which would hold the
/// whole window at display rate — samples this once per tick and passes it
/// to [`PulseRing::phase`]. The caller owns the clock, including reduced
/// motion (sample the resting phase, `1.0`, when it is set).
pub fn pulse_phase(epoch: std::time::Instant, now: std::time::Instant) -> f32 {
    let cycles = now.saturating_duration_since(epoch).as_secs_f32() / PERIOD.as_secs_f32();
    Easing::OUT.sample(cycles.fract())
}

/// A status dot with its pulsing ring. `dot` is the dot diameter (7 px in
/// rows). The ring is drawn as a sibling so the dot itself never moves.
#[derive(IntoElement)]
pub struct PulseRing {
    id: ElementId,
    dot: Pixels,
    color: Hsla,
    active: bool,
    phase: Option<f32>,
}

/// Builds a pulsing dot. With `active = false` it is a plain dot.
pub fn pulse_ring(id: impl Into<ElementId>, dot: Pixels, color: Hsla, active: bool) -> PulseRing {
    PulseRing { id: id.into(), dot, color, active, phase: None }
}

impl PulseRing {
    /// Samples the ring at `phase` instead of mounting the looping
    /// animation: no frame is requested, so the ring only moves when the
    /// caller re-renders. A sampled ring still needs its caller to advance
    /// — on its own timer, pairing this with [`pulse_phase`].
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = Some(phase);
        self
    }
}

impl RenderOnce for PulseRing {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dot = self.dot;
        let base = div().relative().flex_none().size(dot).rounded_full().bg(self.color);
        if !self.active {
            return base;
        }
        // Resting phase 1 = ring fully faded, so reduced motion shows a plain dot.
        let phase = match self.phase {
            Some(phase) => phase,
            None => looping((self.id, "pulse"), Loop::eased(PERIOD, Easing::OUT).resting(1.0), window, cx),
        };
        // The CSS ring box is the dot inset by −4 px, scaled .6 → 1.5.
        let ring_box = dot + px(8.0);
        let scale = 0.6 + 0.9 * phase;
        let ring = ring_box * scale;
        let opacity = 0.7 * (1.0 - phase);
        base.child(
            div()
                .absolute()
                .top((dot - ring) / 2.0)
                .left((dot - ring) / 2.0)
                .size(ring)
                .rounded_full()
                .border(px(1.5))
                .border_color(self.color)
                .opacity(opacity),
        )
    }
}
