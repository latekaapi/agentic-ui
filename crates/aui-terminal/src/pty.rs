//! A real pseudo-terminal running the user's own login shell (feature `pty`).
//!
//! `portable-pty` opens the pty and spawns the shell; a reader thread pushes
//! everything the shell prints down an [`std::sync::mpsc`] channel, and
//! [`poll`](TerminalBackend::poll) drains it without blocking the UI thread.
//!
//! A login shell prints no OSC 133 markers unless it has been taught to, so
//! there would be no blocks. This crate teaches it: [`zsh_integration`] and
//! [`bash_integration`] build the snippet for a session nonce, written into a
//! throwaway directory — a `ZDOTDIR` whose `.zshenv` chains the user's own
//! `.zshenv` for zsh, a `--rcfile` sourcing the user's own bash files for
//! bash. The user's configuration is read, never written, and the wrapper
//! lives in a temporary directory that goes away with the process.
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
/// and the typed command begins, and it is emitted from a `zle-line-init`
/// widget: that widget runs when the line editor starts — after every
/// `precmd`, after the prompt has been drawn — which is exactly where the
/// marker belongs. Because `B` no longer lives in `PS1`, no theme can
/// overwrite it.
///
/// The widget binding is re-asserted on every `precmd`: the wrapper `.zshenv`
/// runs before the user's own rc files, and a user `precmd` (zsh-vi-mode's
/// precmd init, zinit turbo/`wait` plugins, zsh-defer) can bind its own
/// `zle-line-init` after ours at any time. Each `precmd` therefore checks
/// whether the currently bound `zle-line-init` is still ours; if it is not,
/// whatever is bound now is captured as the "original" and ours is installed
/// in front of it. Whatever the user defined is chained, never destroyed.
///
/// `B` is emitted only while the shell is at its own prompt: the `precmd`
/// that emits `A` sets a flag, and the widget emits `B` only when the flag is
/// set (clearing it), so a nested line editor inside a running command
/// (`vared`, `zle recursive-edit`) stays silent. When the widget stays silent
/// it still chains to the original.
///
/// `PROMPT_SP` is switched off as well: it is the reverse-video `%` and the
/// row of padding zsh prints to show a command whose output had no trailing
/// newline, and it would land in the block's output as a line of noise.
const ZSH_TEMPLATE: &str = r#"
# OSC 133 shell integration for the aui block terminal.
autoload -Uz add-zsh-hook

_aui_osc133_precmd() {
  local _aui_status=$?
  if [[ -n ${_AUI_RUNNING-} ]]; then
    printf '\033]133;D;%s;k=__AUI_NONCE__\007' "$_aui_status"
    unset _AUI_RUNNING
  fi
  printf '\033]133;A;k=__AUI_NONCE__\007'
  # At the shell's own prompt now: the next line-editor start owns the B.
  _AUI_AT_PROMPT=1
  _aui_osc133_install_zle 2>/dev/null
}

_aui_osc133_preexec() {
  _AUI_RUNNING=1
  # A command is running now, so a later line-editor start (vared,
  # recursive-edit) is nested output, not a new prompt.
  unset _AUI_AT_PROMPT
  printf '\033]133;C;k=__AUI_NONCE__\007'
}

# The prompt-end marker, emitted where the line editor starts: after every
# precmd, after the prompt has been drawn. Only between A and C: a nested
# line editor inside a running command stays silent (but still chains).
# No theme can overwrite it the way a PS1-embedded marker could be.
_aui_osc133_zle_line_init() {
  if [[ -n ${_AUI_AT_PROMPT-} ]]; then
    unset _AUI_AT_PROMPT
    printf '\033]133;B;k=__AUI_NONCE__\007'
  fi
  # Chain whatever the user's own rc files defined; destroy nothing. The
  # direct call is the fallback for contexts with no active line editor.
  if (( ${+widgets[_aui_orig_zle_line_init]} )); then
    zle _aui_orig_zle_line_init -- "$@" 2>/dev/null || {
      typeset -f _aui_orig_zle_line_init >/dev/null && _aui_orig_zle_line_init "$@"
    }
  elif typeset -f _aui_orig_zle_line_init >/dev/null; then
    _aui_orig_zle_line_init "$@"
  fi
  return 0
}

