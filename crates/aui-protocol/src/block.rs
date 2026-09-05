//! Blocks: one variant per card in the transcript.

use serde::{Deserialize, Serialize};

use crate::intent::ApprovalDecision;
use crate::tool::{ToolBody, ToolKind, ToolStatus};

/// One renderable unit inside an assistant turn.
///
/// Each variant maps to a card in `docs/02-component-spec.md` section 3.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    /// Prose from the assistant (card 31). Markdown, rendered inline.
    Text {
        /// The text so far.
        text: String,
        /// `true` while chunks are still arriving, which shows the caret.
        streaming: bool,
    },
    /// The reasoning trace (card 32).
    Thinking {
        /// Full trace text.
        text: String,
        /// Time spent thinking, in milliseconds.
        elapsed_ms: u64,
        /// One-line model-written summary shown once collapsed.
        summary: Option<String>,
        /// Whether the trace is still growing.
        state: ThinkingState,
    },
    /// A group of quick steps rolled into one card (card 33).
    Activity {
        /// The steps, in execution order.
        steps: Vec<Step>,
        /// Collapsed header line, e.g. `"Ran 3 commands"`.
        summary: String,
        /// Total time for the group, in milliseconds.
        elapsed_ms: u64,
        /// Whether the group is still running.
        state: ActivityState,
    },
    /// One tool invocation with its own body (card 34).
    ToolCall {
        /// Stable id, used to address the call in deltas and intents.
        id: String,
        /// Which tool ran, which picks the icon and the body shape.
        ///
        /// Serialized as `tool_kind`: the enum's own tag already owns `kind`.
        #[serde(rename = "tool_kind")]
        kind: ToolKind,
        /// Header verb, e.g. `"Ran"`, `"Edited"`, `"Searched"`.
        verb: String,
        /// Mono header target, e.g. a command or a path.
        target: String,
        /// Current status, which drives the glyph and the result pill.
        status: ToolStatus,
        /// Duration in milliseconds; `None` while still running.
        duration_ms: Option<u64>,
        /// Tool-specific payload.
        body: ToolBody,
    },
    /// A permission request the person must resolve (card 35).
    Approval {
        /// Stable id, echoed back in [`crate::Intent::Approve`].
        id: String,
        /// Tool name shown in the definition list, e.g. `"Bash"`.
        tool: String,
        /// The exact command that would run.
        command: String,
        /// Why the agent wants it, in its own words.
        reason: String,
        /// Directory the command would run in.
        cwd: String,
        /// Capabilities being granted, e.g. `["modify files", "network"]`.
        capabilities: Vec<String>,
        /// How far an "always allow" would reach.
        scope: ApprovalScope,
        /// Where the request is in its lifecycle.
        state: ApprovalState,
        /// The rule an "always allow" would remember, e.g. `"apt install"`.
        rule: Option<String>,
    },
    /// A structured question with options (card 36).
    Question {
        /// Stable id, echoed back in [`crate::Intent::Answer`].
        id: String,
        /// The question itself.
        prompt: String,
        /// Supporting line under the prompt.
        subtitle: String,
        /// Choices, in display order.
        options: Vec<QuestionOption>,
        /// `true` for checkboxes, `false` for radios.
        multi: bool,
        /// Whether a free-text "Other" row is offered.
        allow_other: bool,
        /// The answer once given; `None` while pending.
        answer: Option<Answer>,
    },
    /// A proposed plan awaiting acceptance (card 36).
    Plan {
        /// Stable id, echoed back in the plan intents.
        id: String,
        /// Plan steps, in order.
        items: Vec<String>,
        /// Whether the plan is still awaiting a decision.
        state: PlanState,
    },
    /// The agent's task list (card 36).
    Todo {
        /// The tasks, in order.
        items: Vec<TodoItem>,
    },
    /// The end-of-work rollup (card 38).
    Summary {
        /// Headline, e.g. `"address validation tightened"`.
        title: String,
        /// Files touched, with their line counts.
        files: Vec<FileChange>,
        /// Checks that were run, e.g. tests and lint.
        checks: Vec<Check>,
        /// Total wall-clock time in milliseconds.
        duration_ms: u64,
        /// Total cost in US dollars.
        cost_usd: f64,
    },
    /// A failure the person may want to retry (card 38).
    Error {
        /// Short headline, e.g. `"Anthropic API rate limited"`.
        title: String,
        /// Detail line under the headline.
        detail: String,
        /// Whether to offer a "Retry now" button.
        retryable: bool,
    },
    /// A hairline marker row between turns (card 30).
    Marker {
        /// What happened.
        ///
        /// Serialized as `marker_kind`: the enum's own tag already owns `kind`.
        #[serde(rename = "marker_kind")]
        kind: MarkerKind,
        /// The one-line text to render.
        text: String,
    },
}

impl Block {
    /// A finished, non-streaming text block.
    pub fn text(text: impl Into<String>) -> Self {
        Block::Text { text: text.into(), streaming: false }
    }

