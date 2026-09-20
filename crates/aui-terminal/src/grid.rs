//! One emulator: [`TerminalSession`] and the [`terminal_grid`] element.
//!
//! [`tui_grid`](crate::tui_grid) wraps an `alacritty_terminal` `Term` with no
//! scrollback, swallowed events and no selection. This module is its
//! replacement: a `Term` with 10 000 lines of scrollback behind
//! [`FairMutex`](alacritty_terminal::sync::FairMutex), a reader thread that
//! drains a boxed [`TerminalBackend`](crate::backend::TerminalBackend) and
//! feeds the `Term` off the UI thread, a channel-based event listener that
//! handles every side-channel event instead of swallowing it, and a gpui
//! element that paints the grid in layers.
//!
//! Decision D44 ("grid is the truth, blocks are an overlay") is why the
//! scrollback lives here: marks and blocks are computed from the same bytes
//! and drawn over this grid by L2.

#![warn(missing_docs)]

use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb};

use crate::backend::{TermEvent, TerminalBackend};
use crate::keys::KeyModes;
use crate::marks::{
    assemble_blocks, Block, BlockAuthor, Mark, MarkScanner, ScanSegment, TextCursor,
};
use crate::parser::{finished_label, live_label};

/// Lines of scrollback behind the visible screen (decision D44).
const SCROLLBACK: usize = 10_000;
/// The backend is drained on this cadence — about twice a display frame.
const POLL_INTERVAL: Duration = Duration::from_millis(8);
/// Smallest grid the session will report, however small the pane is drawn.
const MIN_COLS: u16 = 8;
/// …and fewest rows.
const MIN_ROWS: u16 = 2;
/// Bracketed-paste open/close markers.
const BRACKET_OPEN: &[u8] = b"\x1b[200~";
/// …and close.
const BRACKET_CLOSE: &[u8] = b"\x1b[201~";

/// What the session tells its host outside the grid itself.
///
/// Shaped like [`TerminalIntent`](crate::view::TerminalIntent) — an enum the
/// host matches on — but for the emulator's own side channel: bell, title and
/// exit. Drain with [`TerminalSession::drain_events`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// The program rang the bell.
    Bell,
    /// The program set its title.
    TitleChanged(String),
    /// The program reset its title.
    TitleReset,
    /// The child exited with this status.
    Exited(i32),
}

/// The side-channel events the terminal emulation layer cannot handle itself.
///
/// The reader thread (or [`TerminalSession::pump`]) drains these and acts on
/// them: titles are stored, bells queued, OSC 52 clipboard stores refused and
/// counted, and PTY write-backs (DSR answers and friends) go back to the
/// backend so programs never hang waiting for a reply.
///
/// No `Debug`: three variants carry `Arc<dyn Fn>` formatters, which cannot
/// be formatted.
enum SideEvent {
    /// Store this title.
    Title(String),
    /// Clear the title.
    ResetTitle,
    /// Queue a bell.
    Bell,
    /// A program's OSC 52 clipboard write: refused and counted.
    ClipboardStore,
    /// A program asking for clipboard contents: answered empty.
    ClipboardLoad(Arc<dyn Fn(&str) -> String + Send + Sync + 'static>),
    /// A program asking for a colour value: answered black.
    ColorRequest(Arc<dyn Fn(Rgb) -> String + Send + Sync + 'static>),
    /// Bytes the program wrote for the pty (DSR/cursor-position answers).
    PtyWrite(String),
    /// A program asking for the text-area size: answered from the session.
    TextAreaSizeRequest(Arc<dyn Fn(WindowSize) -> String + Send + Sync + 'static>),
    /// Anything that only needs a repaint.
    Dirty,
    /// The child exited (`None` when the status carries no code).
    ChildExit(Option<i32>),
}

/// The `alacritty_terminal` event listener: a channel, not a swallow.
///
/// Every event becomes a [`SideEvent`] the reader thread handles. The one
/// exception is the repaint marker, which flips the shared dirty flag
/// directly so a repaint is never stuck behind a busy backend.
#[derive(Debug, Clone)]
pub struct SessionEventProxy {
    /// Where side-channel events go.
    tx: mpsc::Sender<SideEvent>,
    /// Flipped on any event that changes what the grid shows.
    shared: Arc<Shared>,
}

impl EventListener for SessionEventProxy {
    fn send_event(&self, event: Event) {
        self.shared.dirty.store(true, Ordering::Release);
        let side = match event {
            Event::Title(title) => SideEvent::Title(title),
            Event::ResetTitle => SideEvent::ResetTitle,
            Event::Bell => SideEvent::Bell,
            Event::ClipboardStore(_, _) => SideEvent::ClipboardStore,
            Event::ClipboardLoad(_, fmt) => SideEvent::ClipboardLoad(fmt),
            Event::ColorRequest(_, fmt) => SideEvent::ColorRequest(fmt),
            Event::PtyWrite(text) => SideEvent::PtyWrite(text),
            Event::TextAreaSizeRequest(fmt) => SideEvent::TextAreaSizeRequest(fmt),
            Event::ChildExit(status) => SideEvent::ChildExit(status.code()),
            Event::Exit => SideEvent::ChildExit(None),
            Event::Wakeup | Event::MouseCursorDirty | Event::CursorBlinkingChange => {
                SideEvent::Dirty
            }
        };
        let _ = self.tx.send(side);
    }
}

/// State shared between the session, its reader thread and its event proxy.
#[derive(Debug, Default)]
struct Shared {
    /// The program's current title, if it set one.
    title: Mutex<Option<String>>,
    /// The child's exit status, once it has one.
    exit: Mutex<Option<i32>>,
    /// Bell/title/exit notifications the host has not drained yet.
    pending: Mutex<Vec<SessionEvent>>,
    /// OSC 52 clipboard writes refused so far.
    refused_clipboard: AtomicUsize,
    /// Set when the grid changed since the last frame.
    dirty: AtomicBool,
    /// Current grid size, for text-area-size answers.
    size: Mutex<(u16, u16)>,
    /// Tells the reader thread to stop.
    stop: AtomicBool,
}

/// The `Term` and the `vte` processor feeding it, behind the fair mutex.
///
/// Both live behind one lock because a processor step borrows the term: the
/// reader thread advances bytes while the UI thread snapshots, never at
/// once.
struct Inner {
    /// The emulator.
    term: Term<SessionEventProxy>,
    /// Turns bytes into terminal actions.
    processor: Processor,
}

/// How the program wants the mouse: forwarded as SGR, or kept local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseReport {
    /// The program did not ask for the mouse: select locally.
    Local,
    /// Clicks (and releases) go to the program.
    Click,
    /// Clicks and drags go to the program.
    Drag,
    /// Every motion goes to the program.
    Motion,
}

/// Which half of an SGR mouse report this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SgrKind {
    /// Press: `…M`.
    Press,
    /// Release: `…m` with button 3.
    Release,
    /// Motion while pressed: `…M` with the motion bit.
    Motion,
}

/// Encodes one SGR (1006) mouse report: `ESC [ < Cb ; Cx ; Cy (M|m)`.
///
/// Columns and rows are zero-based here and become one-based on the wire.
pub fn encode_sgr(button: u8, col: u16, row: u16, kind: SgrKind) -> Vec<u8> {
    let (code, final_byte) = match kind {
        SgrKind::Press => (button, b'M'),
        SgrKind::Release => (3, b'm'),
        SgrKind::Motion => (button + 32, b'M'),
    };
    format!("\x1b[<{code};{};{}{}", col + 1, row + 1, final_byte as char).into_bytes()
}

/// Marks, blocks and the scanner feeding them, shared with the reader thread.
///
/// The scanner watches the same byte stream the emulator sees (see
/// [`feed_with_marks`]), so marks and the grid can never drift apart. Blocks
/// are reassembled from the marks whenever new ones arrive; a resize only
/// flags them stale (see [`TerminalSession::resize`]).
struct MarkState {
    /// The nonce-checked scanner over the session's byte stream.
    scanner: Mutex<MarkScanner>,
    /// Blocks assembled from the marks, oldest first. Append-only by index:
    /// rebuilds carry the host-set authors and surviving command text across.
    blocks: Mutex<Vec<Block>>,
    /// Set by [`TerminalSession::resize`]; the next read rebuilds the blocks
    /// from the marks against the reflowed grid.
    stale: AtomicBool,
}

/// A terminal session: one emulator fed by one backend.
///
/// The host holds this in a `gpui::Entity`. Output arrives on a reader
/// thread ([`start`](Self::start)); the entity's poll timer
/// ([`ensure_polling`](Self::ensure_polling)) turns however much arrived
/// between frames into exactly one coalesced `cx.notify()` per frame. Tests
/// drive the same path synchronously with [`pump`](Self::pump) and never
/// start the thread.
///
/// Every method takes `&self`: the locks inside make sharing cheap, and the
/// element's event closures only ever hold a cloneable reference.
pub struct TerminalSession {
    /// The emulator and its processor behind the fair mutex.
    inner: Arc<FairMutex<Inner>>,
    /// The byte source, shared with the reader thread.
    backend: Arc<Mutex<Box<dyn TerminalBackend + Send>>>,
    /// Side-channel events waiting to be handled, shared with the thread.
    side_rx: Arc<Mutex<mpsc::Receiver<SideEvent>>>,
    /// Title, exit, bells, refusal count and the dirty flag.
    shared: Arc<Shared>,
    /// The reader thread, while it runs.
    reader: Mutex<Option<JoinHandle<()>>>,
    /// The poll timer, while it runs.
    poll: Mutex<Option<gpui::Task<()>>>,
    /// Guard so the timer starts once.
    polling: AtomicBool,
    /// Last measured size in character cells.
    cells: Mutex<(u16, u16)>,
    /// Whether a drag-selection is in progress.
    dragging: AtomicBool,
    /// The cell under the pointer while `⌘` is held, for link underlining.
    hover: Mutex<Option<(usize, usize)>>,
    /// IME marked text waiting to be committed.
    marked: Mutex<Option<String>>,
    /// Last measured element bounds, for mouse mapping.
    bounds: Mutex<Option<gpui::Bounds<gpui::Pixels>>>,
    /// Last measured cell metrics, for mouse mapping.
    metrics: Mutex<Option<CellMetrics>>,
    /// Nonce-checked marks and the blocks over them (decisions D44/D45).
    mark_state: Arc<MarkState>,
}

/// The measured geometry of one mono cell.
#[derive(Debug, Clone, Copy)]
struct CellMetrics {
    /// Cell advance in pixels.
    advance: f32,
    /// Row height in pixels.
    line_height: f32,
}

impl TerminalSession {
    /// A session on `backend` at `cols` × `rows` cells. The backend is not
    /// spawned: call [`spawn_shell`](Self::spawn_shell) or hand it a backend
    /// that is already running (like [`FakePty`](crate::fake::FakePty)).
    pub fn new(backend: Box<dyn TerminalBackend + Send>, cols: u16, rows: u16) -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let proxy = SessionEventProxy { tx, shared: shared.clone() };
        let config = Config { scrolling_history: SCROLLBACK, ..Config::default() };
        let size = TermSize::new(cols.max(1) as usize, rows.max(1) as usize);
        let inner = Inner { term: Term::new(config, &size, proxy), processor: Processor::new() };
        *shared.size.lock().unwrap() = (cols, rows);
        Self {
            inner: Arc::new(FairMutex::new(inner)),
            backend: Arc::new(Mutex::new(backend)),
            side_rx: Arc::new(Mutex::new(rx)),
            shared,
            reader: Mutex::new(None),
            poll: Mutex::new(None),
            polling: AtomicBool::new(false),
            cells: Mutex::new((cols, rows)),
            dragging: AtomicBool::new(false),
            hover: Mutex::new(None),
            marked: Mutex::new(None),
            bounds: Mutex::new(None),
            metrics: Mutex::new(None),
            mark_state: Arc::new(MarkState {
                scanner: Mutex::new(MarkScanner::new(String::new())),
                blocks: Mutex::new(Vec::new()),
                stale: AtomicBool::new(false),
            }),
        }
    }

