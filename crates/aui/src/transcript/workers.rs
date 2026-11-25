//! Orchestration's transcript surfaces (harness docs/85 14.9): the Workers
//! block pinned at the foot of a lead's transcript, and the per-criterion
//! summary card a lead's work ends with.
//!
//! Flat (docs/83): one card, hairlines between rows, colour only on the
//! state word and dot. Every control carries a role and a label.

use std::rc::Rc;

use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, record_ax_label};
use crate::icons::IconName;
use crate::transcript::card::transcript_card;

/// Where a worker's task stands, as the block words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    /// Its `after` tasks have not passed.
    Waiting,
    /// The worker is opening.
    Starting,
    /// The worker is working.
    Running,
    /// Baaz runs the check.
    Checking,
    /// The check passed.
    Passed,
    /// The check failed after the fix-up.
    Failed,
    /// Waiting on the person.
    NeedsYou,
    /// Stopped by the lead or the person.
    Stopped,
    /// Merged.
    Landed,
}

impl WorkerState {
    /// The state's word.
    pub fn word(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting",
            Self::Starting => "Starting",
            Self::Running => "Working",
            Self::Checking => "Checking",
            Self::Passed => "Passed",
            Self::Failed => "Failed",
            Self::NeedsYou => "Needs you",
            Self::Stopped => "Stopped",
            Self::Landed => "Landed",
        }
    }

    /// Done with: nothing more happens without someone.
    pub fn settled(self) -> bool {
        matches!(self, Self::Passed | Self::Failed | Self::NeedsYou | Self::Stopped | Self::Landed)
    }

    fn color(self, p: &Palette) -> Hsla {
        match self {
            Self::Passed | Self::Landed => p.success,
            Self::Failed => p.danger,
            Self::NeedsYou => p.warning,
            Self::Running | Self::Checking | Self::Starting => p.accent,
            Self::Waiting | Self::Stopped => p.fg_muted,
        }
    }
}

/// One worker's row.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerRow {
    /// The worker's session id: what the row's actions report.
    pub id: SharedString,
    /// The task's title.
    pub title: SharedString,
    /// The provider's name ("Codex").
    pub provider: SharedString,
    /// Where its task stands.
    pub state: WorkerState,
    /// `4m`, `1h 2m`.
    /// How long it has run.
    pub elapsed: SharedString,
}

/// What a row's controls ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkersAction {
    /// Open the worker beside this session (the right pane).
    OpenBeside(SharedString),
    /// The block's header: fold or unfold it.
    Toggle,
}

type Handler = Rc<dyn Fn(WorkersAction, &mut Window, &mut App)>;

/// The Workers block. Build with [`workers_block`].
#[derive(IntoElement)]
pub struct WorkersBlock {
    id: ElementId,
    rows: Vec<WorkerRow>,
    open: bool,
    on_action: Option<Handler>,
}

/// The block over `rows`; `open` false folds it to its one header line
/// (what it does by itself once every worker has settled).
pub fn workers_block(id: impl Into<ElementId>, rows: Vec<WorkerRow>, open: bool) -> WorkersBlock {
    WorkersBlock { id: id.into(), rows, open, on_action: None }
}

impl WorkersBlock {
    /// What a control asks for.
    pub fn on_action(mut self, f: impl Fn(WorkersAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

/// `2 working · 1 passed`: the header's count of each state present.
pub fn workers_headline(rows: &[WorkerRow]) -> String {
    let mut parts: Vec<(WorkerState, usize)> = Vec::new();
    for row in rows {
        match parts.iter_mut().find(|(s, _)| *s == row.state) {
            Some((_, n)) => *n += 1,
            None => parts.push((row.state, 1)),
        }
    }
    parts.iter().map(|(s, n)| format!("{n} {}", s.word().to_lowercase())).collect::<Vec<_>>().join(" · ")
}

fn dot(color: Hsla) -> impl IntoElement {
    div().size(px(scale::SP_2)).flex_none().rounded_full().bg(color)
}

impl RenderOnce for WorkersBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let headline = workers_headline(&self.rows);
        let count = self.rows.len();
        let label: SharedString = format!("Workers: {headline}").into();
        record_ax_label(&label);
        let mut body = v_flex().w_full();
        for (i, row) in self.rows.iter().enumerate() {
            let open_label: SharedString = format!("Open {} beside this session", row.title).into();
            record_ax_label(&open_label);
            let handler = self.on_action.clone();
            let id = row.id.clone();
            let open = icon_button(ElementId::from((self.id.clone(), SharedString::from(format!("open-{i}")))), IconName::External)
                .ghost()
                .muted()
                .accessibility_label(open_label)
                .on_click(move |_, w, cx| {
                    if let Some(h) = handler.as_ref() {
                        h(WorkersAction::OpenBeside(id.clone()), w, cx);
                    }
                });
            body = body.child(
                h_flex()
                    .w_full()
                    .h(px(scale::H_MD))
                    .px(px(scale::SP_3))
                    .gap(px(scale::SP_3))
                    .items_center()
                    .when(i > 0, |d| d.border_t_1().border_color(p.line))
                    .child(dot(row.state.color(&p)))
                    .child(div().flex_1().min_w(px(0.0)).truncate().text_role(TextRole::Body).text_color(p.fg).child(row.title.clone()))
                    .child(div().flex_none().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(row.provider.clone()))
                    .child(div().flex_none().role_in(TextRole::Caption, cx).text_color(row.state.color(&p)).child(row.state.word()))
                    .child(div().flex_none().role_in(TextRole::Micro, cx).text_color(p.ink_4).child(row.elapsed.clone()))
                    .child(open),
            );
        }
        let handler = self.on_action.clone();
        transcript_card(self.id.clone(), self.open)
            .header(div().flex_none().role_in(TextRole::Body, cx).text_color(p.ink).child(if count == 1 { "1 worker".to_string() } else { format!("{count} workers") }))
            .header(div().flex_1().min_w(px(0.0)).truncate().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(headline))
            .body(body)
            .on_toggle(move |_, w, cx| {
                if let Some(h) = handler.as_ref() {
                    h(WorkersAction::Toggle, w, cx);
                }
            })
    }
}

/// How a criterion ended, as the summary words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterionState {
    /// Proven met.
    Met,
    /// Proven not met.
    NotMet,
    /// Nothing proves it.
    NotProven,
    /// The person judges it.
    YourCall,
}

