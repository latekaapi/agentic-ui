//! Tool calls: what ran, how it went, and the payload the card renders.

use serde::{Deserialize, Serialize};

use crate::turn::Turn;

/// One line of terminal output.
///
/// Kept as a plain `String` for now, escape sequences included. When
/// `aui-terminal` lands its parser this becomes a struct of styled spans; the
/// alias exists so callers can name the concept today.
pub type AnsiLine = String;

/// Which tool produced a [`crate::Block::ToolCall`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolKind {
    /// A shell command in the session's PTY.
    Shell,
    /// A file read.
    Read,
    /// An edit to an existing file.
    Edit,
    /// A newly written file.
    Write,
    /// A code search across the worktree.
    Search,
    /// A web search or fetch.
    Web,
    /// A browser action driven through the webview.
    Browser,
    /// A nested agent with its own transcript.
    SubAgent,
    /// A tool exposed by an MCP server.
    Mcp {
        /// MCP server name as configured.
        server: String,
        /// Tool name within that server.
        tool: String,
    },
}

/// Server-authored summary of a patch's size, computed by the provider over
/// the whole stored patch document.
///
/// Unlike [`Diff`], which renders one file's hunks in the card body, these
/// are whole-patch counts available without fetching the patch body (the
/// body is a separate round trip on the patch reference). `files` is a file
/// *count* — the number of file entries in the stored patch — not a line
/// count, and nothing derives these numbers from a rendered [`Diff`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiffStat {
    /// Total `+`-prefixed lines across all files' hunks.
    pub added: u64,
    /// Total `-`-prefixed lines across all files' hunks.
    pub removed: u64,
    /// File entries in the stored patch document.
    pub files: u64,
}

/// One tool invocation: what ran, how it went, and the payload the card renders.
///
/// The fields are the [`crate::Block::ToolCall`] variant's fields, so a group
/// ([`crate::Block::ToolGroup`]) reuses this struct instead of restating them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Stable id, used to address the call in deltas and intents.
    pub id: String,
    /// Which tool ran, which picks the icon and the body shape.
    ///
    /// Serialized as `tool_kind`: the shape's own tag already owns `kind`
    /// wherever it is embedded.
    #[serde(rename = "tool_kind")]
    pub kind: ToolKind,
    /// Header verb, e.g. `"Ran"`, `"Edited"`, `"Searched"`.
    pub verb: String,
    /// Mono header target, e.g. a command or a path.
    pub target: String,
    /// Current status, which drives the glyph and the result pill.
    pub status: ToolStatus,
    /// Duration in milliseconds; `None` while still running.
    pub duration_ms: Option<u64>,
    /// Tool-specific payload.
    pub body: ToolBody,
    /// Server-authored diff summary for edit-family calls: the counts the
    /// provider computed over the whole patch, available without fetching
    /// the patch body. `None` for other tools and for payloads written
    /// before the summary existed; the card then draws no chips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_stat: Option<DiffStat>,
}

/// The lifecycle of a tool call, which picks the header glyph and result pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    /// Queued, not started.
    Pending,
    /// Running; the header shows the spinner and a "live" pill.
    Running,
    /// Finished successfully.
    Success,
    /// Finished with an error.
    Error,
    /// Interrupted before it finished.
    Cancelled,
}

