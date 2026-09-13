# Design rules (decided with the user, apply before building anything)

Calm colour
- Accent is the only decorative hue; status colours only carry meaning. Neutrals are tinted toward the accent.
- Labels: the eight-colour `label-1 … label-8` ramp is the identity of a project, never status. A project's mark, group row, rail tile and palette row share its label colour; nothing else takes one, and a label colour never stands in for running, warning, done or failed.
- No glows, no halos. Pending dialogs (approval, question, error) get a plain 1 px border in their status colour at ~70% alpha. Resolved cards fall back to the hairline.
- Tags (repo, branch, counts, paths) are plain muted mono text with no background. Pills with tinted grounds are for status only.
- Selected rows use a surface step (surface-3), never an accent wash. Tab indicators and progress bars use ink, not accent.
- Provider marks stay coloured (they identify the agent); everything else in the sidebar and headers is muted line iconography.

Controls
- Buttons must read as buttons: secondary has a fill, border and hairline shadow; primary is accent; ghost only for icon and tertiary actions; danger is outlined.
- One action-row pattern everywhere: hint on the left, spacer, secondary buttons, primary on the far right. Buttons never wrap.
- All composer toolbar controls are 28 px with a 6 px radius, including the send button and the bordered `+`.

Layout
- Three header cells, one per column, 44 px; dividers run continuously top to bottom. Centre header: provider mark + worktree + branch, overflow menu, right-pane toggle. Right header: the pane's tabs + `+` + close. Pane actions live in the pane body.
- No bottom status bar. Provider usage lives in the sidebar footer.
- Composer is docked: full pane width, top hairline, no floating card.
- Assistant text and cards span the full transcript width.
- Generous vertical rhythm: ui line-height 1.5, body 1.65, mono 1.6; rows 30 px; tool card headers 34 px; 16 px between transcript blocks; 12 px between the status row and the composer.

Sidebar
- Same row anatomy in every grouping (status / project with nested children / date); a sliders icon opens the view menu.
- Assistant roles are bordered sections with muted icons; hierarchy by weight and ink (600 / 500 ink-2 / 400 ink-3), no background on the active role, only the active session highlighted.

Terminal
- Blocks, not walls of text: one card per command with status, duration, agent tag, foldable output; old blocks dim; the live block is lifted; failures tint the command row.

Browser
- An annotator, not a design mode: hover outlines, click pins numbered notes, a side list collects them, one action sends the batch with metadata and a screenshot to the agent.

Files
- Muted file-type icons (ts, tsx, json, md, test, css, lock, folder) with no ground.

Hygiene
- Static design states must not contain unexplained mid-gesture artefacts.
- Every shell change is mirrored in the component library the same turn.
- Style helpers are named for their purpose; never reuse a generic helper across unrelated components.
