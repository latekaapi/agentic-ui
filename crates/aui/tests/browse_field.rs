//! P2: the Field's ranking, completion, labels and keyboard wrap.
//!
//! Pure table tests cover [`rank`] (agent rows first, tabs before history,
//! the caps, the empty-query recents) and [`completion`]; gpui tests draw
//! the element once and assert every row's `<title>, <hint>` label, then
//! drive the arrows/return/escape through [`MENU_CONTEXT`] the way
//! `keyboard.rs` does.

use std::cell::RefCell;
use std::rc::Rc;

use aui::browse::field::{
    completion, field, field_row_visible, field_selected_child_index, intent_for, looks_like_url, rank, select_next, select_prev,
    FieldCommand, FieldHistory, FieldIntent, FieldKey, FieldModel, FieldRow, FieldRowKind, FieldSources, FieldTab,
};
use aui::keys::{Cancel, Confirm, SelectNext, SelectPrev, MENU_CONTEXT};
use aui::transcript::{arm_ax_probe, take_ax_labels};
use gpui::{div, point, prelude::*, px, App, Context, FocusHandle, IntoElement, Modifiers, ScrollHandle, SharedString, TestAppContext, Window};

// ------------------------------------------------------------------ fixtures

fn sources() -> FieldSources {
    let mut out = FieldSources::new("Claude", "Google");
    out.tabs = vec![
        FieldTab { id: "tab-nas".into(), title: "NAS 2024 · state report cards".into(), host: "ncert.nic.in".into() },
        FieldTab { id: "tab-udise".into(), title: "UDISE+ 2024-25 · dashboard".into(), host: "udiseplus.gov.in".into() },
        FieldTab { id: "tab-mail".into(), title: "Inbox".into(), host: "mail.example.com".into() },
    ];
    out.history = vec![
        FieldHistory { title: "NAS 2024 · Karnataka report card".into(), url: "ncert.nic.in/nas".into(), read: "read Tuesday".into() },
        FieldHistory { title: "District report cards 2024-25".into(), url: "udiseplus.gov.in/reports".into(), read: "read Monday".into() },
    ];
    out.commands = vec![
        FieldCommand { id: "split".into(), name: "Split View".into() },
        FieldCommand { id: "reading".into(), name: "Reading mode".into() },
        FieldCommand { id: "space".into(), name: "New Space…".into() },
    ];
    out
}

fn kinds(rows: &[FieldRow]) -> Vec<String> {
    rows.iter()
        .map(|row| match &row.kind {
            FieldRowKind::Open { .. } => "open".to_string(),
            FieldRowKind::Clipboard { .. } => "clipboard".to_string(),
            FieldRowKind::Ask => "ask".to_string(),
            FieldRowKind::Search => "search".to_string(),
            FieldRowKind::Task => "task".to_string(),
            FieldRowKind::Tab { id } => format!("tab:{id}"),
            FieldRowKind::History { url } => format!("history:{url}"),
            FieldRowKind::Command { id } => format!("command:{id}"),
        })
        .collect()
}

// ---------------------------------------------------------------------- rank

#[test]
fn plain_word_query_leads_with_ask_and_orders_tabs_first() {
    // A history entry on a host with no open tab, so the duplicate rule keeps
    // it beside the matching tab.
    let mut with_note = sources();
    with_note.history.push(FieldHistory {
        title: "NAS 2024 · sampling note".into(),
        url: "notes.example.com/nas-sampling".into(),
        read: "read Sunday".into(),
    });
    let rows = rank("nas", &with_note);
    assert!(!looks_like_url("nas"));
    let kinds = kinds(&rows);
    assert_eq!(&kinds[..3], ["ask", "search", "task"]);
    // Tabs first: every tab row precedes every history row.
    let last_tab = kinds.iter().rposition(|k| k.starts_with("tab:")).expect("a tab matches nas");
    let first_history = kinds.iter().position(|k| k.starts_with("history:")).expect("history matches nas");
    assert!(last_tab < first_history, "tabs before history: {kinds:?}");
    // Only the NAS tab matches; the dashboard and the inbox do not.
    assert!(kinds.contains(&"tab:tab-nas".to_string()));
    assert!(!kinds.iter().any(|k| k == "tab:tab-udise" || k == "tab:tab-mail"));
}

