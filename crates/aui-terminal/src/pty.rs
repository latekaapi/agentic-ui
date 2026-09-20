//! A real pseudo-terminal running the user's own login shell (feature `pty`).
//!
//! `portable-pty` opens the pty and spawns the shell; a reader thread pushes
//! everything the shell prints down an [`std::sync::mpsc`] channel, and
//! [`poll`](TerminalBackend::poll) drains it without blocking the UI thread.
//!
//! A login shell prints no OSC 133 markers unless it has been taught to, so
//! there would be no blocks. This crate teaches it: [`zsh_integration`] and
//! [`bash_integration`] build the snippet for a session nonce, written into a
//! throwaway directory — a `ZDOTDIR` chaining the user's own zsh files for
//! zsh, a `--rcfile` sourcing the user's own bash files for bash. The user's
//! configuration is read, never written, and the wrapper lives in a temporary
//! directory that goes away with the process.
//!
//! Every marker the snippets emit carries `k=<nonce>`, so a program that
//! prints a bare `OSC 133` cannot forge a block boundary. The nonce is minted
//! per session ([`generate_nonce`]) and travels in [`PtyConfig`]; there is no
//! way to spawn a session without one.
//!
//! Dropping a [`Pty`] kills the shell, closes the pty and joins the reader
//! thread, so a window that goes away leaves no orphan behind.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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
/// The file name of the generated bash `--rcfile` inside its throwaway directory.
const BASH_RC_NAME: &str = "bashrc";
/// Placeholder for the session nonce inside the snippet templates below.
const NONCE_PLACEHOLDER: &str = "__AUI_NONCE__";

/// The zsh half of OSC 133 shell integration, before the nonce is filled in.
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
/// The hook is deleted before it is re-added, so when several chained startup
/// files each source this snippet the registration still ends up last. The
/// re-append strips any marker already there first, so a static `PS1`
/// does not grow one copy per prompt, and the marker is wrapped in `%{…%}`
/// so zsh does not count it towards the prompt's width.
///
/// `PROMPT_SP` is switched off as well: it is the reverse-video `%` and the
/// row of padding zsh prints to show a command whose output had no trailing
/// newline, and it would land in the block's output as a line of noise.
const ZSH_TEMPLATE: &str = r#"
# OSC 133 shell integration for the aui block terminal.
autoload -Uz add-zsh-hook

# The prompt-end marker, invisible to zsh's width arithmetic.
_AUI_OSC133_B=$'%{\033]133;B;k=__AUI_NONCE__\007%}'

_aui_osc133_precmd() {
  local _aui_status=$?
  if [[ -n ${_AUI_RUNNING-} ]]; then
    printf '\033]133;D;%s;k=__AUI_NONCE__\007' "$_aui_status"
    unset _AUI_RUNNING
  fi
  printf '\033]133;A;k=__AUI_NONCE__\007'
  # Re-attach the marker to whatever prompt the theme just built.
  PS1="${PS1//$_AUI_OSC133_B/}${_AUI_OSC133_B}"
}

_aui_osc133_preexec() {
  _AUI_RUNNING=1
  printf '\033]133;C;k=__AUI_NONCE__\007'
}

# Deleted before re-adding, so that when several chained startup files each
# source this snippet the registration ends up last and runs after the theme.
add-zsh-hook -d precmd _aui_osc133_precmd 2>/dev/null
add-zsh-hook precmd _aui_osc133_precmd
add-zsh-hook -d preexec _aui_osc133_preexec 2>/dev/null
add-zsh-hook preexec _aui_osc133_preexec

# No partial-line marker: the block terminal draws the boundaries itself.
unsetopt PROMPT_SP
"#;

