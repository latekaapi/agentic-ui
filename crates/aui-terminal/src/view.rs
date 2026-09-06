//! The gpui half: a [`TerminalState`] entity that owns a backend and a
//! parser, and the two elements that draw it.
//!
//! `aui`'s [`block_terminal`] and [`tui_pane`] are stateless `RenderOnce`
//! components: data in, intents out. [`TerminalState`] is the data, kept in a
//! `gpui::Entity` so a timer can poll the backend, feed the parser and
//! `notify` — the window re-renders because `Window::use_keyed_state` observes
//! the entity it hands back.
//!
//! ```ignore
//! let state = window.use_keyed_state("term", cx, |_, _| {
//!     let fake = FakePty::card50();
//!     let parser = BlockParser::with_clock(fake.clock());
//!     TerminalState::with_parser(Box::new(fake), parser)
//! });
//! block_terminal_view("term", &state).prompt(prompt).on_intent(|intent, _, _| dbg!(intent));
//! ```

use std::rc::Rc;
use std::time::Duration;

use aui::workbench::{block_terminal, tui_pane, BlockState, TermBlock, TermPrompt, TerminalAction};
use gpui::{canvas, prelude::*, px, App, Bounds, ElementId, Entity, IntoElement, Pixels, SharedString, Task, Window};

use crate::backend::{TermEvent, TerminalBackend};
use crate::parser::BlockParser;
#[cfg(feature = "tui")]
use crate::tui_grid::{TuiGrid, TuiTerm};

/// The backend is drained on this cadence — about twice a display frame, so a
/// block appears in the same frame its bytes arrive.
const POLL_INTERVAL: Duration = Duration::from_millis(8);
/// Output past this many lines collapses behind card 50's fold row.
const FOLD_AFTER: usize = 8;
/// The newest blocks stay at full ink; everything before them dims to .72
/// (card 50 keeps the failure and the live command bright).
const LIVE_TAIL: usize = 2;
/// The interrupt character `run`'s counterpart writes.
const CTRL_C: &[u8] = b"\x03";
/// Both panes draw their terminal text at 12 px mono — `.scroll` on card 50
/// (`crates/aui/src/workbench/terminal.rs`) and `.tui` on the TUI pane
/// (`crates/aui/src/workbench/tui.rs`).
const TERM_TEXT: f32 = 12.0;
/// `.tui{font:12px/1.5 mono}` — the TUI pane's line height.
const TUI_LH: f32 = 1.5;
/// `.scroll{font:12px/1.55 mono}` — the block terminal's line height.
const BLOCK_LH: f32 = 1.55;
/// `.tui{padding:10px 12px}`: the horizontal inset a TUI row loses.
const TUI_INSET_X: f32 = 12.0 * 2.0;
/// …and the vertical one.
const TUI_INSET_Y: f32 = 10.0 * 2.0;
/// `.scroll{padding:10px 12px}` plus `.out{padding-left:26px;padding-right:10px}`:
/// what a block's output line loses horizontally.
const BLOCK_INSET_X: f32 = 12.0 * 2.0 + 26.0 + 10.0;
/// `.scroll{padding:10px 12px}` vertically.
const BLOCK_INSET_Y: f32 = 10.0 * 2.0;
// The grid pane redraws `aui::workbench::tui_pane` with real runs, so it
// carries that component's own numbers. They are only compiled with the grid.
/// `.tui{padding:10px 12px}` — the TUI pane's own padding, mirrored so the
/// grid pane lines up with `aui::workbench::tui_pane` exactly.
#[cfg(feature = "tui")]
const TUI_PAD_Y: f32 = 10.0;
/// …horizontally.
#[cfg(feature = "tui")]
const TUI_PAD_X: f32 = 12.0;
/// `.box{padding:6px 10px;margin:6px 0}` — the docked input box.
#[cfg(feature = "tui")]
const BOX_PAD_Y: f32 = 6.0;
/// …horizontally.
#[cfg(feature = "tui")]
const BOX_PAD_X: f32 = 10.0;
/// …and the gap above and below it.
#[cfg(feature = "tui")]
const BOX_MARGIN_Y: f32 = 6.0;
/// The input box's cursor: 7 x 14, blinking on a one-second loop.
#[cfg(feature = "tui")]
const CURSOR_W: f32 = 7.0;
/// …its height.
#[cfg(feature = "tui")]
const CURSOR_H: f32 = 14.0;
/// …and its period.
#[cfg(feature = "tui")]
const CURSOR_PERIOD: Duration = Duration::from_millis(1000);
/// `.hint{right:10px;top:8px;gap:4px}` — the keycaps in the corner.
#[cfg(feature = "tui")]
const HINT_RIGHT: f32 = 10.0;
/// …from the top.
#[cfg(feature = "tui")]
const HINT_TOP: f32 = 8.0;
/// …and between them.
#[cfg(feature = "tui")]
const HINT_GAP: f32 = 4.0;
/// A pty is never asked for fewer cells than this, however small the pane is
/// drawn — a zero-column terminal makes curses programs misbehave.
const MIN_COLS: u16 = 8;
/// …and never fewer rows.
const MIN_ROWS: u16 = 2;

