//! `aui-terminal` — the PTY-backed terminal pane of the workbench.
//!
//! Nothing is implemented yet: this crate is scaffolding, and the scope below is
//! the contract it must meet, taken from `docs/02-component-spec.md` section 5.1
//! (workbench card 50, rendered at `design/reference/cards/50-terminal.png`).
//!
//! # Scope
//!
//! **PTY** — a real pseudo-terminal running the user's own login shell, with the
//! grid parsed from the byte stream and rendered as a `gpui` element. Tabs live
//! in the shell header; splits (`⌘D` / `⌘⇧D`) resize on the layout spring. The
//! ANSI 16 palette comes from `aui-tokens`, never from hard-coded colours.
//!
//! **Block terminal, not a wall of text** — each command is a block with a radius
//! of 8: a one-line command row (6 px exit dot in success / danger / accent with a
//! ring while running, an accent `$`, the command in mono 12, and on the right the
//! provider mark plus the agent name when an agent ran it, then the duration in
//! mono 11 dim), the output lines below it (one element per line, nowrap with
//! ellipsis), and a fold row once past eight lines. Old blocks sit at .72 opacity;
//! the live block gets surface-1 and a 1 px line; failed blocks tint the command
//! row danger-soft and keep their output at full ink. Hovering a block reveals
//! copy and "ask the agent" actions. A restored-scrollback marker separates the
//! previous session, and the 36 px prompt row carries the blinking accent cursor
//! and the branch and cwd tags.
//!
//! **TUI mode** — a terminal-only view of the agent itself: a 40 px pane toolbar
//! (Chat | Terminal segmented control, agent chips, the "runs in a PTY with your
//! own login" note, a split button), the agent's TUI rendered line by line, its
//! input box docked at the bottom, and a 36 px status row with the model,
//! permission mode and branch as tags.

#![deny(missing_docs)]

/// The delivery phase this crate belongs to, from `docs/01-research-and-plan.md`.
///
/// The terminal ships with the rest of the workbench.
pub const PHASE: &str = "phase 3 · workbench";
