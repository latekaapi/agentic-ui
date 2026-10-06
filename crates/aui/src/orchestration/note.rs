//! The run note: one muted line under the user message — a remembered
//! preference or a plain informational note. It never carries controls.

use aui_protocol::RunNote;
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, App, ElementId, IntoElement, Window};

/// The run note. Build with [`run_note`].
#[derive(IntoElement)]
pub struct RunNoteLine {
    id: ElementId,
    note: RunNote,
}

/// A run note as one muted line, drawn verbatim.
///
/// The card clones the value; it owns no orchestration logic.
pub fn run_note(id: impl Into<ElementId>, note: &RunNote) -> RunNoteLine {
    RunNoteLine { id: id.into(), note: note.clone() }
}

impl RenderOnce for RunNoteLine {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        div().id(self.id).w_full().text_role(TextRole::Meta).text_color(p.ink_3).child(self.note.text.clone())
    }
}