/// What the terminal pane asks its host for.
///
/// [`TerminalAction`] is what the *component* emits (the hover actions and the
/// fold row); this is the wider set, including the things a host's key
/// bindings produce. [`TerminalState::apply`] performs the ones the terminal
/// can perform itself.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalIntent {
    /// Run this command line in the session.
    Run(SharedString),
    /// Interrupt whatever is running.
    Cancel,
    /// Collapse a block's output.
    Fold(SharedString),
    /// Expand a folded block.
    Unfold(SharedString),
    /// Put a block's output on the clipboard.
    Copy(SharedString),
    /// Hand a block to the agent.
    Ask(SharedString),
}

impl From<TerminalAction> for TerminalIntent {
    fn from(action: TerminalAction) -> Self {
        match action {
            TerminalAction::Copy(id) => TerminalIntent::Copy(id),
            TerminalAction::Ask(id) => TerminalIntent::Ask(id),
            TerminalAction::Unfold(id) => TerminalIntent::Unfold(id),
        }
    }
}

/// A terminal session: a backend, the parser reading it, and the prompt row.
///
/// Hold it in a `gpui::Entity`. The poll timer starts on the first render of
/// either view and lives on this struct, so dropping the entity stops it.
pub struct TerminalState {
    backend: Box<dyn TerminalBackend>,
    parser: BlockParser,
    prompt: TermPrompt,
    /// Blocks the user has expanded past [`FOLD_AFTER`].
    unfolded: Vec<SharedString>,
    exit: Option<i32>,
    poll: Option<Task<()>>,
    started: bool,
    /// The size the pane last measured, in character cells. `(0, 0)` until it
    /// has been measured once, so the first measurement always lands.
    cells: (u16, u16),
    /// Grid mode: when this is set, polled bytes go to the screen model
    /// instead of the block parser.
    #[cfg(feature = "tui")]
    grid: Option<TuiTerm>,
}

impl TerminalState {
    /// A session on `backend`, parsed on the real clock.
    pub fn new(backend: Box<dyn TerminalBackend>) -> Self {
        Self::with_parser(backend, BlockParser::new())
    }

    /// A session on `backend` with a parser built by the caller — the way to
    /// give a scripted backend its own clock.
    pub fn with_parser(backend: Box<dyn TerminalBackend>, parser: BlockParser) -> Self {
        Self {
            backend,
            parser,
            prompt: TermPrompt { text: SharedString::default(), context: Vec::new() },
            unfolded: Vec::new(),
            exit: None,
            poll: None,
            started: false,
            cells: (0, 0),
            #[cfg(feature = "tui")]
            grid: None,
        }
    }

