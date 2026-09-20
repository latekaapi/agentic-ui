//! [`BlockParser`] — a byte stream in, [`TermBlock`]s out.
//!
//! Card 50 draws a terminal as *blocks*, not as a wall of text, and the only
//! way to know where one command ends and the next begins is to ask the shell.
//! The de-facto standard for that is OSC 133 "shell integration", which every
//! modern terminal understands:
//!
//! | marker | meaning |
//! |---|---|
//! | `OSC 133 ; A ST` | a prompt is about to be drawn — the previous block ends here, and the next block's clock starts |
//! | `OSC 133 ; B ST` | the prompt is done; what follows is the command line |
//! | `OSC 133 ; C ST` | the command is running; what follows is its output |
//! | `OSC 133 ; D ; <exit> ST` | the command finished with this status |
//!
//! Every marker the integrated shell emits also carries `; k=<nonce>`, a
//! per-session token (see [`with_nonce`](BlockParser::with_nonce)): `A ; k=<nonce>`,
//! `D ; <exit> ; k=<nonce>`, and so on. A [`BlockParser`] with a nonce set
//! ignores a marker whose `k=` is absent or does not match — no block
//! boundary, no state change — so a program printing a bare `OSC 133` cannot
//! forge one. A parser with no nonce behaves exactly as before and accepts
//! every marker, which is what the gallery's scripted replays rely on.
//!
//! `ZSH_INTEGRATION` lives in the `pty` module, behind the `pty` feature.
//!
//! Everything else in the stream is passed through untouched, so the SGR
//! escapes survive into [`TermBlock::output`] and
//! [`aui::transcript::parse_ansi`] can colour the lines at render time. What
//! this parser drops is the OSC 133 sequences themselves, carriage returns,
//! the prompt text between `A` and `B` (which the block terminal draws
//! itself), and every escape that is not SGR — `parse_ansi` reads an escape by
//! swallowing everything up to the next `m`, so a stray `ESC [ 2 K` left in a
//! line would eat the text after it.
//!
//! A shell with no integration installed emits no markers at all. That is not
//! an error: the whole stream becomes one running block with an empty command,
//! which is exactly what the TUI pane wants.
//!
//! The parser does not model a screen. There is no cursor, no scroll region
//! and no in-place redraw — cursor-movement CSI sequences are kept verbatim in
//! the line rather than acted on. For a full-screen program use
//! `TuiGrid` (module `tui_grid`, feature `tui`) instead.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use aui::workbench::{BlockState, TermBlock};

/// Finished blocks print one decimal (`6.2 s`), matching `.dur` on card 50.
const FINISHED_DECIMALS: usize = 1;
/// The CSI final byte that sets graphic rendition — the only one kept.
const SGR: char = 'm';

/// A clock whose "now" is set by hand, for tests and for scripted replay.
///
/// [`FakePty`](crate::fake::FakePty) hands one of these to the parser so that
/// a transcript recorded at human speed can be replayed faster than real time
/// and still produce the durations it was recorded with.
#[derive(Clone, Debug)]
pub struct ManualClock(Rc<Cell<Instant>>);

impl ManualClock {
    /// A clock parked at `now`.
    pub fn new(now: Instant) -> Self {
        Self(Rc::new(Cell::new(now)))
    }

    /// A clock parked at [`Instant::now`].
    pub fn started() -> Self {
        Self::new(Instant::now())
    }

    /// The current time.
    pub fn now(&self) -> Instant {
        self.0.get()
    }

    /// Moves the clock to `t`. Moving backwards is allowed but every duration
    /// this crate computes saturates at zero, so it never panics.
    pub fn set(&self, t: Instant) {
        self.0.set(t);
    }

    /// Moves the clock forward by `d`.
    pub fn advance(&self, d: Duration) {
        self.0.set(self.0.get() + d);
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::started()
    }
}

/// The duration label of a finished block: `6.2 s`, or `3.2 s · exit 1`.
pub fn finished_label(elapsed: Duration, exit: i32) -> String {
    let secs = format!("{:.*} s", FINISHED_DECIMALS, elapsed.as_secs_f64());
    if exit == 0 {
        secs
    } else {
        format!("{secs} · exit {exit}")
    }
}

