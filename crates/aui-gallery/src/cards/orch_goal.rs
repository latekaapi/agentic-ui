//! Orchestration · Session goal line: codex active and complete, claude
//! active, muse active, and reopened.

use aui::orchestration::{goal_line, GoalLineFields};
use aui::protocol::{sample, Block};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its row, and between the states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// One goal block as a captioned line; the block is destructured once and
/// the line borrows the strings.
fn goal(id: impl Into<gpui::ElementId>, block: Block) -> AnyElement {
    let Block::Goal {
        objective,
        status,
        percent_complete,
        provider,
        tokens_used,
        token_budget,
        elapsed,
        last_reason,
        gated_by_baaz,
        reopened,
        ..
    } = block
    else {
        unreachable!("goal samples are goals")
    };
    let fields = GoalLineFields {
        objective: &objective,
        status: &status,
        percent_complete,
        provider,
        tokens_used,
        token_budget,
        elapsed: elapsed.as_deref(),
        last_reason: last_reason.as_deref(),
        gated_by_baaz,
        reopened,
    };
    goal_line(id, &fields).into_any_element()
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(v_flex().w_full().child(caption("CODEX · ACTIVE", &p)).child(goal("orch-goal-codex-active", sample::goal_codex_active())))
        .child(v_flex().w_full().child(caption("CODEX · COMPLETE", &p)).child(goal("orch-goal-codex-complete", sample::goal_codex_complete())))
        .child(v_flex().w_full().child(caption("CLAUDE · ACTIVE WITH LAST REASON", &p)).child(goal("orch-goal-claude-active", sample::goal_claude_active())))
        .child(v_flex().w_full().child(caption("MUSE · JUST STARTED", &p)).child(goal("orch-goal-muse-active", sample::goal_muse_active())))
        .child(v_flex().w_full().child(caption("REOPENED · PROVIDER SAID DONE, CHECKS DISAGREED", &p)).child(goal("orch-goal-reopened", sample::goal_reopened())))
        .into_any_element()
}