    /// A session in **grid mode**: bytes go into an `alacritty_terminal`
    /// screen `cols` × `rows` cells instead of the block parser, which is what
    /// a full-screen program needs. Draw it with [`tui_grid_view`].
    ///
    /// [`blocks`](Self::blocks) stays empty in this mode — a screen that
    /// redraws in place has no blocks to report.
    #[cfg(feature = "tui")]
    pub fn with_grid(backend: Box<dyn TerminalBackend>, cols: u16, rows: u16) -> Self {
        let mut this = Self::new(backend);
        this.grid = Some(TuiTerm::with_size(cols as usize, rows as usize));
        this.cells = (cols, rows);
        this
    }

    /// The visible screen on `palette`, or `None` when this session is not in
    /// grid mode. Cheap enough to call once per frame: it walks the grid.
    #[cfg(feature = "tui")]
    pub fn grid(&self, palette: &aui_tokens::Palette) -> Option<TuiGrid> {
        self.grid.as_ref().map(|t| t.snapshot(palette))
    }

    /// The size the pane last reported, in character cells.
    pub fn cells(&self) -> (u16, u16) {
        self.cells
    }

    /// The prompt row's text and context tags.
    pub fn set_prompt(&mut self, prompt: TermPrompt) {
        self.prompt = prompt;
    }

    /// The prompt row's current state.
    pub fn prompt(&self) -> &TermPrompt {
        &self.prompt
    }

    /// The blocks parsed so far, straight from the parser and undecorated.
    pub fn blocks(&self) -> &[TermBlock] {
        self.parser.blocks()
    }

    /// Every output line of every block, in order — what the TUI pane draws,
    /// since a full-screen program produces one unmarked block.
    pub fn lines(&self) -> Vec<String> {
        self.parser.blocks().iter().flat_map(|b| b.output.iter().cloned()).collect()
    }

    /// The child's exit status, once it has one.
    pub fn exit(&self) -> Option<i32> {
        self.exit
    }

    /// Drains the backend into the parser. The timer calls it; a test can too.
    pub fn pump(&mut self) {
        for event in self.backend.poll() {
            match event {
                TermEvent::Output(bytes) => self.feed(&bytes),
                TermEvent::Exit(code) => self.exit = Some(code),
            }
        }
        self.parser.tick();
    }

    /// Routes a chunk of output: to the screen in grid mode, to the block
    /// parser otherwise.
    fn feed(&mut self, bytes: &[u8]) {
        #[cfg(feature = "tui")]
        if let Some(grid) = self.grid.as_mut() {
            grid.feed(bytes);
            return;
        }
        self.parser.feed(bytes);
    }

    /// Sends raw bytes to the session — a typed key, a pasted line, a signal
    /// character. This is how a host wires a keyboard to a grid pane.
    pub fn write(&mut self, bytes: &[u8]) {
        self.backend.write(bytes);
    }

    /// Tells the session its new size in character cells, resizing the screen
    /// model too when this session is in grid mode.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cells = (cols, rows);
        self.backend.resize(cols, rows);
        #[cfg(feature = "tui")]
        if let Some(grid) = self.grid.as_mut() {
            grid.resize(cols, rows);
        }
    }

    /// Performs the intents a terminal owns: running a line, interrupting it,
    /// and folding. [`TerminalIntent::Copy`] and [`TerminalIntent::Ask`] are
    /// the host's business and are ignored here.
    pub fn apply(&mut self, intent: &TerminalIntent) {
        match intent {
            TerminalIntent::Run(command) => {
                self.backend.write(command.as_bytes());
                self.backend.write(b"\n");
                self.prompt.text = SharedString::default();
            }
            TerminalIntent::Cancel => self.backend.write(CTRL_C),
            TerminalIntent::Fold(id) => self.unfolded.retain(|k| k != id),
            TerminalIntent::Unfold(id) => {
                if !self.unfolded.iter().any(|k| k == id) {
                    self.unfolded.push(id.clone());
                }
            }
            TerminalIntent::Copy(_) | TerminalIntent::Ask(_) => {}
        }
    }

    /// The blocks as card 50 draws them: older ones dimmed, long output folded
    /// unless the user expanded it.
    pub fn decorated_blocks(&self) -> Vec<TermBlock> {
        let blocks = self.parser.blocks();
        let last_bright = blocks.len().saturating_sub(LIVE_TAIL);
        blocks
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let mut b = b.clone();
                if i < last_bright && b.state != BlockState::Running {
                    b = b.old();
                }
                if b.output.len() > FOLD_AFTER && !self.unfolded.contains(&b.id) {
                    b.folded = b.output.len() - FOLD_AFTER;
                    b.output.truncate(FOLD_AFTER);
                }
                b
            })
            .collect()
    }

    /// Starts the poll timer once. Both views call it on every render; only
    /// the first call does anything.
    pub fn ensure_started(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.started {
            return;
        }
        self.started = true;
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let ok = this.update(cx, |this, cx| {
                    this.pump();
                    cx.notify();
                });
                if ok.is_err() {
                    return;
                }
            }
        });
        self.poll = Some(task);
    }
}

