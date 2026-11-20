//! The raised composer's new-session row (docs/83 §5): the folder control
//! and the "Own worktree" toggle sit at the left of the quiet row; each reports its own intent, and neither reports while the
//! draft is moving. Headless windows have no rasterizer, so what a test can
//! prove is intent: a click sweep across the quiet row logs only through
//! the controls in it.

use std::cell::RefCell;
use std::rc::Rc;

use aui::composer::{composer, composer_state, ComposerNewRow, ComposerWorktree};
use aui::icons::Provider;
use gpui::{point, px, IntoElement, Modifiers, TestAppContext, VisualTestContext};

type Log = Rc<RefCell<Vec<String>>>;

struct Host {
    log: Log,
    row: Option<ComposerNewRow>,
}

impl gpui::Render for Host {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let text = window.use_keyed_state("new-row-text", cx, |window, cx| composer_state("Ask anything", window, cx));
        let log = self.log.clone();
        composer("new-row", &text, Provider::Codex, "gpt-5")
            .raised(true)
            .new_row(self.row.clone())
            .on_intent(move |intent, _, _| log.borrow_mut().push(format!("{intent:?}")))
    }
}

/// Clicks across the band under the box, left to right; returns what the
/// composer reported.
fn sweep(cx: &mut TestAppContext, row: Option<ComposerNewRow>) -> Vec<String> {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        move |_, _| Host { log, row }
    });
    let vcx: &mut VisualTestContext = vcx;
    let mut y = 40.0;
    while y <= 110.0 {
        let mut x = 2.0;
        while x <= 1600.0 {
            vcx.simulate_click(point(px(x), px(y)), Modifiers::none());
            x += 6.0;
        }
        vcx.run_until_parked();
        y += 4.0;
    }
    let out = log.borrow().clone();
    out
}

fn first(log: &[String], name: &str) -> Option<usize> {
    log.iter().position(|entry| entry == name)
}

#[gpui::test]
fn the_folder_control_then_the_worktree_toggle_report_their_intents(cx: &mut TestAppContext) {
    let row = ComposerNewRow { folder: "acme-web".into(), worktree: Some(ComposerWorktree::Off), busy: false };
    let log = sweep(cx, Some(row));
    let folder = first(&log, "Folder").unwrap_or_else(|| panic!("no click reached the folder control: {log:?}"));
    let worktree = first(&log, "Worktree").unwrap_or_else(|| panic!("no click reached the worktree toggle: {log:?}"));
    assert!(folder < worktree, "the row reads [folder ▾] [Own worktree], left to right: {log:?}");
}

#[gpui::test]
fn a_moving_draft_reports_neither(cx: &mut TestAppContext) {
    let row = ComposerNewRow { folder: "acme-web".into(), worktree: Some(ComposerWorktree::Busy), busy: true };
    let log = sweep(cx, Some(row));
    assert!(first(&log, "Folder").is_none() && first(&log, "Worktree").is_none(), "inert while moving: {log:?}");
    assert!(first(&log, "TogglePlus").is_some(), "the rest of the composer still answers: {log:?}");
}

#[gpui::test]
fn no_new_row_no_controls(cx: &mut TestAppContext) {
    let log = sweep(cx, None);
    assert!(first(&log, "Folder").is_none() && first(&log, "Worktree").is_none(), "{log:?}");
    let hidden = ComposerNewRow { folder: "notes".into(), worktree: None, busy: false };
    let log = sweep(cx, Some(hidden));
    assert!(first(&log, "Folder").is_some(), "{log:?}");
    assert!(first(&log, "Worktree").is_none(), "a folder that cannot have one shows no toggle: {log:?}");
}
