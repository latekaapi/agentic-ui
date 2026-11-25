//! The goal pill (harness docs/85 15.2), the mock before the build (rule
//! 30): above the composer while a session works toward a goal — running
//! with its latest check, met, paused at a budget, waiting for a usage
//! limit, and with the session's acceptance criteria.

use aui::transcript::{goal_offer, goal_pill, CriterionLine, CriterionState, GoalPillModel, GoalPillState};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, Window};
use gpui_kit::base::v_flex;

fn model(state: GoalPillState, check: Option<&str>, criteria: bool) -> GoalPillModel {
    let line = |t: &str, s| CriterionLine { text: t.to_owned().into(), state: s, by: "the check".into() };
    GoalPillModel {
        condition: "the failing date tests pass".into(),
        state,
        elapsed: "14m".into(),
        turns: 6,
        tokens: "212k".into(),
        check: check.map(|c| c.to_owned().into()),
        criteria: if criteria { vec![line("cargo test passes", CriterionState::NotMet), line("no test is skipped", CriterionState::Met)] } else { Vec::new() },
    }
}

/// `goals/pill`: the pill in each of its states.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let caption = |t: &str| div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(t.to_owned());
    v_flex()
        .w(px(720.0))
        .p(px(scale::SP_6))
        .gap(px(scale::SP_3))
        .bg(p.bg)
        .child(caption("Working, the latest check"))
        .child(goal_pill("goal-running", model(GoalPillState::Running, Some("cargo test: 3 failing (date::parse, date::range, date::tz)"), false)))
        .child(caption("With the session's acceptance criteria"))
        .child(goal_pill("goal-criteria", model(GoalPillState::Running, Some("cargo test: 1 failing"), true)))
        .child(caption("Met"))
        .child(goal_pill("goal-met", model(GoalPillState::Met, Some("cargo test: passed"), false)))
        .child(caption("Paused at its budget"))
        .child(goal_pill("goal-paused", model(GoalPillState::BudgetReached, Some("Reached 3 hours: Resume to keep going"), false)))
        .child(caption("Waiting for the usage limit"))
        .child(goal_pill("goal-usage", model(GoalPillState::UsageWait, Some("Resumes by itself at 14:05"), false)))
        .child(caption("Offered: a possible end, set by one tap"))
        .child(goal_offer("goal-offer", "the login test passes"))
        .into_any_element()
}
