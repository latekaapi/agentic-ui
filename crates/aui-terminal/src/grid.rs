//! One emulator: [`TerminalSession`] and the [`terminal_grid`] element.
//!
//! [`tui_grid`](crate::tui_grid) wraps an `alacritty_terminal` `Term` with no
//! scrollback, swallowed events and no selection. This module is its
//! replacement: a `Term` with 10 000 lines of scrollback behind
//! [`FairMutex`], a reader thread that
//! drains a boxed [`TerminalBackend`] and
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
    atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering},
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
    assemble_blocks, Block, BlockAuthor, BlockStatus, Mark, MarkScanner, ScanSegment, TextCursor,
    block_status_token,
};
use crate::parser::{finished_label, live_label};

/// Shell-command highlighting for the block chrome, kept in its own file
/// but owned here: the overlay is the only consumer, so the module lives
/// under the grid rather than beside it. The grid's own rows never call it.
#[path = "syntax.rs"]
pub mod syntax;

/// Lines of scrollback behind the visible screen (decision D44).
const SCROLLBACK: usize = 10_000;
/// Headroom above [`SCROLLBACK`] the emulator is constructed with. Overflow
/// is trimmed back to [`SCROLLBACK`] on a schedule that keeps every discarded
/// line counted (see [`feed_with_marks`]). There is no fixed byte bound:
/// `ESC[nS`/`ESC[nM` push up to the scroll region's height from a few bytes,
/// so each raw slice is sized from the live row count
/// (`slice_bytes / 4 * rows <= SCROLLBACK_SLACK`, floor 64) and re-sized on
/// every feed so a resize is picked up. If the emulator's own cap
/// (`SCROLLBACK + SCROLLBACK_SLACK`) is ever reached mid-burst, lines may
/// have been discarded before the trim step could count them: that is
/// recorded in [`TerminalSession::history_desyncs`] and the count is then
/// approximate rather than exact.
const SCROLLBACK_SLACK: usize = 4_096;
/// The backend is drained on this cadence — about twice a display frame.
const POLL_INTERVAL: Duration = Duration::from_millis(8);
/// Smallest grid the session will report, however small the pane is drawn.
const MIN_COLS: u16 = 8;
/// …and fewest rows.
const MIN_ROWS: u16 = 2;
/// Introducer byte shared by every escape sequence the advance splitter cuts at.
const ESC: u8 = 0x1b;
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
/// Every event becomes a `SideEvent` the reader thread handles. The one
/// exception is the repaint marker, which flips the shared dirty flag
/// directly so a repaint is never stuck behind a busy backend.
#[derive(Debug, Clone)]
pub struct SessionEventProxy {
    /// Where side-channel events go.
    tx: mpsc::Sender<SideEvent>,
    /// Flipped on any event that changes what the grid shows.
    shared: Arc<Shared>,
}

/// Marks the grid dirty and invalidates the snapshot cache (D20): every path
/// that can change what [`TerminalSession::snapshot`] reads goes through
/// here, so a cached snapshot is never stale.
fn mark_dirty(shared: &Arc<Shared>) {
    shared.dirty.store(true, Ordering::Release);
    shared.snap_gen.fetch_add(1, Ordering::Release);
}