    /// Pins the session nonce, as a builder: every OSC 133 marker the shell
    /// emits must carry `k=<nonce>` (see [`MarkScanner`]). Prefer this over
    /// [`set_nonce`](Self::set_nonce) when the nonce is known up front —
    /// for example `Pty::nonce` right after spawning.
    pub fn with_nonce(self, nonce: &str) -> Self {
        self.set_nonce(nonce);
        self
    }

    /// Pins the session nonce: every OSC 133 marker must carry `k=<nonce>`
    /// to become a mark. Until the host sets one, all markers are ignored
    /// and no blocks form (decision D45).
    pub fn set_nonce(&self, nonce: &str) {
        self.mark_state.scanner.lock().unwrap().set_nonce(nonce);
    }

    /// Starts the reader thread. Idempotent: the second call does nothing.
    ///
    /// The thread drains the backend, feeds the `Term` and handles
    /// side-channel events off the UI thread. Pair with
    /// [`ensure_polling`](Self::ensure_polling) so the entity re-renders.
    pub fn start(&self) {
        if self.reader.lock().unwrap().is_some() {
            return;
        }
        let inner = self.inner.clone();
        let backend = self.backend.clone();
        // The receiver is drained by `drain_side`, which both the thread and
        // `pump` call under the same lock.
        let side_rx = self.side_rx.clone();
        let shared = self.shared.clone();
        let mark_state = self.mark_state.clone();
        let handle = std::thread::spawn(move || {
            while !shared.stop.load(Ordering::Acquire) {
                let had_output = cycle(&inner, &backend, &side_rx, &shared, &mark_state);
                if had_output {
                    shared.dirty.store(true, Ordering::Release);
                }
                std::thread::sleep(POLL_INTERVAL);
            }
        });
        *self.reader.lock().unwrap() = Some(handle);
    }

    /// Starts the poll timer once. Every tick emits at most one coalesced
    /// `cx.notify()`, however much output arrived between frames. Call it
    /// from the element's render; only the first call does anything.
    pub fn ensure_polling(
        &self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.polling.swap(true, Ordering::AcqRel) {
            return;
        }
        let shared = self.shared.clone();
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            let ok = this.update(cx, |_, cx| {
                if shared.dirty.swap(false, Ordering::AcqRel) {
                    cx.notify();
                }
            });
            if ok.is_err() {
                return;
            }
        });
        *self.poll.lock().unwrap() = Some(task);
    }

    /// Drives one backend poll synchronously: the same cycle the reader
    /// thread runs. Returns `true` when output arrived. Tests use this
    /// instead of [`start`](Self::start); hosts with a running thread must
    /// not call it.
    pub fn pump(&self) -> bool {
        let had_output =
            cycle(&self.inner, &self.backend, &self.side_rx, &self.shared, &self.mark_state);
        if had_output {
            self.shared.dirty.store(true, Ordering::Release);
        }
        had_output
    }

    /// Takes the repaint flag: `true` when the grid changed since the last
    /// call. The poll timer uses this shape; tests assert on it directly.
    pub fn take_dirty(&self) -> bool {
        self.shared.dirty.swap(false, Ordering::AcqRel)
    }

    /// Spawns `shell` in `cwd` on the backend.
    pub fn spawn_shell(&self, shell: &str, cwd: &Path) -> std::io::Result<()> {
        self.backend.lock().unwrap().spawn(shell, cwd)
    }

    /// Sends raw bytes to the program — a typed key, a pasted line, a
    /// signal character.
    pub fn write(&self, bytes: &[u8]) {
        self.backend.lock().unwrap().write(bytes);
    }

    /// Pastes text, bracketed when the program enabled bracketed-paste mode.
    pub fn paste(&self, text: &str) {
        let mut out = Vec::new();
        let bracketed = self.with_term(|t| t.mode().contains(TermMode::BRACKETED_PASTE));
        if bracketed {
            out.extend_from_slice(BRACKET_OPEN);
        }
        out.extend_from_slice(text.as_bytes());
        if bracketed {
            out.extend_from_slice(BRACKET_CLOSE);
        }
        self.write(&out);
    }

    /// Tells the session its new size in character cells: resizes the screen
    /// model and the backend together.
    ///
    /// Reflow may shift recorded marks, so the blocks are flagged stale and
    /// rebuilt from the marks against the reflowed grid on the next read
    /// (decision D44: recompute lazily, accept imperfection). What a user
    /// sees in the imperfect case: chrome anchored a line or two off where
    /// wrapped lines moved, until output arrives and the marks re-anchor.
    pub fn resize(&self, cols: u16, rows: u16) {
        let cols = cols.max(MIN_COLS);
        let rows = rows.max(MIN_ROWS);
        *self.cells.lock().unwrap() = (cols, rows);
        *self.shared.size.lock().unwrap() = (cols, rows);
        self.inner.lock().term.resize(TermSize::new(cols as usize, rows as usize));
        self.backend.lock().unwrap().resize(cols, rows);
        self.mark_state.stale.store(true, Ordering::Release);
    }

    /// The last measured size in character cells.
    pub fn cells(&self) -> (u16, u16) {
        *self.cells.lock().unwrap()
    }

    /// Scrolls the viewport `lines` rows: positive scrolls up into the
    /// scrollback, negative back down toward the tail.
    pub fn scroll_lines(&self, lines: isize) {
        let delta = lines.clamp(i32::MIN as isize, i32::MAX as isize) as i32;
        self.inner.lock().term.scroll_display(Scroll::Delta(delta));
    }

    /// Returns the viewport to the live tail.
    pub fn scroll_to_bottom(&self) {
        self.inner.lock().term.scroll_display(Scroll::Bottom);
    }

    /// Whether the viewport sits at the live tail.
    pub fn is_following_tail(&self) -> bool {
        self.with_term(|t| t.grid().display_offset() == 0)
    }

    /// Starts a selection at screen cell (`col`, `row`).
    pub fn select_start(&self, col: usize, row: usize) {
        self.with_term_mut(|session, t| {
            let point = session.screen_to_grid(t, col, row);
            t.selection = Some(Selection::new(SelectionType::Simple, point, Side::Left));
        });
    }

    /// Extends the selection to screen cell (`col`, `row`). Starts one when
    /// none exists, so a stray move without a press cannot panic.
    pub fn select_extend(&self, col: usize, row: usize) {
        self.with_term_mut(|session, t| {
            let point = session.screen_to_grid(t, col, row);
            match t.selection.as_mut() {
                Some(selection) => selection.update(point, Side::Right),
                None => {
                    t.selection = Some(Selection::new(SelectionType::Simple, point, Side::Left));
                }
            }
        });
    }

    /// Selects the word around screen cell (`col`, `row`): double-click.
    pub fn select_word(&self, col: usize, row: usize) {
        self.with_term_mut(|session, t| {
            let point = session.screen_to_grid(t, col, row);
            let start = t.semantic_search_left(point);
            let end = t.semantic_search_right(point);
            let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
            selection.update(end, Side::Right);
            t.selection = Some(selection);
        });
    }

    /// Selects the whole screen line at `row`: triple-click.
    pub fn select_line(&self, row: usize) {
        self.with_term_mut(|session, t| {
            let point = session.screen_to_grid(t, 0, row);
            t.selection = Some(Selection::new(SelectionType::Lines, point, Side::Left));
        });
    }

    /// Clears the selection, if any.
    pub fn select_clear(&self) {
        self.inner.lock().term.selection = None;
    }

    /// The selected text, if the selection is non-empty.
    pub fn selection_text(&self) -> Option<String> {
        self.with_term(|t| t.selection_to_string())
    }

    /// Whether the program switched to its alternate screen (vim, htop):
    /// the block overlay hides while this is true.
    pub fn alt_screen(&self) -> bool {
        self.with_term(|t| t.mode().contains(TermMode::ALT_SCREEN))
    }

    /// The program's current title, if it set one.
    pub fn title(&self) -> Option<String> {
        self.shared.title.lock().unwrap().clone()
    }

    /// The child's exit status, once it has one.
    pub fn exited(&self) -> Option<i32> {
        *self.shared.exit.lock().unwrap()
    }

    /// OSC 52 clipboard writes refused so far. A program's write never
    /// reaches the pasteboard; this counter proves the refusal happened.
    pub fn refused_clipboard_writes(&self) -> usize {
        self.shared.refused_clipboard.load(Ordering::Acquire)
    }

    /// Takes the queued bell/title/exit notifications for the host to drain.
    pub fn drain_events(&self) -> Vec<SessionEvent> {
        std::mem::take(&mut *self.shared.pending.lock().unwrap())
    }

    /// The accepted marks, oldest first. Rebuilds the blocks first when a
    /// resize flagged them stale, so the ranges match the reflowed grid.
    pub fn marks(&self) -> Vec<Mark> {
        self.refresh_blocks();
        self.mark_state.scanner.lock().unwrap().marks().to_vec()
    }

    /// The blocks assembled from the marks, oldest first. Rebuilds first
    /// when a resize flagged them stale or a block is still running, so a
    /// running block's output tracks the live tail.
    pub fn blocks(&self) -> Vec<Block> {
        self.refresh_blocks();
        self.mark_state.blocks.lock().unwrap().clone()
    }

    /// Labels the block at `index` with `author`: the human typed it, or an
    /// agent ran it. The assembler leaves every block [`BlockAuthor::Human`];
    /// the host upgrades the ones its agent started. Out-of-range indices
    /// do nothing. Authors survive block rebuilds.
    pub fn set_block_author(&self, index: usize, author: BlockAuthor) {
        if let Some(block) = self.mark_state.blocks.lock().unwrap().get_mut(index) {
            block.author = author;
        }
    }

    /// Rebuilds the blocks when the marks went stale (after a resize), or
    /// when a block is still running and must track the live tail.
    fn refresh_blocks(&self) {
        let stale = self.mark_state.stale.swap(false, Ordering::AcqRel);
        let running =
            self.mark_state.blocks.lock().unwrap().iter().any(|b| b.running());
        if stale || running {
            self.with_term(|term| rebuild_blocks(term, &self.mark_state));
        }
    }

    /// ANSI-free text of the block's output lines, or `None` for an
    /// out-of-range block.
    pub fn block_text(&self, block: usize) -> Option<String> {
        self.refresh_blocks();
        let range = self.mark_state.blocks.lock().unwrap().get(block).map(|b| b.output)?;
        Some(self.with_term(|term| grid_range_text(term, range.0, range.1)))
    }

    /// ANSI-free text of the visible grid, top row first, one line per row.
    pub fn screen_text(&self) -> String {
        self.with_term(|term| {
            let offset = term.grid().display_offset() as i32;
            let history = term.grid().history_size() as i32;
            let mut lines = Vec::new();
            let mut row = 0;
            while row < term.screen_lines() {
                let absolute = history + row as i32 - offset;
                lines.push(grid_line_text(term, absolute).unwrap_or_default());
                row += 1;
            }
            lines.join("\n")
        })
    }

    /// ANSI-free text appended after `since`: every retained line past the
    /// cursor, joined with newlines. A host polls this with the cursor from
    /// [`tail_cursor`](Self::tail_cursor) to read what is new; see
    /// [`TextCursor`] for the invalidation rule.
    pub fn range_text(&self, since: TextCursor) -> String {
        self.with_term(|term| {
            let tail = absolute_line(term, term.grid().cursor.point.line);
            let from = since.line.saturating_add(1).max(0);
            if from > tail {
                return String::new();
            }
            grid_range_text(term, from, tail + 1)
        })
    }

    /// The cursor describing everything currently on the grid: `range_text`
    /// past this cursor reads empty until more output arrives.
    pub fn tail_cursor(&self) -> TextCursor {
        self.with_term(|term| TextCursor {
            line: absolute_line(term, term.grid().cursor.point.line),
        })
    }

    /// Chrome for the blocks intersecting the current viewport, oldest first:
    /// one entry per block the overlay draws. Empty while
    /// [`alt_screen`](Self::alt_screen) is true — a fullscreen program owns
    /// every row, so the overlay hides entirely.
    fn overlay_chrome(&self) -> Vec<BlockChrome> {
        self.with_term(|term| {
            if term.mode().contains(TermMode::ALT_SCREEN) {
                return Vec::new();
            }
            self.refresh_blocks_under(term);
            let history = term.grid().history_size() as i32;
            let offset = term.grid().display_offset() as i32;
            let rows = term.screen_lines() as i32;
            let top = history - offset;
            let now = std::time::Instant::now();
            self.mark_state
                .blocks
                .lock()
                .unwrap()
                .iter()
                .enumerate()
                .filter_map(|(index, block)| {
                    let bottom = top + rows;
                    if block.end < top || block.prompt.0 >= bottom {
                        return None;
                    }
                    let row = block.end.clamp(top, bottom - 1) - top;
                    let failed = block.exit.is_some_and(|exit| exit != 0);
                    let label = match (block.exit, block.ended) {
                        (Some(exit), Some(ended)) => {
                            finished_label(ended.saturating_duration_since(block.started), exit)
                        }
                        _ => live_label(now.saturating_duration_since(block.started)),
                    };
                    let glyph = if block.running() {
                        "●"
                    } else if failed {
                        "✗"
                    } else {
                        "✓"
                    };
                    Some(BlockChrome {
                        index,
                        row: row as usize,
                        glyph,
                        label,
                        author: block.author,
                        running: block.running(),
                        failed,
                    })
                })
                .collect()
        })
    }

    /// [`refresh_blocks`](Self::refresh_blocks) when the term lock is already
    /// held: rebuilds on stale marks or a running block.
    fn refresh_blocks_under(&self, term: &Term<SessionEventProxy>) {
        let stale = self.mark_state.stale.swap(false, Ordering::AcqRel);
        let running =
            self.mark_state.blocks.lock().unwrap().iter().any(|b| b.running());
        if stale || running {
            rebuild_blocks(term, &self.mark_state);
        }
    }

    /// Runs `f` on the `Term` under the fair mutex. `f` must not call back
    /// into the session: the lock does not re-enter.
    pub fn with_term<R>(&self, f: impl FnOnce(&Term<SessionEventProxy>) -> R) -> R {
        f(&self.inner.lock().term)
    }

    /// Runs `f` on the `Term` mutably under the fair mutex. Like
    /// [`with_term`](Self::with_term), `f` must not call back into the
    /// session — but it may call the session's `&self` helpers that only
    /// read, via the `&TerminalSession` it is handed.
    fn with_term_mut<R>(&self, f: impl FnOnce(&Self, &mut Term<SessionEventProxy>) -> R) -> R {
        let mut guard = self.inner.lock();
        f(self, &mut guard.term)
    }

    /// The key modes the encoder needs: DECCKM and keypad state.
    pub fn key_modes(&self) -> KeyModes {
        self.with_term(|t| KeyModes {
            app_cursor: t.mode().contains(TermMode::APP_CURSOR),
            app_keypad: t.mode().contains(TermMode::APP_KEYPAD),
        })
    }

    /// How the program wants the mouse right now.
    pub fn mouse_report(&self) -> MouseReport {
        self.with_term(|t| {
            let mode = t.mode();
            if mode.contains(TermMode::MOUSE_MOTION) {
                MouseReport::Motion
            } else if mode.contains(TermMode::MOUSE_DRAG) {
                MouseReport::Drag
            } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
                MouseReport::Click
            } else {
                MouseReport::Local
            }
        })
    }

    /// Whether the program asked for SGR (1006) mouse encoding.
    pub fn sgr_mouse(&self) -> bool {
        self.with_term(|t| t.mode().contains(TermMode::SGR_MOUSE))
    }

    /// Maps a screen cell to grid coordinates, accounting for the scrollback
    /// offset: the visible top row is history while scrolled up.
    fn screen_to_grid(&self, term: &Term<SessionEventProxy>, col: usize, row: usize) -> Point {
        let offset = term.grid().display_offset() as i32;
        let columns = term.columns().max(1);
        Point::new(Line(row as i32 - offset), Column(col.min(columns - 1)))
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(handle) = self.reader.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
}

