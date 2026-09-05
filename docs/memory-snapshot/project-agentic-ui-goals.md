---
name: project-agentic-ui-goals
description: What the agentic-ui project is — a gpui UI library powering two macOS apps (an Orca-like agent harness and a day-job AI assistant)
metadata: 
  node_type: memory
  type: project
  originSessionId: 33eb40c6-ef3e-4b37-ab21-fb8ff2a229dd
  modified: 2026-09-04T20:05:38.680Z
---

Project started 2026-09-05 in /Users/latekaapi/Projects/agentic-ui (empty dir, no git yet).
Goal: one polished, animated gpui (Rust) UI library shared by two macOS apps:
1. An agentic coding harness modeled initially "exactly like" onorca.dev (Orca by Stably AI; also refs trysynara.com, T3 Code).
2. A day-job AI assistant with the same 3-pane shell (sidebar / transcript / right pane) whose right pane hosts browsers integrated with chat plus editable docx/xlsx/pdf renders of chat-created files.

Reference UI libraries the user wants to match in polish/micro-interactions: gpui-kit.com, transitions.dev, beautifului.dev, elements.ai-sdk.dev, beui.dev.
Workflow the user asked for: research -> exhaustive component inventory/plan -> design in Claude Design -> implement.

**Why:** the user cares most about design quality (micro-interactions, transitions) and a shared library so both apps stay consistent.
**How to apply:** treat design polish as a first-class requirement; reuse gpui-kit primitives where they exist and build the agentic/chat layer on top. See [[gpui-ecosystem-versions]].
