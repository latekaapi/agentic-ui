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
use gpui::{prelude::*, App, ElementId, Entity, IntoElement, SharedString, Task, Window};

use crate::backend::{TermEvent, TerminalBackend};
use crate::parser::BlockParser;

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
        }
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
                TermEvent::Output(bytes) => self.parser.feed(&bytes),
                TermEvent::Exit(code) => self.exit = Some(code),
            }
        }
        self.parser.tick();
    }

    /// Sends raw bytes to the session.
    pub fn write(&mut self, bytes: &[u8]) {
        self.backend.write(bytes);
    }

    /// Tells the session its new size in character cells.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.backend.resize(cols, rows);
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
                if b.output.len() > FOLD_AFTER && !self.unfolded.iter().any(|k| *k == b.id) {
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
        pane
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
        pane
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
    fn a_component_action_maps_onto_an_intent() {
        assert_eq!(TerminalIntent::from(TerminalAction::Copy("b1".into())), TerminalIntent::Copy("b1".into()));
        assert_eq!(TerminalIntent::from(TerminalAction::Ask("b1".into())), TerminalIntent::Ask("b1".into()));
        assert_eq!(TerminalIntent::from(TerminalAction::Unfold("b1".into())), TerminalIntent::Unfold("b1".into()));
    }
}
