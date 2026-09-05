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
    },
    /// One browser action with an optional screenshot tile.
    Browser {
        /// What the agent did, e.g. `"Clicked 'Try free'"`.
        action: String,
        /// Path or data URI of the screenshot thumbnail.
        screenshot: Option<String>,
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
