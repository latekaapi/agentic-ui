# Backends: `aui-webview` and `aui-terminal`

`crates/aui` does no I/O and never will. Components take data in and hand
intents out; anything that talks to a process or a native view lives in one of
these two crates, and the gallery drives them through a **scripted fake** so
every screen can be rendered, screenshotted and reviewed without a real shell
or a real browser.

Both crates have the same four layers:

1. a **backend trait** — the narrow surface the UI needs;
2. a **fake** implementing it from a script, which is what the gallery's
   original entries run;
3. a **real** implementation behind a Cargo feature — since Phase 5A these are
   demoed, not merely compiled;
4. **view functions** wrapping the existing `aui::workbench` components, each
   holding a gpui `Entity` of state with a poll timer on it.

## Running the real thing

`aui-gallery` has features `wry`, `pty` and `tui` that forward to the backend
crates. They are off by default, so `cargo run -p aui-gallery` still needs no
browser and no shell. Turning one on **adds** an entry; the scripted entries are
untouched and render byte-identically either way.

| feature | entry | what it runs |
|---|---|---|
| `wry` | `workbench/webview-real` | a real WKWebView on `https://example.com` |
| `pty` | `workbench/terminal-real` | your `$SHELL` (a login zsh) in a real pty |
| `pty` + `tui` | `workbench/tui-real` | `top` in that pty, through the alacritty grid |

```bash
cargo run -p aui-gallery --features wry,pty,tui -- --entry workbench/webview-real
cargo run -p aui-gallery --features wry,pty,tui -- --entry workbench/terminal-real
cargo run -p aui-gallery --features wry,pty,tui -- --entry workbench/tui-real
```

**`--screenshot` cannot capture the real browser.** It goes through gpui's
`render_to_image`, which renders the gpui scene; the WKWebView is a native view
composited *over* that scene and is simply not in it. Capture the window
instead:

```bash
screencapture -x -o -l "$WID" out.png
```

The window id is a `CGWindowID`, which only `CGWindowListCopyWindowInfo` will
tell you — `osascript` reports a window's position and size but not its id. With
pyobjc installed, `python3 -c "import Quartz; ..."` is the one-liner; without
it, a dozen lines of swift over the same call, filtered on
`kCGWindowOwnerName == "aui-gallery"`, does the job.

Every configuration must stay clean:

```bash
cargo build --workspace
cargo build --workspace --features aui-webview/wry,aui-terminal/pty,aui-terminal/tui
cargo clippy -p aui-webview -p aui-terminal --all-features --all-targets
cargo test -p aui-terminal -p aui-webview --all-features
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

Gallery entries: `workbench/webview` (the fake) and, with `--features wry`,
`workbench/webview-real` — both 980×640, dark.

**What a native page needs from its host.** Four defaulted trait methods, so the
fake is unchanged by all of this:

| method | what the pane does with it |
|---|---|
| `is_native` | `true` for wry: the pane then stops drawing a document of its own and starts doing the three things below |
| `set_bounds` | pushed from the page area's prepaint, so the webview follows the pane when the window resizes, when the annotations panel opens, and when the stage scrolls |
| `set_visible` | how `WebviewState::set_obscured` hides the view, and how a card the gallery has switched away from stops painting over its replacement |
| `set_focused` | `⌘L` hands first responder back to gpui, or the URL field would never see a keystroke |

**What runs for real** (verified against `https://example.com`): the page loads
into the pane at the right box; `Url`, `Title` and `Loading` events arrive from
wry's page-load and title handlers; back, forward and reload work; `⌘L` types a
URL with address-bar select-all-on-focus semantics and `normalize_url` turns
what was typed into a URL (scheme kept, bare host given `https://`, a phrase
made a search); annotate mode outlines the element under the pointer *in the
page* and a click posts a real `{selector, label, rect, outerHTML}` back over
`window.ipc.postMessage`, which becomes a numbered pin in the panel; and
`SendAnnotations` carries a real PNG.