/// The bash half of OSC 133 shell integration, before the nonce is filled in.
///
/// This is the whole generated `--rcfile`: it sources the user's own files
/// first — the login files when this is a login shell, then `~/.bashrc` — so
/// their prompt setup wins, and installs the hooks afterwards so they observe
/// the final prompt. The hooks are the bash equivalents of the zsh ones: a
/// `PROMPT_COMMAND` entry plays `precmd` (`D` closes the finished command,
/// `A` opens the next block, `B` is re-attached to whatever `PS1` the user's
/// setup just built), and a `DEBUG` trap plays `preexec` (`C` starts the
/// output). The trap also fires for the prompt command itself, so it ignores
/// everything that runs while `PROMPT_COMMAND` is executing and everything
/// this snippet defines.
///
/// `PROMPT_COMMAND` brackets the user's own: the status capture runs first
/// (so `$?` is still the command's) and the marker re-attachment runs after
/// theirs. The `B` marker is wrapped in `\[…\]` so readline does not count it
/// towards the prompt's width, and the re-attachment matches it literally so
/// a static `PS1` does not grow one copy per prompt.
const BASH_TEMPLATE: &str = r#"
# OSC 133 shell integration for the aui block terminal (bash).
# The user's own files come first so their prompt setup wins; the hooks below
# are installed afterwards so they observe the final prompt.
if shopt -q login_shell 2>/dev/null; then
  if [ -f "$HOME/.bash_profile" ]; then . "$HOME/.bash_profile"
  elif [ -f "$HOME/.bash_login" ]; then . "$HOME/.bash_login"
  elif [ -f "$HOME/.profile" ]; then . "$HOME/.profile"
  fi
fi
[ -f "$HOME/.bashrc" ] && . "$HOME/.bashrc"

# The prompt-end marker; \[...\] keeps it out of readline's width arithmetic.
_AUI_OSC133_B=$'\[\033]133;B;k=__AUI_NONCE__\007\]'

# Captures the command's exit status first: this entry runs before the rest of
# PROMPT_COMMAND so `$?` is still the command's.
_aui_osc133_begin() {
  _AUI_STATUS=$?
  _AUI_IN_PROMPT=1
}

_aui_osc133_precmd() {
  local _aui_status=${_AUI_STATUS:-0}
  if [ -n "${_AUI_RUNNING-}" ]; then
    printf '\033]133;D;%s;k=__AUI_NONCE__\007' "$_aui_status"
    unset _AUI_RUNNING
  fi
  printf '\033]133;A;k=__AUI_NONCE__\007'
  # Re-attach the marker to whatever prompt setup just built; the quoted
  # pattern matches literally.
  PS1="${PS1//"$_AUI_OSC133_B"/}${_AUI_OSC133_B}"
  unset _AUI_STATUS _AUI_IN_PROMPT
}

_aui_osc133_preexec() {
  # The DEBUG trap also fires for the prompt command itself; only real
  # commands open a block.
  [ -n "${_AUI_IN_PROMPT-}" ] && return 0
  case "$BASH_COMMAND" in
    _aui_osc133_*|PROMPT_COMMAND*) return 0;;
  esac
  if [ -z "${_AUI_RUNNING-}" ]; then
    _AUI_RUNNING=1
    printf '\033]133;C;k=__AUI_NONCE__\007'
  fi
}

trap '_aui_osc133_preexec' DEBUG
# This brackets the user's own PROMPT_COMMAND: the status capture runs first
# and the marker re-attachment runs after theirs.
PROMPT_COMMAND="_aui_osc133_begin${PROMPT_COMMAND:+; $PROMPT_COMMAND}; _aui_osc133_precmd"
"#;

/// The zsh half of OSC 133 shell integration, with `nonce` baked into every
/// marker it emits (`A`, `B`, `C` and `D` all carry `k=<nonce>`).
///
/// See [`ZSH_TEMPLATE`] for what the snippet does and why; the only
/// difference is that this one is ready to source. It is written into the
/// throwaway `ZDOTDIR` by [`Pty`], after the user's own files.
pub fn zsh_integration(nonce: &str) -> String {
    ZSH_TEMPLATE.replace(NONCE_PLACEHOLDER, nonce)
}

/// The bash half of OSC 133 shell integration, with `nonce` baked into every
/// marker it emits (`A`, `B`, `C` and `D` all carry `k=<nonce>`).
///
/// See [`BASH_TEMPLATE`] for what the snippet does and why; the only
/// difference is that this one is ready to source. It is written out as the
/// generated `--rcfile` by [`Pty`], which already sources the user's own
/// files inside it.
pub fn bash_integration(nonce: &str) -> String {
    BASH_TEMPLATE.replace(NONCE_PLACEHOLDER, nonce)
}

