//! Intents: what the UI asks the app to do.
//!
//! The component library never mutates a [`crate::Session`] itself and never
//! talks to an agent. Every interactive affordance emits an [`Intent`]; the app
//! decides what it means and feeds the result back as [`crate::Delta`]s.

use serde::{Deserialize, Serialize};

use crate::block::Answer;
use crate::session::{PermissionMode, ReasoningEffort};
use crate::turn::{Attachment, Mention};

/// Something the person did that the app must act on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    /// Send the composer's contents now.
    Send {
        /// Message text.
        text: String,
        /// Files and images to send with it.
        attachments: Vec<Attachment>,
        /// Mentions parsed from the text.
        mentions: Vec<Mention>,
    },
    /// Queue the composer's contents to send when the current turn ends.
    Queue {
        /// Message text.
        text: String,
        /// Files and images to send with it.
        attachments: Vec<Attachment>,
        /// Mentions parsed from the text.
        mentions: Vec<Mention>,
    },
    /// Interrupt the running turn.
    Stop,
    /// Resolve an approval card.
    Approve {
        /// Id of the [`crate::Block::Approval`].
        id: String,
        /// What the person chose.
        decision: ApprovalDecision,
    },
    /// Answer a question card.
    Answer {
        /// Id of the [`crate::Block::Question`].
        id: String,
        /// The selection and any free text.
        answer: Answer,
    },
    /// Accept a proposed plan and start running it.
    AcceptPlan {
        /// Id of the [`crate::Block::Plan`].
        id: String,
    },
    /// Reject a proposed plan.
    RejectPlan {
        /// Id of the [`crate::Block::Plan`].
        id: String,
    },
    /// Open a proposed plan for editing.
    EditPlan {
        /// Id of the [`crate::Block::Plan`].
        id: String,
    },
    /// Open a file in the workbench editor.
    OpenFile {
        /// Path relative to the worktree root.
        path: String,
        /// Line to reveal, 1-based.
        line: Option<u32>,
    },
    /// Open a file's diff in the review pane.
    OpenDiff {
        /// Path relative to the worktree root.
        path: String,
    },
    /// Send review notes (from the diff gutter or the browser annotator) to the
    /// agent as a new message.
    SendNotes {
        /// The notes, in the order they were written.
        notes: Vec<Note>,
    },
    /// Switch the workbench to another pane.
    ChangeView {
        /// The pane to show.
        view: WorkbenchView,
    },
    /// Show or hide the workbench pane entirely.
    ToggleRightPane,
    /// Interject into the turn that is already running instead of queueing
    /// behind it (MSP `turn/steer`).
    Steer {
        /// Message text.
        text: String,
        /// Files and images to send with it.
        attachments: Vec<Attachment>,
        /// Mentions parsed from the text.
        mentions: Vec<Mention>,
    },
    /// Reclaim a queued submission that has not launched (MSP `turn/unqueue`).
    Unqueue {
        /// Id of the queued turn, exactly as the queueing ack minted it.
        turn_id: String,
    },
    /// Unqueue a queued submission and restore its text to the composer.
    EditQueued {
        /// Id of the queued turn.
        turn_id: String,
    },
    /// Compact the context window now (MSP `session/compact`).
    Compact,
    /// Switch the session's model (MSP `session/setModel`).
    SetModel {
        /// Model id from the provider catalog, e.g. `"muse-spark-1.3"`.
        model_id: String,
    },
    /// Change how much reasoning the next turn should spend.
    SetEffort {
        /// The tier the person picked.
        effort: ReasoningEffort,
    },
    /// Change the approval mode (MSP `session/setApprovalMode`).
    SetMode {
        /// The mode the person picked.
        mode: PermissionMode,
    },
    /// Fork the session, optionally from a specific turn.
    Fork {
        /// Turn to fork from; `None` forks from the tip.
        turn_id: Option<String>,
    },
    /// Answer a question card with free text instead of a choice, so the model
    /// re-decides (MSP `userInput/clarify`).
    Clarify {
        /// Id of the [`crate::Block::Question`].
        id: String,
        /// The clarification text.
        text: String,
    },
    /// Dismiss a question card without answering (MSP `userInput/cancel`).
    CancelQuestion {
        /// Id of the [`crate::Block::Question`].
        id: String,
    },
    /// Start the provider's sign-in flow.
    Login,
    /// Sign out of the provider.
    Logout,
}

/// What the person chose on an approval card.
///
/// The full MSP `ApprovalDecision` set (`msp.d.ts:62`), keeping the three
/// original names so existing call sites still compile. MSP wire value → variant:
///
/// | MSP | variant |
/// |---|---|
/// | `approved` | [`ApprovalDecision::Once`] |
/// | `approvedForSession` | [`ApprovalDecision::ApprovedForSession`] |
/// | `approvedPolicyAmendment` | [`ApprovalDecision::PolicyAmendment`], or [`ApprovalDecision::Always`] where the UI still speaks the old triad |
/// | `denied` | [`ApprovalDecision::Deny`] |
/// | `deniedPolicyAmendment` | [`ApprovalDecision::DeniedPolicyAmendment`] |
/// | `timedOut` | [`ApprovalDecision::TimedOut`] |
/// | `abort` | [`ApprovalDecision::Abort`] |
///
/// Serialized `snake_case`, so this enum's own wire names are *not* the MSP
/// ones; an adapter maps them at the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Allow just this invocation (MSP `approved`).
    Once,
    /// Allow it and remember the rule for the card's scope (MSP
    /// `approvedPolicyAmendment`).
    Always,
    /// Refuse; the agent must try another approach (MSP `denied`).
    Deny,
    /// Allow it for the rest of the session without writing a durable rule
    /// (MSP `approvedForSession`).
    ApprovedForSession,
    /// Allow it and write the amendment into the policy (MSP
    /// `approvedPolicyAmendment`).
    PolicyAmendment,
    /// Refuse it and write the refusal into the policy (MSP
    /// `deniedPolicyAmendment`).
    DeniedPolicyAmendment,
    /// Nobody answered in time (MSP `timedOut`).
    TimedOut,
    /// Refuse and stop the turn (MSP `abort`).
    Abort,
}

/// One review note anchored to a file and line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// Path relative to the worktree root, or the URL for a browser note.
    pub path: String,
    /// Line the note is anchored to, 1-based; `None` for a whole-file note.
    pub line: Option<u32>,
    /// The note text.
    pub text: String,
}

/// The panes the workbench can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchView {
    /// Diff review.
    Diff,
    /// File tree and editor.
    Files,
    /// Terminal.
    Terminal,
    /// Browser and annotator.
    Browser,
    /// Git and pull requests.
    Git,
    /// Documents and citations.
    Docs,
}
