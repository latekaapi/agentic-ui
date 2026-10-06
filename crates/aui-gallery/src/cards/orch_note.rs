//! Orchestration · Run note: the remembered preference as one muted line.

use aui::orchestration::run_note;
use aui::protocol::{sample, Block};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between the caption and the line.
const CAPTION_GAP: f32 = 6.0;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let Block::RunNote(note) = sample::run_note_preference() else {
        unreachable!("sample::run_note_preference is a note")
    };
    v_flex().w_full().child(
        div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child("PREFERENCE · ONE MUTED LINE"),
    ).child(run_note("orch-note-preference", &note)).into_any_element()
}
