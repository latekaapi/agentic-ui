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
//!
//! Dropping a [`Pty`] kills the shell, closes the pty and joins the reader
//! thread, so a window that goes away leaves no orphan behind.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread::JoinHandle;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::backend::{TermEvent, TerminalBackend};

/// The pty opens at this size until the pane measures itself.
const DEFAULT_COLS: u16 = 100;
/// Rows to match — card 50's left pane is about this tall at 12 px mono.
const DEFAULT_ROWS: u16 = 32;
/// The reader thread hands over at most this many bytes per read.
const READ_CHUNK: usize = 8 * 1024;
/// The shell to fall back to when `$SHELL` is unset — macOS's own default.
const DEFAULT_SHELL: &str = "/bin/zsh";
/// How long [`Pty::shutdown`] waits for the reader thread before detaching it.
/// A killed shell's pty reports EOF within a scheduler tick; this is slack.
const JOIN_GRACE: std::time::Duration = std::time::Duration::from_millis(200);
/// How often that wait re-checks.
const JOIN_POLL: std::time::Duration = std::time::Duration::from_millis(2);

/// The zsh half of OSC 133 shell integration.
///
/// `precmd` runs just before each prompt (so `D` closes the command that just
/// finished and `A` opens the next block), and `preexec` runs after the line
/// is accepted (`C` starts the output). `B` marks the point the prompt ends
/// and the typed command begins, so it has to live inside `PS1`.
///
/// Appending it to `PS1` once, at source time, is not enough: starship,
/// powerlevel10k and oh-my-zsh themes all rebuild `PS1` from their own
/// `precmd`, which throws the marker away. This snippet re-appends it from a
/// `precmd` hook of its own instead. Because the wrapper installs the hook
/// *after* sourcing the user's files, `add-zsh-hook` puts it last in
/// `precmd_functions`, so it runs after the theme has rewritten the prompt.
/// The re-append strips any marker already there first, so a static `PS1`
/// does not grow one copy per prompt, and the marker is wrapped in `%{…%}`
/// so zsh does not count it towards the prompt's width.
///
/// `PROMPT_SP` is switched off as well: it is the reverse-video `%` and the
/// row of padding zsh prints to show a command whose output had no trailing
/// newline, and it would land in the block's output as a line of noise.
pub const ZSH_INTEGRATION: &str = r#"
# OSC 133 shell integration for the aui block terminal.
autoload -Uz add-zsh-hook

# The prompt-end marker, invisible to zsh's width arithmetic.
_AUI_OSC133_B=$'%{\033]133;B\007%}'

_aui_osc133_precmd() {
  local _aui_status=$?
  if [[ -n ${_AUI_RUNNING-} ]]; then
    printf '\033]133;D;%s\007' "$_aui_status"
    unset _AUI_RUNNING
  fi
  printf '\033]133;A\007'
  # Re-attach the marker to whatever prompt the theme just built.
  PS1="${PS1//$_AUI_OSC133_B/}${_AUI_OSC133_B}"
}

_aui_osc133_preexec() {
  _AUI_RUNNING=1
  printf '\033]133;C\007'
}

add-zsh-hook precmd _aui_osc133_precmd
add-zsh-hook preexec _aui_osc133_preexec

# No partial-line marker: the block terminal draws the boundaries itself.
unsetopt PROMPT_SP
"#;

/// A live pseudo-terminal. Build one, then
/// [`spawn`](TerminalBackend::spawn) it.
pub struct Pty {
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    rx: Option<Receiver<Vec<u8>>>,
    reader: Option<JoinHandle<()>>,
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
            reader: None,
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

    /// Ends the session: kills the shell, reaps it, closes the pty and lets
    /// the reader thread finish. Idempotent, and called for you when the
    /// `Pty` is dropped or re-[`spawn`](TerminalBackend::spawn)ed.
    ///
    /// The reader is given a short grace period (`JOIN_GRACE`, 200 ms) to notice the closed pty. If a
    /// background job the shell started still holds the slave open past that,
    /// the thread is detached rather than blocking the UI thread; it ends on
    /// its own as soon as the read fails, and it can no longer reach this
    /// `Pty` because the channel receiver has gone.
    pub fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Closing our ends of the pty is what makes the blocked read return.
        self.writer = None;
        self.master = None;
        self.rx = None;
        if let Some(reader) = self.reader.take() {
            let deadline = std::time::Instant::now() + JOIN_GRACE;
            while !reader.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(JOIN_POLL);
            }
            if reader.is_finished() {
                let _ = reader.join();
            }
        }
        self.zdotdir = None;
        self.exited = true;
    }

    /// Writes a `ZDOTDIR` wrapper that sources the user's own zsh files and
    /// then installs [`ZSH_INTEGRATION`].
    fn write_zdotdir() -> std::io::Result<TempDir> {
        let dir = TempDir::new("aui-zdotdir")?;
        let user = std::env::var("ZDOTDIR").unwrap_or_else(|_| std::env::var("HOME").unwrap_or_default());
        // `.zshenv` and `.zprofile` have to be forwarded too, or a login shell
        // loses the user's PATH.
        for name in ["zshenv", "zprofile", "zlogin"] {
            let user = sh_quote(&format!("{user}/.{name}"));
            std::fs::write(dir.path().join(format!(".{name}")), format!("[[ -f {user} ]] && source {user}\n"))?;
        }
        let rc = sh_quote(&format!("{user}/.zshrc"));
        std::fs::write(dir.path().join(".zshrc"), format!("[[ -f {rc} ]] && source {rc}\n{ZSH_INTEGRATION}"))?;
        Ok(dir)
    }
}

