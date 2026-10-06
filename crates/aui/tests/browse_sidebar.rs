//! P1: every Browse sidebar row renders with its accessible name, and the
//! pure helpers map correctly.
//!
//! Draws the sidebar's rows — the address line, pin grid, tab / folder /
//! task-folder rows, footer and nav cluster — with every handler wired, then
//! asserts the AX probe recorded no unlabelled control and the expected
//! names are present. `split_address` and the face → status-word mapping
//! are plain unit tests.

use std::rc::Rc;

use aui::browse::{
    address_line, browse_footer, browse_nav_cluster, folder_row, pin_grid, split_address, tab_row, task_face,
    task_folder_row, Address, Pin, TabRowData, TaskFace,
};
use aui::data::{arm_ax_probe, take_ax_labels};
use gpui::{AnyElement, IntoElement, ParentElement, TestAppContext, Window};

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
}

/// Builds the rows under test.
type Build = dyn Fn(&mut Window, &mut gpui::App) -> AnyElement;

/// The host view: rows are rendered by a view, as an app renders them.
struct Host {
    build: Rc<Build>,
}

impl gpui::Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        (self.build.clone())(window, cx)
    }
}

/// Draws `build` once in a real window and returns the effective AX labels
/// the render committed.
fn drawn_labels(
    cx: &mut TestAppContext,
    build: impl Fn(&mut Window, &mut gpui::App) -> AnyElement + 'static,
) -> Vec<String> {
    arm_ax_probe(true);
    take_ax_labels();
    let (_host, _) = cx.add_window_view(|_, _| Host { build: Rc::new(build) });
    let labels = take_ax_labels();
    arm_ax_probe(false);
    labels
}

/// Every row kind with all of its controls present: the active tab shows its
/// close `×` (inactive rows reveal it only on hover, which tests cannot do).
fn rows(window: &mut Window, cx: &mut gpui::App) -> AnyElement {
    let _ = (window, cx);
    let pins = vec![
        Pin::new("pin-u", "U", "UDISE+ dashboard"),
        Pin::new("pin-p", "P", "PGI scores"),
        Pin::new("pin-d", "D", "data.gov.in"),
        Pin::new("pin-m", "M", "NAS report cards"),
    ];
    gpui::div()
        .children([
            address_line("t-address", Address::new("pgi.udiseplus.gov.in", "/#/home", false, None, true))
                .on_click(|_, _, _| {})
                .into_any_element(),
            pin_grid("t-pins", pins).on_select(|_, _, _| {}).into_any_element(),
            tab_row("t-tab-active", TabRowData::new("PGI 2.0 · state scores 2023-24", "P").active())
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {})
                .into_any_element(),
            tab_row("t-tab-idle", TabRowData::new("UDISE+ 2024-25 · dashboard", "U"))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {})
                .into_any_element(),
            folder_row("t-folder", "Karnataka review", true, 0).on_toggle(|_, _, _| {}).into_any_element(),
            task_folder_row("t-task-typing", "PGI vs school indicators · 5 states", TaskFace::Typing, "typing", true)
                .on_toggle(|_, _, _| {})
                .on_open_chat(|_, _, _| {})
                .into_any_element(),
            task_folder_row("t-task-reading", "Overnight brief", TaskFace::Reading, "reading", false)
                .on_toggle(|_, _, _| {})
                .on_open_chat(|_, _, _| {})
                .into_any_element(),
            task_folder_row("t-task-needs", "Release sign-off", TaskFace::NeedsYou, "needs you", false)
                .on_toggle(|_, _, _| {})
                .on_open_chat(|_, _, _| {})
                .into_any_element(),
            task_folder_row("t-task-paused", "Backfill job", TaskFace::Paused, "paused", false)
                .on_toggle(|_, _, _| {})
                .on_open_chat(|_, _, _| {})
                .into_any_element(),
            task_folder_row("t-task-done", "Fix ledger chart", TaskFace::Done, "done · 1 tab", false)
                .on_toggle(|_, _, _| {})
                .on_open_chat(|_, _, _| {})
                .into_any_element(),
            task_face(TaskFace::Typing).into_any_element(),
            browse_footer("t-footer", "Education")
                .on_space(|_, _, _| {})
                .on_downloads(|_, _, _| {})
                .on_extensions(|_, _, _| {})
                .into_any_element(),
            browse_nav_cluster("t-nav")
                .can_go_back(true)
                .can_go_forward(false)
                .on_back(|_, _, _| {})
                .on_forward(|_, _, _| {})
                .on_reload(|_, _, _| {})
                .into_any_element(),
        ])
        .into_any_element()
}

