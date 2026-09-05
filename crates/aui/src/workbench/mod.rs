//! Workbench: the block terminal and TUI pane, browser with annotator, diff
//! review, git and PR forms, file tree and document panes, sources and
//! citations (cards 50–55, spec §5).

mod browser;
mod citations;
mod diff_review;
mod docs;
mod files;
mod git;
mod terminal;
mod tui;

pub use terminal::{block_terminal, BlockState, BlockTerminal, TermBlock, TermPrompt, TerminalAction};
pub use tui::{tui_pane, TuiPane};
// card 51 exports
// card 52 exports
// card 53 exports
// card 54 exports
// card 55 exports
