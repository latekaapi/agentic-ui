---
name: project-decisions-2026-09-05
description: Decisions the user made on 2026-09-05 about stack, design workflow, document editing, v1 scope, and the assistant app's domain
metadata:
  type: project
---

Decided 2026-09-05 (user answers to the plan's open questions):
- Stack: gpui-pre 0.3.x + gpui-kit 0.6; own motion engine on top. macOS-only is acceptable.
- Design workflow: build an HTML design-system bundle (gallery previews) -> push to a Claude Design design-system project with DesignSync -> compose screens in Claude Design -> implement in gpui. User asked to be walked through it.
- Assistant document panes: docx, xlsx, pdf, markdown, text all editable; images and video render. Web editors (Univer / ProseMirror-docx / pdf) inside the wry webview for docx/xlsx/pdf; native gpui editor for md/text.
- Harness v1 scope: Orca "core" only (sidebar, transcript, terminal, browser, files, diff+notes, commit/PR). Also wanted: a "terminal-only" mode running an agent TUI (Pi, Claude Code) in a PTY like Orca, logged in with the user's subscription. So the GPU terminal is a v1 requirement and its visual design matters a lot: the user "hates big walls of text".
- Assistant domain: roles (e.g. Director Education, Director Law) -> projects -> sessions; used to draft reports, letters, notes, RFPs; later a tiered RAG pipeline over orders/rules/regulations per role/project/session. Citations/sources UI will matter.

**How to apply:** keep terminal design polish and document-pane editing in scope; sidebar hierarchy for the assistant is role > project > session. See [[project-agentic-ui-goals]].

Progress 2026-09-05: design-system bundle built at design/ (34 cards, build.py, tokens.css; Geist + Geist Mono; iris accent). Not yet pushed: DesignSync needs the user to run /design-login in an interactive `claude` session first. Next steps after login: create design-system project "Agentic UI", finalize_plan on design/dist/**, write_files, then screens in Claude Design.

Implementation phase agreed 2026-09-05 (starts in a new session):
- Deliverables: (1) library crates aui-tokens / aui-motion / aui-icons / aui / aui-protocol / aui-webview / aui-terminal with rustdoc; (2) `aui-gallery` native app: every design card live in every state, theme + density switch, motion playground; (3) a mocked ASSISTANT shell in the demo binary (roles, citations, document panes with sample docx/xlsx/pdf); (4) build/run docs + a card→component→gallery checklist. NO mocked harness app (user dropped it).
- Do NOT wire the library into the user's existing assistant app yet; it already uses the same gpui-pre 0.3 + gpui-kit 0.6 line, and the user will refine it first, then integrate pane by pane.
- Model usage: keep architecture, motion engine, tricky gpui elements and reviews on Fable; delegate routine work (scaffolding, porting cards to gallery entries, sample data, tests, docs) to Opus 5 subagents and review their output.
- Design source of truth: design/tokens/tokens.css, design/src/base.css, design/src/cards/*.html, design/screens/common.py (+ harness/assistant builders). Parity with these cards is the acceptance test.
- Spec package committed (8cbce3e): docs/02-component-spec.md (per-card anatomy/sizes/states/motion/keys/data), docs/03-parity-process.md (loop + card→component→gallery checklist), docs/04-design-rules.md (taste rules), design/reference/{cards,screens}/*.png (reference renders), design/tokens/tokens.json + motion.json (machine-readable). Read these before implementing any component.

Implementation progress 2026-09-05 (evening): phase 1 + motion engine committed (workspace, aui-tokens with generated theme + bundled Geist, aui-icons, aui-protocol, aui-motion, aui-gallery with --screenshot flags; foundation cards 01–05 at parity ≤2 % differing pixels, hashes in docs/03). Next: phase 3 = shell + sidebar in `aui` (cards 10, 11, 20–23) and show the user the gallery early; then transcript/composer, workbench, assistant mock. Parity loop tooling: scripts/parity.sh; references re-rendered with headless Chrome after any design/ change.
