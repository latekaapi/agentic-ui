//! `workbench/terminal-dock` — the Stage L dock frame with its tab strip.
//!
//! [`terminal_dock`] plus [`terminal_tabs`] the way the host mounts them:
//! three tabs, the first active, the agent's tab marked with a running
//! command. The body is the block terminal over the scripted card 50 session,
//! so the running command shows inside the dock frame too.

use aui::workbench::{terminal_dock, terminal_tabs, TermTab};
use aui_icons::Provider;
use aui_terminal::{block_terminal_view, BlockParser, FakePty, TerminalState};
use gpui::*;

/// The dock's width: the gallery adds 20, the card pulls in by 8.
const DOCK_W: f32 = 980.0 - 24.0;
/// `.term{height:690px}` in the 980×720 card…
const DOCK_H: f32 = 690.0;
/// The card pulls in by 8 px against the gallery's 20 px card ground.
const DOCK_INSET: f32 = -8.0;

/// A session on the card 50 recording, kept in window state so its poll timer
/// survives between frames.
fn session(window: &mut Window, cx: &mut App) -> Entity<TerminalState> {
    window.use_keyed_state(SharedString::from("terminal-dock-body"), cx, |_, _| {
        let fake = FakePty::card50();
        let parser = BlockParser::with_clock(fake.clock());
        TerminalState::with_parser(Box::new(fake), parser)
    })
}

/// Builds the card content: the dock frame with three tabs and a live body.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let state = session(window, cx);
    let tabs = vec![
        TermTab::new("shell", "shell"),
        TermTab::new("pi", "pi").agent(Provider::Pi).busy(true),
        TermTab::new("codex", "codex").agent(Provider::Codex),
    ];
    let header = terminal_tabs("terminal-dock-tabs", tabs, 0).on_action(|action, _, _| {
        // The gallery owns no terminals; the card exists to show the frame.
        let _ = action;
    });
    let body = block_terminal_view("terminal-dock-term", &state).on_intent(|intent, _, _| {
        let _ = intent;
    });
    let dock = terminal_dock("terminal-dock", header, Some(body))
        .hint("\u{2318}` toggle \u{b7} agent runs in a PTY with your login")
        .on_action(|action, _, _| {
            let _ = action;
        });
    div().w(px(DOCK_W)).h(px(DOCK_H)).flex_none().m(px(DOCK_INSET)).child(dock).into_any_element()
}
