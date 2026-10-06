//! Orchestration view models: the provider-neutral shapes Baaz's conductor
//! produces and the `aui` orchestration components render.
//!
//! These are plain data: a durable conductor will produce them later, and the
//! `aui` components (a later task) will render them. Field names follow the
//! design doc (§4.2 and §4.6) so the conductor can produce them unchanged.
//!
//! Like the rest of the crate, payload structs keep their `snake_case` field
//! names verbatim (no `rename_all`), and unit-only enums serialize as
//! `snake_case` strings. Two fields the spec calls `kind` are named
//! `note_kind` / `evidence_kind` instead: the [`Block`](crate::Block) enum is
//! internally tagged on `kind`, so a payload field literally named `kind`
//! would collide with the tag.

use serde::{Deserialize, Serialize};

use crate::session::Provider;

/// How a [`Criterion`] is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckMethod {
    /// An automated test run.
    Test,
    /// A browser walkthrough.
    Browser,
    /// A reviewer (human or model) read the diff.
    Review,
    /// The person confirms it by hand.
    User,
}

/// Where one conductor-run task is in its lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Waiting for a worker slot.
    Queued,
    /// A worker is editing.
    Working,
    /// Edits are done; the checks are running.
    Verifying,
    /// Done and accepted into the run.
    Accepted,
    /// Blocked on the person (usually a [`RunQuestion`]).
    NeedsYou,
    /// Failed and waiting on a retry decision.
    Failed,
    /// The conductor is re-deriving state after a worker disagreed.
    Reconciling,
    /// Paused by the person; nothing is running.
    Paused,
}

/// Where one verification rung of a [`TaskLine`] stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RungState {
    /// Not started yet.
    Pending,
    /// Currently running.
    Running,
    /// Passed.
    Pass,
    /// Failed.
    Fail,
    /// Ran before the current edits; the result no longer proves anything.
    Unverified,
}

/// One verification step of a task: scope, build, tests, visual, review,
/// mutation, or a held-out check the implementers cannot see.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rung {
    /// Which rung this is, e.g. `"scope"`, `"build"`, `"tests"`, `"visual"`,
    /// `"review"`, `"mutation"`, `"held-out"`.
    pub name: String,
    /// Where the rung stands.
    pub state: RungState,
    /// Supporting detail, e.g. `"12 passed"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// One stage of a conductor run: which provider runs it and whether it writes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunStage {
    /// Stage name, e.g. `"Implement · Muse"`.
    pub name: String,
    /// Which provider runs this stage. `None` means the Baaz conductor itself
    /// (the verify/evaluator stage): Baaz is not a [`Provider`], so there is
    /// no provider value that could mean it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
    /// Model label, e.g. `"opus 4.6"`.
    pub model: String,
    /// Whether this stage edits the worktree.
    pub writes: bool,
}

/// One acceptance criterion of a [`RunPlan`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Criterion {
    /// What must hold, e.g. `"tests added"`.
    pub text: String,
    /// How it will be checked.
    pub method: CheckMethod,
}

/// The plan card: what the run will do, staged, before it starts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunPlan {
    /// Stable id of the run.
    pub run_id: String,
    /// Run title, e.g. `"Login screen"`.
    pub title: String,
    /// The conductor's own words introducing the plan.
    pub intro: String,
    /// The stages, in execution order.
    pub stages: Vec<RunStage>,
    /// What counts as done.
    pub criteria: Vec<Criterion>,
    /// Premises the plan rests on, e.g. `"from main at 3db2220"`.
    pub assumptions: Vec<String>,
    /// When the run started. `None` means proposed: the footer reads
    /// `Reply "go" to start, or tell me what to change.`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
}

/// One task row of a [`RunProgress`] card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskLine {
    /// Stable id, used to open the worker view.
    pub task_id: String,
    /// Which stage owns this task, e.g. `"Implement"`.
    pub stage: String,
    /// Row title, e.g. `"Implement · auth call"`.
    pub title: String,
    /// Which provider works this task.
    pub provider: Provider,
    /// Model label, when the worker reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Where the task stands.
    pub state: TaskState,
    /// What the worker is doing right now, e.g. `"editing src/auth/login.ts"`.
    pub activity: String,
    /// How long the task has been running, e.g. `"4m"`.
    pub elapsed: String,
    /// The verification rungs and their states.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rungs: Vec<Rung>,
}