#[test]
fn url_query_opens_first_and_ask_moves_second() {
    let query = "udiseplus.gov.in/reports";
    assert!(looks_like_url(query));
    let rows = rank(query, &sources());
    let kinds = kinds(&rows);
    assert_eq!(&kinds[..2], ["open", "ask"]);
    assert_eq!(rows[0].title, SharedString::from(format!("Open {query}")));
}

#[test]
fn scheme_urls_count_as_urls() {
    assert!(looks_like_url("https://ncert.nic.in/nas"));
    assert!(!looks_like_url("that NAS report card I read last week"));
    assert!(!looks_like_url(""));
}

#[test]
fn looks_like_url_tells_hosts_from_filenames() {
    for host in ["ncert.nic.in", "localhost:5173", "https://x.y", "example.com", "about:blank"] {
        assert!(looks_like_url(host), "{host} reads as a URL");
    }
    for not_url in ["3.14", "report.pdf", "nas", "localhost:abc"] {
        assert!(!looks_like_url(not_url), "{not_url} reads as a search");
    }
}

#[test]
fn empty_query_shows_recents_only() {
    let mut full = sources();
    full.tabs.push(FieldTab { id: "tab-4".into(), title: "Fourth".into(), host: "fourth.example.com".into() });
    full.tabs.push(FieldTab { id: "tab-5".into(), title: "Fifth".into(), host: "fifth.example.com".into() });
    // A frequent site on a host with no open tab: the recents keep it while
    // the two entries below duplicate open tabs and stay out.
    full.history.push(FieldHistory {
        title: "NAS 2024 · sampling note".into(),
        url: "notes.example.com/nas-sampling".into(),
        read: "read Sunday".into(),
    });
    full.clipboard = Some("ncert.nic.in/nas/karnataka".into());
    let rows = rank("", &full);
    let kinds = kinds(&rows);
    // Recent tabs (3), the one frequent site whose host is not open, the
    // Clipboard row — and no agent or command rows.
    assert_eq!(kinds.len(), 3 + 1 + 1, "recents only: {kinds:?}");
    assert_eq!(&kinds[..3], ["tab:tab-nas", "tab:tab-udise", "tab:tab-mail"]);
    assert!(kinds.iter().all(|k| !matches!(k.as_str(), "ask" | "search" | "task") && !k.starts_with("command:")));
    assert!(kinds.iter().any(|k| k == "clipboard"));
    let clipboard = rows.iter().find(|row| matches!(row.kind, FieldRowKind::Clipboard { .. })).expect("the Clipboard row");
    assert!(clipboard.title.to_string().starts_with("Clipboard"), "names the clipboard: {}", clipboard.title);
    assert!(
        matches!(&clipboard.kind, FieldRowKind::Clipboard { url } if url == "ncert.nic.in/nas/karnataka"),
        "the Clipboard row carries the URL: {:?}",
        clipboard.kind
    );
}

#[test]
fn empty_query_without_clipboard_has_no_open_row() {
    let rows = rank("  ", &sources());
    assert!(
        rows.iter().all(|row| !matches!(row.kind, FieldRowKind::Open { .. } | FieldRowKind::Clipboard { .. })),
        "no clipboard, no Open row"
    );
}

#[test]
fn caps_hold_with_many_matches() {
    let mut many = FieldSources::new("Claude", "Google");
    for i in 0..9 {
        many.tabs.push(FieldTab {
            id: format!("tab-{i}").into(),
            title: format!("NAS archive {i}").into(),
            host: format!("nas-{i}.example.com").into(),
        });
        many.history.push(FieldHistory {
            title: format!("NAS notes {i}").into(),
            // A host with no open tab, so the duplicate rule keeps the entry
            // and the test measures the history cap.
            url: format!("note-{i}.example.com/notes").into(),
            read: "read today".into(),
        });
        many.commands.push(FieldCommand { id: format!("cmd-{i}").into(), name: format!("NAS command {i}").into() });
    }
    let rows = rank("nas", &many);
    let kinds = kinds(&rows);
    assert_eq!(kinds.iter().filter(|k| k.starts_with("tab:")).count(), 5);
    assert_eq!(kinds.iter().filter(|k| k.starts_with("history:")).count(), 4);
    assert_eq!(kinds.iter().filter(|k| k.starts_with("command:")).count(), 4);
}

