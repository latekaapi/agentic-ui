# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## 0.1.0 — unreleased

### Added

- Sidebar session rows gained an explicit two-line byline: `Byline::TwoLines`
  (a status line over an ink-3 context line), `Byline::Preview`, or
  `Byline::Placeholder`, so every row keeps the same height whether or not it
  has anything to show. Rows report hover enter/leave with their session id
  and measured bounds (`on_hover`, `on_hover_bounds`) so the caller can arm
  and seat a hover card.
- A sidebar hover detail card (`session_detail`, 320 px): the full title with
  relative time, the user's last message quoted, the latest reply's first
  line in the state colour, an attention box for a pending question or
  approval, one meta row, and a project/workspace footer. It opens after
  `SESSION_DETAIL_DELAY` of hover and closes on leave, scroll or click.
- The current project row marks its rolled-up agent state with a 2 px accent
  bar down its left edge (`ProjectGroupRow::current_bar`).
- Transcript turns carry timestamps with how-long-ago captions (`5m ago`, via
  `format_age`), and the turn copy buttons morph to a success check for 1.2 s
  (`COPY_HOLD`) on the caller's flag.
- Transcript text selection spans blocks: markdown reports a
  `MessageSelection` through `span_selection` / `on_selection_change`, and
  `message_selected_text` extracts the covered text for copy.

- Workspace scaffold, `aui-tokens` (colour, space, radius, type, shadow and
  motion tokens; light and dark themes generated for the `gpui-kit` theme
  registry, bundled Geist, CSS-blur shadow semantics) and the `aui-motion`
  animation engine (`Transition`, `Presence`, `Collapse`, `Stagger`,
  `Shimmer`, `IconMorph`, `StreamReveal`).
- `aui-icons`, a Lucide-style icon set plus provider marks, file-type icons
  and status glyphs.
- `aui-protocol`, a transport-agnostic session model (`Session`, `Turn`,
  `Block`, streaming `Delta`s and the `Intent`s the UI emits back) with no
  I/O and no gpui dependency.
- The `aui` component library, built card-by-card at pixel parity with the
  `design/` reference renders: data primitives; shell and sidebar (rail,
  traffic lights, collapsed mode); command palette and toast/banner overlays;
  transcript (turns, prose, tool cards, approvals, questions/plan/todo, code
  and diff blocks, summary/error/status, thinking blocks, activity groups);
  composer (floating and docked variants, plus menu, queue rows, suggestion
  chips, slash/mention menus, attachments); and workbench (terminal, browser
  and annotator, diff review, git/PR, file tree and document pane, sheet and
  PDF panes, sources and citations).
- `aui-gallery`, the storybook app rendering every component in every state,
  a motion playground, a theme switcher, and an assistant mock
  (`screens/assistant`: Main, Sources and Sheet) assembled live from the
  component library, plus a `screens` page showing them side by side.
- The parity loop: `scripts/parity.sh`, gallery screenshot flags
  (`--screenshot`, `--screenshot-window`, `--screenshot-delay`, `--theme`,
  `--entry`, `--list`, `--text-scale`), and the checklist in
  `docs/03-parity-process.md` tracking every card's differing-pixel fraction
  against its design reference.
- Live behaviour in the assistant mock: composer send producing a user turn
  and a streaming assistant turn, approvals and questions resolving,
  right-pane open/close on the layout spring, the sidebar rail toggle
  (⌘B), the command palette (⌘K) with focus restore, live toasts, and the
  view menu switching grouping; keyboard support (Y/A/N on approvals,
  ↑↓/↩/esc in palette and menus, tab stops and an accent focus ring on
  buttons) driven by `aui::keys` actions and the `AUI_GALLERY_STEPS`
  scripting grammar.
- `docs/06-api.md`, a per-module public API overview generated from
  rustdoc, and unit tests for the pure helpers (prose parser, ANSI parser,
  syntax lexer, duration formatting, file-type classification).
- A segmented-control sliding thumb (`tab_strip`) built on a measured layout
  pass, a sized `nav::chevron`, and tree-sitter syntax highlighting behind a
  `tree-sitter` feature flag.
- `aui-webview`: a `WebBackend` trait, a scripted `FakeWebBackend` for the
  gallery, and a `wry`/WKWebView backend behind the `wry` feature, with a
  JS bridge, tabs, nav, an element annotator and screenshot-to-chat.
- `aui-terminal`: a `TerminalBackend` trait, an OSC 133 `BlockParser`, a
  scripted `FakePty`, a `portable-pty`-backed real terminal behind the `pty`
  feature, and an `alacritty_terminal`-backed TUI pane behind the `tui`
  feature, exposed as `workbench/webview`, `workbench/terminal-live` and
  `workbench/tui-live` gallery entries. Documented in `docs/07-backends.md`.

- `block_terminal` scrolls and can track a `ScrollHandle`; the live terminal
  view follows the tail of a real session.

### Changed

- Product text scale defaulted to 1.1 (the design grid itself stays 13 px);
  parity renders continue to compare at scale 1.0.
- Popovers, fanned stacks and anything overflowing its box moved onto
  `aui::overlay::popover_layer` so they paint over later siblings correctly.
- Hover tints changed to fade alpha only via `aui_motion::tint_fade`, rather
  than tweening a colour toward transparent.
- Note cards unified to a single plain hairline box (no accent rail or
  dashed variant), with the corresponding design sources and references
  (cards 10/37/52) re-rendered.
- Design fixes carried back into the reference renders: a continuous first
  divider in the shell and screens, a class-collision fix in card 38, the
  card 50 tab indicator moved to ink/bottom, a notes-column line number on
  card 52, and PR description block flow on card 53.
