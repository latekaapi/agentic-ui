//! Orchestration · Plan message: the proposed plan and the started plan.

use aui::orchestration::plan_message;
use aui::protocol::{sample, Block};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its card, and between the two states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let Block::RunPlan(proposed) = sample::run_plan_proposed() else {
        unreachable!("sample::run_plan_proposed is a plan")
    };
    let Block::RunPlan(started) = sample::run_plan_started() else {
        unreachable!("sample::run_plan_started is a plan")
    };
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(v_flex().w_full().child(caption("PROPOSED · REPLY \"GO\" TO START", &p)).child(plan_message("orch-plan-proposed", &proposed)))
        .child(v_flex().w_full().child(caption("STARTED · CARRIES ITS START TIME", &p)).child(plan_message("orch-plan-started", &started)))
        .into_any_element()
}
