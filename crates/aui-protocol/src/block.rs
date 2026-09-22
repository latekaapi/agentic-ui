//! Blocks: one variant per card in the transcript.

use serde::{Deserialize, Serialize};

use crate::intent::ApprovalDecision;
use crate::tool::{DiffStat, ToolBody, ToolCall, ToolKind, ToolStatus};

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
        /// Server-authored diff summary for edit-family calls: the counts the
        /// provider computed over the whole patch, available without fetching
        /// the patch body. `None` for other tools and for payloads written
        /// before the summary existed; the card then draws no chips.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        diff_stat: Option<DiffStat>,
    },
    /// A run of consecutive tool calls folded into one card.
    ///
    /// Each call keeps its [`ToolCall`] shape, so the open group renders every
    /// call as the full card the lone [`Block::ToolCall`] would have shown.
    ToolGroup {
        /// The calls, in execution order.
        calls: Vec<ToolCall>,
        /// Collapsed header line, e.g. `"Searched web · 5 results"`.
        summary: String,
        /// Whether the group is still running, which picks the header glyph.
        state: ActivityState,
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
        /// Server-minted choices (MSP `availableChoices`).
        ///
        /// The UI renders what it is given, not a fixed allow/always/deny
        /// triad. Empty means the card falls back to the built-in triad.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        choices: Vec<ApprovalChoice>,
        /// The pipeline's stages; a shell pipeline is decided one stage at a
        /// time. Empty for a single-stage subject.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stages: Vec<ApprovalStage>,
        /// Index into `stages` of the stage awaiting a decision; `None` when
        /// nothing is pending or the subject is not staged.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current_stage: Option<usize>,
        /// Badges the header shows beside the title.
        #[serde(default)]
        badges: ApprovalBadges,
        /// Free text typed into a choice whose
        /// [`ApprovalChoice::accepts_feedback`] is set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feedback: Option<String>,
        /// Who settled the request, once it is settled.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resolved_by: Option<ResolvedBy>,
    },
    /// A structured question with options (card 36).
    Question {
        /// Stable id, echoed back in [`crate::Intent::Answer`].
        id: String,
        /// Short label rendered *above* the prompt (MSP
        /// `UserInputQuestion.header`, e.g. `"File"`).
        #[serde(default)]
        header: String,
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
        /// Auto-resolution deadline in milliseconds (MSP `autoResolutionMs`),
        /// which the card shows as a countdown. `None` means no timeout.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// A proposed plan awaiting acceptance (card 36).
    Plan {
        /// Stable id, echoed back in the plan intents.
        id: String,
        /// Plan steps, in order.
        items: Vec<String>,
        /// Unnumbered section labels drawn between the steps.
        ///
        /// A plan written as markdown headings over lists has two levels; the
        /// numbering counts steps only, so a section is a label and the index
        /// of the first item that falls under it.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sections: Vec<PlanSection>,
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
    /// The session's current objective, from MSP `session/goalChanged`.
    ///
    /// Replaced wholesale by every event; an explicit clear removes the block.
    Goal {
        /// What the agent is trying to achieve.
        objective: String,
        /// Free-form status string, verbatim from the provider.
        status: String,
        /// Percentage complete, **verbatim from the provider**: values above
        /// 100 pass through unchanged and clamping for display is the
        /// renderer's job.
        percent_complete: Option<f32>,
        /// What the agent is working on right now.
        current_work: Option<String>,
        /// What it intends to do next.
        next_work: Option<String>,
    },
    /// The mandated fallback card for an item kind this client does not know.
    ///
    /// MSP's `ItemKind` is an **open** enum and clients MUST render an unknown
    /// kind generically: the kind name, the item's `status`, and the server's
    /// `fallbackText`. Never guess a richer card for a kind you do not model.
    Generic {
        /// The wire kind name, e.g. `"reminderChild"`.
        ///
        /// Serialized as `item_kind`: the enum's own tag already owns `kind`.
        #[serde(rename = "item_kind")]
        kind: String,
        /// The item's `status`, e.g. `"inProgress"` or `"completed"`.
        status: String,
        /// The server's `fallbackText` one-liner.
        text: String,
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
    /// A [`Block::ToolCall`] built from its struct shape.
    pub fn tool_call(call: ToolCall) -> Self {
        Block::ToolCall {
            id: call.id,
            kind: call.kind,
            verb: call.verb,
            target: call.target,
            status: call.status,
            duration_ms: call.duration_ms,
            body: call.body,
            diff_stat: call.diff_stat,
        }
    }

    /// The [`ToolCall`] shape of a [`Block::ToolCall`]; `None` for any other
    /// variant, so a grouping pass can collect runs of tool calls without
    /// matching the variant itself.
    pub fn as_tool_call(&self) -> Option<ToolCall> {
        match self {
            Block::ToolCall { id, kind, verb, target, status, duration_ms, body, diff_stat } => Some(ToolCall {
                id: id.clone(),
                kind: kind.clone(),
                verb: verb.clone(),
                target: target.clone(),
                status: *status,
                duration_ms: *duration_ms,
                body: body.clone(),
                diff_stat: *diff_stat,
            }),
            _ => None,
        }
    }

    /// A finished, non-streaming text block.
    pub fn text(text: impl Into<String>) -> Self {
        Block::Text { text: text.into(), streaming: false }
    }

    /// A pending [`Block::Approval`] with the MSP-only fields left empty.
    ///
    /// The shape existing renderers already understand: identity, subject,
    /// scope and rule. Set [`Block::Approval::choices`],
    /// [`Block::Approval::stages`] and the rest afterwards when the provider
    /// supplies them.
    #[allow(clippy::too_many_arguments)]
    pub fn approval(
        id: impl Into<String>,
        tool: impl Into<String>,
        command: impl Into<String>,
        reason: impl Into<String>,
        cwd: impl Into<String>,
        capabilities: Vec<String>,
        scope: ApprovalScope,
        state: ApprovalState,
        rule: Option<String>,
    ) -> Self {
        Block::Approval {
            id: id.into(),
            tool: tool.into(),
            command: command.into(),
            reason: reason.into(),
            cwd: cwd.into(),
            capabilities,
            scope,
            state,
            rule,
            choices: Vec::new(),
            stages: Vec::new(),
            current_stage: None,
            badges: ApprovalBadges::default(),
            feedback: None,
            resolved_by: None,
        }
    }

    /// Record a decision on an [`Block::Approval`] block.
    ///
    /// Every approving decision ([`ApprovalDecision::Once`],
    /// [`ApprovalDecision::Always`], [`ApprovalDecision::ApprovedForSession`],
    /// [`ApprovalDecision::PolicyAmendment`]) moves a pending card to
    /// [`ApprovalState::Approving`] (the command is now running); the two that
    /// remember something — `Always` and `PolicyAmendment` — also record
    /// `rule`. Every refusing decision ([`ApprovalDecision::Deny`],
    /// [`ApprovalDecision::DeniedPolicyAmendment`],
    /// [`ApprovalDecision::TimedOut`], [`ApprovalDecision::Abort`]) moves it to
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
            ApprovalDecision::Once | ApprovalDecision::ApprovedForSession => {
                *state = ApprovalState::Approving;
            }
            ApprovalDecision::Always | ApprovalDecision::PolicyAmendment => {
                if remembered_rule.is_some() {
                    *rule = remembered_rule;
                }
                *state = ApprovalState::Approving;
            }
            ApprovalDecision::Deny
            | ApprovalDecision::DeniedPolicyAmendment
            | ApprovalDecision::TimedOut
            | ApprovalDecision::Abort => *state = ApprovalState::Denied,
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
    /// Policy or the approval judge refused it, so the card was never
    /// actionable: it opened and resolved in the same breath.
    AutoDenied {
        /// The rule that refused it, e.g. `"deny_unmatched"`.
        rule: String,
    },
}

/// One server-minted choice on an approval card (MSP `ApprovalChoice`).
///
/// The UI renders the choices it is given, in order, and sends the chosen
/// [`ApprovalChoice::id`] back — it never invents a choice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApprovalChoice {
    /// Wire id, echoed back as MSP `choiceId`, e.g. `"allow_local_prefix"`.
    pub id: String,
    /// Button label, e.g. `"Always allow in this workspace: echo ..."`.
    pub label: String,
    /// What choosing it means.
    pub decision: ApprovalDecision,
    /// How far the choice reaches.
    pub scope: ApprovalScope,
    /// The rule this choice would write, shown under the button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_preview: Option<String>,
    /// Whether picking this choice reveals a free-text field whose contents go
    /// to the model as [`Block::Approval::feedback`].
    #[serde(default)]
    pub accepts_feedback: bool,
}

/// One stage of a staged approval subject (MSP `ApprovalStage`).
///
/// A shell pipeline is decided one stage at a time: `echo hi && ls` is two
/// stages, each with its own choices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApprovalStage {
    /// 1-based position of this stage, the `n` in `n/N`.
    pub position: u32,
    /// How many stages the subject has, the `N` in `n/N`.
    pub total: u32,
    /// The stage's argv, already split by the server.
    pub argv: Vec<String>,
    /// `false` when the server could not fully parse the stage, in which case
    /// `argv` is a best effort and must be shown as such.
    pub argv_complete: bool,
    /// Whether this stage already has a decision.
    pub resolved: bool,
    /// The rule the server suggests for this stage, e.g. `"echo ..."`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_rule: Option<String>,
}

