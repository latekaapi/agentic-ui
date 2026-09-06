//! A real pseudo-terminal running the user's own login shell (feature `pty`).
//!
//! `portable-pty` opens the pty and spawns the shell; a reader thread pushes
//! everything the shell prints down an [`std::sync::mpsc`] channel, and
//! [`poll`](TerminalBackend::poll) drains it without blocking the UI thread.
//!
//! A login shell prints no OSC 133 markers unless it has been taught to, so
//! there would be no blocks. For zsh this crate teaches it: [`ZSH_INTEGRATION`]
//! is written into a throwaway `ZDOTDIR` whose `.zshrc` sources the user's own
//! first, then installs the hooks. The user's configuration is read, never
//! written, and the wrapper lives in a temporary directory that goes away with
//! the process.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::backend::{TermEvent, TerminalBackend};

/// The pty opens at this size until the pane measures itself.
const DEFAULT_COLS: u16 = 100;
/// Rows to match — card 50's left pane is about this tall at 12 px mono.
const DEFAULT_ROWS: u16 = 32;
/// The reader thread hands over at most this many bytes per read.
const READ_CHUNK: usize = 8 * 1024;

/// The zsh half of OSC 133 shell integration.
///
/// `precmd` runs just before each prompt (so `D` closes the command that just
/// finished and `A` opens the next block), and `preexec` runs after the line
/// is accepted (`C` starts the output). `B` is emitted from `PS1` itself, at
/// the point the prompt ends and the typed command begins.
pub const ZSH_INTEGRATION: &str = r#"
# OSC 133 shell integration for the aui block terminal.
autoload -Uz add-zsh-hook

_aui_osc133_precmd() {
  local _aui_status=$?
  if [[ -n ${_AUI_RUNNING-} ]]; then
    printf '\033]133;D;%s\007' "$_aui_status"
    unset _AUI_RUNNING
  fi
  printf '\033]133;A\007'
}

_aui_osc133_preexec() {
  _AUI_RUNNING=1
  printf '\033]133;C\007'
}

add-zsh-hook precmd _aui_osc133_precmd
add-zsh-hook preexec _aui_osc133_preexec
PS1="${PS1}"$'\033]133;B\007'
"#;

/// A live pseudo-terminal. Build one, then
/// [`spawn`](TerminalBackend::spawn) it.
pub struct Pty {
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    rx: Option<Receiver<Vec<u8>>>,
    size: PtySize,
    /// Kept alive for the life of the session: dropping it removes the
    /// wrapper `.zshrc`.
    zdotdir: Option<TempDir>,
    exited: bool,
}

impl Pty {
    /// A pty that has not been spawned yet.
    pub fn new() -> Self {
        Self {
            master: None,
            writer: None,
            child: None,
            rx: None,
            size: PtySize { rows: DEFAULT_ROWS, cols: DEFAULT_COLS, pixel_width: 0, pixel_height: 0 },
            zdotdir: None,
            exited: false,
        }
    }

    /// The throwaway `ZDOTDIR` this session's shell was started with, while
    /// the session lasts.
    pub fn zdotdir(&self) -> Option<&Path> {
        self.zdotdir.as_ref().map(|d| d.path())
    }

    /// Writes a `ZDOTDIR` wrapper that sources the user's own zsh files and
    /// then installs [`ZSH_INTEGRATION`].
    fn write_zdotdir() -> std::io::Result<TempDir> {
        let dir = TempDir::new("aui-zdotdir")?;
        let user = std::env::var("ZDOTDIR").unwrap_or_else(|_| std::env::var("HOME").unwrap_or_default());
        // `.zshenv` and `.zprofile` have to be forwarded too, or a login shell
        // loses the user's PATH.
        for name in ["zshenv", "zprofile", "zlogin"] {
            std::fs::write(dir.path().join(format!(".{name}")), format!("[[ -f {user}/.{name} ]] && source {user}/.{name}\n"))?;
        }
        std::fs::write(
            dir.path().join(".zshrc"),
            format!("[[ -f {user}/.zshrc ]] && source {user}/.zshrc\n{ZSH_INTEGRATION}"),
        )?;
        Ok(dir)
    }
}

impl Default for Pty {
    fn default() -> Self {
        Self::new()
    }
}

fn io_err(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

impl TerminalBackend for Pty {
    fn spawn(&mut self, shell: &str, cwd: &Path) -> std::io::Result<()> {
        let pair = native_pty_system().openpty(self.size).map_err(io_err)?;
        let mut cmd = CommandBuilder::new(shell);
        cmd.arg("-l");
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        // Only zsh gets shell integration; anything else runs unmarked and the
        // parser falls back to a single running block.
        if shell.ends_with("zsh") {
            match Self::write_zdotdir() {
                Ok(dir) => {
                    cmd.env("ZDOTDIR", dir.path());
                    self.zdotdir = Some(dir);
                }
                Err(e) => eprintln!("aui-terminal: no shell integration ({e})"),
            }
        }
        let child = pair.slave.spawn_command(cmd).map_err(io_err)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(io_err)?;
        let writer = pair.master.take_writer().map_err(io_err)?;
        let (tx, rx) = channel();
        std::thread::Builder::new().name("aui-terminal-reader".into()).spawn(move || {
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            return;
                        }
                    }
                }
            }
        })?;

        self.master = Some(pair.master);
        self.writer = Some(writer);
        self.child = Some(child);
        self.rx = Some(rx);
        self.exited = false;
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) {
        if let Some(w) = self.writer.as_mut() {
            let _ = w.write_all(bytes);
            let _ = w.flush();
        }
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        if let Some(m) = self.master.as_ref() {
            let _ = m.resize(self.size);
        }
    }

    fn poll(&mut self) -> Vec<TermEvent> {
        let mut events = Vec::new();
        if let Some(rx) = self.rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(bytes) => events.push(TermEvent::Output(bytes)),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        }
        if !self.exited {
            if let Some(child) = self.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    self.exited = true;
                    events.push(TermEvent::Exit(status.exit_code() as i32));
                }
            }
        }
        events
    }
}

/// A directory removed when it is dropped. `portable-pty` needs a real path
/// for `ZDOTDIR`, and this crate would rather not take a dependency for four
/// lines of `std::fs`.
struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
