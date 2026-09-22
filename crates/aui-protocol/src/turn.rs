//! Turns: the person's messages and the assistant's replies.

use serde::{Deserialize, Serialize};

use crate::block::Block;

/// One entry in the transcript.
///
/// User turns render as a right-aligned bubble; assistant turns render as
/// full-width blocks with a meta footer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Turn {
    /// Something the person sent.
    User {
        /// Stable turn id.
        id: String,
        /// Message text, with mentions written inline as `@path`.
        text: String,
        /// Files and images sent with the message.
        attachments: Vec<Attachment>,
        /// Structured mentions parsed out of `text`, in order of appearance.
        mentions: Vec<Mention>,
        /// When the turn was sent, as Unix milliseconds. `None` when the
        /// adapter never reported one — an older capture, or a turn the app
        /// built by hand.
        ///
        /// Additive: defaults to `None` when it is absent from serialized
        /// data, and is skipped on the wire while it is `None`, so old
        /// payloads keep decoding and new ones stay small.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timestamp: Option<u64>,
    },
    /// One assistant reply, made of transcript blocks.
    Assistant {
        /// Stable turn id.
        id: String,
        /// The cards and text of this reply, in render order.
        blocks: Vec<Block>,
        /// Footer facts: model, duration, tokens, cost.
        meta: TurnMeta,
        /// When the turn started, as Unix milliseconds. `None` when the
        /// adapter never reported one — see the user variant's `timestamp`.
        ///
        /// Additive: defaults to `None` when it is absent from serialized
        /// data, and is skipped on the wire while it is `None`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timestamp: Option<u64>,
    },
}

impl Turn {
    /// The turn's id, whichever variant it is.
    pub fn id(&self) -> &str {
        match self {
            Turn::User { id, .. } | Turn::Assistant { id, .. } => id,
        }
    }

    /// When the turn was sent (user) or started (assistant), as Unix
    /// milliseconds, or `None` when the adapter never reported one.
    pub fn timestamp(&self) -> Option<u64> {
        match self {
            Turn::User { timestamp, .. } | Turn::Assistant { timestamp, .. } => *timestamp,
        }
    }

    /// The blocks of an assistant turn; empty for a user turn.
    pub fn blocks(&self) -> &[Block] {
        match self {
            Turn::Assistant { blocks, .. } => blocks,
            Turn::User { .. } => &[],
        }
    }

    /// Mutable blocks of an assistant turn, or `None` for a user turn.
    pub fn blocks_mut(&mut self) -> Option<&mut Vec<Block>> {
        match self {
            Turn::Assistant { blocks, .. } => Some(blocks),
            Turn::User { .. } => None,
        }
    }
}

/// The mono footer under an assistant turn: `model · duration · tokens · cost`.
///
/// The cache counters below are informational only: they are *not directly
/// summable across providers*, and `cached_tokens` may be counted inside or
/// beside `tokens_in` depending on the provider's convention — adding any of
/// them to `tokens_in` double-counts. Nothing fills them yet; they ship inert
/// (unknown / `0`) until the fold does.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TurnMeta {
    /// Model that produced the turn, e.g. `"opus 4.6"`.
    pub model: String,
    /// Wall-clock time for the turn, in milliseconds.
    pub duration_ms: u64,
    /// Prompt tokens billed.
    pub tokens_in: u64,
    /// Completion tokens billed.
    pub tokens_out: u64,
    /// Reasoning tokens billed (MSP `TokenUsage.reasoningTokens`).
    ///
    /// Additive: [`TurnMeta`] derives `Default`, and the field defaults to `0`
    /// when it is absent from serialized data.
    #[serde(default)]
    pub reasoning_tokens: u64,
    /// Cost of the turn in US dollars.
    pub cost_usd: f64,
    /// Cache read tokens (MSP `TokenUsage.cacheReadTokens`).
    ///
    /// `None` when the provider does not distinguish cache reads from writes —
    /// "the provider did not tell us", which is different from reporting zero.
    /// Not summable across providers; never add to `tokens_in`.
    ///
    /// Additive: defaults to `None` when it is absent from serialized data.
    #[serde(default)]
    pub cache_read_tokens: Option<u64>,
    /// Cache write tokens (MSP `TokenUsage.cacheWriteTokens`).
    ///
    /// `None` when the provider does not distinguish cache writes from reads —
    /// see `cache_read_tokens`. Not summable across providers; never add to
    /// `tokens_in`.
    ///
    /// Additive: defaults to `None` when it is absent from serialized data.
    #[serde(default)]
    pub cache_write_tokens: Option<u64>,
    /// Provider-reported cache tokens (MSP `TokenUsage.cachedTokens`).
    ///
    /// This is what it is not: it may be counted inside or beside `tokens_in`
    /// depending on the provider's convention, so adding it to `tokens_in`
    /// double-counts, and it is not directly summable across providers.
    ///
    /// Additive: defaults to `0` when it is absent from serialized data.
    #[serde(default)]
    pub cached_tokens: u64,
}

