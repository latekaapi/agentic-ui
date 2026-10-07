//! The stateful orchestration mock: chrome strip, three-pane shell, steps.

use std::time::Duration;

use aui::composer::{composer, composer_state_rows};
use aui::nav::{compact_session_row, RowDensity};
use aui::orchestration::{
    goal_line, native_workflow, peer_message, plan_message, progress_block, run_note,
    run_question, run_summary, run_view, subagent_controls, worker_header, GoalLineFields,
    PeerIntent, ProgressIntent, RunQuestionIntent, RunViewIntent, WorkflowIntent,
    WorkerHeaderIntent,
};
use aui::protocol::{Block, RunQuestion};
use aui::shell::{app_shell, centre_header, right_header, sidebar_header, tab_strip, TabItem, TabStrip};
use aui::transcript::{prose, tool_card, user_turn, ProseStyle};
use aui_icons::{IconName, Provider};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::base::input::TextareaState;

use super::model::{
    scenarios, browser_url_string, run_view_matching, AgentRow, CentreItem, RightPane, Scenario,
    Step,
};

/// Centre transcript body text, matching the assistant mock.
pub const BODY_TEXT: f32 = 13.5;
/// Auto-advance tick for `Play`.
pub const PLAY_TICK: Duration = Duration::from_millis(2500);
/// How long the `AUI_GALLERY_STEPS` harness waits before dispatching.
pub const STEPS_DELAY: Duration = Duration::from_millis(250);
/// Chrome strip padding and the transcript's padding, mirroring the mock.
const CHROME_PAD_X: f32 = 12.0;
const CHROME_PAD_Y: f32 = 8.0;
const TRANSCRIPT_PAD_TOP: f32 = 8.0;
const TRANSCRIPT_PAD_X: f32 = 32.0;
const BLOCK_GAP: f32 = 16.0;

/// Which right-pane tab is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RightTab {
    Run,
    Browser,
}

/// The mock's state. The scenarios are owned copies: intents mutate the
/// current step (answer, expand, retry) and usually advance one step.
pub struct OrchestrationMock {
    scenarios: Vec<Scenario>,
    pub(super) scenario: usize,
    pub(super) index: usize,
    pub(super) playing: bool,
    /// Generation guard for the play loop (see `assistant/script.rs`).
    pub(super) play_run: u64,
    /// Generation guard for workflow retry timers.
    pub(super) retry_run: u64,
    /// A worker session opened over the current step (`OpenWorker`).
    worker_open: Option<WorkerContext>,
    /// Right-pane override set by intents; cleared on step change.
    right_override: Option<RightPane>,
    /// Active right tab; `None` follows the step.
    right_tab: Option<RightTab>,
    composer_msg: Entity<TextareaState>,
    composer_steer: Entity<TextareaState>,
    /// Peer messages with the reply expanded (`Show more`).
    peer_expanded: std::collections::HashSet<String>,
    focus_root: FocusHandle,
    /// `AUI_GALLERY_STEPS`, applied once after the first frame.
    steps: Vec<String>,
    started: bool,
    /// Timers keep running only while their task is alive.
    pub(super) tasks: Vec<Task<()>>,
}

/// A worker session opened from a progress or run-view row.
#[derive(Debug, Clone)]
struct WorkerContext {
    /// Task id that opened it.
    task_id: String,
    /// Stage label for the header.
    stage: String,
}

impl OrchestrationMock {
    /// Creates the mock on a scenario, running the steps harness when asked.
    pub fn new(scenario: usize, scripted: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer_msg = cx.new(|cx| composer_state_rows("Message…", 1, 8, window, cx));
        let composer_steer = cx.new(|cx| composer_state_rows("Steer this worker…", 1, 8, window, cx));
        let steps = if scripted { steps_from_env() } else { Vec::new() };
        Self {
            scenarios: scenarios(),
            scenario: scenario.min(5),
            index: 0,
            playing: false,
            play_run: 0,
            retry_run: 0,
            worker_open: None,
            right_override: None,
            right_tab: None,
            composer_msg,
            composer_steer,
            peer_expanded: std::collections::HashSet::new(),
            focus_root: cx.focus_handle(),
            steps,
            started: false,
            tasks: Vec::new(),
        }
    }

    fn current(&self) -> &Step {
        &self.scenarios[self.scenario].steps[self.index]
    }