/// No Browse control renders without an accessible name, and every row kind
/// reports the name the surface spec gives it.
#[gpui::test]
fn every_browse_row_has_an_accessibility_label(cx: &mut TestAppContext) {
    init(cx);
    let labels = drawn_labels(cx, rows);
    assert!(!labels.is_empty(), "the rows draw controls at all");
    let unlabelled = labels.iter().filter(|l| l.is_empty()).count();
    assert_eq!(unlabelled, 0, "unlabelled controls: {unlabelled} of {}", labels.len());
    for want in [
        "Address, pgi.udiseplus.gov.in/#/home",
        "Pinned: UDISE+ dashboard",
        "Pinned: PGI scores",
        "Pinned: data.gov.in",
        "Pinned: NAS report cards",
        "PGI 2.0 · state scores 2023-24",
        "Close PGI 2.0 · state scores 2023-24",
        "UDISE+ 2024-25 · dashboard",
        "Folder: Karnataka review",
        "Task folder: PGI vs school indicators · 5 states, typing",
        "Task folder: Overnight brief, reading",
        "Task folder: Release sign-off, needs you",
        "Task folder: Backfill job, paused",
        "Task folder: Fix ledger chart, done · 1 tab",
        "Open chat, Fix ledger chart",
        "Space, Education",
        "Downloads",
        "Extensions",
        "Back",
        "Forward",
        "Reload",
    ] {
        assert!(labels.iter().any(|l| l == want), "missing label {want:?}; got {labels:?}");
    }
}

/// The address splitter: host, path, locality and port.
#[test]
fn split_address_table() {
    let gubernatorial = split_address("https://pgi.udiseplus.gov.in/#/home");
    assert_eq!(gubernatorial.host.as_ref(), "pgi.udiseplus.gov.in");
    assert_eq!(gubernatorial.path.as_ref(), "/#/home");
    assert!(!gubernatorial.local);
    assert_eq!(gubernatorial.port, None);
    assert!(gubernatorial.secure);

    let dev = split_address("http://localhost:5173/overview");
    assert_eq!(dev.host.as_ref(), "localhost");
    assert_eq!(dev.path.as_ref(), "/overview");
    assert!(dev.local);
    assert_eq!(dev.port, Some(5173));
    assert!(!dev.secure);

    let bare = split_address("https://data.gov.in");
    assert_eq!(bare.host.as_ref(), "data.gov.in");
    assert_eq!(bare.path.as_ref(), "/");
    assert!(!bare.local);
    assert_eq!(bare.port, None);

    assert!(split_address("http://127.0.0.1:3000/x").local);
    assert!(split_address("http://app.localhost/").local);
    assert!(split_address("https://acme.test/a/b?c=1").local);
    assert!(!split_address("https://example.com:8443/a").local);
    assert_eq!(split_address("https://example.com:8443/a").port, Some(8443));
    assert_eq!(split_address("https://example.com:8443/a").path.as_ref(), "/a");
    // A non-numeric port is not a port: the authority stays intact.
    assert_eq!(split_address("https://example.com:abc/a").port, None);
    assert_eq!(split_address("https://example.com:abc/a").host.as_ref(), "example.com:abc");
}

/// Each face maps to its quiet status word; `done` composes with the count
/// at the call site (`done · N tabs`).
#[test]
fn task_face_status_words() {
    assert_eq!(TaskFace::Reading.status_word(), "reading");
    assert_eq!(TaskFace::Typing.status_word(), "typing");
    assert_eq!(TaskFace::NeedsYou.status_word(), "needs you");
    assert_eq!(TaskFace::Paused.status_word(), "paused");
    assert_eq!(TaskFace::Done.status_word(), "done");
}