/// Mints a per-session nonce: a random token every emitted OSC 133 marker
/// carries as `k=<nonce>`, so a program that prints a bare `OSC 133` cannot
/// forge a block boundary.
///
/// Randomness is a mix of the process id, a nanosecond timestamp and a
/// counter — no extra dependencies for a token that only has to be unique
/// per session, not unguessable across machines.
pub fn generate_nonce() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:x}{:x}", std::process::id(), nanos, n)
}

/// How to spawn a shell: which shell, where, which extra environment, and the
/// per-session nonce its OSC 133 markers carry.
///
/// The `env` list is how the host passes its own marker variables down: this
/// crate never names them, it just installs whatever the config carries
/// alongside `TERM` and `COLORTERM`. The `nonce` is always present — mint one
/// with [`generate_nonce`] ([`PtyConfig::new`] does it for you) — so a
/// session cannot be spawned without marked boundaries.
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// The shell binary to spawn, e.g. `/bin/zsh`.
    pub shell: String,
    /// The directory the shell starts in.
    pub cwd: PathBuf,
    /// Extra environment variables for the child, installed as-is.
    pub env: Vec<(String, String)>,
    /// The per-session token every emitted marker carries as `k=<nonce>`.
    pub nonce: String,
}

impl PtyConfig {
    /// A config for `shell` in `cwd`, with a fresh [`generate_nonce`] nonce
    /// and no extra environment.
    pub fn new(shell: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self { shell: shell.into(), cwd: cwd.into(), env: Vec::new(), nonce: generate_nonce() }
    }

    /// A config for the user's login shell (see [`login_shell`]) in `cwd`.
    pub fn login(cwd: impl Into<PathBuf>) -> Self {
        Self::new(Self::default_shell(), cwd)
    }

    /// The user's login shell, or `/bin/zsh` when `$SHELL` is unset or empty.
    pub fn default_shell() -> String {
        std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| DEFAULT_SHELL.to_string())
    }

    /// Pins the nonce, e.g. to replay a fixed session in a test.
    pub fn with_nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = nonce.into();
        self
    }

    /// Adds one extra environment variable for the child.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }
}

/// The user's login shell, or `/bin/zsh` when `$SHELL` is not set — the
/// argument [`spawn`](TerminalBackend::spawn) wants. A thin wrapper over
/// [`PtyConfig::default_shell`] so existing callers do not break.
pub fn login_shell() -> String {
    PtyConfig::default_shell()
}

/// A live pseudo-terminal. Build one, then
/// [`spawn`](TerminalBackend::spawn) it — or [`spawn_config`](Pty::spawn_config)
/// it for a full [`PtyConfig`] with extra environment.
pub struct Pty {
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    rx: Option<Receiver<Vec<u8>>>,
    reader: Option<JoinHandle<()>>,
    size: PtySize,
    /// Kept alive for the life of the session: dropping it removes the
    /// wrapper `ZDOTDIR`.
    zdotdir: Option<TempDir>,
    /// Kept alive for the life of the session: dropping it removes the
    /// generated bash `--rcfile`. Only one of this and `zdotdir` is set.
    bash_dir: Option<TempDir>,
    /// This session's nonce: every marker the child emits carries it. Always
    /// present, so spawning without marked boundaries is impossible.
    nonce: String,
    /// Extra environment for the child, from [`PtyConfig::env`].
    extra_env: Vec<(String, String)>,
    exited: bool,
}

impl Pty {
    /// A pty that has not been spawned yet, with a fresh [`generate_nonce`]
    /// nonce.
    pub fn new() -> Self {
        Self::with_nonce(generate_nonce())
    }

    /// A pty that has not been spawned yet, with a pinned nonce — e.g. to
    /// replay a fixed session in a test.
    pub fn with_nonce(nonce: impl Into<String>) -> Self {
        Self {
            master: None,
            writer: None,
            child: None,
            rx: None,
            reader: None,
            size: PtySize { rows: DEFAULT_ROWS, cols: DEFAULT_COLS, pixel_width: 0, pixel_height: 0 },
            zdotdir: None,
            bash_dir: None,
            nonce: nonce.into(),
            extra_env: Vec::new(),
            exited: false,
        }
    }