impl EventListener for SessionEventProxy {
    fn send_event(&self, event: Event) {
        mark_dirty(&self.shared);
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

/// What the program last said about cursor blinking (D19). Alacritty's own
/// `cursor_style` collapses "never touched" and "steady block" into one
/// `blinking: false`, so the session mirrors the byte stream itself: only an
/// explicit steady shape (a DECSCUSR steady variant, or mode 12 off) counts
/// as steady. Anything else — including the untouched default — blinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CursorBlink {
    /// Never touched, or reset (DECSCUSR 0, RIS): blinks, like a terminal.
    #[default]
    Default,
    /// DECSCUSR blinking variant, or mode 12 on.
    Blink,
    /// DECSCUSR steady variant, or mode 12 off.
    Steady,
}

/// The cursor-blink mirror over the fed bytes, plus the carry for sequences
/// split across backend chunks (see [`note_cursor_sequences`]).
#[derive(Debug, Default)]
struct CursorScan {
    /// The program's last word on blinking.
    mode: CursorBlink,
    /// Tail of the previous feed, so a sequence straddling two chunks still
    /// parses as one.
    tail: Vec<u8>,
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
    /// Bumped alongside every `dirty` store and every other snapshot input
    /// (selection, scroll, resize, hover, marked text): the snapshot cache's
    /// invalidation generation (D20).
    snap_gen: AtomicU64,
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
    /// Blocks assembled from the marks, oldest first. Pruned to the retained
    /// window: rebuilds carry the host-set authors and surviving command
    /// text across by block id, never by index.
    blocks: Mutex<Vec<Block>>,
    /// Set by [`TerminalSession::resize`]; the next read rebuilds the blocks
    /// from the marks against the reflowed grid.
    stale: AtomicBool,
    /// Lines evicted from the retained window since the session started.
    /// Starts at 0 and never decreases; added to the grid's own history
    /// offset so absolute lines stay monotonic past the scrollback cap.
    evicted: AtomicI64,
    /// Bursts that reached the emulator's own cap before the trim step ran.
    /// Lines may have been discarded uncounted; the evicted count is then
    /// approximate (monotonicity is still preserved).
    desync: AtomicU64,
    /// The main grid's history size, snapshotted when the alternate screen
    /// is entered. While the alternate screen is active its grid has no
    /// scrollback, so no eviction accounting runs at all; on exit the
    /// snapshot is dropped and accounting resumes from the live grid.
    alt_saved_history: Mutex<Option<usize>>,
    /// The cursor-blink mirror over the fed bytes (D19).
    cursor: Mutex<CursorScan>,
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
    /// The element's focus handle, minted once and held across renders (D15).
    /// A fresh handle per render can never be focused and drops window focus
    /// when its refcount hits zero; the host may override it per element
    /// with [`TerminalGrid::focus_handle`].
    focus: Mutex<Option<gpui::FocusHandle>>,
    /// The last snapshot and the generation that built it, by palette (D20).
    /// A blinking cursor repaints every frame while only its own phase
    /// changes; the cache makes those frames cost the cursor, not the grid.
    snap_cache: Mutex<Option<(u64, aui_tokens::Palette, std::sync::Arc<GridSnapshot>)>>,
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
        let config =
            Config { scrolling_history: SCROLLBACK + SCROLLBACK_SLACK, ..Config::default() };
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
                evicted: AtomicI64::new(0),
                desync: AtomicU64::new(0),
                alt_saved_history: Mutex::new(None),
                cursor: Mutex::new(CursorScan::default()),
            }),
            focus: Mutex::new(None),
            snap_cache: Mutex::new(None),
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
                    mark_dirty(&shared);
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
            mark_dirty(&self.shared);
        }
        had_output
    }

    /// Takes the repaint flag: `true` when the grid changed since the last
    /// call. The poll timer uses this shape; tests assert on it directly.
    pub fn take_dirty(&self) -> bool {
        self.shared.dirty.swap(false, Ordering::AcqRel)
    }

    /// The element's focus handle (D15): the cached handle, minting and
    /// holding one on first use so every render tracks the same handle. A
    /// host with its own handle passes it per element with
    /// [`TerminalGrid::focus_handle`] instead.
    pub fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        if let Some(handle) = self.focus.lock().unwrap().clone() {
            return handle;
        }
        let handle = cx.focus_handle();
        *self.focus.lock().unwrap() = Some(handle.clone());
        handle
    }

    /// Invalidates the snapshot cache without touching the repaint flag:
    /// selection, scroll, resize, hover and marked text change what the
    /// snapshot reads but set no dirty flag of their own (D20).
    fn bump_snapshot(&self) {
        self.shared.snap_gen.fetch_add(1, Ordering::Release);
    }

    /// The cached snapshot for `palette` (D20): rebuilt only when the
    /// generation moved or the palette changed, so blink frames share one
    /// grid instead of rebuilding it at refresh rate.
    fn snapshot_cached(&self, palette: &aui_tokens::Palette) -> std::sync::Arc<GridSnapshot> {
        let gen = self.shared.snap_gen.load(Ordering::Acquire);
        if let Some((cached_gen, cached_palette, cached)) =
            self.snap_cache.lock().unwrap().clone()
        {
            if cached_gen == gen && cached_palette == *palette {
                return cached;
            }
        }
        let fresh = std::sync::Arc::new(self.snapshot(palette));
        *self.snap_cache.lock().unwrap() = Some((gen, *palette, fresh.clone()));
        fresh
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
        self.bump_snapshot();
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
        self.bump_snapshot();
    }

    /// Returns the viewport to the live tail.
    pub fn scroll_to_bottom(&self) {
        self.inner.lock().term.scroll_display(Scroll::Bottom);
        self.bump_snapshot();
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
        self.bump_snapshot();
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

    /// Bursts that reached the emulator's own history cap before the trim
    /// step could count them. Zero means the evicted count is exact; after a
    /// burst the absolutes stay monotonic but the count is approximate.
    pub fn history_desyncs(&self) -> u64 {
        self.mark_state.desync.load(Ordering::Acquire)
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
        Some(self.with_term(|term| {
            let evicted = self.mark_state.evicted.load(Ordering::Acquire);
            grid_range_text(term, evicted, range.0, range.1)
        }))
    }

    /// ANSI-free text of the visible grid, top row first, one line per row.
    pub fn screen_text(&self) -> String {
        self.with_term(|term| {
            let evicted = self.mark_state.evicted.load(Ordering::Acquire);
            let offset = term.grid().display_offset() as i32;
            let mut lines = Vec::new();
            let mut row = 0;
            while row < term.screen_lines() {
                let absolute = absolute_line(term, evicted, Line(row as i32 - offset));
                lines.push(grid_line_text(term, evicted, absolute).unwrap_or_default());
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
            let evicted = self.mark_state.evicted.load(Ordering::Acquire);
            let tail = absolute_line(term, evicted, term.grid().cursor.point.line);
            // A cursor older than the retained window clamps forward to the
            // oldest retained line: the host gets what is still retained
            // instead of an empty stall.
            let floor = evicted_floor(&self.mark_state);
            let from = since.line.saturating_add(1).max(0).max(floor);
            if from > tail {
                return String::new();
            }
            grid_range_text(term, evicted, from, tail + 1)
        })
    }

    /// The cursor describing everything currently on the grid: `range_text`
    /// past this cursor reads empty until more output arrives.
    pub fn tail_cursor(&self) -> TextCursor {
        self.with_term(|term| {
            let evicted = self.mark_state.evicted.load(Ordering::Acquire);
            TextCursor { line: absolute_line(term, evicted, term.grid().cursor.point.line) }
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
            let evicted = self.mark_state.evicted.load(Ordering::Acquire);
            let offset = term.grid().display_offset() as i32;
            let rows = term.screen_lines() as i32;
            let top = absolute_line(term, evicted, Line(-offset));
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
                    let cols = term.columns();
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
                    // Right-aligned at the trailing edge: the strip is
                    // exactly its own text width, against the grid's right
                    // inset (the element shrink-wraps it inside the padded
                    // text area) — never mid-sentence after the prompt. One
                    // blank cell of separation from the row's own text, or no
                    // chrome at all: a hidden status beats an unreadable one,
                    // and a full row always hides.
                    let used = anchor_used_cols(term, Line(row - offset), cols).min(cols);
                    let author = block.author;
                    let col =
                        cols.saturating_sub(chrome_cells(glyph, &label, &block.command, author));
                    if used < col {
                        return Some(BlockChrome {
                            index,
                            row: row as usize,
                            col,
                            glyph,
                            label,
                            command: block.command.clone(),
                            author,
                            running: block.running(),
                            failed,
                        });
                    }
                    // Tight: a failure must stay visible (D18), so the strip
                    // degrades instead of hiding — the command text goes
                    // first, then the duration, keeping the status glyph and
                    // the exit code, which is the part that matters. Running
                    // and succeeded blocks keep the V3 rule and hide: only a
                    // finished failure degrades here.
                    if !failed {
                        return None;
                    }
                    let bare = chrome_cells(glyph, &label, "", author);
                    if used < cols.saturating_sub(bare) {
                        return Some(BlockChrome {
                            index,
                            row: row as usize,
                            col: cols.saturating_sub(bare),
                            glyph,
                            label,
                            command: String::new(),
                            author,
                            running: block.running(),
                            failed,
                        });
                    }
                    if let Some(exit) = block.exit {
                        let status = format!("exit {exit}");
                        let minimal = chrome_cells(glyph, &status, "", author);
                        if used < cols.saturating_sub(minimal) {
                            return Some(BlockChrome {
                                index,
                                row: row as usize,
                                col: cols.saturating_sub(minimal),
                                glyph,
                                label: status,
                                command: String::new(),
                                author,
                                running: block.running(),
                                failed,
                            });
                        }
                    }
                    None
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
        let out = f(self, &mut guard.term);
        // The only callers are the selection setters: the selection is a
        // snapshot input with no dirty flag of its own (D20).
        self.bump_snapshot();
        out
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
                mark_dirty(shared);
            }
        }
    }
    drain_side(inner, backend, side_rx, shared);
    had_output
}

/// Feeds `bytes` to the emulator and the mark scanner together: the chunk is
/// split at complete OSC 133 boundaries and at alt-screen switches, every
/// piece goes to the `Term` in order, and each accepted marker is recorded
/// at the emulator's absolute line at that point — the cursor has not moved
/// for the marker's own bytes, so the line is the shell's line. Afterwards
/// the blocks are reassembled.
fn feed_with_marks(faced: &mut Inner, bytes: &[u8], state: &MarkState) {
    // The cursor mirror sees the same bytes the emulator is about to advance.
    note_cursor_sequences(state, bytes);
    let segments = state.scanner.lock().unwrap().split_feed(bytes);
    if segments.is_empty() {
        return;
    }
    for segment in segments {
        match segment {
            ScanSegment::Emit(raw) => {
                // One counted advance per escape sequence: `ESC[2J` scrolls
                // the viewport into history and `ESC[3J` wipes it, so sharing
                // an advance would hide the scrolled lines from the
                // before/after history comparison and undercount. Text runs
                // keep the row-sized cap from `feed_slice_len`.
                let mut start = 0;
                while start < raw.len() {
                    let end = emit_advance_end(&faced.term, &raw, start);
                    advance_counted(faced, state, &raw[start..end]);
                    start = end;
                }
            }
            ScanSegment::Marker { raw, kind, exit, command } => {
                advance_counted(faced, state, &raw);
                let evicted = state.evicted.load(Ordering::Acquire);
                let line =
                    absolute_line(&faced.term, evicted, faced.term.grid().cursor.point.line);
                let at = std::time::Instant::now();
                // `seq` is overwritten by `push_mark`: identity is minted at
                // the event, never derived from geometry.
                state.scanner.lock().unwrap().push_mark(Mark { line, kind, exit, command, at, seq: 0 });
            }
            ScanSegment::AltSwitch { raw } => {
                enter_or_exit_alt(faced, state, &raw);
            }
        }
    }
    rebuild_blocks(&faced.term, state);
}

/// Bytes of CSI carry kept between feeds: any cursor sequence is far shorter,
/// so a sequence split across two backend chunks still parses as one.
const CURSOR_CARRY: usize = 64;

/// Mirrors the cursor-affecting control sequences in `bytes` into the
/// session's blink state (D19): DECSCUSR (`CSI Ps SP q`), mode 12
/// (`CSI ? … 12 h/l`) and RIS (`ESC c`, back to the blinking default). Runs
/// on the same bytes the emulator advances, so the mirror agrees with the
/// term's explicit-steady bit — the one thing `Term::cursor_style` cannot
/// report, since an explicit steady block reads exactly like the default.
fn note_cursor_sequences(state: &MarkState, bytes: &[u8]) {
    let mut scan = state.cursor.lock().unwrap();
    // The carry plus this feed, so a split sequence still parses as one.
    let mut window = std::mem::take(&mut scan.tail);
    window.extend_from_slice(bytes);
    let mut i = 0;
    while i < window.len() {
        if window[i] != 0x1b {
            i += 1;
            continue;
        }
        let Some(&next) = window.get(i + 1) else {
            break; // Split ESC: carry it.
        };
        if next == b'c' {
            // RIS: alacritty's full reset clears the cursor style.
            scan.mode = CursorBlink::Default;
            i += 2;
            continue;
        }
        if next != b'[' {
            // Any other two-byte escape: skip both.
            i += 2;
            continue;
        }
        // A CSI: one private marker, params, intermediates, then the final.
        let mut j = i + 2;
        let mut private = None;
        if let Some(&b) = window.get(j) {
            if matches!(b, b'<' | b'=' | b'>' | b'?') {
                private = Some(b);
                j += 1;
            }
        }
        let params_start = j;
        while let Some(&b) = window.get(j) {
            if (0x30..=0x3f).contains(&b) {
                j += 1;
            } else {
                break;
            }
        }
        let params_end = j;
        let intermediates_start = j;
        while let Some(&b) = window.get(j) {
            if (0x20..=0x2f).contains(&b) {
                j += 1;
            } else {
                break;
            }
        }
        let Some(&final_byte) = window.get(j) else {
            break; // Split mid-sequence: carry from the ESC.
        };
        if !(0x40..=0x7e).contains(&final_byte) {
            i += 2; // Not a CSI after all: rescan inside.
            continue;
        }
        let params = &window[params_start..params_end];
        let intermediates = &window[intermediates_start..j];
        if final_byte == b'q' && private.is_none() && intermediates == *b" " {
            // DECSCUSR: 0 resets to the default, odd blinks, even steadies.
            scan.mode = match first_param(params) {
                0 => CursorBlink::Default,
                ps if ps % 2 == 1 => CursorBlink::Blink,
                _ => CursorBlink::Steady,
            };
        } else if private == Some(b'?') && (final_byte == b'h' || final_byte == b'l') {
            // Mode 12 anywhere in a ?-list: set blinks, reset steadies.
            if params_contain(params, 12) {
                scan.mode =
                    if final_byte == b'h' { CursorBlink::Blink } else { CursorBlink::Steady };
            }
        }
        i = j + 1;
    }
    // Carry the tail for the next feed.
    let keep = CURSOR_CARRY.min(window.len());
    scan.tail = window[window.len() - keep..].to_vec();
}

/// The first CSI param value, saturating; missing or empty reads 0, the way
/// the emulator's own defaulting does.
fn first_param(params: &[u8]) -> u32 {
    params
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .fold(0u32, |acc, b| acc.saturating_mul(10).saturating_add(u32::from(b - b'0')))
}

/// Whether a `;`-separated CSI param list holds `want` as a whole number
/// (subparams after `:` do not count).
fn params_contain(params: &[u8], want: u32) -> bool {
    params.split(|b| *b == b';').any(|segment| {
        let digits: Vec<u8> =
            segment.iter().take_while(|b| b.is_ascii_digit()).copied().collect();
        !digits.is_empty() && first_param(&digits) == want
    })
}

/// Slice length for one counted advance, from the live row count so a resize
/// is picked up on the next feed. Never below 64 bytes so a tall window does
/// not degenerate into per-byte feeding.
fn feed_slice_len(term: &Term<SessionEventProxy>) -> usize {
    let rows = term.screen_lines().max(1);
    (SCROLLBACK_SLACK * 4 / rows).max(64)
}

/// Length of the escape sequence at `raw[start]` (which is `ESC`), final byte
/// included: CSI (`ESC [` … final byte `0x40..=0x7E`), OSC and the other
/// `ESC`-introduced strings (`ESC ]`, `ESC P/X/^/_`, terminated by `BEL`,
/// `ESC \` or — exactly like vte — any other `ESC`, which a buggy `PS1`
/// title or a program dying mid-string can leave behind), charset selections
/// (`ESC ( X`, three bytes), and every other two-byte sequence (including
/// RIS, `ESC c`). An unterminated sequence runs to the end of the chunk.
/// Always at least 1 and never past `raw.len()`.
fn escape_len(raw: &[u8], start: usize) -> usize {
    const BEL: u8 = 0x07;
    const ST_FINAL: u8 = 0x5c;
    const CAN: u8 = 0x18;
    const SUB: u8 = 0x1a;
    /// 8-bit ST. vte ends a DCS on it and consumes it (`vte/src/lib.rs:331`),
    /// but its OSC and SOS/PM/APC states have no case for the byte at all —
    /// there it is ordinary payload (`advance_osc_string`, `anywhere`). So
    /// this terminates a DCS and nothing else; treating it as a general
    /// terminator swallows an OSC's remaining bytes and every line after it.
    const ST_8BIT: u8 = 0x9c;
    let rest = &raw[start..];
    if rest.len() < 2 {
        return rest.len();
    }
    let end = match rest[1] {
        b'[' => {
            // CSI: parameter and intermediate bytes after the `[`, then a
            // final byte. (The scan starts past the introducer: `[` itself
            // is in the final-byte range.)
            rest[2..]
                .iter()
                .position(|&b| (0x40..=0x7E).contains(&b))
                .map(|i| start + 2 + i + 1)
                .unwrap_or(raw.len())
        }
        b']' | b'P' | b'X' | b'^' | b'_' => {
            // BEL-, ST-, CAN-, SUB- or ESC-terminated string, exactly like
            // vte: CAN (0x18) and SUB (0x1a) abort the string, and any ESC
            // ends it. A bare ESC, CAN or SUB is left for the next advance
            // to parse as a new sequence (the emulator executes the
            // control); BEL and 7-bit ST consume their terminator, and 8-bit
            // ST does too — but only for a DCS, which is the one state vte
            // ends on it.
            let mut i = 2;
            let mut end = raw.len();
            while i < rest.len() {
                if rest[i] == BEL {
                    end = start + i + 1;
                    break;
                }
                if rest[i] == CAN || rest[i] == SUB {
                    end = start + i;
                    break;
                }
                if rest[i] == ST_8BIT && rest[1] == b'P' {
                    end = start + i + 1;
                    break;
                }
                if rest[i] == ESC {
                    if rest.get(i + 1) == Some(&ST_FINAL) {
                        end = start + i + 2;
                    } else {
                        end = start + i;
                    }
                    break;
                }
                i += 1;
            }
            end
        }
        b'(' | b')' | b'*' | b'+' => start + 3.min(rest.len()),
        _ => start + 2.min(rest.len()),
    };
    end.max(start + 1).min(raw.len())
}

/// End offset (exclusive) of the next counted advance starting at `start`:
/// a text run up to the next `ESC` (capped at [`feed_slice_len`]), or one
/// escape sequence ending at its FINAL BYTE. An escape-prefixed advance used
/// to run to the next `ESC` or the end of the chunk, uncapped, so the
/// row-sized cap never applied to the plain text following any escape: the
/// emulator discarded lines past its own cap before the trim step could count
/// them. Every advance holds at most one escape, so a scroll-into-history
/// and the wipe that follows can never share one before/after history
/// comparison. Always past `start`.
fn emit_advance_end(term: &Term<SessionEventProxy>, raw: &[u8], start: usize) -> usize {
    if raw[start] == ESC {
        escape_len(raw, start)
    } else {
        let cap = (start + feed_slice_len(term)).min(raw.len());
        match raw[start..cap].iter().position(|&b| b == ESC) {
            Some(rel) => start + rel,
            None => cap,
        }
    }
}

/// Advances the emulator by `bytes`, counting every departure from the
/// retained window: `ESC[3J` (`Grid::clear_history`) and `ESC c`
/// (`grid.reset()`) shrink history without ever passing through
/// [`trim_history`], so any shrink across the advance is absorbed into
/// `evicted` first and the trim's own contribution is then counted on top
/// without double counting.
///
/// The whole rule is history-only: `evicted` grows by
/// `history_before.saturating_sub(history_after)` per advance. Cursor
/// movement inside the screen (`ESC[nA` redraws, `ESC[H` homing) moves no
/// line out of the retained window and counts nothing on its own.
///
/// One tail guard remains, and only where lines really left: when the
/// advance shrank history, the wipe also displaced the cursor (a full reset
/// homes it in the same advance), so the residual is topped up to keep the
/// absolute tail monotonic. Advances without a shrink — every cursor-up
/// redraw — never touch `evicted`, however far the cursor moved.
///
/// While the alternate screen is active the grid is swapped, not wiped, so
/// nothing is counted at all (see [`enter_or_exit_alt`]). The scanner
/// isolates every switch in its own slice, so a transition never shares an
/// advance with counted bytes; the mid-slice branch below is only a safety
/// net.
fn advance_counted(faced: &mut Inner, state: &MarkState, bytes: &[u8]) {
    if faced.term.mode().contains(TermMode::ALT_SCREEN) {
        faced.processor.advance(&mut faced.term, bytes);
        return;
    }
    let e0 = state.evicted.load(Ordering::Acquire);
    let h0 = faced.term.grid().history_size() as i64;
    let r0 = faced.term.grid().cursor.point.line.0 as i64;
    faced.processor.advance(&mut faced.term, bytes);
    if faced.term.mode().contains(TermMode::ALT_SCREEN) {
        *state.alt_saved_history.lock().unwrap() = Some(h0.max(0) as usize);
        return;
    }
    let mid = faced.term.grid().history_size();
    if (mid as i64) < h0 {
        state.evicted.fetch_add(h0 - (mid as i64), Ordering::AcqRel);
        let e1 = state.evicted.load(Ordering::Acquire);
        let tail_before = e0 + h0 + r0;
        let tail_after =
            e1 + mid as i64 + faced.term.grid().cursor.point.line.0 as i64;
        if tail_after < tail_before {
            state.evicted.fetch_add(tail_before - tail_after, Ordering::AcqRel);
        }
    }
    // Safety net: the emulator discards past its own cap
    // (`SCROLLBACK + SCROLLBACK_SLACK`) before the trim step can count.
    if mid >= SCROLLBACK + SCROLLBACK_SLACK {
        state.desync.fetch_add(1, Ordering::AcqRel);
    }
    trim_history(&mut faced.term, state);
}

/// Advances the emulator past one alt-screen switch sequence with no
/// eviction accounting: entering swaps to a grid with no scrollback (history
/// drops to 0 with nothing evicted) and exiting restores the main grid, so
/// neither direction may move `evicted`. The main history size is
/// snapshotted on entry; on exit the snapshot is dropped — the emulator
/// restored the main grid itself — and accounting resumes from the live
/// grid. Switches are always slice boundaries, so a `clear` sharing the pump
/// is a separate slice accounted on its own.
fn enter_or_exit_alt(faced: &mut Inner, state: &MarkState, raw: &[u8]) {
    if !faced.term.mode().contains(TermMode::ALT_SCREEN) {
        *state.alt_saved_history.lock().unwrap() = Some(faced.term.grid().history_size());
    }
    faced.processor.advance(&mut faced.term, raw);
    if !faced.term.mode().contains(TermMode::ALT_SCREEN) {
        let _saved = state.alt_saved_history.lock().unwrap().take();
    }
}

/// Counts what overflowed the retained window and restores the headroom:
/// afterwards `history_size() <= SCROLLBACK` and the cap is back at
/// `SCROLLBACK + SCROLLBACK_SLACK`. Overflow past [`SCROLLBACK`] is counted
/// here; drops below it (`ESC[3J`, `ESC c`) are counted in
/// [`advance_counted`]. The evicted count is exact unless the safety net
/// fired (see [`TerminalSession::history_desyncs`]), but the absolutes stay
/// monotonic either way.
fn trim_history(term: &mut Term<SessionEventProxy>, state: &MarkState) {
    let over = term.grid().history_size().saturating_sub(SCROLLBACK);
    if over > 0 {
        state.evicted.fetch_add(over as i64, Ordering::AcqRel);
        term.grid_mut().update_history(SCROLLBACK);
        term.grid_mut().update_history(SCROLLBACK + SCROLLBACK_SLACK);
    }
}

/// The oldest retained absolute line: everything below it has been evicted.
fn evicted_floor(state: &MarkState) -> i32 {
    state.evicted.load(Ordering::Acquire).clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// The absolute grid line for a screen-relative [`Line`]: evicted lines plus
/// history lines behind plus the line itself. Monotonic for the life of the
/// session: `evicted` starts at 0 and never decreases, and every drop from
/// the retained window (`ESC[3J`, `ESC c`, overflow) is counted there. Exact
/// unless [`TerminalSession::history_desyncs`] fired after a burst past the
/// emulator's own cap.
fn absolute_line(term: &Term<SessionEventProxy>, evicted: i64, line: Line) -> i32 {
    (evicted + term.grid().history_size() as i64 + line.0 as i64) as i32
}

/// The inverse of [`absolute_line`]: an absolute line back to a
/// screen-relative [`Line`]. These two helpers own every origin computation;
/// route new code through them so the origin can never drift again.
fn screen_line(term: &Term<SessionEventProxy>, evicted: i64, absolute: i32) -> Line {
    Line((absolute as i64 - evicted - term.grid().history_size() as i64) as i32)
}

/// Reassembles the blocks from the accepted marks against the current grid,
/// pruning what fell out of the retained window and carrying the host-set
/// authors (and surviving command text) across by block id — never by index,
/// which pruning shifts.
fn rebuild_blocks(term: &Term<SessionEventProxy>, state: &MarkState) {
    let evicted = state.evicted.load(Ordering::Acquire);
    let floor = evicted_floor(state);
    let tail = absolute_line(term, evicted, term.grid().cursor.point.line);
    let text = |a: i32, b: i32| grid_range_text(term, evicted, a, b);
    // Assemble from everything first so blocks straddling the eviction floor
    // report their full ranges; their marks are then kept while fully-evicted
    // blocks (and their marks) are dropped.
    let marks: Vec<Mark> = state.scanner.lock().unwrap().marks().to_vec();
    if marks.is_empty() {
        return;
    }
    let full = assemble_blocks(&marks, tail, &text);
    let keep: Vec<(i32, i32)> =
        full.iter().filter(|b| b.end >= floor).map(|b| (b.prompt.0, b.end)).collect();
    state.scanner.lock().unwrap().prune_before(floor, &keep);
    let marks: Vec<Mark> = state.scanner.lock().unwrap().marks().to_vec();
    let mut fresh = assemble_blocks(&marks, tail, &text);
    fresh.retain(|b| b.end >= floor);
    let mut stored = state.blocks.lock().unwrap();
    let mut used = vec![false; stored.len()];
    for block in fresh.iter_mut() {
        // Stable identity is minted at the event: the block's `id` is the
        // sequence number of the mark that opened it, so two blocks sharing
        // a start anchor never collide and there is nothing left to match
        // on. Host-set state carries across by id lookup exactly.
        let mut found: Option<usize> = None;
        for (i, old) in stored.iter().enumerate() {
            if !used[i] && old.id == block.id {
                found = Some(i);
                break;
            }
        }
        match found {
            Some(i) => {
                used[i] = true;
                let old = &stored[i];
                block.author = old.author;
                if block.command.is_empty() && !old.command.is_empty() {
                    // The lines scrolled past the retained window: keep what
                    // the grid can no longer tell us.
                    block.command.clone_from(&old.command);
                }
            }
            None => {
                // First sight of this opening mark: the assembler already
                // minted the id, so there is nothing to assign.
            }
        }
    }
    *stored = fresh;
}

/// The trimmed, ANSI-free text of one absolute grid line, or `None` when the
/// line is older than the retained scrollback or past the live tail.
fn grid_line_text(term: &Term<SessionEventProxy>, evicted: i64, absolute: i32) -> Option<String> {
    use alacritty_terminal::term::cell::Flags;
    let line = screen_line(term, evicted, absolute);
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
fn grid_range_text(term: &Term<SessionEventProxy>, evicted: i64, start: i32, end: i32) -> String {
    let from = start.max(0);
    let mut lines = Vec::new();
    let mut line = from;
    while line < end {
        lines.push(grid_line_text(term, evicted, line).unwrap_or_default());
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
                mark_dirty(shared);
            }
            SideEvent::ChildExit(code) => {
                let code = code.unwrap_or(-1);
                *shared.exit.lock().unwrap() = Some(code);
                shared.pending.lock().unwrap().push(SessionEvent::Exited(code));
                mark_dirty(shared);
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
/// Side padding of the grid text area, from the token scale: the block
/// terminal's own side rhythm, so the two panes align. Text, cursor and
/// chrome all live inside it; the layout probe measures the inset box, so the
/// program is told the columns it really has.
const GRID_PAD_X: f32 = aui_tokens::scale::SP_4;
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
    /// The program asked for a blinking cursor: a DECSCUSR blinking variant
    /// or mode 12 (`Term::cursor_style`). `CursorBlinkingChange` already
    /// marks the session dirty, so this is fresh on every snapshot.
    blinking: bool,
    /// The program explicitly asked for a steady cursor: a DECSCUSR steady
    /// variant or mode 12 off (see [`note_cursor_sequences`]). The default —
    /// never touched — is NOT steady: a terminal cursor blinks by default.
    steady: bool,
    /// The cursor sits on a two-cell (wide) character: the drawn block spans
    /// two cells so it covers the glyph instead of its first half.
    wide: bool,
}

/// One block's overlay chrome, owned so the term lock is released before any
/// element is built. Only blocks intersecting the viewport are returned.
#[derive(Debug, Clone)]
struct BlockChrome {
    /// Index into the session's block list; intents carry this back.
    index: usize,
    /// Screen row anchoring the chrome (the block's end line, clamped).
    row: usize,
    /// Leading screen column of the chrome strip: the strip's own text width
    /// back from the row's trailing edge, so the chrome sits against the
    /// grid's right inset instead of mid-sentence after the prompt. The
    /// overlay returns no entry when the row's text reaches within one cell
    /// of this column; a hidden status beats an unreadable one, and a full
    /// row always hides. A finished failure degrades instead of hiding
    /// (D18): the shown command or label may be shortened, and `col` follows
    /// the shown text.
    col: usize,
    /// Gutter status glyph: running, failed or done.
    glyph: &'static str,
    /// Exit code and duration, via the parser's shared labels.
    label: String,
    /// The block's command text as the overlay renders it: the `C` payload
    /// when the emitter sent one, else the grid scrape. Highlighted with
    /// [`syntax::command_runs`] — never the grid's own rows.
    command: String,
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
            // The explicit-steady mirror (D19): steady only when the program
            // said so and the term agrees it is not blinking.
            let steady = self.mark_state.cursor.lock().unwrap().mode == CursorBlink::Steady
                && !term.cursor_style().blinking;
            let cursor = match content.cursor.shape {
                CursorShape::Hidden => None,
                shape => {
                    let row = content.cursor.point.line.0 + disp;
                    let col = content.cursor.point.column.0;
                    if row < 0 || (row as usize) >= out.len() || col >= cols {
                        None
                    } else {
                        Some(CursorCell {
                            row: row as usize,
                            col,
                            shape,
                            blinking: term.cursor_style().blinking,
                            steady,
                            wide: is_wide_lead(term, content.cursor.point.line, col, cols),
                        })
                    }
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
    /// Positions in the grid's side padding clamp to the nearest column
    /// (D21): a selection dragged past the text area keeps extending instead
    /// of stopping at the padding edge.
    fn cell_at(&self, position: gpui::Point<gpui::Pixels>) -> Option<(usize, usize)> {
        let bounds = (*self.bounds.lock().unwrap())?;
        let metrics = (*self.metrics.lock().unwrap())?;
        if metrics.advance <= 0.0 || metrics.line_height <= 0.0 {
            return None;
        }
        let (cols, rows) = self.cells();
        if cols == 0 || rows == 0 {
            return None;
        }
        let x = f32::from(position.x - bounds.origin.x);
        let y = f32::from(position.y - bounds.origin.y);
        let col = (x / metrics.advance).floor().clamp(0.0, f32::from(cols - 1)) as usize;
        let row =
            (y / metrics.line_height).floor().clamp(0.0, f32::from(rows - 1)) as usize;
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

use aui_motion::{looping, Loop};
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
/// hollow when unfocused, blinking unless the program asked for steady), and
/// the overlay:
/// per-block chrome (status glyph, exit code and duration, author mark) plus
/// the jump-to-latest affordance while detached. The text area is inset by
/// [`GRID_PAD_X`] on both sides, and the chrome strip starts clear of the
/// row's own text. The overlay hides entirely while the program owns the
/// alternate screen (decision D44).
#[derive(gpui::IntoElement)]
pub struct TerminalGrid {
    session: gpui::Entity<TerminalSession>,
    option_as_meta: bool,
    show_actions: bool,
    on_intent: Option<IntentHandler>,
    focus: Option<gpui::FocusHandle>,
}

/// The terminal grid fed by `session` (held in a gpui `Entity` the host
/// owns). State lives in the session; this component only renders.
pub fn terminal_grid(session: &gpui::Entity<TerminalSession>) -> TerminalGrid {
    TerminalGrid {
        session: session.clone(),
        option_as_meta: false,
        show_actions: false,
        on_intent: None,
        focus: None,
    }
}

impl TerminalGrid {
    /// Treats Option as Meta for key encoding (the macOS terminal setting).
    pub fn option_as_meta(mut self, option_as_meta: bool) -> Self {
        self.option_as_meta = option_as_meta;
        self
    }

    /// Shows the per-block Copy / Rerun / Stop / Ask buttons in the overlay
    /// chrome. Off by default: nothing appears unless the host asks, so a
    /// grid built with no opt-in renders no action controls at all (the
    /// library's global UI rule against unnecessary actions). The intents
    /// stay in the API either way.
    pub fn show_actions(mut self, show_actions: bool) -> Self {
        self.show_actions = show_actions;
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

    /// The focus handle the grid tracks, focuses on click and tests for its
    /// cursor blink (D15): the host's own handle — the dock already owns one
    /// for the pane. Without it the grid falls back to one cached handle per
    /// session, so every render still tracks the same handle. Never mint one
    /// per render: a fresh handle can never be focused, and its refcount
    /// hitting zero drops the window's keyboard focus a frame later.
    pub fn focus_handle(mut self, handle: gpui::FocusHandle) -> Self {
        self.focus = Some(handle);
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
        // The cached grid (D20): blink frames share it instead of rebuilding
        // the whole snapshot at refresh rate.
        let snapshot = self.session.read(cx).snapshot_cached(&palette);
        // The element's own handle (D15): the host's when told, else the one
        // cached handle per session — never a fresh handle per render, which
        // could never be focused and dropped window focus a frame later.
        let focus =
            self.focus.clone().unwrap_or_else(|| self.session.read(cx).focus_handle(cx));
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
        let grid_id = gpui::ElementId::named_usize(
            "terminal-grid",
            self.session.entity_id().as_u64() as usize,
        );
        // Layer 3: the cursor — block, bar or underline per DECSCUSR —
        // drawn hollow when the element is not focused. It blinks when
        // focused unless the program explicitly asked for a steady shape
        // (D19); a steady or unfocused cursor never touches the motion
        // clock, so it subscribes to no frames and forces no repaint.
        let mut cursor_el = div();
        if let Some(cursor) = snapshot.cursor {
            let want_blink = focused && cursor_blinks(&cursor);
            // Under reduced motion `looping` holds its resting phase without
            // subscribing; `blink_visible` still forces the cursor on (D19).
            let phase = if want_blink {
                looping(
                    (grid_id.clone(), "cursor-blink"),
                    Loop::linear(aui_tokens::scale::D_SLOW * 4).resting(0.0),
                    window,
                    cx,
                )
            } else {
                0.0
            };
            let on = blink_visible(want_blink, cx.reduce_motion(), phase);
            let (x, y, w, h) = cursor_rect(&cursor, advance, line_height);
            cursor_el = match cursor.shape {
                // A hollow block is already the unfocused look: always an
                // outline, whatever the focus.
                CursorShape::HollowBlock => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .bg(gpui::transparent_black())
                    .border_1()
                    .border_color(palette.term_cursor),
                CursorShape::Block => {
                    let mut el = div()
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(w))
                        .h(px(h))
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
                    .h(px(h))
                    .bg(palette.term_cursor),
                CursorShape::Underline => div()
                    .absolute()
                    .left(px(x))
                    .top(px(h - CURSOR_BAR_W + y))
                    .w(px(w))
                    .h(px(CURSOR_BAR_W))
                    .bg(palette.term_cursor),
                CursorShape::Hidden => div(),
            };
            if !on {
                cursor_el = cursor_el.opacity(0.0);
            }
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
                    self.show_actions,
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

        // The inset text area: every paint layer lives inside the side
        // padding, so the probe measures the columns the program really has
        // and hit-testing needs no offset.
        let content = div()
            .absolute()
            .top(px(0.0))
            .bottom(px(0.0))
            .left(px(GRID_PAD_X))
            .right(px(GRID_PAD_X))
            .child(backgrounds)
            .child(lines)
            .child(cursor_el)
            .child(overlay)
            .child(layout_probe(&self.session, advance, line_height));
        div()
            .id(grid_id)
            .relative()
            .size_full()
            .bg(palette.term_bg)
            .text_color(palette.term_fg)
            .track_focus(&focus)
            .child(content)
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
                        let hovered = if event.modifiers.platform { cell } else { None };
                        if *session.hover.lock().unwrap() != hovered {
                            *session.hover.lock().unwrap() = hovered;
                            // Hover underlines are a snapshot input (D20).
                            session.bump_snapshot();
                        }
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

/// One block's overlay strip at its anchor row: status glyph, exit code and
/// duration, author mark, then the action row — but only when the host opted
/// in (see [`TerminalGrid::show_actions`]) — raising [`TerminalGridIntent`]s.
/// The strip starts at the anchor column and shrink-wraps its content, so the
/// whole strip stays visible against the grid's right inset instead of
/// running off it (D17); the session hides the strip — or degrades a
/// failure — when the row's text reaches it, so it never covers the row's
/// own text. Overflow goes through [`popover_layer`](aui::overlay::popover_layer),
/// like the jump affordance.
fn block_chrome_el(
    chrome: &BlockChrome,
    advance: f32,
    line_height: f32,
    palette: &aui_tokens::Palette,
    show_actions: bool,
    on_intent: Option<IntentHandler>,
) -> impl gpui::IntoElement {
    use aui_tokens::scale;
    use gpui::{div, prelude::*, px};
    // The header earns its colour from state, resolved through the token
    // palette so both themes read: running is in-progress, failed is a
    // failure, succeeded stays quiet.
    let kind = if chrome.running {
        BlockStatus::Running
    } else if chrome.failed {
        BlockStatus::Failed
    } else {
        BlockStatus::Succeeded
    };
    let status = palette.color(block_status_token(kind)).unwrap_or(palette.ink_2);
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
    let mut inner = div().flex().flex_row().items_center().child(gutter).child(meta);
    // The chrome only: the block's own command text, highlighted. The
    // grid's rows are untouched — program output renders from its SGR alone.
    if !chrome.command.is_empty() {
        inner = inner.child(
            div()
                .ml(px(scale::SP_1))
                .overflow_hidden()
                .whitespace_nowrap()
                .mono(scale::FS_11)
                .text_color(palette.ink)
                .child(
                    gpui::StyledText::new(chrome.command.clone())
                        .with_runs(syntax::command_runs(&chrome.command, palette)),
                ),
        );
    }
    inner = inner.child(div().flex_1());
    // No opt-in, no action row at all: not even an empty container.
    let buttons = chrome_action_buttons(show_actions, index);
    if !buttons.is_empty() {
        let mut actions = div().flex().flex_row().items_center();
        for (name, intent) in buttons {
            let label = match intent {
                TerminalGridIntent::Copy(_) => "Copy",
                TerminalGridIntent::Rerun(_) => "Rerun",
                TerminalGridIntent::Stop(_) => "Stop",
                TerminalGridIntent::Ask(_) => "Ask",
                TerminalGridIntent::OpenUrl(_) => continue,
            };
            actions = actions.child(div().ml(px(scale::SP_1)).child(action(name, label, intent)));
        }
        inner = inner.child(actions);
    }
    // Shrink-wrapped (D17): the box is exactly the strip's real rendered
    // width, so the status tail cannot fall off the right edge the way the
    // old stretched-and-clipped box cut it. The pill padding past the text
    // cells lands in the grid's right inset, still inside the pane.
    let strip = div()
        .absolute()
        .top(px(chrome.row as f32 * line_height))
        .left(px(chrome.col as f32 * advance))
        .child(inner);
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

/// Whether the grid cell at (`line`, `col`) leads a wide character: flagged
/// wide, or followed by its spacer when the flag was already consumed by an
/// overwrite. Spacers themselves are never leads; out-of-range columns read
/// narrow.
fn is_wide_lead(term: &Term<SessionEventProxy>, line: Line, col: usize, cols: usize) -> bool {
    use alacritty_terminal::term::cell::Flags;
    if col >= cols {
        return false;
    }
    let cell = &term.grid()[Point::new(line, Column(col))];
    if cell.flags.contains(Flags::WIDE_CHAR) {
        return true;
    }
    col + 1 < cols
        && term.grid()[Point::new(line, Column(col + 1))]
            .flags
            .contains(Flags::WIDE_CHAR_SPACER)
}

/// Used text cells on a screen row: one past the last occupied cell, with a
/// leading wide char counting two. A cell is occupied when it holds a
/// character — or when SGR painted a background under a blank (D18): a
/// trailing run of spaces with a background colour is visible colour, not
/// free space, so the chrome must not land on it. The chrome's separation
/// check measures from here, so the strip can never cover the row's own
/// text; a full row yields `cols`.
fn anchor_used_cols(term: &Term<SessionEventProxy>, line: Line, cols: usize) -> usize {
    use alacritty_terminal::term::cell::Flags;
    let mut used = 0usize;
    for col in 0..cols {
        let cell = &term.grid()[Point::new(line, Column(col))];
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let blank = cell.c == '\0' || cell.c == ' ';
        // An inverse blank shows the foreground as its ground; any other
        // non-default background paints the cell directly.
        let painted = cell.flags.contains(Flags::INVERSE)
            || cell.bg != Color::Named(NamedColor::Background);
        if !blank || painted {
            used = col + if is_wide_lead(term, line, col, cols) { 2 } else { 1 };
        }
    }
    used.min(cols)
}

/// Grid cells the chrome strip's text needs: the status glyph, the meta
/// label, the command and the agent mark in display cells (see
/// [`str_cells`]). The strip sits this wide at the row's trailing edge, so
/// its real rendered width — including the author mark — stays inside the
/// grid instead of running off it (D17).
fn chrome_cells(glyph: &str, label: &str, command: &str, author: BlockAuthor) -> usize {
    let mark = match author {
        BlockAuthor::Agent => str_cells("M"),
        BlockAuthor::Human => 0,
    };
    str_cells(glyph) + str_cells(label) + str_cells(command) + mark
}

/// Display cells in `text`: one per character, two per East-Asian wide or
/// fullwidth character (see [`is_wide`]). An unlisted character counts one,
/// which only ever narrows the strip — clipped at the trailing edge — and
/// can never push it into the row's own text.
fn str_cells(text: &str) -> usize {
    text.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

/// Whether `c` fills two grid cells: East-Asian Wide or Fullwidth
/// (Hiragana, Katakana, CJK unified and compatibility, Hangul, fullwidth
/// ASCII and punctuation). This mirrors what the emulator flags `WIDE_CHAR`;
/// emoji and other ambiguous widths count one (see [`str_cells`]).
fn is_wide(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x3FFFD
    )
}

/// Whether the cursor wants to blink (D19): the program asked for blinking,
/// or it never asked for steady — a terminal cursor blinks by default, and
/// only an explicit steady shape (a DECSCUSR steady variant, or mode 12 off)
/// holds it still.
fn cursor_blinks(cursor: &CursorCell) -> bool {
    cursor.blinking || !cursor.steady
}

/// Whether the cursor paints this frame (D19): no blink wanted, or reduced
/// motion, means a steady VISIBLE cursor — never a hidden one. Under reduced
/// motion the loop holds its resting phase, which a phase test could read as
/// off; short-circuiting keeps the cursor on without touching the clock.
fn blink_visible(want_blink: bool, reduce_motion: bool, phase: f32) -> bool {
    if !want_blink || reduce_motion {
        true
    } else {
        phase < 0.5
    }
}

/// The cursor's pixel rect `(x, y, w, h)`: the grid cell's position — never
/// measured from the row text, so wide lines cannot shift it — two cells wide
/// when it sits on a wide character.
fn cursor_rect(cursor: &CursorCell, advance: f32, line_height: f32) -> (f32, f32, f32, f32) {
    let width = if cursor.wide { 2.0 } else { 1.0 };
    (
        cursor.col as f32 * advance,
        cursor.row as f32 * line_height,
        width * advance,
        line_height,
    )
}

/// Whole columns for a measured content width, however small the pane is
/// drawn (see [`MIN_COLS`]).
fn content_cols(width_px: f32, advance: f32) -> u16 {
    ((width_px / advance).floor() as u16).clamp(MIN_COLS, u16::MAX)
}

/// The per-block action buttons behind the host opt-in, left to right: empty
/// unless the host asked for them (see [`TerminalGrid::show_actions`]). The
/// intents stay in the API either way.
fn chrome_action_buttons(show_actions: bool, index: usize) -> Vec<(&'static str, TerminalGridIntent)> {
    if !show_actions {
        return Vec::new();
    }
    vec![
        ("term-block-copy", TerminalGridIntent::Copy(index)),
        ("term-block-rerun", TerminalGridIntent::Rerun(index)),
        ("term-block-stop", TerminalGridIntent::Stop(index)),
        ("term-block-ask", TerminalGridIntent::Ask(index)),
    ]
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
            let cols = content_cols(w, advance);
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
            self.bump_snapshot();
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
        self.bump_snapshot();
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

    fn light() -> aui_tokens::Palette {
        aui_tokens::Palette::for_kind(aui_tokens::ThemeKind::Light)
    }

    /// A running command with `tail` bytes after its `C` marker: the block is
    /// still open, so its chrome anchors on the live tail row.
    fn running_bytes(nonce: &str, echo: &str, tail: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(marker_bytes(nonce, "A"));
        out.extend(b"p$ ");
        out.extend(marker_bytes(nonce, "B"));
        out.extend(echo.as_bytes());
        out.extend(b"\r\n");
        out.extend(marker_bytes(nonce, "C"));
        out.extend(tail);
        out
    }

    /// V3: the chrome sits at the row's trailing edge, not after the text. A
    /// short tail row carries the strip right-aligned — its own text width
    /// back from the last column — so the prompt and the pill never touch. A
    /// full-width tail row hides the chrome entirely and keeps its text
    /// (V1's case: the strip used to clip to nothing here, now it hides).
    /// Without the fix the strip starts at the first free cell, jammed
    /// against the prompt.
    #[test]
    fn chrome_starts_clear_of_the_row_text() {
        for palette in [dark(), light()] {
            // Short tail row: the leading edge sits at the trailing inset.
            let h = live_session("v3trail", 20, 6);
            h.feed(&running_bytes(&h.nonce, "run", b"hi"));
            let chrome = h.session.overlay_chrome();
            assert_eq!(chrome.len(), 1, "{chrome:?}");
            let need =
                1 + chrome[0].label.chars().count() + chrome[0].command.chars().count();
            assert_eq!(chrome[0].col, 20 - need, "not right-aligned: {chrome:?}");
            assert_ne!(chrome[0].col, 2, "jammed against the prompt: {chrome:?}");
            assert!(chrome[0].running);
            // V2 kept: the state colouring resolves and the command highlight
            // covers the text, in this theme.
            assert!(palette.color(block_status_token(BlockStatus::Running)).is_some());
            assert!(palette.color(block_status_token(BlockStatus::Failed)).is_some());
            assert!(palette.color(block_status_token(BlockStatus::Succeeded)).is_some());
            let runs = syntax::command_runs(&chrome[0].command, &palette);
            assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), chrome[0].command.len());
        }
        // Full-width tail row: no chrome, and the text is untouched.
        let h = live_session("v3full", 20, 6);
        h.feed(&running_bytes(&h.nonce, "run", &[b'X'; 20]));
        assert!(h.session.blocks().iter().any(|b| b.running()), "precondition: still running");
        assert!(h.session.overlay_chrome().is_empty(), "chrome drawn over a full row");
        assert!(
            h.session.screen_text().contains(&"X".repeat(20)),
            "row text touched: {:?}",
            h.session.screen_text()
        );
    }

    /// V3: the chrome hides when the row's text reaches within one cell of
    /// it. Grown one cell at a time from a short tail: still drawn with
    /// exactly one blank cell of separation, gone the cell after. Without
    /// the fix the strip follows the text and both halves fail.
    #[test]
    fn chrome_hides_when_text_reaches_within_one_cell_of_it() {
        let h = live_session("v3near", 20, 6);
        h.feed(&running_bytes(&h.nonce, "run", b"hi"));
        let col = {
            let chrome = h.session.overlay_chrome();
            assert_eq!(chrome.len(), 1, "{chrome:?}");
            chrome[0].col
        };
        assert_ne!(col, 2, "jammed against the prompt: {col}");
        // One blank cell of separation: still drawn, leading edge unmoved.
        h.feed(&vec![b'y'; col - 1 - 2]);
        let chrome = h.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "{chrome:?}");
        assert_eq!(chrome[0].col, col, "leading edge moved: {chrome:?}");
        // The separating cell is gone: no chrome rather than a collision.
        h.feed(b"y");
        assert!(h.session.overlay_chrome().is_empty(), "chrome drawn touching the text");
    }

    /// V3: a row of wide characters reaching the trailing edge hides the
    /// chrome too. The trailing cell is a wide-char spacer, so only a
    /// cell-counting measure sees the row as full; a character count would
    /// park the strip mid-text. Without the fix the strip is drawn at the
    /// end of the row.
    #[test]
    fn chrome_hides_when_a_wide_row_reaches_the_trailing_edge() {
        let h = live_session("v3wide", 20, 6);
        h.feed(&running_bytes(&h.nonce, "run", "あ".repeat(10).as_bytes()));
        assert!(h.session.blocks().iter().any(|b| b.running()), "precondition: still running");
        assert!(h.session.overlay_chrome().is_empty(), "chrome drawn over a wide-char row");
        assert!(
            h.session.screen_text().contains(&"あ".repeat(10)),
            "row text touched: {:?}",
            h.session.screen_text()
        );
    }

    /// V1 D1/D2: the drawn cursor rect sits on the grid's cursor cell — for a
    /// plain line and for one with wide characters — and spans two cells on a
    /// wide char, in both themes. Without the fix the cursor is always one
    /// cell wide and carries no blink state.
    #[test]
    fn cursor_rect_matches_the_grid_cell_with_and_without_wide_chars() {
        for palette in [dark(), light()] {
            let plain = pumped(vec![b"hi"], 20, 5);
            let grid = plain.snapshot(&palette);
            let cursor = grid.cursor.expect("a cursor");
            assert_eq!((cursor.row, cursor.col), (0, 2));
            assert!(!cursor.wide, "plain line");
            assert_eq!(cursor_rect(&cursor, 8.0, 16.0), (16.0, 0.0, 8.0, 16.0));
            // A wide char, then two cells back onto it.
            let wide = pumped(vec!["あ".as_bytes(), b"\x1b[2D"], 20, 5);
            let grid = wide.snapshot(&palette);
            let cursor = grid.cursor.expect("a cursor");
            assert_eq!((cursor.row, cursor.col), (0, 0));
            assert!(cursor.wide, "cursor sits on a wide char");
            let (x, y, w, h) = cursor_rect(&cursor, 8.0, 16.0);
            assert_eq!((x, y), (0.0, 0.0));
            assert_eq!(w, 16.0, "two cells wide");
            assert_eq!(h, 16.0);
        }
    }

    /// V1 D2: the snapshot carries the terminal's own blink mode — DECSCUSR's
    /// blinking variants and mode 12 — so the element blinks a willing cursor
    /// and leaves a steady one alone, in both themes.
    #[test]
    fn cursor_blink_state_follows_the_terminal_mode() {
        for palette in [dark(), light()] {
            let h = live_session("v1d2", 20, 5);
            h.feed(b"hi");
            assert!(!h.session.snapshot(&palette).cursor.expect("a cursor").blinking);
            // DECSCUSR blinking block, then steady block.
            h.feed(b"\x1b[1 q");
            assert!(h.session.snapshot(&palette).cursor.expect("a cursor").blinking);
            h.feed(b"\x1b[2 q");
            assert!(!h.session.snapshot(&palette).cursor.expect("a cursor").blinking);
            // Mode 12 set and reset.
            h.feed(b"\x1b[?12h");
            assert!(h.session.snapshot(&palette).cursor.expect("a cursor").blinking);
            h.feed(b"\x1b[?12l");
            assert!(!h.session.snapshot(&palette).cursor.expect("a cursor").blinking);
        }
    }

    /// V1 D3: no per-block action buttons unless the host opts in. The
    /// intents stay in the API; the default carries none.
    #[test]
    fn no_action_buttons_unless_opted_in() {
        assert!(chrome_action_buttons(false, 0).is_empty());
        let intents: Vec<TerminalGridIntent> =
            chrome_action_buttons(true, 3).into_iter().map(|(_, intent)| intent).collect();
        assert_eq!(
            intents,
            vec![
                TerminalGridIntent::Copy(3),
                TerminalGridIntent::Rerun(3),
                TerminalGridIntent::Stop(3),
                TerminalGridIntent::Ask(3),
            ]
        );
    }

    /// V1 D4: the grid's side padding comes from the token scale, and the
    /// probe counts whole columns for a measured width, floored at the
    /// session minimum.
    #[test]
    fn grid_side_padding_comes_from_the_token_scale() {
        assert_eq!(GRID_PAD_X, aui_tokens::scale::SP_4);
        assert_eq!(content_cols(100.0, 10.0), 10);
        assert_eq!(content_cols(1.0, 10.0), MIN_COLS);
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

    /// V2: the chrome carries the command text the `C` marker reports, so
    /// the overlay renders the payload — not a scrape, not nothing.
    #[test]
    fn chrome_carries_the_reported_command_text() {
        let h = live_session("v2chrome", 80, 6);
        let mut bytes = Vec::new();
        bytes.extend(marker_bytes(&h.nonce, "A"));
        bytes.extend(b"prompt$ ");
        bytes.extend(marker_bytes(&h.nonce, "B"));
        bytes.extend(b"echo hi\r\n");
        bytes.extend(
            format!("\x1b]133;C;cmd=echo \"hi\" $HOME;enc=raw;k={}\x07", h.nonce).into_bytes(),
        );
        h.feed(&bytes);
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].running(), "no D arrived yet");
        assert_eq!(blocks[0].command, "echo \"hi\" $HOME");
        let chrome = h.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "{chrome:?}");
        assert_eq!(chrome[0].command, "echo \"hi\" $HOME", "chrome: {chrome:?}");
        assert!(chrome[0].running);
        // The overlay's runs highlight that text without touching the grid.
        let runs = syntax::command_runs(&chrome[0].command, &dark());
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), chrome[0].command.len());
    }

    /// V2: the grid's own row rendering is unchanged — a row of program
    /// output with its own SGR renders from that SGR alone, even when its
    /// text looks like a shell command the chrome would highlight.
    #[test]
    fn program_output_rows_render_from_sgr_alone() {
        let session = pumped(vec![b"\x1b[31mgit \"hi\"\x1b[0m"], 20, 5);
        let palette = dark();
        let row = &session.snapshot(&palette).rows[0];
        assert!(row.text.starts_with("git \"hi\""), "row: {:?}", row.text);
        assert_eq!(row.runs[0].color, palette.ansi_red, "runs: {:?}", row.runs);
        // The chrome highlighter would read `git` as a command in accent —
        // the grid row must not.
        assert_ne!(
            row.runs[0].color,
            syntax::role_color(syntax::SyntaxRole::Command, &palette)
        );
        assert_eq!(row.runs.iter().map(|r| r.len).sum::<usize>(), row.text.len());
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

    // ------------------------------------------------------------------
    // Eviction accounting (monotonic absolute lines past the scrollback cap).
    // ------------------------------------------------------------------

    /// A refillable queue backend: the test pushes output, then pumps.
    struct LiveBackend {
        queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    }

    impl TerminalBackend for LiveBackend {
        fn spawn(&mut self, _shell: &str, _cwd: &Path) -> std::io::Result<()> {
            Ok(())
        }

        fn write(&mut self, _bytes: &[u8]) {}

        fn resize(&mut self, _cols: u16, _rows: u16) {}

        fn poll(&mut self) -> Vec<TermEvent> {
            self.queue.lock().unwrap().drain(..).map(TermEvent::Output).collect()
        }
    }

    /// A nonce-pinned session the test feeds incrementally, one pump per feed.
    struct LiveSession {
        session: TerminalSession,
        queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
        nonce: String,
    }

    impl LiveSession {
        fn feed(&self, bytes: &[u8]) {
            self.queue.lock().unwrap().push_back(bytes.to_vec());
            self.session.pump();
        }
    }

    fn live_session(nonce: &str, cols: u16, rows: u16) -> LiveSession {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let backend = LiveBackend { queue: queue.clone() };
        let session = TerminalSession::new(Box::new(backend), cols, rows).with_nonce(nonce);
        LiveSession { session, queue, nonce: nonce.to_string() }
    }

    /// One nonce-checked marker's raw bytes.
    fn marker_bytes(nonce: &str, body: &str) -> Vec<u8> {
        format!("\x1b]133;{body};k={nonce}\x07").into_bytes()
    }

    /// A finished command with `lines` output lines between `C` and `D`.
    fn command_bytes(nonce: &str, echo: &str, exit: i32, lines: usize, tag: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(marker_bytes(nonce, "A"));
        out.extend(b"prompt$ ");
        out.extend(marker_bytes(nonce, "B"));
        out.extend(echo.as_bytes());
        out.extend(b"\r\n");
        out.extend(marker_bytes(nonce, "C"));
        out.extend(output_bytes(lines, tag));
        out.extend(marker_bytes(nonce, &format!("D;{exit}")));
        out
    }

    /// `lines` plain output lines, each tagged.
    fn output_bytes(lines: usize, tag: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..lines {
            out.extend(format!("{tag}-fill-{i:05}\r\n").into_bytes());
        }
        out
    }

    /// The proved defect: three commands whose output (12,000 lines together)
    /// pushes past the scrollback cap. The old origin stalled at the cap and
    /// collapsed the later `D` marks onto one line; the blocks must stay
    /// distinct with strictly increasing ends and distinct command text.
    #[test]
    fn saturated_outputs_keep_distinct_monotonic_blocks() {
        let h = live_session("l2defect", 80, 24);
        h.feed(&command_bytes(&h.nonce, "cmd-one", 0, 4_000, "one"));
        h.feed(&command_bytes(&h.nonce, "cmd-two", 1, 4_000, "two"));
        h.feed(&command_bytes(&h.nonce, "cmd-three", 0, 4_000, "three"));
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 3, "blocks: {blocks:?}");
        assert!(
            blocks[0].end < blocks[1].end && blocks[1].end < blocks[2].end,
            "ends not strictly increasing: {:?}",
            blocks.iter().map(|b| b.end).collect::<Vec<_>>()
        );
        assert_ne!(blocks[0].end, blocks[1].end);
        assert_ne!(blocks[1].end, blocks[2].end);
        assert!(blocks[0].command.contains("cmd-one"), "command: {:?}", blocks[0].command);
        assert!(blocks[1].command.contains("cmd-two"), "command: {:?}", blocks[1].command);
        assert!(blocks[2].command.contains("cmd-three"), "command: {:?}", blocks[2].command);
    }

    /// The polling stall: past saturation `range_text` from an old cursor
    /// must return the new output and the tail must keep advancing — twice in
    /// a row, to prove it keeps working.
    #[test]
    fn polling_keeps_working_past_saturation() {
        let h = live_session("l2poll", 80, 24);
        h.feed(&output_bytes(11_000, "base"));
        let c1 = h.session.tail_cursor();
        h.feed(&output_bytes(5_000, "second"));
        let text = h.session.range_text(c1);
        assert!(text.contains("second-fill"), "poll stalled past saturation");
        let c2 = h.session.tail_cursor();
        assert!(c2.line > c1.line, "tail stalled: {c1:?} -> {c2:?}");
        h.feed(&output_bytes(5_000, "third"));
        let text2 = h.session.range_text(c2);
        assert!(text2.contains("third-fill"), "poll stalled on the second batch");
        let c3 = h.session.tail_cursor();
        assert!(c3.line > c2.line, "tail stalled again: {c2:?} -> {c3:?}");
    }

    /// Pruning is bounded: ~500 commands with enough output to evict most of
    /// them must not grow the blocks Vec or the scanner marks linearly, and
    /// the survivors are the newest commands.
    #[test]
    fn pruning_bounds_marks_and_blocks() {
        let h = live_session("l2prune", 80, 24);
        let total = 600usize;
        for i in 0..total {
            let echo = format!("prune-cmd-{i:03}");
            let tag = format!("p{i:03}");
            h.feed(&command_bytes(&h.nonce, &echo, 0, 50, &tag));
        }
        let blocks = h.session.blocks();
        let marks = h.session.marks();
        assert!(blocks.len() < 300, "blocks grew linearly: {}", blocks.len());
        assert!(marks.len() < 1200, "marks grew linearly: {}", marks.len());
        assert!(blocks.len() > 50, "pruned too aggressively: {}", blocks.len());
        let last = blocks.last().expect("a surviving block");
        assert!(last.command.contains("prune-cmd-599"), "newest lost: {:?}", last.command);
        assert!(
            !blocks[0].command.contains("prune-cmd-000"),
            "oldest should have been evicted: {:?}",
            blocks[0].command
        );
    }

    /// Ids are stable across pruning: the surviving block keeps its id, and
    /// the host-set author set before the prune is still on it afterwards.
    #[test]
    fn block_ids_survive_pruning_with_authors() {
        let h = live_session("l2ids", 80, 24);
        h.feed(&command_bytes(&h.nonce, "first-cmd", 0, 0, "g"));
        h.feed(&output_bytes(6_000, "gap1"));
        h.feed(&command_bytes(&h.nonce, "second-cmd", 0, 0, "g"));
        let before = h.session.blocks();
        assert_eq!(before.len(), 2);
        assert_ne!(before[0].id, before[1].id, "ids must be distinct");
        let id = before[1].id;
        h.session.set_block_author(1, BlockAuthor::Agent);
        h.feed(&output_bytes(6_000, "gap2"));
        let after = h.session.blocks();
        assert_eq!(after.len(), 1, "the first block should have been evicted: {after:?}");
        assert_eq!(after[0].id, id, "id changed across the prune");
        assert_eq!(after[0].author, BlockAuthor::Agent, "author lost across the prune");
        assert!(after[0].command.contains("second-cmd"), "command: {:?}", after[0].command);
    }

    /// `resize` rebuilds coherently: blocks survive and the grid reads back.
    #[test]
    fn resize_rebuilds_and_stays_coherent() {
        let h = live_session("l2resize", 80, 24);
        h.feed(&command_bytes(&h.nonce, "resize-cmd", 0, 3, "r"));
        assert_eq!(h.session.blocks().len(), 1);
        h.session.resize(40, 10);
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 1, "resize lost the block");
        assert!(blocks[0].command.contains("resize-cmd"), "command: {:?}", blocks[0].command);
        let screen = h.session.screen_text();
        assert!(screen.contains("resize-cmd"), "screen: {screen:?}");
        assert!(h.session.block_text(0).is_some());
    }

    /// Authors survive block rebuilds: set one, force a rebuild, read it back.
    #[test]
    fn authors_survive_block_rebuilds() {
        let h = live_session("l2author", 80, 24);
        h.feed(&command_bytes(&h.nonce, "author-cmd", 0, 2, "a"));
        h.session.set_block_author(0, BlockAuthor::Agent);
        h.session.resize(80, 24);
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].author, BlockAuthor::Agent);
    }

    /// Viewport filtering, partially outside: a tall block still appears when
    /// its top has scrolled above the viewport and when its bottom hangs
    /// below it.
    #[test]
    fn overlay_shows_blocks_partially_outside_the_viewport() {
        let h = live_session("l2chrome", 80, 6);
        h.feed(&command_bytes(&h.nonce, "tall-cmd", 0, 10, "t"));
        // Following the tail: the block's top has scrolled above the viewport.
        let chrome = h.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "tall block missing at the tail: {chrome:?}");
        assert_eq!(chrome[0].index, 0);
        // Scrolled to the top of the retained window: the block's bottom
        // hangs below the viewport.
        h.session.scroll_lines(8);
        let chrome = h.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "tall block missing when scrolled up: {chrome:?}");
        assert_eq!(chrome[0].row, 5, "chrome not clamped to the viewport bottom");
    }

    /// D1: the final output line and `D` arriving in ONE pump (what a real
    /// pty delivers) keeps the block's id and host-set author: the carry-over
    /// keys on the START anchor, which cannot move at finish.
    #[test]
    fn finishing_a_block_in_one_pump_keeps_id_and_author() {
        let h = live_session("l2d1", 80, 24);
        let mut head = Vec::new();
        head.extend(marker_bytes(&h.nonce, "A"));
        head.extend(b"prompt$ ");
        head.extend(marker_bytes(&h.nonce, "B"));
        head.extend(b"agent-cmd\r\n");
        head.extend(marker_bytes(&h.nonce, "C"));
        head.extend(b"line1\r\n");
        h.feed(&head);
        let running = h.session.blocks();
        assert_eq!(running.len(), 1);
        assert!(running[0].running());
        let id = running[0].id;
        h.session.set_block_author(0, BlockAuthor::Agent);
        let mut tail = Vec::new();
        tail.extend(b"line2\r\n");
        tail.extend(marker_bytes(&h.nonce, "D;0"));
        h.feed(&tail);
        let done = h.session.blocks();
        assert_eq!(done.len(), 1);
        assert!(!done[0].running());
        assert_eq!(done[0].id, id, "id changed when the block finished");
        assert_eq!(done[0].author, BlockAuthor::Agent, "author lost when the block finished");
    }

    /// `lines` plain output lines in the audit's `tag-00000` format.
    fn fill_lines(lines: usize, tag: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..lines {
            out.extend(format!("{tag}-{i:05}\r\n").into_bytes());
        }
        out
    }

    /// D2 (`ESC[3J`): `clear` between two commands keeps the tail monotonic,
    /// block ends strictly increasing, and a pre-clear cursor reading forward.
    ///
    /// Exact count: the `clear` genuinely evicts the first command's 11,000
    /// lines — its end falls below the retained floor — so exactly the
    /// `clear` block and `cmd-three` survive. The survivor in front is the
    /// `clear` block itself: its own `ESC[3J` wiped its echoed command line
    /// before its `D` was recorded, and no earlier rebuild ever saw it, so
    /// its command text is genuinely unrecoverable from the grid and reads
    /// empty, with an empty output range to match.
    #[test]
    fn esc_3j_between_commands_keeps_tail_and_order() {
        let h = live_session("l2d2a", 80, 24);
        h.feed(&command_bytes(&h.nonce, "cmd-one", 0, 11_000, "one"));
        let c1 = h.session.tail_cursor();
        // The `clear` command's own output is what ncurses >= 6 emits.
        let mut clear = Vec::new();
        clear.extend(marker_bytes(&h.nonce, "A"));
        clear.extend(b"prompt$ ");
        clear.extend(marker_bytes(&h.nonce, "B"));
        clear.extend(b"clear\r\n");
        clear.extend(marker_bytes(&h.nonce, "C"));
        clear.extend(b"\x1b[H\x1b[2J\x1b[3J");
        clear.extend(marker_bytes(&h.nonce, "D;0"));
        h.feed(&clear);
        let c2 = h.session.tail_cursor();
        assert!(c2.line >= c1.line, "tail went backwards on ESC[3J: {c1:?} -> {c2:?}");
        h.feed(&command_bytes(&h.nonce, "cmd-three", 0, 3, "three"));
        let c3 = h.session.tail_cursor();
        assert!(c3.line >= c2.line, "tail went backwards after clear: {c2:?} -> {c3:?}");
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 2, "block count: {blocks:?}");
        assert!(
            blocks.windows(2).all(|w| w[0].end < w[1].end),
            "ends not strictly increasing: {:?}",
            blocks.iter().map(|b| b.end).collect::<Vec<_>>()
        );
        assert!(
            blocks.iter().all(|b| b.exit == Some(0)),
            "a block lost its exit: {blocks:?}"
        );
        assert!(
            blocks.last().expect("a block").command.contains("cmd-three"),
            "command: {blocks:?}"
        );
        assert!(
            blocks[0].command.is_empty(),
            "the clear block's echo was wiped by its own ESC[3J: {:?}",
            blocks[0]
        );
        assert_eq!(
            blocks[0].output.0, blocks[0].output.1,
            "nothing survived between the clear block's C and D: {:?}",
            blocks[0]
        );
        let polled = h.session.range_text(c1);
        assert!(polled.contains("three-fill"), "poll stalled after clear");
    }

    /// D2 (`ESC c` / RIS): same assertions as the `ESC[3J` case.
    #[test]
    fn esc_c_ris_between_commands_keeps_tail_and_order() {
        let h = live_session("l2d2b", 80, 24);
        h.feed(&command_bytes(&h.nonce, "cmd-one", 0, 11_000, "one"));
        let c1 = h.session.tail_cursor();
        // RIS as the `clear` command's own output.
        let mut clear = Vec::new();
        clear.extend(marker_bytes(&h.nonce, "A"));
        clear.extend(b"prompt$ ");
        clear.extend(marker_bytes(&h.nonce, "B"));
        clear.extend(b"clear\r\n");
        clear.extend(marker_bytes(&h.nonce, "C"));
        clear.extend(b"\x1bc");
        clear.extend(marker_bytes(&h.nonce, "D;0"));
        h.feed(&clear);
        let c2 = h.session.tail_cursor();
        assert!(c2.line >= c1.line, "tail went backwards on ESC c: {c1:?} -> {c2:?}");
        h.feed(&command_bytes(&h.nonce, "cmd-three", 0, 3, "three"));
        let c3 = h.session.tail_cursor();
        assert!(c3.line >= c2.line, "tail went backwards after RIS: {c2:?} -> {c3:?}");
        let blocks = h.session.blocks();
        assert!(blocks.len() >= 2, "wiped all blocks: {blocks:?}");
        assert!(
            blocks.windows(2).all(|w| w[0].end < w[1].end),
            "ends not strictly increasing: {:?}",
            blocks.iter().map(|b| b.end).collect::<Vec<_>>()
        );
        assert!(
            blocks.iter().all(|b| b.exit == Some(0)),
            "a block lost its exit: {blocks:?}"
        );
        assert!(
            blocks.last().expect("a block").command.contains("cmd-three"),
            "command: {blocks:?}"
        );
        let polled = h.session.range_text(c1);
        assert!(polled.contains("three-fill"), "poll stalled after RIS");
    }

    /// D3: `ESC[24S` bursts push 24 lines from ~5 bytes, breaking any byte
    /// bound. Row-sized slices keep the accounting exact; where the safety
    /// net fires the desync is reported rather than silently wrong.
    #[test]
    fn scroll_up_bursts_stay_counted_or_report_desync() {
        let a = live_session("l2d3a", 80, 24);
        let b = live_session("l2d3b", 80, 24);
        for h in [&a, &b] {
            h.feed(&output_bytes(10_024, "pre"));
        }
        let ca = a.session.tail_cursor();
        let cb = b.session.tail_cursor();
        assert_eq!(ca, cb);
        let n = 700usize;
        a.feed(&"\n".repeat(24 * n).into_bytes());
        b.feed(&"\x1b[24S".repeat(n).into_bytes());
        let ta = a.session.tail_cursor();
        let tb = b.session.tail_cursor();
        assert!(tb.line > cb.line, "SU tail stalled: {cb:?} -> {tb:?}");
        if b.session.history_desyncs() == 0 {
            assert_eq!(
                ta.line - ca.line,
                tb.line - cb.line,
                "same line count, different eviction accounting (hist a={:?} b={:?})",
                a.session.with_term(|t| (t.grid().history_size(), t.grid().display_offset())),
                b.session.with_term(|t| (t.grid().history_size(), t.grid().display_offset())),
            );
        }
    }

    /// Alt screen, finished block: a block that closed before the alternate
    /// screen was entered still hides under the overlay.
    #[test]
    fn the_overlay_hides_a_finished_block_on_the_alternate_screen() {
        let h = live_session("l2alt", 80, 24);
        h.feed(&command_bytes(&h.nonce, "done-cmd", 0, 2, "d"));
        assert_eq!(h.session.blocks().len(), 1);
        assert!(!h.session.blocks()[0].running());
        h.feed(b"\x1b[?1049h");
        assert!(h.session.alt_screen());
        assert_eq!(h.session.blocks().len(), 1);
        assert!(
            h.session.overlay_chrome().is_empty(),
            "chrome drawn over a finished block on fullscreen"
        );
    }

    /// D4: plain cursor-up redraws (progress bars, `docker pull`, `ink`)
    /// move no line out of the retained window, so they must not inflate the
    /// evicted count, prune a block that is still on screen, or drift a
    /// finished block's text.
    #[test]
    fn cursor_up_redraws_evict_nothing() {
        let h = live_session("l2d4", 80, 24);
        h.feed(&command_bytes(&h.nonce, "first", 0, 3, "one"));
        let first_text = h.session.block_text(0).expect("first block text");
        let mut running = Vec::new();
        running.extend(marker_bytes(&h.nonce, "A"));
        running.extend(b"prompt$ ");
        running.extend(marker_bytes(&h.nonce, "B"));
        running.extend(b"progress\r\n");
        running.extend(marker_bytes(&h.nonce, "C"));
        running.extend(fill_lines(3, "step"));
        h.feed(&running);
        let t0 = h.session.tail_cursor();
        let (h0, _) =
            h.session.with_term(|t| (t.grid().history_size(), t.grid().display_offset()));
        // Redraw three lines the way ink/docker do, ten times, one pump each.
        for _ in 0..10 {
            h.feed(b"\x1b[3A");
            h.feed(&fill_lines(3, "step"));
        }
        let t1 = h.session.tail_cursor();
        let (h1, _) =
            h.session.with_term(|t| (t.grid().history_size(), t.grid().display_offset()));
        let after_text = h.session.block_text(0).expect("first block text after redraws");
        let blocks = h.session.blocks();
        assert_eq!(h1, h0, "history unchanged (precondition)");
        assert_eq!(t1, t0, "tail moved although no line was pushed or evicted");
        assert_eq!(after_text, first_text, "finished block's text drifted after cursor-up");
        assert_eq!(blocks.len(), 2, "a block was pruned while still on screen: {blocks:?}");
    }

    /// D5: `clear` + alt-screen enter in one pump (`clear; vim`). The switch
    /// is a slice boundary, so the clear is accounted on its own slice and
    /// the tail does not regress.
    #[test]
    fn clear_then_alt_enter_in_one_pump_is_counted() {
        let h = live_session("l2d5a", 80, 24);
        h.feed(&fill_lines(11_000, "base"));
        let c1 = h.session.tail_cursor();
        h.feed(b"\x1b[H\x1b[2J\x1b[3J\x1b[?1049h");
        assert!(h.session.alt_screen());
        h.feed(b"\x1b[?1049l");
        let c2 = h.session.tail_cursor();
        h.feed(&fill_lines(100, "after"));
        let c3 = h.session.tail_cursor();
        let polled = h.session.range_text(c1);
        assert!(c2.line >= c1.line, "tail regressed: {c1:?} -> {c2:?}");
        assert!(c3.line >= c2.line, "tail regressed after exit: {c2:?} -> {c3:?}");
        assert!(polled.contains("after-00099"), "poll stalled: {polled:?}");
    }

    /// D5: alt-screen exit + `ESC[3J` in one pump (`tput rmcup; clear`).
    /// The exit resumes from the snapshot and the clear is accounted on its
    /// own slice, so the tail does not regress.
    #[test]
    fn alt_exit_then_clear_in_one_pump_is_counted() {
        let h = live_session("l2d5b", 80, 24);
        h.feed(&fill_lines(11_000, "base"));
        let c1 = h.session.tail_cursor();
        h.feed(b"\x1b[?1049h");
        h.feed(b"\x1b[?1049l\x1b[H\x1b[2J\x1b[3J");
        let c2 = h.session.tail_cursor();
        h.feed(&fill_lines(100, "after"));
        let polled = h.session.range_text(c1);
        assert!(c2.line >= c1.line, "tail regressed: {c1:?} -> {c2:?}");
        assert!(polled.contains("after-00099"), "poll stalled: {polled:?}");
    }

    /// D6: two blocks sharing a start anchor (a stray first-precmd `D`, then
    /// a prompt on the same line) keep their own ids and authors once the
    /// first is pruned. Identity is minted at the event, not matched on the
    /// anchor.
    #[test]
    fn shared_start_anchor_does_not_swap_ids() {
        let h = live_session("l2d6a", 80, 24);
        // A stray D with no open block, then a normal prompt on the same line.
        h.feed(&marker_bytes(&h.nonce, "D;0"));
        h.feed(&command_bytes(&h.nonce, "real-cmd", 0, 60, "out"));
        let before = h.session.blocks();
        assert_eq!(before.len(), 2, "blocks: {before:?}");
        assert_eq!(
            before[0].prompt.0, before[1].prompt.0,
            "precondition: shared start anchor"
        );
        assert_ne!(before[0].id, before[1].id, "ids collide on the shared anchor");
        let id_real = before[1].id;
        h.session.set_block_author(0, BlockAuthor::Agent);
        h.session.set_block_author(1, BlockAuthor::Human);
        // Scroll far enough that the stray block is pruned but the real one is not.
        h.feed(&command_bytes(&h.nonce, "third", 0, 10_000, "fill"));
        let after = h.session.blocks();
        let real =
            after.iter().find(|b| b.command.contains("real-cmd")).expect("real-cmd survives");
        assert_eq!(real.id, id_real, "real-cmd changed id after the stray block was pruned");
        assert_eq!(real.author, BlockAuthor::Human, "real-cmd inherited the stray block's author");
    }

    /// D6: a `C`-only block (no `A`) with a prompt landing on its line keeps
    /// its own id and author once the older block is pruned.
    #[test]
    fn c_started_block_then_prompt_on_same_line() {
        let h = live_session("l2d6b", 80, 24);
        h.feed(&command_bytes(&h.nonce, "zero", 0, 2, "z"));
        let mut b = Vec::new();
        b.extend(marker_bytes(&h.nonce, "C"));
        b.extend(marker_bytes(&h.nonce, "D;7"));
        h.feed(&b);
        h.feed(&command_bytes(&h.nonce, "real-cmd", 0, 60, "out"));
        let before = h.session.blocks();
        let real_before =
            before.iter().find(|b| b.command.contains("real-cmd")).expect("real-cmd opens").clone();
        let idx = before.iter().position(|b| b.command.contains("real-cmd")).expect("index");
        h.session.set_block_author(idx, BlockAuthor::Agent);
        h.feed(&command_bytes(&h.nonce, "third", 0, 10_000, "fill"));
        let after = h.session.blocks();
        let real = after.iter().find(|b| b.command.contains("real-cmd")).expect("real-cmd survives");
        assert_eq!(real.id, real_before.id, "id changed");
        assert_eq!(real.author, BlockAuthor::Agent, "author lost");
        assert_eq!(real.exit, Some(0));
    }

    // ------------------------------------------------------------------
    // L3-fix6: the grid path (`MarkScanner` via `TerminalSession`) under
    // realistic 1024-byte pty reads, escape-capped advances, and RIS.
    // ------------------------------------------------------------------

    /// Standard-alphabet base64 (what the `base64` CLI emits), for building
    /// `C` payloads in tests.
    fn b64_encode(bytes: &[u8]) -> String {
        const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let mut n: u32 = 0;
            for &b in chunk {
                n = (n << 8) | u32::from(b);
            }
            n <<= 8 * (3 - chunk.len());
            out.push(ALPHA[(n >> 18) as usize & 63] as char);
            out.push(ALPHA[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 { ALPHA[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if chunk.len() > 2 { ALPHA[n as usize & 63] as char } else { '=' });
        }
        out
    }

    /// A `C` marker carrying `cmd` as a base64 payload — what the real
    /// snippets emit (no `B`, which our snippets no longer send).
    fn c_marker_with_cmd(nonce: &str, cmd: &str) -> Vec<u8> {
        format!("\x1b]133;C;k={nonce};cmd={};enc=b64\x07", b64_encode(cmd.as_bytes()))
            .into_bytes()
    }

    /// Feeds `bytes` in `chunk`-sized pumps: one pump per chunk, the way a
    /// macOS pty delivers output in 1024-byte reads.
    fn feed_chunked(h: &LiveSession, bytes: &[u8], chunk: usize) {
        for c in bytes.chunks(chunk) {
            h.feed(c);
        }
    }

    /// A finished command whose `C` carries the exact `cmd` text as a
    /// payload, fed the way a pty delivers it: 1024 bytes per pump.
    fn feed_command_chunked(h: &LiveSession, nonce: &str, cmd: &str, exit: i32, output: &[u8]) {
        let mut stream = Vec::new();
        stream.extend(marker_bytes(nonce, "A"));
        stream.extend(b"prompt$ ");
        stream.extend(cmd.as_bytes());
        stream.extend(b"\r\n");
        stream.extend(c_marker_with_cmd(nonce, cmd));
        stream.extend(output);
        stream.extend(marker_bytes(nonce, &format!("D;{exit}")));
        feed_chunked(h, &stream, 1024);
    }

    /// D1 (grid path): a 1,500-character command fed in 1024-byte pty reads
    /// keeps its `C` marker. Sized from the real bound, not a round number:
    /// the marker (~2 KB) exceeds one 1024-byte read plus the old 512-byte
    /// hold, so some pump strands more than 512 pending bytes and the old
    /// code flushed the marker as plain text — this test fails with the hold
    /// reverted to 512 — while fitting the marker cap derived from
    /// `MAX_CMD_LEN` with room to spare.
    #[test]
    fn long_c_marker_survives_1024_byte_pty_reads() {
        let h = live_session("l3d1a", 80, 24);
        let nonce = h.nonce.clone();
        let cmd = format!("curl-{}", "x".repeat(1495));
        assert_eq!(cmd.len(), 1500);
        assert!(
            c_marker_with_cmd(&nonce, &cmd).len() > 1024 + 512,
            "precondition: some pump strands more than the old 512-byte hold"
        );
        feed_command_chunked(&h, &nonce, &cmd, 0, b"ok\r\n");
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert_eq!(blocks[0].command, cmd);
        assert_eq!(blocks[0].exit, Some(0));
    }

    /// D1 (grid path): a 1,505-character command across several 1024-byte
    /// reads — the marker head, middle and tail all land in different pumps.
    #[test]
    fn longer_c_marker_survives_1024_byte_pty_reads() {
        let h = live_session("l3d1b", 80, 24);
        let nonce = h.nonce.clone();
        let cmd = format!("curl-{}", "y".repeat(1500));
        assert_eq!(cmd.len(), 1505);
        feed_command_chunked(&h, &nonce, &cmd, 0, b"ok\r\n");
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert_eq!(blocks[0].command, cmd);
        assert_eq!(blocks[0].exit, Some(0));
    }

    /// D1 (grid path): a command exactly at the 4 KB payload cap arrives
    /// whole through 1024-byte reads.
    #[test]
    fn c_marker_at_the_4kb_cap_survives_1024_byte_pty_reads() {
        let h = live_session("l3d1c", 80, 24);
        let nonce = h.nonce.clone();
        let cmd = "z".repeat(crate::parser::MAX_CMD_LEN);
        feed_command_chunked(&h, &nonce, &cmd, 0, b"ok\r\n");
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert_eq!(blocks[0].command, cmd);
    }

    /// D1 (grid path): a 5 KB command truncates to the first 4096 bytes on
    /// this path too — the equivalent of the decoder-level cap test, driven
    /// through `TerminalSession` in 1024-byte reads.
    #[test]
    fn oversized_command_truncates_to_4kb_on_the_session_path() {
        let h = live_session("l3d1d", 80, 24);
        let nonce = h.nonce.clone();
        let cmd = "w".repeat(5000);
        feed_command_chunked(&h, &nonce, &cmd, 0, b"ok\r\n");
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert_eq!(blocks[0].command.len(), crate::parser::MAX_CMD_LEN);
        assert_eq!(blocks[0].command, cmd[..crate::parser::MAX_CMD_LEN]);
    }

    /// D1 (grid path): the worst split boundary — one byte of the `C` marker
    /// in the first chunk, the rest arriving later. Sized from the real
    /// bound: the ~2 KB marker spans three 1024-byte pumps after the split,
    /// so no chunk boundary can complete it exactly and the old 512-byte
    /// hold flushes it mid-marker — this test fails with the hold reverted
    /// to 512.
    #[test]
    fn c_marker_split_with_one_byte_in_the_first_chunk() {
        let h = live_session("l3d1e", 80, 24);
        let nonce = h.nonce.clone();
        let cmd = format!("worst-{}", "v".repeat(1494));
        assert_eq!(cmd.len(), 1500);
        let mut stream = Vec::new();
        stream.extend(marker_bytes(&nonce, "A"));
        stream.extend(b"prompt$ ");
        stream.extend(cmd.as_bytes());
        stream.extend(b"\r\n");
        stream.extend(c_marker_with_cmd(&nonce, &cmd));
        stream.extend(b"ok\r\n");
        stream.extend(marker_bytes(&nonce, "D;0"));
        // Split so the first chunk holds exactly one byte of the `C` marker.
        let c_marker = c_marker_with_cmd(&nonce, &cmd);
        let c_pos = stream
            .windows(c_marker.len())
            .position(|w| w == c_marker.as_slice())
            .expect("the C marker is in the stream");
        h.feed(&stream[..c_pos + 1]);
        feed_chunked(&h, &stream[c_pos + 1..], 1024);
        let blocks = h.session.blocks();
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert_eq!(blocks[0].command, cmd);
        assert_eq!(blocks[0].exit, Some(0));
    }

    /// D7: an `ESC`-prefixed run of 20,000 lines in one pump advances the
    /// tail by exactly 20,000 with no desync — the row-sized cap applies past
    /// the escape's final byte, exactly as in the plain-text case.
    #[test]
    fn esc_prefixed_burst_advances_the_tail_exactly() {
        for (tag, prefix) in [("l3d7a", b"\x1b[0m".as_slice()), ("l3d7b", b"".as_slice())] {
            let h = live_session(tag, 80, 24);
            let c0 = h.session.tail_cursor();
            let mut burst = Vec::new();
            burst.extend_from_slice(prefix);
            burst.extend(fill_lines(20_000, "burst"));
            h.feed(&burst);
            let c1 = h.session.tail_cursor();
            assert_eq!(c1.line - c0.line, 20_000, "tail delta with prefix {prefix:?}");
            assert_eq!(h.session.history_desyncs(), 0, "burst desynced with prefix {prefix:?}");
        }
    }

    /// D7 residual: an OSC/DCS/APC string cut off by an `ESC` — a buggy `PS1`
    /// title, a program dying mid-string — ends there, exactly like vte, so
    /// the 20,000 lines after it advance the tail by exactly 20,000 with no
    /// desync. Without the fix the whole burst is a single advance, the
    /// emulator discards past its own cap, and the tail lands short with one
    /// desync.
    #[test]
    fn esc_terminated_strings_do_not_swallow_later_lines() {
        for (tag, prefix) in [
            ("l3d7c", b"\x1b]0;title\x1b[0m".as_slice()),
            ("l3d7d", b"\x1bPq#0\x1b[0m".as_slice()),
            ("l3d7e", b"\x1b_Gf=100\x1b[0m".as_slice()),
        ] {
            let h = live_session(tag, 80, 24);
            let c0 = h.session.tail_cursor();
            let mut burst = Vec::new();
            burst.extend_from_slice(prefix);
            burst.extend(fill_lines(20_000, "str"));
            h.feed(&burst);
            let c1 = h.session.tail_cursor();
            assert_eq!(c1.line - c0.line, 20_000, "tail delta with prefix {prefix:?}");
            assert_eq!(h.session.history_desyncs(), 0, "burst desynced with prefix {prefix:?}");
        }
    }

    /// D14: a title OSC cut off by CAN (0x18) or SUB (0x1a), or a DCS
    /// closed by 8-bit ST (0x9c), ends there,
    /// exactly like vte, so the 20,000 lines after it advance the tail by
    /// exactly 20,000 with no desync. Without the fix the whole burst is a
    /// single advance, the emulator discards past its own cap, and the tail
    /// lands 5,881 short with one desync — the same loss shape as D7, from
    /// a narrower trigger.
    #[test]
    fn can_and_sub_terminated_strings_do_not_swallow_later_lines() {
        for (tag, prefix) in [
            ("l3d14a", b"\x1b]0;title\x18".as_slice()),
            ("l3d14b", b"\x1b]0;title\x1a".as_slice()),
            // 8-bit ST: vte ends the string AND consumes the byte
            // (`vte/src/lib.rs:331`), so the count has to match it.
            // 8-bit ST ends a DCS in vte and is ordinary payload in an
            // OSC, so only the DCS form is a terminator here.
            ("l3d14c", b"\x1bPq#0\x9c".as_slice()),
        ] {
            let h = live_session(tag, 80, 24);
            let c0 = h.session.tail_cursor();
            let mut burst = Vec::new();
            burst.extend_from_slice(prefix);
            burst.extend(fill_lines(20_000, "can"));
            h.feed(&burst);
            let c1 = h.session.tail_cursor();
            assert_eq!(c1.line - c0.line, 20_000, "tail delta with prefix {prefix:?}");
            assert_eq!(h.session.history_desyncs(), 0, "burst desynced with prefix {prefix:?}");
        }
    }

    /// D8: `clear` plus 100 lines in one pump — the `ESC[3J` is its own
    /// advance, so the wipe is counted before the text lands: the tail moves
    /// exactly 100 and the poll resumes at the first line after the cursor
    /// (`after-00001` sits past it; `after-00000` lands exactly on the
    /// already-delivered cursor line, because the clear contributes net zero
    /// to the tail). With the escape sharing its advance with the text, the
    /// wipe undercounts: the tail lands 77 short and the poll starts at
    /// `after-00078`.
    #[test]
    fn clear_plus_100_lines_in_one_pump_shows_the_first_line_after() {
        let h = live_session("l3d8", 80, 24);
        h.feed(&fill_lines(11_000, "base"));
        let c1 = h.session.tail_cursor();
        let mut burst = b"\x1b[H\x1b[2J\x1b[3J".to_vec();
        burst.extend(fill_lines(100, "after"));
        h.feed(&burst);
        let c2 = h.session.tail_cursor();
        assert_eq!(c2.line - c1.line, 100, "tail miscounted across the clear: {c1:?} -> {c2:?}");
        let polled = h.session.range_text(c1);
        assert_eq!(
            polled.lines().next().unwrap_or_default(),
            "after-00001",
            "poll did not resume at the first line after the cursor"
        );
        assert!(polled.contains("after-00099"), "poll stalled");
    }

    /// One block's id, ranges, end, exit, author and command — the shape the
    /// ported `ris_mid_running_block` assertions read.
    type BlockSummary = (u64, (i32, i32), (i32, i32), i32, Option<i32>, BlockAuthor, String);

    /// RIS mid-block with a running command keeps the block's id and closes
    /// it sanely. Ported verbatim from the L2 audit (`l2_audit2.rs`): the
    /// narrowed history-only top-up satisfies it alongside the accounting
    /// rule the previous task feared it contradicted.
    #[test]
    fn ris_mid_running_block() {
        let h = live_session("t5", 80, 24);
        let nonce = h.nonce.clone();
        h.feed(&command_bytes(&nonce, "zero", 0, 11_000, "z"));
        let mut head = Vec::new();
        head.extend(marker_bytes(&nonce, "A"));
        head.extend(b"prompt$ ");
        head.extend(marker_bytes(&nonce, "B"));
        head.extend(b"long\r\n");
        head.extend(marker_bytes(&nonce, "C"));
        head.extend(fill_lines(5, "pre"));
        h.feed(&head);
        let running: Vec<BlockSummary> =
            h.session.blocks().iter().map(|b| (b.id, b.prompt, b.output, b.end, b.exit, b.author, b.command.clone())).collect();
        let id = running.last().unwrap().0;
        let idx = running.len() - 1;
        h.session.set_block_author(idx, BlockAuthor::Agent);
        let t0 = h.session.tail_cursor().line;
        h.feed(b"\x1bc");
        h.feed(&fill_lines(5, "post"));
        let t1 = h.session.tail_cursor().line;
        let mut tail = Vec::new();
        tail.extend(marker_bytes(&nonce, "D;3"));
        h.feed(&tail);
        let done: Vec<BlockSummary> =
            h.session.blocks().iter().map(|b| (b.id, b.prompt, b.output, b.end, b.exit, b.author, b.command.clone())).collect();
        eprintln!("tail {t0} -> {t1}; running={running:?}\ndone={done:?}");
        assert!(t1 >= t0);
        let b = done.last().unwrap();
        assert_eq!(b.0, id, "id changed across RIS");
        assert_eq!(b.5, BlockAuthor::Agent, "author lost across RIS");
        assert_eq!(b.4, Some(3));
        assert!(b.2.0 <= b.3, "output range inverted: {b:?}");
    }

    // ------------------------------------------------------------------
    // V4 (D15–D21).
    // ------------------------------------------------------------------

    /// D17: the chrome accounts for its real rendered width — the agent mark
    /// counts as a cell, and the strip's text right edge sits exactly at the
    /// grid edge, so the whole strip stays visible. Without the fix the mark
    /// is free and the strip's tail runs off the grid.
    #[test]
    fn chrome_accounts_for_its_real_width() {
        // Human: the text right edge is flush with the grid edge.
        let h = live_session("v4human", 20, 6);
        h.feed(&running_bytes(&h.nonce, "run", b"hi"));
        let chrome = h.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "{chrome:?}");
        let need = chrome_cells(
            chrome[0].glyph,
            &chrome[0].label,
            &chrome[0].command,
            chrome[0].author,
        );
        assert_eq!(chrome[0].col + need, 20, "strip not flush: {chrome:?}");
        // Agent, same content: one cell earlier for the mark.
        let a = live_session("v4agent", 20, 6);
        a.feed(&running_bytes(&a.nonce, "run", b"hi"));
        a.session.set_block_author(0, BlockAuthor::Agent);
        let agent = a.session.overlay_chrome();
        assert_eq!(agent.len(), 1, "{agent:?}");
        assert_eq!(agent[0].author, BlockAuthor::Agent);
        assert_eq!(agent[0].col + 1, chrome[0].col, "no room for the mark: {agent:?}");
    }

    /// One finished failed command with `tail` output bytes after its `C`
    /// marker: the block is closed, so the chrome anchors on the tail row.
    fn failed_bytes(nonce: &str, echo: &str, tail: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(marker_bytes(nonce, "A"));
        out.extend(b"p$ ");
        out.extend(marker_bytes(nonce, "B"));
        out.extend(echo.as_bytes());
        out.extend(b"\r\n");
        out.extend(marker_bytes(nonce, "C"));
        out.extend(tail);
        out.extend(marker_bytes(nonce, "D;1"));
        out
    }

    /// D18: a failed block degrades instead of hiding — the command text
    /// goes first, then the duration — keeping the status glyph and exit
    /// code visible. Without the fix a tight row hides the failure entirely.
    /// Related, same site: SGR-painted trailing spaces count as occupied.
    #[test]
    fn failed_chrome_degrades_before_hiding() {
        // Loose tail row: the whole strip, command included. (The scrape
        // keeps the prompt, so the command reads `p$ run`.)
        let loose = live_session("v4loose", 40, 6);
        loose.feed(&failed_bytes(&loose.nonce, "run", b""));
        let chrome = loose.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "{chrome:?}");
        assert!(chrome[0].failed);
        assert_eq!(chrome[0].command, "p$ run", "over-degraded: {chrome:?}");
        assert!(chrome[0].label.contains("exit 1"), "status lost: {chrome:?}");
        // Tight: no room for the command, room for glyph plus duration —
        // the command drops but the failure stays.
        let tight = live_session("v4tight", 40, 6);
        tight.feed(&failed_bytes(&tight.nonce, "run", &[b'y'; 22]));
        let chrome = tight.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "failure hidden: {chrome:?}");
        assert!(chrome[0].command.is_empty(), "command not dropped: {chrome:?}");
        assert!(chrome[0].label.contains("exit 1"), "status lost: {chrome:?}");
        assert_eq!(chrome[0].col, 25, "not right-aligned: {chrome:?}");
        // Tighter: only the glyph and exit code fit — the duration drops too.
        let tighter = live_session("v4tighter", 40, 6);
        tighter.feed(&failed_bytes(&tighter.nonce, "run", &[b'y'; 30]));
        let chrome = tighter.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "failure hidden: {chrome:?}");
        assert!(chrome[0].command.is_empty(), "command not dropped: {chrome:?}");
        assert_eq!(chrome[0].label, "exit 1", "duration not dropped: {chrome:?}");
        assert_eq!(chrome[0].glyph, "✗", "status glyph lost: {chrome:?}");
        // Painted blanks are occupied: thirty SGR-backed spaces degrade the
        // strip exactly like thirty `y`s do.
        let painted = live_session("v4painted", 40, 6);
        let mut tail = b"\x1b[41m".to_vec();
        tail.extend([b' '; 30]);
        tail.extend(b"\x1b[0m");
        painted.feed(&failed_bytes(&painted.nonce, "run", &tail));
        let chrome = painted.session.overlay_chrome();
        assert_eq!(chrome.len(), 1, "failure hidden: {chrome:?}");
        assert!(chrome[0].command.is_empty(), "painted blanks read as free: {chrome:?}");
        assert_eq!(chrome[0].label, "exit 1", "painted blanks read as free: {chrome:?}");
    }

    /// D19: the cursor blinks by default — only an explicit steady shape (a
    /// DECSCUSR steady variant, or mode 12 off) holds it still — in both
    /// themes. Without the fix the untouched default never blinks.
    #[test]
    fn cursor_blinks_unless_explicitly_steady() {
        for palette in [dark(), light()] {
            let h = live_session("v4blink", 20, 5);
            h.feed(b"hi");
            let cursor = h.session.snapshot(&palette).cursor.expect("a cursor");
            assert!(!cursor.blinking, "precondition: untouched default");
            assert!(cursor_blinks(&cursor), "the default cursor must blink");
            // DECSCUSR steady, then back to blinking.
            h.feed(b"\x1b[2 q");
            let cursor = h.session.snapshot(&palette).cursor.expect("a cursor");
            assert!(cursor.steady, "DECSCUSR steady not mirrored");
            assert!(!cursor_blinks(&cursor));
            h.feed(b"\x1b[1 q");
            let cursor = h.session.snapshot(&palette).cursor.expect("a cursor");
            assert!(!cursor.steady, "DECSCUSR blink did not clear steady");
            assert!(cursor_blinks(&cursor));
            // Mode 12 off steadies, on blinks, RIS resets to the default.
            h.feed(b"\x1b[?12l");
            let cursor = h.session.snapshot(&palette).cursor.expect("a cursor");
            assert!(cursor.steady, "mode 12 off not mirrored");
            assert!(!cursor_blinks(&cursor));
            h.feed(b"\x1b[?12h");
            assert!(cursor_blinks(&h.session.snapshot(&palette).cursor.expect("a cursor")));
            h.feed(b"\x1bc");
            let cursor = h.session.snapshot(&palette).cursor.expect("a cursor");
            assert!(!cursor.steady, "RIS did not reset to the default");
            assert!(cursor_blinks(&cursor));
            // A steady sequence split across two feeds still steadies.
            h.feed(b"\x1b[2");
            h.feed(b" q");
            let cursor = h.session.snapshot(&palette).cursor.expect("a cursor");
            assert!(cursor.steady, "split steady sequence missed");
            assert!(!cursor_blinks(&cursor));
        }
    }

    /// D19: reduced motion is a steady VISIBLE cursor — the resting phase
    /// must never read as off. Without the fix the `resting(1.0)` phase
    /// hides the cursor permanently under reduced motion.
    #[test]
    fn reduced_motion_keeps_the_cursor_visible() {
        assert!(blink_visible(true, true, 1.0), "resting phase hides the cursor");
        assert!(blink_visible(true, true, 0.0));
        assert!(blink_visible(true, false, 0.2));
        assert!(!blink_visible(true, false, 0.7));
        assert!(blink_visible(false, false, 0.9));
        assert!(blink_visible(false, true, 0.9));
    }

    /// D20: blink frames share one snapshot — rebuilt only on new output, a
    /// selection change or a palette change. Without the fix every frame
    /// rebuilds the whole grid at refresh rate.
    #[test]
    fn blink_frames_share_one_snapshot() {
        let h = live_session("v4cache", 20, 5);
        h.feed(b"hello");
        let dark_palette = dark();
        let a = h.session.snapshot_cached(&dark_palette);
        let b = h.session.snapshot_cached(&dark_palette);
        assert!(std::sync::Arc::ptr_eq(&a, &b), "a blink frame rebuilt the grid");
        h.feed(b"!");
        let c = h.session.snapshot_cached(&dark_palette);
        assert!(!std::sync::Arc::ptr_eq(&b, &c), "new output kept a stale grid");
        assert!(c.rows[0].text.starts_with("hello!"), "stale rows: {:?}", c.rows[0].text);
        h.session.select_start(0, 0);
        let d = h.session.snapshot_cached(&dark_palette);
        assert!(!std::sync::Arc::ptr_eq(&c, &d), "selection kept a stale highlight");
        let e = h.session.snapshot_cached(&light());
        assert!(!std::sync::Arc::ptr_eq(&d, &e), "theme switch kept stale colours");
        assert!(std::sync::Arc::ptr_eq(&e, &h.session.snapshot_cached(&light())));
    }

    /// D21: positions in the grid's side padding clamp to the nearest cell
    /// instead of missing, so a drag past the text keeps extending. Without
    /// the fix the padding maps to `None` and the selection stalls.
    #[test]
    fn padding_clamps_to_the_nearest_cell() {
        let session = pumped(vec![b"hello"], 20, 5);
        let (advance, line_height) = (8.0, 16.0);
        session.note_layout(
            gpui::Bounds {
                origin: gpui::point(gpui::px(100.0), gpui::px(50.0)),
                size: gpui::size(gpui::px(20.0 * advance), gpui::px(5.0 * line_height)),
            },
            CellMetrics { advance, line_height },
            20,
            5,
        );
        let at = |x: f32, y: f32| session.cell_at(gpui::point(gpui::px(x), gpui::px(y)));
        // Inside the text area: unchanged.
        assert_eq!(at(100.0 + 2.0 * advance, 50.0), Some((2, 0)));
        // Left padding clamps to column 0, right padding to the last column.
        assert_eq!(at(100.0 - 6.0, 50.0), Some((0, 0)));
        assert_eq!(
            at(100.0 + 20.0 * advance + 4.0, 50.0 + 2.0 * line_height),
            Some((19, 2))
        );
        // Above and below clamp to the edge rows.
        assert_eq!(at(100.0 + 3.0 * advance, 49.0), Some((3, 0)));
        assert_eq!(at(100.0 + 3.0 * advance, 500.0), Some((3, 4)));
    }

    /// D15: the element takes its focus handle from the caller and holds it
    /// across renders instead of minting one per frame. This pins the API
    /// shape the dock host programs against — `aui-terminal` has no
    /// `test-support`, so the click-keeps-focus behaviour itself is proven by
    /// re-running the audit's `v1_grid_focus` test against this code (see
    /// the report), which fails with the per-frame handle and passes with it.
    /// Without the fix neither method exists and this test fails to compile.
    #[test]
    fn grid_takes_its_focus_handle_from_the_host() {
        fn host_wires_a_handle(
            grid: TerminalGrid,
            handle: Option<gpui::FocusHandle>,
        ) -> TerminalGrid {
            match handle {
                Some(handle) => grid.focus_handle(handle),
                None => grid,
            }
        }

        fn session_falls_back_to_a_cached_handle(
            session: &TerminalSession,
            cx: &gpui::App,
        ) -> gpui::FocusHandle {
            let first = session.focus_handle(cx);
            assert_eq!(first, session.focus_handle(cx), "the fallback minted twice");
            first
        }

        let _ = host_wires_a_handle;
        let _ = session_falls_back_to_a_cached_handle;
    }
}