#[test]
fn history_never_duplicates_an_open_tab_host() {
    // Both frequent sites duplicate an open tab's host; the sampling note
    // does not.
    let mut duplicate = sources();
    duplicate.history.push(FieldHistory {
        title: "NAS 2024 · sampling note".into(),
        url: "notes.example.com/nas-sampling".into(),
        read: "read Sunday".into(),
    });
    let rows = rank("nas", &duplicate);
    let kinds = kinds(&rows);
    assert!(!kinds.iter().any(|k| k == "history:ncert.nic.in/nas"), "open tab covers it: {kinds:?}");
    assert!(kinds.iter().any(|k| k == "history:notes.example.com/nas-sampling"), "kept: {kinds:?}");
}

#[test]
fn history_never_duplicates_the_open_row() {
    let query = "udiseplus.gov.in/reports";
    let rows = rank(query, &sources());
    let kinds = kinds(&rows);
    assert_eq!(&kinds[..2], ["open", "ask"]);
    assert!(!kinds.iter().any(|k| k == "history:udiseplus.gov.in/reports"), "the Open row says it: {kinds:?}");
}

#[test]
fn prefix_matches_sort_before_substrings() {
    let mut scored = FieldSources::new("Claude", "Google");
    scored.tabs = vec![
        FieldTab { id: "mid".into(), title: "The final NAS results".into(), host: "example.com".into() },
        FieldTab { id: "pre".into(), title: "NAS 2024 · state report cards".into(), host: "ncert.nic.in".into() },
    ];
    let rows = rank("nas", &scored);
    let tabs: Vec<_> = rows.iter().filter(|row| matches!(row.kind, FieldRowKind::Tab { .. })).collect();
    assert_eq!(tabs.len(), 2);
    assert!(matches!(&tabs[0].kind, FieldRowKind::Tab { id } if id.as_ref() == "pre"), "prefix first");
}

// ---------------------------------------------------------------- completion

#[test]
fn completion_is_the_first_prefix_url() {
    // No open tab on that host, so the history entry survives to complete.
    let mut unopened = sources();
    unopened.tabs.retain(|tab| tab.id.as_ref() == "tab-mail");
    let rows = rank("udiseplus.gov.in", &unopened);
    assert_eq!(completion("udiseplus.gov.in", &rows), Some("udiseplus.gov.in/reports".to_string()));
}

#[test]
fn completion_matches_case_insensitively_but_keeps_the_url() {
    let rows = rank("NCERT", &sources());
    // Tabs sort before history, so the tab host wins over the history URL.
    assert_eq!(completion("NCERT", &rows), Some("ncert.nic.in".to_string()));
    // Without the NAS tab the history URL survives the duplicate rule and
    // completes, keeping its own case.
    let mut unopened = sources();
    unopened.tabs.retain(|tab| tab.id.as_ref() != "tab-nas");
    let rows = rank("NCERT", &unopened);
    let history_only: Vec<_> = rows.iter().filter(|row| matches!(row.kind, FieldRowKind::History { .. })).cloned().collect();
    assert_eq!(completion("NCERT", &history_only), Some("ncert.nic.in/nas".to_string()));
}

#[test]
fn completion_is_none_without_a_prefix_or_query() {
    let rows = rank("nas", &sources());
    assert_eq!(completion("report card week", &rows), None);
    assert_eq!(completion("", &rows), None);
    assert_eq!(completion("nas", &[]), None);
}

#[test]
fn completion_rejects_queries_with_surrounding_whitespace() {
    let rows = rank("ncert", &sources());
    assert_eq!(completion("ncert", &rows), Some("ncert.nic.in".to_string()));
    // The tail is sliced at the query length, so an untrimmed query must not
    // complete at all.
    assert_eq!(completion("ncert ", &rows), None);
    assert_eq!(completion(" ncert", &rows), None);
}

#[test]
fn completion_folds_case_like_rank() {
    let rows = rank("NCERT", &sources());
    assert_eq!(completion("NCERT", &rows), Some("ncert.nic.in".to_string()));
}

// ------------------------------------------------------------------- intents

fn intent_model() -> FieldModel {
    FieldModel::new("ncert", rank("ncert", &sources()))
}