/// One shared drain cycle: backend events into the `Term`, side-channel
/// events into storage or back into the backend. Both the reader thread and
/// [`TerminalSession::pump`] run this. Returns `true` when output arrived.
fn cycle(
    inner: &Arc<FairMutex<Inner>>,
    backend: &Arc<Mutex<Box<dyn TerminalBackend + Send>>>,
    side_rx: &Mutex<mpsc::Receiver<SideEvent>>,
    shared: &Arc<Shared>,
    mark_state: &Arc<MarkState>,
) -> bool {
    let mut had_output = false;
    for event in backend.lock().unwrap().poll() {
        match event {
            TermEvent::Output(bytes) => {
                had_output = true;
                // Disjoint fields through one deref: the only way to feed
                // the term without unlocking between processor and screen.
                let faced: &mut Inner = &mut inner.lock();
                feed_with_marks(faced, &bytes, mark_state);
            }
            TermEvent::Exit(code) => {
                *shared.exit.lock().unwrap() = Some(code);
                shared.pending.lock().unwrap().push(SessionEvent::Exited(code));
                shared.dirty.store(true, Ordering::Release);
            }
        }
    }
    drain_side(inner, backend, side_rx, shared);
    had_output
}

/// Feeds `bytes` to the emulator and the mark scanner together: the chunk is
/// split at complete OSC 133 boundaries, every piece goes to the `Term` in
/// order, and each accepted marker is recorded at the emulator's absolute
/// line at that point — the cursor has not moved for the marker's own bytes,
/// so the line is the shell's line. Afterwards the blocks are reassembled.
fn feed_with_marks(faced: &mut Inner, bytes: &[u8], state: &MarkState) {
    let segments = state.scanner.lock().unwrap().split_feed(bytes);
    if segments.is_empty() {
        return;
    }
    for segment in segments {
        match segment {
            ScanSegment::Emit(raw) => {
                faced.processor.advance(&mut faced.term, &raw);
            }
            ScanSegment::Marker { raw, kind, exit } => {
                faced.processor.advance(&mut faced.term, &raw);
                let line = absolute_line(&faced.term, faced.term.grid().cursor.point.line);
                let at = std::time::Instant::now();
                state.scanner.lock().unwrap().push_mark(Mark { line, kind, exit, at });
            }
        }
    }
    rebuild_blocks(&faced.term, state);
}

/// The absolute grid line for a screen-relative [`Line`]: history lines
/// behind plus the line itself. Absolute 0 is the oldest retained line.
fn absolute_line(term: &Term<SessionEventProxy>, line: Line) -> i32 {
    term.grid().history_size() as i32 + line.0
}

/// Reassembles the blocks from the accepted marks against the current grid,
/// carrying the host-set authors (and surviving command text) across by
/// index — blocks only ever grow, so indices are stable.
fn rebuild_blocks(term: &Term<SessionEventProxy>, state: &MarkState) {
    let marks: Vec<Mark> = state.scanner.lock().unwrap().marks().to_vec();
    if marks.is_empty() {
        return;
    }
    let tail = absolute_line(term, term.grid().cursor.point.line);
    let mut fresh = assemble_blocks(&marks, tail, &|a, b| grid_range_text(term, a, b));
    let mut stored = state.blocks.lock().unwrap();
    for (i, block) in fresh.iter_mut().enumerate() {
        if let Some(old) = stored.get(i) {
            block.author = old.author;
            if block.command.is_empty() && !old.command.is_empty() {
                // The lines scrolled past the retained window: keep what the
                // grid can no longer tell us.
                block.command.clone_from(&old.command);
            }
        }
    }
    *stored = fresh;
}

/// The trimmed, ANSI-free text of one absolute grid line, or `None` when the
/// line is older than the retained scrollback or past the live tail.
fn grid_line_text(term: &Term<SessionEventProxy>, absolute: i32) -> Option<String> {
    use alacritty_terminal::term::cell::Flags;
    let history = term.grid().history_size() as i32;
    let line = Line(absolute - history);
    if line.0 < term.grid().topmost_line().0 || line.0 > term.grid().bottommost_line().0 {
        return None;
    }
    let mut out = String::new();
    for col in 0..term.columns() {
        let cell = &term.grid()[Point::new(line, Column(col))];
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        out.push(if cell.c == '\0' { ' ' } else { cell.c });
    }
    Some(out.trim_end().to_string())
}

/// The ANSI-free text of absolute lines `start..end`, one per line.
/// Lines outside the retained window read as empty.
fn grid_range_text(term: &Term<SessionEventProxy>, start: i32, end: i32) -> String {
    let from = start.max(0);
    let mut lines = Vec::new();
    let mut line = from;
    while line < end {
        lines.push(grid_line_text(term, line).unwrap_or_default());
        line += 1;
    }
    lines.join("\n")
}