    /// This session's nonce: the `k=<nonce>` every emitted marker carries.
    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    /// The throwaway `ZDOTDIR` this session's shell was started with, while
    /// the session lasts.
    pub fn zdotdir(&self) -> Option<&Path> {
        self.zdotdir.as_ref().map(|d| d.path())
    }

    /// The directory holding this session's generated bash `--rcfile`, while
    /// the session lasts.
    pub fn bash_dir(&self) -> Option<&Path> {
        self.bash_dir.as_ref().map(|d| d.path())
    }

    /// Spawns the shell described by `config`: its nonce and extra env become
    /// this session's, so every marker the child emits carries
    /// `config.nonce`.
    pub fn spawn_config(&mut self, config: &PtyConfig) -> std::io::Result<()> {
        self.nonce = config.nonce.clone();
        self.extra_env = config.env.clone();
        self.spawn_inner(&config.shell.clone(), &config.cwd.clone())
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
        self.bash_dir = None;
        self.exited = true;
    }

    /// The guts of [`spawn`](TerminalBackend::spawn): `$SHELL -l -i` with
    /// `TERM`, `COLORTERM` and [`extra_env`](Pty::spawn_config), plus shell
    /// integration — a chained `ZDOTDIR` for zsh, a generated `--rcfile`
    /// for bash, nothing for anything else (which runs unmarked: the whole
    /// stream becomes one running block, and that is not an error).
    fn spawn_inner(&mut self, shell: &str, cwd: &Path) -> std::io::Result<()> {
        // "Calling it twice replaces the session" — so the old shell has to go
        // first, or the previous reader thread keeps feeding this pty.
        self.shutdown();
        let pair = native_pty_system().openpty(self.size).map_err(io_err)?;
        let mut cmd = CommandBuilder::new(shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, value) in &self.extra_env {
            cmd.env(key, value);
        }
        let nonce = self.nonce.clone();
        if shell.ends_with("zsh") {
            cmd.arg("-l");
            cmd.arg("-i");
            match Self::write_zdotdir(&nonce) {
                Ok(dir) => {
                    cmd.env("ZDOTDIR", dir.path());
                    self.zdotdir = Some(dir);
                }
                Err(e) => eprintln!("aui-terminal: no shell integration ({e})"),
            }
        } else if shell.ends_with("bash") {
            match Self::write_bash_rc(&nonce) {
                Ok(dir) => {
                    // GNU long options travel first: the macOS bash 3.2
                    // rejects them after short options.
                    cmd.arg("--rcfile");
                    cmd.arg(dir.path().join(BASH_RC_NAME).to_string_lossy().into_owned());
                    cmd.arg("-l");
                    cmd.arg("-i");
                    self.bash_dir = Some(dir);
                }
                Err(e) => eprintln!("aui-terminal: no shell integration ({e})"),
            }
        } else {
            cmd.arg("-l");
            cmd.arg("-i");
        }
        cmd.cwd(cwd);
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

    /// Writes a `ZDOTDIR` wrapper that sources the user's own zsh files and
    /// then installs [`zsh_integration`], using the original `ZDOTDIR` (or
    /// `$HOME` when unset) for both.
    fn write_zdotdir(nonce: &str) -> std::io::Result<TempDir> {
        match std::env::var("ZDOTDIR") {
            Ok(orig) if !orig.is_empty() => {
                Self::write_zdotdir_for(nonce, &orig, &format!("export ZDOTDIR={}", sh_quote(&orig)))
            }
            _ => {
                let home = std::env::var("HOME").unwrap_or_default();
                Self::write_zdotdir_for(nonce, &home, "unset ZDOTDIR")
            }
        }
    }

    /// Writes the four chained files — `.zshenv`, `.zprofile`, `.zshrc` and
    /// `.zlogin` — each of which sources the user's own file from `user_dir`
    /// first, then runs `restore` (putting `ZDOTDIR` back to its original
    /// value so children of the shell see the real one), and only then
    /// installs the hooks, so the hooks land last and survive
    /// powerlevel10k's instant prompt. The user's files are read, never
    /// written.
    fn write_zdotdir_for(nonce: &str, user_dir: &str, restore: &str) -> std::io::Result<TempDir> {
        let dir = TempDir::new("aui-zdotdir")?;
        let snippet = zsh_integration(nonce);
        for name in ["zshenv", "zprofile", "zshrc", "zlogin"] {
            let user = sh_quote(&format!("{user_dir}/.{name}"));
            let body = format!("[[ -f {user} ]] && source {user}\n{restore}\n{snippet}");
            std::fs::write(dir.path().join(format!(".{name}")), body)?;
        }
        Ok(dir)
    }

    /// Writes the generated bash `--rcfile` (see [`BASH_TEMPLATE`]) into a
    /// throwaway directory and hands the directory back; it lives as long as
    /// the session does.
    fn write_bash_rc(nonce: &str) -> std::io::Result<TempDir> {
        let dir = TempDir::new("aui-bashrc")?;
        std::fs::write(dir.path().join(BASH_RC_NAME), bash_integration(nonce))?;
        Ok(dir)
    }
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
        self.spawn_inner(shell, cwd)
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
/// for `ZDOTDIR` and the bash `--rcfile`, and this crate would rather not take
/// a dependency for four lines of `std::fs`.
struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        // A counter as well as the clock: parallel tests share one process,
        // and two wrappers created in the same nanosecond must not share one
        // directory — since the nonce era they carry different bytes.
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("{prefix}-{}-{nanos}-{n}", std::process::id()));
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
    use crate::fake::{FakePty, ScriptChunk};
    use crate::parser::BlockParser;
    use aui::workbench::BlockState;
    use std::time::{Duration, Instant};