    pub(super) fn current_mut(&mut self) -> &mut Step {
        &mut self.scenarios[self.scenario].steps[self.index]
    }

    fn step_count(&self) -> usize {
        self.scenarios[self.scenario].steps.len()
    }

    fn clear_overlays(&mut self) {
        self.worker_open = None;
        self.right_override = None;
        self.right_tab = None;
        self.peer_expanded.clear();
    }

    /// Advances one step; `false` when already on the last step.
    pub(super) fn advance(&mut self, cx: &mut Context<Self>) -> bool {
        if self.index + 1 < self.step_count() {
            self.index += 1;
            self.clear_overlays();
            cx.notify();
            true
        } else {
            false
        }
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if self.index > 0 {
            self.index -= 1;
            self.clear_overlays();
            cx.notify();
        }
    }

    fn goto(&mut self, n: usize, cx: &mut Context<Self>) {
        // `goto:<n>` is 1-based; clamp into range.
        let count = self.step_count();
        self.index = n.saturating_sub(1).min(count.saturating_sub(1));
        self.clear_overlays();
        cx.notify();
    }

    fn pick_scenario(&mut self, id: char, cx: &mut Context<Self>) {
        if let Some(s) = self.scenarios.iter().position(|s| s.id == id) {
            self.scenario = s;
            self.index = 0;
            self.playing = false;
            self.play_run += 1;
            self.clear_overlays();
            cx.notify();
        }
    }

    /// `RunQuestionIntent::Answer`: collapse to the answered line, advance.
    fn answer(&mut self, question_id: String, option_id: String, cx: &mut Context<Self>) {
        let current = self.current_mut();
        let mut label: Option<String> = None;
        let mut open_worker = false;
        for item in &mut current.items {
            let q = match item {
                CentreItem::Block(Block::RunQuestion(q)) if q.question_id == question_id => q,
                _ => continue,
            };
            answer_question(q, &option_id);
            label = q.answer.clone();
            open_worker = option_id == "open";
        }
        if label.is_some() {
            if open_worker {
                self.worker_open = Some(WorkerContext {
                    task_id: "t-auth-call".into(),
                    stage: "Implement · auth call".into(),
                });
            } else {
                self.advance(cx);
                return;
            }
            cx.notify();
        }
    }

    /// `PeerIntent::ToggleReply`: expand or collapse the reply.
    fn toggle_reply(&mut self, message_id: String, cx: &mut Context<Self>) {
        if !self.peer_expanded.remove(&message_id) {
            self.peer_expanded.insert(message_id);
        }
        cx.notify();
    }

    /// Whether a peer message renders with its reply expanded.
    fn is_expanded(&self, message_id: &str, item_expanded: bool) -> bool {
        item_expanded || self.peer_expanded.contains(message_id)
    }

    /// `ProgressIntent::OpenWorker` / `RunViewIntent::OpenWorker`.
    fn open_worker(&mut self, task_id: String, cx: &mut Context<Self>) {
        let stage = self
            .current()
            .items
            .iter()
            .filter_map(|item| match item {
                CentreItem::Block(Block::RunProgress(p)) => {
                    p.lines.iter().find(|l| l.task_id == task_id).map(|l| l.title.clone())
                }
                _ => None,
            })
            .next()
            .unwrap_or_else(|| task_id.clone());
        self.worker_open = Some(WorkerContext { task_id, stage });
        cx.notify();
    }
}

/// Collapses a run question to the picked option's label.
fn answer_question(q: &mut RunQuestion, option_id: &str) {
    let label = q.options.iter().find(|o| o.id == option_id).map(|o| o.label.clone()).unwrap_or_else(|| option_id.to_string());
    q.answer = Some(label);
    q.answered_at = Some("14:31".into());
}