/// Handles everything the [`SessionEventProxy`] collected.
fn drain_side(
    _inner: &Arc<FairMutex<Inner>>,
    backend: &Arc<Mutex<Box<dyn TerminalBackend + Send>>>,
    side_rx: &Mutex<mpsc::Receiver<SideEvent>>,
    shared: &Arc<Shared>,
) {
    let events: Vec<SideEvent> = {
        let rx = side_rx.lock().unwrap();
        let mut out = Vec::new();
        while let Ok(event) = rx.try_recv() {
            out.push(event);
        }
        out
    };
    for event in events {
        match event {
            SideEvent::Title(title) => {
                *shared.title.lock().unwrap() = Some(title.clone());
                shared.pending.lock().unwrap().push(SessionEvent::TitleChanged(title));
            }
            SideEvent::ResetTitle => {
                *shared.title.lock().unwrap() = None;
                shared.pending.lock().unwrap().push(SessionEvent::TitleReset);
            }
            SideEvent::Bell => {
                shared.pending.lock().unwrap().push(SessionEvent::Bell);
            }
            SideEvent::ClipboardStore => {
                // Refuse: the bytes never reach the pasteboard (D53).
                shared.refused_clipboard.fetch_add(1, Ordering::AcqRel);
            }
            SideEvent::ClipboardLoad(format) => {
                // Refuse with an empty answer so the program does not hang.
                backend.lock().unwrap().write(format("").as_bytes());
            }
            SideEvent::ColorRequest(format) => {
                backend.lock().unwrap().write(format(Rgb { r: 0, g: 0, b: 0 }).as_bytes());
            }
            SideEvent::PtyWrite(text) => {
                // DSR and cursor-position answers: without the write-back
                // fullscreen programs hang waiting for their own reply.
                backend.lock().unwrap().write(text.as_bytes());
            }
            SideEvent::TextAreaSizeRequest(format) => {
                let (cols, rows) = *shared.size.lock().unwrap();
                let answer = format(WindowSize {
                    num_lines: rows,
                    num_cols: cols,
                    cell_width: 0,
                    cell_height: 0,
                });
                backend.lock().unwrap().write(answer.as_bytes());
            }
            SideEvent::Dirty => {
                shared.dirty.store(true, Ordering::Release);
            }
            SideEvent::ChildExit(code) => {
                let code = code.unwrap_or(-1);
                *shared.exit.lock().unwrap() = Some(code);
                shared.pending.lock().unwrap().push(SessionEvent::Exited(code));
                shared.dirty.store(true, Ordering::Release);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot: the owned grid the element paints.
// ---------------------------------------------------------------------------

/// Underline thickness for cell and link underlines, in px. The palette
/// carries no hairline token; the transcript's prose uses the same 1 px.
const UNDERLINE_W: f32 = 1.0;
/// Strikethrough thickness, in px.
const STRIKE_W: f32 = 1.0;
/// The grid's text size, in px (the TUI pane's `.tui` size).
const TERM_PX: f32 = 12.0;
/// The grid's line height: the design's mono rhythm.
/// Rationale: `docs/04-design-rules.md` sets mono to 1.6.
const TERM_LH: f32 = 1.6;
/// Bar-cursor and underline-cursor thickness, in px.
const CURSOR_BAR_W: f32 = 2.0;
/// Inset of the jump-to-latest affordance from the pane corner, in px.
const JUMP_OFFSET: f32 = 8.0;

/// One background rect: byte range `start..end` of the row in `color`.
#[derive(Debug, Clone)]
struct BgSpan {
    /// Start byte offset in the row text.
    start: usize,
    /// End byte offset in the row text.
    end: usize,
    /// The fill.
    color: gpui::Hsla,
}

/// One hyperlink: byte range `start..end` opening `url` on `⌘`-click.
#[derive(Debug, Clone)]
struct LinkSpan {
    /// Start byte offset in the row text.
    start: usize,
    /// End byte offset in the row text.
    end: usize,
    /// OSC 8 URI or detected URL.
    url: String,
}

/// One painted row: text plus the runs, background rects and links over it.
#[derive(Debug, Clone)]
struct GridRow {
    /// The row's characters.
    text: String,
    /// Batched same-style runs covering `text` exactly.
    runs: Vec<gpui::TextRun>,
    /// Background rects (layer 1).
    bgs: Vec<BgSpan>,
    /// Hyperlinks and detected URLs.
    links: Vec<LinkSpan>,
}

/// The cursor as a screen cell plus its DECSCUSR shape.
#[derive(Debug, Clone, Copy)]
struct CursorCell {
    /// Screen row.
    row: usize,
    /// Screen column.
    col: usize,
    /// Block, bar or underline.
    shape: CursorShape,
}

/// One block's overlay chrome, owned so the term lock is released before any
/// element is built. Only blocks intersecting the viewport are returned.
#[derive(Debug, Clone)]
struct BlockChrome {
    /// Index into the session's block list; intents carry this back.
    index: usize,
    /// Screen row anchoring the chrome (the block's end line, clamped).
    row: usize,
    /// Gutter status glyph: running, failed or done.
    glyph: &'static str,
    /// Exit code and duration, via the parser's shared labels.
    label: String,
    /// Who started the block.
    author: BlockAuthor,
    /// Whether the block is still running.
    running: bool,
    /// Whether the block exited non-zero.
    failed: bool,
}

/// Everything [`TerminalGrid`] paints, owned so the term lock is released
/// before any element is built.
#[derive(Debug, Clone)]
struct GridSnapshot {
    /// Top to bottom, one entry per visible row.
    rows: Vec<GridRow>,
    /// The cursor in screen coordinates, unless the program hid it.
    cursor: Option<CursorCell>,
    /// Whether the viewport left the tail (shows "Jump to latest").
    detached: bool,
}

/// A cell foreground on the design palette: ANSI 0–15 from
/// [`Palette::ansi16`](aui_tokens::Palette::ansi16), 16–231 from the
/// 256-colour cube, 232–255 from the grayscale ramp, true colour as itself.
fn resolve_fg(
    color: Color,
    ansi: &[gpui::Hsla; 16],
    palette: &aui_tokens::Palette,
    dim: bool,
) -> gpui::Hsla {
    match color {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) if (i as usize) < 16 => ansi[i as usize],
        Color::Indexed(i) if i < 232 => cube(i - 16),
        Color::Indexed(i) => gray(i - 232),
        Color::Named(named) => match named {
            NamedColor::Background => palette.term_bg,
            NamedColor::Cursor => palette.term_cursor,
            NamedColor::Foreground if dim => palette.term_dim,
            NamedColor::Foreground | NamedColor::BrightForeground => palette.term_fg,
            NamedColor::DimForeground => palette.term_dim,
            other => {
                let i = other as usize;
                if i < 16 { ansi[i] } else { palette.term_dim }
            }
        },
    }
}

/// A cell background. The default background is unset so the pane's own
/// `term-bg` shows through rather than being painted per character.
fn resolve_bg(
    color: Color,
    ansi: &[gpui::Hsla; 16],
    palette: &aui_tokens::Palette,
) -> Option<gpui::Hsla> {
    match color {
        Color::Named(NamedColor::Background) => None,
        other => Some(resolve_fg(other, ansi, palette, false)),
    }
}

/// The 6×6×6 colour cube for indices 16–231.
fn cube(i: u8) -> gpui::Hsla {
    let v = i as f32;
    let r = (v / 36.0).floor();
    let g = ((v % 36.0) / 6.0).floor();
    let b = v % 6.0;
    let level = |c: f32| {
        if c == 0.0 { 0.0 } else { (55.0 + 40.0 * c) / 255.0 }
    };
    gpui::Rgba { r: level(r), g: level(g), b: level(b), a: 1.0 }.into()
}

/// The 24-step grayscale ramp for indices 232–255.
fn gray(i: u8) -> gpui::Hsla {
    let l = (8.0 + 10.0 * i as f32) / 255.0;
    gpui::Rgba { r: l, g: l, b: l, a: 1.0 }.into()
}

/// An `Rgb` cell colour as itself.
fn rgb(c: Rgb) -> gpui::Hsla {
    gpui::Rgba { r: c.r as f32 / 255.0, g: c.g as f32 / 255.0, b: c.b as f32 / 255.0, a: 1.0 }
        .into()
}

/// The mono font for a run: semibold for bold, italic for italic.
fn run_font(bold: bool, italic: bool) -> gpui::Font {
    let mut f = gpui::font(aui_tokens::scale::FONT_MONO);
    if bold {
        f.weight = gpui::FontWeight::SEMIBOLD;
    }
    if italic {
        f.style = gpui::FontStyle::Italic;
    }
    f
}

impl TerminalSession {
    /// Snapshots the visible grid onto `palette`: owned rows the element
    /// paints without holding the term lock.
    fn snapshot(&self, palette: &aui_tokens::Palette) -> GridSnapshot {
        let ansi = palette.ansi16();
        let marked = self.marked.lock().unwrap().clone();
        let hover = *self.hover.lock().unwrap();
        self.with_term(|term| {
            let content = term.renderable_content();
            let disp = content.display_offset as i32;
            let cols = term.columns();
            let selection = content.selection;
            // Selected columns per screen row, mapped back into screen
            // coordinates the same way the cursor is.
            let selected = |row: usize| -> Option<(usize, usize)> {
                let range = selection?;
                let mut start = range.start;
                let mut end = range.end;
                if start > end {
                    std::mem::swap(&mut start, &mut end);
                }
                let top = start.line.0 + disp;
                let bottom = end.line.0 + disp;
                // Parenthesised: `as` casts followed by `<` parse as generic
                // arguments without them.
                if (row as i32) < top || (row as i32) > bottom {
                    return None;
                }
                let from = if row as i32 == top { start.column.0 } else { 0 };
                let to = if row as i32 == bottom { end.column.0 } else { cols.saturating_sub(1) };
                (from <= to).then_some((from, to.min(cols.saturating_sub(1))))
            };
            // Group the display iterator's cells into screen rows.
            let mut rows: Vec<Vec<(usize, alacritty_terminal::term::cell::Cell)>> = Vec::new();
            let mut cur: Option<i32> = None;
            for indexed in content.display_iter {
                let line = indexed.point.line.0 + disp;
                if cur != Some(line) {
                    rows.push(Vec::new());
                    cur = Some(line);
                }
                let col = indexed.point.column.0;
                if let Some(row) = rows.last_mut() {
                    row.push((col, indexed.cell.clone()));
                }
            }
            let mut out = Vec::with_capacity(rows.len());
            for (row_ix, cells) in rows.iter().enumerate() {
                out.push(render_row(cells, selected(row_ix), palette, &ansi, hover, row_ix));
            }
            let cursor = match content.cursor.shape {
                CursorShape::Hidden => None,
                shape => {
                    let row = content.cursor.point.line.0 + disp;
                    let col = content.cursor.point.column.0;
                    (row >= 0 && (row as usize) < out.len() && col < cols)
                        .then_some(CursorCell { row: row as usize, col, shape })
                }
            };
            let mut snapshot =
                GridSnapshot { rows: out, cursor, detached: content.display_offset > 0 };
            // IME marked text draws at the cursor.
            if let (Some(text), Some(cursor)) = (marked, snapshot.cursor) {
                if let Some(row) = snapshot.rows.get_mut(cursor.row) {
                    insert_marked(row, cursor.col, &text, palette);
                }
            }
            snapshot
        })
    }

    /// The IME marked text waiting to be committed, if any.
    pub fn marked_text(&self) -> Option<String> {
        self.marked.lock().unwrap().clone()
    }

    /// Records the element bounds and cell metrics from the layout probe, so
    /// mouse positions map onto cells.
    fn note_layout(
        &self,
        bounds: gpui::Bounds<gpui::Pixels>,
        metrics: CellMetrics,
        cols: u16,
        rows: u16,
    ) {
        *self.bounds.lock().unwrap() = Some(bounds);
        *self.metrics.lock().unwrap() = Some(metrics);
        if self.cells() != (cols, rows) {
            self.resize(cols, rows);
        }
    }

    /// Maps a window position onto a screen cell, if the probe measured one.
    fn cell_at(&self, position: gpui::Point<gpui::Pixels>) -> Option<(usize, usize)> {
        let bounds = (*self.bounds.lock().unwrap())?;
        let metrics = (*self.metrics.lock().unwrap())?;
        if !bounds.contains(&position) {
            return None;
        }
        let x = f32::from(position.x - bounds.origin.x);
        let y = f32::from(position.y - bounds.origin.y);
        if x < 0.0 || y < 0.0 || metrics.advance <= 0.0 || metrics.line_height <= 0.0 {
            return None;
        }
        let (cols, rows) = self.cells();
        let col = ((x / metrics.advance).floor() as usize).min(cols.saturating_sub(1) as usize);
        let row = ((y / metrics.line_height).floor() as usize).min(rows.saturating_sub(1) as usize);
        Some((col, row))
    }
}

/// Renders one screen row: batched same-style runs, background spans and
/// link spans over the row text.
fn render_row(
    cells: &[(usize, alacritty_terminal::term::cell::Cell)],
    selected: Option<(usize, usize)>,
    palette: &aui_tokens::Palette,
    ansi: &[gpui::Hsla; 16],
    hover: Option<(usize, usize)>,
    row_ix: usize,
) -> GridRow {
    use alacritty_terminal::term::cell::Flags;
    let mut text = String::new();
    let mut runs: Vec<gpui::TextRun> = Vec::new();
    let mut bgs: Vec<BgSpan> = Vec::new();
    let mut links: Vec<LinkSpan> = Vec::new();
    let mut last_key: Option<(gpui::Hsla, Option<gpui::Hsla>, bool, bool, bool, bool)> = None;
    // Hover underline: the ⌘-held cell's link, if any.
    let hover_url = hover.and_then(|(col, row)| {
        (row == row_ix)
            .then(|| cells.iter().find(|(c, _)| *c == col))
            .flatten()
            .and_then(|(_, cell)| cell.hyperlink())
            .map(|link| link.uri().to_string())
    });
    for (col, cell) in cells {
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let bold = cell.flags.contains(Flags::BOLD);
        let italic = cell.flags.contains(Flags::ITALIC);
        let underline_flag =
            cell.flags.intersects(Flags::UNDERLINE | Flags::DOUBLE_UNDERLINE | Flags::DOTTED_UNDERLINE | Flags::DASHED_UNDERLINE);
        let strike = cell.flags.contains(Flags::STRIKEOUT);
        let dim = cell.flags.contains(Flags::DIM);
        let inverse = cell.flags.contains(Flags::INVERSE);
        let hidden = cell.flags.contains(Flags::HIDDEN);
        let base_fg = resolve_fg(cell.fg, ansi, palette, dim);
        let base_bg = resolve_bg(cell.bg, ansi, palette);
        let (mut fg, mut bg) = if inverse {
            (base_bg.unwrap_or(palette.term_bg), Some(base_fg))
        } else {
            (base_fg, base_bg)
        };
        if hidden {
            fg = bg.unwrap_or(palette.term_bg);
        }
        let link = cell.hyperlink().map(|l| l.uri().to_string());
        let underlined = underline_flag
            || link.as_ref().is_some_and(|url| Some(url) == hover_url.as_ref());
        if let Some((from, to)) = selected {
            if *col >= from && *col <= to {
                bg = Some(palette.surface_3);
            }
        }
        if let Some(color) = bg {
            let byte = text.len();
            match bgs.last_mut() {
                Some(span) if span.end == byte && span.color == color => span.end += 1,
                // Widened below once the character length is known.
                _ => bgs.push(BgSpan { start: byte, end: byte, color }),
            }
        }
        if let Some(url) = link.clone() {
            let byte = text.len();
            match links.last_mut() {
                Some(span) if span.end == byte && span.url == url => span.end += 1,
                _ => links.push(LinkSpan { start: byte, end: byte, url }),
            }
        }
        let c = if cell.c == '\0' { ' ' } else { cell.c };
        let key = (fg, bg, bold, italic, underlined, strike);
        if last_key == Some(key) {
            if let Some(run) = runs.last_mut() {
                run.len += c.len_utf8();
            }
        } else {
            runs.push(gpui::TextRun {
                len: c.len_utf8(),
                font: run_font(bold, italic),
                color: fg,
                background_color: None,
                underline: underlined.then(|| gpui::UnderlineStyle {
                    thickness: gpui::px(UNDERLINE_W),
                    color: Some(fg),
                    wavy: false,
                }),
                strikethrough: strike.then(|| gpui::StrikethroughStyle {
                    thickness: gpui::px(STRIKE_W),
                    color: Some(fg),
                }),
            });
            last_key = Some(key);
        }
        text.push(c);
        // Backfill the span ends opened above with the real byte length.
        if let Some(span) = bgs.last_mut() {
            if span.end == text.len() - c.len_utf8() {
                span.end = text.len();
            }
        }
        if let Some(span) = links.last_mut() {
            if span.end == text.len() - c.len_utf8() {
                span.end = text.len();
            }
        }
    }
    // Plain detected URLs underline like OSC 8 links (without a stored URI
    // they open as-is on ⌘-click).
    for (start, end) in find_urls(&text) {
        if links.iter().any(|span| span.start < end && start < span.end) {
            continue;
        }
        // Underline the run range.
        apply_underline(&mut runs, start, end);
        links.push(LinkSpan { start, end, url: text[start..end].to_string() });
    }
    // Runs cover the text exactly, even for an empty row.
    if runs.is_empty() {
        runs.push(gpui::TextRun {
            len: 0,
            font: run_font(false, false),
            color: palette.term_fg,
            background_color: None,
            underline: None,
            strikethrough: None,
        });
    }
    debug_assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), text.len());
    GridRow { text, runs, bgs, links }
}

/// Draws IME marked text into the row at the cursor column, underlined.
fn insert_marked(row: &mut GridRow, col: usize, text: &str, palette: &aui_tokens::Palette) {
    let at = col_to_byte(row, col);
    row.text.insert_str(at, text);
    let run = gpui::TextRun {
        len: text.len(),
        font: run_font(false, false),
        color: palette.term_fg,
        background_color: None,
        underline: Some(gpui::UnderlineStyle {
            thickness: gpui::px(UNDERLINE_W),
            color: Some(palette.term_fg),
            wavy: false,
        }),
        strikethrough: None,
    };
    // Split the run covering `at` so the marked text carries its own run.
    let mut out = Vec::with_capacity(row.runs.len() + 2);
    let mut kept = 0usize;
    let mut inserted = false;
    for mut existing in std::mem::take(&mut row.runs) {
        if !inserted && at >= kept && at <= kept + existing.len {
            let before = at - kept;
            if before > 0 {
                out.push(gpui::TextRun { len: before, ..existing.clone() });
                existing.len -= before;
            }
            out.push(run.clone());
            inserted = true;
        }
        out.push(existing.clone());
        kept += existing.len;
    }
    if !inserted {
        out.push(run);
    }
    row.runs = out;
}

/// Byte offset of a screen column in a row's text.
fn col_to_byte(row: &GridRow, col: usize) -> usize {
    for (current, (ix, _)) in row.text.char_indices().enumerate() {
        if current == col {
            return ix;
        }
    }
    row.text.len()
}

/// Underlines `start..end` in place, splitting runs as needed.
fn apply_underline(runs: &mut [gpui::TextRun], start: usize, end: usize) {
    let mut kept = 0usize;
    for run in runs.iter_mut() {
        let run_end = kept + run.len;
        if run_end > start && kept < end {
            run.underline = Some(gpui::UnderlineStyle {
                thickness: gpui::px(UNDERLINE_W),
                color: Some(run.color),
                wavy: false,
            });
        }
        kept = run_end;
    }
}

/// Finds `http://` and `https://` URLs: start offset and end offset pairs.
fn find_urls(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut ix = 0usize;
    while ix < bytes.len() {
        let rest = &text[ix..];
        let offset =
            [rest.find("http://"), rest.find("https://")].into_iter().flatten().min();
        let Some(matched) = offset else { break };
        let start = ix + matched;
        let mut end = start;
        while end < text.len() {
            let c = text[end..].chars().next().unwrap();
            if c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | ']') {
                break;
            }
            end += c.len_utf8();
        }
        if end > start {
            out.push((start, end));
        }
        ix = end.max(start + 1);
    }
    out
}

