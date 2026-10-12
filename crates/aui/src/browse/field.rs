//! The Field: the browser's single input for addresses, tabs, history,
//! commands, Ask and Task.
//!
//! `⌘T` opens it empty for a new tab, `⌘L` opens it with the current address
//! selected. The Field is drawn as an overlay — the host decides whether
//! in-window (through [`crate::overlay::popover_layer`]) or in its own
//! window — and this module builds the component and its model. The look and
//! the keyboard conventions follow [`crate::overlay::command_palette`]: the
//! caller owns the query string, the rows and which row is selected, and
//! receives intents back. The card carries [`crate::keys::MENU_CONTEXT`] and
//! the host wires it the way the palette hosts do: `↑`/`↓`
//! ([`crate::keys::SelectPrev`]/[`crate::keys::SelectNext`]) move the
//! highlight with wrap (see [`select_next`]/[`select_prev`]), `↩`
//! ([`crate::keys::Confirm`]) confirms the selected row, `esc`
//! ([`crate::keys::Cancel`]) closes.
//!
//! The modified keys are the host's to interpret from the same model: `⌘↩`
//! confirms as background work (the `Task` row), `⌥↩` confirms as an exact
//! search (the `Search` row), and `⇥` accepts [`completion`] into the query.
//! The grey completion is also clickable, which reports through
//! [`Field::on_query_change`].

use std::cell::RefCell;
use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, ScrollHandle, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::base::input::TextareaState;

use crate::data::{kbd, record_ax_label};
use crate::keys::MENU_CONTEXT;
use crate::util::{indexed_child, interaction_flags};

// ---------------------------------------------------------------- the model

/// What a [`FieldRow`] does on confirm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldRowKind {
    /// Open the typed address. The first row while the query looks like a
    /// URL or host; Ask moves second. Carries the address so the host never
    /// re-derives it from the title.
    Open {
        /// The address confirming the row opens.
        url: String,
    },
    /// Reopen the copied link. An empty query shows it when the clipboard
    /// holds a link; a row of its own so it never reads as a typed address.
    Clipboard {
        /// The copied address confirming the row opens.
        url: String,
    },
    /// Ask the provider; the answer lands here.
    Ask,
    /// Run the query as an exact search in the configured engine.
    Search,
    /// Hand the query to the provider as background work in its own tabs.
    Task,
    /// Switch to an open tab. Never duplicates the tab.
    Tab {
        /// The tab the host switches to.
        id: SharedString,
    },
    /// Reopen a history entry.
    History {
        /// The address the entry points at.
        url: SharedString,
    },
    /// Run a browser command or setting.
    Command {
        /// The command the host runs.
        id: SharedString,
    },
}

/// One row of the Field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldRow {
    /// What confirming the row does.
    pub kind: FieldRowKind,
    /// The row title (`Ask Claude — answers here`, a tab title, …).
    pub title: SharedString,
    /// The faint second line: a tab's host, a history URL, nothing for
    /// agent and command rows.
    pub detail: Option<SharedString>,
    /// The right-hand hint: `↩`, `⌘↩`, `⌥↩`, or a history row's relative
    /// read word (`read Tuesday`).
    pub hint: SharedString,
}

impl FieldRow {
    /// The accessible name of the row's button: `<title>, <hint>`.
    pub fn label(&self) -> SharedString {
        SharedString::from(format!("{}, {}", self.title, self.hint))
    }
}

/// The Field's state, owned by the host. Build the rows with [`rank`].
#[derive(Debug, Clone, PartialEq)]
pub struct FieldModel {
    /// The typed query.
    pub query: SharedString,
    /// The rows, in display order (see [`rank`]).
    pub rows: Vec<FieldRow>,
    /// The highlighted row, an index into [`FieldModel::rows`].
    pub selected: usize,
}

impl FieldModel {
    /// A model over `rows` with nothing selected past the first row.
    pub fn new(query: impl Into<SharedString>, rows: Vec<FieldRow>) -> Self {
        Self { query: query.into(), rows, selected: 0 }
    }

    /// The highlighted row, if the selection lands on one.
    pub fn selected_row(&self) -> Option<&FieldRow> {
        self.rows.get(self.selected)
    }
}

/// An open tab the Field can switch to. `tabs` order is recency order: the
/// first three are the recent tabs an empty query shows.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldTab {
    /// Stable identity, echoed back in [`FieldRowKind::Tab`].
    pub id: SharedString,
    /// The tab title.
    pub title: SharedString,
    /// The tab host, matched and shown faint after the title.
    pub host: SharedString,
}

/// A history entry. `history` order is frequency order: the first three are
/// the frequent sites an empty query shows.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldHistory {
    /// The page title.
    pub title: SharedString,
    /// The address, matched and shown faint after the title.
    pub url: SharedString,
    /// The right-aligned relative word (`read Tuesday`).
    pub read: SharedString,
}

/// A browser command or setting, matched by name.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldCommand {
    /// Stable identity, echoed back in [`FieldRowKind::Command`].
    pub id: SharedString,
    /// The command name (`Split View`, `Reading mode`, `New Space…`).
    pub name: SharedString,
}