    /// How long the shell gets to boot, run two commands and report them.
    /// A cold zsh with the user's own rc files is the slow part.
    const TEST_TIMEOUT: Duration = Duration::from_secs(20);

    /// A two-command session whose markers all carry `k=<nonce>`.
    fn nonced_script(nonce: &str) -> Vec<ScriptChunk> {
        let a = format!("\u{1b}]133;A;k={nonce}\u{7}");
        let b = format!("\u{1b}]133;B;k={nonce}\u{7}");
        let c = format!("\u{1b}]133;C;k={nonce}\u{7}");
        let d = |code: i32| format!("\u{1b}]133;D;{code};k={nonce}\u{7}");
        vec![
            ScriptChunk::new(0, format!("{a}~/work/acme $ {b}echo hi{c}")),
            ScriptChunk::new(100, format!("hi\n{}", d(0))),
            ScriptChunk::new(200, format!("{a}~/work/acme $ {b}false{c}")),
            ScriptChunk::new(300, d(1)),
        ]
    }

    /// Correctly nonced `A`/`B`/`C`/`D` markers replay into a `Done` block
    /// and a `Failed` block, exactly as a real integrated shell's would.
    #[test]
    fn nonced_markers_become_done_and_failed_blocks() {
        let parser = FakePty::replay(&nonced_script("test-nonce-1"));
        let blocks = parser.blocks();
        assert_eq!(blocks.len(), 2, "{blocks:#?}");
        assert_eq!(blocks[0].command, "echo hi");
        assert_eq!(blocks[0].state, BlockState::Done);
        assert_eq!(blocks[0].output, vec!["hi".to_string()]);
        assert_eq!(blocks[1].command, "false");
        assert_eq!(blocks[1].state, BlockState::Failed);
        assert!(blocks[1].output.is_empty(), "{blocks:#?}");
        assert!(blocks[1].duration.contains("exit 1"), "{:?}", blocks[1].duration);
    }

    /// A marker with the wrong nonce is still bytes the shell can emit and a
    /// script can carry. Filtering it out is the parser's job (not this
    /// module's) — this proves the bytes survive a scripted replay.
    #[test]
    fn a_wrong_nonce_marker_stays_in_the_bytes() {
        let emitted = zsh_integration("right-nonce");
        assert!(emitted.contains("k=right-nonce"));
        assert!(!emitted.contains("k=wrong-nonce"));
        let a = "\u{1b}]133;A;k=right-nonce\u{7}";
        let b = "\u{1b}]133;B;k=right-nonce\u{7}";
        let c = "\u{1b}]133;C;k=right-nonce\u{7}";
        let forged = "\u{1b}]133;D;0;k=wrong-nonce\u{7}".to_string();
        let script =
            vec![ScriptChunk::new(0, format!("{a}$ {b}echo hi{c}hi\n")), ScriptChunk::new(50, forged)];
        let raw: Vec<u8> = script.iter().flat_map(|ch| ch.bytes.iter().copied()).collect();
        let needle = b"k=wrong-nonce";
        assert!(
            raw.windows(needle.len()).any(|w| w == needle),
            "the forged marker must be present in the scripted bytes"
        );
        let parser = FakePty::replay(&script);
        assert!(!parser.blocks().is_empty(), "the stream still parses as blocks");
    }