// ---------------------------------------------------------------------------
// Element: terminal_grid.
// ---------------------------------------------------------------------------

use std::rc::Rc;

use aui_tokens::{ActiveAui, AuiStyled};

/// What [`TerminalGrid`] asks its host for. The component is stateless; the
/// host owns the [`TerminalSession`] entity and performs the intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalGridIntent {
    /// `⌘`-clicked a hyperlink or detected URL: open it.
    OpenUrl(gpui::SharedString),
    /// Copies the block's output text. The payload is the block index into
    /// [`TerminalSession::blocks`]; the host reads it back with
    /// [`TerminalSession::block_text`] and performs the copy.
    Copy(usize),
    /// Re-runs the block's command. The host reads the command back from
    /// [`TerminalSession::blocks`] and decides how to run it.
    Rerun(usize),
    /// Stops the running block. The host decides how (a `Ctrl-C` write, for
    /// example); raising this on a finished block is a no-op for the host.
    Stop(usize),
    /// Asks about the block: the host opens whatever surfaces the block's
    /// output for questioning.
    Ask(usize),
}

/// The grid's intent handler: what [`TerminalGrid::on_intent`] stores.
type IntentHandler = Rc<dyn Fn(TerminalGridIntent, &mut gpui::Window, &mut gpui::App)>;

/// The one-emulator grid over a [`TerminalSession`]. Build with
/// [`terminal_grid`].
///
/// Paints in four layers, in order: row background rects, batched
/// same-style text runs, the cursor (block, bar or underline per DECSCUSR,
/// hollow when unfocused), and the overlay: per-block chrome (gutter status
/// glyph, exit code and duration, author mark, action row) plus the
/// jump-to-latest affordance while detached. The overlay hides entirely while
/// the program owns the alternate screen (decision D44).
#[derive(gpui::IntoElement)]
pub struct TerminalGrid {
    session: gpui::Entity<TerminalSession>,
    option_as_meta: bool,
    on_intent: Option<IntentHandler>,
}

/// The terminal grid fed by `session` (held in a gpui `Entity` the host
/// owns). State lives in the session; this component only renders.
pub fn terminal_grid(session: &gpui::Entity<TerminalSession>) -> TerminalGrid {
    TerminalGrid { session: session.clone(), option_as_meta: false, on_intent: None }
}

impl TerminalGrid {
    /// Treats Option as Meta for key encoding (the macOS terminal setting).
    pub fn option_as_meta(mut self, option_as_meta: bool) -> Self {
        self.option_as_meta = option_as_meta;
        self
    }