/// `AUI_GALLERY_STEPS` split into steps, empty when the variable is unset.
fn steps_from_env() -> Vec<String> {
    std::env::var("AUI_GALLERY_STEPS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ---- rendering -----------------------------------------------------------

impl OrchestrationMock {
    fn prose_style(cx: &App) -> ProseStyle {
        let p = cx.aui().colors;
        ProseStyle {
            ink: p.ink,
            code_ink: p.accent_ink,
            code_bg: p.accent_soft,
            size: BODY_TEXT,
            line_height: scale::LH_BODY,
            paragraph_gap: 10.0,
        }
    }

    /// One centre item. Every click routes through the components' intents.
    fn render_item(&self, step_no: usize, item_no: usize, item: &CentreItem, cx: &mut Context<Self>) -> AnyElement {
        let id = SharedString::from(format!("orch-s{step_no}-i{item_no}"));
        match item {
            CentreItem::User(text) => div()
                .w_full()
                .flex()
                .justify_end()
                .child(user_turn(id, text.clone()).max_width_fraction(0.7))
                .into_any_element(),
            CentreItem::Agent(row) => self.render_agent_row(id, row, cx),
            CentreItem::Lead(text) => prose(id, text.as_str(), Self::prose_style(cx)).into_any_element(),
            CentreItem::Muted(text) => div()
                .w_full()
                .ui(scale::FS_12)
                .text_color(cx.aui().colors.ink_3)
                .child(text.clone())
                .into_any_element(),
            CentreItem::BackLink(label) => {
                let back = cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.worker_open = None;
                    cx.notify();
                });
                div()
                    .id(id)
                    .w_full()
                    .ui(scale::FS_12)
                    .text_color(cx.aui().colors.accent_ink)
                    .cursor_pointer()
                    .on_click(back)
                    .child(format!("← {label}"))
                    .into_any_element()
            }
            CentreItem::WorkerHead(header) => {
                let open = cx.listener(|this, _: &WorkerHeaderIntent, _, cx| {
                    this.right_override = Some(RightPane::Run(sample_run_view()));
                    cx.notify();
                });
                worker_header(id, header).on_intent(move |ev, w, cx| open(&ev, w, cx)).into_any_element()
            }
            CentreItem::Controls(controls) => {
                subagent_controls(id, controls).on_intent(|_, _, _| {}).into_any_element()
            }
            CentreItem::PeerExpanded(message) => {
                let toggle = cx.listener(|this, intent: &PeerIntent, _, cx| match intent {
                    PeerIntent::ToggleReply(mid) => this.toggle_reply(mid.clone(), cx),
                    PeerIntent::OpenSession(_) => {
                        if let Some(n) = this.current().peer_goto {
                            this.index = n;
                            this.clear_overlays();
                            cx.notify();
                        }
                    }
                });
                peer_message(id, message).expanded(true).on_intent(move |ev, w, cx| toggle(&ev, w, cx)).into_any_element()
            }
            CentreItem::Block(block) => self.render_block(id, block, cx),
        }
    }

    /// One protocol block, through the library component that owns it.
    fn render_block(&self, id: SharedString, block: &Block, cx: &mut Context<Self>) -> AnyElement {
        match block {
            Block::Text { text, .. } => prose(id, text.as_str(), Self::prose_style(cx)).into_any_element(),
            Block::RunPlan(plan) => plan_message(id, plan).into_any_element(),
            Block::RunProgress(progress) => {
                let intent = cx.listener(|this, intent: &ProgressIntent, _, cx| match intent {
                    ProgressIntent::OpenWorker(task_id) => this.open_worker(task_id.clone(), cx),
                    ProgressIntent::OpenRunView => {
                        // The progress header opens the Run tab with this
                        // step's own tasks.
                        let view = run_view_matching(&this.current().items);
                        this.right_override = Some(RightPane::Run(view));
                        this.right_tab = Some(RightTab::Run);
                        cx.notify();
                    }
                });
                progress_block(id, progress).on_intent(move |ev, w, cx| intent(&ev, w, cx)).into_any_element()
            }
            Block::RunQuestion(question) => {
                let qid = question.question_id.clone();
                let answer = cx.listener(move |this, intent: &RunQuestionIntent, _, cx| match intent {
                    RunQuestionIntent::Answer(option_id) => this.answer(qid.clone(), option_id.clone(), cx),
                });
                run_question(id, question).on_intent(move |ev, w, cx| answer(&ev, w, cx)).into_any_element()
            }
            Block::RunSummary(summary) => run_summary(id, summary).into_any_element(),
            Block::RunNote(note) => run_note(id, note).into_any_element(),
            Block::NativeWorkflow(workflow) => {
                let retry = cx.listener(|this, intent: &WorkflowIntent, window, cx| match intent {
                    WorkflowIntent::Retry { child_id, .. } => this.retry_child(child_id.clone(), window, cx),
                    WorkflowIntent::Skip { .. } => {}
                });
                native_workflow(id, workflow).on_intent(move |ev, w, cx| retry(&ev, w, cx)).into_any_element()
            }
            Block::PeerMessage(message) => {
                let expanded = self.is_expanded(&message.message_id, false);
                let toggle = cx.listener(|this, intent: &PeerIntent, _, cx| match intent {
                    PeerIntent::ToggleReply(mid) => this.toggle_reply(mid.clone(), cx),
                    PeerIntent::OpenSession(_) => {
                        if let Some(n) = this.current().peer_goto {
                            this.index = n;
                            this.clear_overlays();
                            cx.notify();
                        }
                    }
                });
                peer_message(id, message).expanded(expanded).on_intent(move |ev, w, cx| toggle(&ev, w, cx)).into_any_element()
            }
            Block::Goal {
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
            } => {
                let fields = GoalLineFields {
                    objective,
                    status,
                    percent_complete: *percent_complete,
                    provider: *provider,
                    tokens_used: *tokens_used,
                    token_budget: *token_budget,
                    elapsed: elapsed.as_deref(),
                    last_reason: last_reason.as_deref(),
                    gated_by_baaz: *gated_by_baaz,
                    reopened: *reopened,
                };
                goal_line(id, &fields).into_any_element()
            }
            Block::ToolCall { id: call_id, verb, target, status, duration_ms, body, .. } => {
                tool_card(format!("{id}-{call_id}"), verb.clone(), target.clone(), *status, body.clone())
                    .duration_ms(*duration_ms)
                    .into_any_element()
            }
            other => div()
                .w_full()
                .ui(scale::FS_12)
                .text_color(cx.aui().colors.ink_3)
                .child(format!("(unshown block: {})", block_kind_name(other)))
                .into_any_element(),
        }
    }

    /// The composed delegation row: a status dot, `Agent · <tool>`, muted state.
    fn render_agent_row(&self, id: SharedString, row: &AgentRow, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        h_flex()
            .id(id)
            .w_full()
            .items_center()
            .gap(px(8.0))
            .px(px(2.0))
            .py(px(6.0))
            .child(div().size(px(8.0)).rounded(px(4.0)).bg(if row.running { p.accent } else { p.ink_3 }))
            .child(
                div()
                    .ui(scale::FS_13)
                    .text_color(p.ink)
                    .child(format!("Agent · {}", row.tool)),
            )
            .child(div().ui(scale::FS_12).text_color(p.ink_3).child(row.state.to_string()))
            .into_any_element()
    }

    /// Unrelated idle rows below the scenario's rows, for realism.
    fn idle_rows() -> Vec<aui::nav::SessionSummary> {
        vec![
            aui::nav::SessionSummary::new("idle-terminal", "Terminal grid follow-ups", AgentState::Idle, "3h")
                .density(RowDensity::One),
            aui::nav::SessionSummary::new("idle-browser", "Browser B2b polish", AgentState::Idle, "1d")
                .density(RowDensity::One),
        ]
    }

    /// Rows stack from the top at their natural height: no `flex_1`, no
    /// `size_full`, on any row or wrapper.
    fn render_sidebar(&self, step: &Step, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut col = v_flex().w_full().gap(px(2.0)).p(px(8.0));
        col = col.child(
            div()
                .w_full()
                .px(px(4.0))
                .pb(px(4.0))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("HARNESS".to_string()),
        );
        for row in &step.sidebar {
            col = col.child(self.render_row(row, step, 0, cx));
        }
        for (index, idle) in Self::idle_rows().iter().enumerate() {
            let row_id = SharedString::from(format!("orch-idle-{index}"));
            col = col.child(
                compact_session_row(row_id, idle.clone())
                    .density(idle.density.unwrap_or(RowDensity::One))
                    .selected(false)
                    .into_any_element(),
            );
        }
        col
    }

    fn render_row(&self, row: &aui::nav::SessionSummary, step: &Step, depth: usize, cx: &mut Context<Self>) -> AnyElement {
        let selected = match &self.worker_open {
            Some(w) => row.id.as_ref() == w.task_id || row.id.as_ref() == "worker-auth",
            None => row.id.as_ref() == step.active_row,
        };
        let row_id = SharedString::from(format!("orch-row-{}-{depth}", row.id));
        let select = cx.listener(|this, id: &SharedString, _, cx| {
            let id = id.to_string();
            if id.starts_with("worker-") || id.starts_with('t') {
                this.worker_open = Some(WorkerContext { task_id: id.clone(), stage: id });
            } else {
                this.worker_open = None;
            }
            cx.notify();
        });
        let el = compact_session_row(row_id, row.clone())
            .density(row.density.unwrap_or(RowDensity::One))
            .selected(selected)
            .on_select(move |id, w, cx| select(id, w, cx));
        let _ = depth;
        el.into_any_element()
    }

    /// The static browser placeholder: a URL row plus a login form from divs.
    fn render_browser(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        let field = |label: &str| {
            v_flex().w_full().gap(px(4.0)).child(div().ui(scale::FS_12).text_color(p.ink_2).child(label.to_string())).child(
                div().w_full().h(px(32.0)).rounded(px(6.0)).border_1().border_color(p.line).bg(p.surface_2),
            )
        };
        let email_field = v_flex()
            .w_full()
            .gap(px(4.0))
            .child(div().ui(scale::FS_12).text_color(p.ink_2).child("Email".to_string()))
            .child(
                div()
                    .w_full()
                    .h(px(32.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(p.danger)
                    .bg(p.surface_2),
            )
            .child(
                div()
                    .ui(scale::FS_12)
                    .text_color(p.danger)
                    .child("Enter a valid email".to_string()),
            );
        v_flex()
            .size_full()
            .bg(p.surface_1)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_b_1()
                    .border_color(p.line)
                    .bg(p.surface_2)
                    .child(div().mono(scale::FS_12).text_color(p.ink_2).child(browser_url_string())),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .items_start()
                    .justify_center()
                    .pt(px(48.0))
                    .child(
                        v_flex()
                            .w(px(300.0))
                            .gap(px(12.0))
                            .p(px(20.0))
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(p.line)
                            .bg(p.surface_2)
                            .child(div().ui(scale::FS_13).semibold().text_color(p.ink).child("Log in"))
                            .child(email_field)
                            .child(field("Password"))
                            .child(
                                div()
                                    .w_full()
                                    .h(px(34.0))
                                    .rounded(px(6.0))
                                    .bg(p.accent)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .ui(scale::FS_13)
                                    .semibold()
                                    .text_color(p.surface_1)
                                    .child("Sign in"),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// The effective right pane: the intent override wins over the step.
    fn right_pane(&self, step: &Step) -> RightPane {
        self.right_override.clone().unwrap_or_else(|| step.right.clone())
    }

    fn right_open(&self, step: &Step) -> bool {
        !matches!(self.right_pane(step), RightPane::Closed)
    }

    /// The active right tab; follows the pane unless the person switched.
    fn active_tab(&self, step: &Step) -> RightTab {
        self.right_tab.unwrap_or(match self.right_pane(step) {
            RightPane::Run(_) => RightTab::Run,
            _ => RightTab::Browser,
        })
    }

    /// The tab strip for the right header: a `Run` tab and a `Browser` tab.
    fn render_right_tabs(&self, step: &Step, cx: &mut Context<Self>) -> TabStrip {
        let active = self.active_tab(step);
        let tabs = vec![
            TabItem::new("run", "Run", IconName::Doc).closable(false),
            TabItem::new("browser", "Browser", IconName::Globe).closable(false),
        ];
        let selected = match active {
            RightTab::Run => 0,
            RightTab::Browser => 1,
        };
        tab_strip("orch-right-tabs", tabs, selected).on_select(cx.listener(|this, id: &SharedString, _, cx| {
            this.right_tab = Some(if id.as_ref() == "browser" { RightTab::Browser } else { RightTab::Run });
            cx.notify();
        }))
    }

    /// The right body: `run_view` on the Run tab, the placeholder on Browser.
    /// The Run tab always matches the step's own progress lines.
    fn render_right_body(&self, step: &Step, cx: &mut Context<Self>) -> AnyElement {
        match self.active_tab(step) {
            RightTab::Browser => self.render_browser(cx),
            RightTab::Run => {
                let view = match self.right_pane(step) {
                    RightPane::Run(view) => view,
                    _ => run_view_matching(&step.items),
                };
                let open = cx.listener(|this, intent: &RunViewIntent, _, cx| match intent {
                    RunViewIntent::OpenWorker(task_id) => this.open_worker(task_id.clone(), cx),
                });
                run_view("orch-run-view", &view).on_intent(move |ev, w, cx| open(&ev, w, cx)).into_any_element()
            }
        }
    }

    fn render_chrome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut picker = h_flex().items_center().gap(px(4.0));
        for s in &self.scenarios {
            let active = s.id == self.scenarios[self.scenario].id;
            let id = s.id;
            let name = s.name;
            let pick = cx.listener(move |this, _: &gpui::ClickEvent, _, cx| this.pick_scenario(id, cx));
            picker = picker.child(
                div()
                    .id(SharedString::from(format!("orch-pick-{}", s.id)))
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .bg(if active { p.surface_1 } else { p.surface_3 })
                    .ui(scale::FS_12)
                    .text_color(if active { p.ink } else { p.ink_2 })
                    .on_click(pick)
                    .child(name.to_string()),
            );
        }
        let back = cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
            this.pause(cx);
            this.back(cx);
        });
        let next = cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
            this.pause(cx);
            this.advance(cx);
        });
        let play = cx.listener(|this, _: &gpui::ClickEvent, window, cx| this.toggle_play(window, cx));
        let steps_total = self.step_count();
        let chrome_btn = |id: &str, label: &str| {
            div()
                .id(SharedString::from(format!("orch-{id}")))
                .px(px(8.0))
                .py(px(2.0))
                .rounded(px(6.0))
                .cursor_pointer()
                .ui(scale::FS_12)
                .text_color(p.ink_2)
                .child(label.to_string())
        };
        v_flex()
            .w_full()
            .bg(p.surface_3)
            .border_b_1()
            .border_color(p.line)
            .px(px(CHROME_PAD_X))
            .py(px(CHROME_PAD_Y))
            .gap(px(4.0))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(8.0))
                    .child(picker)
                    .child(chrome_btn("back", "Back").on_click(back))
                    .child(chrome_btn("next", "Next").on_click(next))
                    .child(chrome_btn("play", if self.playing { "Pause" } else { "Play" }).on_click(play))
                    .child(
                        div()
                            .mono(scale::FS_12)
                            .text_color(p.ink_2)
                            .child(format!("Step {} of {steps_total}", self.index + 1)),
                    ),
            )
            .child(div().ui(scale::FS_12).text_color(p.ink_2).child(self.current().narration.to_string()))
    }

    // ---- the AUI_GALLERY_STEPS harness ----------------------------------

    /// Applies one step. Every step goes through the same handler a click or
    /// keystroke does, never straight into the model.
    fn apply_step(&mut self, step: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(letter) = step.strip_prefix("scenario:") {
            if let Some(id) = letter.chars().next() {
                self.pick_scenario(id.to_ascii_uppercase(), cx);
            }
            return;
        }
        if let Some(n) = step.strip_prefix("goto:") {
            self.goto(n.parse::<usize>().unwrap_or(1), cx);
            return;
        }
        match step {
            "next" => {
                self.advance(cx);
            }
            "back" => self.back(cx),
            "play" => self.toggle_play(window, cx),
            "shot" | "" => {}
            other => eprintln!("AUI_GALLERY_STEPS: unknown step `{other}`"),
        }
    }

    /// Starts the harness after the first frame.
    fn start_steps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let steps = std::mem::take(&mut self.steps);
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            cx.background_executor().timer(STEPS_DELAY).await;
            for step in steps {
                if let Some(ms) = step.strip_prefix("wait:") {
                    let ms: u64 = ms.parse().unwrap_or(0);
                    cx.background_executor().timer(Duration::from_millis(ms)).await;
                    continue;
                }
                if this.update_in(cx, |this, window, cx| this.apply_step(&step, window, cx)).is_err() {
                    return;
                }
            }
        });
        self.tasks.push(task);
    }
}