/// A file or image sent with a user turn, shown as a chip above the bubble.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// File name as shown on the chip, e.g. `"checkout-form.png"`.
    pub name: String,
    /// What the chip's icon tile should show.
    pub kind: AttachmentKind,
    /// Size on disk in bytes; `None` while it is still unknown.
    pub size_bytes: Option<u64>,
    /// One-line detail shown beside the name on the chip, e.g. `"180 lines"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<String>,
    /// Upload progress for the chip's overlay.
    pub state: UploadState,
}

/// The three attachment shapes the composer accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    /// A raster image, shown with a thumbnail tile.
    Image,
    /// Any other binary or source file.
    File,
    /// A pasted text snippet promoted to an attachment.
    Text,
}

/// Where an attachment is in its upload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UploadState {
    /// Uploaded and usable by the agent.
    Ready,
    /// Still transferring.
    Uploading {
        /// Fraction complete, 0.0 to 1.0.
        progress: f32,
    },
    /// The transfer failed; the chip shows a retry affordance.
    Failed {
        /// Human-readable failure, e.g. `"file too large"`.
        reason: String,
    },
}

/// An `@` mention chip inside a user turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mention {
    /// What kind of thing was mentioned, which picks the chip's icon.
    pub kind: MentionKind,
    /// Text shown on the chip, e.g. `"@src/checkout"`.
    pub label: String,
}

/// The things the composer can mention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentionKind {
    /// A file or directory in the worktree.
    File,
    /// A symbol resolved from the index.
    Symbol,
    /// Another worktree in the project.
    Worktree,
    /// An external URL.
    Url,
    /// A named skill or slash command.
    Skill,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_counters_default_when_absent() {
        // Old serialized data carries none of the three keys: it still parses.
        let meta: TurnMeta = serde_json::from_value(serde_json::json!({
            "model": "opus 4.6",
            "duration_ms": 3_100,
            "tokens_in": 1_900,
            "tokens_out": 500,
            "reasoning_tokens": 120,
            "cost_usd": 0.04,
        }))
        .expect("decode");
        assert_eq!(meta.cache_read_tokens, None);
        assert_eq!(meta.cache_write_tokens, None);
        assert_eq!(meta.cached_tokens, 0);
    }

    #[test]
    fn cache_counters_survive_a_round_trip() {
        let meta = TurnMeta {
            model: "opus 4.6".into(),
            duration_ms: 3_100,
            tokens_in: 1_900,
            tokens_out: 500,
            reasoning_tokens: 120,
            cost_usd: 0.04,
            cache_read_tokens: Some(800),
            cache_write_tokens: Some(200),
            cached_tokens: 1_000,
        };
        let json = serde_json::to_value(&meta).expect("serialize");
        assert_eq!(json["cache_read_tokens"], 800);
        assert_eq!(json["cache_write_tokens"], 200);
        assert_eq!(json["cached_tokens"], 1_000);
        let back: TurnMeta = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, meta);

        // "The provider said zero" stays distinct from "did not tell us".
        let zero = TurnMeta {
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            cached_tokens: 0,
            ..TurnMeta::default()
        };
        let back: TurnMeta =
            serde_json::from_value(serde_json::to_value(&zero).expect("serialize"))
                .expect("deserialize");
        assert_eq!(back.cache_read_tokens, Some(0));
        assert_eq!(back.cache_write_tokens, Some(0));
        assert_eq!(back, zero);
    }
}