    /// Every emitted zsh marker carries the nonce, and the three details that
    /// make the snippet work on a real machine are still there: `B`
    /// re-appended from `precmd`, `%{…%}` around the escape, `unsetopt
    /// PROMPT_SP`.
    #[test]
    fn zsh_snippet_marks_every_boundary_and_keeps_the_three_details() {
        let snippet = zsh_integration("abc123");
        assert_eq!(snippet.matches("k=abc123").count(), 4, "{snippet}");
        assert!(snippet.contains("%{"), "{snippet}");
        assert!(snippet.contains("%}"), "{snippet}");
        assert!(snippet.contains("PS1=\"${PS1//$_AUI_OSC133_B/}${_AUI_OSC133_B}\""));
        assert!(snippet.contains("add-zsh-hook precmd _aui_osc133_precmd"));
        assert!(snippet.contains("add-zsh-hook preexec _aui_osc133_preexec"));
        assert!(snippet.contains("unsetopt PROMPT_SP"));
    }

    /// Every chained file sources the user's own file first, restores
    /// `ZDOTDIR`, and only then installs the hooks — in that order.
    #[test]
    fn zdotdir_chains_four_files_that_source_restore_then_hook() {
        let dir =
            Pty::write_zdotdir_for("test-nonce", "/tmp/au i-home", "export ZDOTDIR='/tmp/au i-home'")
                .expect("the wrapper writes");
        for name in ["zshenv", "zprofile", "zshrc", "zlogin"] {
            let body = std::fs::read_to_string(dir.path().join(format!(".{name}"))).expect("four files");
            let source = format!("[[ -f '/tmp/au i-home/.{name}' ]] && source '/tmp/au i-home/.{name}'");
            let restore = body.find("export ZDOTDIR='/tmp/au i-home'").expect("restore");
            let hook = body.find("add-zsh-hook precmd").expect("hooks");
            assert!(body.contains(&source), "{body}");
            assert!(body.find(&source).unwrap() < restore, "{body}");
            assert!(restore < hook, "{body}");
            assert!(body.contains("k=test-nonce"), "{body}");
        }
    }

    /// When `ZDOTDIR` was unset, the wrapper sources from `$HOME` and
    /// restores the unset state.
    #[test]
    fn zdotdir_restore_unsets_when_nothing_was_set() {
        let dir = Pty::write_zdotdir_for("n", "/home/ada", "unset ZDOTDIR").expect("the wrapper writes");
        let body = std::fs::read_to_string(dir.path().join(".zshrc")).expect("zshrc");
        assert!(body.contains("[[ -f '/home/ada/.zshrc' ]] && source '/home/ada/.zshrc'"), "{body}");
        assert!(body.contains("unset ZDOTDIR"), "{body}");
    }

    /// The generated bash `--rcfile` sources the user's login files (when a
    /// login shell) and `~/.bashrc`, then installs `PROMPT_COMMAND` / `DEBUG`
    /// hooks whose markers all carry the nonce.
    #[test]
    fn bash_rcfile_sources_user_files_then_installs_hooks() {
        let dir = Pty::write_bash_rc("bash-nonce").expect("the rcfile writes");
        let body = std::fs::read_to_string(dir.path().join(BASH_RC_NAME)).expect("the rcfile");
        assert!(body.contains("shopt -q login_shell"), "{body}");
        for login in [".bash_profile", ".bash_login", ".profile"] {
            assert!(body.contains(login), "{body}");
        }
        assert!(body.contains("[ -f \"$HOME/.bashrc\" ] && . \"$HOME/.bashrc\""), "{body}");
        let rc = body.find("[ -f \"$HOME/.bashrc\" ]").unwrap();
        let hooks = body.find("PROMPT_COMMAND=").expect("PROMPT_COMMAND");
        assert!(rc < hooks, "hooks install after the user's files:\n{body}");
        assert!(body.contains("trap '_aui_osc133_preexec' DEBUG"), "{body}");
        assert_eq!(body.matches("k=bash-nonce").count(), 4, "{body}");
        assert!(body.contains("\\[") && body.contains("\\]"), "{body}");
    }

