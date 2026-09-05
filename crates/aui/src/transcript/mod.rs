//! Transcript: markers, user and assistant turns with streaming reveal,
//! thinking block, activity group, tool cards and bodies, approval,
//! question / plan / todo, code and diff blocks, summary / error / status
//! rows, citations and sources (cards 30–38, 55).

mod card;
mod marker;
mod prose;
mod turns;

pub use card::{transcript_card, TranscriptCard};
pub use prose::{prose, ProseStyle};
pub use marker::{marker_row, HandOff, MarkerRow};
pub use turns::{assistant_turn, user_turn, AssistantTurn, AssistantTurnAction, UserTurn, UserTurnAction};