/// The badges an approval card shows beside its title.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalBadges {
    /// The action would write to a protected path (MSP `protectedWrite`).
    #[serde(default)]
    pub protected_write: bool,
    /// The LLM approval judge escalated the request to a human (MSP
    /// `judgeEscalated`).
    #[serde(default)]
    pub judge_escalated: bool,
}

/// Who settled an approval request (MSP `ApprovalResolved.resolvedBy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedBy {
    /// The person pressed a choice.
    User,
    /// A policy rule matched, with no user interaction at all.
    Policy,
    /// The LLM approval judge decided it.
    LlmJudge,
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
    /// A rendered preview the card can expand, e.g. a diff or a snippet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<QuestionPreview>,
}

/// A rendered preview attached to a [`QuestionOption`] (MSP
/// `UserInputOption.preview`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuestionPreview {
    /// The preview body.
    pub content: String,
    /// How to render `content`, e.g. `"text"`, `"markdown"` or `"diff"`.
    pub format: String,
}

/// The person's reply to a [`Block::Question`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    /// Indices into [`Block::Question::options`], in selection order.
    pub selected: Vec<usize>,
    /// Free text typed into the "Other" row.
    pub other: Option<String>,
}

/// One unnumbered section label inside a [`Block::Plan`].
///
/// Plans arrive as markdown: headings group the steps, and the steps are the
/// numbered list items under them. `first_item` is the index into
/// [`Block::Plan::items`] of the first step the label covers, so a section with
/// no steps under it can simply be left out.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanSection {
    /// The heading text, e.g. `"Read the code"`.
    pub label: String,
    /// Index into [`Block::Plan::items`] of the first step under the label.
    pub first_item: usize,
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
    /// A turn was interrupted before it finished.
    TurnCancelled,
    /// A turn was retracted before any output committed, so its prompt went
    /// back to the composer.
    TurnRetracted,
    /// The provider is retrying after a failure ("attempt 2/5 · retrying in 4s").
    RetryScheduled,
    /// The client's view of the transcript has a hole in it, because it
    /// reconnected past the server's retained window.
    ViewGap,
    /// This session was forked from another one.
    ForkedFrom,
}