    /// Called for every [`TerminalGridIntent`] the grid raises.
    pub fn on_intent(
        mut self,
        f: impl Fn(TerminalGridIntent, &mut gpui::Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl gpui::RenderOnce for TerminalGrid {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        use gpui::{div, prelude::*, px};
        // The session owns the reader thread and the poll timer; both start
        // once, here, so mounting the element is all the host does.
        self.session.update(cx, |session, cx| {
            session.start();
            session.ensure_polling(window, cx);
        });
        let palette = cx.aui().colors;
        // Layer 0, measured once: the mono cell from the theme's mono font
        // through the window's own text system, so it matches the paint.
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&gpui::font(aui_tokens::scale::FONT_MONO));
        let advance =
            text_system.ch_advance(font_id, px(TERM_PX)).map(f32::from).unwrap_or(0.0);
        let line_height = TERM_PX * TERM_LH;
        let snapshot = self.session.read(cx).snapshot(&palette);
        let focus = cx.focus_handle();
        let focused = focus.is_focused(window);
        let option_as_meta = self.option_as_meta;
        let on_intent = self.on_intent.clone();
        let session = self.session.clone();

        // Layer 1: row background rects over the pane ground.
        let mut backgrounds = div().absolute().inset_0();
        for (row_ix, row) in snapshot.rows.iter().enumerate() {
            for span in &row.bgs {
                let cols = span_cols(&row.text, span.start, span.end);
                backgrounds = backgrounds.child(
                    div()
                        .absolute()
                        .left(px(row_offset(0, span.start, &row.text, advance)))
                        .top(px(row_ix as f32 * line_height))
                        .w(px(cols as f32 * advance))
                        .h(px(line_height))
                        .bg(span.color),
                );
            }
        }
        // Layer 2: batched same-style text runs, one line per row.
        let mut lines = gpui::div().w_full().mono(TERM_PX).line_height(gpui::relative(TERM_LH));
        for row in &snapshot.rows {
            let text = if row.text.is_empty() { " ".to_string() } else { row.text.clone() };
            lines = lines.child(
                div()
                    .h(px(line_height))
                    .w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(gpui::StyledText::new(text).with_runs(row.runs.clone())),
            );
        }
        // Layer 3: the cursor — block, bar or underline per DECSCUSR —
        // drawn hollow when the element is not focused.
        let mut cursor_el = div();
        if let Some(cursor) = snapshot.cursor {
            let x = cursor.col as f32 * advance;
            let y = cursor.row as f32 * line_height;
            cursor_el = match cursor.shape {
                // A hollow block is already the unfocused look: always an
                // outline, whatever the focus.
                CursorShape::HollowBlock => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(advance))
                    .h(px(line_height))
                    .bg(gpui::transparent_black())
                    .border_1()
                    .border_color(palette.term_cursor),
                CursorShape::Block => {
                    let mut el = div()
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(advance))
                        .h(px(line_height))
                        .bg(palette.term_cursor);
                    if !focused {
                        el = el.bg(gpui::transparent_black()).border_1().border_color(palette.term_cursor);
                    }
                    el
                }
                CursorShape::Beam => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(CURSOR_BAR_W))
                    .h(px(line_height))
                    .bg(palette.term_cursor),
                CursorShape::Underline => div()
                    .absolute()
                    .left(px(x))
                    .top(px(line_height - CURSOR_BAR_W + y))
                    .w(px(advance))
                    .h(px(CURSOR_BAR_W))
                    .bg(palette.term_cursor),
                CursorShape::Hidden => div(),
            };
        }
        // Layer 4: the overlay layer. Block chrome first (one strip per
        // block intersecting the viewport, decision D44), then the
        // jump-to-latest affordance while detached. A fullscreen program
        // owns every row, so the whole layer hides on the alternate screen.
        let alt_screen = self.session.read(cx).alt_screen();
        let chromes =
            if alt_screen { Vec::new() } else { self.session.read(cx).overlay_chrome() };
        let mut overlay = div().absolute().inset_0();
        if !alt_screen {
            for chrome in &chromes {
                overlay = overlay.child(block_chrome_el(
                    chrome,
                    advance,
                    line_height,
                    &palette,
                    on_intent.clone(),
                ));
            }
        }
        if snapshot.detached && !alt_screen {
            let jump = self.session.clone();
            let affordance = aui::data::button("terminal-jump-latest", "Jump to latest").sm()
                .on_click(move |_, _, cx| {
                    jump.update(cx, |session, cx| {
                        session.scroll_to_bottom();
                        cx.notify();
                    });
                });
            overlay = overlay.child(
                div()
                    .absolute()
                    .bottom(px(JUMP_OFFSET))
                    .right(px(JUMP_OFFSET))
                    .child(aui::overlay::popover_layer(affordance)),
            );
        }

        let grid_id = gpui::ElementId::named_usize(
            "terminal-grid",
            self.session.entity_id().as_u64() as usize,
        );
        div()
            .id(grid_id)
            .relative()
            .size_full()
            .bg(palette.term_bg)
            .text_color(palette.term_fg)
            .track_focus(&focus)
            .child(backgrounds)
            .child(lines)
            .child(cursor_el)
            .child(overlay)
            .child(layout_probe(&self.session, advance, line_height))
            .on_mouse_down(gpui::MouseButton::Left, {
                let session = session.clone();
                let on_intent = on_intent.clone();
                let focus = focus.clone();
                move |event: &gpui::MouseDownEvent, window: &mut gpui::Window, cx: &mut gpui::App| {
                    window.focus(&focus, cx);
                    let palette = cx.aui().colors;
                    let Some((col, row)) = session.read(cx).cell_at(event.position) else { return };
                    grid_mouse_down(
                        &session,
                        on_intent.clone(),
                        &palette,
                        (col, row),
                        event,
                        window,
                        cx,
                    );
                }
            })
            .on_mouse_up(gpui::MouseButton::Left, {
                let session = session.clone();
                move |event: &gpui::MouseUpEvent, _: &mut gpui::Window, cx: &mut gpui::App| {
                    let Some((col, row)) = session.read(cx).cell_at(event.position) else {
                        session.update(cx, |session, _| {
                            session.dragging.store(false, Ordering::Release);
                        });
                        return;
                    };
                    grid_mouse_up(&session, col, row, cx);
                }
            })
            .on_mouse_move({
                let session = session.clone();
                move |event: &gpui::MouseMoveEvent, _: &mut gpui::Window, cx: &mut gpui::App| {
                    session.update(cx, |session, cx| {
                        let cell = session.cell_at(event.position);
                        // ⌘-hover tracks the link cell for underlining.
                        *session.hover.lock().unwrap() =
                            if event.modifiers.platform { cell } else { None };
                        if session.dragging.load(Ordering::Acquire) {
                            if let Some((col, row)) = cell {
                                if session.mouse_report() != MouseReport::Local
                                    && session.sgr_mouse()
                                    && !event.modifiers.shift
                                {
                                    let report = encode_sgr(0, col as u16, row as u16, SgrKind::Motion);
                                    session.write(&report);
                                } else {
                                    session.select_extend(col, row);
                                }
                                cx.notify();
                            }
                        } else if event.modifiers.platform {
                            cx.notify();
                        }
                    });
                }
            })
            .on_scroll_wheel({
                let session = session.clone();
                move |event: &gpui::ScrollWheelEvent, _: &mut gpui::Window, cx: &mut gpui::App| {
                    session.update(cx, |session, cx| {
                        grid_wheel(session, event, line_height, cx);
                    });
                }
            })
            .on_key_down({
                let session = session.clone();
                move |event: &gpui::KeyDownEvent, _: &mut gpui::Window, cx: &mut gpui::App| {
                    grid_key(&session, option_as_meta, event, cx);
                }
            })
    }
}