    /// Every session mints its own nonce, and a `Pty` cannot exist without
    /// one: spawning without marked boundaries is impossible by construction.
    #[test]
    fn pty_config_mints_a_fresh_nonce_per_session() {
        let a = PtyConfig::new("/bin/zsh", std::env::temp_dir());
        let b = PtyConfig::new("/bin/zsh", std::env::temp_dir());
        assert!(!a.nonce.is_empty());
        assert_ne!(a.nonce, b.nonce);
        let pinned = PtyConfig::new("/bin/zsh", std::env::temp_dir()).with_nonce("fixed");
        assert_eq!(pinned.nonce, "fixed");
        assert!(!login_shell().is_empty());
        assert!(!Pty::new().nonce().is_empty());
        assert_eq!(Pty::with_nonce("n").nonce(), "n");
    }

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
        let prompt_end = format!("\x1b]133;B;k={}\x07", pty.nonce());
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
            let prompts =
                raw.windows(prompt_end.len()).filter(|w| *w == prompt_end.as_bytes()).count();
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

    /// Spawns a real zsh through [`PtyConfig`] and asserts a nonce-marked
    /// block appears. Ignored: it needs a real interactive shell and does not
    /// run in the gate.
    #[test]
    #[ignore]
    fn a_real_zsh_reports_a_nonced_block() {
        real_shell_reports_a_nonced_block("/bin/zsh");
    }

    /// Spawns a real bash through [`PtyConfig`] (generated `--rcfile`) and
    /// asserts a nonce-marked block appears. Ignored: it needs a real
    /// interactive shell and does not run in the gate.
    #[test]
    #[ignore]
    fn a_real_bash_reports_a_nonced_block() {
        real_shell_reports_a_nonced_block("/bin/bash");
    }

    /// Boots `shell` under a [`PtyConfig`], echoes a sentinel environment
    /// value (proving the config's extra env reaches the child), and waits
    /// for the nonce-marked block to finish `Done` with that output.
    fn real_shell_reports_a_nonced_block(shell: &str) {
        if !Path::new(shell).exists() {
            eprintln!("skipped: no {shell}");
            return;
        }
        let config = PtyConfig::new(shell, std::env::temp_dir()).with_env("AUI_L3_PROBE", "l3-ok");
        let mut pty = Pty::new();
        pty.spawn_config(&config).expect("the shell spawns");
        assert_eq!(pty.nonce(), config.nonce, "the session carries the config nonce");
        let mut parser = BlockParser::new();
        let prompt_end = format!("\x1b]133;B;k={}\x07", config.nonce);
        let deadline = Instant::now() + TEST_TIMEOUT;
        let mut raw: Vec<u8> = Vec::new();
        let mut sent = false;
        let mut done = false;
        while Instant::now() < deadline && !done {
            for event in pty.poll() {
                if let TermEvent::Output(bytes) = event {
                    parser.feed(&bytes);
                    raw.extend_from_slice(&bytes);
                }
            }
            if !sent && raw.windows(prompt_end.len()).any(|w| w == prompt_end.as_bytes()) {
                pty.write(b"echo hi $AUI_L3_PROBE\n");
                sent = true;
            }
            done = sent
                && parser
                    .blocks()
                    .iter()
                    .any(|b| b.command == "echo hi $AUI_L3_PROBE" && b.state == BlockState::Done);
            std::thread::sleep(Duration::from_millis(10));
        }
        let blocks = parser.blocks();
        assert!(done, "timed out with blocks {blocks:#?}");
        let hi = blocks
            .iter()
            .find(|b| b.command == "echo hi $AUI_L3_PROBE")
            .unwrap_or_else(|| panic!("{blocks:#?}"));
        assert_eq!(hi.output, vec!["hi l3-ok".to_string()], "{blocks:#?}");
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