/// The pane's box in character cells, given the padding its text sits inside
/// and the line height it is drawn at. The cell width is the mono font's
/// advance for `0` at 12 px, measured through the window's own text system so
/// it matches what will actually be painted.
fn cells_for(bounds: Bounds<Pixels>, inset_x: f32, inset_y: f32, line_height: f32, window: &Window) -> (u16, u16) {
    let text = window.text_system();
    let font_id = text.resolve_font(&gpui::font(aui_tokens::scale::FONT_MONO));
    let advance = text.ch_advance(font_id, px(TERM_TEXT)).map(f32::from).unwrap_or(0.0);
    let row_h = TERM_TEXT * line_height;
    if advance <= 0.0 || row_h <= 0.0 {
        return (MIN_COLS, MIN_ROWS);
    }
    let w = f32::from(bounds.size.width) - inset_x;
    let h = f32::from(bounds.size.height) - inset_y;
    let cols = (w / advance).floor().clamp(MIN_COLS as f32, u16::MAX as f32) as u16;
    let rows = (h / row_h).floor().clamp(MIN_ROWS as f32, u16::MAX as f32) as u16;
    (cols, rows)
}

/// An element that paints nothing and measures the pane, telling `state` its
/// size in cells whenever that changes.
///
/// It has to be a `canvas`, because only prepaint knows how big the pane was
/// laid out. It never calls `notify`: a resized program redraws itself and the
/// poll timer picks that up, so a notify here would only spin the frame loop.
fn resize_probe(
    state: &Entity<TerminalState>,
    inset_x: f32,
    inset_y: f32,
    line_height: f32,
) -> impl IntoElement + use<> {
    let state = state.clone();
    canvas(
        move |bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App| {
            let size = cells_for(bounds, inset_x, inset_y, line_height, window);
            if state.read(cx).cells() != size {
                state.update(cx, |state, _| state.resize(size.0, size.1));
            }
        },
        |_, _: (), _, _| {},
    )
    .absolute()
    .size_full()
}

/// Wraps a pane so the [`resize_probe`] has a box to measure and a positioned
/// ancestor to sit in.
fn measured(pane: impl IntoElement, probe: impl IntoElement) -> impl IntoElement {
    gpui::div().relative().size_full().child(pane).child(probe)
}

type IntentHandler = Rc<dyn Fn(TerminalIntent, &mut Window, &mut App)>;

/// The block terminal over a [`TerminalState`]. Build with
/// [`block_terminal_view`].
#[derive(IntoElement)]
pub struct BlockTerminalView {
    id: ElementId,
    state: Entity<TerminalState>,
    marker: Option<SharedString>,
    prompt: Option<TermPrompt>,
    on_intent: Option<IntentHandler>,
}

/// The block terminal fed by `state`.
pub fn block_terminal_view(id: impl Into<ElementId>, state: &Entity<TerminalState>) -> BlockTerminalView {
    BlockTerminalView { id: id.into(), state: state.clone(), marker: None, prompt: None, on_intent: None }
}