/// One block's overlay strip at its anchor row: gutter status glyph, exit
/// code and duration, author mark, then the action row (Copy, Rerun, Stop,
/// Ask) raising [`TerminalGridIntent`]s. Overflow goes through
/// [`popover_layer`](aui::overlay::popover_layer), like the jump affordance.
fn block_chrome_el(
    chrome: &BlockChrome,
    advance: f32,
    line_height: f32,
    palette: &aui_tokens::Palette,
    on_intent: Option<IntentHandler>,
) -> impl gpui::IntoElement {
    use aui_tokens::scale;
    use gpui::{div, prelude::*, px};
    let status = if chrome.running {
        palette.accent
    } else if chrome.failed {
        palette.danger
    } else {
        palette.success
    };
    let index = chrome.index;
    // One ghost action button raising its intent with the block index.
    let action = |name: &'static str, label: &'static str, intent: TerminalGridIntent| {
        let emit = on_intent.clone();
        aui::data::button(gpui::ElementId::named_usize(name, index), label).xs().ghost().on_click(
            move |_, window: &mut gpui::Window, cx: &mut gpui::App| {
                if let Some(emit) = &emit {
                    emit(intent.clone(), window, cx);
                }
            },
        )
    };
    let gutter = div()
        .w(px(advance))
        .h(px(line_height))
        .flex()
        .items_center()
        .justify_center()
        .mono(scale::FS_11)
        .text_color(status)
        .child(chrome.glyph.to_string());
    let mut meta = div()
        .flex()
        .flex_row()
        .items_center()
        .mono(scale::FS_11)
        .text_color(palette.ink_2)
        .bg(palette.surface_2)
        .rounded(px(scale::R_SM))
        .px(px(scale::SP_2))
        .child(chrome.label.clone());
    if chrome.author == BlockAuthor::Agent {
        // D43's `M` mark, rendered by the host's choice of author label.
        meta = meta.child(div().ml(px(scale::SP_1)).text_color(palette.ink_3).child("M"));
    }
    let mut actions = div().flex().flex_row().items_center();
    for (name, label, intent) in [
        ("term-block-copy", "Copy", TerminalGridIntent::Copy(index)),
        ("term-block-rerun", "Rerun", TerminalGridIntent::Rerun(index)),
        ("term-block-stop", "Stop", TerminalGridIntent::Stop(index)),
        ("term-block-ask", "Ask", TerminalGridIntent::Ask(index)),
    ] {
        actions = actions.child(div().ml(px(scale::SP_1)).child(action(name, label, intent)));
    }
    let strip = div()
        .absolute()
        .top(px(chrome.row as f32 * line_height))
        .left(px(0.0))
        .right(px(0.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .child(gutter)
                .child(meta)
                .child(div().flex_1())
                .child(actions),
        );
    aui::overlay::popover_layer(strip)
}

/// Pixels from the row start to byte offset `at`, counting whole cells.
fn row_offset(_row: usize, at: usize, text: &str, advance: f32) -> f32 {
    let cols = text[..at.min(text.len())].chars().count();
    cols as f32 * advance
}

/// Whole cells covered by byte range `start..end`.
fn span_cols(text: &str, start: usize, end: usize) -> usize {
    text.get(start..end).map(|s| s.chars().count()).unwrap_or(0).max(1)
}

/// Left press: focus is handled by the caller. `⌘`-click on a link opens it;
/// SGR-reporting programs get a press report (unless `⇧` forces selection);
/// otherwise a drag-selection starts (word on double-, line on triple-click).
fn grid_mouse_down(
    session: &gpui::Entity<TerminalSession>,
    on_intent: Option<IntentHandler>,
    palette: &aui_tokens::Palette,
    cell: (usize, usize),
    event: &gpui::MouseDownEvent,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    let (col, row) = cell;
    session.update(cx, |session, cx| {
        // ⌘-click opens the hyperlink or detected URL under the pointer.
        if event.modifiers.platform {
            let url = session.snapshot(palette).rows.get(row).and_then(|r| {
                let at = col_to_byte(r, col);
                r.links.iter().find(|span| span.start <= at && at < span.end).map(|s| s.url.clone())
            });
            if let Some(url) = url {
                if let Some(emit) = &on_intent {
                    emit(TerminalGridIntent::OpenUrl(url.into()), window, cx);
                }
                return;
            }
        }
        let reporting = session.mouse_report() != MouseReport::Local
            && session.sgr_mouse()
            && !event.modifiers.shift;
        if reporting {
            session.write(&encode_sgr(0, col as u16, row as u16, SgrKind::Press));
            session.dragging.store(true, Ordering::Release);
        } else {
            match event.click_count {
                0 | 1 => session.select_start(col, row),
                2 => session.select_word(col, row),
                _ => session.select_line(row),
            }
            session.dragging.store(true, Ordering::Release);
        }
        cx.notify();
    });
}

/// Left release: closes an SGR report pair, or ends the drag-selection.
fn grid_mouse_up(session: &gpui::Entity<TerminalSession>, col: usize, row: usize, cx: &mut gpui::App) {
    session.update(cx, |session, cx| {
        if session.dragging.swap(false, Ordering::AcqRel) && session.sgr_mouse() {
            // Only programs that were reporting get a release; a local drag
            // just ends. Re-check the mode rather than storing it: the
            // program may have changed its mind mid-drag.
            if session.mouse_report() != MouseReport::Local {
                session.write(&encode_sgr(0, col as u16, row as u16, SgrKind::Release));
            }
        }
        cx.notify();
    });
}

/// Wheel: to the program as SGR when it asked (unless `⇧` forces local),
/// otherwise local scroll that re-glues to the tail at the bottom.
fn grid_wheel(
    session: &TerminalSession,
    event: &gpui::ScrollWheelEvent,
    line_height: f32,
    cx: &mut gpui::Context<TerminalSession>,
) {
    use gpui::ScrollDelta;
    let lines: isize = match event.delta {
        // Wheel up is negative y: toward the scrollback, i.e. positive.
        ScrollDelta::Pixels(point) => (-f32::from(point.y) / line_height).round() as isize,
        ScrollDelta::Lines(point) => (-point.y).round() as isize,
    };
    if lines == 0 {
        return;
    }
    let reporting =
        session.mouse_report() != MouseReport::Local && session.sgr_mouse() && !event.modifiers.shift;
    if reporting {
        // One SGR report per line: 64 up, 65 down.
        for _ in 0..lines.abs().min(32) {
            let button = if lines > 0 { 64 } else { 65 };
            let (cols, rows) = session.cells();
            session.write(&encode_sgr(button, cols / 2, rows / 2, SgrKind::Press));
        }
        return;
    }
    if lines < 0 && session.is_following_tail() {
        return;
    }
    if lines < 0 {
        // Scrolling down re-glues once the tail is within reach.
        let offset = session.with_term(|t| t.grid().display_offset() as isize);
        if offset <= lines.abs() {
            session.scroll_to_bottom();
        } else {
            session.scroll_lines(lines);
        }
    } else {
        session.scroll_lines(lines);
    }
    cx.notify();
}

/// Keys: `⌘C` copies a selection (and is not consumed when there is none),
/// `⌘V` pastes, everything else goes through the key encoder.
fn grid_key(
    session: &gpui::Entity<TerminalSession>,
    option_as_meta: bool,
    event: &gpui::KeyDownEvent,
    cx: &mut gpui::App,
) {
    use crate::keys::{encode, KeyModifiers};
    let mods = &event.keystroke.modifiers;
    let key = event.keystroke.key.as_str();
    if mods.platform && !mods.control && !mods.alt && key.eq_ignore_ascii_case("c") {
        if let Some(text) = session.read(cx).selection_text() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
        // No selection: deliberately not consumed, so the host keymap still
        // sees ⌘C.
        return;
    }
    if mods.platform && !mods.control && !mods.alt && key.eq_ignore_ascii_case("v") {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                session.read(cx).paste(&text);
            }
        }
        return;
    }
    let Some(input) = map_keystroke(key) else { return };
    let modifiers = KeyModifiers {
        shift: mods.shift,
        alt: mods.alt,
        ctrl: mods.control,
        meta: mods.platform,
    };
    session.update(cx, |session, _| {
        let modes = session.key_modes();
        session.write(&encode(&input, &modifiers, &modes, option_as_meta));
    });
}

/// Maps a gpui key name onto the encoder. Single characters pass through as
/// text; anything unknown returns `None` and is ignored.
fn map_keystroke(key: &str) -> Option<crate::keys::KeyInput> {
    use crate::keys::SpecialKey;
    if key.chars().count() == 1 {
        return key.chars().next().map(crate::keys::KeyInput::Text);
    }
    let special = match key {
        "enter" => SpecialKey::Enter,
        "tab" => SpecialKey::Tab,
        "backspace" => SpecialKey::Backspace,
        "escape" => SpecialKey::Escape,
        "left" => SpecialKey::Left,
        "up" => SpecialKey::Up,
        "right" => SpecialKey::Right,
        "down" => SpecialKey::Down,
        "home" => SpecialKey::Home,
        "end" => SpecialKey::End,
        "insert" => SpecialKey::Insert,
        "delete" => SpecialKey::Delete,
        "pageup" => SpecialKey::PageUp,
        "pagedown" => SpecialKey::PageDown,
        "space" => return Some(crate::keys::KeyInput::Text(' ')),
        _ => {
            if let Some(number) = key.strip_prefix('f') {
                if let Ok(n) = number.parse::<u8>() {
                    if (1..=20).contains(&n) {
                        return Some(crate::keys::KeyInput::Key(SpecialKey::F(n)));
                    }
                }
            }
            return None;
        }
    };
    Some(crate::keys::KeyInput::Key(special))
}

/// An element that paints nothing and measures the pane: reports cols/rows
/// from its bounds and calls [`TerminalSession::resize`] only when the cell
/// count actually changed. Never notifies: a resized program redraws itself
/// and the poll timer picks that up.
fn layout_probe(
    session: &gpui::Entity<TerminalSession>,
    advance: f32,
    line_height: f32,
) -> impl gpui::IntoElement {
    use gpui::prelude::*;
    let session = session.clone();
    gpui::canvas(
        move |bounds: gpui::Bounds<gpui::Pixels>, _: &mut gpui::Window, cx: &mut gpui::App| {
            if advance <= 0.0 || line_height <= 0.0 {
                return;
            }
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            let cols = ((w / advance).floor() as u16).clamp(MIN_COLS, u16::MAX);
            let rows = ((h / line_height).floor() as u16).clamp(MIN_ROWS, u16::MAX);
            let metrics = CellMetrics { advance, line_height };
            session.update(cx, |session, _| session.note_layout(bounds, metrics, cols, rows));
        },
        |_, _: (), _, _| {},
    )
    .absolute()
    .size_full()
}

// ---------------------------------------------------------------------------
// IME: the session implements gpui's input handler contract.
// ---------------------------------------------------------------------------
//
// TODO(L1): registering the handler with `window.handle_input` needs the
// element's bounds, which only exist in a stateful element's paint phase —
// `ElementInputHandler::new` takes them as an argument, and a stateless
// `RenderOnce` render never has them. So the grid wires typed input through
// `on_key_down` (above) while the session below already implements the full
// `EntityInputHandler` contract, including marked text: a stateful host
// element can register it, and the snapshot draws marked text at the cursor
// either way.

impl gpui::EntityInputHandler for TerminalSession {
    fn text_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _adjusted_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        // The terminal has no linear text buffer to quote from.
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::UTF16Selection> {
        None
    }

    fn marked_text_range(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        self.marked.lock().unwrap().as_ref().map(|text| 0..text.len())
    }

