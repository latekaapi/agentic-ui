//! `.dot`: the 7 px status dot, with the pulse ring for running / waiting.

use aui_motion::pulse_ring;
use aui_tokens::{ActiveAui, AgentState};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, Pixels, Window};

/// `.dot{width:7px;height:7px}`.
pub const DOT_SIZE: f32 = 7.0;

/// A status dot. Build with [`status_dot`].
#[derive(IntoElement)]
pub struct StatusDot {
    id: ElementId,
    state: AgentState,
    pulse: bool,
    phase: Option<f32>,
    size: Pixels,
}

/// A dot in the state's colour.
pub fn status_dot(id: impl Into<ElementId>, state: AgentState) -> StatusDot {
    StatusDot { id: id.into(), state, pulse: false, phase: None, size: px(DOT_SIZE) }
}

impl StatusDot {
    /// Adds the expanding ring (`.dot.pulse`).
    pub fn pulse(mut self, pulse: bool) -> Self {
        self.pulse = pulse;
        self
    }

    /// Samples the ring at `phase` instead of mounting the looping
    /// animation: no frame is requested, so the ring only moves when the
    /// caller re-renders (a view on its own timer, pairing this with
    /// [`pulse_phase`](aui_motion::pulse_phase)). Unset: the dot loops as
    /// before.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = Some(phase);
        self
    }

    /// Overrides the diameter (the rail uses 8).
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for StatusDot {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = cx.aui().colors.agent_state(self.state);
        let mut ring = pulse_ring(self.id, self.size, color, self.pulse);
        if let Some(phase) = self.phase {
            ring = ring.phase(phase);
        }
        div().flex_none().size(self.size).child(ring)
    }
}
