//! Orchestration · Run note: the remembered preference and the info note,
//! each as one muted line.

use aui::orchestration::run_note;
use aui::protocol::{sample, Block, NoteKind, RunNote};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its line, and between the two states.
const CAPTION_GAP: f32 = 6.0;
const SECTION_GAP: f32 = 14.0;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let Block::RunNote(preference) = sample::run_note_preference() else {
        unreachable!("sample::run_note_preference is a note")
    };
    let info = RunNote {
        text: "Heads up: the quota resets at 15:00, so the run resumes then.".into(),
        note_kind: NoteKind::Info,
    };
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(
            v_flex().w_full().child(
                div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child("PREFERENCE · ONE MUTED LINE"),
            ).child(run_note("orch-note-preference", &preference)),
        )
        .child(
            v_flex().w_full().child(
                div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child("INFO · LEADING INFO DOT"),
            ).child(run_note("orch-note-info", &info)),
        )
        .into_any_element()
}
