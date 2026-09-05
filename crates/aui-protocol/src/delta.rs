//! Streaming updates and how they fold into a [`Session`].

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::session::Session;
use crate::turn::{Turn, TurnMeta};

/// One incremental change to a session, as an adapter emits it while the agent
/// runs.
///
/// Deltas are additive and ordered. Apply them with [`Session::apply`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Delta {
    /// A new turn was appended to the transcript.
    TurnStarted {
        /// The turn, usually an assistant turn with no blocks yet.
        turn: Turn,
    },
    /// More text arrived for a [`Block::Text`].
    TextDelta {
        /// Turn that owns the block.
        turn_id: String,
        /// Index of the block within the turn.
        block_index: usize,
        /// The chunk to append.
        text: String,
    },
    /// A block was appended to a turn.
    BlockAdded {
        /// Turn to append to.
        turn_id: String,
        /// The new block.
        block: Block,
    },
    /// A block was replaced wholesale, e.g. a tool call that finished.
    BlockUpdated {
        /// Turn that owns the block.
        turn_id: String,
        /// Index of the block within the turn.
        block_index: usize,
        /// The block's new value.
        block: Block,
    },
    /// The turn is complete; its footer meta is final.
    TurnFinished {
        /// Turn that finished.
        turn_id: String,
        /// Final model, duration, tokens and cost.
        meta: TurnMeta,
    },
}

/// Fold `delta` into `session`. See [`Session::apply`].
pub(crate) fn apply(session: &mut Session, delta: Delta) -> bool {
    match delta {
        Delta::TurnStarted { turn } => {
            session.turns.push(turn);
            true
        }
        Delta::TextDelta { turn_id, block_index, text } => {
            let Some(blocks) = session.turn_mut(&turn_id).and_then(Turn::blocks_mut) else {
                return false;
            };
            match blocks.get_mut(block_index) {
                Some(Block::Text { text: existing, .. }) => {
                    existing.push_str(&text);
                    true
                }
                _ => false,
            }
        }
        Delta::BlockAdded { turn_id, block } => {
            let Some(blocks) = session.turn_mut(&turn_id).and_then(Turn::blocks_mut) else {
                return false;
            };
            blocks.push(block);
            true
        }
        Delta::BlockUpdated { turn_id, block_index, block } => {
            let Some(blocks) = session.turn_mut(&turn_id).and_then(Turn::blocks_mut) else {
                return false;
            };
            match blocks.get_mut(block_index) {
                Some(slot) => {
                    *slot = block;
                    true
                }
                None => false,
            }
        }
        Delta::TurnFinished { turn_id, meta: new_meta } => {
            match session.turn_mut(&turn_id) {
                Some(Turn::Assistant { blocks, meta, .. }) => {
                    *meta = new_meta;
                    for block in blocks.iter_mut() {
                        if let Block::Text { streaming, .. } = block {
                            *streaming = false;
                        }
                    }
                    true
                }
                _ => false,
            }
        }
    }
}
