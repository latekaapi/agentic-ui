# Handoff prompt for the implementation session

Paste the block below into a new Claude Code session opened in this repository.

```
Start the implementation phase of the Agentic UI library in /Users/latekaapi/Projects/agentic-ui.

Read first, in this order: your memory files, docs/04-design-rules.md, docs/02-component-spec.md, docs/03-parity-process.md, docs/01-research-and-plan.md. The design is the contract: design/src/cards/*.html with design/src/base.css and design/tokens/tokens.css hold the exact values; design/reference/cards and design/reference/screens hold the PNGs each component must match; design/tokens/tokens.json and motion.json are the machine-readable tokens and spring constants. The design system is also live in the Claude Design project "Agentic UI"; the screen canvases are https://claude.ai/code/artifact/e77554a4-ff0f-4eb6-ae2f-a838de61d384 (harness) and https://claude.ai/code/artifact/bdc61083-243f-4dce-8b4f-9e65ff4dce7a (assistant).

Stack: Rust, gpui-pre 0.3.x (crates.io "gpui-pre", renamed to gpui) with gpui-kit 0.6. macOS only. Rust stable via rustup; cargo and Homebrew reach PATH through the SessionStart hook.

Deliverables:
1. Workspace crates with rustdoc: aui-tokens, aui-motion, aui-icons, aui, aui-protocol, aui-webview, aui-terminal, exactly as scoped in docs/02-component-spec.md.
2. aui-gallery: a native macOS app with every design card as a live, interactive gpui component in every state, a sidebar, a theme and density switch, a motion playground, and a --screenshot <entry> <out.png> flag for parity checks.
3. A mocked assistant shell inside the gallery binary (role sections, a session with citations, document panes with sample docx, xlsx and pdf content). No mocked harness app.
4. Build and run instructions, and docs/03-parity-process.md's checklist filled in with commit hashes as components reach parity.

Rules:
- Follow the parity loop in docs/03-parity-process.md for every component: build, gallery entry, screenshot at the card's size, compare against design/reference, exercise every interaction and motion in the spec. Correct docs/02-component-spec.md if the CSS disagrees with it.
- Do not touch or wire into my existing assistant app; it already uses the same gpui line and I will integrate later, pane by pane.
- Keep architecture, the motion engine, tricky gpui elements and all reviews on Fable. Delegate routine work (scaffolding, porting cards to gallery entries, sample data, tests, docs) to Opus 5 subagents and review their output before it lands.
- Order: workspace and tokens, motion engine, gallery with shell and sidebar, then transcript and composer, then workbench panels, then the assistant mock. Show me the gallery as soon as the shell and sidebar run so I can react early.
- Commit at the end of each phase with a clear message.

Begin with phase 1: create the Cargo workspace, get a gpui-pre plus gpui-kit window running, and generate the gpui theme from design/tokens/tokens.json.
```

The session's persistent memory lives outside the repo at
`~/.claude/projects/-Users-latekaapi-Projects-agentic-ui/memory/`; `docs/memory-snapshot/` is a copy taken at handoff time.