**The annotator bridge.** `wry_backend::ANNOTATOR_JS` is injected with
`with_initialization_script`. In annotate mode it outlines the hovered element
in the page itself and, on click, posts
`{selector, label, rect, outerHTML}` — outer HTML trimmed to 2 KB — through
`window.ipc.postMessage`. `with_ipc_handler` parses that with serde_json into a
`WebEvent::Annotation` plus its `ElementInfo`, queued on a mutex the poll drains.

**Screenshots.** `WebBackend::capture` is implemented for the real backend.
`wry::WebViewExtMacOS::webview` hands back a `Retained<WryWebView>`, which
derefs to `WKWebView`; the backend calls
`takeSnapshotWithConfiguration:completionHandler:` with a null configuration
(the visible viewport) and encodes the `NSImage` the completion block delivers
as PNG through `NSImage → TIFFRepresentation → NSBitmapImageRep →
representationUsingType:` — `NSImage` has no PNG encoder of its own. The bytes
land on the same queue the poll drains, as `WebEvent::Screenshot`.

The `objc2` crates are **pinned to the versions wry 0.55 already resolves to**
(objc2 0.6, block2 0.6, objc2-foundation / objc2-app-kit / objc2-web-kit 0.3;
check with `cargo tree -i objc2`). That is load-bearing, not tidiness: the
handle wry returns is an objc2 0.6 type, and a different objc2 major would make
it a different, unrelated Rust type that none of these methods exist on.

The pane asks for a capture when a load finishes and whenever the page area
changes size, so the stand-in an overlay is painted over is a picture of the
page at its current box. A capture is taken while the view is *visible*:
`takeSnapshot` on a hidden `WKWebView` comes back blank.

**The native-overlay caveat, and what to do about it.** A wry child view is
composited **above** the gpui scene. gpui cannot paint over it, so every
popover, menu, tooltip and note bubble over the page is hidden. This is not
fixable by compositing — the only way to put gpui pixels in that rectangle is to
take the native view out of it, which is what `WebviewState::set_obscured(true)`
does: it hides the view and paints the last screenshot in its place (a flat
surface if there is none yet). The gallery card wires it to a ⌘K command
palette. Call it for any overlay that overlaps the pane; call it with `false`
again when the overlay closes.

None of this applies to the fake, whose page is gpui elements — which is why the
scripted card can show a note popover over the page at all.

**Remaining limitations.**

- *gpui's clipping does not reach the native view.* A child `NSView` is clipped
  by the window's content view and by nothing else, so a pane inside a scrolling
  container would paint its page over the app's own chrome. The pane compensates
  by intersecting the page area with `window.content_mask()` and giving the
  backend that. The intersection crops the **frame**, so a page whose top edge is
  cut reflows into the shorter box rather than scrolling under the clip; a real
  fix needs a clip container `NSView` between the window and the webview.
- *The gpui overlays over the page are still invisible* unless the pane is
  obscured: the hover outline is drawn inside the page by `ANNOTATOR_JS`, but the
  numbered pins, the selected-element outline and the note popover are gpui
  elements and only appear once `set_obscured(true)` has hidden the view.
- wry 0.55 exposes no history API, so `back`/`forward` go through
  `history.back()` with a counted depth rather than the real back-forward list.
  A page that navigates itself is not counted.
- `ElementInfo::source` is always empty for a real page: nothing maps an element
  back to a source file, because no dev server is wired up.
- The native view eats scroll wheel and keystrokes over its own rectangle, so a
  host cannot scroll a container *through* the page, and only `⌘`-modified keys
  reliably reach gpui while the page has focus.
- `--screenshot` (gpui `render_to_image`) never contains the page; see
  "Running the real thing".

---

## `aui-terminal`

| module | what it is |
|---|---|
| `backend` | `TerminalBackend` (`spawn`, `write`, `resize`, `poll`) and `TermEvent::{Output, Exit}` |
| `parser` | `BlockParser`, a `vte::Perform` splitting the byte stream into `aui::workbench::TermBlock`s on OSC 133 markers |
| `fake` | `FakePty`, which replays card 50's recorded session as raw bytes — markers and SGR colour, no `TermBlock` built by hand |
| `pty` (feature `pty`) | a real login shell over `portable-pty` 0.8; `login_shell()`, `Pty::shutdown` and a `Drop` that kills it |
| `tui_grid` (feature `tui`) | `alacritty_terminal` 0.26 as the grid model; cells become `gpui::TextRun`s on `Palette::ansi16()`, and `TuiGrid::cursor` inverts the cursor cell |
| `view` | `TerminalState` (block mode, or grid mode via `with_grid`), `block_terminal_view`, `tui_view`, `tui_grid_view`, `TerminalIntent` |

