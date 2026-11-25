//! The goal pill (harness docs/85 15.2): one line above the composer while
//! a session works toward a goal — the condition, its state, time, turns,
//! tokens, the latest check line, Pause (or Resume) and Stop. When the
//! session has acceptance criteria, they list under it with their states.
//! It shares the Workers block's look (a transcript card, hairlines, colour
//! only on the state word and dot).

use std::rc::Rc;

use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, record_ax_label};
use crate::transcript::card::transcript_card;
use crate::transcript::workers::{CriterionLine, CriterionState};

/// Where a goal stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalPillState {
    /// Working toward it.
    Running,
    /// The check passed.
    Met,
    /// Paused by the person or a limit.
    Paused,
    /// Stuck: needs the person.
    Blocked,
    /// Waiting for a usage limit to reset.
    UsageWait,
    /// A time or token ceiling was reached.
    BudgetReached,
}

impl GoalPillState {
    /// The state's word.
    pub fn word(self) -> &'static str {
        match self {
            Self::Running => "Working",
            Self::Met => "Met",
            Self::Paused => "Paused",
            Self::Blocked => "Blocked",
            Self::UsageWait => "Waiting for the usage limit",
            Self::BudgetReached => "Budget reached",
        }
    }

    fn color(self, p: &Palette) -> Hsla {
        match self {
            Self::Running => p.accent,
            Self::Met => p.success,
            Self::Paused | Self::UsageWait | Self::BudgetReached => p.warning,
            Self::Blocked => p.danger,
        }
    }
}

/// What the pill shows.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalPillModel {
    /// The goal's condition, as set.
    pub condition: SharedString,
    /// Its state.
    pub state: GoalPillState,
    /// `12m`, `1h 4m`.
    pub elapsed: SharedString,
    /// Turns spent on it.
    pub turns: u32,
    /// Tokens spent, as words (`48k`).
    pub tokens: SharedString,
    /// The latest check's line (`cargo test: 3 failing`), or why it paused.
    pub check: Option<SharedString>,
    /// The session's acceptance criteria, when it has any.
    pub criteria: Vec<CriterionLine>,
}

/// What the pill's buttons ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalAction {
    /// Pause a running goal.
    Pause,
    /// Resume a paused one.
    Resume,
    /// Stop pursuing it.
    Stop,
}

type Handler = Rc<dyn Fn(GoalAction, &mut Window, &mut App)>;

/// The goal pill. Build with [`goal_pill`].
#[derive(IntoElement)]
pub struct GoalPill {
    id: ElementId,
    model: GoalPillModel,
    on_action: Option<Handler>,
}

/// The pill for `model`.
pub fn goal_pill(id: impl Into<ElementId>, model: GoalPillModel) -> GoalPill {
    GoalPill { id: id.into(), model, on_action: None }
}

impl GoalPill {
    /// What a button asks for.
    pub fn on_action(mut self, f: impl Fn(GoalAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

/// `12m · 4 turns · 48k tokens`.
pub fn goal_meta(model: &GoalPillModel) -> String {
    format!("{} · {} turn{} · {} tokens", model.elapsed, model.turns, if model.turns == 1 { "" } else { "s" }, model.tokens)
}

impl RenderOnce for GoalPill {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let m = &self.model;
        let label: SharedString = format!("Goal: {} — {}", m.condition, m.state.word()).into();
        record_ax_label(&label);
        let act = |id: &str, word: &'static str, action: GoalAction, aria: String| {
            let handler = self.on_action.clone();
            record_ax_label(&aria);
            button(ElementId::from((self.id.clone(), SharedString::from(id.to_owned()))), word).ghost().accessibility_label(aria).on_click(move |_, w, cx| {
                if let Some(h) = handler.as_ref() {
                    h(action, w, cx);
                }
            })
        };
        let show_body = m.check.is_some() || !m.criteria.is_empty();
        let running = matches!(m.state, GoalPillState::Running | GoalPillState::UsageWait);
        let paused = matches!(m.state, GoalPillState::Paused | GoalPillState::BudgetReached | GoalPillState::Blocked);
        let mut header = transcript_card(self.id.clone(), show_body)
            .chevron(false)
            .hover_tint(false)
            .header(div().size(px(scale::SP_2)).flex_none().rounded_full().bg(m.state.color(&p)))
            .header(div().flex_1().min_w(px(0.0)).truncate().text_role(TextRole::Body).text_color(p.ink).child(m.condition.clone()))
            .header(div().flex_none().role_in(TextRole::Caption, cx).text_color(m.state.color(&p)).child(m.state.word()))
            .header(div().flex_none().role_in(TextRole::Micro, cx).text_color(p.ink_4).child(goal_meta(m)));
        if running {
            header = header.header(act("pause", "Pause", GoalAction::Pause, format!("Pause the goal {}", m.condition)));
        }
        if paused {
            header = header.header(act("resume", "Resume", GoalAction::Resume, format!("Resume the goal {}", m.condition)));
        }
        if m.state != GoalPillState::Met {
            header = header.header(act("stop", "Stop", GoalAction::Stop, format!("Stop the goal {}", m.condition)));
        }
        let mut body = v_flex().w_full().px(px(scale::SP_3)).py(px(scale::SP_2)).gap(px(scale::SP_1));
        if let Some(check) = &m.check {
            body = body.child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(check.clone()));
        }
        for line in &m.criteria {
            let color = match line.state {
                CriterionState::Met => p.success,
                CriterionState::NotMet => p.danger,
                CriterionState::NotProven | CriterionState::YourCall => p.warning,
            };
            body = body.child(
                h_flex()
                    .w_full()
                    .gap(px(scale::SP_2))
                    .items_center()
                    .child(div().size(px(scale::SP_2)).flex_none().rounded_full().bg(color))
                    .child(div().flex_1().min_w(px(0.0)).truncate().role_in(TextRole::Caption, cx).text_color(p.fg).child(line.text.clone()))
                    .child(div().flex_none().role_in(TextRole::Caption, cx).text_color(color).child(line.state.word())),
            );
        }
        if show_body {
            header = header.body(body);
        }
        header
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_meta_line_counts() {
        let m = GoalPillModel { condition: "tests pass".into(), state: GoalPillState::Running, elapsed: "12m".into(), turns: 1, tokens: "48k".into(), check: None, criteria: Vec::new() };
        assert_eq!(goal_meta(&m), "12m · 1 turn · 48k tokens");
        assert_eq!(GoalPillState::UsageWait.word(), "Waiting for the usage limit");
    }
}
