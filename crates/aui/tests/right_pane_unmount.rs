//! The closed right pane unmounts once its spring settles, the way the
//! sidebar does. Clipped-but-mounted content stayed in the accessibility tree
//! as off-screen tabs and close buttons (seen in Baaz's AX tree at x≈1405–1746
//! with the pane closed).
//!
//! What is deliberately NOT tested here: the close animation itself (the
//! content must stay mounted while the column springs shut) — that needs a
//! watcher, not a prepaint flag.

use std::cell::Cell;
use std::rc::Rc;

use aui::shell::app_shell;
use gpui::{div, prelude::*, IntoElement, TestAppContext};

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
}

struct Host {
    right_seen: Rc<Cell<bool>>,
    header_seen: Rc<Cell<bool>>,
    open: bool,
}

impl gpui::Render for Host {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let right_seen = self.right_seen.clone();
        let header_seen = self.header_seen.clone();
        app_shell("unmount-shell")
            .right_open(self.open)
            .centre(div().size_full())
            .header_right(div().size_full().on_children_prepainted(move |_, _, _| header_seen.set(true)).child(div()))
            .right(div().size_full().on_children_prepainted(move |_, _, _| right_seen.set(true)).child(div()))
    }
}

fn seen(cx: &mut TestAppContext, open: bool) -> (bool, bool) {
    init(cx);
    let right_seen = Rc::new(Cell::new(false));
    let header_seen = Rc::new(Cell::new(false));
    let _handle = cx.open_window(gpui::size(gpui::px(1600.0), gpui::px(900.0)), {
        let (right_seen, header_seen) = (right_seen.clone(), header_seen.clone());
        move |_, _| Host { right_seen, header_seen, open }
    });
    cx.run_until_parked();
    (right_seen.get(), header_seen.get())
}

#[gpui::test]
fn the_open_right_pane_is_mounted(cx: &mut TestAppContext) {
    let (right, header) = seen(cx, true);
    assert!(right, "the open pane's content must be in the tree");
    assert!(header, "the open pane's header cell must be in the tree");
}

#[gpui::test]
fn the_settled_closed_right_pane_is_not_mounted(cx: &mut TestAppContext) {
    let (right, header) = seen(cx, false);
    assert!(!right, "a settled closed pane must not keep its content mounted (it lingers in the AX tree)");
    assert!(!header, "a settled closed pane must not keep its header tabs mounted");
}
