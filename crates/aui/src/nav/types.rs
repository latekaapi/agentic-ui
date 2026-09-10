//! View-model types the sidebar rows render. These are what an app derives
//! from its own state; the rows never read anything else.

use aui_icons::Provider;
use aui_tokens::AgentState;
use gpui::SharedString;

/// One session / worktree as the sidebar shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    /// Stable id, reported by click and action handlers.
    pub id: SharedString,
    /// Name (worktree or task title).
    pub name: SharedString,
    /// Colours the dot.
    pub state: AgentState,
    /// Live (running / waiting): the dot pulses.
    pub pulse: bool,
    /// Elapsed / relative time at the right: `49m`, `3h`, `now`.
    pub elapsed: SharedString,
    /// Repository tag.
    pub repo: Option<SharedString>,
    /// Branch tag.
    pub branch: Option<SharedString>,
    /// Provider marks after the tags.
    pub providers: Vec<Provider>,
    /// Extra items on the meta line, after the tags.
    pub meta: Vec<MetaItem>,
    /// The optional third line.
    pub activity: Option<Activity>,
    /// Unread: the 3 px accent bar at the left edge.
    pub unread: bool,
    /// Pinned: date groupings lift the row into the leading `Pinned` group
    /// instead of its date bucket (card 21's `Pinned 3`).
    pub pinned: bool,
    /// Fan-out children, nested under the row.
    pub children: Vec<SessionSummary>,
}

impl SessionSummary {
    /// A minimal summary; fill the rest with the builder methods.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, state: AgentState, elapsed: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            state,
            pulse: false,
            elapsed: elapsed.into(),
            repo: None,
            branch: None,
            providers: Vec::new(),
            meta: Vec::new(),
            activity: None,
            unread: false,
            pinned: false,
            children: Vec::new(),
        }
    }

    /// Pulses the dot.
    pub fn pulse(mut self) -> Self {
        self.pulse = true;
        self
    }

    /// Sets the repo tag.
    pub fn repo(mut self, repo: impl Into<SharedString>) -> Self {
        self.repo = Some(repo.into());
        self
    }

    /// Sets the branch tag.
    pub fn branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Adds a provider mark.
    pub fn provider(mut self, provider: Provider) -> Self {
        self.providers.push(provider);
        self
    }

    /// Adds a meta item.
    pub fn meta(mut self, item: MetaItem) -> Self {
        self.meta.push(item);
        self
    }

    /// Sets the activity line.
    pub fn activity(mut self, kind: ActivityKind, text: impl Into<SharedString>) -> Self {
        self.activity = Some(Activity { kind, text: text.into() });
        self
    }

    /// Marks unread.
    pub fn unread(mut self) -> Self {
        self.unread = true;
        self
    }

    /// Pins the session: date groupings render it in the leading `Pinned`
    /// group, excluded from the date buckets.
    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }

    /// Adds a child session.
    pub fn child(mut self, child: SessionSummary) -> Self {
        self.children.push(child);
        self
    }
}

/// An item on the meta line.
#[derive(Debug, Clone, PartialEq)]
pub enum MetaItem {
    /// Plain ink-3 text (`PR #2491 open`).
    Text(SharedString),
    /// A mono tag.
    Tag(SharedString),
    /// Danger-coloured text (`2 tests failed`).
    Danger(SharedString),
    /// Warning-coloured text (`awaiting permission`).
    Warning(SharedString),
}

/// The live third line of a row.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    /// How it is drawn.
    pub kind: ActivityKind,
    /// The sentence, truncated.
    pub text: SharedString,
}

/// How the activity line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    /// 10 px spinner + ink-2 text: the agent is working.
    Working,
    /// Shield glyph + warning text: waiting for the person.
    Waiting,
    /// Danger text: something failed.
    Failed,
    /// ink-2 text with no glyph.
    Plain,
}