/// Everything [`rank`] reads: the tabs, the history and the commands, plus
/// the parameters the agent rows are built from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FieldSources {
    /// The provider name on the Ask and Task rows (`Claude`).
    pub provider: SharedString,
    /// The search engine name on the Search row (`Google`).
    pub search_engine: SharedString,
    /// The open tabs, most recent first.
    pub tabs: Vec<FieldTab>,
    /// The history, most frequent first.
    pub history: Vec<FieldHistory>,
    /// The commands and settings.
    pub commands: Vec<FieldCommand>,
    /// A link the person copied since, if any: an empty query shows it as
    /// the Clipboard row.
    pub clipboard: Option<SharedString>,
}

impl FieldSources {
    /// Sources for `provider` answering with `search_engine` searching.
    pub fn new(provider: impl Into<SharedString>, search_engine: impl Into<SharedString>) -> Self {
        Self {
            provider: provider.into(),
            search_engine: search_engine.into(),
            tabs: Vec::new(),
            history: Vec::new(),
            commands: Vec::new(),
            clipboard: None,
        }
    }
}

/// At most this many tab rows under a query.
const MAX_TABS: usize = 5;
/// At most this many history rows under a query.
const MAX_HISTORY: usize = 4;
/// At most this many command rows under a query.
const MAX_COMMANDS: usize = 4;
/// How many recent tabs and frequent sites an empty query shows of each.
const EMPTY_EACH: usize = 3;

/// Whether the query looks like a URL or bare host: no spaces, and either a
/// `scheme://` form (`https://x.y`), an `about:`/`file:` form, `localhost`
/// with an optional numeric port, or a dotted host whose last label is at
/// least two letters (`ncert.nic.in`). A single dot is not enough on its
/// own — `report.pdf` reads as a filename, and `3.14` as a number — unless
/// the last label is a familiar top-level domain (`example.com`). While the
/// query looks like a URL, the first row opens it and Ask moves second.
pub fn looks_like_url(query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() || query.contains(char::is_whitespace) {
        return false;
    }
    let lower = query.to_lowercase();
    if lower.starts_with("about:") || lower.starts_with("file:") {
        return true;
    }
    if let Some(scheme_end) = lower.find("://") {
        let scheme = &lower[..scheme_end];
        if !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
            return true;
        }
    }
    // The host is whatever precedes any path, without any `:port`.
    let host = query.split('/').next().unwrap_or("");
    let bare = host.split(':').next().unwrap_or("");
    if bare.eq_ignore_ascii_case("localhost") {
        let port_ok = match host.split_once(':') {
            None => true,
            Some((_, port)) => !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()),
        };
        if port_ok {
            return true;
        }
    }
    let labels: Vec<&str> = bare.split('.').collect();
    if labels.len() >= 2 && labels.iter().all(|label| !label.is_empty()) {
        let tld = labels[labels.len() - 1].to_lowercase();
        if tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) {
            // Several dots read as a host on their own; a single dot needs a
            // familiar top-level domain, or the token is a filename.
            if labels.len() >= 3 || KNOWN_TLDS.contains(&tld.as_str()) {
                return true;
            }
        }
    }
    false
}

/// Top-level domains a lone `name.tld` token may end in and still read as a
/// host. Anything else with a single dot reads as a filename (`report.pdf`).
const KNOWN_TLDS: &[&str] = &[
    "com", "org", "net", "io", "dev", "app", "gov", "edu", "mil", "int", "biz", "info", "name", "me", "tv", "cc", "co", "uk", "us",
    "ca", "au", "de", "fr", "jp", "cn", "in", "br", "kr", "it", "es", "nl", "se", "ch", "nz", "ie", "za", "sg", "hk", "tw", "mx",
    "pl", "be", "at", "dk", "fi", "no", "pt", "gr", "cz", "hu", "ro", "tr", "il", "ae", "sa", "ph", "id", "my", "th", "vn", "pk",
    "bd", "lk", "np", "ru",
];

/// Builds the rows for `query`, in display order: the agent rows (no
/// caption), then matching tabs, history and commands. Matching is a
/// case-insensitive substring with prefix matches first; tabs sort before
/// history regardless of score. A history entry never duplicates an open tab
/// — when its URL's host is already open the tab row says it — and never
/// duplicates the Open row either. An empty query shows nothing but the
/// recent tabs, the frequent sites and the Clipboard row.
pub fn rank(query: &str, sources: &FieldSources) -> Vec<FieldRow> {
    let query = query.trim();
    if query.is_empty() {
        let mut rows = Vec::new();
        rows.extend(sources.tabs.iter().take(EMPTY_EACH).map(tab_row));
        rows.extend(sources.history.iter().filter(|entry| !host_open(&entry.url, sources)).take(EMPTY_EACH).map(history_row));
        if let Some(url) = &sources.clipboard {
            rows.push(FieldRow {
                kind: FieldRowKind::Clipboard { url: url.to_string() },
                title: SharedString::from(format!("Clipboard — {url}")),
                detail: None,
                hint: SharedString::from("↩"),
            });
        }
        return rows;
    }
    let open = looks_like_url(query);
    let mut rows = Vec::with_capacity(3 + MAX_TABS + MAX_HISTORY + MAX_COMMANDS);
    if open {
        rows.push(FieldRow {
            kind: FieldRowKind::Open { url: query.to_string() },
            title: SharedString::from(format!("Open {query}")),
            detail: None,
            hint: SharedString::from("↩"),
        });
    }
    rows.push(FieldRow {
        kind: FieldRowKind::Ask,
        title: SharedString::from(format!("Ask {} — answers here", sources.provider)),
        detail: None,
        hint: SharedString::from("↩"),
    });
    rows.push(FieldRow {
        kind: FieldRowKind::Search,
        title: SharedString::from(format!("Search {}", sources.search_engine)),
        detail: None,
        hint: SharedString::from("⌥↩"),
    });
    rows.push(FieldRow {
        kind: FieldRowKind::Task,
        title: SharedString::from(format!("Task for {} — in its own tabs", sources.provider)),
        detail: None,
        hint: SharedString::from("⌘↩"),
    });
    rows.extend(ranked(sources.tabs.iter().map(|tab| (score(&[&tab.title, &tab.host], query), tab_row(tab))), MAX_TABS));
    rows.extend(ranked(
        sources
            .history
            .iter()
            .filter(|entry| !host_open(&entry.url, sources) && (!open || !same_url(&entry.url, query)))
            .map(|entry| (score(&[&entry.title, &entry.url], query), history_row(entry))),
        MAX_HISTORY,
    ));
    rows.extend(ranked(sources.commands.iter().map(|command| (score(&[&command.name], query), command_row(command))), MAX_COMMANDS));
    rows
}