/// The progress card: header counts plus one row per task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunProgress {
    /// Stable id of the run.
    pub run_id: String,
    /// Run title, e.g. `"Login screen"`.
    pub title: String,
    /// Index of the current stage (0-based).
    pub stage_index: u32,
    /// How many stages the run has.
    pub stage_count: u32,
    /// Name of the current stage.
    pub stage_name: String,
    /// How many tasks are working right now.
    pub working: u32,
    /// How many tasks are verifying right now.
    pub verifying: u32,
    /// How many tasks need the person right now.
    pub needs_you: u32,
    /// How many tasks are accepted so far.
    pub accepted: u32,
    /// How long the run has been going, e.g. `"31m"`.
    pub elapsed: String,
    /// Tokens spent, when measured, e.g. `"48k"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    /// Whether the person paused the run.
    pub paused: bool,
    /// Why the run is waiting, e.g. a quota note. `None` while moving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_reason: Option<String>,
    /// The task rows.
    pub lines: Vec<TaskLine>,
}

/// One offered answer of a [`RunQuestion`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuestionOption {
    /// Stable id, echoed back in the answer.
    pub id: String,
    /// The option label, e.g. `"Leave it out"`.
    pub label: String,
    /// Whether the conductor recommends this option.
    pub recommended: bool,
}

/// A question the run asks the person, pausing its worker until answered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunQuestion {
    /// Stable id of the question.
    pub question_id: String,
    /// Stable id of the run that asks.
    pub run_id: String,
    /// Where the question comes from, e.g.
    /// `"QUESTION FROM THE RUN · IMPLEMENT · AUTH CALL"`.
    pub source_label: String,
    /// The question itself.
    pub text: String,
    /// Supporting line under the question, e.g. when the worker paused.
    pub hint: String,
    /// The offered answers, in display order.
    pub options: Vec<QuestionOption>,
    /// The chosen answer. `Some` means answered and collapsed: the card reads
    /// `"You: <answer> · 14:31"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    /// When the answer was given. `None` while open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<String>,
}

/// One piece of proof attached to a [`CriterionResult`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// The evidence line, e.g. `"npm test exited 0 · 12 passed"`.
    pub label: String,
    /// What kind of proof this is. Named `evidence_kind`, not `kind`: the
    /// [`Block`](crate::Block) tag already owns `kind`.
    pub evidence_kind: EvidenceKind,
}

/// What kind of proof an [`Evidence`] row carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A test run.
    Test,
    /// A screenshot.
    Screenshot,
    /// A review pass.
    Review,
    /// A held-out check the implementers never saw.
    HeldOut,
    /// A bare count, e.g. `"+4 tests"`.
    Count,
    /// The person confirmed it.
    User,
}

/// Whether one acceptance criterion held, and the proof.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CriterionResult {
    /// The criterion text, matching the plan's [`Criterion::text`].
    pub text: String,
    /// Whether the criterion held.
    pub met: bool,
    /// The proof.
    pub evidence: Vec<Evidence>,
}

/// How a conductor run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    /// Ran to the end.
    Finished,
    /// The person stopped it early.
    Stopped,
    /// It failed.
    Failed,
}

/// The end-of-run rollup: outcome, proof per criterion, diff stats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Stable id of the run.
    pub run_id: String,
    /// Run title, e.g. `"Login screen"`.
    pub title: String,
    /// How the run ended.
    pub outcome: RunOutcome,
    /// How long the run took, e.g. `"31m"`.
    pub elapsed: String,
    /// How many workers took part.
    pub workers: u32,
    /// One result per acceptance criterion.
    pub criteria: Vec<CriterionResult>,
    /// Lines added.
    pub additions: u32,
    /// Lines removed.
    pub deletions: u32,
    /// Files touched.
    pub files: u32,
    /// How many review rounds ran.
    pub review_rounds: u32,
    /// Extra proof that fits no criterion, e.g. a mutation-check note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
    /// What the run could not prove, e.g. a flow with no test account.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_proven: Vec<String>,
    /// What the person can say next, e.g. which branch to merge.
    pub next_hint: String,
    /// The merge commit, once merged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged: Option<String>,
}

/// A one-line note the run files, e.g. a remembered preference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunNote {
    /// The note text.
    pub text: String,
    /// What kind of note this is. Named `note_kind`, not `kind`: the
    /// [`Block`](crate::Block) tag already owns `kind`.
    pub note_kind: NoteKind,
}

/// What kind of note a [`RunNote`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    /// A preference the conductor remembered.
    Preference,
    /// A plain informational note.
    Info,
}

/// Where a session goal stands. Rendered from the extended
/// [`Block::Goal`](crate::Block::Goal), not from a new `GoalLine` type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    /// Work is moving.
    Active,
    /// The person paused it.
    Paused,
    /// Blocked on something outside the session.
    Blocked,
    /// Paused by a usage limit.
    UsageLimited,
    /// Paused by a budget limit.
    BudgetLimited,
    /// Done.
    Complete,
    /// The provider said done, the checks disagreed, and it is moving again.
    Reopened,
}