Gallery entries: `workbench/terminal-live` and `workbench/tui-live` (the fakes),
plus `workbench/terminal-real` and `workbench/tui-real` with the features on.

**What runs for real.** `workbench/terminal-real` spawns `$SHELL -l` (a login
zsh) in `$HOME` with the OSC 133 wrapper and forwards every keystroke straight
to it — there is no local line editing, the shell's own echo is the line editor,
so history, completion and `⌃C` all behave. Verified: `ls -la` becomes a `Done`
block with the fold row, `false` becomes a `Failed` block reading `exit 1`, and
SGR colour is drawn on the design palette. `workbench/tui-real` runs `top` in
the same kind of pty through the alacritty grid: it redraws in place, resizes
with the pane, and the cursor is visible as an inverted cell.

**Sizing.** Both views measure themselves in a prepaint `canvas`, convert the
box to character cells with the mono font's advance at 12 px from
`window.text_system()` (minus each pane's own padding), and call
`TerminalState::resize` only when the cell count changed. The pty gets a real
`SIGWINCH` and the program redraws.

**Cleanup.** Dropping a `Pty` kills the shell, reaps it, closes the pty, joins
the reader thread and removes the throwaway `ZDOTDIR`. Closing the gallery
window drops the window state and therefore the `Pty`, and leaves no process and
no temp directory behind — verified. A `SIGKILL`/`SIGTERM` of the gallery runs
no destructors, so the temp `ZDOTDIR` survives that; nothing can be done about
it from inside the process.

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
unsetopt PROMPT_SP
```

Three details are the difference between this working and not, on a real
machine:

- **`B` is re-appended from `precmd`, not set once.** starship, powerlevel10k
  and oh-my-zsh themes all rebuild `PS1` from their own `precmd`; a marker
  appended at source time is thrown away on the first prompt. The wrapper
  installs its hook *after* sourcing the user's files, so `add-zsh-hook` puts it
  last and it runs after the theme.
- **`%{…%}`** keeps the escape out of zsh's prompt-width arithmetic; without it
  the line wraps in the wrong place.
- **`unsetopt PROMPT_SP`** stops the reverse-video `%` zsh prints for a command
  whose output had no trailing newline from landing in the block as a junk line.

**Limitations.**

- The parser models a stream, not a screen: no cursor, no scroll region, no
  in-place redraw. Full-screen programs belong in `tui_grid`. It does honour
  backspace in the command phase, because zle echoes the first key, rubs it out
  and redraws — without that a typed `echo hi` arrives as `eecho hi`.
- SGR escapes are kept verbatim so `aui::transcript::parse_ansi` can colour the
  line; **other escapes are dropped**, because `parse_ansi` swallows everything
  up to the next `m` and a stray `ESC[2K` would eat the rest of the line.
- `TuiGrid` carries the cursor's **position** (inverted in place in the
  snapshot's runs) but not its shape or blink, and no selection.
- **`aui::workbench::block_terminal` does not scroll.** Its body is
  `overflow_hidden`, which is right for a design card and wrong for a session
  that outgrows the pane: a long transcript is clipped at the bottom with no way
  to reach it. Fixing that means changing the component in `crates/aui`.
- `tui_grid_view` reproduces `aui::workbench::tui_pane`'s padding, type, input
  box, footer and hint keys rather than reusing it, because the component takes
  `Vec<String>` and the grid needs `TextRun`s. The two panes are drawn from
  duplicated constants and will drift if `tui.rs` changes.
- Only zsh gets shell integration. Any other shell runs unmarked, which is not
  an error: the whole stream becomes one running block.
- The measured cols/rows have been exercised against a real `SIGWINCH` (`top`
  redraws when the pane changes size) but the arithmetic is derived from each
  pane's padding constants by hand, so it is one number's drift away from being
  a cell out.