impl BlockTerminalView {
    /// The restored-scrollback marker above the first block.
    pub fn marker(mut self, text: impl Into<SharedString>) -> Self {
        self.marker = Some(text.into());
        self
    }

    /// Overrides the prompt row for this frame; without it the state's own
    /// prompt is drawn.
    pub fn prompt(mut self, prompt: TermPrompt) -> Self {
        self.prompt = Some(prompt);
        self
    }

    /// Called for every intent the pane raises, after the terminal has applied
    /// the part it owns.
    pub fn on_intent(mut self, f: impl Fn(TerminalIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for BlockTerminalView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| state.ensure_started(window, cx));
        let state = self.state.clone();
        let blocks = state.read(cx).decorated_blocks();
        let prompt = match self.prompt {
            Some(prompt) => prompt,
            None => state.read(cx).prompt().clone(),
        };
        let on_intent = self.on_intent.clone();
        let mut pane = block_terminal(self.id, blocks).prompt(prompt).on_action(move |action, window, cx| {
            let intent = TerminalIntent::from(action);
            state.update(cx, |state, cx| {
                state.apply(&intent);
                cx.notify();
            });
            if let Some(f) = &on_intent {
                f(intent, window, cx);
            }
        });
        if let Some(marker) = self.marker {
            pane = pane.marker(marker);
        }
        measured(pane, resize_probe(&self.state, BLOCK_INSET_X, BLOCK_INSET_Y, BLOCK_LH))
    }
}

/// The TUI pane over a [`TerminalState`]. Build with [`tui_view`].
#[derive(IntoElement)]
pub struct TuiView {
    id: ElementId,
    state: Entity<TerminalState>,
    input: Option<SharedString>,
    footer: Option<SharedString>,
    hint_keys: Vec<SharedString>,
}

/// The agent's TUI screen fed by `state`.
pub fn tui_view(id: impl Into<ElementId>, state: &Entity<TerminalState>) -> TuiView {
    TuiView { id: id.into(), state: state.clone(), input: None, footer: None, hint_keys: Vec::new() }
}

impl TuiView {
    /// The docked input box and its text.
    pub fn input(mut self, text: impl Into<SharedString>) -> Self {
        self.input = Some(text.into());
        self
    }

    /// The dim hint line under the input.
    pub fn footer(mut self, text: impl Into<SharedString>) -> Self {
        self.footer = Some(text.into());
        self
    }

    /// A keycap pinned to the pane's top right.
    pub fn hint_key(mut self, key: impl Into<SharedString>) -> Self {
        self.hint_keys.push(key.into());
        self
    }
}

impl RenderOnce for TuiView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| state.ensure_started(window, cx));
        let lines = self.state.read(cx).lines();
        let mut pane = tui_pane(self.id, lines);
        if let Some(input) = self.input {
            pane = pane.input(input);
        }
        if let Some(footer) = self.footer {
            pane = pane.footer(footer);
        }
        for key in self.hint_keys {
            pane = pane.hint_key(key);
        }
        measured(pane, resize_probe(&self.state, TUI_INSET_X, TUI_INSET_Y, TUI_LH))
    }
}

/// The TUI grid over a [`TerminalState`] in grid mode. Build with
/// [`tui_grid_view`].
///
/// This draws the `alacritty_terminal` screen — real ANSI colour, the cursor
/// inverted in place — rather than [`tui_pane`]'s line-by-line ANSI path, and
/// it carries the same padding, 12 px/1.5 mono type, docked input box, footer
/// and hint keys so the two panes are indistinguishable at rest.
///
/// Keys are the host's business: write bytes to the session with
/// `state.update(cx, |state, _| state.write(b"..."))`.
#[cfg(feature = "tui")]
#[derive(IntoElement)]
pub struct TuiGridView {
    id: ElementId,
    state: Entity<TerminalState>,
    input: Option<SharedString>,
    footer: Option<SharedString>,
    hint_keys: Vec<SharedString>,
}

