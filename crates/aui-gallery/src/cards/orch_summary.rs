//! Orchestration · Run summary: finished, stopped, failed and merged.

use aui::orchestration::run_summary;
use aui::protocol::{sample, Block, RunOutcome};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its card, and between the states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let Block::RunSummary(finished) = sample::run_summary_finished() else {
        unreachable!("sample::run_summary_finished is a summary")
    };
    let Block::RunSummary(stopped) = sample::run_summary_stopped() else {
        unreachable!("sample::run_summary_stopped is a summary")
    };
    let Block::RunSummary(merged) = sample::run_summary_merged() else {
        unreachable!("sample::run_summary_merged is a summary")
    };
    // No failed sample exists, so the failed state reuses the finished
    // value with a failed outcome and one unmet criterion: the unmet row
    // shows the error glyph and no evidence chips.
    let mut failed = finished.clone();
    failed.outcome = RunOutcome::Failed;
    if let Some(first) = failed.criteria.first_mut() {
        first.met = false;
    }
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(v_flex().w_full().child(caption("FINISHED · CRITERIA WITH EVIDENCE CHIPS", &p)).child(run_summary("orch-summary-finished", &finished)))
        .child(v_flex().w_full().child(caption("STOPPED · OUTCOME IN MUTED INK", &p)).child(run_summary("orch-summary-stopped", &stopped)))
        .child(v_flex().w_full().child(caption("FAILED · UNMET CRITERION SHOWS NO CHIPS", &p)).child(run_summary("orch-summary-failed", &failed)))
        .child(v_flex().w_full().child(caption("MERGED · FOOTER NAMES THE COMMIT", &p)).child(run_summary("orch-summary-merged", &merged)))
        .into_any_element()
}
