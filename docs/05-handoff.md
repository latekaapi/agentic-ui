# Handoff prompt for the next implementation session

Paste the block below into a new Claude Code session opened in this repository.

```
Continue the implementation of the Agentic UI library in /Users/latekaapi/Projects/agentic-ui. Phases 1 and 2 are done and committed (workspace, aui-tokens with the generated gpui-kit theme and bundled Geist, aui-icons, aui-protocol, aui-motion, aui-gallery with foundation cards 01–05 at parity). Start phase 3.

Read first, in this order: your memory files (especially gpui-pre-api-notes and feedback-design-taste), docs/04-design-rules.md, docs/02-component-spec.md sections 1 and 2, docs/03-parity-process.md (tooling, known gaps, checklist), README.md, crates/aui-tokens/src/{lib,theme,styled}.rs, crates/aui-motion/src/lib.rs, crates/aui-gallery/src/{registry,gallery}.rs and cards/motion.rs as the reference for how a card is ported.

The design is the contract: design/src/cards/*.html + design/src/base.css + design/tokens/tokens.css hold exact values; design/reference/{cards,screens}/*.png are what to match; docs/02 is corrected when the CSS disagrees; references are re-rendered with headless Chrome after any design change (command in docs/03). Screen canvases: https://claude.ai/code/artifact/e77554a4-ff0f-4eb6-ae2f-a838de61d384 (harness), https://claude.ai/code/artifact/bdc61083-243f-4dce-8b4f-9e65ff4dce7a (assistant).

Stack: Rust, gpui-pre 0.3.3 as `gpui` + gpui-kit 0.6 (registry sources under ~/.cargo/registry/src/index.crates.io-*/), macOS only. Prefix shell commands with `export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"`. Tools: `cargo run -p aui-gallery` (flags: --theme, --entry, --list, --screenshot <id> <png>, --screenshot-window <png>, --screenshot-delay <ms>, --text-scale), `scripts/parity.sh <entry-id> <reference-stem>` prints differing pixels (≤2 % is parity for a static card), ImageMagick `compare`/`magick` are installed.

Conventions already decided: no literal colours/sizes/durations in components — read `cx.aui().colors`, `aui_tokens::scale::*`, `cx.aui().metrics`, and use `AuiStyled::{text_role, ui, mono, text_px}` for text so the product text scale (1.1 default; parity screenshots run at 1.0) applies; motion only through aui-motion (spring/tween/presence/collapse/stagger/shimmer/icon_morph/pulse/shake/check_draw/looping); cards follow the active theme and must be correct in light and dark; components take data in (aui-protocol types) and emit intents out, no I/O. gpui has no letter-spacing and no element transform (scale = size change, translate = relative offset) — see the known-gap list in docs/03.

Phase 3 deliverables, in order:
1. `aui::data` primitives the cards share: Button (secondary filled+bordered, primary accent, ghost, danger outline; sm/xs/icon variants, press spring), Chip, Pill (status variants), Tag, StatusDot (+pulse), Kbd, Avatar, Glyph (ok/err/spinner), UsageMeter. Gallery entries are not required for these alone; they are exercised by the cards.
2. `aui::shell`: AppShell (three header cells 44 px with continuous dividers, sidebar 252 | centre | right 400, right-pane toggle on the layout spring, docked composer placeholder), PanelHeader, TabStrip with the sliding ink indicator on the swap spring, DropZones. Gallery entries shell/app-shell (card 10, 1280×820 — body padding 12 in that card) and shell/panel-chrome (card 11).
3. `aui::nav`: SessionRow in every state (card 20), Sidebar + Rail (card 21), RoleSections for the assistant (card 22), SidebarView status/project/date + ViewMenu (card 23). Gallery entries sidebar/rows, sidebar/sidebar, sidebar/assistant, sidebar/views.
4. `aui::overlay::CommandPalette` (card 12) and `aui::feedback::{ToastStack, Banner}` (card 13).
Show me the gallery (window capture, both themes) as soon as the app shell and sidebar run, before moving to cards 12/13. Then continue with transcript + composer (spec §3–4), workbench (§5), and the assistant mock inside the gallery binary (no mocked harness app).

Rules: follow the parity loop for every card (build, gallery entry at the card's size, scripts/parity.sh, compare with the reference, exercise every interaction and motion in the spec, tick docs/03 with the commit hash). Keep architecture, the motion engine, tricky gpui elements and all reviews on Fable; delegate routine card ports, sample data, tests and docs to Opus 5 subagents (give them the exact conventions above and forbid editing files outside their scope; run only `cargo build -p aui-gallery` / `cargo test -p <crate>`) and review their output before it lands. Do not touch or wire into my existing assistant app. Commit at the end of each phase with a clear message.
```

The session's persistent memory lives outside the repo at
`~/.claude/projects/-Users-latekaapi-Projects-agentic-ui/memory/`; `docs/memory-snapshot/` is a copy taken at handoff time.
