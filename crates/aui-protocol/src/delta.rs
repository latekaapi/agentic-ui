//! Streaming updates and how they fold into a [`Session`].

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::session::Session;
use crate::tool::ToolBody;
use crate::turn::{Turn, TurnMeta};

/// One incremental change to a session, as an adapter emits it while the agent
/// runs.
///
/// Deltas are ordered. Most are additive; [`Delta::TurnRemoved`] and
/// [`Delta::BlockRemoved`] take something away.
///
/// # The fold contract
///
/// Every variant follows the same rule: **a delta that names a turn, a block or
/// a body shape that is not there is ignored, and [`Session::apply`] returns
/// whether the session actually changed.** A late delta from a cancelled turn
/// can therefore never corrupt the transcript, and a caller can use the return
/// value to decide whether to repaint.
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
    /// More reasoning text arrived for a [`Block::Thinking`]; appended to its
    /// `text`.
    ThinkingDelta {
        /// Turn that owns the block.
        turn_id: String,
        /// Index of the block within the turn.
        block_index: usize,
        /// The chunk to append.
        text: String,
    },
    /// More terminal output arrived for a [`Block::ToolCall`].
    ///
    /// Appended to the call's output, splitting on `\n` so a partial line
    /// merges into the last entry rather than starting a new one. Only
    /// [`crate::ToolBody::Shell`] has somewhere to put it; every other body
    /// shape ignores the delta.
    ToolOutputDelta {
        /// Turn that owns the block.
        turn_id: String,
        /// Index of the block within the turn.
        block_index: usize,
        /// The chunk to append.
        text: String,
    },
    /// A turn left the transcript: MSP `turn/retracted` or `turn/unqueued`.
    TurnRemoved {
        /// Turn to remove.
        turn_id: String,
    },
    /// One block left a turn.
    BlockRemoved {
        /// Turn that owns the block.
        turn_id: String,
        /// Index of the block to remove.
        block_index: usize,
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
        Delta::ThinkingDelta { turn_id, block_index, text } => {
            let Some(blocks) = session.turn_mut(&turn_id).and_then(Turn::blocks_mut) else {
                return false;
            };
            match blocks.get_mut(block_index) {
                Some(Block::Thinking { text: existing, .. }) => {
                    existing.push_str(&text);
                    true
                }
                _ => false,
            }
        }
        Delta::ToolOutputDelta { turn_id, block_index, text } => {
            let Some(blocks) = session.turn_mut(&turn_id).and_then(Turn::blocks_mut) else {
                return false;
            };
            match blocks.get_mut(block_index) {
                Some(Block::ToolCall { body: ToolBody::Shell { output_lines, .. }, .. }) => {
                    append_output(output_lines, &text)
                }
                _ => false,
            }
        }
        Delta::TurnRemoved { turn_id } => {
            let Some(index) = session.turns.iter().position(|t| t.id() == turn_id) else {
                return false;
            };
            session.turns.remove(index);
            true
        }
        Delta::BlockRemoved { turn_id, block_index } => {
            let Some(blocks) = session.turn_mut(&turn_id).and_then(Turn::blocks_mut) else {
                return false;
            };
            if block_index >= blocks.len() {
                return false;
            }
            blocks.remove(block_index);
            true
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

/// Append a chunk of terminal output to `output_lines`, keeping partial lines
/// whole.
///
/// The chunk is split on `\n`: the first segment continues the last line the
/// call already has, and every following segment starts a new one. An empty
/// chunk changes nothing and returns `false`.
fn append_output(output_lines: &mut Vec<crate::tool::AnsiLine>, text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let mut parts = text.split('\n');
    let first = parts.next().unwrap_or_default();
    if !first.is_empty() {
        match output_lines.last_mut() {
            Some(last) => last.push_str(first),
            None => output_lines.push(first.to_string()),
        }
    }
    for part in parts {
        output_lines.push(part.to_string());
    }
    true
}
