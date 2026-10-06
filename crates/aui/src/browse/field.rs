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

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
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
    /// URL or host; Ask moves second.
    Open,
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

/// Whether the query looks like a URL or bare host: no spaces, and either an
/// `http(s)` scheme or a dotted host (`udiseplus.gov.in/…`). While it does,
/// the first row opens it and Ask moves second.
pub fn looks_like_url(query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() || query.contains(char::is_whitespace) {
        return false;
    }
    let lower = query.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return true;
    }
    let host = query.split('/').next().unwrap_or("");
    host.contains('.') && !host.starts_with('.') && !host.ends_with('.') && host.len() > 3
}

/// Builds the rows for `query`, in display order: the agent rows (no
/// caption), then matching tabs, history and commands. Matching is a
/// case-insensitive substring with prefix matches first; tabs sort before
/// history regardless of score. An empty query shows nothing but the recent
/// tabs, the frequent sites and the Clipboard row.
pub fn rank(query: &str, sources: &FieldSources) -> Vec<FieldRow> {
    let query = query.trim();
    if query.is_empty() {
        let mut rows = Vec::new();
        rows.extend(sources.tabs.iter().take(EMPTY_EACH).map(tab_row));
        rows.extend(sources.history.iter().take(EMPTY_EACH).map(history_row));
        if let Some(url) = &sources.clipboard {
            rows.push(FieldRow {
                kind: FieldRowKind::Open,
                title: SharedString::from(format!("Clipboard — {url}")),
                detail: None,
                hint: SharedString::from("↩"),
            });
        }
        return rows;
    }
    let mut rows = Vec::with_capacity(3 + MAX_TABS + MAX_HISTORY + MAX_COMMANDS);
    if looks_like_url(query) {
        rows.push(FieldRow {
            kind: FieldRowKind::Open,
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
        sources.history.iter().map(|entry| (score(&[&entry.title, &entry.url], query), history_row(entry))),
        MAX_HISTORY,
    ));
    rows.extend(ranked(sources.commands.iter().map(|command| (score(&[&command.name], query), command_row(command))), MAX_COMMANDS));
    rows
}

/// The grey inline completion: the first tab host or history URL that starts
/// with the query. The element draws the untyped tail grey after the caret;
/// `⇥` (or a click on it) accepts it. `None` for an empty query or when
/// nothing completes it.
pub fn completion(query: &str, rows: &[FieldRow]) -> Option<String> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    for row in rows {
        let candidate: &str = match &row.kind {
            FieldRowKind::Tab { .. } => match &row.detail {
                Some(host) => host,
                None => continue,
            },
            FieldRowKind::History { url } => url,
            _ => continue,
        };
        if candidate.len() > query.len()
            && candidate.get(..query.len()).is_some_and(|prefix| prefix.eq_ignore_ascii_case(query))
        {
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
/// How tall the grouped list may grow before it scrolls.
const BODY_MAX_H: f32 = 320.0;
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
    /// through this same handler.
    pub fn on_choose(mut self, f: impl Fn(FieldRow, &mut Window, &mut App) + 'static) -> Self {
        self.on_choose = Some(Rc::new(f));
        self
    }

    /// The `esc` keycap was clicked. The keyboard `esc`
    /// ([`crate::keys::Cancel`]) is the host's to route here.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }

    /// The grey inline completion was clicked: the argument is the full
    /// completion text to put in the query. The `⇥` key itself is the host's
    /// (there is no `⇥` action in [`MENU_CONTEXT`]); it reports through this
    /// same handler via [`completion`].
    pub fn on_query_change(mut self, f: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_query_change = Some(Rc::new(f));
        self
    }
}

/// The caption a row group draws, if any: agent rows draw none.
fn group_caption(kind: &FieldRowKind) -> Option<&'static str> {
    match kind {
        FieldRowKind::Open | FieldRowKind::Ask | FieldRowKind::Search | FieldRowKind::Task => None,
        FieldRowKind::Tab { .. } => Some("TABS"),
        FieldRowKind::History { .. } => Some("HISTORY"),
        FieldRowKind::Command { .. } => Some("COMMANDS"),
    }
}

/// The leading glyph of a row.
fn row_icon(kind: &FieldRowKind) -> IconName {
    match kind {
        FieldRowKind::Open => IconName::Globe,
        FieldRowKind::Ask => IconName::Sparkle,
        FieldRowKind::Search => IconName::Search,
        FieldRowKind::Task => IconName::Zap,
        FieldRowKind::Tab { .. } => IconName::Globe,
        FieldRowKind::History { .. } => IconName::Clock,
        FieldRowKind::Command { .. } => IconName::Slash,
    }
}

impl RenderOnce for Field {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let len = self.rows.len();
        let selected = if len == 0 { 0 } else { self.selected.min(len - 1) };

        // One highlight, not one per row: while the pointer is over a row it
        // owns the highlight, and the keyboard's `selected` takes it back the
        // moment the pointer leaves — the command palette's rule.
        let mut hovered_row = None;
        let mut states = Vec::with_capacity(len);
        for index in 0..len {
            let (state, flags) = interaction_flags(indexed_child(&id, "row-", index), window, cx);
            if flags.hovered {
                hovered_row = Some(index);
            }
            states.push(state);
        }
        let active = hovered_row.unwrap_or(selected);

        let mut card = v_flex()
            .flex_none()
            .relative()
            .w(px(FIELD_W))
            .rounded(px(scale::R_XL))
            .border_1()
            .border_color(p.line)
            .bg(p.overlay)
            .shadow(p.shadow(3))
            .overflow_hidden()
            .text_color(p.ink)
            .occlude()
            .key_context(MENU_CONTEXT)
            .child(input_row(&id, &p, &self.query, &self.placeholder, &self.complete_to, self.query_slot, &self.on_close, &self.on_query_change));

        let mut body = v_flex()
            .id((id.clone(), "body"))
            .flex_none()
            .w_full()
            .px(px(SEC_PAD_X))
            .pb(px(SEC_PAD_BOTTOM))
            .max_h(px(BODY_MAX_H))
            .overflow_y_scroll();
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
                index == active,
                &self.on_choose,
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
) -> impl IntoElement {
    let mut esc = div().id((id.clone(), "esc")).flex_none().ml_auto().child(kbd("esc"));
    if let Some(handler) = on_close.clone() {
        esc = esc
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(SharedString::from("Close the field"))
            .on_click(move |_, w, cx| handler(w, cx));
        record_ax_label("Close the field");
    }
    let text: gpui::AnyElement = match slot {
        Some(editor) => div().flex_1().min_w(px(0.0)).child(editor).into_any_element(),
        None => query_text(id, p, query, placeholder, complete_to, on_query_change).into_any_element(),
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
) -> impl IntoElement {
    let mut row = h_flex().flex_1().min_w(px(0.0)).overflow_hidden().items_center();
    if query.is_empty() {
        row = row.child(div().flex_none().truncate().text_color(p.ink_4).child(placeholder.clone()));
    } else {
        row = row.child(div().flex_none().child(query.clone()));
        if let Some(full) = complete_to {
            // The candidate starts with the query (see [`completion`]), so
            // the tail is the untyped rest; an empty tail draws nothing.
            let tail = full.get(query.len()..).unwrap_or("");
            if !tail.is_empty() {
                let mut tail_el = div().id((id.clone(), "accept")).flex_none().truncate().text_color(p.ink_4).child(tail.to_string());
                if let Some(handler) = on_query_change.clone() {
                    let full = full.clone();
                    let label = SharedString::from(format!("Accept completion {full}"));
                    tail_el = tail_el
                        .cursor_pointer()
                        .role(gpui::Role::Button)
                        .aria_label(label.clone())
                        .on_click(move |_, w, cx| handler(full.clone(), w, cx));
                    record_ax_label(&label);
                }
                row = row.child(tail_el);
            }
        }
        row = row.child(div().flex_none().w(px(CARET_W)).h(px(CARET_H)).bg(p.ink));
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
    on: bool,
    on_choose: &Option<ChooseHandler>,
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

    {
        let hovered_state = state.clone();
        el = el.on_hover(move |hovered, _, cx| {
            hovered_state.update(cx, |s, cx| {
                if s.hovered != *hovered {
                    s.hovered = *hovered;
                    cx.notify();
                }
            })
        });
    }

    if let Some(handler) = on_choose.clone() {
        let label = row.label();
        let picked = row.clone();
        record_ax_label(&label);
        el = el.cursor_pointer().role(gpui::Role::Button).aria_label(label).on_click(move |_, w, cx| handler(picked.clone(), w, cx));
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
        .child(footer_hint("↩", "open"))
        .child(footer_hint("⌘↩", "task"))
        .child(footer_hint("⇥", "complete"))
        .child(div().flex_1())
        .child(footer_hint("esc", "close"))
}

/// One `<span class="kbd">…</span> word` footer hint.
fn footer_hint(key: &'static str, word: &'static str) -> impl IntoElement {
    h_flex().flex_none().gap(px(FT_KEY_GAP)).child(kbd(key)).child(word)
}
