# Backends: `aui-webview` and `aui-terminal`

`crates/aui` does no I/O and never will. Components take data in and hand
intents out; anything that talks to a process or a native view lives in one of
these two crates, and the gallery drives them through a **scripted fake** so
every screen can be rendered, screenshotted and reviewed without a real shell
or a real browser.

Both crates have the same four layers:

1. a **backend trait** — the narrow surface the UI needs;
2. a **fake** implementing it from a script, which is what the gallery runs;
3. a **real** implementation behind a Cargo feature, which must compile but
   need not be demoed;
4. **view functions** wrapping the existing `aui::workbench` components, each
   holding a gpui `Entity` of state with a poll timer on it.

Features are off by default. Both configurations must stay clean:

```bash
cargo build --workspace
cargo build --workspace --features aui-webview/wry,aui-terminal/pty,aui-terminal/tui
cargo test -p aui-terminal
```

---

## `aui-webview`

| module | what it is |
|---|---|
| `backend` | `WebBackend` (`navigate`, `back`, `forward`, `reload`, `eval`, `set_annotate`, `poll_events`) and `WebEvent::{Title, Url, Loading, Annotation, Screenshot}`; `ElementInfo` carries the selector, box, source and trimmed outer HTML behind each pin |
| `page` | the "Simple pricing" mock card 51 is drawn over, moved out of the gallery so the card and the live pane paint one document. `page()` is card 51's frozen picture; `page_body()` is the document alone; `fake_elements()` exposes the element boxes as data |
| `fake` | `FakeWebBackend`: navigation history, loading/url/title events, and annotations produced by hit-testing pointer positions against `fake_elements()` |
| `wry_backend` (feature `wry`) | a real child WKWebView |
| `view` | `WebviewState` + `webview_pane`, and `WebviewIntent::SendAnnotations { annotations, screenshot, url }` |

Gallery entry: `workbench/webview` (980×640, dark).

**The annotator bridge.** `wry_backend::ANNOTATOR_JS` is injected with
`with_initialization_script`. In annotate mode it outlines the hovered element
in the page itself and, on click, posts
`{selector, label, rect, outerHTML}` — outer HTML trimmed to 2 KB — through
`window.ipc.postMessage`. `with_ipc_handler` parses that with serde_json into a
`WebEvent::Annotation` plus its `ElementInfo`, queued on a mutex the poll drains.

**Limitations.**

- *The native-overlay caveat.* A wry child view is composited **above** the gpui
  scene. gpui cannot paint over it, so every popover, menu, tooltip and note
  bubble over the page is hidden; overlays must sit outside the webview's bounds
  or be drawn inside the page through the bridge. This does not apply to the
  fake, whose page is gpui elements — which is why the gallery card can show a
  note popover over the page at all.
- *Screenshots are not implemented* on either backend. wry 0.55 exposes no
  capture API; WKWebView `takeSnapshot` would need `objc2`/`objc2-web-kit`,
  which are only transitive dependencies here, so no manifest change was made.
  `WebEvent::Screenshot` exists and is plumbed through to `SendAnnotations`, but
  nothing emits it yet.
- wry 0.55 exposes no history API, so `back`/`forward` go through
  `history.back()` with a counted depth rather than the real back-forward list.

---

## `aui-terminal`

| module | what it is |
|---|---|
| `backend` | `TerminalBackend` (`spawn`, `write`, `resize`, `poll`) and `TermEvent::{Output, Exit}` |
| `parser` | `BlockParser`, a `vte::Perform` splitting the byte stream into `aui::workbench::TermBlock`s on OSC 133 markers |
| `fake` | `FakePty`, which replays card 50's recorded session as raw bytes — markers and SGR colour, no `TermBlock` built by hand |
| `pty` (feature `pty`) | a real login shell over `portable-pty` 0.8 |
| `tui_grid` (feature `tui`) | `alacritty_terminal` 0.26 as the grid model; cells become `gpui::TextRun`s on `Palette::ansi16()` |
| `view` | `TerminalState`, `block_terminal_view`, `tui_view`, `TerminalIntent` |

Gallery entries: `workbench/terminal-live` (980×720, dark) and
`workbench/tui-live` (980×560, dark).

**Block boundaries.** The only way to know where one command ends and the next
begins is to ask the shell, so the parser reads OSC 133 shell integration:

| marker | meaning |
|---|---|
| `OSC 133 ; A ST` | prompt start — the previous block ends, the next block's clock starts |
| `OSC 133 ; B ST` | prompt drawn; the command line follows |
| `OSC 133 ; C ST` | the command is running; output follows |
| `OSC 133 ; D ; <exit> ST` | finished — exit 0 is `Done`, anything else `Failed` |

Durations come from the A→D timestamps (`6.2 s`, `3.2 s · exit 1`, live `12 s`).
A shell with no integration emits no markers, which is not an error: the whole
stream becomes one running block, and that is the path the TUI pane uses.

**The zsh snippet** (`aui_terminal::ZSH_INTEGRATION`, written into a throwaway
`ZDOTDIR` whose `.zshrc` sources the user's own first — read, never written):

```zsh
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
```

**Limitations.**

- The parser models a stream, not a screen: no cursor, no scroll region, no
  in-place redraw. Full-screen programs belong in `tui_grid`.
- SGR escapes are kept verbatim so `aui::transcript::parse_ansi` can colour the
  line; **other escapes are dropped**, because `parse_ansi` swallows everything
  up to the next `m` and a stray `ESC[2K` would eat the rest of the line.
- `TuiGrid` snapshots cells and colours only — cursor and selection are not
  modelled yet.
- The `pty` backend compiles but has not been spawned against a real shell.
