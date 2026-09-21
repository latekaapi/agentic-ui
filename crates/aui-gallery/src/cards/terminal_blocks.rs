//! `workbench/terminal-blocks` — the Stage L grid with the block overlay.
//!
//! A scripted transcript drives a real [`TerminalSession`] through the
//! nonce-checked OSC 133 marks: four commands, one still running, one failed
//! with a non-zero exit, one tagged [`BlockAuthor::Agent`]. The card shows the
//! [`terminal_grid`] element with its overlay chrome (status glyph, duration,
//! exit) over the grid. The frame and tab strip are card 50's
//! (`super::terminal_live`), exactly like the other terminal cards.
//!
//! The nonce is a fixed card literal: every scripted marker carries
//! `k=<NONCE>` where `NONCE` is the session's own nonce, because a marker
//! without the session's nonce is ignored by design and would show no blocks.
//! The whole transcript is pumped synchronously before the first frame, so the
//! screenshot is deterministic.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;

use aui_terminal::{terminal_grid, BlockAuthor, TermEvent, TerminalBackend, TerminalSession};
use gpui::*;

use super::terminal_live::{frame, strip, TERM_FRAME_H};

/// The card's session nonce. Every scripted marker carries `k=<NONCE>`.
const NONCE: &str = "l6blocks01";
/// The grid opens at this size until the pane measures itself.
const GRID_COLS: u16 = 100;
/// …and this many rows.
const GRID_ROWS: u16 = 32;

/// One nonce-checked marker: `OSC 133 ; <body> ; k=<NONCE> BEL`.
fn marker(body: &str) -> String {
    format!("\x1b]133;{body};k={NONCE}\x07")
}

/// The pre-exec marker carrying the exact command text (`enc=raw` has no `;`
/// by construction, so none of the card's commands contains one).
fn out_start(cmd: &str) -> String {
    format!("\x1b]133;C;k={NONCE};cmd={cmd};enc=raw\x07")
}

/// The scripted transcript: three finished commands and one still running.
fn transcript() -> String {
    let d = "\x1b[2m";
    let r = "\x1b[0m";
    let g = "\x1b[32m";
    let red = "\x1b[31m";
    let prompt = format!("~/work/acme {d}\u{276f}{r} ");
    let mut s = String::new();
    // 1. A quick success.
    s.push_str(&marker("A"));
    s.push_str(&prompt);
    s.push_str(&marker("B"));
    s.push_str("git status -sb\r\n");
    s.push_str(&out_start("git status -sb"));
    s.push_str(&format!("{g}##{r} feature/checkout-flow-v2\r\n"));
    s.push_str(&format!("{d} M{r} src/checkout/validators.ts\r\n"));
    s.push_str(&marker("D;0"));
    s.push_str("\r\n");
    // 2. A failure with a non-zero exit.
    s.push_str(&marker("A"));
    s.push_str(&prompt);
    s.push_str(&marker("B"));
    s.push_str("pnpm test --filter checkout\r\n");
    s.push_str(&out_start("pnpm test --filter checkout"));
    s.push_str(&format!("{red}\u{2716}{r} validators.test.ts {d}(1 failed){r}\r\n"));
    s.push_str("  46:5  'validateCanadianPostal' is not defined\r\n");
    s.push_str(&marker("D;1"));
    s.push_str("\r\n");
    // 3. An agent-run build (tagged Agent below).
    s.push_str(&marker("A"));
    s.push_str(&prompt);
    s.push_str(&marker("B"));
    s.push_str("cargo build -p aui-terminal\r\n");
    s.push_str(&out_start("cargo build -p aui-terminal"));
    s.push_str(&format!("{d}   Compiling{r} aui-terminal v0.1.0\r\n"));
    s.push_str(&format!("{g}    Finished{r} dev profile in 4.2 s\r\n"));
    s.push_str(&marker("D;0"));
    s.push_str("\r\n");
    // 4. Still running: `C` arrived, no `D` yet.
    s.push_str(&marker("A"));
    s.push_str(&prompt);
    s.push_str(&marker("B"));
    s.push_str("pnpm vitest run src/checkout\r\n");
    s.push_str(&out_start("pnpm vitest run src/checkout"));
    s.push_str(&format!("{g}\u{2713}{r} validators.test.ts {d}(18){r}\r\n"));
    s.push_str(&format!("{d}\u{283c}{r} checkout.e2e.ts {d}running\u{2026}{r}\r\n"));
    s
}

/// A backend holding the scripted transcript: one poll drains it, later
/// polls are empty. (`FakePty` cannot serve here — its clock is `!Send` and
/// the session takes its backend across threads.)
struct OnceBackend {
    chunks: Mutex<VecDeque<Vec<u8>>>,
}

impl TerminalBackend for OnceBackend {
    fn spawn(&mut self, _shell: &str, _cwd: &Path) -> std::io::Result<()> {
        Ok(())
    }

    fn write(&mut self, _bytes: &[u8]) {}

    fn resize(&mut self, _cols: u16, _rows: u16) {}

    fn poll(&mut self) -> Vec<TermEvent> {
        self.chunks.lock().unwrap().drain(..).map(TermEvent::Output).collect()
    }
}

/// A session on the scripted transcript, kept in window state. The transcript
/// is pumped synchronously so the card (and its screenshot) shows the whole
/// session on the first frame; the grid's reader thread then finds an
/// exhausted backend and stays quiet.
fn session(window: &mut Window, cx: &mut App) -> Entity<TerminalSession> {
    window.use_keyed_state(SharedString::from("terminal-blocks-session"), cx, |_, _| {
        let backend = OnceBackend { chunks: Mutex::new(VecDeque::from([transcript().into_bytes()])) };
        let session =
            TerminalSession::new(Box::new(backend), GRID_COLS, GRID_ROWS).with_nonce(NONCE);
        session.pump();
        // The build ran under the agent; the host says so after the fact.
        session.set_block_author(2, BlockAuthor::Agent);
        session
    })
}

/// Builds the card content: card 50's frame and strip around the live grid.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let state = session(window, cx);
    let tabs = strip("terminal-blocks-tabs", 0, window, cx);
    let grid = terminal_grid(&state).on_intent(|intent, _, _| {
        // The gallery has nothing to open, copy or stop; the card exists to
        // show that the wiring reaches a host.
        let _ = intent;
    });
    frame(TERM_FRAME_H, tabs, div().w_full().flex_1().min_h(px(0.0)).child(grid), cx)
}