# Re-asserted on every precmd: the wrapper .zshenv runs before the user's
# rc files, so a user precmd that binds its own zle-line-init afterwards
# (vi-mode init, turbo/wait plugins, zsh-defer) would otherwise silently
# overwrite ours. When the bound widget is already ours there is nothing to
# do — which is also what keeps us from wrapping ourselves.
_aui_osc133_install_zle() {
  # Only an interactive shell has a line editor to wrap.
  [[ -o interactive ]] || return 0
  local _aui_cur=""
  if (( ${+widgets[zle-line-init]} )); then
    _aui_cur=${widgets[zle-line-init]}
  fi
  [[ $_aui_cur == "user:_aui_osc133_zle_line_init" ]] && return 0
  # Capture whatever is bound now as the original. The widget alias first:
  # aliasing keeps a real widget behind the preserved name, so the chain
  # also works where no line editor is active. A bare function with no
  # widget bound is copied instead — unless it is ourselves.
  if (( ${+widgets[zle-line-init]} )); then
    zle -A zle-line-init _aui_orig_zle_line_init 2>/dev/null || {
      typeset -f zle-line-init >/dev/null && functions[_aui_orig_zle_line_init]=$functions[zle-line-init]
    }
    # A user:* widget is backed by a plain function: keep a copy of that too,
    # so the chain also runs where no line editor is active.
    local _aui_fn=${_aui_cur#user:}
    if [[ $_aui_cur == user:* ]] && typeset -f "$_aui_fn" >/dev/null; then
      functions[_aui_orig_zle_line_init]=${functions[$_aui_fn]}
    fi
  elif typeset -f zle-line-init >/dev/null; then
    if [[ $functions[zle-line-init] != ${functions[_aui_osc133_zle_line_init]-} ]]; then
      functions[_aui_orig_zle_line_init]=$functions[zle-line-init]
    fi
  else
    # Nothing left to chain to: drop a stale original so the widget is silent.
    zle -D _aui_orig_zle_line_init 2>/dev/null || true
    unfunction _aui_orig_zle_line_init 2>/dev/null || true
  fi
  zle -N zle-line-init _aui_osc133_zle_line_init 2>/dev/null || true
}

# Deleted before re-adding, so sourcing this twice keeps one registration.
add-zsh-hook -d precmd _aui_osc133_precmd 2>/dev/null
add-zsh-hook precmd _aui_osc133_precmd
add-zsh-hook -d preexec _aui_osc133_preexec 2>/dev/null
add-zsh-hook preexec _aui_osc133_preexec

# No partial-line marker: the block terminal draws the boundaries itself.
unsetopt PROMPT_SP
"#;

/// The bash half of OSC 133 shell integration, before the nonce is filled in.
///
/// This is the whole generated `--rcfile`, and the shell is spawned as
/// `bash --rcfile <generated> -i` (no `-l`: bash ignores `--rcfile` for
/// login shells). The rcfile itself does the chaining: it sources the user's
/// own files first — the login files in bash's documented order
/// (`~/.bash_profile`, else `~/.bash_login`, else `~/.profile`, else
/// `~/.bashrc` as the fallback, exactly what a real login bash does) — so
/// their prompt setup and PATH win, and installs the hooks afterwards so
/// they observe the final prompt. A `.bash_profile` that sources `.bashrc`
/// itself therefore still runs `.bashrc` exactly once. Limitation: this
/// shell is not a login shell, so a user file guarded by `shopt -q
/// login_shell` is skipped here; the wrapper deliberately does not fake it.
/// The hooks are the bash equivalents of the zsh ones: a `PROMPT_COMMAND`
/// entry plays `precmd` (`D` closes the finished command, `A` opens the next
/// block, `B` is re-attached to whatever `PS1` the user's setup just built),
/// and a `DEBUG` trap plays `preexec` (`C` starts the output). The trap also
/// fires for the prompt command itself, so it ignores everything that runs
/// while `PROMPT_COMMAND` is executing and everything this snippet defines.
/// The user's own `DEBUG` trap, captured after their files are sourced, is
/// chained from ours (ours first, theirs after).
///
/// `PROMPT_COMMAND` brackets the user's own: the status capture runs first
/// (so `$?` is still the command's) and the marker re-attachment runs after
/// theirs. On bash ≥ 5.1 `PROMPT_COMMAND` may be an array (bash-preexec 0.5
/// uses it); that form is bracketed element-wise so later elements cannot
/// slip past the re-attachment. (There is no bash 5 on this machine, so the
/// array branch is code-correctness, not something executed here.) The `B`
/// marker is wrapped in `\[…\]` so readline does not count it towards the
/// prompt's width, and the re-attachment matches it literally so a static
/// `PS1` does not grow one copy per prompt.
const BASH_TEMPLATE: &str = r#"
# OSC 133 shell integration for the aui block terminal (bash).
# The shell runs as `bash --rcfile <this> -i` (never `-l`: bash ignores
# --rcfile for login shells), so this file chains the user's own files
# itself, in bash's documented login order, before installing the hooks
# below. `.bashrc` is the fallback of that chain — exactly what a real login
# bash does — so a `.bash_profile` that sources `.bashrc` itself still runs
# it once. Limitation: this shell is not a login shell, so a user file
# guarded by the login-shell check is skipped here; that state is
# deliberately not faked.
if [ -f "$HOME/.bash_profile" ]; then . "$HOME/.bash_profile"
elif [ -f "$HOME/.bash_login" ]; then . "$HOME/.bash_login"
elif [ -f "$HOME/.profile" ]; then . "$HOME/.profile"
else
  [ -f "$HOME/.bashrc" ] && . "$HOME/.bashrc"
fi

# The DEBUG trap the user had after their files were sourced, if any, so the
# preexec below can chain it (ours first, theirs after) instead of
# clobbering it.
_AUI_USER_DEBUG_TRAP=""
_aui_osc133_debug_line=$(trap -p DEBUG 2>/dev/null || true)
if [ -n "$_aui_osc133_debug_line" ]; then
  case "$_aui_osc133_debug_line" in
    *"_aui_osc133_preexec"*) ;;
    "trap -- "*) _AUI_USER_DEBUG_TRAP=${_aui_osc133_debug_line#trap -- }
      _AUI_USER_DEBUG_TRAP=${_AUI_USER_DEBUG_TRAP% DEBUG};;
  esac
fi
unset _aui_osc133_debug_line

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
  # Saved first: the guards and the marker below clobber both, and the
  # user's trap chained at the end should see what the trapped command saw.
  local _aui_status=$? _aui_cmd_arg=$_
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
  # Chain the user's own DEBUG trap, if they had one: ours first, theirs
  # after. Guarded against re-entry while it runs, since its own commands
  # would otherwise re-enter this handler. `$BASH_COMMAND` still names the
  # trapped command; `$?` and `$_` are restored best-effort below.
  if [ -n "${_AUI_USER_DEBUG_TRAP-}" ] && [ -z "${_AUI_IN_PREEXEC-}" ]; then
    _AUI_IN_PREEXEC=1
    _="$_aui_cmd_arg"
    (exit "$_aui_status")
    eval "${_AUI_USER_DEBUG_TRAP}"
    unset _AUI_IN_PREEXEC
  fi
}