/// The duration label of the live block: whole seconds, `12 s`.
pub fn live_label(elapsed: Duration) -> String {
    format!("{} s", elapsed.as_secs())
}

/// Where in a command's life cycle the stream currently is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Before the first marker, or after a `D` — bytes here open an
    /// unattributed block (the no-integration fallback).
    Idle,
    /// Between `A` and `B`: the shell is painting its prompt. Dropped.
    Prompt,
    /// Between `B` and `C`: the command line the user ran.
    Command,
    /// Between `C` and `D`: the command's output.
    Output,
}

/// Turns a terminal byte stream into [`TermBlock`]s. See the [module
/// docs](self).
pub struct BlockParser {
    vte: vte::Parser,
    perform: Perform,
}

impl BlockParser {
    /// A parser on the real clock.
    pub fn new() -> Self {
        Self::with_clock(ManualClock::started())
    }

    /// A parser reading its timestamps from `clock`.
    pub fn with_clock(clock: ManualClock) -> Self {
        Self { vte: vte::Parser::new(), perform: Perform::new(clock) }
    }

    /// A parser that only honours markers carrying `k=<nonce>`.
    ///
    /// A marker whose `k=` is absent or does not match is ignored entirely:
    /// no block boundary, no state change. A parser with no nonce accepts
    /// every marker, exactly as before.
    pub fn with_nonce(mut self, nonce: impl Into<String>) -> Self {
        self.perform.nonce = Some(nonce.into());
        self
    }

    /// The nonce this parser enforces, if any (see [`with_nonce`](Self::with_nonce)).
    pub fn nonce(&self) -> Option<&str> {
        self.perform.nonce.as_deref()
    }

    /// The clock this parser stamps blocks with.
    pub fn clock(&self) -> &ManualClock {
        &self.perform.clock
    }

    /// Feeds bytes. Chunk boundaries are free: an escape sequence split across
    /// two calls is reassembled by `vte`'s state machine.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.vte.advance(&mut self.perform, bytes);
        self.perform.refresh_live();
    }

    /// Recomputes the running block's elapsed label. Call it on the UI timer
    /// so the live duration ticks even while the command is silent.
    pub fn tick(&mut self) {
        self.perform.refresh_live();
    }

    /// The blocks parsed so far, oldest first. The last one is still running
    /// unless the stream ended with a `D` marker.
    pub fn blocks(&self) -> &[TermBlock] {
        &self.perform.blocks
    }
}

impl Default for BlockParser {
    fn default() -> Self {
        Self::new()
    }
}

/// The `vte::Perform` half: all the state lives here so that `BlockParser` can
/// lend `vte::Parser` and this struct out at the same time.
struct Perform {
    clock: ManualClock,
    /// When set, only markers carrying `k=<nonce>` are honoured.
    nonce: Option<String>,
    blocks: Vec<TermBlock>,
    phase: Phase,
    /// Index of the block currently taking output, if any.
    open: Option<usize>,
    /// True while the open block's last output line is unterminated.
    partial: bool,
    /// The command line accumulated between `B` and `C`.
    command: String,
    /// When the open (or next) block's clock started.
    started: Option<Instant>,
    next_id: usize,
}

impl Perform {
    fn new(clock: ManualClock) -> Self {
        Self {
            clock,
            nonce: None,
            blocks: Vec::new(),
            phase: Phase::Idle,
            open: None,
            partial: false,
            command: String::new(),
            started: None,
            next_id: 0,
        }
    }

    fn now(&self) -> Instant {
        self.clock.now()
    }

    fn elapsed(&self) -> Duration {
        match self.started {
            Some(t) => self.now().saturating_duration_since(t),
            None => Duration::ZERO,
        }
    }

    /// Opens a block for `command`, starting its clock now if `A` never came.
    fn open_block(&mut self, command: String) {
        self.next_id += 1;
        let id = format!("b{}", self.next_id);
        if self.started.is_none() {
            self.started = Some(self.now());
        }
        self.blocks.push(TermBlock::new(id, command, BlockState::Running, live_label(Duration::ZERO)));
        self.open = Some(self.blocks.len() - 1);
        self.partial = false;
    }

