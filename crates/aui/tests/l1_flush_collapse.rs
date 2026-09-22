//! L1: flush panel chrome, and the rail mounted beneath the sidebar.
//!
//! `flush()` must drop the outer card chrome (radius + outer border) while
//! keeping every internal divider, so a flush panel paints fewer quads than
//! the default card. The file tree draws no outer chrome today, so its
//! `flush()` must paint no more than the default.
//!
//! The sidebar cell must mount the rail beneath the sidebar on every frame
//! the sidebar is shown (open first frame included), instead of choosing one
//! or the other by a threshold; the settled collapsed column still shows the
//! rail alone.
//!
//! What is deliberately NOT tested here: whether the collapse still pops,
//! whether stacked flush panels look right, and whether the plain-text
//! Changes count reads better than the old pill. Those need a watcher, the
//! consumer's layout, and a judgment call — no quad count proves them.

use std::cell::Cell;
use std::rc::Rc;

use aui::shell::app_shell;
use aui::workbench::{
    diff_review, file_tree, git_changes, pr_check, pr_form, DiffScope, DiffView, FileNode,
    PrDescription, ReviewFile,
};
use aui_protocol::{ChangeKind, Diff, FileChange};
use gpui::{div, prelude::*, AnyElement, IntoElement, TestAppContext};

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
}

fn change() -> FileChange {
    FileChange { path: "src/main.rs".to_string(), change: ChangeKind::Modified, added: 8, removed: 3 }
}

enum Panel {
    Git(bool),
    Pr(bool),
    Diff(bool),
    Files(bool),
}

struct PanelHost {
    panel: Panel,
}

impl gpui::Render for PanelHost {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        match self.panel {
            Panel::Git(flush) => {
                let mut el = git_changes(
                    "l1-changes",
                    vec![(change(), true)],
                    "Drafted message",
                    1,
                    0,
                );
                if flush {
                    el = el.flush();
                }
                el.into_any_element()
            }
            Panel::Pr(flush) => {
                let mut el = pr_form(
                    "l1-pr",
                    "main",
                    "Add flush panes",
                    vec![PrDescription::Text("Body text".into())],
                    vec![pr_check("ci / unit", true)],
                );
                if flush {
                    el = el.flush();
                }
                el.into_any_element()
            }
            Panel::Diff(flush) => {
                let files = vec![ReviewFile { change: change(), notes: 0, selected: true }];
                let diff = Diff { path: "src/main.rs".to_string(), hunks: Vec::new(), added: 8, removed: 3 };
                let mut el = diff_review("l1-diff", files, diff, Vec::new(), DiffScope::ThisTurn, DiffView::Unified);
                if flush {
                    el = el.flush();
                }
                el.into_any_element()
            }
            Panel::Files(flush) => {
                let nodes = vec![
                    FileNode::dir("root", "repo", true),
                    FileNode::dir("src", "src", true).depth(1),
                ];
                let mut el = file_tree("l1-tree", nodes).header("repo").footer("worktree demo");
                if flush {
                    el = el.flush();
                }
                el.into_any_element()
            }
        }
    }
}

/// First-frame painted quads for `panel`. `add_window_view` draws exactly
/// once, so this is the same frame a host would show first.
fn panel_quads(cx: &mut TestAppContext, panel: Panel) -> usize {
    let (_host, cx) = cx.add_window_view(|_, _| PanelHost { panel });
    cx.update(|window, _| window.painted_quads()).len()
}

#[gpui::test]
fn flush_changes_paints_fewer_quads_than_the_default_card(cx: &mut TestAppContext) {
    init(cx);
    let card = panel_quads(cx, Panel::Git(false));
    let flush = panel_quads(cx, Panel::Git(true));
    assert!(
        flush < card,
        "flush() must drop the outer border quads: card painted {card}, flush painted {flush}"
    );
}

#[gpui::test]
fn flush_pr_form_paints_fewer_quads_than_the_default_card(cx: &mut TestAppContext) {
    init(cx);
    let card = panel_quads(cx, Panel::Pr(false));
    let flush = panel_quads(cx, Panel::Pr(true));
    assert!(
        flush < card,
        "flush() must drop the outer border quads: card painted {card}, flush painted {flush}"
    );
}

#[gpui::test]
fn flush_diff_review_paints_fewer_quads_than_the_default_card(cx: &mut TestAppContext) {
    init(cx);
    let card = panel_quads(cx, Panel::Diff(false));
    let flush = panel_quads(cx, Panel::Diff(true));
    assert!(
        flush < card,
        "flush() must drop the outer border quads: card painted {card}, flush painted {flush}"
    );
}

#[gpui::test]
fn flush_file_tree_paints_no_more_quads_than_default(cx: &mut TestAppContext) {
    init(cx);
    let plain = panel_quads(cx, Panel::Files(false));
    let flush = panel_quads(cx, Panel::Files(true));
    assert!(
        flush <= plain,
        "the tree draws no outer chrome, so flush() must add nothing: default painted {plain}, flush painted {flush}"
    );
}

struct ShellHost {
    sidebar_seen: Rc<Cell<bool>>,
    rail_seen: Rc<Cell<bool>>,
    open: bool,
}

impl gpui::Render for ShellHost {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let sidebar_seen = self.sidebar_seen.clone();
        let rail_seen = self.rail_seen.clone();
        let sidebar = div()
            .size_full()
            .on_children_prepainted(move |_, _, _| sidebar_seen.set(true))
            .child(div().size_full())
            .into_any_element();
        let rail = div()
            .size_full()
            .on_children_prepainted(move |_, _, _| rail_seen.set(true))
            .child(div().size_full())
            .into_any_element();
        let centre: AnyElement = div().size_full().into_any_element();
        app_shell("l1-shell")
            .sidebar_open(self.open)
            .sidebar(sidebar)
            .rail(rail)
            .centre(centre)
    }
}

#[gpui::test]
fn the_rail_is_mounted_beneath_the_sidebar_while_open(cx: &mut TestAppContext) {
    init(cx);
    let sidebar_seen = Rc::new(Cell::new(false));
    let rail_seen = Rc::new(Cell::new(false));
    let (_host, _) = cx.add_window_view({
        let (sidebar_seen, rail_seen) = (sidebar_seen.clone(), rail_seen.clone());
        move |_, _| ShellHost { sidebar_seen, rail_seen, open: true }
    });
    assert!(
        sidebar_seen.get(),
        "the open sidebar must be in the tree on the first frame"
    );
    assert!(
        rail_seen.get(),
        "the rail must already be mounted beneath the open sidebar, not swapped in by a threshold later"
    );
}

#[gpui::test]
fn the_settled_collapsed_column_shows_the_rail_alone(cx: &mut TestAppContext) {
    init(cx);
    let sidebar_seen = Rc::new(Cell::new(false));
    let rail_seen = Rc::new(Cell::new(false));
    let _handle = cx.open_window(gpui::size(gpui::px(1600.0), gpui::px(900.0)), {
        let (sidebar_seen, rail_seen) = (sidebar_seen.clone(), rail_seen.clone());
        move |_, _| ShellHost { sidebar_seen, rail_seen, open: false }
    });
    cx.run_until_parked();
    assert!(rail_seen.get(), "the settled collapsed column must show the rail");
    assert!(
        !sidebar_seen.get(),
        "the settled collapsed column must not keep the sidebar mounted over the rail"
    );
}
