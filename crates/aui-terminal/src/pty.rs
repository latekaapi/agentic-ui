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
/// Two `precmd` hooks, deliberately split, bracketing the user's own the way
/// the bash side brackets `PROMPT_COMMAND`:
///
/// 1. `_aui_osc133_status` stays FIRST in `precmd_functions`. It captures
///    `$?` for the `D` marker before any user precmd can clobber it.
/// 2. `_aui_osc133_precmd` keeps itself LAST in `precmd_functions`. On every
///    invocation, if it is not the final element it removes itself and
///    appends itself to the end. `precmd_functions` is an array of names,
///    not a chain of wrappers, so re-appending an already-present name is
///    idempotent and — unlike wrapping the `zle-line-init` widget, which
///    this replaces — cannot form a cycle. (That widget is gone entirely:
///    wrapping it let a user wrapper and our wrapper capture each other and
///    recurse to zsh's nesting limit on every prompt after the first.)
///
/// Running last means every user precmd, including a theme that rebuilds
/// `PS1` from scratch, has already run; the hook then emits `A` and
/// re-attaches the `B` marker to the end of whatever `PS1` resulted. The
/// re-attachment matches the marker literally first, so a static `PS1` does
/// not grow one copy per prompt. When a user hook registered after ours, the
/// first prompt can miss `B`; the hook order converges after one prompt.
///
/// `B` is emitted only while the shell is at its own prompt: the `precmd`
/// that emits `A` sets `_AUI_AT_PROMPT`, `preexec` unsets it when a command
/// starts running, and the `PS1`-embedded marker is conditional on it (which
/// needs `PROMPT_SUBST` below), so a prompt redraw in the middle of a
/// running command stays silent. A redraw at the same prompt (`zle
/// reset-prompt`, e.g. powerlevel10k's transient prompt) can still emit a
/// second `B`; the parser ignores a `B` that arrives while already reading
/// the command line.
///
/// `PROMPT_SP` is switched off as well: it is the reverse-video `%` and the
/// row of padding zsh prints to show a command whose output had no trailing
/// newline, and it would land in the block's output as a line of noise.
const ZSH_TEMPLATE: &str = r#"
# OSC 133 shell integration for the aui block terminal.
autoload -Uz add-zsh-hook

# The prompt-end marker, drawn as part of PS1. %{...%} keeps it out of the
# line editor's width arithmetic; the ${...:+...} conditional (needs
# PROMPT_SUBST below) emits the marker only while the shell is at its own
# prompt. The inner double quotes matter: an unquoted %{ inside ${...+...}
# misparses, leaking a literal brace into the prompt.
_AUI_OSC133_B='${_AUI_AT_PROMPT:+"%{'$'\033]133;B;k=__AUI_NONCE__\007''%}"}'