#[test]
fn every_key_maps_to_its_intent() {
    let model = intent_model();
    let selected = model.selected_row().cloned().expect("a selected row");
    assert_eq!(intent_for(&FieldKey::Enter, &model), Some(FieldIntent::Open(selected.clone())));
    assert_eq!(intent_for(&FieldKey::CmdEnter, &model), Some(FieldIntent::Background(selected)));
    assert_eq!(intent_for(&FieldKey::AltEnter, &model), Some(FieldIntent::SearchExact("ncert".to_string())));
    assert_eq!(intent_for(&FieldKey::Escape, &model), Some(FieldIntent::Close));
    // "ncert" completes against the NAS tab host, so ⇥ accepts it.
    assert_eq!(intent_for(&FieldKey::Tab, &model), Some(FieldIntent::Complete));
}

#[test]
fn intent_is_none_when_the_key_has_nothing_to_act_on() {
    // ⇥ with no completion: nothing matches this query.
    let model = FieldModel::new("report card week", rank("report card week", &sources()));
    assert_eq!(completion("report card week", &model.rows), None);
    assert_eq!(intent_for(&FieldKey::Tab, &model), None);
    // ↩/⌘↩ past the last row, ⌥↩ on an empty query.
    let empty = FieldModel::new("", Vec::new());
    assert_eq!(intent_for(&FieldKey::Enter, &empty), None);
    assert_eq!(intent_for(&FieldKey::CmdEnter, &empty), None);
    assert_eq!(intent_for(&FieldKey::AltEnter, &empty), None);
    // `esc` always closes.
    assert_eq!(intent_for(&FieldKey::Escape, &empty), Some(FieldIntent::Close));
}

#[test]
fn select_helpers_wrap() {
    assert_eq!(select_next(2, 3), 0);
    assert_eq!(select_next(0, 3), 1);
    assert_eq!(select_prev(0, 3), 2);
    assert_eq!(select_prev(1, 3), 0);
    assert_eq!(select_next(0, 0), 0);
    assert_eq!(select_prev(0, 0), 0);
    // A stale selection recovers instead of pointing past the end.
    assert_eq!(select_next(9, 3), 0);
    assert_eq!(select_prev(9, 3), 2);
}

// ------------------------------------------------------------- labels & keys

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
}

/// The host view: the Field is rendered by a view, as an app renders it.
struct LabelHost {
    model: FieldModel,
}

impl gpui::Render for LabelHost {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let _ = cx;
        field("labels", &self.model).on_choose(|_, _, _| {}).on_close(|_, _| {}).on_query_change(|_, _, _| {})
    }
}

/// Draws `model` once in a real window and returns the effective AX labels
/// the render committed.
fn drawn_labels(cx: &mut TestAppContext, model: FieldModel) -> Vec<String> {
    arm_ax_probe(true);
    take_ax_labels();
    let (_host, _) = cx.add_window_view(|_, _| LabelHost { model });
    let labels = take_ax_labels();
    arm_ax_probe(false);
    labels
}

#[gpui::test]
fn every_field_row_carries_its_title_hint_label(cx: &mut TestAppContext) {
    init(cx);
    // One row per group, hand-built: the element takes any model, and the
    // rank tests above cover which rows a query produces.
    let model = FieldModel::new(
        "nas",
        vec![
            FieldRow {
                kind: FieldRowKind::Ask,
                title: "Ask Claude — answers here".into(),
                detail: None,
                hint: "↩".into(),
            },
            FieldRow {
                kind: FieldRowKind::Search,
                title: "Search Google".into(),
                detail: None,
                hint: "⌥↩".into(),
            },
            FieldRow {
                kind: FieldRowKind::Task,
                title: "Task for Claude — in its own tabs".into(),
                detail: None,
                hint: "⌘↩".into(),
            },
            FieldRow {
                kind: FieldRowKind::Tab { id: "tab-nas".into() },
                title: "NAS 2024 · state report cards".into(),
                detail: Some("ncert.nic.in".into()),
                hint: "↩".into(),
            },
            FieldRow {
                kind: FieldRowKind::History { url: "ncert.nic.in/nas".into() },
                title: "NAS 2024 · Karnataka report card".into(),
                detail: Some("ncert.nic.in/nas".into()),
                hint: "read Tuesday".into(),
            },
            FieldRow {
                kind: FieldRowKind::Command { id: "split".into() },
                title: "Split View".into(),
                detail: None,
                hint: "↩".into(),
            },
        ],
    );
    let labels = drawn_labels(cx, model);
    assert!(!labels.is_empty(), "the field draws controls at all");
    assert!(!labels.iter().any(|label| label.is_empty()), "no unlabelled control: {labels:?}");
    for want in [
        "Ask Claude — answers here, ↩",
        "Search Google, ⌥↩",
        "Task for Claude — in its own tabs, ⌘↩",
        "NAS 2024 · state report cards, ↩",
        "NAS 2024 · Karnataka report card, read Tuesday",
        "Split View, ↩",
    ] {
        assert!(labels.iter().any(|label| label == want), "missing label {want:?}; got {labels:?}");
    }
}

