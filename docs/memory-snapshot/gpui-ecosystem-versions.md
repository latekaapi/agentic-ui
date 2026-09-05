---
name: gpui-ecosystem-versions
description: "gpui crate landscape and versions found on 2026-09-05, plus local Rust toolchain quirks on this Mac"
metadata: 
  node_type: memory
  type: reference
  originSessionId: 33eb40c6-ef3e-4b37-ab21-fb8ff2a229dd
  modified: 2026-09-04T20:05:48.561Z
---

As of 2026-09-05:
- gpui stable 0.2.2 (crates.io, Aug 2026); gpui-pre 0.3.3 (Sep 3 2026, snapshot of zed main) — gpui-kit 0.6.0 depends on gpui-pre 0.3.1 renamed as `gpui`.
- gpui-kit 0.6.0 (Longbridge, formerly gpui-component; docs at gpui-kit.com). Has Message/Bubble/MessageScroller/Attachment/Marker chat primitives, Dock (DockSkin/DockLayout/DockAreaState), Resizable, Sidebar, Command palette, TextView (markdown/html), Editor, VirtualList, Theme (ActiveTheme, ThemeRegistry, gradient tokens). No animation/transition module of its own.
- gpui-wry 0.6.0 (crates/webview in gpui-kit repo) — wry/WKWebView overlay, renders on top of gpui content.
- gpui-animation 0.2.63 — with_transition/transition_on_hover/transition_when on gpui 0.2.2.
- guise-ui 1.5 (MIT, wess.io/guise) — 130+ components, anime.js-style motion (Motion/Sequence/Stagger/Presence/Collapse, springs), AI chat kit, webview; targets gpui 0.2.2.
- Zed's own agent UI lives in zed/crates/agent_ui (reference for tool-call cards, diffs, message editor).

Local machine: rustup at ~/.cargo/bin (stable 1.98.1, aarch64) but NOT on the non-interactive shell PATH — prefix commands with `export PATH="$HOME/.cargo/bin:$PATH"`. Xcode 26.6 installed; Homebrew at /opt/homebrew (see update below).

**How to apply:** pick gpui-pre 0.3.x + gpui-kit 0.6 as the base unless the user prefers stable gpui 0.2.2 (then guise/gpui-animation are compatible instead).

Update 2026-09-05: Homebrew IS installed at /opt/homebrew (6.0.21); it looked missing because the desktop-app session inherits the GUI env, not the login shell. Fixed via a SessionStart hook in ~/.claude/settings.json that writes a PATH export to $CLAUDE_ENV_FILE. In this session still prefix with `export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"`.
