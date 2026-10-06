//! Orchestration transcript components: the conductor run's plan message,
//! progress block, run question, run note and peer message.
//!
//! Every component renders a protocol value from
//! [`aui_protocol::run`](aui_protocol::run) and emits the person's intent
//! through a small per-card enum; no component owns orchestration logic.

pub mod note;
pub mod peer;
pub mod plan_message;
pub mod progress;
pub mod question;

pub use note::{run_note, RunNoteLine};
pub use peer::{peer_message, PeerIntent, PeerMessageCard};
pub use plan_message::{plan_message, PlanMessageCard};
pub use progress::{progress_block, ProgressBlock, ProgressIntent};
pub use question::{run_question, RunQuestionCard, RunQuestionIntent};