    fn unmark_text(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) {
        // Commit: the marked text goes to the program as typed text.
        if let Some(text) = self.marked.lock().unwrap().take() {
            self.write(text.as_bytes());
        }
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        text: &str,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) {
        self.write(text.as_bytes());
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) {
        *self.marked.lock().unwrap() = Some(new_text.to_string());
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: std::ops::Range<usize>,
        _element_bounds: gpui::Bounds<gpui::Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<gpui::Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// A deterministic backend: queued output, one exit, recorded writes.
    struct ScriptBackend {
        outputs: Mutex<VecDeque<Vec<u8>>>,
        writes: Mutex<Vec<Vec<u8>>>,
        exit: Mutex<Option<i32>>,
    }

    impl ScriptBackend {
        fn new(chunks: Vec<&[u8]>) -> Self {
            Self {
                outputs: Mutex::new(chunks.into_iter().map(|c| c.to_vec()).collect()),
                writes: Mutex::new(Vec::new()),
                exit: Mutex::new(None),
            }
        }

        fn with_exit(self, code: i32) -> Self {
            *self.exit.lock().unwrap() = Some(code);
            self
        }
    }

    impl TerminalBackend for ScriptBackend {
        fn spawn(&mut self, _shell: &str, _cwd: &Path) -> std::io::Result<()> {
            Ok(())
        }

        fn write(&mut self, bytes: &[u8]) {
            self.writes.lock().unwrap().push(bytes.to_vec());
        }

        fn resize(&mut self, _cols: u16, _rows: u16) {}

        fn poll(&mut self) -> Vec<TermEvent> {
            let mut out: Vec<TermEvent> =
                self.outputs.lock().unwrap().drain(..).map(TermEvent::Output).collect();
            if let Some(code) = self.exit.lock().unwrap().take() {
                out.push(TermEvent::Exit(code));
            }
            out
        }
    }

    /// A session on queued bytes, `cols` × `rows`, pumped once.
    fn pumped(chunks: Vec<&[u8]>, cols: u16, rows: u16) -> TerminalSession {
        let backend = ScriptBackend::new(chunks);
        let session = TerminalSession::new(Box::new(backend), cols, rows);
        session.pump();
        session
    }

    fn dark() -> aui_tokens::Palette {
        aui_tokens::Palette::for_kind(aui_tokens::ThemeKind::Dark)
    }

    #[test]
    fn fed_text_appears_in_the_snapshot() {
        let session = pumped(vec![b"hello"], 20, 5);
        let grid = session.snapshot(&dark());
        assert_eq!(grid.rows.len(), 5);
        assert!(grid.rows[0].text.starts_with("hello"));
        assert_eq!(grid.cursor.map(|c| (c.row, c.col)), Some((0, 5)));
        assert!(!grid.detached);
        // Runs cover the text exactly.
        assert_eq!(grid.rows[0].runs.iter().map(|r| r.len).sum::<usize>(), grid.rows[0].text.len());
    }

    #[test]
    fn scrollback_survives_and_the_tail_follows() {
        let mut script = Vec::new();
        for i in 0..30 {
            script.push(format!("line{i:02}\r\n"));
        }
        let owned: Vec<Vec<u8>> = script.into_iter().map(String::into_bytes).collect();
        let refs: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
        let session = pumped(refs, 20, 10);
        assert!(session.is_following_tail());
        session.scroll_lines(5);
        assert!(!session.is_following_tail());
        let grid = session.snapshot(&dark());
        assert!(grid.detached);
        assert!(grid.rows[0].text.starts_with("line"));
        session.scroll_to_bottom();
        assert!(session.is_following_tail());
    }

    #[test]
    fn a_drag_selection_round_trips_to_text() {
        let session = pumped(vec![b"hello"], 20, 5);
        assert_eq!(session.selection_text(), None);
        session.select_start(0, 0);
        session.select_extend(4, 0);
        assert_eq!(session.selection_text().as_deref(), Some("hello"));
        session.select_clear();
        assert_eq!(session.selection_text(), None);
    }

    #[test]
    fn double_click_selects_a_word_and_triple_click_a_line() {
        let session = pumped(vec![b"hello world"], 20, 5);
        session.select_word(1, 0);
        assert_eq!(session.selection_text().as_deref(), Some("hello"));
        session.select_line(0);
        let text = session.selection_text().expect("a line selection");
        assert!(text.contains("hello world"), "unexpected selection {text:?}");
    }

    #[test]
    fn title_bell_and_exit_reach_the_host() {
        let session = pumped(vec![b"\x1b]0;my title\x07", b"\x07"], 20, 5);
        assert_eq!(session.title().as_deref(), Some("my title"));
        let events = session.drain_events();
        assert!(events.contains(&SessionEvent::TitleChanged("my title".to_string())));
        assert!(events.contains(&SessionEvent::Bell));
        assert!(session.drain_events().is_empty(), "draining takes everything");
    }

    #[test]
    fn a_backend_exit_records_the_status() {
        let backend = ScriptBackend::new(vec![]).with_exit(3);
        let session = TerminalSession::new(Box::new(backend), 20, 5);
        session.pump();
        assert_eq!(session.exited(), Some(3));
        assert!(session.drain_events().contains(&SessionEvent::Exited(3)));
    }

    #[test]
    fn a_clipboard_store_is_refused_and_counted() {
        let session = pumped(vec![b"\x1b]52;c;aGVsbG8=\x07"], 20, 5);
        assert_eq!(session.refused_clipboard_writes(), 1);
    }

    #[test]
    fn bracketed_paste_wraps_through_write() {
        // `paste` consults the live mode, so enable it first.
        let backend = ScriptBackend::new(vec![]);
        let session = TerminalSession::new(Box::new(backend), 20, 5);
        session.pump();
        session.paste("plain");
        session.with_term(|t| {
            assert!(!t.mode().contains(TermMode::BRACKETED_PASTE));
        });
        // Feed the mode switch, then paste again through a recording backend.
        let shared = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        struct Tap {
            writes: Arc<Mutex<Vec<Vec<u8>>>>,
        }
        impl TerminalBackend for Tap {
            fn spawn(&mut self, _shell: &str, _cwd: &Path) -> std::io::Result<()> {
                Ok(())
            }
            fn write(&mut self, bytes: &[u8]) {
                self.writes.lock().unwrap().push(bytes.to_vec());
            }
            fn resize(&mut self, _cols: u16, _rows: u16) {}
            fn poll(&mut self) -> Vec<TermEvent> {
                vec![TermEvent::Output(b"\x1b[?2004h".to_vec())]
            }
        }
        let tap = Tap { writes: shared.clone() };
        let session = TerminalSession::new(Box::new(tap), 20, 5);
        session.pump();
        session.paste("cmd");
        let writes = shared.lock().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0], b"\x1b[200~cmd\x1b[201~");
    }

    #[test]
    fn a_dsr_answer_is_written_back_to_the_backend() {
        let shared = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        struct Tap {
            writes: Arc<Mutex<Vec<Vec<u8>>>>,
        }
        impl TerminalBackend for Tap {
            fn spawn(&mut self, _shell: &str, _cwd: &Path) -> std::io::Result<()> {
                Ok(())
            }
            fn write(&mut self, bytes: &[u8]) {
                self.writes.lock().unwrap().push(bytes.to_vec());
            }
            fn resize(&mut self, _cols: u16, _rows: u16) {}
            fn poll(&mut self) -> Vec<TermEvent> {
                vec![TermEvent::Output(b"hi\x1b[6n".to_vec())]
            }
        }
        let tap = Tap { writes: shared.clone() };
        let session = TerminalSession::new(Box::new(tap), 20, 5);
        session.pump();
        let writes = shared.lock().unwrap();
        assert!(
            writes.iter().any(|w| w.starts_with(b"\x1b[") && w.ends_with(b"R")),
            "no DSR reply in {writes:?}"
        );
    }

    #[test]
    fn the_dirty_flag_coalesces_between_frames() {
        let backend = ScriptBackend::new(vec![b"a", b"b"]);
        let session = TerminalSession::new(Box::new(backend), 20, 5);
        assert!(session.pump(), "output arrived");
        assert!(session.take_dirty(), "one pending repaint");
        assert!(!session.take_dirty(), "coalesced: nothing more to report");
        assert!(!session.pump(), "backend drained");
    }

    #[test]
    fn alt_screen_and_cursor_modes_are_visible() {
        let session = pumped(vec![b"\x1b[?1049h"], 20, 5);
        assert!(session.alt_screen());
        let modes = pumped(vec![b"\x1b[?1h"], 20, 5);
        assert!(modes.key_modes().app_cursor);
        assert!(!modes.key_modes().app_keypad);
    }

    #[test]
    fn mouse_modes_are_reported() {
        let session = pumped(vec![b"\x1b[?1000h\x1b[?1006h"], 20, 5);
        assert_eq!(session.mouse_report(), MouseReport::Click);
        assert!(session.sgr_mouse());
    }

    #[test]
    fn sgr_reports_encode() {
        assert_eq!(encode_sgr(0, 4, 7, SgrKind::Press), b"\x1b[<0;5;8M");
        assert_eq!(encode_sgr(0, 4, 7, SgrKind::Release), b"\x1b[<3;5;8m");
        assert_eq!(encode_sgr(0, 4, 7, SgrKind::Motion), b"\x1b[<32;5;8M");
    }

    #[test]
    fn indexed_and_truecolor_resolve() {
        let session = pumped(vec![b"\x1b[38;5;200mX\x1b[0m\x1b[38;2;10;20;30mY"], 20, 5);
        let grid = session.snapshot(&dark());
        let row = &grid.rows[0];
        assert!(row.text.starts_with("XY"));
        let expected = cube(200 - 16);
        assert_eq!(row.runs[0].color, expected);
    }

    #[test]
    fn urls_are_detected() {
        assert_eq!(find_urls("see https://example.com/a (ok)"), vec![(4, 25)]);
        assert_eq!(find_urls("no links here"), Vec::new());
    }

    /// A `Send` replay backend: [`FakePty`](crate::fake::FakePty) owns a
    /// `ManualClock` and is not `Send`, so the session cannot hold it
    /// directly. The transcript still travels through the fake — see
    /// [`session_on_fake`].
    struct ReplayBackend {
        chunks: Mutex<VecDeque<Vec<u8>>>,
    }

    impl ReplayBackend {
        fn new(chunks: Vec<Vec<u8>>) -> Self {
            Self { chunks: Mutex::new(chunks.into_iter().collect()) }
        }
    }

    impl TerminalBackend for ReplayBackend {
        fn spawn(&mut self, _shell: &str, _cwd: &Path) -> std::io::Result<()> {
            Ok(())
        }

        fn write(&mut self, _bytes: &[u8]) {}

        fn resize(&mut self, _cols: u16, _rows: u16) {}

        fn poll(&mut self) -> Vec<TermEvent> {
            self.chunks.lock().unwrap().drain(..).map(TermEvent::Output).collect()
        }
    }

    /// Pumps `transcript` through [`FakePty`](crate::fake::FakePty)'s own
    /// script and poll path into a nonce-pinned session: the chunks are
    /// built by `FakePty::new` and handed over by `FakePty::poll`, then
    /// replayed into the session synchronously.
    fn session_on_fake(transcript: String, nonce: &str) -> TerminalSession {
        use crate::fake::{FakePty, ScriptChunk};
        let mut fake = FakePty::new(vec![ScriptChunk::new(0, transcript)]);
        let mut chunks = Vec::new();
        for _ in 0..3 {
            for event in fake.poll() {
                if let TermEvent::Output(bytes) = event {
                    chunks.push(bytes);
                }
            }
        }
        let session =
            TerminalSession::new(Box::new(ReplayBackend::new(chunks)), 40, 10).with_nonce(nonce);
        session.pump();
        session.pump();
        session
    }

    /// A nonce-checked transcript through [`FakePty`](crate::fake::FakePty):
    /// the marks become one finished block with ANSI-free text APIs over it.
    #[test]
    fn nonce_checked_marks_become_blocks() {
        let nonce = "k7test";
        let transcript = format!(
            "\x1b]133;A;k={nonce}\x07prompt$ \x1b]133;B;k={nonce}\x07echo hi\r\n\
             \x1b]133;C;k={nonce}\x07hi\r\n\x1b]133;D;0;k={nonce}\x07"
        );
        let session = session_on_fake(transcript, nonce);
        assert_eq!(session.marks().len(), 4);
        let blocks = session.blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].exit, Some(0));
        assert!(!blocks[0].running());
        assert!(blocks[0].command.contains("echo hi"), "command: {:?}", blocks[0].command);
        let text = session.block_text(0).expect("block text");
        assert!(text.contains("hi"), "block text: {text:?}");
        assert!(session.block_text(7).is_none(), "out-of-range block reads None");
        let screen = session.screen_text();
        assert!(screen.contains("echo hi"), "screen: {screen:?}");
        let tail = session.tail_cursor();
        assert_eq!(session.range_text(tail), "", "nothing new past the tail");
        assert!(session.range_text(TextCursor::start()).contains("hi"));
        session.set_block_author(0, BlockAuthor::Agent);
        assert_eq!(session.blocks()[0].author, BlockAuthor::Agent);
        assert_eq!(session.overlay_chrome().len(), 1, "the block intersects the viewport");
    }

    /// Decision D45, end to end: a transcript whose markers carry the wrong
    /// nonce — and markers with no `k=` at all — produces no marks and no
    /// blocks when fed through [`FakePty`](crate::fake::FakePty).
    #[test]
    fn forged_markers_without_the_nonce_make_no_blocks() {
        let transcript = "\x1b]133;A;k=wrong\x07prompt$ \x1b]133;B\x07echo hi\r\n\
             \x1b]133;C;k=someone-else\x07hi\r\n\x1b]133;D;0\x07"
            .to_string();
        let session = session_on_fake(transcript, "k7test");
        assert!(session.marks().is_empty(), "forged marks accepted: {:?}", session.marks());
        assert!(session.blocks().is_empty(), "forged markers made a block");
    }

    /// The overlay hides entirely on the alternate screen (vim, htop).
    #[test]
    fn the_overlay_hides_on_the_alternate_screen() {
        let nonce = "k7test";
        let transcript = format!(
            "\x1b]133;A;k={nonce}\x07prompt$ \x1b]133;B;k={nonce}\x07vim\r\n\
             \x1b]133;C;k={nonce}\x07\x1b[?1049h"
        );
        let session = session_on_fake(transcript, nonce);
        assert!(session.alt_screen());
        assert_eq!(session.blocks().len(), 1);
        assert!(session.blocks()[0].running(), "no D arrived yet");
        assert!(session.overlay_chrome().is_empty(), "chrome drawn over fullscreen");
    }
}