/// What the Field host recorded, in order.
type Log = Rc<RefCell<Vec<String>>>;

struct KeyHost {
    focus: FocusHandle,
    model: FieldModel,
    log: Log,
}

impl KeyHost {
    fn choose_selected(&mut self, cx: &mut App) {
        if let Some(row) = self.model.selected_row() {
            self.log.borrow_mut().push(format!("choose:{}", row.title));
        }
        let _ = cx;
    }
}

impl gpui::Render for KeyHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choose_log = self.log.clone();
        div()
            .key_context(MENU_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                this.model.selected = select_next(this.model.selected, this.model.rows.len());
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| {
                this.model.selected = select_prev(this.model.selected, this.model.rows.len());
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Confirm, _, cx| this.choose_selected(cx)))
            .on_action(cx.listener(|this, _: &Cancel, _, cx| {
                this.log.borrow_mut().push("close".into());
                let _ = cx;
            }))
            .child(
                field("keys", &self.model)
                    .on_choose(move |row, _, _| choose_log.borrow_mut().push(format!("click:{}", row.title))),
            )
            .into_any_element()
    }
}

#[gpui::test]
fn field_down_wraps_past_the_last_row(cx: &mut TestAppContext) {
    init(cx);
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (host, cx) = cx.add_window_view({
        let log = log.clone();
        move |window, cx: &mut Context<KeyHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            let mut model = FieldModel::new("nas", rank("nas", &sources()));
            model.selected = model.rows.len() - 1;
            KeyHost { focus, model, log }
        }
    });
    cx.simulate_keystrokes("down");
    assert_eq!(host.read_with(cx, |host, _| host.model.selected), 0, "down wraps to the first row");
    cx.simulate_keystrokes("down");
    assert_eq!(host.read_with(cx, |host, _| host.model.selected), 1);
    assert!(log.borrow().is_empty(), "moving never chooses");
}

#[gpui::test]
fn field_up_wraps_past_the_first_row(cx: &mut TestAppContext) {
    init(cx);
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (host, cx) = cx.add_window_view({
        let log = log.clone();
        move |window, cx: &mut Context<KeyHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            KeyHost { focus, model: FieldModel::new("nas", rank("nas", &sources())), log }
        }
    });
    cx.simulate_keystrokes("up");
    let (selected, last) = host.read_with(cx, |host, _| (host.model.selected, host.model.rows.len() - 1));
    assert_eq!(selected, last, "up wraps to the last row");
    assert!(log.borrow().is_empty(), "moving never chooses");
}

/// The host for the hover test: hovering a row moves the selection, `enter`
/// confirms whatever is lit.
struct HoverHost {
    focus: FocusHandle,
    state: Rc<RefCell<FieldModel>>,
    log: Log,
}

impl gpui::Render for HoverHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        div()
            .key_context(MENU_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Confirm, _, cx| {
                if let Some(row) = this.state.borrow().selected_row() {
                    this.log.borrow_mut().push(format!("choose:{}", row.title));
                }
                let _ = cx;
            }))
            .child(field("hover", &self.state.borrow()).on_hover(move |index, _, _| {
                state.borrow_mut().selected = index;
            }))
            .into_any_element()
    }
}