# This brackets the user's own PROMPT_COMMAND: the status capture runs first
# and the marker re-attachment runs after theirs. PROMPT_COMMAND is an array
# on bash >= 5.1 (bash-preexec 0.5 uses it): bracket that form element-wise
# so later elements cannot slip past the re-attachment.
if declare -p PROMPT_COMMAND 2>/dev/null | grep -q 'declare -a'; then
  PROMPT_COMMAND=(_aui_osc133_begin "${PROMPT_COMMAND[@]}" _aui_osc133_precmd)
else
  PROMPT_COMMAND="_aui_osc133_begin${PROMPT_COMMAND:+; $PROMPT_COMMAND}; _aui_osc133_precmd"
fi
# Last: installing this earlier would fire it (and the user's chained trap)
# for the setup lines above.
trap '_aui_osc133_preexec' DEBUG
"#;

/// A nonce is an opaque token the markers can safely carry: non-empty,
/// at most 128 characters, ASCII alphanumerics plus `-` and `_` only.
/// Anything else could break out of the quoted shell strings the nonce is
/// spliced into, so every public entry point that takes a nonce panics on
/// one that is not. See [`assert_valid_nonce`].
const NONCE_MAX_LEN: usize = 128;

/// Panics unless `nonce` is something the markers can safely carry:
/// non-empty, at most [`NONCE_MAX_LEN`] characters, ASCII alphanumerics
/// plus `-` and `_` only. The nonce is spliced into `$'...'` ANSI-C
/// strings and `printf` format literals in generated shell source, so a
/// quote or other metacharacter would inject code into a file the shell
/// sources on every login.
fn assert_valid_nonce(nonce: &str) {
    assert!(
        !nonce.is_empty()
            && nonce.len() <= NONCE_MAX_LEN
            && nonce.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "aui-terminal: invalid nonce {nonce:?}: use 1-128 ASCII alphanumerics, '-' or '_'"
    );
}

/// The zsh half of OSC 133 shell integration, with `nonce` baked into every
/// marker it emits (`A`, `B`, `C` and `D` all carry `k=<nonce>`).
///
/// See [`ZSH_TEMPLATE`] for what the snippet does and why; the only
/// difference is that this one is ready to source. It is written into the
/// throwaway `ZDOTDIR` by [`Pty`], after the user's own files.
///
/// Panics if `nonce` is not a safe token (see [`assert_valid_nonce`]).
pub fn zsh_integration(nonce: &str) -> String {
    assert_valid_nonce(nonce);
    ZSH_TEMPLATE.replace(NONCE_PLACEHOLDER, nonce)
}

/// The bash half of OSC 133 shell integration, with `nonce` baked into every
/// marker it emits (`A`, `B`, `C` and `D` all carry `k=<nonce>`).
///
/// See [`BASH_TEMPLATE`] for what the snippet does and why; the only
/// difference is that this one is ready to source. It is written out as the
/// generated `--rcfile` by [`Pty`], which already sources the user's own
/// files inside it.
///
/// Panics if `nonce` is not a safe token (see [`assert_valid_nonce`]).
pub fn bash_integration(nonce: &str) -> String {
    assert_valid_nonce(nonce);
    BASH_TEMPLATE.replace(NONCE_PLACEHOLDER, nonce)
}

