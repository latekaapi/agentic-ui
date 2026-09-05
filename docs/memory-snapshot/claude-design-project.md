---
name: claude-design-project
description: The Claude Design design-system project that holds the aui design bundle, and how to push updates to it
metadata:
  type: reference
---

Claude Design design-system project "Agentic UI", projectId ca8c3dbe-2b3e-4dc8-ab07-f0ecac7aca07 (created 2026-09-05, owned by the user). Holds design/dist from the repo: 34 cards under components/<group>/, plus tokens.css, cards.json, index.html.

**How to apply:** to update, rebuild with `python3 design/build.py`, then DesignSync finalize_plan (localDir design/dist, writes components/**/*.html etc., deletes []) and write_files with the planId. The user authorized with /design-login from the in-app terminal; the credential persists on this Mac. See [[project-decisions-2026-09-05]].

Harness screens canvas (design skill, published 2026-09-05): https://claude.ai/code/artifact/e77554a4-ff0f-4eb6-ae2f-a838de61d384 — working files in design/screens/harness/ (build_screens.py generates Main/DiffReview/TerminalMode/NewTask .dc.html + canvas.json; seed with the design skill helper, republish with contract 0.1.31, favicon 🪟). The user may edit it in the canvas editor; read the artifact and --extract before re-seeding if they have saved.
Assistant screens canvas (published 2026-09-05): https://claude.ai/code/artifact/bdc61083-243f-4dce-8b4f-9e65ff4dce7a — working files in design/screens/assistant/ (Main/Sources/Sheet .dc.html, light default, favicon 📄). Shared chrome for both generators lives in design/screens/common.py.
