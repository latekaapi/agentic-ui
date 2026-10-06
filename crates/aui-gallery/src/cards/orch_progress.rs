//! Orchestration · Progress block: running, needs-you, paused, waiting for
//! quota, and reconciling.

use aui::orchestration::progress_block;
use aui::protocol::{sample, Block};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its card, and between the six states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// One captioned state.
fn state(id: &'static str, note: &str, block: &Block, p: &aui_tokens::Palette) -> impl IntoElement {
    let Block::RunProgress(progress) = block else { unreachable!("orchestration progress samples are progress") };
    v_flex().w_full().child(caption(note, p)).child(progress_block(id, progress).on_intent(|_, _, _| {}))
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let running = sample::run_progress_running();
    let needs_you = sample::run_progress_needs_you();
    let paused = sample::run_progress_paused();
    let quota = sample::run_progress_quota_wait();
    let reconciling = sample::run_progress_reconciling();
    let mut paused_reconciling = sample::run_progress_reconciling();
    if let Block::RunProgress(progress) = &mut paused_reconciling {
        progress.paused = true;
    }
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(state("orch-progress-running", "RUNNING · ONE WORKING, ONE VERIFYING", &running, &p))
        .child(state("orch-progress-needs-you", "NEEDS YOU · SORTED FIRST, WARNING INK", &needs_you, &p))
        .child(state("orch-progress-paused", "PAUSED · NOTHING PULSES", &paused, &p))
        .child(state("orch-progress-quota", "WAITING FOR QUOTA · WARNING HEADER", &quota, &p))
        .child(state("orch-progress-reconciling", "RECONCILING · SPINNER ON THE ROW", &reconciling, &p))
        .child(state("orch-progress-paused-reconciling", "PAUSED WHILE RECONCILING · STATIC DOT, NO SPINNER", &paused_reconciling, &p))
        .into_any_element()
}