/// Mints a per-session nonce: a random token every emitted OSC 133 marker
/// carries as `k=<nonce>`, so a program that prints a bare `OSC 133` cannot
/// forge a block boundary.
///
/// The 16 bytes come from the operating system's random source (via the
/// `getrandom` crate), so the nonce is unguessable to a program running
/// inside the terminal that might otherwise forge a block boundary. This
/// fails closed: if the random source cannot be read there is no weak
/// fallback — this panics rather than minting a guessable token, because a
/// silent downgrade of the one property the nonce rests on is the worst
/// available failure mode.
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .expect("aui-terminal: OS random source unavailable; refusing to mint a guessable nonce");
    let mut out = String::with_capacity(32);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
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
    ///
    /// Private so every nonce passes through [`assert_valid_nonce`]: use
    /// [`with_nonce`](Self::with_nonce) to pin one and [`nonce`](Self::nonce)
    /// to read it back.
    nonce: String,
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
    ///
    /// Panics if `nonce` is not a safe token (see [`assert_valid_nonce`]).
    pub fn with_nonce(mut self, nonce: impl Into<String>) -> Self {
        let nonce = nonce.into();
        assert_valid_nonce(&nonce);
        self.nonce = nonce;
        self
    }

    /// The per-session token every emitted marker carries as `k=<nonce>`.
    pub fn nonce(&self) -> &str {
        &self.nonce
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
    ///
    /// Panics if `nonce` is not a safe token (see [`assert_valid_nonce`]).
    pub fn with_nonce(nonce: impl Into<String>) -> Self {
        let nonce = nonce.into();
        assert_valid_nonce(&nonce);
        Self {
            master: None,
            writer: None,
            child: None,
            rx: None,
            reader: None,
            size: PtySize { rows: DEFAULT_ROWS, cols: DEFAULT_COLS, pixel_width: 0, pixel_height: 0 },
            zdotdir: None,
            bash_dir: None,
            nonce,
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
    /// this session's, so every marker the child emits carries the config
    /// nonce.
    ///
    /// The nonce is validated before any resource is opened, so a bad nonce
    /// fails without leaking a pty pair.
    pub fn spawn_config(&mut self, config: &PtyConfig) -> std::io::Result<()> {
        assert_valid_nonce(&config.nonce);
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
                    // No `-l`: bash ignores `--rcfile` for login shells, so
                    // the generated rcfile chains the user's login files
                    // itself (see BASH_TEMPLATE). GNU long options travel
                    // first: the macOS bash 3.2 rejects them after short
                    // options.
                    cmd.arg("--rcfile");
                    cmd.arg(dir.path().join(BASH_RC_NAME).to_string_lossy().into_owned());
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

    /// Writes the one chained file — `.zshenv` — which runs `restore`
    /// (putting `ZDOTDIR` back to its original value) BEFORE sourcing the
    /// user's own `.zshenv` from `user_dir`, and then installs
    /// [`zsh_integration`]. The user's files are read, never written.
    ///
    /// Restoring first is what makes the user's own `.zshenv` see their real
    /// `ZDOTDIR`: the standard XDG idiom inside it — `source
    /// "$ZDOTDIR/exports.zsh"`, or `${ZDOTDIR:-$HOME}/…` — would otherwise
    /// resolve into this throwaway wrapper and fail. At that point `ZDOTDIR`
    /// is by definition still the wrapper, so the guard is evaluated there;
    /// its right-hand side is quoted so it compares literally rather than as
    /// a glob pattern (a `$TMPDIR` containing `[` or `*` must not change the
    /// outcome). A user `.zshenv` that sets `ZDOTDIR` itself then wins
    /// naturally and `.zshrc` loads from their directory. Anything that stops
    /// zsh reaching a later startup file (`emulate sh`, `unsetopt RCS`,
    /// `exec fish`, …) cannot strand the restore either, because it already
    /// ran — `zsh -c` included.
    fn write_zdotdir_for(nonce: &str, user_dir: &str, restore: &str) -> std::io::Result<TempDir> {
        let dir = TempDir::new("aui-zdotdir")?;
        let snippet = zsh_integration(nonce);
        let user = sh_quote(&format!("{user_dir}/.zshenv"));
        let body = format!(
            "__aui_wrapper_zdotdir=$ZDOTDIR\n\
             if [[ ${{ZDOTDIR-}} == \"$__aui_wrapper_zdotdir\" ]]; then\n  {restore}\nfi\n\
             [[ -f {user} ]] && source {user}\n\
             {snippet}\n\
             unset __aui_wrapper_zdotdir"
        );
        std::fs::write(dir.path().join(".zshenv"), body)?;
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
        // A crashed or SIGKILLed app leaks its wrappers, because `Drop` below
        // never runs: sweep this crate's own stale ones on every spawn.
        Self::sweep_stale();
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

    /// Removes this crate's own wrapper directories older than 24 h
    /// (`aui-zdotdir-*`, `aui-bashrc-*`). Defensive and never fatal:
    /// anything that cannot be read or removed is left alone, and entries
    /// owned by another user simply fail to remove and are ignored.
    fn sweep_stale() {
        const MAX_AGE_SECS: u64 = 24 * 60 * 60;
        let tmp = std::env::temp_dir();
        let entries = match std::fs::read_dir(&tmp) {
            Ok(e) => e,
            Err(_) => return,
        };
        let now = std::time::SystemTime::now();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("aui-zdotdir-") && !name.starts_with("aui-bashrc-") {
                continue;
            }
            let old_enough = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .map(|d| d.as_secs() > MAX_AGE_SECS)
                .unwrap_or(false);
            if old_enough {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
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

    /// Every emitted zsh marker carries the nonce, and the details that make
    /// the snippet work on a real machine are still there: `B` from a
    /// chained `zle-line-init` widget (never from `PS1`), re-asserted on
    /// every `precmd`, gated on the at-prompt flag, `unsetopt PROMPT_SP`.
    #[test]
    fn zsh_snippet_marks_every_boundary_and_keeps_the_three_details() {
        let snippet = zsh_integration("abc123");
        assert_eq!(snippet.matches("k=abc123").count(), 4, "{snippet}");
        assert!(snippet.contains("_aui_osc133_zle_line_init"), "{snippet}");
        assert!(snippet.contains("zle -N zle-line-init _aui_osc133_zle_line_init"), "{snippet}");
        assert!(snippet.contains("_aui_osc133_install_zle"), "{snippet}");
        assert!(!snippet.contains("_AUI_ZLE_INSTALLED"), "no one-shot guard:\n{snippet}");
        assert!(snippet.contains("_AUI_AT_PROMPT"), "the B gate:\n{snippet}");
        assert!(snippet.contains("user:_aui_osc133_zle_line_init"), "re-assert check:\n{snippet}");
        assert!(!snippet.contains("_AUI_OSC133_B"), "no PS1 marker variable:\n{snippet}");
        assert!(!snippet.contains("PS1="), "nothing rewrites PS1:\n{snippet}");
        assert!(snippet.contains("add-zsh-hook -d precmd _aui_osc133_precmd"), "{snippet}");
        assert!(snippet.contains("add-zsh-hook precmd _aui_osc133_precmd"), "{snippet}");
        assert!(snippet.contains("add-zsh-hook -d preexec _aui_osc133_preexec"), "{snippet}");
        assert!(snippet.contains("add-zsh-hook preexec _aui_osc133_preexec"), "{snippet}");
        assert!(snippet.contains("unsetopt PROMPT_SP"), "{snippet}");
    }

    /// The wrapper is a single `.zshenv`: it restores `ZDOTDIR` first, then
    /// sources the user's own `.zshenv`, then installs the hooks — guarded
    /// so a user's `.zshenv` that moved `ZDOTDIR` itself (the XDG pattern)
    /// wins. No other wrapper file exists: they were the source of both
    /// the hook-ordering and the stranded-restore defects.
    #[test]
    fn zdotdir_restores_in_zshenv_and_chains_only_that_file() {
        let restore = "export ZDOTDIR='/tmp/au i-home'";
        let dir = Pty::write_zdotdir_for("test-nonce", "/tmp/au i-home", restore)
            .expect("the wrapper writes");
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .expect("readable")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec![".zshenv".to_string()], "{entries:?}");
        let body = std::fs::read_to_string(dir.path().join(".zshenv")).expect(".zshenv");
        let source = "[[ -f '/tmp/au i-home/.zshenv' ]] && source '/tmp/au i-home/.zshenv'";
        let guard = body.find("__aui_wrapper_zdotdir").expect("guard");
        let restore_at = body.find(restore).expect("restore");
        let source_at = body.find(source).expect("user file");
        let hook = body.find("add-zsh-hook precmd").expect("hooks");
        assert!(body.contains(source), "{body}");
        assert!(restore_at < source_at, "restore before the user file:\n{body}");
        assert!(source_at < hook, "user file before the hooks:\n{body}");
        assert!(guard < restore_at, "guard saved before the restore:\n{body}");
        assert!(
            body.contains("== \"$__aui_wrapper_zdotdir\""),
            "the comparison is quoted, not a glob:\n{body}"
        );
        assert!(body.contains("k=test-nonce"), "{body}");
    }

    /// A nonce with a quote, a metacharacter, or nothing at all must not
    /// reach the generated shell source: it would break out of the quoted
    /// strings it is spliced into.
    #[test]
    #[should_panic(expected = "invalid nonce")]
    fn zsh_integration_rejects_a_nonce_with_a_quote() {
        zsh_integration("abc'; touch /tmp/pwned; echo '");
    }

    /// The same boundary on the bash side: command substitution in the
    /// nonce must not reach the generated `--rcfile`.
    #[test]
    #[should_panic(expected = "invalid nonce")]
    fn bash_integration_rejects_a_nonce_with_metacharacters() {
        bash_integration("$(touch /tmp/pwned)");
    }

    /// The pinned-nonce constructors are the same boundary: they panic on
    /// an empty nonce rather than emitting unmarked markers.
    #[test]
    #[should_panic(expected = "invalid nonce")]
    fn pty_config_with_nonce_rejects_an_empty_nonce() {
        PtyConfig::new("/bin/zsh", std::env::temp_dir()).with_nonce("");
    }

    /// A nonce that is too long to be a token is rejected as well.
    #[test]
    #[should_panic(expected = "invalid nonce")]
    fn pty_with_nonce_rejects_an_overlong_nonce() {
        Pty::with_nonce("a".repeat(129));
    }

    /// Generated nonces are 32 hex characters, and valid nonces — including
    /// `-` and `_` — pass every boundary untouched.
    #[test]
    fn generated_nonces_are_hex_and_valid_nonces_pass() {
        for _ in 0..10 {
            let nonce = generate_nonce();
            assert_eq!(nonce.len(), 32, "{nonce}");
            assert!(nonce.bytes().all(|b| b.is_ascii_hexdigit()), "{nonce}");
        }
        let a = generate_nonce();
        let b = generate_nonce();
        assert_ne!(a, b);
        assert!(zsh_integration("abc-123_XYZ").contains("k=abc-123_XYZ"));
        assert!(bash_integration("abc-123_XYZ").contains("k=abc-123_XYZ"));
        assert_eq!(
            PtyConfig::new("/bin/zsh", std::env::temp_dir()).with_nonce("pinned_1-A").nonce(),
            "pinned_1-A"
        );
        assert_eq!(Pty::with_nonce("pinned_1-A").nonce(), "pinned_1-A");
    }

    /// When `ZDOTDIR` was unset, the wrapper sources from `$HOME` and
    /// restores the unset state.
    #[test]
    fn zdotdir_restore_unsets_when_nothing_was_set() {
        let dir = Pty::write_zdotdir_for("n", "/home/ada", "unset ZDOTDIR").expect("the wrapper writes");
        let body = std::fs::read_to_string(dir.path().join(".zshenv")).expect(".zshenv");
        assert!(body.contains("[[ -f '/home/ada/.zshenv' ]] && source '/home/ada/.zshenv'"), "{body}");
        assert!(body.contains("unset ZDOTDIR"), "{body}");
    }

    /// The generated bash `--rcfile` chains the user's login files in bash's
    /// documented order with `~/.bashrc` as the fallback — the shell runs as
    /// `bash --rcfile <this> -i`, never `-l` — then captures the user's
    /// `DEBUG` trap and installs `PROMPT_COMMAND` / `DEBUG` hooks whose
    /// markers all carry the nonce.
    #[test]
    fn bash_rcfile_sources_user_files_then_installs_hooks() {
        let dir = Pty::write_bash_rc("bash-nonce").expect("the rcfile writes");
        let body = std::fs::read_to_string(dir.path().join(BASH_RC_NAME)).expect("the rcfile");
        assert!(body.contains("login shell"), "the login-shell limitation is documented:\n{body}");
        let profile = body.find(".bash_profile").expect("profile chain");
        let login = body.find(".bash_login").expect("profile chain");
        let dot_profile = body.find(".profile\"").expect("profile chain");
        assert!(profile < login && login < dot_profile, "documented order:\n{body}");
        assert!(body.contains("[ -f \"$HOME/.bashrc\" ] && . \"$HOME/.bashrc\""), "{body}");
        let rc = body.find("[ -f \"$HOME/.bashrc\" ]").unwrap();
        let els = body.find("\nelse").expect("fallback");
        assert!(dot_profile < els && els < rc, "bashrc is the fallback of the chain:\n{body}");
        let capture = body.find("trap -p DEBUG").expect("DEBUG capture");
        assert!(rc < capture, "capture runs after the user's files:\n{body}");
        let hooks = body.find("PROMPT_COMMAND=").expect("PROMPT_COMMAND");
        assert!(capture < hooks, "hooks install after the capture:\n{body}");
        assert!(body.contains("declare -p PROMPT_COMMAND"), "array PROMPT_COMMAND:\n{body}");
        assert!(body.contains("_AUI_USER_DEBUG_TRAP"), "DEBUG chaining:\n{body}");
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
        assert!(!a.nonce().is_empty());
        assert_ne!(a.nonce(), b.nonce());
        let pinned = PtyConfig::new("/bin/zsh", std::env::temp_dir()).with_nonce("fixed");
        assert_eq!(pinned.nonce(), "fixed");
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

    /// A scratch "home" with the given files, plus a wrapper chained at it
    /// with `restore`. The caller runs the real zsh against both.
    /// Hermetic: the child gets its own `HOME`/`ZDOTDIR` via its
    /// environment, so the process environment (and parallel tests) are
    /// untouched.
    fn scratch_home(
        files: &[(&str, &str)],
        nonce: &str,
        restore: &str,
        tag: &str,
    ) -> (PathBuf, TempDir, String) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let home = std::env::temp_dir().join(format!(
            "aui-l3-{tag}-{}-{stamp}-{}",
            std::process::id(),
            scratch_counter(),
        ));
        std::fs::create_dir_all(&home).expect("home dir");
        for (name, body) in files {
            std::fs::write(home.join(name), body).expect("user file");
        }
        let home_str = home.to_string_lossy().into_owned();
        let dir = Pty::write_zdotdir_for(nonce, &home_str, restore).expect("the wrapper writes");
        (home, dir, home_str)
    }

    /// A per-process counter so parallel tests never share a scratch home.
    fn scratch_counter() -> u64 {
        static N: AtomicU64 = AtomicU64::new(0);
        N.fetch_add(1, Ordering::Relaxed)
    }

    /// Runs the real zsh with `ZDOTDIR` at the wrapper and `HOME` at the
    /// scratch home, and returns the combined stdout/stderr.
    fn run_zsh(home: &Path, zdotdir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("/bin/zsh")
            .env("ZDOTDIR", zdotdir)
            .env("HOME", home)
            // A hermetic shell: no system rc files, no saved state.
            .env("ZSH_DISABLE_COMPFIX", "true")
            .args(args)
            .output()
            .expect("zsh runs");
        assert!(out.status.success(), "zsh failed with {args:?}: {out:?}");
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        text
    }

    /// The XDG pattern — the user's own `~/.zshenv` moves `ZDOTDIR` — must
    /// not divert the integration: the hooks still install (so markers are
    /// still emitted), the user's own XDG rc files still load, and `ZDOTDIR`
    /// holds the user's moved value rather than the deleted wrapper.
    /// Ignored: it needs a real zsh and does not run in the default gate.
    #[test]
    #[ignore]
    fn a_real_zsh_survives_a_zdotdir_moving_zshenv() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let config =
            std::env::temp_dir().join(format!("aui-l3-xdg-conf-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&config).expect("config dir");
        let config_str = config.to_string_lossy().into_owned();
        std::fs::write(config.join(".zshrc"), "export AUI_XDG_SENTINEL=xdg-ok\n")
            .expect("xdg zshrc");
        // Originally-unset ZDOTDIR, like the common case: the restore is
        // `unset ZDOTDIR`, and the guard must let the user's move win.
        let (home, dir, _) = scratch_home(
            &[(".zshenv", &format!("export ZDOTDIR={}\n", sh_quote(&config_str)))],
            "abc123XYZ",
            "unset ZDOTDIR",
            "xdg",
        );
        let out = run_zsh(
            &home,
            dir.path(),
            &["-i", "-c", "_aui_osc133_precmd; print -l ${precmd_functions}; echo SENTINEL=$AUI_XDG_SENTINEL; echo ZDOTDIR=$ZDOTDIR"],
        );
        assert!(out.contains("_aui_osc133_precmd"), "hook installed:\n{out}");
        assert!(
            out.contains("\x1b]133;A;k=abc123XYZ\x07"),
            "the installed hook still emits markers:\n{out:?}"
        );
        assert!(out.contains("SENTINEL=xdg-ok"), "the user's XDG rc files load:\n{out}");
        assert!(
            out.contains(&format!("ZDOTDIR={config_str}")),
            "the user's own ZDOTDIR move wins:\n{out}"
        );
        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&config).ok();
    }

    /// A user `.zshrc` defining its own `zle-line-init` must not lose it,
    /// and must not lose the `B` marker either: invoking the installed
    /// widget emits `B` and then runs the user's widget. Ignored: it needs
    /// a real zsh and does not run in the default gate.
    #[test]
    #[ignore]
    fn a_real_zsh_chains_a_user_zle_line_init() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let (home, dir, home_str) = scratch_home(
            &[(
                ".zshrc",
                "zle-line-init() { touch \"$HOME/user-widget-ran\"; }\nzle -N zle-line-init\n",
            )],
            "abc123XYZ",
            "unset ZDOTDIR",
            "zle",
        );
        let _ = home_str;
        // Source the user's rc files the way a real startup would (the
        // wrapper only chains `.zshenv`), run the first precmd to lazily
        // install the widget, then invoke it the way the line editor would.
        let out = run_zsh(
            &home,
            dir.path(),
            &[
                "-i",
                "-c",
                "[[ -f $HOME/.zshrc ]] && source $HOME/.zshrc; _aui_osc133_precmd >/dev/null; _aui_osc133_zle_line_init; echo AFTER=$widgets[zle-line-init]",
            ],
        );
        assert!(
            out.contains("\x1b]133;B;k=abc123XYZ\x07"),
            "the widget emits B:\n{out:?}"
        );
        assert!(
            home.join("user-widget-ran").exists(),
            "the user's own widget still runs:\n{out}"
        );
        assert!(
            out.contains("AFTER=user:_aui_osc133_zle_line_init"),
            "the widget is rebound to ours:\n{out}"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// `ZDOTDIR` holds the user's real value (or is unset, when nothing was
    /// set) in `zsh -c`, in a non-login interactive shell, and in a login
    /// interactive shell — because the restore already ran in `.zshenv`.
    /// Ignored: it needs a real zsh and does not run in the default gate.
    #[test]
    #[ignore]
    fn a_real_zsh_restores_zdotdir_for_every_shell_kind() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        // Case 1: ZDOTDIR was originally set — the shell must see it again.
        let (home, dir, home_str) =
            scratch_home(&[], "abc123XYZ", &format!("export ZDOTDIR={}", sh_quote("/tmp/au i-real")), "zd");
        for (label, args) in [
            ("c", vec!["-c", "echo ZDOTDIR=$ZDOTDIR"]),
            ("i", vec!["-i", "-c", "echo ZDOTDIR=$ZDOTDIR"]),
            ("login", vec!["-l", "-i", "-c", "echo ZDOTDIR=$ZDOTDIR"]),
        ] {
            let out = run_zsh(&home, dir.path(), &args);
            assert!(out.contains("ZDOTDIR=/tmp/au i-real"), "{label}: ZDOTDIR restored:\n{out}");
        }
        assert!(home_str.contains("aui-l3-zd"), "{home_str}");
        std::fs::remove_dir_all(&home).ok();
        // Case 2: ZDOTDIR was originally unset — it stays unset everywhere.
        let (home, dir, _) = scratch_home(&[], "abc123XYZ", "unset ZDOTDIR", "zd-unset");
        for (label, args) in [
            ("c", vec!["-c", "echo ZDOTDIR=${ZDOTDIR-unset}"]),
            ("i", vec!["-i", "-c", "echo ZDOTDIR=${ZDOTDIR-unset}"]),
            ("login", vec!["-l", "-i", "-c", "echo ZDOTDIR=${ZDOTDIR-unset}"]),
        ] {
            let out = run_zsh(&home, dir.path(), &args);
            assert!(out.contains("ZDOTDIR=unset"), "{label}: ZDOTDIR stays unset:\n{out}");
        }
        std::fs::remove_dir_all(&home).ok();
    }

    /// Runs the real bash with `HOME` at the scratch home and `--rcfile` at
    /// the generated file, and returns the combined stdout/stderr.
    fn run_bash(home: &Path, rcfile: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("/bin/bash")
            .env("HOME", home)
            .arg("--rcfile")
            .arg(rcfile)
            .args(args)
            .output()
            .expect("bash runs");
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        text
    }

    /// The user's own `.zshenv` sees the user's real `ZDOTDIR`: the XDG idiom
    /// `source "$ZDOTDIR/exports.zsh"` loads the USER's file, not the
    /// wrapper's. Fails against the old order (source while `ZDOTDIR` still
    /// points at the wrapper). Ignored: it needs a real zsh.
    #[test]
    #[ignore]
    fn a_real_zsh_zshenv_sees_the_users_zdotdir() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let home = std::env::temp_dir().join(format!(
            "aui-l3-zshenv-{}-{stamp}-{}",
            std::process::id(),
            scratch_counter(),
        ));
        std::fs::create_dir_all(&home).expect("home dir");
        std::fs::write(home.join(".zshenv"), "source \"$ZDOTDIR/exports.zsh\"\n").expect("user file");
        std::fs::write(home.join("exports.zsh"), "export AUI_ZENV_SENTINEL=zshenv-ok\n")
            .expect("user file");
        let home_str = home.to_string_lossy().into_owned();
        let dir = Pty::write_zdotdir_for(
            "zshenv-nonce",
            &home_str,
            &format!("export ZDOTDIR={}", sh_quote(&home_str)),
        )
        .expect("the wrapper writes");
        let out = run_zsh(&home, dir.path(), &["-c", "echo SENTINEL=${AUI_ZENV_SENTINEL-unset}"]);
        assert!(!out.contains("no such file"), "the user file sources cleanly:\n{out}");
        assert!(out.contains("SENTINEL=zshenv-ok"), "the USER's exports.zsh loads:\n{out}");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A user precmd that binds its own `zle-line-init` AFTER aui's must not
    /// destroy the `B` marker: the next precmd re-asserts the chain in front
    /// of theirs, and their widget still runs. Fails against the install-once
    /// guard (no `B` at all after the overwrite). Ignored: needs a real zsh.
    #[test]
    #[ignore]
    fn a_real_zsh_reasserts_the_zle_chain_over_a_late_user_widget() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let (home, dir, _) = scratch_home(
            &[(
                ".zshrc",
                "zle-line-init() { touch \"$HOME/late-widget-ran\"; }\n\
                 install_late_widget() { zle -N zle-line-init; }\n\
                 precmd_functions+=(install_late_widget)\n",
            )],
            "late-nonce",
            "unset ZDOTDIR",
            "latezle",
        );
        // Two prompt cycles with the user's installer running between them,
        // the way every real prompt cycle runs.
        let out = run_zsh(
            &home,
            dir.path(),
            &[
                "-i",
                "-c",
                "[[ -f $HOME/.zshrc ]] && source $HOME/.zshrc; \
                 _aui_osc133_precmd >/dev/null; install_late_widget; _aui_osc133_precmd >/dev/null; \
                 echo BOUND=$widgets[zle-line-init]; _aui_osc133_zle_line_init",
            ],
        );
        assert!(
            out.contains("BOUND=user:_aui_osc133_zle_line_init"),
            "ours is re-asserted over the later user install:\n{out}"
        );
        assert!(
            out.contains("\x1b]133;B;k=late-nonce\x07"),
            "the rebound widget still emits B:\n{out:?}"
        );
        assert!(home.join("late-widget-ran").exists(), "the user's own widget still runs:\n{out}");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A nested line editor inside a running command emits no `B`: only the
    /// prompt's own editor start (between `A` and `C`) does. Fails against
    /// the unconditional widget. Ignored: it needs a real zsh.
    #[test]
    #[ignore]
    fn a_real_zsh_emits_no_b_from_a_nested_line_editor() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let (home, dir, _) = scratch_home(&[], "vared-nonce", "unset ZDOTDIR", "vared");
        // A prompt cycle (A, B), then a command (C) with two nested editor
        // starts (vared, recursive-edit): exactly one B must come out.
        let out = run_zsh(
            &home,
            dir.path(),
            &[
                "-i",
                "-c",
                "_aui_osc133_precmd >/dev/null; _aui_osc133_zle_line_init; \
                 _aui_osc133_preexec >/dev/null; _aui_osc133_zle_line_init; \
                 _aui_osc133_zle_line_init; echo DONE",
            ],
        );
        assert!(out.contains("DONE"), "the script ran:\n{out:?}");
        assert_eq!(
            out.matches("\x1b]133;B;k=vared-nonce\x07").count(),
            1,
            "one B for the prompt, none for the nested editors:\n{out:?}"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// A `.bash_profile` that sources `.bashrc` itself runs `.bashrc`
    /// exactly once (fallback, not addition), and a user `DEBUG` trap
    /// installed in `.bashrc` still fires. Fails against the old template
    /// (count 2, user trap clobbered). Ignored: it needs a real bash.
    #[test]
    #[ignore]
    fn a_real_bash_sources_bashrc_once_and_chains_the_debug_trap() {
        let bash = "/bin/bash";
        if !Path::new(bash).exists() {
            eprintln!("skipped: no {bash}");
            return;
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let home = std::env::temp_dir().join(format!(
            "aui-l3-bash-{}-{stamp}-{}",
            std::process::id(),
            scratch_counter(),
        ));
        std::fs::create_dir_all(&home).expect("home dir");
        std::fs::write(home.join(".bash_profile"), ". \"$HOME/.bashrc\"\n").expect("user file");
        std::fs::write(
            home.join(".bashrc"),
            "echo bashrc-ran >> \"$HOME/bashrc.count\"\n\
             trap 'echo user-debug-fired >> \"$HOME/debug.log\"' DEBUG\n",
        )
        .expect("user file");
        let dir = Pty::write_bash_rc("bashfix3").expect("the rcfile writes");
        let out = run_bash(&home, &dir.path().join(BASH_RC_NAME), &["-i", "-c", "echo HELLO"]);
        assert!(out.contains("HELLO"), "the command ran:\n{out}");
        let count = std::fs::read_to_string(home.join("bashrc.count")).unwrap_or_default();
        assert_eq!(
            count.lines().filter(|l| *l == "bashrc-ran").count(),
            1,
            ".bashrc runs exactly once:\n{out}"
        );
        let debug = std::fs::read_to_string(home.join("debug.log")).unwrap_or_default();
        assert!(
            debug.contains("user-debug-fired"),
            "the user's DEBUG trap still fires:\n{out}"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// A user `.zshrc` calling `emulate sh` cannot strand the restore: it
    /// already ran in `.zshenv`, so `ZDOTDIR` is still correct. Ignored: it
    /// needs a real zsh and does not run in the default gate.
    #[test]
    #[ignore]
    fn a_real_zsh_survives_emulate_sh_in_zshrc() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let (home, dir, _) =
            scratch_home(&[(".zshrc", "emulate sh\n")], "abc123XYZ", "unset ZDOTDIR", "emulate");
        let out =
            run_zsh(&home, dir.path(), &["-i", "-c", "echo ZDOTDIR=${ZDOTDIR-unset}"]);
        assert!(out.contains("ZDOTDIR=unset"), "ZDOTDIR still correct:\n{out}");
        std::fs::remove_dir_all(&home).ok();
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
        assert_eq!(pty.nonce(), config.nonce(), "the session carries the config nonce");
        let mut parser = BlockParser::new();
        let prompt_end = format!("\x1b]133;B;k={}\x07", config.nonce());
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
