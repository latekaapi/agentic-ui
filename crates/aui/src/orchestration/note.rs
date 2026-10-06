//! The run note: one muted line under the user message — a remembered
//! preference or a plain informational note. It never carries controls.

use aui_protocol::{NoteKind, RunNote};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, Window};
use gpui_kit::base::h_flex;

/// The diameter of the info note's leading dot.
const INFO_DOT: f32 = 5.0;

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
        let text =
            div().min_w(px(0.0)).text_role(TextRole::Meta).text_color(p.ink_3).child(self.note.text.clone());
        match self.note.note_kind {
            NoteKind::Preference => div().id(self.id).w_full().child(text).into_any_element(),
            // No info glyph exists in the sprite, so the info note leads
            // with a 5 px dot in the info ink.
            NoteKind::Info => h_flex()
                .id(self.id)
                .w_full()
                .items_center()
                .gap(px(scale::SP_2))
                .child(div().flex_none().size(px(INFO_DOT)).rounded_full().bg(p.info))
                .child(text)
                .into_any_element(),
        }
    }
}
