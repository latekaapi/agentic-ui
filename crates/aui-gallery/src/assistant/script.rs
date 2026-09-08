//! A scripted fake backend for the assistant mock.
//!
//! There is no agent behind the mock, so a turn is played out by a small
//! script: `gpui::Timer`s on the window's executor push the same model changes
//! a real backend would stream in. Every entry point here is the handler a UI
//! intent calls, so the `AUI_GALLERY_STEPS` harness in [`super::view`] drives
//! the mock through exactly the code path a click or a keystroke does.
//!
//! Three scenarios, picked from the text that was sent:
//!
//! | text | scenario |
//! |---|---|
//! | contains `?` | a question card, then the default scenario |
//! | starts with `write` / `update` / `change` | an approval card, then the default scenario |
//! | anything else | the default scenario |
//!
//! The default scenario is: an activity group live for ~1.2 s with its steps
//! flipping to done one at a time, then an answer streaming word groups at
//! ~40 ms each, then the status row back to "Done".

use std::time::Duration;

use aui::feedback::{ToastData, ToastKind};
use aui::protocol::{ActivityState, ApprovalDecision, ApprovalState, QuestionOption, StepState};
use aui_tokens::AgentState;
use gpui::{Context, SharedString, Window};

use super::model::{pending, word_groups, Block, BlockKind, Status};
use super::view::{AssistantMock, Focus};

/// How long each activity step runs before it flips to done; three of them
/// make the ~1.2 s live activity the scenario asks for.
pub const STEP_TIME: Duration = Duration::from_millis(400);
/// One word group of the answer per tick.
pub const WORD_TIME: Duration = Duration::from_millis(40);
/// How long a toast stays before it dismisses itself.
pub const TOAST_TIME: Duration = Duration::from_millis(4000);
/// How often the toast timer wakes to check whether the pointer is holding it.
pub const TOAST_TICK: Duration = Duration::from_millis(100);
/// How long the `AUI_GALLERY_STEPS` harness waits for the first frame before
/// it starts dispatching.
pub const STEPS_DELAY: Duration = Duration::from_millis(250);

/// The answer the default scenario streams.
const ANSWER: &str = "I re-read the matrix in Annex A and applied the weights you gave me[[1]]. Northlight still leads once experience is weighted at 45, and Civic Talent moves up two places on price[[3]]. The weights live in row 7, so the totals follow anything you change there.";
/// The command the approval scenario asks about.
const APPROVAL_TOOL: &str = "Write file";
const APPROVAL_COMMAND: &str = "Write vendor-scoring.xlsx";
const APPROVAL_REASON: &str = "The weights changed, so the Scores sheet has to be rewritten with the new totals.";
/// The rule an "always allow" remembers.
const APPROVAL_RULE: &str = "Write *.xlsx";
/// What the denied branch answers with.
const DENIED_ANSWER: &str = "Left the sheet unchanged.";
/// The toast an approved write raises.
const TOAST_TITLE: &str = "Saved vendor-scoring.xlsx";
const TOAST_BODY: &str = "Scores, Matrix and Notes \u{b7} v2";

/// Which scenario a prompt starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// Activity, then a streaming answer.
    Default,
    /// A question card first.
    Question,
    /// An approval card first.
    Approval,
}

/// Picks the scenario for a prompt. A question wins over a write, so
/// "update the weights?" asks before it writes.
pub fn scenario(text: &str) -> Scenario {
    if text.contains('?') {
        return Scenario::Question;
    }
    let lower = text.trim_start().to_lowercase();
    if ["write", "update", "change"].iter().any(|verb| lower.starts_with(verb)) {
        Scenario::Approval
    } else {
        Scenario::Default
    }
}