#[gpui::test]
fn hovering_a_row_moves_the_selection_enter_confirms_it(cx: &mut TestAppContext) {
    init(cx);
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    // Ask, Search, Task, then the NAS tab: row 2 is the Task row.
    let state = Rc::new(RefCell::new(FieldModel::new("nas", rank("nas", &sources()))));
    assert_eq!(state.borrow().rows.len(), 4);
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        let state = state.clone();
        move |window, cx: &mut Context<HoverHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            HoverHost { focus, state, log }
        }
    });
    // Row 2 spans y 114..148 (the 46 px query row, then two 34 px rows).
    cx.simulate_mouse_move(point(px(320.0), px(131.0)), None, Modifiers::default());
    assert_eq!(state.borrow().selected, 2, "hovering row 2 selects it");
    cx.simulate_keystrokes("enter");
    assert_eq!(&*log.borrow(), &["choose:Task for Claude — in its own tabs".to_string()]);
}

/// The host for the scroll test: a full three-group result driven by the
/// real arrow actions.
struct ScrollHost {
    focus: FocusHandle,
    model: FieldModel,
}

impl gpui::Render for ScrollHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(MENU_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                this.model.selected = select_next(this.model.selected, this.model.rows.len());
                cx.notify();
            }))
            .child(field("scroll-field", &self.model).on_choose(|_, _, _| {}))
            .into_any_element()
    }
}

/// Sixteen rows at the caps: 3 agent + 5 tabs + 4 history + 4 commands, with
/// tab and history hosts kept apart so the duplicate rule keeps them all.
fn sixteen_rows() -> Vec<FieldRow> {
    let mut many = FieldSources::new("Claude", "Google");
    for i in 0..9 {
        many.tabs.push(FieldTab {
            id: format!("tab-{i}").into(),
            title: format!("NAS archive {i}").into(),
            host: format!("tab-{i}.example.com").into(),
        });
        many.history.push(FieldHistory {
            title: format!("NAS notes {i}").into(),
            url: format!("note-{i}.example.com/notes").into(),
            read: "read today".into(),
        });
        many.commands.push(FieldCommand { id: format!("cmd-{i}").into(), name: format!("NAS command {i}").into() });
    }
    let rows = rank("nas", &many);
    assert_eq!(rows.len(), 3 + 5 + 4 + 4, "full caps: {:?}", kinds(&rows));
    rows
}

#[gpui::test]
fn selected_row_below_the_fold_scrolls_into_view(cx: &mut TestAppContext) {
    init(cx);
    let (host, cx) = cx.add_window_view(|window, cx: &mut Context<ScrollHost>| {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        ScrollHost { focus, model: FieldModel::new("nas", sixteen_rows()) }
    });
    for _ in 0..12 {
        cx.simulate_keystrokes("down");
    }
    assert_eq!(host.read_with(cx, |host, _| host.model.selected), 12, "twelve downs land on row 12");
    let rows = host.read_with(cx, |host, _| host.model.rows.clone());
    // Row 12 is the first command row: twelve rows plus the TABS, HISTORY
    // and COMMANDS captions precede it in the scrolling body.
    let target = field_selected_child_index(&rows, 12).expect("the selected row maps to a child");
    assert_eq!(target, 15);
    // A handle that never painted reports nothing visible — the exact case
    // the element's re-fire loop is written for.
    assert!(!field_row_visible(&ScrollHandle::new(), target));
    // Pump frames so the ported scroll-into-view runs: the first request
    // lands before paint knows the overflow, the re-fire completes it, and
    // the element asks for no further frames once the row is seen in view.
    for _ in 0..30 {
        host.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        cx.update(|window, app| {
            window.simulate_next_frame(app);
        });
        cx.executor().advance_clock(std::time::Duration::from_millis(16));
    }
    let mut rest = 0;
    for _ in 0..10 {
        host.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        rest += cx.update(|window, app| window.simulate_next_frame(app));
        cx.executor().advance_clock(std::time::Duration::from_millis(16));
    }
    assert_eq!(rest, 0, "the list settles once the selected row is visible");
}

#[gpui::test]
fn field_enter_confirms_and_escape_closes(cx: &mut TestAppContext) {
    init(cx);
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (host, cx) = cx.add_window_view({
        let log = log.clone();
        move |window, cx: &mut Context<KeyHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            KeyHost { focus, model: FieldModel::new("nas", rank("nas", &sources())), log }
        }
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(&*log.borrow(), &["choose:Ask Claude — answers here".to_string()]);
    cx.simulate_keystrokes("escape");
    assert_eq!(&*log.borrow(), &["choose:Ask Claude — answers here".to_string(), "close".to_string()]);
    let _ = host;
}
