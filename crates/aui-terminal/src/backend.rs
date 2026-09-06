//! The byte-stream contract every terminal source implements.
//!
//! A backend is a pull source: the UI owns a timer, calls [`TerminalBackend::poll`]
//! on it every frame or so, feeds whatever came back into a
//! [`BlockParser`](crate::parser::BlockParser) and re-renders. Nothing here
//! blocks, and nothing here knows about gpui.

use std::path::Path;

/// One thing that happened on the terminal since the last poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermEvent {
    /// Raw bytes read from the terminal, escapes and all.
    Output(Vec<u8>),
    /// The child process ended with this status code.
    Exit(i32),
}

/// A source of terminal bytes: a real pty, a scripted transcript, a recording.
///
/// Implementations are single-threaded from the caller's point of view — the
/// UI thread owns the backend and drives it — so a backend that reads on a
/// worker thread must buffer internally and hand the bytes over in
/// [`poll`](TerminalBackend::poll).
pub trait TerminalBackend {
    /// Starts `shell` in `cwd`. Calling it twice replaces the session.
    fn spawn(&mut self, shell: &str, cwd: &Path) -> std::io::Result<()>;

    /// Sends bytes to the terminal (typed keys, a pasted command, a signal
    /// character). Dropped when no session is running.
    fn write(&mut self, bytes: &[u8]);

    /// Tells the terminal its new window size in character cells.
    fn resize(&mut self, cols: u16, rows: u16);

    /// Takes everything that arrived since the last call. Never blocks;
    /// returns an empty vector when nothing happened.
    fn poll(&mut self) -> Vec<TermEvent>;
}