/// A static sample of the run tree for intent-opened panes.
fn sample_run_view() -> aui::protocol::RunViewModel {
    aui::protocol::sample::run_view_sample()
}

/// The kind name of a block the transcript does not render here.
fn block_kind_name(block: &Block) -> &'static str {
    match block {
        Block::Text { .. } => "text",
        Block::Thinking { .. } => "thinking",
        Block::Activity { .. } => "activity",
        Block::ToolCall { .. } => "tool-call",
        Block::ToolGroup { .. } => "tool-group",
        Block::Approval { .. } => "approval",
        Block::Question { .. } => "question",
        Block::Plan { .. } => "plan",
        Block::Todo { .. } => "todo",
        Block::Summary { .. } => "summary",
        Block::Error { .. } => "error",
        Block::Goal { .. } => "goal",
        Block::Generic { .. } => "generic",
        Block::Marker { .. } => "marker",
        Block::Handoff { .. } => "handoff",
        Block::RunPlan(_) => "run-plan",
        Block::RunProgress(_) => "run-progress",
        Block::RunQuestion(_) => "run-question",
        Block::RunSummary(_) => "run-summary",
        Block::RunNote(_) => "run-note",
        Block::NativeWorkflow(_) => "workflow",
        Block::PeerMessage(_) => "peer",
    }
}