/// The `alacritty` screen behind `state` (which must have been built with
/// [`TerminalState::with_grid`]; a state without a grid draws an empty pane).
#[cfg(feature = "tui")]
pub fn tui_grid_view(id: impl Into<ElementId>, state: &Entity<TerminalState>) -> TuiGridView {
    TuiGridView { id: id.into(), state: state.clone(), input: None, footer: None, hint_keys: Vec::new() }
}

#[cfg(feature = "tui")]
impl TuiGridView {
    /// The docked input box and its text, with the blinking cursor after it.
    pub fn input(mut self, text: impl Into<SharedString>) -> Self {
        self.input = Some(text.into());
        self
    }

    /// The dim hint line under the input.
    pub fn footer(mut self, text: impl Into<SharedString>) -> Self {
        self.footer = Some(text.into());
        self
    }

    /// A keycap pinned to the pane's top right.
    pub fn hint_key(mut self, key: impl Into<SharedString>) -> Self {
        self.hint_keys.push(key.into());
        self
    }
}

/// One grid row as a styled line. The runs come from the snapshot, so the
/// colour is the program's own; the fallback is only for a row whose runs do
/// not add up (an empty line, which still needs a box of its own height).
#[cfg(feature = "tui")]
fn grid_line(row: &crate::tui_grid::TuiRow, p: &aui_tokens::Palette) -> impl IntoElement {
    use gpui::{font, StyledText, TextRun};
    let text = if row.text.is_empty() { " ".to_string() } else { row.text.clone() };
    let fits = !row.runs.is_empty() && row.runs.iter().map(|r| r.len).sum::<usize>() == text.len();
    let runs = if fits {
        row.runs.clone()
    } else {
        vec![TextRun {
            len: text.len(),
            font: font(aui_tokens::scale::FONT_MONO),
            color: p.term_fg,
            background_color: None,
            underline: None,
            strikethrough: None,
        }]
    };
    gpui::div().w_full().overflow_hidden().whitespace_nowrap().child(StyledText::new(text).with_runs(runs))
}

