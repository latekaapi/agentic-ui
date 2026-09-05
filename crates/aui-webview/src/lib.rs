//! `aui-webview` — the embedded browser pane of the workbench.
//!
//! Nothing is implemented yet: this crate is scaffolding, and the scope below is
//! the contract it must meet, taken from `docs/02-component-spec.md` section 5.2
//! (workbench card 51, rendered at `design/reference/cards/51-browser.png`).
//!
//! # Scope
//!
//! **The pane** — a `wry`/WKWebView surface hosted in a `gpui` element, with tabs
//! in the shell header and a 38 px nav row: back / forward (disabled at .4 alpha)
//! / reload, a 28 px mono URL field with a success shield and `⌘L`, an Annotate
//! mode toggle with `esc`, and screenshot and console icon buttons. Loading shows
//! a 2 px accent hairline along the bottom of the nav row.
//!
//! **JS bridge** — script injection and evaluation both ways, so the pane can
//! read the DOM for the annotator, drive the page on the agent's behalf, and
//! surface console messages and network activity.
//!
//! **Annotator** — a crosshair mode that outlines the hovered element (1 px
//! accent at .6 over an accent-soft fill) with a mono tag above-left
//! (`p · 392 × 34`), drops a numbered 20 px teardrop pin on click, and opens a
//! 236 px note popover carrying the element path. Annotations collect in a 272 px
//! side panel with the selector, box, outer HTML, computed styles, source file,
//! URL and a screenshot preview with the pins on it.
//!
//! **Screenshot to chat** — the "Send to Claude Code" action packages the pins,
//! notes and element metadata as `aui-protocol` notes and hands them to the app
//! as a `SendNotes` intent, with the annotated screenshot attached.
//!
//! # The native-overlay caveat
//!
//! The webview is a **native overlay** composited above the `gpui` scene, not a
//! layer inside it. It always paints on top, and `gpui` cannot draw over it. So
//! every popover, menu, tooltip and note bubble must either be positioned outside
//! the webview's bounds or be rendered inside the page itself through the JS
//! bridge. The same applies to drag overlays and the command palette when the
//! browser pane is open.

#![deny(missing_docs)]

/// The delivery phase this crate belongs to, from `docs/01-research-and-plan.md`.
///
/// The browser pane ships with the rest of the workbench.
pub const PHASE: &str = "phase 3 · workbench";
