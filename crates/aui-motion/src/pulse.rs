//! Pulse: the ring that expands out of a running / waiting status dot every
//! 2 s (`.dot.pulse::after` in base.css: inset −4 px, 1.5 px border, scale
//! .6 → 1.5, opacity .7 → 0, ease-out).

use std::time::Duration;

use aui_tokens::Easing;
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, Pixels, Window};

use crate::looping::{looping, Loop};

/// One pulse cycle.
pub const PERIOD: Duration = Duration::from_millis(2000);

/// A status dot with its pulsing ring. `dot` is the dot diameter (7 px in
/// rows). The ring is drawn as a sibling so the dot itself never moves.
#[derive(IntoElement)]
pub struct PulseRing {
    id: ElementId,
    dot: Pixels,
    color: Hsla,
    active: bool,
}

/// Builds a pulsing dot. With `active = false` it is a plain dot.
pub fn pulse_ring(id: impl Into<ElementId>, dot: Pixels, color: Hsla, active: bool) -> PulseRing {
    PulseRing { id: id.into(), dot, color, active }
}

impl RenderOnce for PulseRing {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dot = self.dot;
        let base = div().relative().flex_none().size(dot).rounded_full().bg(self.color);
        if !self.active {
            return base;
        }
        // Resting phase 1 = ring fully faded, so reduced motion shows a plain dot.
        let phase = looping((self.id, "pulse"), Loop::eased(PERIOD, Easing::OUT).resting(1.0), window, cx);
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