/// Whether an open tab already covers `url`: the URL's host, case-folded,
/// equals some tab's host. Such a history entry is suppressed — the tab row
/// says it already.
fn host_open(url: &str, sources: &FieldSources) -> bool {
    let host = url_host(url);
    sources.tabs.iter().any(|tab| tab.host.to_lowercase() == host)
}

/// The host of `url`, case-folded: without any scheme, path or query.
fn url_host(url: &str) -> String {
    let url = url.trim().to_lowercase();
    let bare = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => url.as_str(),
    };
    bare.split('/').next().unwrap_or("").to_string()
}

/// Whether two addresses name the same page: case-folded, without any
/// scheme, and ignoring one trailing slash.
fn same_url(left: &str, right: &str) -> bool {
    fn norm(url: &str) -> String {
        let url = url.trim().to_lowercase();
        let bare = match url.split_once("://") {
            Some((_, rest)) => rest.to_string(),
            None => url,
        };
        bare.strip_suffix('/').unwrap_or(&bare).to_string()
    }
    norm(left) == norm(right)
}

/// The grey inline completion: the first tab host or history URL that starts
/// with the query. The element draws the untyped tail grey after the caret;
/// `⇥` (or a click on it) accepts it. `None` for an empty query, for a query
/// with surrounding whitespace (the tail would slice at the wrong length),
/// or when nothing completes it. Matching case-folds with `str::to_lowercase`,
/// like [`rank`].
pub fn completion(query: &str, rows: &[FieldRow]) -> Option<String> {
    if query != query.trim() {
        return None;
    }
    if query.is_empty() {
        return None;
    }
    let folded = query.to_lowercase();
    for row in rows {
        let candidate: &str = match &row.kind {
            FieldRowKind::Tab { .. } => match &row.detail {
                Some(host) => host,
                None => continue,
            },
            FieldRowKind::History { url } => url,
            _ => continue,
        };
        let candidate_folded = candidate.to_lowercase();
        if candidate_folded.len() > folded.len() && candidate_folded.starts_with(folded.as_str()) {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Moves the highlight one row down, wrapping past the last row to the first.
/// A stale selection (past the end) restarts at the first row; an empty list
/// stays at zero.
pub fn select_next(selected: usize, len: usize) -> usize {
    if len == 0 || selected >= len {
        return 0;
    }
    (selected + 1) % len
}

/// Moves the highlight one row up, wrapping past the first row to the last.
/// A stale selection (past the end) lands on the last row; an empty list
/// stays at zero.
pub fn select_prev(selected: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if selected == 0 || selected >= len {
        return len - 1;
    }
    selected - 1
}

// ------------------------------------------------------------------ intents

/// The keys the Field host turns into [`FieldIntent`]s through
/// [`intent_for`]. The element itself owns no keys — the host wires
/// [`MENU_CONTEXT`] the way the palette hosts do — so keys arrive here
/// already interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKey {
    /// `↩`: confirm the selected row.
    Enter,
    /// `⌘↩`: confirm the selected row as background work.
    CmdEnter,
    /// `⌥↩`: run the query as an exact search.
    AltEnter,
    /// `⇥`: accept the grey [`completion`] into the query.
    Tab,
    /// `esc`: close the Field.
    Escape,
}

/// What the host should do next: the Field's answer to one [`FieldKey`], or
/// to a click on a row, the completion tail, or the `esc` keycap.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldIntent {
    /// Confirm `row` in this window.
    Open(FieldRow),
    /// Confirm `row` as background work, in its own tabs.
    Background(FieldRow),
    /// Run the query as an exact search in the configured engine.
    SearchExact(String),
    /// Accept the grey [`completion`] into the query.
    Complete,
    /// Close the Field.
    Close,
}

/// The intent one [`FieldKey`] means over `model`, or `None` when the key
/// has nothing to act on: `↩`/`⌘↩` past the last row, `⌥↩` on an empty
/// query, `⇥` with no [`completion`].
pub fn intent_for(key: &FieldKey, model: &FieldModel) -> Option<FieldIntent> {
    match key {
        FieldKey::Enter => model.selected_row().cloned().map(FieldIntent::Open),
        FieldKey::CmdEnter => model.selected_row().cloned().map(FieldIntent::Background),
        FieldKey::AltEnter => {
            let query = model.query.trim();
            if query.is_empty() { None } else { Some(FieldIntent::SearchExact(query.to_string())) }
        }
        FieldKey::Tab => completion(&model.query, &model.rows).map(|_| FieldIntent::Complete),
        FieldKey::Escape => Some(FieldIntent::Close),
    }
}

/// Prefix (2) beats substring (1); no match is `None`.
fn score(haystacks: &[&str], needle: &str) -> Option<u8> {
    let needle = needle.to_lowercase();
    let mut best = None;
    for haystack in haystacks {
        let haystack = haystack.to_lowercase();
        let at = if haystack.starts_with(needle.as_str()) {
            Some(2)
        } else if haystack.contains(needle.as_str()) {
            Some(1)
        } else {
            None
        };
        best = best.max(at);
    }
    best
}

/// Keeps the matches in `scored` (stable: score order, source order inside a
/// score) up to `max`.
fn ranked<T>(scored: impl Iterator<Item = (Option<u8>, T)>, max: usize) -> Vec<T> {
    let mut kept: Vec<(u8, usize, T)> = Vec::new();
    for (order, (score, row)) in scored.enumerate() {
        if let Some(score) = score {
            kept.push((score, order, row));
        }
    }
    kept.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    kept.into_iter().take(max).map(|(_, _, row)| row).collect()
}

/// A tab row: the title, the faint host, `↩` to switch.
fn tab_row(tab: &FieldTab) -> FieldRow {
    FieldRow {
        kind: FieldRowKind::Tab { id: tab.id.clone() },
        title: tab.title.clone(),
        detail: Some(tab.host.clone()),
        hint: SharedString::from("↩"),
    }
}

/// A history row: the title, the faint URL, the relative read word.
fn history_row(entry: &FieldHistory) -> FieldRow {
    FieldRow {
        kind: FieldRowKind::History { url: entry.url.clone() },
        title: entry.title.clone(),
        detail: Some(entry.url.clone()),
        hint: entry.read.clone(),
    }
}

/// A command row: the name, `↩` to run it.
fn command_row(command: &FieldCommand) -> FieldRow {
    FieldRow {
        kind: FieldRowKind::Command { id: command.id.clone() },
        title: command.name.clone(),
        detail: None,
        hint: SharedString::from("↩"),
    }
}

// -------------------------------------------------------------- the element

/// `.field{width:640px;border-radius:var(--r-xl)}` on the overlay ground with
/// a 1 px hairline border and `box-shadow:var(--shadow-3)`.
const FIELD_W: f32 = 640.0;
/// `.q{gap:10px;padding:0 14px;height:46px;font-size:14px}` with a 16 px
/// search icon, like the command palette's query row.
const Q_H: f32 = 46.0;
const Q_GAP: f32 = 10.0;
const Q_PAD_X: f32 = 14.0;
const Q_TEXT: f32 = scale::FS_14;
const Q_ICON: f32 = 16.0;
/// The caret after the query: a 1 px bar at the text height.
const CARET_W: f32 = 1.0;
const CARET_H: f32 = 18.0;
/// How tall the grouped list may grow before it scrolls: twelve 34 px rows
/// plus three captions (8 px top, an 11 px caps line at 1.5 line-height,
/// 6 px bottom) plus the 4 px body bottom fit inside with room to spare:
/// 408 + 91.5 + 4 = 503.5 against the 512 cap.
const BODY_MAX_H: f32 = 512.0;
/// `.sec{padding:8px 6px 4px}` and `.sec .caps{padding:0 10px 6px}`, like the
/// command palette's sections.
const SEC_PAD_TOP: f32 = 8.0;
const SEC_PAD_X: f32 = 6.0;
const SEC_PAD_BOTTOM: f32 = 4.0;
const CAPS_PAD_X: f32 = 10.0;
const CAPS_PAD_BOTTOM: f32 = 6.0;
/// `.it{gap:10px;height:34px;padding:0 10px;border-radius:var(--r-sm);
/// font-size:13px}` with a 14 px leading glyph, like the palette's rows.
const ITEM_H: f32 = 34.0;
const ITEM_GAP: f32 = 10.0;
const ITEM_PAD_X: f32 = 10.0;
const ITEM_ICON: f32 = 14.0;
/// `.it .detail{font-size:11px}` — the faint tab host or history URL.
const DETAIL_TEXT: f32 = scale::FS_11;
/// `.ft{gap:14px;height:32px;padding:0 14px;font-size:11px}`.
const FT_H: f32 = 32.0;
const FT_GAP: f32 = 14.0;
const FT_PAD_X: f32 = 14.0;
const FT_TEXT: f32 = scale::FS_11;
/// The single space between a footer keycap and its word.
const FT_KEY_GAP: f32 = 4.0;

type ChooseHandler = Rc<dyn Fn(FieldRow, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type QueryHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type IntentHandler = Rc<dyn Fn(FieldIntent, &mut Window, &mut App)>;

/// Creates the single-line text state a live Field edits; keep it in the
/// view (or in window state) and hand its value to [`FieldModel`] every
/// frame. The same [`TextareaState`] pattern the composer builds with
/// [`crate::composer::composer_state`], pinned to one row.
pub fn field_state(
    placeholder: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut gpui::Context<TextareaState>,
) -> TextareaState {
    // Enter submits (the host confirms the selected row) and a single row
    // never inserts a newline, so the flag is exact here.
    TextareaState::new(window, cx).placeholder(placeholder).auto_grow(1, 1).submit_on_enter(true)
}

/// The Field. Build with [`field`].
#[derive(IntoElement)]
pub struct Field {
    id: ElementId,
    query: SharedString,
    placeholder: SharedString,
    rows: Vec<FieldRow>,
    selected: usize,
    complete_to: Option<String>,
    query_slot: Option<gpui::AnyElement>,
    on_choose: Option<ChooseHandler>,
    on_close: Option<CloseHandler>,
    on_query_change: Option<QueryHandler>,
    on_hover: Option<HoverHandler>,
    on_intent: Option<IntentHandler>,
}

/// The 640 px Field over `model`: the input row, then the grouped rows. The
/// host owns the query, the rows ([`rank`]) and the selection, and wires
/// [`MENU_CONTEXT`] itself — [`select_next`]/[`select_prev`] for the arrows,
/// [`Field::on_choose`] for `↩`, [`Field::on_close`] for `esc` — exactly the
/// way the command palette's hosts do.
pub fn field(id: impl Into<ElementId>, model: &FieldModel) -> Field {
    let complete_to = completion(&model.query, &model.rows);
    Field {
        id: id.into(),
        query: model.query.clone(),
        placeholder: SharedString::default(),
        rows: model.rows.clone(),
        selected: model.selected,
        complete_to,
        query_slot: None,
        on_choose: None,
        on_close: None,
        on_query_change: None,
        on_hover: None,
        on_intent: None,
    }
}

impl Field {
    /// The ink-4 text shown in the input row while the query is empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// A real editor for the input row: the caller's own field, drawn
    /// chromeless where the display query would be. The card stays one
    /// surface with the field inside it rather than a field floating above.
    pub fn query_slot(mut self, editor: impl IntoElement) -> Self {
        self.query_slot = Some(editor.into_any_element());
        self
    }

    /// A row was clicked. The argument is the row itself; the host's `↩`
    /// ([`crate::keys::Confirm`]) handler calls back with the selected row
    /// through this same handler. A thin wrapper over [`Field::on_intent`]'s
    /// [`FieldIntent::Open`]: when both are set, `on_intent` wins.
    pub fn on_choose(mut self, f: impl Fn(FieldRow, &mut Window, &mut App) + 'static) -> Self {
        self.on_choose = Some(Rc::new(f));
        self
    }

    /// The `esc` keycap was clicked. The keyboard `esc`
    /// ([`crate::keys::Cancel`]) is the host's to route here. A thin wrapper
    /// over [`Field::on_intent`]'s [`FieldIntent::Close`]: when both are set,
    /// `on_intent` wins.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }

    /// The pointer entered a row; the argument is its index, so the host can
    /// move the selection to it. The highlight follows the selection only —
    /// hovering never steals it — so a host that ignores this stays correct,
    /// and one that tracks it confirms whatever is lit.
    pub fn on_hover(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    /// Every click as its [`FieldIntent`]: a row click is
    /// [`FieldIntent::Open`], the completion tail is [`FieldIntent::Complete`],
    /// the `esc` keycap is [`FieldIntent::Close`]. The host maps its keys to
    /// intents itself with [`intent_for`]. When set, this wins over
    /// [`Field::on_choose`], [`Field::on_close`] and
    /// [`Field::on_query_change`].
    pub fn on_intent(mut self, f: impl Fn(FieldIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }

    /// The grey inline completion was clicked: the argument is the full
    /// completion text to put in the query. The `⇥` key itself is the host's
    /// (there is no `⇥` action in [`MENU_CONTEXT`]); it reports through this
    /// same handler via [`completion`]. When [`Field::on_intent`] is set, the
    /// click reports [`FieldIntent::Complete`] there instead.
    pub fn on_query_change(mut self, f: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_query_change = Some(Rc::new(f));
        self
    }
}

/// The caption a row group draws, if any: agent rows draw none.
fn group_caption(kind: &FieldRowKind) -> Option<&'static str> {
    match kind {
        FieldRowKind::Open { .. } | FieldRowKind::Clipboard { .. } | FieldRowKind::Ask | FieldRowKind::Search | FieldRowKind::Task => None,
        FieldRowKind::Tab { .. } => Some("TABS"),
        FieldRowKind::History { .. } => Some("HISTORY"),
        FieldRowKind::Command { .. } => Some("COMMANDS"),
    }
}

/// The leading glyph of a row.
fn row_icon(kind: &FieldRowKind) -> IconName {
    match kind {
        FieldRowKind::Open { .. } => IconName::Globe,
        FieldRowKind::Clipboard { .. } => IconName::Copy,
        FieldRowKind::Ask => IconName::Sparkle,
        FieldRowKind::Search => IconName::Search,
        FieldRowKind::Task => IconName::Zap,
        FieldRowKind::Tab { .. } => IconName::Globe,
        FieldRowKind::History { .. } => IconName::Clock,
        FieldRowKind::Command { .. } => IconName::Slash,
    }
}

/// What the list asked its scroll container for: the selection it scrolled
/// and whether that row has been seen in view since. Ported from the command
/// palette's scroll logic: the handle only learns the overflow from paint, so
/// a first-frame `scroll_to_item` lands before it knows it scrolls and is
/// dropped; the flag re-fires until the row arrives instead of trusting the
/// first request.
#[derive(Clone, Copy)]
struct FieldScroll {
    selected: usize,
    settled: bool,
}

/// The direct-child index of the `selected`th row inside the scrolling body:
/// agent rows draw no caption, every other group draws one caption div before
/// its first row, so the child index leads the row index by the captions
/// before it. `None` when `selected` is past the last row.
pub fn field_selected_child_index(rows: &[FieldRow], selected: usize) -> Option<usize> {
    if selected >= rows.len() {
        return None;
    }
    let mut child = 0usize;
    let mut open_group: Option<&str> = None;
    for (index, row) in rows.iter().enumerate() {
        let caption = group_caption(&row.kind);
        if caption != open_group {
            open_group = caption;
            if caption.is_some() {
                child += 1;
            }
        }
        if index == selected {
            return Some(child);
        }
        child += 1;
    }
    None
}

/// Whether the `target`th direct child is inside the scrolled viewport.
/// `false` while the container never painted (no bounds, no children yet),
/// which is exactly when the request needs (re-)firing.
pub fn field_row_visible(scroll: &ScrollHandle, target: usize) -> bool {
    if scroll.bounds_for_item(target).is_none() {
        return false;
    }
    target >= scroll.top_item() && target <= scroll.bottom_item()
}

impl RenderOnce for Field {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let len = self.rows.len();
        let selected = if len == 0 { 0 } else { self.selected.min(len - 1) };

        // One highlight: the selection. Hovering reports through `on_hover`
        // so the host can move the selection, but never lights the row
        // itself — what `↩` confirms is always what is lit.
        let mut states = Vec::with_capacity(len);
        for index in 0..len {
            let (state, _) = interaction_flags(indexed_child(&id, "row-", index), window, cx);
            states.push(state);
        }

        let mut card = v_flex()
            .flex_none()
            .relative()
            .w(px(FIELD_W))
            .max_w(gpui::relative(1.0))
            .rounded(px(scale::R_XL))
            .bg(p.overlay)
            .shadow(p.floating_shadow())
            .overflow_hidden()
            .text_color(p.ink)
            .occlude()
            .key_context(MENU_CONTEXT)
            .child(input_row(
                &id,
                &p,
                &self.query,
                &self.placeholder,
                &self.complete_to,
                self.query_slot,
                &self.on_close,
                &self.on_query_change,
                &self.on_intent,
            ));

        let scroll: ScrollHandle =
            window.use_keyed_state((id.clone(), "scroll"), cx, |_, _| ScrollHandle::new()).read(cx).clone();
        let scrolled: Rc<RefCell<FieldScroll>> = window
            .use_keyed_state((id.clone(), "scrolled"), cx, |_, _| Rc::new(RefCell::new(FieldScroll { selected: usize::MAX, settled: false })))
            .read(cx)
            .clone();
        let mut body = v_flex()
            .id((id.clone(), "body"))
            .flex_none()
            .w_full()
            .px(px(SEC_PAD_X))
            .pb(px(SEC_PAD_BOTTOM))
            .max_h(px(BODY_MAX_H))
            .overflow_y_scroll()
            .track_scroll(&scroll);

        // The arrow keys' `selected` scrolls into view and stays there: until
        // the row has been seen in view, every render re-fires the request.
        if len > 0 {
            let mut kept = scrolled.borrow_mut();
            if kept.selected != selected {
                kept.selected = selected;
                kept.settled = false;
            }
            if !kept.settled {
                match field_selected_child_index(&self.rows, selected) {
                    Some(target) if !field_row_visible(&scroll, target) => {
                        scroll.scroll_to_item(target);
                        // The re-fire above may itself be dropped (see
                        // [`FieldScroll`]); ask for the frame that retries it.
                        window.request_animation_frame();
                    }
                    _ => kept.settled = true,
                }
            }
        }

        let mut open_group: Option<&str> = None;
        for (index, row) in self.rows.iter().enumerate() {
            let caption = group_caption(&row.kind);
            if caption != open_group {
                open_group = caption;
                if let Some(caption) = caption {
                    body = body.child(
                        div()
                            .flex_none()
                            .pt(px(SEC_PAD_TOP))
                            .px(px(CAPS_PAD_X))
                            .pb(px(CAPS_PAD_BOTTOM))
                            .text_role(TextRole::Caps)
                            .line_height(gpui::relative(scale::LH_UI))
                            .text_color(p.ink_3)
                            .child(caption),
                    );
                }
            }
            body = body.child(field_row(
                indexed_child(&id, "row-", index),
                states[index].clone(),
                &p,
                row,
                index,
                index == selected,
                &self.on_choose,
                &self.on_hover,
                &self.on_intent,
                window,
                cx,
            ));
        }
        card = card.child(body).child(footer(&p));
        card
    }
}

/// The input row: the search glyph, the query with its caret and the grey
/// completion tail (or the caller's live editor), and the `esc` keycap.
#[allow(clippy::too_many_arguments)]
fn input_row(
    id: &ElementId,
    p: &aui_tokens::Palette,
    query: &SharedString,
    placeholder: &SharedString,
    complete_to: &Option<String>,
    slot: Option<gpui::AnyElement>,
    on_close: &Option<CloseHandler>,
    on_query_change: &Option<QueryHandler>,
    on_intent: &Option<IntentHandler>,
) -> impl IntoElement {
    let mut esc = div().id((id.clone(), "esc")).flex_none().ml_auto().child(kbd("esc"));
    if let Some(handler) = on_intent.clone() {
        esc = esc
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(SharedString::from("Close the field"))
            .on_click(move |_, w, cx| handler(FieldIntent::Close, w, cx));
        record_ax_label("Close the field");
    } else if let Some(handler) = on_close.clone() {
        esc = esc
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(SharedString::from("Close the field"))
            .on_click(move |_, w, cx| handler(w, cx));
        record_ax_label("Close the field");
    }
    let text: gpui::AnyElement = match slot {
        Some(editor) => div().flex_1().min_w(px(0.0)).child(editor).into_any_element(),
        None => query_text(id, p, query, placeholder, complete_to, on_query_change, on_intent).into_any_element(),
    };
    h_flex()
        .flex_none()
        .w_full()
        .h(px(Q_H))
        .gap(px(Q_GAP))
        .px(px(Q_PAD_X))
        .border_b_1()
        .border_color(p.line)
        .ui(Q_TEXT)
        .child(icon(IconName::Search).size(px(Q_ICON)).color(p.ink_3))
        .child(text)
        .child(esc)
}

/// The display query: the typed text in ink (or the placeholder in ink-4),
/// the caret, then the completion's untyped tail in ink-4. Clicking the tail
/// accepts the full completion through `on_query_change`.
fn query_text(
    id: &ElementId,
    p: &aui_tokens::Palette,
    query: &SharedString,
    placeholder: &SharedString,
    complete_to: &Option<String>,
    on_query_change: &Option<QueryHandler>,
    on_intent: &Option<IntentHandler>,
) -> impl IntoElement {
    let mut row = h_flex().flex_1().min_w(px(0.0)).overflow_hidden().items_center();
    if query.is_empty() {
        row = row.child(div().flex_none().truncate().text_color(p.ink_4).child(placeholder.clone()));
    } else {
        // The query and the caret draw as one inline run, so the caret
        // follows the text; the spacer pushes the completion tail to the
        // row's right edge, ahead of the `esc` keycap.
        row = row.child(div().flex_none().truncate().child(query.clone()));
        row = row.child(div().flex_none().w(px(CARET_W)).h(px(CARET_H)).bg(p.ink));
        row = row.child(div().flex_1());
        if let Some(full) = complete_to {
            // The candidate starts with the query (see [`completion`]), so
            // the tail is the untyped rest; an empty tail draws nothing.
            let tail = full.get(query.len()..).unwrap_or("");
            if !tail.is_empty() {
                let mut tail_el = div().id((id.clone(), "accept")).flex_none().truncate().text_color(p.ink_4).child(tail.to_string());
                if on_intent.is_some() || on_query_change.is_some() {
                    let label = SharedString::from(format!("Accept completion {full}"));
                    tail_el = tail_el.cursor_pointer().role(gpui::Role::Button).aria_label(label.clone());
                    if let Some(handler) = on_intent.clone() {
                        tail_el = tail_el.on_click(move |_, w, cx| handler(FieldIntent::Complete, w, cx));
                    } else if let Some(handler) = on_query_change.clone() {
                        let full = full.clone();
                        tail_el = tail_el.on_click(move |_, w, cx| handler(full.clone(), w, cx));
                    }
                    record_ax_label(&label);
                }
                row = row.child(tail_el);
            }
        }
    }
    row
}

/// One row: the glyph, the title, the faint detail, the right-hand hint. The
/// selected row wears `surface-3`; every row is a button labelled
/// `<title>, <hint>`.
#[allow(clippy::too_many_arguments)]
fn field_row(
    row_id: ElementId,
    state: gpui::Entity<crate::util::Interaction>,
    p: &aui_tokens::Palette,
    row: &FieldRow,
    index: usize,
    on: bool,
    on_choose: &Option<ChooseHandler>,
    on_hover: &Option<HoverHandler>,
    on_intent: &Option<IntentHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let ground = tint_fade((row_id.clone(), "bg"), on, p.surface_3, Tween::FAST, window, cx);
    let glyph_color = tween((row_id.clone(), "glyph"), if on { p.ink } else { p.ink_3 }, Tween::FAST, window, cx);
    let text_color = tween((row_id.clone(), "text"), if on { p.ink } else { p.ink_2 }, Tween::FAST, window, cx);

    let mut el = h_flex()
        .id(row_id.clone())
        .flex_none()
        .w_full()
        .h(px(ITEM_H))
        .gap(px(ITEM_GAP))
        .px(px(ITEM_PAD_X))
        .rounded(px(scale::R_SM))
        .bg(ground)
        .ui(scale::FS_13)
        .text_color(text_color)
        .child(icon(row_icon(&row.kind)).size(px(ITEM_ICON)).color(glyph_color))
        .child(div().flex_shrink(1.0).min_w(px(0.0)).truncate().child(row.title.clone()));
    if let Some(detail) = &row.detail {
        el = el.child(div().flex_shrink(1.0).min_w(px(0.0)).truncate().mono(DETAIL_TEXT).text_color(p.ink_3).child(detail.clone()));
    }
    // A history row's hint is a relative word, not a keystroke: plain faint
    // text at the right edge. Every other hint is a keycap.
    if matches!(row.kind, FieldRowKind::History { .. }) {
        el = el.child(div().flex_none().ml_auto().ui(DETAIL_TEXT).text_color(p.ink_3).child(row.hint.clone()));
    } else {
        el = el.child(div().flex_none().ml_auto().child(kbd(row.hint.clone())));
    }

    // One `on_hover` per element is all gpui allows, so the row's own hover
    // tint and the host's hover report share a single handler — the palette's
    // arrangement. The report fires on enter only, with the row index, so the
    // host can move the selection to it.
    {
        let hovered_state = state.clone();
        let handler = on_hover.clone();
        el = el.on_hover(move |hovered, w, cx| {
            hovered_state.update(cx, |s, cx| {
                if s.hovered != *hovered {
                    s.hovered = *hovered;
                    cx.notify();
                }
            });
            if *hovered {
                if let Some(handler) = &handler {
                    handler(index, w, cx);
                }
            }
        });
    }

    if on_intent.is_some() || on_choose.is_some() {
        let label = row.label();
        record_ax_label(&label);
        el = el.cursor_pointer().role(gpui::Role::Button).aria_label(label);
        if let Some(handler) = on_intent.clone() {
            let picked = row.clone();
            el = el.on_click(move |_, w, cx| handler(FieldIntent::Open(picked.clone()), w, cx));
        } else if let Some(handler) = on_choose.clone() {
            let picked = row.clone();
            el = el.on_click(move |_, w, cx| handler(picked.clone(), w, cx));
        }
    }
    el
}

/// `.ft`: the key hints, then `esc` at the right.
fn footer(p: &aui_tokens::Palette) -> impl IntoElement {
    h_flex()
        .flex_none()
        .w_full()
        .h(px(FT_H))
        .gap(px(FT_GAP))
        .px(px(FT_PAD_X))
        .border_t_1()
        .border_color(p.line)
        .ui(FT_TEXT)
        .text_color(p.ink_3)
        .child(footer_hint("↑↓", "navigate"))
        .child(open_hint())
        .child(footer_hint("⇥", "complete"))
        .child(div().flex_1())
        .child(footer_hint("esc", "close"))
}

/// The open hint: plain `↩` confirms here, `⌘↩` as background work, `⌥↩`
/// as an exact search.
fn open_hint() -> impl IntoElement {
    h_flex()
        .flex_none()
        .gap(px(FT_KEY_GAP))
        .child(kbd("↩"))
        .child("open ·")
        .child(kbd("⌘↩"))
        .child("background ·")
        .child(kbd("⌥↩"))
        .child("search")
}

/// One `<span class="kbd">…</span> word` footer hint.
fn footer_hint(key: &'static str, word: &'static str) -> impl IntoElement {
    h_flex().flex_none().gap(px(FT_KEY_GAP)).child(kbd(key)).child(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_row(kind: FieldRowKind) -> FieldRow {
        FieldRow { kind, title: SharedString::from("agent"), detail: None, hint: SharedString::from("↩") }
    }

    fn tab_row_at(id: usize) -> FieldRow {
        FieldRow {
            kind: FieldRowKind::Tab { id: SharedString::from(format!("tab-{id}")) },
            title: SharedString::from(format!("tab {id}")),
            detail: None,
            hint: SharedString::from("↩"),
        }
    }

    /// Three agent rows, then two tabs under their caption: children are the
    /// three rows, the caption, then the two tab rows.
    #[test]
    fn selected_child_index_counts_captions_but_not_agent_rows() {
        let rows = vec![
            agent_row(FieldRowKind::Ask),
            agent_row(FieldRowKind::Search),
            agent_row(FieldRowKind::Task),
            tab_row_at(0),
            tab_row_at(1),
        ];
        assert_eq!(field_selected_child_index(&rows, 0), Some(0));
        assert_eq!(field_selected_child_index(&rows, 2), Some(2));
        assert_eq!(field_selected_child_index(&rows, 3), Some(4));
        assert_eq!(field_selected_child_index(&rows, 4), Some(5));
        assert_eq!(field_selected_child_index(&rows, 5), None);
    }

    /// A full three-group result: agent rows draw no caption, each of tabs,
    /// history and commands draws one.
    #[test]
    fn selected_child_index_counts_one_caption_per_group() {
        let rows = vec![
            agent_row(FieldRowKind::Ask),
            agent_row(FieldRowKind::Search),
            agent_row(FieldRowKind::Task),
            tab_row_at(0),
            FieldRow {
                kind: FieldRowKind::History { url: SharedString::from("example.com/a") },
                title: SharedString::from("a"),
                detail: None,
                hint: SharedString::from("read today"),
            },
            FieldRow {
                kind: FieldRowKind::Command { id: SharedString::from("split") },
                title: SharedString::from("Split View"),
                detail: None,
                hint: SharedString::from("↩"),
            },
        ];
        // Children: 3 agent rows, TABS caption, tab, HISTORY caption, history,
        // COMMANDS caption, command.
        assert_eq!(field_selected_child_index(&rows, 3), Some(4));
        assert_eq!(field_selected_child_index(&rows, 4), Some(6));
        assert_eq!(field_selected_child_index(&rows, 5), Some(8));
    }

    #[test]
    fn selected_child_index_without_rows_is_none() {
        let rows: Vec<FieldRow> = Vec::new();
        assert_eq!(field_selected_child_index(&rows, 0), None);
    }
}
