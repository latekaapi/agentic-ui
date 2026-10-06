//! Shared helpers for the orchestration cards: the wire-provider mark
//! mapping, the plain state words used in accessible labels, and the
//! task-state dot mapping. The R2 cards (`plan_message`, `progress`,
//! `question`, `note`, `peer`) each grew their own copy; new cards use these.

use aui_protocol::{Provider as WireProvider, TaskState};
use aui_tokens::AgentState;

use crate::icons::Provider as IconProvider;

/// Maps the wire provider onto its coloured mark.
pub(crate) fn mark(provider: WireProvider) -> IconProvider {
    match provider {
        WireProvider::Claude => IconProvider::Claude,
        WireProvider::Codex => IconProvider::Codex,
        WireProvider::Grok => IconProvider::Grok,
        WireProvider::Gemini => IconProvider::Gemini,
        WireProvider::Pi => IconProvider::Pi,
        WireProvider::Cursor => IconProvider::Cursor,
        WireProvider::Muse => IconProvider::Muse,
    }
}

/// The plain state word a task row carries in its accessible label.
pub(crate) fn state_word(state: TaskState) -> &'static str {
    match state {
        TaskState::Queued => "queued",
        TaskState::Working => "working",
        TaskState::Verifying => "verifying",
        TaskState::Accepted => "accepted",
        TaskState::NeedsYou => "needs you",
        TaskState::Failed => "failed",
        TaskState::Reconciling => "reconciling",
        TaskState::Paused => "paused",
    }
}

/// The dot a task row draws: the agent state and whether it is in motion.
pub(crate) fn dot_state(state: TaskState) -> (AgentState, bool) {
    match state {
        TaskState::Working | TaskState::Verifying | TaskState::Reconciling => {
            (AgentState::Running, true)
        }
        TaskState::NeedsYou => (AgentState::Waiting, false),
        TaskState::Accepted => (AgentState::Done, false),
        TaskState::Failed => (AgentState::Failed, false),
        TaskState::Queued | TaskState::Paused => (AgentState::Idle, false),
    }
}