#[cfg(feature = "tui")]
impl RenderOnce for TuiGridView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use aui::data::kbd;
        use aui_motion::{looping, Loop};
        use aui_tokens::{scale, ActiveAui, AuiStyled};
        use gpui::{div, relative};
        use gpui_kit::base::{h_flex, v_flex};

        self.state.update(cx, |state, cx| state.ensure_started(window, cx));
        let p = cx.aui().colors;
        let grid = self.state.read(cx).grid(&p).unwrap_or_default();

        let mut body = v_flex()
            .w_full()
            .py(px(TUI_PAD_Y))
            .px(px(TUI_PAD_X))
            .mono(TERM_TEXT)
            .line_height(relative(TUI_LH))
            .text_color(p.term_fg);
        for row in &grid.rows {
            body = body.child(grid_line(row, &p));
        }
        if let Some(input) = self.input {
            let on = looping((self.id.clone(), "cursor"), Loop::linear(CURSOR_PERIOD).resting(1.0), window, cx) < 0.5;
            body = body.child(
                h_flex()
                    .my(px(BOX_MARGIN_Y))
                    .py(px(BOX_PAD_Y))
                    .px(px(BOX_PAD_X))
                    .gap(px(scale::SP_2))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.line_strong)
                    .child(div().text_color(p.term_dim).child("\u{203a}"))
                    .child(div().child(input))
                    .child(div().flex_none().w(px(CURSOR_W)).h(px(CURSOR_H)).bg(p.term_cursor).opacity(if on { 1.0 } else { 0.0 })),
            );
        }
        if let Some(footer) = self.footer {
            body = body.child(div().text_color(p.term_dim).child(footer));
        }
        let mut pane = div().id(self.id).relative().size_full().bg(p.term_bg).child(body);
        if !self.hint_keys.is_empty() {
            let mut hint = h_flex().absolute().right(px(HINT_RIGHT)).top(px(HINT_TOP)).gap(px(HINT_GAP));
            for key in self.hint_keys {
                hint = hint.child(kbd(key));
            }
            pane = pane.child(hint);
        }
        pane.child(resize_probe(&self.state, TUI_INSET_X, TUI_INSET_Y, TUI_LH))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakePty;

    #[test]
    fn a_state_pumps_its_backend_into_blocks() {
        let fake = FakePty::card50();
        let parser = BlockParser::with_clock(fake.clock());
        let mut state = TerminalState::with_parser(Box::new(fake), parser);
        // Poll until the whole recording has been replayed.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while state.blocks().len() < 4 && std::time::Instant::now() < deadline {
            state.pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(state.blocks().len(), 4);
        assert_eq!(state.exit(), None, "the recording ends with vitest still running");
        let decorated = state.decorated_blocks();
        assert!(decorated[0].old, "the oldest blocks dim");
        assert!(!decorated[3].old, "the live block does not");
        assert!(!state.lines().is_empty());
    }

    #[test]
    fn folding_and_unfolding_an_intent_round_trips() {
        let fake = FakePty::new(vec![crate::fake::ScriptChunk::new(0, "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n")]);
        let parser = BlockParser::with_clock(fake.clock());
        let mut state = TerminalState::with_parser(Box::new(fake), parser);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while state.blocks().is_empty() && std::time::Instant::now() < deadline {
            state.pump();
        }
        let id = state.blocks()[0].id.clone();
        assert_eq!(state.decorated_blocks()[0].folded, 2);
        assert_eq!(state.decorated_blocks()[0].output.len(), FOLD_AFTER);
        state.apply(&TerminalIntent::Unfold(id.clone()));
        assert_eq!(state.decorated_blocks()[0].folded, 0);
        assert_eq!(state.decorated_blocks()[0].output.len(), 10);
        state.apply(&TerminalIntent::Fold(id));
        assert_eq!(state.decorated_blocks()[0].folded, 2);
    }

    #[test]
    fn a_resize_is_remembered_so_the_pane_can_skip_the_next_one() {
        let mut state = TerminalState::new(Box::new(FakePty::card50()));
        assert_eq!(state.cells(), (0, 0), "unmeasured until the pane says otherwise");
        state.resize(80, 24);
        assert_eq!(state.cells(), (80, 24));
    }

    #[cfg(feature = "tui")]
    #[test]
    fn grid_mode_feeds_the_screen_and_leaves_the_block_parser_empty() {
        use aui_tokens::{Palette, ThemeKind};
        let fake = FakePty::new(vec![crate::fake::ScriptChunk::new(0, "first\r\nsecond")]);
        let mut state = TerminalState::with_grid(Box::new(fake), 20, 3);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let p = Palette::for_kind(ThemeKind::Dark);
        while std::time::Instant::now() < deadline
            && state.grid(&p).map(|g| g.rows[1].text.trim().is_empty()).unwrap_or(true)
        {
            state.pump();
        }
        let grid = state.grid(&p).expect("grid mode");
        assert_eq!(grid.rows.len(), 3);
        assert_eq!(grid.rows[0].text, "first");
        assert_eq!(grid.rows[1].text, "second ", "the trailing blank is the cursor cell");
        assert_eq!(grid.cursor, Some((1, 6)));
        assert!(state.blocks().is_empty(), "grid mode does not build blocks");
        assert_eq!(state.cells(), (20, 3));
    }

    #[cfg(not(feature = "tui"))]
    #[test]
    fn without_the_tui_feature_a_state_still_parses_blocks() {
        let mut state = TerminalState::new(Box::new(FakePty::card50()));
        state.pump();
        assert_eq!(state.cells(), (0, 0));
    }

    #[test]
    fn a_component_action_maps_onto_an_intent() {
        assert_eq!(TerminalIntent::from(TerminalAction::Copy("b1".into())), TerminalIntent::Copy("b1".into()));
        assert_eq!(TerminalIntent::from(TerminalAction::Ask("b1".into())), TerminalIntent::Ask("b1".into()));
        assert_eq!(TerminalIntent::from(TerminalAction::Unfold("b1".into())), TerminalIntent::Unfold("b1".into()));
    }
}
