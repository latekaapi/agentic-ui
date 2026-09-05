//! Transcript: markers, user and assistant turns with streaming reveal,
//! thinking block, activity group, tool cards and bodies, approval,
//! question / plan / todo, code and diff blocks, summary / error / status
//! rows, citations and sources (cards 30–38, 55).

mod card;
mod marker;

pub use card::{transcript_card, TranscriptCard};
pub use marker::{marker_row, HandOff, MarkerRow};
