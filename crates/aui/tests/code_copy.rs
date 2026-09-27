//! The code block's Copy button copies by default.
//!
//! The button used to only emit [`CodeBlockAction::Copy`](aui::transcript::CodeBlockAction);
//! with no host handler wired, clicking it did nothing. Now the block writes
//! its text to the clipboard itself, and a host handler only adds behaviour.
//! Headless windows have no rasterizer, so the tests click-sweep the header
//! band the way the composer's chip test sweeps its toolbar.

use std::cell::RefCell;
use std::rc::Rc;

use aui::transcript::{code_block, CodeBlockAction};
use gpui::{point, px, size, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window};

/// The block text the sweep is expected to land on the clipboard.
const CODE: &str = "fn main() {\n    println!(\"hello\");\n}";

/// Clicks sweep the 30 px header and a little beyond it, so header-height
/// drift still lands rows on the Copy button at the row's right end.
const SWEEP_TOP: f32 = 2.0;
const SWEEP_BOTTOM: f32 = 36.0;
const SWEEP_STEP_Y: f32 = 4.0;
const SWEEP_LEFT: f32 = 4.0;
const SWEEP_RIGHT: f32 = 596.0;
const SWEEP_STEP_X: f32 = 6.0;

/// What the host handler saw, in order.
type Log = Rc<RefCell<Vec<CodeBlockAction>>>;

struct CodeHost {
    log: Option<Log>,
}

impl Render for CodeHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let block = code_block("code-copy", "main.rs", CODE);
        match &self.log {
            Some(log) => {
                let log = log.clone();
                block.on_action(move |action, _, _| log.borrow_mut().push(action))
            }
            None => block,
        }
    }
}

/// Draws the block with (or without) a host handler, sweeps the header band,
/// and returns the clipboard text plus what the handler saw.
fn sweep_header(cx: &mut TestAppContext, wired: bool) -> (Option<String>, Vec<CodeBlockAction>) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(600.0), px(400.0)), {
        let log = log.clone();
        move |_, _| CodeHost { log: wired.then(|| log.clone()) }
    });
    let mut vcx = VisualTestContext::from_window(window.into(), cx);
    vcx.run_until_parked();
    let mut y = SWEEP_TOP;
    while y <= SWEEP_BOTTOM {
        let mut x = SWEEP_LEFT;
        while x <= SWEEP_RIGHT {
            vcx.simulate_click(point(px(x), px(y)), Modifiers::none());
            x += SWEEP_STEP_X;
        }
        vcx.run_until_parked();
        y += SWEEP_STEP_Y;
    }
    let text = cx.read_from_clipboard().as_ref().and_then(|item| item.text());
    let out = (text, log.borrow().clone());
    out
}

/// With no host handler, a Copy click puts the block's text on the test
/// clipboard.
#[gpui::test]
fn copy_click_copies_with_no_host_handler(cx: &mut TestAppContext) {
    let (text, _) = sweep_header(cx, false);
    assert_eq!(text.as_deref(), Some(CODE), "a Copy click with no handler must copy the block, got {text:?}");
}

/// A host handler still sees the Copy action, and the text is copied anyway.
#[gpui::test]
fn copy_click_reports_to_the_host_and_still_copies(cx: &mut TestAppContext) {
    let (text, log) = sweep_header(cx, true);
    assert!(log.contains(&CodeBlockAction::Copy), "the host must still see Copy, got {log:?}");
    assert_eq!(text.as_deref(), Some(CODE), "the block must copy even with a handler, got {text:?}");
}
