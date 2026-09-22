//! `aui-protocol` — the transport-agnostic session model that the Agentic UI
//! transcript renders.
//!
//! This crate is pure data. It performs no I/O, spawns no tasks and does not
//! depend on `gpui`, so it can be shared by the UI, by CLI adapters (Claude Code
//! `stream-json`, ACP, …) and by tests. The shapes here are the ones described in
//! `docs/02-component-spec.md` section 7, "Data model the transcript renders".
//!
//! # Layout
//!
//! - [`Session`] is the root: identity ([`Provider`], model, [`PermissionMode`],
//!   the client-side [`Session::plan`] overlay, cwd, branch, [`Environment`])
//!   plus an ordered list of [`Turn`]s.
//! - A [`Turn`] is either the person's message or an assistant reply made of
//!   [`Block`]s. Every card in the transcript is one block variant.
//! - Live updates arrive as [`Delta`]s and are folded into a session with
//!   [`Session::apply`]. A delta whose target is gone is ignored, and `apply`
//!   reports whether anything changed.
//! - The UI never mutates the app's state directly; it emits [`Intent`]s,
//!   including the session controls [`Intent::SetModel`], [`Intent::SetEffort`]
//!   ([`ReasoningEffort`]) and [`Intent::SetMode`].
//! - [`sample`] builds the realistic session used by the gallery and the design
//!   reference screens.
//!
//! # Serialization
//!
//! Every type derives `Serialize`/`Deserialize`. Enums that carry data are
//! internally tagged with `kind` and `snake_case` names, so a text block is
//! `{"kind":"text","text":"…","streaming":false}`. Enums whose variants are all
//! unit variants (statuses, states, modes) serialize as plain `snake_case`
//! strings — with two deliberate exceptions, [`PermissionMode`] and
//! [`ReasoningEffort`], which serialize `camelCase` because those strings *are*
//! the MSP wire values.
//!
//! # Muse (MSP) shapes
//!
//! Fields added for Meta's Muse Code all carry `#[serde(default)]`, so a
//! session serialized before they existed still loads:
//!
//! - [`Block::Approval`] carries the server-minted [`ApprovalChoice`] list, the
//!   [`ApprovalStage`] pipeline with its current stage, [`ApprovalBadges`],
//!   feedback text and [`ResolvedBy`]; [`ApprovalState::AutoDenied`] is the
//!   never-actionable policy refusal.
//! - [`Block::Question`] gains a header, a countdown and per-option
//!   [`QuestionPreview`]s.
//! - [`Block::Goal`] mirrors `session/goalChanged`, and [`Block::Generic`] is
//!   the mandated fallback card for an item kind this client does not model.
//! - [`Delta::ThinkingDelta`], [`Delta::ToolOutputDelta`],
//!   [`Delta::TurnRemoved`] and [`Delta::BlockRemoved`] cover streamed
//!   reasoning, streamed tool output, and retraction/unqueue.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod block;
mod delta;
mod intent;
mod session;
mod tool;
mod turn;

pub mod sample;

pub use block::{
    ActivityState, Answer, ApprovalBadges, ApprovalChoice, ApprovalScope, ApprovalStage,
    ApprovalState, Block, Check, ChangeKind, FileChange, MarkerKind, PlanSection, PlanState,
    QuestionOption, QuestionPreview, ResolvedBy, Step, StepState, ThinkingState, TodoItem,
    TodoState,
};
pub use delta::Delta;
pub use intent::{ApprovalDecision, Intent, Note, WorkbenchView};
pub use session::{Environment, PermissionMode, Provider, ReasoningEffort, Session};
pub use tool::{
    AnsiLine, Diff, DiffKind, DiffLine, DiffStat, Hunk, SearchHit, ToolBody, ToolCall, ToolKind,
    ToolStatus, WebResult,
};
pub use turn::{Attachment, AttachmentKind, Mention, MentionKind, Turn, TurnMeta, UploadState};
