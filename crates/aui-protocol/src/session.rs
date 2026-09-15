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
    /// Client-side plan-mode overlay; MSP has no plan mode — see harness spec §3.1.
    ///
    /// Plan mode is not a wire value: the client turns it on, remembers the
    /// previous [`PermissionMode`], sets [`PermissionMode::DenyUnmatched`] for
    /// the duration and prefixes the prompt. It rides here so the chrome can
    /// show the "Plan" pill.
    #[serde(default)]
    pub plan: bool,
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
            mode: PermissionMode::default(),
            plan: false,
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
    /// Meta's Muse Code.
    Muse,
}

/// How the session gates tool calls that need permission.
///
/// These are the four MSP approval modes (`ApprovalMode`, `msp.d.ts:79`), and
/// the `camelCase` serde names below **are** the MSP wire values
/// (`allowAll` / `onRequest` / `promptUnmatched` / `denyUnmatched`) — that is
/// deliberate, so an adapter can hand the value straight to
/// `session/setApprovalMode` without a translation table. The set is closed: a
/// client selects a preconfigured mode and can never construct one.
///
/// Plan mode is **not** a member: it is a client-side overlay carried by
/// [`Session::plan`]. See the harness spec §3.6.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Nothing is gated; every action runs. Labelled "Full access".
    AllowAll,
    /// The default. Policy decides, and an action with no matching rule is only
    /// raised when the agent itself asks. Labelled "Auto".
    #[default]
    OnRequest,
    /// Anything without a matching rule raises an approval card. Labelled "Ask".
    PromptUnmatched,
    /// Anything without a matching rule is refused outright. Labelled
    /// "Read-only".
    DenyUnmatched,
}

impl PermissionMode {
    /// The picker label, from harness spec §3.6.
    pub fn label(&self) -> &'static str {
        match self {
            PermissionMode::AllowAll => "Full access",
            PermissionMode::OnRequest => "Auto",
            PermissionMode::PromptUnmatched => "Ask",
            PermissionMode::DenyUnmatched => "Read-only",
        }
    }

    /// The one-line description shown under the label in the mode picker.
    pub fn description(&self) -> &'static str {
        match self {
            PermissionMode::AllowAll => "Nothing is gated; every action runs.",
            PermissionMode::OnRequest => "Policy decides; unmatched actions ask only when the agent requests it.",
            PermissionMode::PromptUnmatched => "Anything without a matching rule raises an approval card.",
            PermissionMode::DenyUnmatched => "Anything without a matching rule is refused.",
        }
    }
}

/// How much reasoning the provider should spend on a turn.
///
/// This is a **closed** set on purpose: a backend's on-disk catalog can
/// advertise a tier before its wire protocol actually accepts it (seen in
/// the wild — a tier the catalog listed came back `unknown variant` from the
/// server for one release), so a client should drive its effort picker from
/// this enum, never from a backend's catalog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReasoningEffort {
    /// No reasoning at all.
    None,
    /// The smallest budget the provider offers.
    Minimal,
    /// A short budget.
    Low,
    /// The middle budget, and the usual default.
    #[default]
    Medium,
    /// A long budget.
    High,
    /// Longer than [`ReasoningEffort::High`].
    Xhigh,
    /// Longer than extra high, below ultra (muse 1.1.1).
    Max,
    /// The largest budget MSP accepts.
    Ultra,
}

impl ReasoningEffort {
    /// The picker label for this tier.
    pub fn label(&self) -> &'static str {
        match self {
            ReasoningEffort::None => "None",
            ReasoningEffort::Minimal => "Minimal",
            ReasoningEffort::Low => "Low",
            ReasoningEffort::Medium => "Medium",
            ReasoningEffort::High => "High",
            ReasoningEffort::Xhigh => "Extra high",
            ReasoningEffort::Max => "Max",
            ReasoningEffort::Ultra => "Ultra",
        }
    }
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