    /// The block output goes into, opening the fallback block if needed.
    fn output_block(&mut self) -> usize {
        match self.open {
            Some(i) => i,
            None => {
                self.open_block(String::new());
                self.phase = Phase::Output;
                self.open.expect("just opened")
            }
        }
    }

    /// Appends to the open block's trailing (unterminated) line.
    fn push_output(&mut self, text: &str) {
        let i = self.output_block();
        if !self.partial {
            self.blocks[i].output.push(String::new());
            self.partial = true;
        }
        if let Some(line) = self.blocks[i].output.last_mut() {
            line.push_str(text);
        }
    }

    /// Ends the trailing line. An empty line still counts as a line.
    fn newline(&mut self) {
        let i = self.output_block();
        if !self.partial {
            self.blocks[i].output.push(String::new());
        }
        self.partial = false;
    }

    /// Retimes the running block's duration label.
    fn refresh_live(&mut self) {
        if let Some(i) = self.open {
            if self.blocks[i].state == BlockState::Running {
                let label = live_label(self.elapsed());
                self.blocks[i].duration = label.into();
            }
        }
    }

    /// `OSC 133;A` — a prompt is coming, so whatever ran before is over.
    fn mark_prompt(&mut self) {
        if let Some(i) = self.open {
            if self.blocks[i].state == BlockState::Running {
                // The shell drew a new prompt without reporting a status:
                // treat the command as finished cleanly.
                let label = finished_label(self.elapsed(), 0);
                self.blocks[i].state = BlockState::Done;
                self.blocks[i].duration = label.into();
            }
        }
        self.open = None;
        self.partial = false;
        self.command.clear();
        self.started = Some(self.now());
        self.phase = Phase::Prompt;
    }

    /// `OSC 133;B` — the prompt is drawn; the command line follows.
    fn mark_command(&mut self) {
        self.command.clear();
        self.phase = Phase::Command;
    }

    /// `OSC 133;C` — the command is running; its output follows.
    fn mark_output(&mut self) {
        let command = std::mem::take(&mut self.command).trim_end().to_string();
        self.open_block(command);
        self.phase = Phase::Output;
    }

    /// `OSC 133;D;<exit>` — the command finished.
    fn mark_done(&mut self, exit: i32) {
        if self.open.is_none() {
            // A `D` with no `C`: report the command with no output at all.
            let command = std::mem::take(&mut self.command).trim_end().to_string();
            self.open_block(command);
        }
        if let Some(i) = self.open {
            let label = finished_label(self.elapsed(), exit);
            self.blocks[i].state = if exit == 0 { BlockState::Done } else { BlockState::Failed };
            self.blocks[i].duration = label.into();
        }
        self.open = None;
        self.partial = false;
        self.started = None;
        self.phase = Phase::Idle;
    }
}