impl AssistantMock {
    /// `ComposerIntent::Send`: append the user's turn and start a scenario.
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).value();
        if text.trim().is_empty() {
            return;
        }
        self.composer.update(cx, |state, cx| state.set_value("", window, cx));
        self.plus_open = false;
        self.run += 1;
        let run = self.run;
        self.transcript.blocks.push(Block::new(format!("assistant-user-{run}"), BlockKind::UserTurn { text: text.clone() }));
        match scenario(&text) {
            Scenario::Question => self.ask_question(run),
            Scenario::Approval => self.ask_approval(run),
            Scenario::Default => self.run_default(run, window, cx),
        }
        cx.notify();
    }

    /// `ComposerIntent::Stop`: freeze the streaming answer where it is and
    /// mark the turn done. The streaming task sees the cleared flag and stops.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.transcript.streaming_answer() {
            if let BlockKind::Answer { streaming, .. } = &mut self.transcript.blocks[index].kind {
                *streaming = false;
            }
        }
        self.finish();
        cx.notify();
    }

    /// The status row after a turn ends.
    fn finish(&mut self) {
        self.transcript.status = Status { state: AgentState::Done, label: "Done".into(), detail: "3 sources cited".into() };
    }

    /// The approval branch: ask before writing the sheet.
    fn ask_approval(&mut self, run: u64) {
        self.transcript.blocks.push(Block::new(
            format!("assistant-approval-{run}"),
            BlockKind::Approval {
                tool: APPROVAL_TOOL.into(),
                command: APPROVAL_COMMAND.into(),
                reason: APPROVAL_REASON.into(),
                state: ApprovalState::Pending,
            },
        ));
        self.transcript.status = Status { state: AgentState::Waiting, label: "Needs you".into(), detail: "waiting for permission".into() };
        self.want_focus = Some(Focus::Approval);
    }

    /// Y / A / N on the newest pending approval card.
    pub fn decide(&mut self, decision: ApprovalDecision, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.transcript.pending_approval() else { return };
        let run = self.run;
        // The script only ever emits the built-in triad, but `ApprovalDecision`
        // carries the wider MSP set, so classify rather than compare.
        let approved = !matches!(
            decision,
            ApprovalDecision::Deny
                | ApprovalDecision::DeniedPolicyAmendment
                | ApprovalDecision::TimedOut
                | ApprovalDecision::Abort
        );
        if let BlockKind::Approval { state, .. } = &mut self.transcript.blocks[index].kind {
            *state = match decision {
                ApprovalDecision::Always | ApprovalDecision::PolicyAmendment => {
                    ApprovalState::AutoAllowed { rule: APPROVAL_RULE.to_string() }
                }
                _ if approved => ApprovalState::AllowedOnce { exit_code: 0, duration_ms: 420 },
                _ => ApprovalState::Denied,
            };
        }
        self.want_focus = Some(Focus::Composer);
        if approved {
            self.raise_toast(ToastData::new(format!("saved-{run}"), TOAST_TITLE, TOAST_BODY).kind(ToastKind::Ok), window, cx);
            self.run_default(run, window, cx);
        } else {
            self.transcript.blocks.push(Block::new(
                format!("assistant-answer-{run}"),
                BlockKind::Answer { text: DENIED_ANSWER.into(), revealed: None, streaming: false },
            ));
            self.finish();
        }
        cx.notify();
    }

    /// The question branch: ask which weighting to use.
    fn ask_question(&mut self, run: u64) {
        let option = |label: &str, description: &str, key: &str| QuestionOption {
            label: label.to_string(),
            description: description.to_string(),
            key: key.to_string(),
            preview: None,
        };
        self.transcript.blocks.push(Block::new(
            format!("assistant-question-{run}"),
            BlockKind::Question {
                prompt: "Which weighting should I score against?".into(),
                subtitle: "The brief and Annex A disagree, so pick one before I touch the sheet.".into(),
                options: vec![
                    option("Annex A matrix", "45 experience, 30 coverage, 25 price", "1"),
                    option("Equal weights", "a third each, as the brief's appendix has it", "2"),
                    option("Price-led", "50 price, 30 experience, 20 coverage", "3"),
                ],
                selected: None,
                answered: None,
            },
        ));
        self.transcript.status = Status { state: AgentState::Waiting, label: "Needs you".into(), detail: "waiting on an answer".into() };
        self.want_focus = Some(Focus::Question);
    }

    /// Moves the highlight inside the newest pending question card.
    pub fn move_question(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(index) = self.transcript.pending_question() else { return };
        if let BlockKind::Question { options, selected, .. } = &mut self.transcript.blocks[index].kind {
            let count = options.len() as isize;
            let current = selected.map(|s| s as isize).unwrap_or(if delta > 0 { -1 } else { count });
            *selected = Some(((current + delta).rem_euclid(count)) as usize);
            cx.notify();
        }
    }

    /// Picks an option on the newest pending question card, which resolves it
    /// to an answered row and starts the default scenario.
    pub fn answer_question(&mut self, choice: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.transcript.pending_question() else { return };
        let run = self.run;
        let mut label = None;
        if let BlockKind::Question { options, selected, answered, .. } = &mut self.transcript.blocks[index].kind {
            let choice = choice.min(options.len().saturating_sub(1));
            *selected = Some(choice);
            let chosen: SharedString = options[choice].label.clone().into();
            *answered = Some(chosen.clone());
            label = Some(chosen);
        }
        if label.is_some() {
            self.want_focus = Some(Focus::Composer);
            self.run_default(run, window, cx);
            cx.notify();
        }
    }

    /// Confirms whatever the question card has highlighted.
    pub fn confirm_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.transcript.pending_question() else { return };
        let choice = match &self.transcript.blocks[index].kind {
            BlockKind::Question { selected, .. } => selected.unwrap_or(0),
            _ => 0,
        };
        self.answer_question(choice, window, cx);
    }

    /// The default scenario: a live activity group, then a streaming answer.
    pub fn run_default(&mut self, run: u64, window: &mut Window, cx: &mut Context<Self>) {
        let id = format!("assistant-activity-{run}");
        self.transcript.blocks.push(Block::new(
            id,
            BlockKind::Activity {
                steps: vec![
                    pending("Read", "Annex A", None),
                    pending("Rebuilt", "the scoring matrix", None),
                    pending("Wrote", "vendor-scoring.xlsx", None),
                ],
                summary: "Reading Annex A".into(),
                detail: None,
                elapsed: "0 s".into(),
                state: ActivityState::Working,
                open: true,
            },
        ));
        let activity = self.transcript.blocks.len() - 1;
        self.transcript.status = Status { state: AgentState::Running, label: "Working\u{2026}".into(), detail: "3 steps".into() };

        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            for step in 0..3usize {
                cx.background_executor().timer(STEP_TIME).await;
                let alive = this
                    .update(cx, |this, cx| {
                        if this.run != run {
                            return false;
                        }
                        if let Some(BlockKind::Activity { steps, summary, elapsed, .. }) = this.transcript.blocks.get_mut(activity).map(|b| &mut b.kind) {
                            steps[step].state = StepState::Done;
                            if let Some(next) = steps.get(step + 1) {
                                *summary = format!("{} {}", next.verb, next.target).into();
                            }
                            *elapsed = format!("{} s", step + 1).into();
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    return;
                }
            }
            // The activity folds back up and the answer starts arriving.
            let total = this
                .update(cx, |this, cx| {
                    if this.run != run {
                        return 0;
                    }
                    if let Some(BlockKind::Activity { summary, detail, elapsed, state, open, .. }) = this.transcript.blocks.get_mut(activity).map(|b| &mut b.kind) {
                        *summary = "Read Annex A".into();
                        *detail = Some("\u{b7} rebuilt the matrix \u{b7} wrote vendor-scoring.xlsx".into());
                        *elapsed = "3 s".into();
                        *state = ActivityState::Done;
                        *open = false;
                    }
                    this.transcript.blocks.push(Block::new(
                        format!("assistant-answer-{run}"),
                        BlockKind::Answer { text: ANSWER.into(), revealed: Some(0), streaming: true },
                    ));
                    this.transcript.status = Status { state: AgentState::Running, label: "Working\u{2026}".into(), detail: "writing the answer".into() };
                    cx.notify();
                    word_groups(ANSWER).len()
                })
                .unwrap_or(0);

            for n in 1..=total {
                cx.background_executor().timer(WORD_TIME).await;
                let running = this
                    .update(cx, |this, cx| {
                        if this.run != run {
                            return false;
                        }
                        let Some(index) = this.transcript.streaming_answer() else { return false };
                        if let BlockKind::Answer { revealed, .. } = &mut this.transcript.blocks[index].kind {
                            *revealed = Some(n);
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !running {
                    return;
                }
            }
            let _ = this.update(cx, |this, cx| {
                if this.run != run {
                    return;
                }
                if let Some(index) = this.transcript.streaming_answer() {
                    if let BlockKind::Answer { revealed, streaming, .. } = &mut this.transcript.blocks[index].kind {
                        *revealed = None;
                        *streaming = false;
                    }
                }
                this.finish();
                cx.notify();
            });
        });
        self.tasks.push(task);
    }

    /// Pushes a toast into the shell's stack and dismisses it after
    /// [`TOAST_TIME`]; the timer holds while the pointer is over the stack.
    pub fn raise_toast(&mut self, data: ToastData, window: &mut Window, cx: &mut Context<Self>) {
        let id = data.id.clone();
        self.toasts.push(data);
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let mut waited = Duration::ZERO;
            while waited < TOAST_TIME {
                cx.background_executor().timer(TOAST_TICK).await;
                let held = this.update(cx, |this, _| this.toast_hovered).unwrap_or(true);
                if !held {
                    waited += TOAST_TICK;
                }
                if this.update(cx, |this, _| !this.toasts.iter().any(|t| t.id == id)).unwrap_or(true) {
                    return;
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.toasts.retain(|t| t.id != id);
                cx.notify();
            });
        });
        self.tasks.push(task);
    }
}
