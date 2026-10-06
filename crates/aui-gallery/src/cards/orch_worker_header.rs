//! Orchestration · Worker header: stage, attempt, provider mark, model,
//! branch and the run link.

use aui::orchestration::worker_header;
use aui::protocol::sample;
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between the caption and the line.
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above the line.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let header = sample::worker_header_sample();
    v_flex()
        .w_full()
        .child(caption("MUSE WORKER · AUTH CALL, ATTEMPT 1", &p))
        .child(worker_header("orch-worker-header", &header).on_intent(|_, _, _| {}))
        .into_any_element()
}
