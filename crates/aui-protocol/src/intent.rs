//! Intents: what the UI asks the app to do.
//!
//! The component library never mutates a [`crate::Session`] itself and never
//! talks to an agent. Every interactive affordance emits an [`Intent`]; the app
//! decides what it means and feeds the result back as [`crate::Delta`]s.

use serde::{Deserialize, Serialize};

use crate::block::Answer;
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
}

/// What the person chose on an approval card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Allow just this invocation.
    Once,
    /// Allow it and remember the rule for the card's scope.
    Always,
    /// Refuse; the agent must try another approach.
    Deny,
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