# Status capture. Stays FIRST in precmd_functions so $? is still the
# command's exit status: a user precmd running before us would clobber it.
_aui_osc133_status() {
  _AUI_STATUS=$?
  if [[ ${precmd_functions[1]:-} != _aui_osc133_status ]]; then
    precmd_functions=(${precmd_functions:#_aui_osc133_status})
    precmd_functions=(_aui_osc133_status "${precmd_functions[@]}")
  fi
  return 0
}

# Block markers. Keeps itself LAST in precmd_functions (see the module docs
# for why an array slot cannot cycle the way widget wrapping did): every
# user precmd has already run, so re-attach B after whatever PS1 they left.
_aui_osc133_precmd() {
  if [[ ${precmd_functions[-1]:-} != _aui_osc133_precmd ]]; then
    precmd_functions=(${precmd_functions:#_aui_osc133_precmd})
    precmd_functions+=(_aui_osc133_precmd)
  fi
  local _aui_status=${_AUI_STATUS:-0}
  if [[ -n ${_AUI_RUNNING-} ]]; then
    printf '\033]133;D;%s;k=__AUI_NONCE__\007' "$_aui_status"
    unset _AUI_RUNNING
  fi
  printf '\033]133;A;k=__AUI_NONCE__\007'
  # At the shell's own prompt now: the prompt about to be drawn owns the B.
  _AUI_AT_PROMPT=1
  # Re-attach the marker to whatever prompt setup just built; the quoted
  # check matches literally, so a static PS1 keeps exactly one copy.
  [[ $PS1 == *"$_AUI_OSC133_B"* ]] || PS1+="$_AUI_OSC133_B"
}

_aui_osc133_preexec() {
  _AUI_RUNNING=1
  # A command is running now, so a prompt redraw before it finishes
  # (vared, recursive-edit) expands to no B, not a new prompt.
  unset _AUI_AT_PROMPT
  printf '\033]133;C;k=__AUI_NONCE__\007'
}

# Deleted before re-adding, so sourcing this twice keeps one registration
# (and the status hook first, the marker hook last).
add-zsh-hook -d precmd _aui_osc133_status 2>/dev/null
add-zsh-hook -d precmd _aui_osc133_precmd 2>/dev/null
add-zsh-hook precmd _aui_osc133_status
add-zsh-hook precmd _aui_osc133_precmd
add-zsh-hook -d preexec _aui_osc133_preexec 2>/dev/null
add-zsh-hook preexec _aui_osc133_preexec

# The B marker above is conditional on parameter expansion in the prompt.
setopt PROMPT_SUBST
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
/// chained from ours (ours first, theirs after). The capture lets the shell
/// do the unquoting via `eval` rather than hand-stripping quotes off
/// `trap -p` output. A trap the user installs after rc time (replacing ours
/// outright) is noticed by a check bracketed into `PROMPT_COMMAND` at the
/// top level — a function body cannot see the DEBUG trap, so the check runs
/// inline — and captured the same way, then ours is reinstalled around it.
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
# clobbering it. `trap -p` prints the body safely quoted
# (`trap -- 'user cmd' DEBUG`); eval'ing the assignment lets the shell do
# the unquoting, which stays correct for bodies containing quotes, `$_` and
# `$?`. Hand-stripping the quotes instead leaves them on, so the shell looks
# for a command literally named `user cmd`. A line that already mentions our
# own preexec is our own chained trap, never the user's.
_AUI_USER_DEBUG_TRAP=""
_aui_osc133_dbg_line=$(trap -p DEBUG 2>/dev/null || true)
if [ -n "$_aui_osc133_dbg_line" ]; then
  case "$_aui_osc133_dbg_line" in
    *"_aui_osc133_preexec"*) ;;
    *)
      _aui_osc133_dbg_rest=${_aui_osc133_dbg_line#trap -- }
      eval "_AUI_USER_DEBUG_TRAP=${_aui_osc133_dbg_rest% DEBUG}"
      ;;
  esac
fi
unset _aui_osc133_dbg_line _aui_osc133_dbg_rest

# The prompt-end marker; \[...\] keeps it out of readline's width arithmetic.
_AUI_OSC133_B=$'\[\033]133;B;k=__AUI_NONCE__\007\]'

# Captures the command's exit status first: this entry runs before the rest of
# PROMPT_COMMAND so `$?` is still the command's.
_aui_osc133_begin() {
  _AUI_STATUS=$?
  _AUI_IN_PROMPT=1
}

# A user trap installed after rc time replaces ours outright. The next
# prompt's top-level check (see below) notices ours is gone and calls this
# with the visible trap line; it captures theirs the same way as above. The
# substring test is against our own function name, so it cannot mistake our
# own chained trap for the user's. Returns 0 when the caller must reinstall
# ours (which only the top level can do: a `trap` run inside a function is
# function-local, and a function body cannot even see the DEBUG trap). This
# function itself never touches the installed trap.
_aui_osc133_rechain() {
  case "$1" in
    *"_aui_osc133_preexec"*) return 1;;
    *)
      if [ -n "$1" ]; then
        _aui_re_rest=${1#trap -- }
        eval "_AUI_USER_DEBUG_TRAP=${_aui_re_rest% DEBUG}"
      else
        _AUI_USER_DEBUG_TRAP=""
      fi
      unset _aui_re_rest
      return 0
      ;;
  esac
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
    _aui_osc133_*|PROMPT_COMMAND*|if\ _aui_osc133_rechain*) return 0;;
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
# so later elements cannot slip past the re-attachment. Between theirs and
# ours sits the late-trap check: it must run inline at the top level (not
# inside a function) because a function body cannot see the DEBUG trap, and
# for the same reason only this top level may reinstall ours.
_aui_osc133_rechain_inline='if _aui_osc133_rechain "$(trap -p DEBUG 2>/dev/null)"; then trap '\''_aui_osc133_preexec'\'' DEBUG; fi'
if declare -p PROMPT_COMMAND 2>/dev/null | grep -q 'declare -a'; then
  PROMPT_COMMAND=(_aui_osc133_begin "${PROMPT_COMMAND[@]}" "$_aui_osc133_rechain_inline" _aui_osc133_precmd)
else
  PROMPT_COMMAND="_aui_osc133_begin${PROMPT_COMMAND:+; $PROMPT_COMMAND}; ${_aui_osc133_rechain_inline}; _aui_osc133_precmd"
fi
unset _aui_osc133_rechain_inline
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
    /// the snippet work on a real machine are still there: a status hook
    /// that stays first in `precmd_functions`, a marker hook that keeps
    /// itself last, `B` re-attached to `PS1` (matched literally first),
    /// gated on the at-prompt flag, `unsetopt PROMPT_SP` — and no
    /// `zle-line-init` widget anywhere.
    #[test]
    fn zsh_snippet_marks_every_boundary_from_hooks_that_reorder_themselves() {
        let snippet = zsh_integration("abc123");
        assert_eq!(snippet.matches("k=abc123").count(), 4, "{snippet}");
        assert!(!snippet.contains("zle-line-init"), "no widget wrapping:\n{snippet}");
        assert!(!snippet.contains("_aui_osc133_install_zle"), "no installer:\n{snippet}");
        assert!(!snippet.contains("_aui_orig_zle_line_init"), "no saved widget:\n{snippet}");
        assert!(snippet.contains("_aui_osc133_status"), "{snippet}");
        assert!(snippet.contains("_aui_osc133_precmd"), "{snippet}");
        assert!(
            snippet.contains("precmd_functions=(_aui_osc133_status"),
            "the status hook stays first:\n{snippet}"
        );
        assert!(
            snippet.contains("precmd_functions+=(_aui_osc133_precmd)"),
            "the marker hook keeps itself last:\n{snippet}"
        );
        assert!(snippet.contains("_AUI_OSC133_B"), "the PS1 marker variable:\n{snippet}");
        assert!(snippet.contains("PS1+="), "the marker is re-attached to PS1:\n{snippet}");
        assert!(snippet.contains("_AUI_AT_PROMPT"), "the B gate:\n{snippet}");
        assert!(snippet.contains("PROMPT_SUBST"), "the gate needs prompt substitution:\n{snippet}");
        assert!(snippet.contains("add-zsh-hook -d precmd _aui_osc133_status"), "{snippet}");
        assert!(snippet.contains("add-zsh-hook precmd _aui_osc133_status"), "{snippet}");
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
    /// markers all carry the nonce. A trap installed after rc time is
    /// re-captured by a check bracketed into `PROMPT_COMMAND`.
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
        assert!(
            body.contains("eval \"_AUI_USER_DEBUG_TRAP=${_aui_osc133_dbg_rest% DEBUG}\""),
            "the shell does the unquoting, not hand-stripped quotes:\n{body}"
        );
        let hooks = body.find("PROMPT_COMMAND=").expect("PROMPT_COMMAND");
        assert!(capture < hooks, "hooks install after the capture:\n{body}");
        assert!(body.contains("declare -p PROMPT_COMMAND"), "array PROMPT_COMMAND:\n{body}");
        assert!(body.contains("_AUI_USER_DEBUG_TRAP"), "DEBUG chaining:\n{body}");
        assert!(body.contains("_aui_osc133_rechain"), "late-trap re-capture:\n{body}");
        assert!(
            body.contains("trap '_aui_osc133_preexec' DEBUG"),
            "our trap installs (rc time and re-chain):\n{body}"
        );
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

    /// A live interactive zsh on a pty: `HOME` at the scratch home (whose
    /// `.zshenv` the wrapper chains) and `ZDOTDIR` at the wrapper, so the
    /// line editor runs for real. Hermetic: only the child sees this
    /// environment, so the process environment (and parallel tests) are
    /// untouched.
    struct LiveZsh {
        child: Box<dyn portable_pty::Child + Send + Sync>,
        writer: Box<dyn Write + Send>,
        rx: Receiver<Vec<u8>>,
        #[allow(dead_code)]
        reader: JoinHandle<()>,
        raw: Vec<u8>,
    }

    impl LiveZsh {
        fn spawn(home: &Path, zdotdir: &Path) -> Self {
            let pair = native_pty_system()
                .openpty(PtySize { rows: 32, cols: 100, pixel_width: 0, pixel_height: 0 })
                .expect("pty opens");
            let mut cmd = CommandBuilder::new("/bin/zsh");
            cmd.arg("-l");
            cmd.arg("-i");
            cmd.env("TERM", "xterm-256color");
            cmd.env("HOME", home);
            cmd.env("ZDOTDIR", zdotdir);
            cmd.cwd(std::env::temp_dir());
            let child = pair.slave.spawn_command(cmd).expect("zsh spawns");
            drop(pair.slave);
            let mut reader = pair.master.try_clone_reader().expect("reader");
            let writer = pair.master.take_writer().expect("writer");
            let (tx, rx) = channel();
            let reader = std::thread::Builder::new()
                .name("aui-test-reader".into())
                .spawn(move || {
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
                })
                .expect("reader thread");
            Self { child, writer, rx, reader, raw: Vec::new() }
        }

        /// Pulls everything pending into `raw`.
        fn drain(&mut self) {
            while let Ok(bytes) = self.rx.try_recv() {
                self.raw.extend_from_slice(&bytes);
            }
        }

        fn count(&self, needle: &[u8]) -> usize {
            self.raw.windows(needle.len()).filter(|w| *w == needle).count()
        }

        /// Sends a command line, then waits until `needle` has appeared
        /// `want` times. Returns false on timeout, so a stuck shell fails
        /// fast instead of stalling the gate one full timeout per command.
        fn send_and_wait(&mut self, line: &str, needle: &[u8], want: usize, timeout: Duration) -> bool {
            self.writer.write_all(line.as_bytes()).expect("pty write");
            self.writer.flush().expect("pty flush");
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                self.drain();
                if self.count(needle) >= want {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            self.drain();
            false
        }

        /// Waits until `needle` has appeared `want` times (or the deadline
        /// passes), for the initial prompt where nothing is sent.
        fn wait_for(&mut self, needle: &[u8], want: usize, timeout: Duration) -> bool {
            self.send_and_wait("", needle, want, timeout)
        }

        fn shutdown(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// Drives `commands` (one per prompt) through a live zsh and returns the
    /// raw pty bytes. Each command is sent only after the next prompt's `B`
    /// has arrived, so no line is lost to a line editor that is not
    /// listening yet.
    fn drive_live_zsh(home: &Path, zdotdir: &Path, nonce: &str, commands: &[&str]) -> Vec<u8> {
        let mut sh = LiveZsh::spawn(home, zdotdir);
        let prompt_end = format!("\x1b]133;B;k={nonce}\x07");
        let needle = prompt_end.as_bytes().to_vec();
        assert!(sh.wait_for(&needle, 1, Duration::from_secs(15)), "the shell never drew its first prompt");
        for (i, cmd) in commands.iter().enumerate() {
            if !sh.send_and_wait(cmd, &needle, i + 2, Duration::from_secs(10)) {
                break;
            }
        }
        // Let the last prompt settle, then take the shell down: killing it
        // cannot lose a marker we already waited for.
        std::thread::sleep(Duration::from_millis(300));
        sh.drain();
        let raw = std::mem::take(&mut sh.raw);
        sh.shutdown();
        raw
    }

    /// Counts one full marker in the raw pty bytes. `mid` is the marker
    /// middle: `A`, `B`, `C`, or `D;0` (`D` carries the exit status, and
    /// every command these tests type exits zero).
    fn count_marker(raw: &[u8], nonce: &str, mid: &str) -> usize {
        let marker = format!("\x1b]133;{mid};k={nonce}\x07");
        raw.windows(marker.len()).filter(|w| *w == marker.as_bytes()).count()
    }

    /// The powerlevel10k / zsh-vi-mode pattern: a user `.zshenv` whose init
    /// precmd wraps `zle-line-init` AFTER our integration installed (their
    /// saved `orig` points at whatever is bound then). There is no widget
    /// left to capture on our side, so the user's wrapper must run exactly
    /// once per prompt — and `A B C D` must appear on every cycle. Fails
    /// against the widget wrapper (the two wrappers capture each other and
    /// the user's runs hundreds of times per prompt). Ignored: it needs a
    /// real interactive zsh.
    #[test]
    #[ignore]
    fn a_real_zsh_p10k_wrap_pattern_runs_once_per_prompt_with_all_markers() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let nonce = "p10knonce1";
        let (home, dir, _) = scratch_home(
            &[(
                ".zshenv",
                "_user_base() { :; }\n\
                 zle -N zle-line-init _user_base\n\
                 _user_wrapper() { echo x >> \"$HOME/wrapper.count\"; zle _user_orig -- \"$@\"; }\n\
                 _p10k_like_install() {\n\
                   zle -A zle-line-init _user_orig 2>/dev/null || return 0\n\
                   zle -N zle-line-init _user_wrapper\n\
                   precmd_functions=(${precmd_functions:#_p10k_like_install})\n\
                 }\n\
                 precmd_functions+=(_p10k_like_install)\n",
            )],
            nonce,
            "unset ZDOTDIR",
            "p10k",
        );
        let cmds = ["echo cmd1\n", "echo cmd2\n", "echo cmd3\n", "echo cmd4\n", "echo cmd5\n"];
        let raw = drive_live_zsh(&home, dir.path(), nonce, &cmds);
        // Six prompts (the initial one plus one per command), six widgets.
        let a = count_marker(&raw, nonce, "A");
        let b = count_marker(&raw, nonce, "B");
        let c = count_marker(&raw, nonce, "C");
        let d = count_marker(&raw, nonce, "D;0");
        assert_eq!(a, 6, "one A per prompt");
        assert_eq!(b, 6, "one B per prompt");
        assert_eq!(c, 5, "one C per typed command");
        assert_eq!(d, 5, "one D per typed command");
        let wrapped = std::fs::read_to_string(home.join("wrapper.count")).unwrap_or_default();
        assert_eq!(wrapped.lines().count(), b, "the user's wrapper runs exactly once per prompt");
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
            .current_dir(home)
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

    /// A user precmd that rebinds `zle-line-init` on EVERY precmd: there is
    /// no widget left to rebind on our side, so `B` must still be emitted on
    /// every cycle. Fails against the widget approach (their rebind wins
    /// every cycle: `A C D A C D`, no `B` ever). Ignored: needs a real
    /// interactive zsh.
    #[test]
    #[ignore]
    fn a_real_zsh_every_precmd_rebind_still_gets_b() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let nonce = "rebindnonce";
        let (home, dir, _) = scratch_home(
            &[(
                ".zshenv",
                "_rebind_widget() { :; }\n\
                 _rebind() { zle -N zle-line-init _rebind_widget; }\n\
                 _install_rebind() {\n\
                   precmd_functions+=(_rebind)\n\
                   precmd_functions=(${precmd_functions:#_install_rebind})\n\
                 }\n\
                 precmd_functions+=(_install_rebind)\n",
            )],
            nonce,
            "unset ZDOTDIR",
            "rebind",
        );
        let cmds = ["echo cmd1\n", "echo cmd2\n", "echo cmd3\n", "echo cmd4\n", "echo cmd5\n"];
        let raw = drive_live_zsh(&home, dir.path(), nonce, &cmds);
        assert_eq!(count_marker(&raw, nonce, "A"), 6, "one A per prompt");
        assert_eq!(count_marker(&raw, nonce, "B"), 6, "one B per prompt, rebind or not");
        assert_eq!(count_marker(&raw, nonce, "C"), 5, "one C per typed command");
        assert_eq!(count_marker(&raw, nonce, "D;0"), 5, "one D per typed command");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A theme that rebuilds `PS1` from scratch in its own precmd,
    /// registered after ours: `B` still arrives, because our marker hook
    /// keeps itself last and re-attaches after the theme ran — and `PS1`
    /// does not grow across prompts, because the re-attachment matches the
    /// marker literally first. The first prompt can miss `B` (the order
    /// converges after one prompt). Fails against the widget approach, whose
    /// `B` never lived in `PS1` at all. Ignored: needs a real zsh.
    #[test]
    #[ignore]
    fn a_real_zsh_theme_rebuilding_ps1_still_gets_b_without_growth() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let nonce = "themenonce1";
        let (home, dir, _) = scratch_home(
            &[(
                ".zshenv",
                "_user_theme() { PS1='t> '; }\n\
                 _install_theme_late() {\n\
                   precmd_functions+=(_user_theme)\n\
                   precmd_functions=(${precmd_functions:#_install_theme_late})\n\
                 }\n\
                 precmd_functions+=(_install_theme_late)\n",
            )],
            nonce,
            "unset ZDOTDIR",
            "theme",
        );
        // Ten commands, two PS1 length probes, thirteen prompts: a B on
        // every prompt once converged (at most one miss, on the cycle where
        // the late theme lands after the marker) and a PS1 that stops
        // growing.
        let mut cmds: Vec<String> =
            (1..=10).map(|i| format!("echo cmd{i}\n")).collect();
        cmds.push("print LEN1=${#PS1}\n".to_string());
        cmds.push("print LEN2=${#PS1}\n".to_string());
        let cmd_refs: Vec<&str> = cmds.iter().map(String::as_str).collect();
        let raw = drive_live_zsh(&home, dir.path(), nonce, &cmd_refs);
        let b = count_marker(&raw, nonce, "B");
        assert_eq!(count_marker(&raw, nonce, "A"), 13, "one A per prompt");
        assert!((12..=13).contains(&b), "a B on every prompt once converged: {b}");
        assert_eq!(count_marker(&raw, nonce, "C"), 12, "one C per typed command");
        assert_eq!(count_marker(&raw, nonce, "D;0"), 12, "one D per typed command");
        let lens = prompt_lengths(&raw);
        assert_eq!(lens.len(), 2, "both length probes reported: {lens:?}");
        assert_eq!(lens[0], lens[1], "PS1 does not grow across prompts: {lens:?}");
        assert!(lens[0] > 10, "PS1 carries the attached marker: {lens:?}");
        std::fs::remove_dir_all(&home).ok();
    }

    /// Reads back `LEN<n>=<digits>` lines the shell printed.
    fn prompt_lengths(raw: &[u8]) -> Vec<usize> {
        let mut out = Vec::new();
        let text = String::from_utf8_lossy(raw);
        for line in text.split(['\n', '\r']) {
            for tag in ["LEN1=", "LEN2="] {
                if let Some(rest) = line.find(tag).map(|i| &line[i + tag.len()..]) {
                    let digits: String = rest.chars().take_while(|c| c.is_numeric()).collect();
                    if let Ok(n) = digits.parse::<usize>() {
                        out.push(n);
                    }
                }
            }
        }
        out
    }

    /// `B` is emitted only while the shell is at its own prompt: after the
    /// precmd a prompt expansion of `PS1` carries `B`, and after the preexec
    /// (a command running: `vared`, `read`, PS2 continuation,
    /// `recursive-edit`) the same expansion carries none. Fails against the
    /// widget approach, which never touches `PS1`. Ignored: needs a real
    /// zsh.
    #[test]
    #[ignore]
    fn a_real_zsh_b_is_silent_while_a_command_runs() {
        let zsh = "/bin/zsh";
        if !Path::new(zsh).exists() {
            eprintln!("skipped: no {zsh}");
            return;
        }
        let (home, dir, _) = scratch_home(&[], "guardnonce", "unset ZDOTDIR", "guard");
        // Expand PS1 the way the prompt does (parameter expansion, then
        // prompt expansion) once at the prompt and once mid-command.
        let out = run_zsh(
            &home,
            dir.path(),
            &[
                "-i",
                "-c",
                "_expand() { local _e=${(e)PS1}; print -r -- \"${(%)_e}\"; }; \
                 _aui_osc133_precmd >/dev/null; _expand; \
                 _aui_osc133_preexec >/dev/null; _expand; echo DONE",
            ],
        );
        assert!(out.contains("DONE"), "the script ran:\n{out:?}");
        assert_eq!(
            out.matches("\x1b]133;B;k=guardnonce\x07").count(),
            1,
            "one B at the prompt, none while the command runs:\n{out:?}"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// Runs the real bash the way `run_bash` does, but with `script` on
    /// stdin, so `PROMPT_COMMAND` (and real prompt cycles) actually run.
    /// Returns the combined stdout/stderr.
    fn run_bash_with_stdin(home: &Path, rcfile: &Path, script: &str) -> String {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("/bin/bash")
            .env("HOME", home)
            .arg("--rcfile")
            .arg(rcfile)
            .arg("-i")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("bash runs");
        child.stdin.as_mut().expect("piped stdin").write_all(script.as_bytes()).expect("stdin write");
        let out = child.wait_with_output().expect("bash finishes");
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        text
    }

    /// A `.bash_profile` that sources `.bashrc` itself runs `.bashrc`
    /// exactly once (fallback, not addition), and a user `DEBUG` trap
    /// installed in `.bashrc` — whose body contains quotes, `$_` and `$?` —
    /// still fires for a TYPED command after the chain is installed, not
    /// just during rc sourcing, with no `command not found`. Fails against
    /// the old template (the hand-stripped quotes leave the body quoted, so
    /// bash looks for a command literally named by the body). Ignored: it
    /// needs a real bash.
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
             trap 'echo \"user-debug:$BASH_COMMAND:$_:$?\" >> debug.log' DEBUG\n",
        )
        .expect("user file");
        let dir = Pty::write_bash_rc("bashfix4").expect("the rcfile writes");
        // `run_bash` starts in the scratch home, so the relative log path
        // lands there — and the relative path keeps the old failure as a
        // `command not found` rather than bash's slashy variant.
        let out =
            run_bash(&home, &dir.path().join(BASH_RC_NAME), &["-i", "-c", "echo TYPED-SENTINEL"]);
        assert!(out.contains("TYPED-SENTINEL"), "the command ran:\n{out}");
        let count = std::fs::read_to_string(home.join("bashrc.count")).unwrap_or_default();
        assert_eq!(
            count.lines().filter(|l| *l == "bashrc-ran").count(),
            1,
            ".bashrc runs exactly once:\n{out}"
        );
        let debug = std::fs::read_to_string(home.join("debug.log")).unwrap_or_default();
        assert!(
            debug.lines().any(|l| l.contains("user-debug:echo TYPED-SENTINEL")),
            "the user's DEBUG trap fires for the TYPED command, not just rc sourcing:\n{debug}\n{out}"
        );
        assert!(
            !out.contains("command not found"),
            "no broken chaining in the session output:\n{out}"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    /// A user `DEBUG` trap installed after rc time — from a setup function
    /// run at the first prompt, replacing ours outright — is noticed at the
    /// next prompt, captured the same way, and re-chained: it fires on the
    /// next command AND our `C` marker is there too. Fails against the old
    /// template (no re-chain: the user's trap runs alone, no `C` ever).
    /// Ignored: it needs a real interactive bash.
    #[test]
    #[ignore]
    fn a_real_bash_rechains_a_late_debug_trap() {
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
            "aui-l3-bashlate-{}-{stamp}-{}",
            std::process::id(),
            scratch_counter(),
        ));
        std::fs::create_dir_all(&home).expect("home dir");
        let dir = Pty::write_bash_rc("bashlate4").expect("the rcfile writes");
        // One line clears ours and installs theirs with no prompt between,
        // so the function-installed trap survives the return; the next
        // prompt must re-chain it.
        let out = run_bash_with_stdin(
            &home,
            &dir.path().join(BASH_RC_NAME),
            "late_installer() { trap 'echo \"late-debug:$BASH_COMMAND\" >> \"$HOME/late.log\"' DEBUG; }\n\
             trap - DEBUG; late_installer\n\
             echo \"P1=[$(trap -p DEBUG)]\"\n\
             echo AFTER-RECHAIN\n",
        );
        assert!(
            out.contains("P1=[trap -- '_aui_osc133_preexec' DEBUG]"),
            "ours is reinstalled at the next prompt:\n{out}"
        );
        assert!(
            out.contains("\x1b]133;C;k=bashlate4\x07"),
            "our preexec runs (the chain is back):\n{out:?}"
        );
        let late = std::fs::read_to_string(home.join("late.log")).unwrap_or_default();
        assert!(
            late.lines().any(|l| l.contains("late-debug:echo AFTER-RECHAIN")),
            "the late trap fires on the next command:\n{late}\n{out}"
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
