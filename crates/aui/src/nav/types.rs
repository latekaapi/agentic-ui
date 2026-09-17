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
    /// An explicit second line, distinct from the `meta` preview tags. When
    /// set it wins the row's second-line slot; rows without one show the
    /// legacy meta tags (or empty space, keeping the two-line height).
    pub byline: Option<Byline>,
    /// The optional third line.
    pub activity: Option<Activity>,
    /// Option B's context override: the approval command or pending question
    /// when one exists. Wins the context (second) line over [`Byline`],
    /// preview and `project · branch`; the caller supplies the exact words.
    pub attention: Option<SharedString>,
    /// Option B's status verb line (third line): the semibold state-coloured
    /// sentence. The caller supplies the variable words in `detail`; the
    /// library owns the colour and weight. `None` keeps the line as empty
    /// space, so every row stays the same height.
    pub status: Option<RowStatus>,
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
            byline: None,
            activity: None,
            attention: None,
            status: None,
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

    /// Sets the second line to a muted placeholder (`Working…`, `No reply
    /// yet`): what the row shows while there is nothing else to say, e.g.
    /// while a generated title is still being written. The library dims it;
    /// the caller picks the words.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.byline = Some(Byline::Placeholder(text.into()));
        self
    }

    /// Sets the second line to one line of preview text, in place of the
    /// meta tags for this row. Rows without an explicit second line keep
    /// showing the [`Self::meta`] tags, so existing callers are unchanged.
    pub fn preview(mut self, text: impl Into<SharedString>) -> Self {
        self.byline = Some(Byline::Preview(text.into()));
        self
    }

    /// Sets the second line to the two-part byline: what was last asked
    /// (`ask`) and what came back (`result`), side by side on the row's one
    /// second line, each truncating with an ellipsis at the row's width.
    pub fn byline(mut self, ask: impl Into<SharedString>, result: impl Into<SharedString>) -> Self {
        self.byline = Some(Byline::TwoLines { ask: ask.into(), result: result.into() });
        self
    }

    /// Sets the activity line.
    pub fn activity(mut self, kind: ActivityKind, text: impl Into<SharedString>) -> Self {
        self.activity = Some(Activity { kind, text: text.into() });
        self
    }

    /// Sets option B's context override: the approval command or pending
    /// question, shown verbatim on the context line ahead of [`Self::byline`],
    /// preview and `project · branch`.
    pub fn attention(mut self, text: impl Into<SharedString>) -> Self {
        self.attention = Some(text.into());
        self
    }

    /// Sets option B's status verb line: `kind` picks the vocabulary, colour
    /// and weight; `detail` carries the variable words (elapsed, the quoted
    /// question, `12m · 5 turns`). Empty detail renders the bare verb
    /// (`Working`, `Settled`, `Failed`, `No reply yet`).
    pub fn status(mut self, kind: RowStatusKind, detail: impl Into<SharedString>) -> Self {
        self.status = Some(RowStatus { kind, detail: detail.into() });
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

/// An explicit second line for a session row, distinct from the `meta`
/// preview tags. Whatever the caller passes, the row keeps its two-line
/// height: with nothing to show the line renders as empty space.
#[derive(Debug, Clone, PartialEq)]
pub enum Byline {
    /// Muted placeholder text (`Working…`, `No reply yet`): dimmed ink, no
    /// spinner. The app decides the words; the library decides the look.
    Placeholder(SharedString),
    /// One line of preview text.
    Preview(SharedString),
    /// Last ask plus last result, side by side on the row's one second
    /// line. Each half truncates with an ellipsis at the row's width and
    /// never wraps onto another line.
    TwoLines {
        /// What was last asked.
        ask: SharedString,
        /// What came back.
        result: SharedString,
    },
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

/// Option B's status verb: which sentence the status line draws, and in
/// which state colour. The caller supplies the variable words through
/// [`RowStatus::detail`]; the library owns the vocabulary, colour and
/// weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatusKind {
    /// `Working · {detail}` in accent-ink, the mockup's running tone.
    Working,
    /// `Needs approval` in warning.
    NeedsApproval,
    /// `Asked: "{detail}"` in warning; bare `Asked` when detail is empty.
    Asked,
    /// `Settled · {detail}` in muted.
    Settled,
    /// `Failed · {detail}` in danger; bare `Failed` when detail is empty.
    Failed,
    /// `No reply yet · {detail}` in muted; bare `No reply yet` when empty.
    NoReply,
}

/// Option B's status verb line: `kind` picks the vocabulary, colour and
/// weight; `detail` carries the variable words (elapsed, the quoted
/// question, `12m · 5 turns`). Build through [`SessionSummary::status`].
#[derive(Debug, Clone, PartialEq)]
pub struct RowStatus {
    /// Which sentence, colour and weight.
    pub kind: RowStatusKind,
    /// The variable words; empty renders the bare verb.
    pub detail: SharedString,
}

impl RowStatus {
    /// A status of `kind` with `detail`'s variable words.
    pub fn new(kind: RowStatusKind, detail: impl Into<SharedString>) -> Self {
        Self { kind, detail: detail.into() }
    }

    /// The sentence the status line draws: the verb plus `detail` where the
    /// state carries one. `Asked` wraps a non-empty detail in double quotes;
    /// the other suffixed states join with ` · `.
    pub fn text(&self) -> SharedString {
        let detail = self.detail.trim();
        match self.kind {
            RowStatusKind::Working if detail.is_empty() => "Working".into(),
            RowStatusKind::Working => format!("Working · {detail}").into(),
            RowStatusKind::NeedsApproval => "Needs approval".into(),
            RowStatusKind::Asked if detail.is_empty() => "Asked".into(),
            RowStatusKind::Asked => format!("Asked: \"{detail}\"").into(),
            RowStatusKind::Settled if detail.is_empty() => "Settled".into(),
            RowStatusKind::Settled => format!("Settled · {detail}").into(),
            RowStatusKind::Failed if detail.is_empty() => "Failed".into(),
            RowStatusKind::Failed => format!("Failed · {detail}").into(),
            RowStatusKind::NoReply if detail.is_empty() => "No reply yet".into(),
            RowStatusKind::NoReply => format!("No reply yet · {detail}").into(),
        }
    }

    /// Ink for the status sentence, from tokens: accent-ink for working,
    /// warning for approval/questions, danger for failed, muted otherwise.
    pub fn ink(&self, palette: &aui_tokens::Palette) -> gpui::Hsla {
        match self.kind {
            RowStatusKind::Working => palette.accent_ink,
            RowStatusKind::NeedsApproval | RowStatusKind::Asked => palette.warning,
            RowStatusKind::Failed => palette.danger,
            RowStatusKind::Settled | RowStatusKind::NoReply => palette.ink_3,
        }
    }
}