impl CriterionState {
    /// The state's word.
    pub fn word(self) -> &'static str {
        match self {
            Self::Met => "Met",
            Self::NotMet => "Not met",
            Self::NotProven => "Not proven",
            Self::YourCall => "Yours to judge",
        }
    }

    fn color(self, p: &Palette) -> Hsla {
        match self {
            Self::Met => p.success,
            Self::NotMet => p.danger,
            Self::NotProven | Self::YourCall => p.warning,
        }
    }
}

/// One criterion's line: its words, its state, and what proved it.
#[derive(Debug, Clone, PartialEq)]
pub struct CriterionLine {
    /// The criterion, as frozen.
    pub text: SharedString,
    /// How it ended.
    pub state: CriterionState,
    /// `the check`, `the reviewer`, `nothing checks it`.
    pub by: SharedString,
}

/// One task's part of the summary.
#[derive(Debug, Clone, PartialEq)]
pub struct CriteriaTask {
    /// The task's title.
    pub title: SharedString,
    /// Its provider's name.
    pub provider: SharedString,
    /// Its criteria.
    pub lines: Vec<CriterionLine>,
}

/// The per-criterion summary card. Build with [`criteria_card`].
#[derive(IntoElement)]
pub struct CriteriaCard {
    id: ElementId,
    tasks: Vec<CriteriaTask>,
}

/// The summary over `tasks`: each task, then each of its criteria with its
/// state and what proved it.
pub fn criteria_card(id: impl Into<ElementId>, tasks: Vec<CriteriaTask>) -> CriteriaCard {
    CriteriaCard { id: id.into(), tasks }
}

/// `4 of 5 criteria met`.
pub fn criteria_headline(tasks: &[CriteriaTask]) -> String {
    let all: Vec<&CriterionLine> = tasks.iter().flat_map(|t| t.lines.iter()).collect();
    let met = all.iter().filter(|l| l.state == CriterionState::Met).count();
    format!("{met} of {} criteria met", all.len())
}

impl RenderOnce for CriteriaCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let headline = criteria_headline(&self.tasks);
        record_ax_label(&headline);
        let mut body = v_flex().w_full().px(px(scale::SP_3)).py(px(scale::SP_2)).gap(px(scale::SP_3));
        for task in &self.tasks {
            let mut block = v_flex().w_full().gap(px(scale::SP_1)).child(
                h_flex()
                    .gap(px(scale::SP_2))
                    .child(div().role_in(TextRole::Body, cx).text_color(p.ink).child(task.title.clone()))
                    .child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(task.provider.clone())),
            );
            for line in &task.lines {
                block = block.child(
                    h_flex()
                        .w_full()
                        .gap(px(scale::SP_2))
                        .items_center()
                        .pl(px(scale::SP_3))
                        .child(dot(line.state.color(&p)))
                        .child(div().flex_1().min_w(px(0.0)).truncate().role_in(TextRole::Caption, cx).text_color(p.fg).child(line.text.clone()))
                        .child(div().flex_none().role_in(TextRole::Caption, cx).text_color(line.state.color(&p)).child(line.state.word()))
                        .child(div().flex_none().role_in(TextRole::Micro, cx).text_color(p.ink_4).child(format!("by {}", line.by))),
                );
            }
            body = body.child(block);
        }
        transcript_card(self.id.clone(), true)
            .chevron(false)
            .hover_tint(false)
            .header(div().flex_none().role_in(TextRole::Body, cx).text_color(p.ink).child("Delegated work"))
            .header(div().flex_1().min_w(px(0.0)).truncate().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(headline))
            .body(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(state: WorkerState) -> WorkerRow {
        WorkerRow { id: "w".into(), title: "t".into(), provider: "Codex".into(), state, elapsed: "1m".into() }
    }

    #[test]
    fn the_headlines_count_states_and_criteria() {
        assert_eq!(workers_headline(&[row(WorkerState::Running), row(WorkerState::Running), row(WorkerState::Passed)]), "2 working · 1 passed");
        let line = |state| CriterionLine { text: "x".into(), state, by: "the check".into() };
        let tasks = vec![CriteriaTask { title: "a".into(), provider: "Codex".into(), lines: vec![line(CriterionState::Met), line(CriterionState::NotProven)] }];
        assert_eq!(criteria_headline(&tasks), "1 of 2 criteria met");
        assert!(WorkerState::NeedsYou.settled() && !WorkerState::Checking.settled());
    }
}