// ---- shell -----------------------------------------------------------------

impl OrchestrationMock {
    /// The centre transcript: worker overlay wins over the step when open.
    fn render_transcript(&self, step_no: usize, cx: &mut Context<Self>) -> AnyElement {
        let items: Vec<(usize, CentreItem)> = match &self.worker_open {
            Some(w) => worker_items(&w.stage),
            None => self.current().items.iter().cloned().enumerate().collect(),
        };
        let mut list = div()
            .id("orch-transcript")
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pt(px(TRANSCRIPT_PAD_TOP))
            .px(px(TRANSCRIPT_PAD_X))
            .gap(px(BLOCK_GAP));
        for (item_no, item) in &items {
            list = list.child(self.render_item(step_no, *item_no, item, cx));
        }
        list.into_any_element()
    }

    fn render_composer(&self) -> AnyElement {
        let step = self.current();
        let steer = self.worker_open.is_some() || step.steer;
        let state = if steer { &self.composer_steer } else { &self.composer_msg };
        let provider = step.centre_provider;
        let model_label = match provider {
            Provider::Claude => "Opus 5.5",
            Provider::Codex => "codex-5.2",
            Provider::Muse => "muse-spark-1.3",
            _ => "default",
        };
        composer("orch-composer", state, provider, model_label).docked(true).into_any_element()
    }

