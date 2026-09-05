//! The session root: who is running, where, and the turns so far.

use serde::{Deserialize, Serialize};

use crate::turn::Turn;

/// One agent conversation, with everything the shell chrome needs to describe it
/// (provider mark, worktree, branch, permission mode) plus the transcript.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Stable identifier for the session, unique within the app.
    pub id: String,
    /// Which agent CLI is driving this session.
    pub agent: Provider,
    /// Model label as the provider reports it, e.g. `"opus 4.6"`.
    pub model: String,
    /// How tool calls are gated for this session.
    pub mode: PermissionMode,
    /// Working directory the agent runs in, e.g. `"~/work/acme/checkout-flow-v2"`.
    pub cwd: String,
    /// Git branch checked out in [`Session::cwd`], if the directory is a repo.
    pub branch: Option<String>,
    /// Where the agent process actually lives.
    pub environment: Environment,
    /// The transcript, oldest turn first.
    pub turns: Vec<Turn>,
}

impl Session {
    /// A session with no turns yet, running locally in `cwd`.
    pub fn new(id: impl Into<String>, agent: Provider, model: impl Into<String>, cwd: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            agent,
            model: model.into(),
            mode: PermissionMode::Ask,
            cwd: cwd.into(),
            branch: None,
            environment: Environment::Local,
            turns: Vec::new(),
        }
    }

    /// The turn with this id, if it is still in the transcript.
    pub fn turn(&self, turn_id: &str) -> Option<&Turn> {
        self.turns.iter().find(|t| t.id() == turn_id)
    }

    /// Mutable access to the turn with this id.
    pub fn turn_mut(&mut self, turn_id: &str) -> Option<&mut Turn> {
        self.turns.iter_mut().find(|t| t.id() == turn_id)
    }

    /// The id of the last turn, if any.
    pub fn last_turn_id(&self) -> Option<&str> {
        self.turns.last().map(|t| t.id())
    }

    /// Fold a streaming update into the transcript.
    ///
    /// Deltas that name a turn or block that no longer exists are ignored, so a
    /// late delta from a cancelled turn cannot corrupt the session. Returns
    /// `true` when the session changed.
    pub fn apply(&mut self, delta: crate::Delta) -> bool {
        crate::delta::apply(self, delta)
    }
}

/// The agent CLIs the harness can drive.
///
/// This is the semantic list. `aui-icons` keeps its own visual enum for provider
/// marks so the icon set can be reordered or extended without touching the wire
/// format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// Anthropic Claude Code.
    Claude,
    /// OpenAI Codex.
    Codex,
    /// xAI Grok.
    Grok,
    /// Google Gemini.
    Gemini,
    /// Inflection Pi.
    Pi,
    /// Cursor's agent.
    Cursor,
}

/// How the session gates tool calls that need permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Every unapproved tool call raises an approval card.
    Ask,
    /// The agent proposes a plan and edits nothing until it is accepted.
    Plan,
    /// Tool calls matching the remembered rules run without asking.
    Auto,
    /// Nothing is gated; every tool call runs.
    Bypass,
}

/// Where the agent process runs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Environment {
    /// On this machine, in the user's own login shell.
    Local,
    /// Over SSH on `host`.
    Ssh {
        /// SSH destination, e.g. `"build-box"` or `"user@10.0.0.4"`.
        host: String,
    },
    /// On a self-hosted agent server.
    Server {
        /// Display name of the server.
        name: String,
    },
    /// In a managed cloud VM or sandbox.
    CloudVm {
        /// Display name of the VM.
        name: String,
    },
}
