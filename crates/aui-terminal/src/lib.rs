//! `aui-terminal` — the backends behind the workbench's terminal panes.
//!
//! [`aui::workbench`] draws the block terminal and the TUI pane but does no
//! I/O. This crate supplies the bytes and turns them into the shapes those
//! components take:
//!
//! | module | what it does |
//! |---|---|
//! | [`backend`] | the [`TerminalBackend`] trait and its [`TermEvent`] stream |
//! | [`parser`] | [`BlockParser`] — OSC 133 shell integration to `TermBlock`s |
//! | [`fake`] | [`FakePty`], a scripted transcript that replays card 50 |
//! | `pty` | a real login shell over `portable-pty` (feature `pty`) |
//! | `tui_grid` | the alacritty grid model behind the TUI pane (feature `tui`) |
//! | [`view`] | [`TerminalState`] plus the three gpui elements |
//!
//! ```ignore
//! let state = window.use_keyed_state("term", cx, |window, cx| {
//!     let fake = FakePty::card50();
//!     let parser = BlockParser::with_clock(fake.clock());
//!     TerminalState::with_parser(Box::new(fake), parser)
//! });
//! block_terminal_view("term", &state).marker("restored scrollback · 09:02")
//! ```
//!
//! Nothing here holds a colour or a hard-coded font size: everything visible
//! comes from `aui`, `aui-tokens`, `aui-icons` and `aui-motion`.

#![warn(missing_docs)]

pub mod backend;
pub mod fake;
pub mod parser;
#[cfg(feature = "pty")]
pub mod pty;
#[cfg(feature = "tui")]
pub mod tui_grid;
pub mod view;

pub use backend::{TermEvent, TerminalBackend};
pub use fake::{FakePty, ScriptChunk};
pub use parser::{BlockParser, ManualClock};
#[cfg(feature = "pty")]
pub use pty::{bash_integration, generate_nonce, login_shell, zsh_integration, Pty, PtyConfig};
#[cfg(feature = "tui")]
pub use tui_grid::{TuiGrid, TuiRow, TuiTerm};
pub use view::{block_terminal_view, tui_view, BlockTerminalView, TerminalIntent, TerminalState, TuiView};
#[cfg(feature = "tui")]
pub use view::{tui_grid_view, TuiGridView};