    fn render_goal_pin(&self) -> Option<AnyElement> {
        let pin = self.current().goal_pin.clone()?;
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
        } = pin
        else {
            return None;
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
        Some(
            div()
                .w_full()
                .px(px(TRANSCRIPT_PAD_X))
                .pb(px(8.0))
                .child(goal_line("orch-goal-pin", &fields))
                .into_any_element(),
        )
    }
}

/// The worker session view for an opened task: header, a few tool cards, a
/// native subagent card and the steer composer.
fn worker_items(stage: &str) -> Vec<(usize, CentreItem)> {
    let header = aui::protocol::WorkerHeader {
        run_title: "Login screen".into(),
        stage: stage.into(),
        attempt: 1,
        provider: aui::protocol::Provider::Muse,
        model: "muse-spark-1.3".into(),
        branch: "baaz/login-screen".into(),
    };
    vec![
        (0, CentreItem::BackLink("Back to lead session".into())),
        (1, CentreItem::WorkerHead(header)),
        (2, CentreItem::Lead("Working the task against the agreed contract.".into())),
        (
            3,
            CentreItem::Block(Block::ToolCall {
                id: "tc-worker-edit".into(),
                kind: aui::protocol::ToolKind::Edit,
                verb: "Edited".into(),
                target: "src/login/Form.tsx".into(),
                status: aui::protocol::ToolStatus::Success,
                duration_ms: Some(4_200),
                body: aui::protocol::ToolBody::Read { lines: 18 },
                diff_stat: None,
            }),
        ),
        (
            4,
            CentreItem::Block(Block::ToolCall {
                id: "tc-worker-test".into(),
                kind: aui::protocol::ToolKind::Shell,
                verb: "Ran".into(),
                target: "npm test".into(),
                status: aui::protocol::ToolStatus::Success,
                duration_ms: Some(8_100),
                body: aui::protocol::ToolBody::Shell {
                    output_lines: vec!["12 passed".into()],
                    exit_code: Some(0),
                    live: false,
                },
                diff_stat: None,
            }),
        ),
        (
            5,
            CentreItem::Block(Block::ToolCall {
                id: "tc-worker-subagent".into(),
                kind: aui::protocol::ToolKind::SubAgent,
                verb: "Delegated".into(),
                target: "form-checks".into(),
                status: aui::protocol::ToolStatus::Success,
                duration_ms: Some(21_000),
                body: aui::protocol::ToolBody::SubAgent { turns: Vec::new() },
                diff_stat: None,
            }),
        ),
    ]
}