/// The user's login shell, or `/bin/zsh` when `$SHELL` is not set — the
/// argument [`spawn`](TerminalBackend::spawn) wants.
pub fn login_shell() -> String {
    std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| DEFAULT_SHELL.to_string())
}

/// Wraps `s` in single quotes for a POSIX shell, so a home directory with a
/// space (or a quote) in it still sources the right file.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
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
        // "Calling it twice replaces the session" — so the old shell has to go
        // first, or the previous reader thread keeps feeding this pty.
        self.shutdown();
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
        let reader = std::thread::Builder::new().name("aui-terminal-reader".into()).spawn(move || {
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
        self.reader = Some(reader);
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

impl Drop for Pty {
    /// No zombie, no orphan: the window closing takes the shell with it.
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::BlockParser;
    use aui::workbench::BlockState;
    use std::time::{Duration, Instant};

    /// How long the shell gets to boot, run two commands and report them.
    /// A cold zsh with the user's own rc files is the slow part.
    const TEST_TIMEOUT: Duration = Duration::from_secs(20);

    /// Spawns the user's real login shell, runs `echo hi` and `false`, and
    /// waits for the OSC 133 markers to turn them into blocks.
    ///
    /// This is the only test that proves the shell integration works, so it
    /// runs the real thing rather than a fixture. It needs a zsh on the box
    /// (macOS always has one) and it is skipped, not failed, if `$SHELL` is
    /// something else — the wrapper only teaches zsh.
    #[test]
    fn a_real_zsh_reports_a_success_and_a_failure_as_blocks() {
        let shell = login_shell();
        if !shell.ends_with("zsh") || !Path::new(&shell).exists() {
            eprintln!("skipped: {shell} is not a zsh");
            return;
        }
        let mut pty = Pty::new();
        pty.spawn(&shell, &std::env::temp_dir()).expect("the shell spawns");
        let mut parser = BlockParser::new();

        // Type a line only once the shell has drawn a prompt — counting the
        // `B` markers in the raw stream is the only reliable signal, because
        // a command sent before zle is listening is lost to the tty.
        const PROMPT_END: &[u8] = b"\x1b]133;B\x07";
        let script: [&[u8]; 2] = [b"echo hi\n", b"false\n"];
        let deadline = Instant::now() + TEST_TIMEOUT;
        let mut raw: Vec<u8> = Vec::new();
        let mut sent = 0usize;
        let mut done = false;
        while Instant::now() < deadline && !done {
            for event in pty.poll() {
                if let TermEvent::Output(bytes) = event {
                    parser.feed(&bytes);
                    raw.extend_from_slice(&bytes);
                }
            }
            let prompts = raw.windows(PROMPT_END.len()).filter(|w| *w == PROMPT_END).count();
            if sent < script.len() && prompts > sent {
                pty.write(script[sent]);
                sent += 1;
            }
            done = sent == script.len()
                && parser.blocks().iter().any(|b| b.command == "false" && b.state != BlockState::Running);
            std::thread::sleep(Duration::from_millis(10));
        }

        // Anything the rc files print before the first `A` marker is its own
        // unattributed block, so look the two commands up rather than
        // indexing — a noisy .zshrc must not fail this test.
        let blocks = parser.blocks();
        assert!(done, "timed out with blocks {blocks:#?}");
        let hi = blocks.iter().find(|b| b.command == "echo hi").unwrap_or_else(|| panic!("{blocks:#?}"));
        assert_eq!(hi.state, BlockState::Done, "{blocks:#?}");
        assert_eq!(hi.output, vec!["hi".to_string()], "{blocks:#?}");
        let failed = blocks.iter().find(|b| b.command == "false").unwrap_or_else(|| panic!("{blocks:#?}"));
        assert_eq!(failed.state, BlockState::Failed, "{blocks:#?}");
        assert!(failed.output.is_empty(), "{blocks:#?}");
        assert!(failed.duration.ends_with("exit 1"), "{:?}", failed.duration);
    }

    #[test]
    fn a_dropped_pty_leaves_no_child_behind() {
        let shell = login_shell();
        if !Path::new(&shell).exists() {
            eprintln!("skipped: no {shell}");
            return;
        }
        let mut pty = Pty::new();
        pty.spawn(&shell, &std::env::temp_dir()).expect("the shell spawns");
        let pid = pty.child.as_ref().and_then(|c| c.process_id()).expect("a live child");
        // Let it get as far as its first prompt.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && pty.poll().is_empty() {
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(pty);
        // `kill -0` is the cheapest "does this process exist" there is, and a
        // reaped child is gone rather than a zombie.
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(!alive, "pid {pid} survived the drop");
    }

    #[test]
    fn a_home_with_a_space_is_quoted_into_the_wrapper() {
        assert_eq!(sh_quote("/Users/ada lovelace/.zshrc"), "'/Users/ada lovelace/.zshrc'");
        assert_eq!(sh_quote("/tmp/it's/.zshrc"), r"'/tmp/it'\''s/.zshrc'");
    }
}
