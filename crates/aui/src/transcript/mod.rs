//! Transcript: markers, user and assistant turns with streaming reveal,
//! thinking block, activity group, tool cards and bodies, approval,
//! question / plan / todo, code and diff blocks, summary / error / status
//! rows, citations and sources (cards 30–38, 55).

mod activity;
mod approval;
mod card;
mod code;
mod marker;
mod plan;
mod question;
mod status;
mod summary;
mod thinking;
mod todo;
mod tool_card;
mod prose;
mod turns;

pub use activity::{activity_group, ActivityGroup};
pub use card::{transcript_card, TranscriptCard};
pub use thinking::{thinking_block, ThinkingBlock};
pub use prose::{prose, ProseStyle};
pub use marker::{marker_row, HandOff, MarkerRow};
pub use turns::{assistant_turn, user_turn, AssistantTurn, AssistantTurnAction, UserTurn, UserTurnAction};
// card 34 exports
// card 35 exports
// card 36 exports
// card 37 exports
// card 38 exports