impl Render for OrchestrationMock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.started {
            self.started = true;
            window.focus(&self.focus_root, cx);
            if !self.steps.is_empty() {
                self.start_steps(window, cx);
            }
        }
        // Clone the step: the render tree borrows nothing from `self`.
        let step_no = self.index;
        let step = self.current().clone();

        let sidebar = self.render_sidebar(&step, cx);
        let transcript = self.render_transcript(step_no, cx);
        let goal_pin = self.render_goal_pin();
        let composer_el = self.render_composer();
        let right_open = self.right_open(&step);
        let right_body = self.render_right_body(&step, cx);
        let right_tabs = self.render_right_tabs(&step, cx);

        let centre_title = match &self.worker_open {
            Some(w) => SharedString::from(w.stage.clone()),
            None => SharedString::from(step.centre_title),
        };
        // Keyboard on the root: ← and → step, space plays and pauses.
        let keys = cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| match event.keystroke.key.as_str() {
            "left" => {
                this.pause(cx);
                this.back(cx);
            }
            "right" => {
                this.pause(cx);
                this.advance(cx);
            }
            "space" => this.toggle_play(window, cx),
            _ => {}
        });
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .key_context("orch-root")
            .track_focus(&self.focus_root)
            .on_key_down(keys)
            .child(self.render_chrome(cx))
            .child(
                div().flex_1().min_h(px(0.0)).w_full().child(
                    app_shell("orch-shell")
                        .traffic_lights(true)
                        .right_open(right_open)
                        .sidebar_open(true)
                        .header_sidebar(
                            sidebar_header("orch-hd-side").traffic_lights(true).collapsed(false).on_toggle_sidebar(
                                cx.listener(|_, _, _, _| {}),
                            ),
                        )
                        .header_centre(
                            centre_header("orch-hd-centre", centre_title).provider(step.centre_provider).on_toggle_right(
                                cx.listener(|this, _, _, cx| {
                                    if this.right_override.is_some() || !matches!(this.current().right, RightPane::Closed) {
                                        this.right_override = Some(RightPane::Closed);
                                    } else {
                                        this.right_override = Some(RightPane::Run(sample_run_view()));
                                    }
                                    cx.notify();
                                }),
                            ),
                        )
                        .header_right(
                            right_header("orch-hd-right").tabs(right_tabs).on_add(|_, _, _| {}).on_close(cx.listener(
                                |this, _, _, cx| {
                                    this.right_override = Some(RightPane::Closed);
                                    cx.notify();
                                },
                            )),
                        )
                        .sidebar(sidebar)
                        .centre(
                            v_flex()
                                .size_full()
                                .child(transcript)
                                .children(goal_pin)
                                .child(composer_el),
                        )
                        .right(right_body),
                ),
            )
    }
}

/// One mock in window state.
pub fn mock(key: &'static str, scenario: usize, scripted: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let view = window.use_keyed_state(SharedString::from(key), cx, move |window, cx| OrchestrationMock::new(scenario, scripted, window, cx));
    div().size_full().child(view).into_any_element()
}

/// Reads `AUI_GALLERY_SCENARIO` (`A`–`F`, default `A`) for parity renders.
fn scenario_from_env() -> usize {
    match std::env::var("AUI_GALLERY_SCENARIO").unwrap_or_default().as_str() {
        "B" | "b" => 1,
        "C" | "c" => 2,
        "D" | "d" => 3,
        "E" | "e" => 4,
        "F" | "f" => 5,
        _ => 0,
    }
}

/// The `screens/orchestration` entry.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    // `AUI_GALLERY_STEPS` may carry `scenario:<A-F>` itself; the env default
    // only matters when it does not.
    mock("orchestration-mock", scenario_from_env(), true, window, cx)
}

