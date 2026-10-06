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
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, TestAppContext, Window};

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

/// Every row kind with all of its controls present: every tab row carries
/// its close `×` (opacity 0 until hovered or active, always in the tree).
fn rows(window: &mut Window, cx: &mut gpui::App) -> AnyElement {
    let _ = (window, cx);
    let pins = vec![
        Pin::new("pin-u", "U", "UDISE+ dashboard").position(1),
        Pin::new("pin-p", "P", "PGI scores").position(2),
        Pin::new("pin-d", "D", "data.gov.in").position(3),
        Pin::new("pin-m", "M", "NAS report cards").position(4),
    ];
    gpui::div()
        .children([
            address_line(
                "t-address",
                Address::new("pgi.udiseplus.gov.in", "/#/home", false, None, true, Some("https".to_owned())),
            )
            .on_click(|_, _, _| {})
            .into_any_element(),
            pin_grid("t-pins", pins).on_select(|_, _, _| {}).into_any_element(),
            tab_row("t-tab-active", TabRowData::new("PGI 2.0 · state scores 2023-24", "P").active().position(1))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {})
                .into_any_element(),
            tab_row("t-tab-idle", TabRowData::new("UDISE+ 2024-25 · dashboard", "U").position(2))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {})
                .into_any_element(),
            folder_row("t-folder", "Karnataka review", true, 0).on_toggle(|_, _, _| {}).into_any_element(),
            task_folder_row(
                "t-task-typing",
                "PGI vs school indicators · 5 states",
                TaskFace::Typing,
                TaskFace::Typing.status_word(),
                true,
            )
            .on_toggle(|_, _, _| {})
            .on_open_chat(|_, _, _| {})
            .into_any_element(),
            task_folder_row(
                "t-task-reading",
                "Overnight brief",
                TaskFace::Reading,
                TaskFace::Reading.status_word(),
                false,
            )
            .on_toggle(|_, _, _| {})
            .on_open_chat(|_, _, _| {})
            .into_any_element(),
            task_folder_row(
                "t-task-needs",
                "Release sign-off",
                TaskFace::NeedsYou,
                TaskFace::NeedsYou.status_word(),
                false,
            )
            .on_toggle(|_, _, _| {})
            .on_open_chat(|_, _, _| {})
            .into_any_element(),
            task_folder_row(
                "t-task-paused",
                "Backfill job",
                TaskFace::Paused,
                TaskFace::Paused.status_word(),
                false,
            )
            .on_toggle(|_, _, _| {})
            .on_open_chat(|_, _, _| {})
            .into_any_element(),
            task_folder_row(
                "t-task-done",
                "Fix ledger chart",
                TaskFace::Done,
                SharedString::from(format!("{} · 1 tab", TaskFace::Done.status_word())),
                false,
            )
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
        "Pinned 1: UDISE+ dashboard",
        "Pinned 2: PGI scores",
        "Pinned 3: data.gov.in",
        "Pinned 4: NAS report cards",
        "Tab 1: PGI 2.0 · state scores 2023-24, selected",
        "Close tab 1: PGI 2.0 · state scores 2023-24",
        "Tab 2: UDISE+ 2024-25 · dashboard",
        "Close tab 2: UDISE+ 2024-25 · dashboard",
        "Folder: Karnataka review",
        "Task folder: PGI vs school indicators · 5 states, typing",
        "typing, PGI vs school indicators · 5 states",
        "Task folder: Overnight brief, reading",
        "reading, Overnight brief",
        "Task folder: Release sign-off, needs you",
        "needs you, Release sign-off",
        "Task folder: Backfill job, paused",
        "paused, Backfill job",
        "Task folder: Fix ledger chart, done · 1 tab",
        "done · 1 tab, Fix ledger chart",
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
    let site = split_address("https://pgi.udiseplus.gov.in/#/home");
    assert_eq!(site.host.as_ref(), "pgi.udiseplus.gov.in");
    assert_eq!(site.path.as_ref(), "/#/home");
    assert!(!site.local);
    assert_eq!(site.port, None);
    assert!(site.secure);

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

/// The review cases, one row each: the authority ends at the first `/`,
/// `?` or `#`, schemes match case-insensitively, internal schemes stay
/// secure without a port, userinfo never reaches the host, every loopback
/// form is local, non-default ports show and default ports hide.
#[test]
fn split_address_review_cases() {
    struct Case {
        url: &'static str,
        host: &'static str,
        path: &'static str,
        local: bool,
        port: Option<u16>,
        secure: bool,
    }
    let cases = [
        Case { url: "http://h?u=http://x/y", host: "h", path: "/?u=http://x/y", local: false, port: None, secure: false },
        Case { url: "http://[::1]/", host: "[::1]", path: "/", local: true, port: None, secure: false },
        Case { url: "example.com:8443", host: "example.com", path: "/", local: false, port: Some(8443), secure: true },
        Case { url: "http://example.com:80/", host: "example.com", path: "/", local: false, port: None, secure: false },
        Case { url: "data:text/html,hi", host: "data", path: "/text/html,hi", local: false, port: None, secure: true },
        Case { url: "", host: "", path: "/", local: false, port: None, secure: true },
        Case { url: "https://", host: "", path: "/", local: false, port: None, secure: true },
        Case { url: "http://localhost:", host: "localhost", path: "/", local: true, port: None, secure: false },
        Case { url: "http://localhost:99999", host: "localhost", path: "/", local: true, port: None, secure: false },
        Case { url: "javascript:alert(1)", host: "javascript:", path: "/", local: false, port: None, secure: false },
        Case { url: "mailto:a@b.example", host: "mailto:", path: "/", local: false, port: None, secure: false },
        Case { url: "localhost:5173", host: "localhost", path: "/", local: true, port: Some(5173), secure: true },
        Case {
            url: "https://example.com?x=1",
            host: "example.com",
            path: "/?x=1",
            local: false,
            port: None,
            secure: true,
        },
        Case {
            url: "https://example.com#frag",
            host: "example.com",
            path: "/#frag",
            local: false,
            port: None,
            secure: true,
        },
        Case {
            url: "HTTPS://example.com/",
            host: "example.com",
            path: "/",
            local: false,
            port: None,
            secure: true,
        },
        Case {
            url: "about:blank",
            host: "about:blank",
            path: "/",
            local: false,
            port: None,
            secure: true,
        },
        Case {
            url: "file:///x",
            host: "file",
            path: "/x",
            local: false,
            port: None,
            secure: true,
        },
        Case {
            url: "chrome://settings/",
            host: "settings",
            path: "/",
            local: false,
            port: None,
            secure: true,
        },
        Case {
            url: "https://user:pw@example.com/",
            host: "example.com",
            path: "/",
            local: false,
            port: None,
            secure: true,
        },
        Case {
            url: "http://[::1]:8080/a",
            host: "[::1]",
            path: "/a",
            local: true,
            port: Some(8080),
            secure: false,
        },
        Case {
            url: "http://127.0.0.5/",
            host: "127.0.0.5",
            path: "/",
            local: true,
            port: None,
            secure: false,
        },
        Case {
            url: "http://localhost./",
            host: "localhost.",
            path: "/",
            local: true,
            port: None,
            secure: false,
        },
        Case {
            url: "http://0.0.0.0:3000/",
            host: "0.0.0.0",
            path: "/",
            local: true,
            port: Some(3000),
            secure: false,
        },
        Case {
            url: "https://example.com:8443/",
            host: "example.com",
            path: "/",
            local: false,
            port: Some(8443),
            secure: true,
        },
        Case {
            url: "https://example.com:443/",
            host: "example.com",
            path: "/",
            local: false,
            port: None,
            secure: true,
        },
        Case {
            url: "http://example.com",
            host: "example.com",
            path: "/",
            local: false,
            port: None,
            secure: false,
        },
    ];
    for case in cases {
        let got = split_address(case.url);
        assert_eq!(got.host.as_ref(), case.host, "host for {}", case.url);
        assert_eq!(got.path.as_ref(), case.path, "path for {}", case.url);
        assert_eq!(got.local, case.local, "local for {}", case.url);
        assert_eq!(got.port, case.port, "port for {}", case.url);
        assert_eq!(got.secure, case.secure, "secure for {}", case.url);
    }
    // The internal schemes keep their scheme so callers can tell.
    assert_eq!(split_address("chrome://settings/").scheme.as_deref(), Some("chrome"));
    assert_eq!(split_address("about:blank").scheme.as_deref(), Some("about"));
    assert_eq!(split_address("file:///x").scheme.as_deref(), Some("file"));
    assert_eq!(split_address("https://example.com/").scheme.as_deref(), Some("https"));
}

/// An inactive, unhovered tab row still carries its close button: the `×`
/// hides through opacity, never by leaving the tree.
#[gpui::test]
fn inactive_tab_row_keeps_its_close_button(cx: &mut TestAppContext) {
    init(cx);
    let labels = drawn_labels(cx, |_, _| {
        tab_row("t-tab-idle", TabRowData::new("UDISE+ 2024-25 · dashboard", "U").position(2))
            .on_select(|_, _, _| {})
            .on_close(|_, _, _| {})
            .into_any_element()
    });
    assert!(
        labels.iter().any(|l| l == "Tab 2: UDISE+ 2024-25 · dashboard"),
        "missing tab label; got {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == "Close tab 2: UDISE+ 2024-25 · dashboard"),
        "missing close label; got {labels:?}"
    );
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

#[test]
fn split_address_never_panics_on_malformed_input() {
    for url in ["", ":", "://", "http://", "http://[", "http://[::1", "a:b:c", "http://host:", "http://host:99999", "\\", "http://h?#", "about:", "file:", "chrome:", "data:", "http://user@", "http://@host/", "[", "]", "http://[]:1/"] {
        let _ = split_address(url);
    }
}