impl vte::Perform for Perform {
    fn print(&mut self, c: char) {
        match self.phase {
            Phase::Prompt => {}
            Phase::Command => self.command.push(c),
            Phase::Idle | Phase::Output => {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf).to_string();
                self.push_output(&s);
            }
        }
    }

    fn execute(&mut self, byte: u8) {
        const LF: u8 = 0x0a;
        const CR: u8 = 0x0d;
        const TAB: u8 = 0x09;
        const BS: u8 = 0x08;
        match (byte, self.phase) {
            // zsh's line editor echoes the first character typed, then backs
            // over it and redraws the whole line, so a real shell's command
            // arrives as `e\x08echo hi`. Undo the erase instead of keeping
            // the character it erased.
            (BS, Phase::Command) => {
                self.command.pop();
            }
            (BS, _) => {}
            // A carriage return is layout, not content: card 50 draws one
            // element per line and never redraws one in place.
            (CR, _) => {}
            // The newline that ends the echoed command line is punctuation.
            (LF, Phase::Command | Phase::Prompt) => {}
            (LF, _) => self.newline(),
            (TAB, Phase::Command) => self.command.push('\t'),
            (TAB, Phase::Prompt) => {}
            (TAB, _) => self.push_output("\t"),
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        // Only OSC 133 means anything here; other OSC sequences (window title,
        // clipboard, hyperlinks) carry no visible text and are dropped.
        if params.first().map(|p| *p != b"133".as_slice()).unwrap_or(true) {
            return;
        }
        // When a nonce is set, a marker whose `k=` is absent or does not
        // match is ignored entirely — no block boundary, no state change —
        // so a program printing a bare `OSC 133` cannot forge one.
        if let Some(expected) = self.nonce.as_deref() {
            let got = params.iter().skip(1).find_map(|p| p.strip_prefix(b"k="));
            if got != Some(expected.as_bytes()) {
                return;
            }
        }
        match params.get(1).and_then(|p| p.first()).copied() {
            Some(b'A') => self.mark_prompt(),
            Some(b'B') => self.mark_command(),
            Some(b'C') => self.mark_output(),
            Some(b'D') => {
                let exit = params
                    .get(2)
                    .and_then(|p| std::str::from_utf8(p).ok())
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(0);
                self.mark_done(exit);
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &vte::Params, intermediates: &[u8], _ignore: bool, action: char) {
        // Only SGR survives. `parse_ansi` reads an escape by swallowing
        // everything up to the next `m`, so a cursor-movement or erase
        // sequence left in the line would eat the text after it.
        if action != SGR || !intermediates.is_empty() || matches!(self.phase, Phase::Prompt | Phase::Command) {
            return;
        }
        let mut seq = String::from("\u{1b}[");
        let mut first = true;
        for param in params.iter() {
            if !first {
                seq.push(';');
            }
            first = false;
            for (i, sub) in param.iter().enumerate() {
                if i > 0 {
                    seq.push(':');
                }
                seq.push_str(&sub.to_string());
            }
        }
        seq.push(action);
        self.push_output(&seq);
    }

    /// Two-byte escapes (charset selection, index, save/restore cursor) carry
    /// no text and no colour, and none of them survive into a line.
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use aui::transcript::parse_ansi;

    /// A parser on a clock the test drives by hand.
    fn parser() -> (BlockParser, ManualClock) {
        let clock = ManualClock::new(Instant::now());
        (BlockParser::with_clock(clock.clone()), clock)
    }

    fn a() -> &'static [u8] {
        b"\x1b]133;A\x07"
    }
    fn b() -> &'static [u8] {
        b"\x1b]133;B\x07"
    }
    fn c() -> &'static [u8] {
        b"\x1b]133;C\x07"
    }

    #[test]
    fn osc_133_splits_the_stream_into_blocks() {
        let (mut p, _) = parser();
        p.feed(a());
        p.feed(b"user@host $ ");
        p.feed(b());
        p.feed(b"echo one");
        p.feed(c());
        p.feed(b"one\n");
        p.feed(b"\x1b]133;D;0\x07");
        p.feed(a());
        p.feed(b());
        p.feed(b"echo two");
        p.feed(c());
        p.feed(b"two\n");
        p.feed(b"\x1b]133;D;0\x07");
        assert_eq!(p.blocks().len(), 2);
        assert_eq!(p.blocks()[0].command, "echo one");
        assert_eq!(p.blocks()[1].command, "echo two");
        // The prompt string between A and B never reaches a block.
        assert_eq!(p.blocks()[0].output, vec!["one".to_string()]);
    }

    #[test]
    fn command_text_is_what_sits_between_b_and_c() {
        let (mut p, _) = parser();
        p.feed(a());
        p.feed("~/work/acme \u{276f} ".as_bytes());
        p.feed(b());
        p.feed(b"pnpm vitest run src/checkout");
        p.feed(c());
        p.feed(b"ok\n");
        assert_eq!(p.blocks()[0].command, "pnpm vitest run src/checkout");
    }

    #[test]
    fn exit_zero_is_done_and_non_zero_is_failed() {
        let (mut p, clock) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"true");
        p.feed(c());
        clock.advance(Duration::from_millis(500));
        p.feed(b"\x1b]133;D;0\x07");
        p.feed(a());
        p.feed(b());
        p.feed(b"false");
        p.feed(c());
        p.feed(b"\x1b]133;D;3\x07");
        assert_eq!(p.blocks()[0].state, BlockState::Done);
        assert_eq!(p.blocks()[1].state, BlockState::Failed);
    }

    #[test]
    fn a_failure_labels_its_duration_with_the_exit_code() {
        let (mut p, clock) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"pnpm lint");
        p.feed(c());
        clock.advance(Duration::from_millis(3200));
        p.feed(b"\x1b]133;D;1\x07");
        assert_eq!(p.blocks()[0].duration, "3.2 s · exit 1");
    }

    #[test]
    fn a_success_labels_its_duration_with_one_decimal() {
        let (mut p, clock) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"pnpm i");
        p.feed(c());
        clock.advance(Duration::from_millis(6200));
        p.feed(b"\x1b]133;D;0\x07");
        assert_eq!(p.blocks()[0].duration, "6.2 s");
    }

    #[test]
    fn the_live_block_counts_whole_seconds() {
        let (mut p, clock) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"pnpm vitest");
        p.feed(c());
        clock.advance(Duration::from_millis(12_400));
        p.tick();
        assert_eq!(p.blocks()[0].state, BlockState::Running);
        assert_eq!(p.blocks()[0].duration, "12 s");
    }

    #[test]
    fn sgr_escapes_survive_into_the_output_line() {
        let (mut p, _) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"pnpm lint");
        p.feed(c());
        p.feed(b"\x1b[31m\xe2\x9c\x96\x1b[0m src/checkout/validators.ts\n");
        let line = &p.blocks()[0].output[0];
        assert!(line.starts_with("\u{1b}[31m"), "raw escape kept: {line:?}");
        let spans = parse_ansi(line);
        assert_eq!(spans[0].color, Some(1));
        assert_eq!(spans[0].text, "✖");
        assert_eq!(spans[1].color, None);
    }

    #[test]
    fn with_no_markers_the_whole_stream_is_one_running_block() {
        let (mut p, _) = parser();
        p.feed(b"hello\nworld\n");
        assert_eq!(p.blocks().len(), 1);
        assert_eq!(p.blocks()[0].command, "");
        assert_eq!(p.blocks()[0].state, BlockState::Running);
        assert_eq!(p.blocks()[0].output, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn an_escape_split_across_two_chunks_still_parses() {
        let (mut p, _) = parser();
        // The OSC 133;C marker is cut in half, and so is an SGR sequence.
        p.feed(b"\x1b]133;A\x07\x1b]133;B\x07ls\x1b]13");
        p.feed(b"3;C\x07\x1b[3");
        p.feed(b"2mok\x1b[0m\n");
        assert_eq!(p.blocks().len(), 1);
        assert_eq!(p.blocks()[0].command, "ls");
        assert_eq!(p.blocks()[0].output, vec!["\u{1b}[32mok\u{1b}[0m".to_string()]);
    }

    #[test]
    fn a_trailing_line_with_no_newline_is_still_visible() {
        let (mut p, _) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"tail -f log");
        p.feed(c());
        p.feed(b"done\nrunning");
        assert_eq!(p.blocks()[0].output, vec!["done".to_string(), "running".to_string()]);
        // …and the next chunk extends that same line rather than opening a new one.
        p.feed("…".as_bytes());
        assert_eq!(p.blocks()[0].output, vec!["done".to_string(), "running…".to_string()]);
    }

    #[test]
    fn several_blocks_can_arrive_in_a_single_feed() {
        let (mut p, _) = parser();
        p.feed(
            b"\x1b]133;A\x07\x1b]133;B\x07one\x1b]133;C\x071\n\x1b]133;D;0\x07\
              \x1b]133;A\x07\x1b]133;B\x07two\x1b]133;C\x072\n\x1b]133;D;1\x07\
              \x1b]133;A\x07\x1b]133;B\x07three\x1b]133;C\x073\n",
        );
        let states: Vec<_> = p.blocks().iter().map(|b| b.state).collect();
        assert_eq!(states, vec![BlockState::Done, BlockState::Failed, BlockState::Running]);
        assert_eq!(p.blocks().len(), 3);
    }

    #[test]
    fn carriage_returns_are_dropped_rather_than_starting_a_line() {
        let (mut p, _) = parser();
        p.feed(b"\r\nprogress\r\nmore\r");
        assert_eq!(p.blocks()[0].output, vec!["".to_string(), "progress".to_string(), "more".to_string()]);
    }

    #[test]
    fn a_command_with_no_output_keeps_an_empty_block() {
        let (mut p, clock) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"true");
        p.feed(c());
        clock.advance(Duration::from_millis(100));
        p.feed(b"\x1b]133;D;0\x07");
        assert_eq!(p.blocks().len(), 1);
        assert!(p.blocks()[0].output.is_empty());
        assert_eq!(p.blocks()[0].duration, "0.1 s");
    }

    #[test]
    fn escapes_that_are_not_sgr_are_dropped_from_the_line() {
        let (mut p, _) = parser();
        // Erase-line, cursor-home, hide-cursor and a charset switch all vanish;
        // the SGR around them does not.
        p.feed(b"\x1b[2K\x1b[H\x1b[?25l\x1b(B\x1b[33mwarn\x1b[0m tidy\n");
        assert_eq!(p.blocks()[0].output, vec!["\u{1b}[33mwarn\u{1b}[0m tidy".to_string()]);
        let spans = parse_ansi(&p.blocks()[0].output[0]);
        assert_eq!(spans[0].text, "warn");
        assert_eq!(spans[1].text, " tidy");
    }

    #[test]
    fn an_osc_133_marker_split_mid_parameter_still_dispatches() {
        let (mut p, _) = parser();
        // Every byte of the D marker arrives in its own feed.
        p.feed(a());
        p.feed(b());
        p.feed(b"pnpm lint");
        p.feed(c());
        p.feed(b"boom\n");
        for byte in b"\x1b]133;D;7\x07" {
            p.feed(&[*byte]);
        }
        assert_eq!(p.blocks().len(), 1);
        assert_eq!(p.blocks()[0].state, BlockState::Failed);
        assert!(p.blocks()[0].duration.ends_with("exit 7"), "{:?}", p.blocks()[0].duration);
    }

    #[test]
    fn a_done_marker_with_no_c_still_reports_the_command() {
        let (mut p, clock) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"cd /tmp");
        clock.advance(Duration::from_millis(400));
        // A shell that reports D without ever having reported C (an empty
        // line, or a builtin its integration does not wrap).
        p.feed(b"\x1b]133;D;0\x07");
        assert_eq!(p.blocks().len(), 1);
        assert_eq!(p.blocks()[0].command, "cd /tmp");
        assert_eq!(p.blocks()[0].state, BlockState::Done);
        assert_eq!(p.blocks()[0].duration, "0.4 s");
        assert!(p.blocks()[0].output.is_empty());
    }

    #[test]
    fn a_done_marker_with_no_block_at_all_opens_one() {
        let (mut p, _) = parser();
        p.feed(b"\x1b]133;D;2\x07");
        assert_eq!(p.blocks().len(), 1);
        assert_eq!(p.blocks()[0].command, "");
        assert_eq!(p.blocks()[0].state, BlockState::Failed);
    }

    #[test]
    fn an_osc_0_title_inside_a_block_is_dropped_without_eating_the_output() {
        let (mut p, _) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"vite build");
        p.feed(c());
        p.feed(b"before\n\x1b]0;vite \xe2\x80\x94 building\x07after\n");
        p.feed(b"\x1b]133;D;0\x07");
        assert_eq!(p.blocks().len(), 1);
        assert_eq!(p.blocks()[0].output, vec!["before".to_string(), "after".to_string()]);
    }

    #[test]
    fn an_osc_133_after_an_unterminated_osc_0_still_splits_the_block() {
        let (mut p, _) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"vite build");
        p.feed(c());
        // The title sequence never gets its terminator: the ESC that starts
        // the next OSC ends it, and the 133 marker behind it must still land.
        p.feed(b"out\n\x1b]0;half a title\x1b]133;D;0\x07");
        p.feed(a());
        p.feed(b());
        p.feed(b"echo next");
        p.feed(c());
        p.feed(b"next\n");
        assert_eq!(p.blocks().len(), 2);
        assert_eq!(p.blocks()[0].state, BlockState::Done);
        assert_eq!(p.blocks()[0].output, vec!["out".to_string()]);
        assert_eq!(p.blocks()[1].command, "echo next");
        assert_eq!(p.blocks()[1].output, vec!["next".to_string()]);
    }

    #[test]
    fn an_unterminated_osc_at_the_end_of_the_stream_is_held_not_printed() {
        let (mut p, _) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"echo hi");
        p.feed(c());
        p.feed(b"hi\n\x1b]133;D;0");
        // Nothing has been printed and nothing has been closed yet…
        assert_eq!(p.blocks()[0].state, BlockState::Running);
        assert_eq!(p.blocks()[0].output, vec!["hi".to_string()]);
        // …and the terminator in the next chunk completes the same sequence.
        p.feed(b"\x07");
        assert_eq!(p.blocks()[0].state, BlockState::Done);
    }

    #[test]
    fn the_line_editors_backspace_echo_does_not_double_the_first_character() {
        let (mut p, _) = parser();
        p.feed(a());
        p.feed(b());
        // What a real zsh writes: the first key echoed, erased, then the line.
        p.feed(b"e\x08echo hi");
        p.feed(c());
        p.feed(b"hi\n");
        assert_eq!(p.blocks()[0].command, "echo hi");
    }

    #[test]
    fn a_new_prompt_closes_a_command_that_never_reported_a_status() {
        let (mut p, clock) = parser();
        p.feed(a());
        p.feed(b());
        p.feed(b"vim");
        p.feed(c());
        clock.advance(Duration::from_millis(2000));
        p.feed(a());
        assert_eq!(p.blocks()[0].state, BlockState::Done);
        assert_eq!(p.blocks()[0].duration, "2.0 s");
    }

    /// With a nonce set, a marker with the wrong `k=` and a marker with no
    /// `k=` at all are ignored entirely: no block boundary, no state change.
    #[test]
    fn a_nonce_set_parser_ignores_wrong_and_missing_k() {
        let clock = ManualClock::new(Instant::now());
        let mut p = BlockParser::with_clock(clock).with_nonce("right-nonce");
        assert_eq!(p.nonce(), Some("right-nonce"));
        // Markers alone open no block while their `k=` is missing or wrong:
        // ignored entirely means no boundary and no state change.
        p.feed(b"\x1b]133;A\x07");
        p.feed(b"\x1b]133;B\x07");
        p.feed(b"\x1b]133;C\x07");
        assert!(p.blocks().is_empty(), "{:#?}", p.blocks());
        p.feed(b"\x1b]133;D;0;k=wrong-nonce\x07");
        assert!(p.blocks().is_empty(), "{:#?}", p.blocks());
        // The correctly nonced twin of the same session parses normally.
        p.feed(b"\x1b]133;A;k=right-nonce\x07");
        p.feed(b"\x1b]133;B;k=right-nonce\x07");
        p.feed(b"echo hi");
        p.feed(b"\x1b]133;C;k=right-nonce\x07");
        p.feed(b"hi\n");
        p.feed(b"\x1b]133;D;0;k=right-nonce\x07");
        assert_eq!(p.blocks().len(), 1);
        assert_eq!(p.blocks()[0].command, "echo hi");
        assert_eq!(p.blocks()[0].state, BlockState::Done);
        assert_eq!(p.blocks()[0].output, vec!["hi".to_string()]);
    }

    /// With no nonce set, behaviour is exactly as before: bare markers and
    /// `k=`-carrying markers are both honoured, so scripted replays keep
    /// working unchanged.
    #[test]
    fn without_a_nonce_bare_markers_still_split_blocks() {
        let (mut p, _) = parser();
        assert_eq!(p.nonce(), None);
        p.feed(a());
        p.feed(b());
        p.feed(b"echo one");
        p.feed(c());
        p.feed(b"one\n");
        p.feed(b"\x1b]133;D;0\x07");
        assert_eq!(p.blocks().len(), 1);
        assert_eq!(p.blocks()[0].command, "echo one");
        assert_eq!(p.blocks()[0].state, BlockState::Done);
        let (mut q, _) = parser();
        q.feed(b"\x1b]133;A;k=whatever\x07\x1b]133;B;k=whatever\x07echo two\x1b]133;C;k=whatever\x07two\n\x1b]133;D;0;k=whatever\x07");
        assert_eq!(q.blocks().len(), 1);
        assert_eq!(q.blocks()[0].command, "echo two");
        assert_eq!(q.blocks()[0].state, BlockState::Done);
    }
}