/// The tool-specific payload rendered under a tool call's header.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolBody {
    /// Terminal output on the terminal ground.
    Shell {
        /// Output lines, oldest first.
        output_lines: Vec<AnsiLine>,
        /// Process exit code; `None` while still running.
        exit_code: Option<i32>,
        /// `true` while output is still arriving, which shows the "live" pill.
        live: bool,
    },
    /// A file read; the header alone carries the information.
    Read {
        /// Number of lines read.
        lines: usize,
    },
    /// A unified diff of an edit or a new file.
    Edit {
        /// The change.
        diff: Diff,
    },
    /// Code-search results.
    Search {
        /// Matching lines, in file order.
        hits: Vec<SearchHit>,
    },
    /// Web results with favicons and domains.
    Web {
        /// The results, ranked.
        results: Vec<WebResult>,
        /// Results beyond the listed ones (`+3 more`).
        #[serde(default)]
        hidden: usize,
    },
    /// One browser action with an optional screenshot tile.
    Browser {
        /// What the agent did, e.g. `"Clicked 'Try free'"`.
        action: String,
        /// Path or data URI of the screenshot thumbnail.
        screenshot: Option<String>,
        /// The last action's caption on the screenshot, e.g. `click “Try free”`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caption: Option<String>,
    },
    /// A nested transcript from a sub-agent.
    SubAgent {
        /// The sub-agent's own turns.
        turns: Vec<Turn>,
    },
    /// A generic MCP call: a definition list of params plus a JSON result.
    Mcp {
        /// Call parameters as name/value pairs, in declaration order.
        params: Vec<(String, String)>,
        /// The raw JSON result, pretty-printed by the renderer.
        result_json: String,
    },
    /// No body; the header is the whole card.
    None,
}

/// One code-search hit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Path relative to the worktree root.
    pub path: String,
    /// 1-based line number.
    pub line: u32,
    /// The matching line, trimmed for display.
    pub snippet: String,
}

/// One web result row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebResult {
    /// Page title.
    pub title: String,
    /// Full URL.
    pub url: String,
    /// Domain shown after the title, e.g. `"wikipedia.org"`.
    pub domain: String,
}

/// A unified diff for one file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    /// Path relative to the worktree root.
    pub path: String,
    /// Hunks, in file order.
    pub hunks: Vec<Hunk>,
    /// Total lines added.
    pub added: u32,
    /// Total lines removed.
    pub removed: u32,
}

/// One `@@` hunk of a [`Diff`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hunk {
    /// The hunk header, e.g. `"@@ -42,5 +42,7 @@ export function applyMigration"`.
    pub header: String,
    /// The rows of the hunk, in order.
    pub lines: Vec<DiffLine>,
}

/// One row of a [`Hunk`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffLine {
    /// Whether the row is context, an addition or a deletion.
    pub kind: DiffKind,
    /// Line number in the old file; `None` for additions.
    pub old_no: Option<u32>,
    /// Line number in the new file; `None` for deletions.
    pub new_no: Option<u32>,
    /// The line's text, without the leading `+`/`-`.
    pub text: String,
}

/// The three diff row kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    /// Unchanged.
    Context,
    /// Added in the new file.
    Add,
    /// Removed from the old file.
    Del,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_call() -> ToolCall {
        ToolCall {
            id: "tc1".into(),
            kind: ToolKind::Read,
            verb: "Read".into(),
            target: "src/main.rs".into(),
            status: ToolStatus::Success,
            duration_ms: Some(4),
            body: ToolBody::Read { lines: 12 },
            diff_stat: None,
        }
    }

    #[test]
    fn old_payloads_without_diff_stat_decode_with_none() {
        // Payloads written before the summary existed carry no `diff_stat`
        // key; the rest of the call still decodes unchanged.
        let back: ToolCall = serde_json::from_value(serde_json::json!({
            "id": "tc1",
            "tool_kind": {"kind": "read"},
            "verb": "Read",
            "target": "src/main.rs",
            "status": "success",
            "duration_ms": 4,
            "body": {"kind": "read", "lines": 12},
        }))
        .expect("deserialize");
        assert_eq!(back, read_call());
        assert_eq!(back.diff_stat, None);
    }

    #[test]
    fn diff_stat_round_trips_and_omits_none() {
        let call = ToolCall {
            diff_stat: Some(DiffStat { added: 20, removed: 7, files: 4 }),
            ..read_call()
        };
        let json = serde_json::to_value(&call).expect("serialize");
        assert_eq!(json["diff_stat"]["added"], 20);
        assert_eq!(json["diff_stat"]["removed"], 7);
        assert_eq!(json["diff_stat"]["files"], 4);
        let back: ToolCall = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, call);

        // `None` stays off the wire, so old consumers see the old shape.
        let plain = read_call();
        let json = serde_json::to_value(&plain).expect("serialize");
        assert!(json.get("diff_stat").is_none());
        let back: ToolCall = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back.diff_stat, None);
    }
}
