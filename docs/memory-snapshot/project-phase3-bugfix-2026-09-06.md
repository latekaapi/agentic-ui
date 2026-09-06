---
name: project-phase3-bugfix-2026-09-06
description: Phase 3 bug-fix pass on 2026-09-06 — what was fixed from the user's annotated screenshots, and that it was left uncommitted for the next session
metadata:
  type: project
---

On 2026-09-06 the user reviewed the Phase 3 gallery with annotated screenshots. Fixed (all in the working tree, NOT committed at handoff): collapsed sidebar now renders a real `Rail` via `AppShell::rail`, 72 px column when traffic lights are present, reopen toggle at the centre header's leading edge (`CentreHeader::on_expand_sidebar`), `AUI_GALLERY_COLLAPSED=1` renders screens collapsed; toast fan-out deferred above siblings with a hover pad; composer plus menu and citation card via `popover_layer`; note cards unified (see [[feedback-design-taste]]); hover tints via `tint_fade` across palette, menus, nav rows, workbench rows.

**Why:** these were the user's blockers before closing Phase 3 and starting Phase 4 (docs/05-handoff.md). **How to apply:** next session should commit this tree first, then start Phase 4 step 1. Unverified by hand: live spring motion for sidebar toggle, toast collapse, menu morph; light-theme renders of cards 37/52.