    /// Record a decision on an [`Block::Approval`] block.
    ///
    /// [`ApprovalDecision::Once`] and [`ApprovalDecision::Always`] move a pending
    /// card to [`ApprovalState::Approving`] (the command is now running);
    /// `Always` also records `rule`. [`ApprovalDecision::Deny`] moves it to
    /// [`ApprovalState::Denied`]. Returns `false` — changing nothing — for a
    /// non-approval block or an approval that is no longer pending, so a
    /// double-press cannot re-run a command.
    pub fn decide_approval(&mut self, decision: ApprovalDecision, remembered_rule: Option<String>) -> bool {
        let Block::Approval { state, rule, .. } = self else {
            return false;
        };
        if *state != ApprovalState::Pending {
            return false;
        }
        match decision {
            ApprovalDecision::Once => *state = ApprovalState::Approving,
            ApprovalDecision::Always => {
                if remembered_rule.is_some() {
                    *rule = remembered_rule;
                }
                *state = ApprovalState::Approving;
            }
            ApprovalDecision::Deny => *state = ApprovalState::Denied,
        }
        true
    }

    /// Settle an approval that was [`ApprovalState::Approving`] once the command
    /// has exited. Returns `false` if the block was in any other state.
    pub fn complete_approval(&mut self, exit_code: i32, duration_ms: u64) -> bool {
        let Block::Approval { state, .. } = self else {
            return false;
        };
        if *state != ApprovalState::Approving {
            return false;
        }
        *state = ApprovalState::AllowedOnce { exit_code, duration_ms };
        true
    }
}

/// Whether a reasoning trace is still growing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingState {
    /// Still thinking; the label shimmers and the viewport is capped.
    Thinking,
    /// Finished; the card collapses to a summary line.
    Done,
}

/// Whether an activity group is still running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    /// At least one step is still running.
    Working,
    /// Every step finished successfully.
    Done,
    /// A step failed.
    Failed,
}

/// One row in an activity group's timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// Leading verb, e.g. `"Searched"`.
    pub verb: String,
    /// Mono target, e.g. `"src/checkout/validators.ts"`.
    pub target: String,
    /// Node state, which picks the dot styling.
    pub state: StepState,
    /// Right-aligned result, e.g. `"2 hits"` or `"+8 −3"`.
    pub result: Option<String>,
}

/// The state of one activity step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    /// Queued, not started.
    Pending,
    /// Currently running; the dot pulses.
    Running,
    /// Finished successfully.
    Done,
    /// Finished with an error.
    Failed,
}

/// How far an "always allow" decision reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalScope {
    /// Just this one invocation.
    ThisCommand,
    /// Any matching command in this worktree.
    ThisWorktree,
    /// Any matching command for the rest of the session.
    ThisSession,
    /// Any matching command everywhere, saved to the user's rules.
    Global,
}

/// Where an approval request is in its lifecycle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalState {
    /// Waiting for the person; the only visually marked state.
    Pending,
    /// Approved and running.
    Approving,
    /// Approved and finished.
    AllowedOnce {
        /// Process exit code.
        exit_code: i32,
        /// How long the command took, in milliseconds.
        duration_ms: u64,
    },
    /// The person said no; the agent will try another approach.
    Denied,
    /// A remembered rule matched, so nothing was asked.
    AutoAllowed {
        /// The rule that matched, e.g. `"pnpm test *"`.
        rule: String,
    },
}

/// One choice in a [`Block::Question`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuestionOption {
    /// Primary label, e.g. `"Canadian postal"`.
    pub label: String,
    /// Secondary line, e.g. `"A1A 1A1, space optional"`.
    pub description: String,
    /// Keyboard shortcut shown as a `kbd` on the right, e.g. `"2"`.
    pub key: String,
}

/// The person's reply to a [`Block::Question`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    /// Indices into [`Block::Question::options`], in selection order.
    pub selected: Vec<usize>,
    /// Free text typed into the "Other" row.
    pub other: Option<String>,
}

/// Whether a proposed plan has been decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanState {
    /// Awaiting a decision.
    Proposed,
    /// Accepted; the agent is running it.
    Accepted,
    /// Rejected; the agent will propose something else.
    Rejected,
    /// The person is editing the plan text.
    Editing,
}

/// One row in a [`Block::Todo`] list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TodoItem {
    /// Task text.
    pub label: String,
    /// Progress mark for the row.
    pub state: TodoState,
    /// Time spent on the task, in milliseconds; `None` when not started.
    pub elapsed_ms: Option<u64>,
}

/// The state of one todo row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoState {
    /// Not started.
    Pending,
    /// In progress; the mark pulses.
    Running,
    /// Complete; the label is struck through.
    Done,
}

/// One file row in a [`Block::Summary`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    /// Path relative to the worktree root.
    pub path: String,
    /// M / A / D letter for the row.
    pub change: ChangeKind,
    /// Lines added.
    pub added: u32,
    /// Lines removed.
    pub removed: u32,
}

/// How a file changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Existing file edited.
    Modified,
    /// New file.
    Added,
    /// File removed.
    Deleted,
}

/// One verification in a [`Block::Summary`], e.g. `"27 tests pass"`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    /// What was checked.
    pub label: String,
    /// Whether it passed.
    pub passed: bool,
}

/// What a [`Block::Marker`] row announces.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MarkerKind {
    /// The session opened.
    SessionStarted,
    /// Work moved from one provider to another with full context.
    HandOff {
        /// Provider that handed off.
        from: crate::session::Provider,
        /// Provider that took over.
        to: crate::session::Provider,
    },
    /// The context window was compacted.
    ContextCompacted,
    /// The permission mode changed mid-session.
    PermissionModeChanged {
        /// The new mode.
        mode: crate::session::PermissionMode,
    },
}