/// One child of a provider-native workflow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowChild {
    /// Stable id of the child.
    pub child_id: String,
    /// Which attempt this is.
    pub attempt: u32,
    /// Row label, e.g. `"Implement · form and validation"`.
    pub label: String,
    /// The provider's own phase name, when it reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// Where the child stands.
    pub status: TaskState,
}

/// A provider-native subagent tree (Codex threads, Claude teams, …) the
/// conductor watches but does not own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeWorkflow {
    /// Which provider owns the tree.
    pub provider: Provider,
    /// Workflow name.
    pub name: String,
    /// The children.
    pub children: Vec<WorkflowChild>,
    /// Whether a failed child can be retried.
    pub can_retry: bool,
    /// Whether a failed child can be skipped.
    pub can_skip: bool,
    /// Whether the tree is provisional (the provider may still reshape it).
    pub provisional: bool,
}

/// Which native controls a provider offers for its subagents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubagentControls {
    /// The provider can take a plain message for the subagent.
    pub message: bool,
    /// The provider supports following up on a finished subagent.
    pub follow_up: bool,
    /// The provider can interrupt a running subagent.
    pub interrupt: bool,
    /// The provider can stop a running subagent.
    pub stop: bool,
}

/// Which way a [`PeerMessage`] travels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerDirection {
    /// To a peer session.
    Outgoing,
    /// From a peer session.
    Incoming,
}

/// Where a [`PeerMessage`] is in its delivery lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerState {
    /// Left this session; not yet queued downstream.
    Sent,
    /// Queued for a peer that is not listening yet.
    Queued,
    /// Arrived at the peer session.
    Delivered,
    /// The peer is working on it.
    Working,
    /// The peer replied.
    Replied,
    /// It can never arrive (the peer session is gone).
    Unreachable,
}

/// A message to or from a peer session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PeerMessage {
    /// Stable id of the message.
    pub message_id: String,
    /// Which way the message travels.
    pub direction: PeerDirection,
    /// Peer title, e.g. `"Browser audit"`.
    pub peer_title: String,
    /// Which provider runs the peer.
    pub peer_provider: Provider,
    /// The peer's project, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_project: Option<String>,
    /// The message text.
    pub text: String,
    /// Where the message stands.
    pub state: PeerState,
    /// Why the message cannot arrive. Set for [`PeerState::Unreachable`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// When the message was sent, e.g. `"14:20"`.
    pub at: String,
    /// The peer's reply, once there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<PeerReply>,
}

/// A peer's reply to a [`PeerMessage`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PeerReply {
    /// The reply text.
    pub text: String,
    /// When the reply arrived, e.g. `"14:26"`.
    pub at: String,
}

/// The header of a worker view: which run, stage and attempt this worker is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerHeader {
    /// The run's title, e.g. `"Login screen"`.
    pub run_title: String,
    /// The stage the worker runs, e.g. `"Implement · auth call"`.
    pub stage: String,
    /// Which attempt this worker is.
    pub attempt: u32,
    /// Which provider runs the worker.
    pub provider: Provider,
    /// Model label.
    pub model: String,
    /// The worker's branch, e.g. `"baaz/login-screen"`.
    pub branch: String,
}

/// The run tree view: the task tree plus the evidence table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunViewModel {
    /// Stable id of the run.
    pub run_id: String,
    /// Run title, e.g. `"Login screen"`.
    pub title: String,
    /// The task tree, in display order.
    pub tree: Vec<RunViewNode>,
    /// The evidence table.
    pub evidence: Vec<EvidenceRow>,
}

/// One node of a [`RunViewModel`] task tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunViewNode {
    /// Stable id of the task.
    pub task_id: String,
    /// Nesting depth; 0 is a root.
    pub depth: u32,
    /// Node label, e.g. `"Implement · auth call"`.
    pub label: String,
    /// Which provider works this node.
    pub provider: Provider,
    /// Where the node stands.
    pub state: TaskState,
    /// A note under the label, e.g. what the worker is doing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One row of a [`RunViewModel`] evidence table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRow {
    /// Stable id of the task the evidence proves.
    pub task_id: String,
    /// The evidence line.
    pub text: String,
    /// What kind of proof this is. Named `evidence_kind`, not `kind`: the
    /// [`Block`](crate::Block) tag already owns `kind`.
    pub evidence_kind: EvidenceKind,
}
