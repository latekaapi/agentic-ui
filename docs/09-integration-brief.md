# Integrating `aui` into an app (brief for a Codex session)

Paste the block below into the Codex session opened in the app's repository
(Cockpit). It assumes the app already builds on gpui-pre 0.3.x + gpui-kit 0.6,
the line this library is pinned to.

```
Integrate the Agentic UI library at /Users/latekaapi/Projects/agentic-ui into
this app, one pane at a time, without rewriting the app's own state or I/O.

Read first, in this order: agentic-ui/docs/08-getting-started.md,
agentic-ui/crates/aui/examples/minimal.rs (the template for a consumer),
agentic-ui/docs/06-api.md (public API per module), agentic-ui/docs/07-backends.md
(browser and terminal panes), agentic-ui/crates/aui-gallery/src/assistant/view.rs
(how every component composes into the assistant screen, and how intents flow
back). Do not read docs/02 or docs/03; they are the design contract and the
parity process, not the API.

Rules of the library you must respect:
- `aui` components are stateless `RenderOnce` values: data in, intents out via
  closures. They never do I/O. The app owns state (gpui `Entity`s), builds a
  `Vec` of blocks/rows each frame, and handles `Intent`s.
- No literal colours, sizes or durations in app UI code: use `cx.aui().colors`,
  `aui_tokens::scale::*`, `AuiStyled` and `aui_motion`. Both themes must work.
- Anything that overflows its box goes through `aui::overlay::popover_layer`.
- Do not modify agentic-ui from this session. If a component lacks an option
  you need, write it down in a list of library change requests instead.

Steps:
1. Dependencies. Add path dependencies on `aui`, `aui-protocol` (and
   `aui-webview` / `aui-terminal` if those panes are wanted; features `wry`,
   `pty`, `tui` for the real backends). Confirm `cargo tree -d` shows ONE
   `gpui-pre` and ONE `gpui-kit`: if the app pins a different patch version,
   align the app to the library's (gpui-pre 0.3.3 / gpui-kit 0.6.0), never the
   other way.
2. Boot. Replace the app's own `gpui_kit::init` + theme + font setup with
   `gpui_kit::application().with_assets(aui::assets::AuiAssets)` and
   `aui::init(theme, cx)` as in minimal.rs, then
   `AuiTheme::set_text_scale(scale::TEXT_SCALE, None, cx)`. Keep the app's
   existing windows and views; only the init order changes. Build, run, check
   icons and fonts render (blank icon boxes = assets wired wrong).
3. Shell. Swap the app's three-pane layout for `aui::shell::app_shell` with the
   app's existing sidebar/centre/right views as children. Wire ⌘B (rail) and the
   right-pane toggle to the app's state through `aui::keys` actions.
4. Sidebar. Map the app's role > project > session tree onto
   `aui::nav::{RoleSections, SidebarView, ViewMenu}`; selection and grouping
   changes come back as intents.
5. Transcript. Convert the app's message model to `aui_protocol::Session`
   (`Turn`, `Block`, streaming `Delta`s) in an adapter module; render with
   `aui::transcript::*` and `aui_motion::stream_reveal` exactly as
   minimal.rs does. Approvals/questions resolve through `Intent`s.
6. Composer. `aui::composer::docked_composer` with the app's send/attach
   handlers; slash and mention menus take the app's command list as data.
7. Right pane. Document panes (`aui::workbench::docs`), browser
   (`aui_webview::webview_pane`), terminal (`aui_terminal::block_terminal_view`).
   The wry webview is a native overlay: call `WebviewState::set_obscured(true)`
   while any palette/popover covers it.
8. After each step: `cargo build`, run the app, screenshot light and dark, and
   compare against agentic-ui/design/reference/screens/assistant-*.png. Commit
   per step.

Report: which panes are on `aui`, what still uses the old code, and the list of
library change requests with the exact option or component you were missing.
```

## Caveats before starting

- **Version lock.** Both sides must resolve to the same `gpui-pre` and
  `gpui-kit`; two copies of gpui in one binary will not link cleanly and the
  theme globals will not be shared.
- **Path dependencies for now.** The crates are not published; use `path = …`
  or a git dependency on this repository. Publishing to crates.io needs the
  bundled Geist fonts and the design assets checked for size first.
- **The library is not a framework.** It draws; the app owns state, threads,
  the agent connection and file I/O. The adapter from the app's message model to
  `aui_protocol::Session` is the one piece of real work in the integration.
