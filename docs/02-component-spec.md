# Component spec (source of truth for the gpui port)

Every entry maps one design-system card (`design/src/cards/…`, rendered in `design/reference/cards/…png`) to the gpui component that must reproduce it. Values are the ones in the cards; when the CSS and this file disagree, the CSS wins and this file should be corrected. Tokens are named as in `design/tokens/tokens.json`; motion as in `design/tokens/motion.json`.

Conventions used below: **anatomy** (parts, left to right / top to bottom), **size** (px), **states**, **motion**, **keys**, **data** (what the app supplies). "ink" = `--ink`, "ink-2/3/4" = secondary/tertiary/quaternary text. All radii: xs 4, sm 6, md 8 (cards, tool cards), lg 12 (dialogs, panels), xl 16, full.

---

## 0. Foundations

### 0.1 Colour (card 01)
- Two themes, both first-class. Harness defaults dark, assistant defaults light; every window exposes a theme switch.
- Neutrals are hue-tinted toward the accent (iris). Accent is the only decorative hue and is used for: primary buttons, send button, focus ring, running-state dot, links, citation markers, the blockquote rail on a source hover card. Nothing else.
- Status colours carry meaning only: success, warning, danger, info. Agent states map: running → accent, waiting/needs-you → warning, done → success, failed → danger, idle → ink-4.
- Soft variants (`*-soft`, ~10% alpha) are the only tinted grounds allowed: status pills, glyph circles, diff add/remove rows, waiting/error banners.
- Provider marks (Claude, Codex, Grok, Gemini, Pi, Cursor) are 16 px rounded-4 squares with a single letter; opacity .92; sizes 16 / 13 / 12 / 11 exist.

### 0.2 Typography (card 02)
- UI face Geist; mono face Geist Mono (bundle the TTFs; SIL OFL). Fallback: system UI / SF Mono.
- Scale: 11, 12, 13 (default UI), 14 (body in transcript is 13.5), 16, 18, 20, 24. Line heights: ui 1.5, body 1.65, mono 1.6, tight 1.3.
- Product text scale: the design grid is 13 px, but apps render text at 1.1× by default (`--text-scale` / `AuiTheme::text_scale`; UI 14.3, body 14.85). Spacing, radii and control heights do not scale. Decided 2026-09-05; parity screenshots stay at 1.0.
- Weights: 400 body, 500 controls and emphasis, 600 headings and labels. Never 700 except display.
- Eyebrow/caps labels: 11 px, 600, letter-spacing .08em, uppercase, ink-3.
- Tags (repo, branch, counts): mono 10.5 px, 500, ink-3, no background.

### 0.3 Spacing, radius, elevation (card 03)
- Space: 2, 4, 8, 12, 16, 24, 32. Control heights: xs 20, sm 24, md 28, lg 32, xl 40.
- Elevation: 1 rows/chips (0 1 2 rgba 6–35%), 2 popovers/toasts, 3 palette/dialogs. Docked panels are flat.
- Padding belongs to the component; gaps to the parent. Never double padding between nested containers.

### 0.4 Motion (card 04) — implement in `aui-motion`
- Durations: fast 120, base 180, enter 220, exit 160, slow 280 ms. Easings: out (.16,1,.3,1), inout (.65,0,.35,1), std (.2,0,0,1).
- Springs (stiffness/damping/mass): press 500/30/0.6 (buttons, chips, rows), swap 460/30/0.55 (icon morph, chevrons, tab indicator, checkbox), layout 360/32/1 (collapse, panel width, card resize), gentle 200/26/1 (popover morph, drop overlay).
- Primitives: `Transition` (tween colour/opacity/size/padding/radius/shadow/transform on state change), `Presence` (enter/exit with exit hold), `Collapse` (height 0↔auto on layout spring), `Stagger` (per-child delay, default 40–60 ms), `Shimmer` (text gradient sweep 1.8 s linear; surface skeleton 1.4 s), `IconMorph` (out: fade + 3 px + scale .8; in: reverse; swap spring), `NumberTicker`, `StreamReveal` (per-chunk fade + 3 px rise, base duration), `Pulse` (dot ring 2 s ease-out), `Shake` (500 ms, ±4 px), `CheckDraw` (stroke draw 320 ms ease-out).
- Rules: animate transform and opacity first; never animate layout when a fade will do; every animation has a readable resting state; reduced-motion sets all durations to 0 and springs resolve instantly.

### 0.5 Icons (card 05) — `aui-icons`
- Lucide-style 14 px (16 px in headers), stroke 1.6, round caps. Provider marks as above. File-type icons (`fic`): folder, folder-open, ts, tsx, json, md, test, css, lock, image, file; muted hues (ts #5B8DD6, tsx #4FA8B8, json #C9A15A, md #7C8AA5, test #8C9C6E, css #8E79C9, lock ink-4, folder ink-3), no ground. Role icons: grad-cap, scale, gear, ink-3. Status glyphs: ok (14 px circle, success-soft ground, success check), err (danger), spinner (14 px, 1.5 px ring, accent top, 0.9 s), braille spinner in status lines.

---

## 1. Shell

### 1.1 App shell (card 10; screens harness-*.png)
- Grid: columns 252 | 1fr | 400 (392 in the card); rows 44 (header) | 1fr. No bottom status bar.
- Three header cells, each 44 px, surface-1, 1 px bottom border; the sidebar cell owns the first divider (its right border) and the right cell the second (its left border), so each lines up with the pane border beneath it and the dividers run continuously top to bottom (the card originally staggered the first one by 1 px; fixed 2026-09-05).
  - Sidebar cell: traffic lights (11 px, 7 gap), back, forward (disabled at .45), spacer, search, sidebar-toggle. Icon buttons are `btn icon sm ghost`, ink-3.
  - Centre cell: provider mark 16 px + worktree name (600, 13 px) + branch tag; spacer; overflow menu (dots) and right-pane toggle (`panel-right`). Nothing else.
  - Right cell: the right pane's tab strip (tabs 44 px tall, 12 px icon + label, active = ink with 2 px ink underline at the bottom edge, inactive ink-3), a `+` (xs ghost), spacer, close (`x`). Pane-specific controls live inside the pane body, never in the header.
- Centre pane: transcript scroll area padding 18 32 0 (28 in the card), blocks gap 16; then a status row (spinner/glyph + text, 12 px, ink-3, 12 px bottom padding); then the docked composer.
- Docked composer: full pane width, 1 px top border, surface-1, padding 12 32 12 (28 in the card). Text area 13.5/1.5, min 40 px; toolbar row 10 px below: `+` (bordered `btn icon`, 28), model chip, mode chip, effort chip (28 px chips in the composer, 22 elsewhere), context %, spacer, send/stop button 28 × 28 radius 6 accent (IconMorph between arrow-up and stop square).
- Sidebar footer: 1 px top border, padding 10 12: avatar 22, name (ink-3 12 px, truncates), provider usage meter (mark 11 px + 40 × 3 bar in ink-3 on surface-3 + "78%" mono 11), chevron-down xs ghost.
- Sidebar toggle: collapsing the sidebar swaps the pane for the rail (never a clipped sidebar) on the layout spring. In the shell the rail column is 72 wide, not 48, because the window's traffic lights own the top-left; the collapsed sidebar header cell keeps the lights and nothing else, and the sidebar toggle moves to the leading edge of the centre header (⌘B does the same) so the sidebar can always be reopened.
- Right pane toggle: closing the pane collapses the third column with the layout spring (slow 280 ms) and the toggle icon in the centre header reopens it; the `x` in the right cell does the same.
- Data: worktree name, branch, provider, right-pane tabs, active tab.

### 1.2 Panel chrome and tabs (card 11)
- Floating/docked panel header: 36 px, grip (4 × 4 dot pattern), icon, title 600, subtitle ink-3, spacer, quiet icon actions.
- Tab strip: 34 px (44 inside the shell header). Sliding indicator 2 px ink, animates left/width on the swap spring. Close affordance (14 px, radius 3) appears at opacity 1 on hover or when active; dirty dot 6 px ink-3.
- Drop zones while dragging a tab: 3 × 3 grid over the target panel; four edge zones dashed accent-ring border on accent-soft at .55; centre zone solid at .9. Ghost tab follows the pointer, rotated −2°, elevation 2.

### 1.3 Command palette (card 12)
- Scrim rgba(0,0,0,.25→.45) gradient. Panel 560 wide, overlay ground, radius 16, elevation 3, opens with fade + 6 px rise + scale .985 (enter 220 ms out).
- Query row 46 px: search icon, input 14 px, `esc` kbd. Sections with caps headers; rows 34 px: icon (ink-3), label, muted context, right shortcut kbds; selected row surface-3 with ink text; matched substrings accent-ink 600. Footer 32 px hints.
- Keys: ⌘K palette, ⌘P files, ↑↓, ↩ open, ⌘↩ open to the side, esc.

### 1.4 Toasts and banners (card 13)
- Toast: 320 wide, overlay ground, 1 px line-strong, radius 12, elevation 2, grid 22 px icon | text | close. Title 13 600, body 12 ink-2, actions row (xs buttons) 8 px below. Auto-dismiss hairline 2 px at the bottom, ink-3, pauses on hover.
- Stack: newest in front; older ones translateY(−10) scale(.96) opacity .7, then −20 / .92 / .4. Hover fans the stack out (layout spring). Enter: 8 px rise + scale .97 (enter). Exit: fade (exit 160).
- Banner: one line, 1 px border, radius 8, icon 14 left, text flex, one xs button right. Only waiting (warning-soft ground+border) and error (danger-soft) are tinted; info and success stay on surface-1.

---

## 2. Sidebar

### 2.1 Worktree / session rows (cards 20, 23)
- Row: grid 14 px | 1fr | auto; padding 9 10; margin 2 8; radius 8. Line 1: status dot 7 px (pulse ring when running/waiting), name 12.5–13 px 500 ink, elapsed mono 11 ink-3 right. Line 2 (meta, 12 px ink-3, nowrap, truncates): repo tag, branch tag, provider marks 13 px. Optional line 3: live activity sentence (spinner 10 px + text, ink-2) or the waiting reason in warning or the failure in danger.
- Selected: surface-3 ground (no accent wash). Hover: surface-2; the elapsed time fades out and four xs ghost actions fade in (terminal, browser, pin, more) on a surface-2 tray. Unread: 3 px accent bar at the left edge.
- Children nest at +22 px with a 1 px line rail on the left; names 12 px.
- Compact session row (`sr`, used in project and date views): min 30 px, padding 4 10 4 12, one line + optional meta line.

### 2.2 Sidebar (card 21) and views (card 23)
- Sections: nav (Tasks with count, Automations, Inbox with warning-coloured count) 30 px rows; group row 28 px (caps label, spacer, sliders icon = view options, plus); group headers 28 px with chevron (rotates 90° on the swap spring), 12 px 600 label, mono count.
- Three groupings, same row anatomy: **status** (Needs you / Running / Done groups), **project** (project rows 30 px 600 with folder-open/folder `fic` icon and count; sessions and child sessions beneath), **date** (caps headers Today / Yesterday / This week with a hairline rule).
- View options menu (from the sliders icon): 250 wide, overlay, radius 12, elevation 3; rows 30 px: Status ▸ Active, Environment ▸ All, — Group by ▸ Project, Sort by ▸ Last activity, — Show empty groups, Show PR status ✓, Show archived. Submenu 170 wide: Status, Project ✓, Date, Custom groups, — None. Choice persists per workspace.
- Collapsed rail (⌘B): 48 wide standalone (72 as the shell's collapsed column, clearing the traffic lights); nav icons 32 px; one dot per active worktree in its state colour; avatar at the bottom; tooltips on hover; width animates on the layout spring.

### 2.3 Assistant sidebar (card 22)
- Roles are bordered sections (1 px bottom border each). Role header 44 px: chevron 12 px, role icon (grad-cap / scale / gear, ink-3), name 600 ink; closed roles ink-2 with a count tag at the right. No background on the active role.
- Open role: group label rows 24 px (caps "Projects" / "Knowledge", hairline rule, mono count); project rows 30 px 500 ink-2 with folder icon and count; session rows 30 px 400 ink-3 indented 40 px with a 1 px rail at x=29, icon by kind (doc / sparkle / sheet), elapsed right; active session surface-3 + ink. Knowledge card: surface-2, 1 px line, radius 8, chips (book icon + name) wrapping, "+ add" in accent-ink.
- Hierarchy is told by weight and ink only: roles 600 ink, projects 500 ink-2, sessions 400 ink-3.

---

## 3. Transcript

### 3.1 Header identity and markers (card 30)
- No session header inside the transcript. Provider mark + worktree in the centre header; model/mode chips in the composer.
- Marker row: 11.5 px ink-3, hairline on both sides, 12 px icon, one line max. Kinds: time/session started, hand-off (pill with two provider marks and an arrow), context compacted (link "view summary"), permission mode changed.

### 3.2 Turns (card 31)
- User turn: right-aligned, max 72% width, surface-3, radius 12/12/4/12, padding 9 13, 13 px/1.5. Attachments strip above (30 px chips with 18 px icon tile). Mention chips inside text: surface-3 ground, accent-ink mono 12. Hover reveals edit / copy / re-send xs ghost buttons to the left.
- Assistant turn: full pane width, no bubble, 13.5 px/1.65 ink. Streaming: chunks fade + 3 px rise (StreamReveal); caret 2 × 15 px accent blinking 1 s steps(2). Hover toolbar (copy, retry, fork, bookmark) appears above-right at opacity 1 with 4 px rise, never shifting layout. Footer meta mono 11 ink-4: model · duration · tokens · cost.
- Inline code: mono 12 on surface-2, radius 4. Lists 18 px indent.

### 3.3 Thinking block (card 32)
- Card radius 8, 1 px line, surface-1. Header 34 px: brain icon (accent-ink while thinking, ink-3 after), "Thinking" shimmer 500, elapsed mono 11 right, chevron.
- While thinking: viewport max 96 px with a 34 px top fade, content bottom-aligned so new text pushes up; 12.5/1.55 ink-2.
- Done: collapses (layout spring) to one line "Thought for 14 s · summary" (summary ink-2 400). Click toggles the full trace (chevron rotates 90°).

### 3.4 Activity group (card 33)
- Header 34 px: spinner + shimmer label while working, or ok/err glyph + "Ran 3 commands" 500 + "· read 2 files · edited 1 file" ink-3; step glyph strip (14 px squares, gap 3: ok success-soft, running accent-soft, error danger-soft, pending surface-3); elapsed right; chevron.
- Timeline rows 30 px: 18 px node on a 1 px rail (dot 8 px: success / accent with 3 px accent-soft ring while running / danger / surface-3 outlined when pending), verb 500, mono target ink-2 12 px, right result mono 11 (add counts in success). Pending rows ink-3.
- Expand/collapse on the layout spring; new rows enter with fade-rise.

### 3.5 Tool call cards (card 34)
- One header for all tools, 34 px: status glyph | verb 500 | mono target (ink-2, truncates) | right group (result pill or count, duration mono 11 ink-3, chevron). Card radius 8, 1 px line, surface-1; body separated by a 1 px top border.
- Bodies: **shell** on term-bg with 11.5/1.65 mono, ANSI colours, exit pill (success-soft "exit 0" / danger-soft "exit 1" / line "live"), fold row after 6 lines ("14 more lines" + "open in terminal"; the spec said 8, the card shows 6); **read** header only ("180 lines"); **edit** unified diff rows 11.5/1.6 with 26 px gutters, add/del grounds, "+8 −3" in success/danger, fold row "2 more hunks · open in Diff"; **search** hit rows path:line ink-3 + match accent-ink 500; **web** rows favicon 14 + title + domain ink-3; **browser** 70 px screenshot tile with the click ring (18 px accent ring pulsing) and an action label; **sub-agent** nested transcript; **generic MCP** definition list + JSON tree.
- Running header shows the spinner and the live pill; completion morphs spinner → glyph (IconMorph).

### 3.6 Approval card (card 35)
- Pending: radius 12, **1 px border in warning at 70% alpha**, no halo, no left edge. Header: 26 px warning-soft icon tile with shield, title 13 600, subtitle 12 ink-2 (cwd + capabilities), "Waiting" warning pill right. Command block on term-bg mono 12. Details 12 ink-2 with a 2-col definition list. Action row (surface-2, 1 px top border, padding 8 12): key hints left (`Y` allow · `A` always · `N` deny), spacer, Deny (danger outline), Always allow with the rule in mono at .7, Allow once (primary). Buttons enter staggered 40 ms on ease-out; card enters with 6 px rise.
- Resolved states are quiet single-line cards (1 px line border): Approving… (spinner tile, "Running" pill), Allowed once (success tile, "Complete" pill, exit/duration), Denied (danger tile, "Denied" pill), Auto-allowed (accent tile, "Rule" pill, "manage rules" link).
- Keys: Y / A / N while the card is focused; Enter = primary.

### 3.7 Question, plan, todo (card 36)
- Question: radius 12, **1 px accent border at 70%**. Header with 26 px accent-soft tile (question icon), title 600, subtitle ink-2. Options: 1 px line, radius 8, padding 8 10, radio/checkbox 16 px (accent when on, inner 8 px dot pops in on the swap spring), label 12.5 500 + description 12 ink-2, number kbd right; selected option surface-2 + accent border. "Other" dashed row. Action row: "2 selected" hint, spacer, Skip ghost, Continue primary. Answered: collapses to a one-line card with chips and "Change".
- Plan: 1 px line card; header list icon + "Proposed plan" 600 + "Plan mode" line pill; ordered list 12.5/1.6 ink-2; action row: `⇧⇥` mode hint, spacer, Reject ghost, Edit, Accept and run primary.
- Todo: header 34 px (list icon, "Tasks" 500, "3 / 7" mono), 3 px progress bar in ink-3 (width on layout spring), rows 30 px: 15 px status mark (done = success fill + white check; running = accent border + 3 px ring + pulsing 6 px dot; pending = line-strong ring), label (done: ink-3 + strikethrough; running: ink 500), elapsed mono right. Marks morph on the swap spring.

### 3.8 Code and diff blocks (card 37)
- Code block: term-bg, 1 px line, radius 8. Header 30 px surface-2: file icon + filename mono 11.5, language ink-3, actions (wrap, open in editor, copy) at .6 → 1 on hover. Copy morphs to a success check (IconMorph, swap spring) for ~1.2 s. Body mono 12/1.6 with 22 px line numbers in term-dim. Fold row 24 px after 12 lines. Syntax colours: keyword magenta, function blue, string green, number yellow, comment dim italic.
- Diff block: header with `+2 −1`, Unified/Split ghost toggle, "Open in Diff". Hunk row surface-2 11 px ink-3. Rows with old/new gutters (36 + 20 px), add/del grounds; hovering a row shows a 16 px accent "+" at the left edge to add a note. Note: one uniform card, the same frame the approval card uses — surface-2, a 1 px solid `line` border all the way round, radius 6. No left accent rail and no dashed variant. Caps "NOTE · LINE 44" in accent-ink, body 12/1.4, Cancel/Add note xs actions right-aligned while pending, Edit/Delete once saved.

### 3.9 Summary, error, status rows (card 38)
- Summary: 1 px line, radius 12. Header ok glyph + "Done · title" 600 + duration/cost mono right. File rows 26 px mono 12 with M/A/D letter (warning/success/danger) and +/− counts right. Checks row (success checks + labels). Action row: "3 files changed" hint, spacer, Create PR ghost, Commit…, Review diff primary.
- Error: **1 px danger border at 70%**, radius 8, grid icon | text | button; title 600, body 12 ink-2 with a danger "details" link; Retry now (secondary) vertically centred.
- Status rows 28 px 12 px ink-3: spinner + shimmer "Working…" + mono elapsed + `esc` kbd + "to interrupt"; braille spinner variant; "Waiting for you" with a warning shield.
- Needs-you banner (warning-soft) pinned above the composer with "Jump to it" primary xs; enters with 6 px rise.
- Jump-to-latest pill: 28 px, overlay ground, 1 px line-strong, elevation 2, chevron-down + label + count badge (ink on bg). Appears when the reader scrolls away from the tail; counts new turns.

---

## 4. Composer

### 4.1 Composer (card 40)
- Docked variant is the shell default (see 1.1). Focused floating variant (card): 1 px line-strong, radius 14, accent border + 3 px accent-ring shadow when focused.
- Above the text: context chips (22 px; `@` mention, image attachment, `$` skill) with an `x` at .4. Text 14/1.55, grows 2 → 8 rows.
- Toolbar: `+` bordered icon button 28 that rotates 45° (swap spring) while its menu is open; the menu morphs from the button's corner (gentle spring, clip-path reveal): Attach file ⌘U, Screenshot browser, Mention file or symbol, Use a skill `$`, Commands `/`. Model chip (provider mark + name + chevron), mode chip (Ask / Plan / Auto / Bypass), effort chip (brain icon + level), context % ink-3, spacer, send/stop 28 px.
- Send ↔ stop: IconMorph on the swap spring; the button scales .92 on press (press spring). Disabled send: surface-3 ground, ink-4 icon.
- Queued messages: dashed 1 px line-strong rows 30 px above the composer with a "queued" badge and edit/remove xs ghost actions. ⌘↩ queues while a turn runs.
- Suggestion chips: 26 px pill outlines below the last turn, stagger 60 ms fade-rise.
- Meta strip (optional, above the box): branch, context meter (48 × 4), session cost, "⌘↩ to queue".

### 4.2 Slash commands and mentions (card 41)
- Popover anchored to the caret, opens upward: overlay ground, 1 px line-strong, radius 12, elevation 3, padding 6, enters with 6 px rise + scale .98.
- Rows 32 px: mono command (100 px column) or icon + name, description ink-3, right kbd or source tag; selected row accent-soft + ink text; typed characters highlighted accent-ink 600. Sections: Built-in, Skills, Custom (with source tags). Footer hints ↑↓ move · ⇥ complete · esc.
- `@` picker sections: Files (fic icon, path ink-3), Symbols (ƒ), Worktrees (status dot), URLs/tabs (globe). Selecting inserts a chip; `$` opens skills.

### 4.3 Attachments and drop (card 42)
- Row 44 px: 32 px thumb tile (gradient placeholder or type icon), name 500, one meta line 11 ink-3, `x` at .4 (or Cancel / Retry xs). Uploading: thumb .5, name shimmer, 2 px accent progress hairline. Failed: danger border, danger meta text. Paste hint row dashed.
- Drop overlay covers the transcript pane: 8 px inset, 2 px dashed accent border, accent-soft ground, radius 12, centred 40 px accent tile with an arrow bobbing 4 px (1.4 s inout), "Drop to attach to this turn" 14 px + subtitle.

---

## 5. Workbench

### 5.1 Terminal (card 50)
- Pane tabs (in the shell header). Body on term-bg. Blocks, not a wall of text: each command is a `blk` (radius 8) with a 1-line command row (6 px exit dot success/danger/accent-with-ring while running, accent `$`, command mono 12, right: provider mark + "claude" 10 px when an agent ran it, duration mono 11 dim), then output lines (padding 2 10 8 26, ink-2, one element per line, nowrap + ellipsis), then a fold row after 8 lines ("38 lines folded"). Old blocks opacity .72; the live block gets surface-1 + 1 px line; failed blocks tint the command row danger-soft and keep output at full ink. Hovering a block shows copy and "ask the agent" xs ghost actions top-right.
- Restored-scrollback marker with hairlines. Prompt row 36 px surface-1: accent `$`, 7 × 14 accent cursor blinking, right context tags (branch, cwd). Splits ⌘D / ⌘⇧D animate on the layout spring.
- Terminal-only mode: pane toolbar 40 px (Chat | Terminal segmented, agent chips, "Runs in a PTY with your own login", split button); the agent's TUI rendered line by line 12.5/1.7; the TUI input box docked at the bottom (1 px line-strong, radius 6, padding 8 12) above a hint line and a 36 px status row (model, permission mode, branch as tags).
- ANSI 16 palette from tokens.

### 5.2 Browser and annotator (card 51)
- Tabs in the shell header. Nav row 38 px: back / forward (disabled .4) / reload, URL field 28 px surface-2 mono 12 with a shield in success and `⌘L`, Annotate mode toggle (ink-filled when on, with `esc`), screenshot and console icon buttons. Loading hairline 2 px accent at the bottom of the nav row.
- Annotate mode: cursor crosshair; hovering outlines the element (1 px accent at .6, accent-soft fill) with a tag label above-left (mono 10 on accent, "p · 392 × 34"); clicking drops a numbered pin (20 px teardrop, accent, white 11 px 600) at the element's top-right and opens a note popover (236 wide, element path mono 11 ink-3, textarea, Cancel / Save note xs). Selected element keeps a 1.5 px accent outline.
- Annotations side panel 272 px: header 36 px (edit icon, "Annotations · 3", x), rows (18 px numbered disc, element path mono 10.5 ink-3, note text 12 ink), pending row dashed disc; screenshot preview tile with the pins; metadata note (selector, box, outer HTML, computed styles, source file, URL, screenshot); action row: Clear ghost, "Send to ‹mark› Claude Code" primary. Agent-action overlay pill at the bottom-left ("Clicking 'Try free'" shimmer) while the agent drives the page.
- Because the webview is a native overlay, popovers must be positioned outside its bounds or rendered inside it.

### 5.3 Diff review (card 52)
- Top row 38 px: scope segmented control (This turn / Branch / Unstaged; 24 px segments, active surface-1 + elevation 1), summary "vs main · 3 files · +51 −7", spacer, Unified/Split segmented, search, "Collapse all".
- Files column 230 px: caps header, rows 26 px (M/A/D letter, name, note count badge 14 px accent, +/− mono right), selected surface-3; Notes list beneath.
- Diff body: sticky file header 30 px surface-2 (file icon, mono path, +/− pill, Open in editor, Stage); hunk rows; code rows with 36 + 26 px gutters, add/del grounds, word-level highlights (add-strong / del-strong); hover shows the 16 px accent "+" note affordance. Notes as in 3.8 — the same plain hairline card whether saved or pending; pending carries the textarea and the Cancel / Add note row, saved carries Edit / Delete. The border never changes.
- Bottom action row: "2 notes on 1 file" hint, spacer, Clear ghost, "Send to ‹mark› Claude Code" primary.

### 5.4 Git and PR (card 53)
- Changes panel 300 px: header 36 px with count pill; rows 26 px with 14 px checkbox (accent when checked), M/A letter, name, +/− right; commit area (1 px top border) with "Drafted from this turn · regenerate" accent-ink 11 px, 3-row textarea on surface-2, actions right-aligned (Amend ghost, Commit N files primary); ahead/behind row mono 11 with Push xs.
- Create PR form: caps labels 11 px; fields 30 px surface-2 (base branch select, title, description multi-line — prose with mono identifiers, block flow); checks list (ok glyph / spinner + name + duration); footer: Linear chip, spacer, Cancel ghost, Create PR primary.

### 5.5 File tree and document panes (card 54)
- Tree rows 24 px (the card's value; the spec said 26): chevron 10 px (rotates), `fic` icon 14 px, name 12 px (dirs ink 500, files ink-2), git badge right (M warning, A success, D danger, U ink-4, mono 10 600); selected accent-soft + ink (the card's value); indent 4 + 14 px per level. Footer status 28 px 11 px ink-3.
- Document pane (assistant): tabs in the shell header (doc kind icon 12 px muted, name); toolbar 34 px (the card's value) (style select, font select, B I U, list, link, image; spacer; "Ask" sparkle chip in accent-ink; Export); paper area surface-2 padding 18 with a white page (elevation 2, serif body 12.5/1.6; 520 wide in the card, 340 with 34/36 padding in the screens) and chat-made changes underlined with accent at .12 ground + 2 px accent underline; "Created in chat" strip (caps label + 26 px artifact chips with version tags, active one surface-3); status row 28 px (page, words, "3 changes from chat highlighted" accent-ink, Saved).
- PDF pane: toolbar with page stepper (mono "31 / 88"), zoom select, search, "cited as 1" line pill, Insert quote; page with the cited passage highlighted (accent .16 + 2 px outline); status "Highlight from citation 1 · Opened from chat".
- Sheet pane: formula bar 30 px (cell ref tile, formula mono, Ask chip); grid 26 px rows, 32 px row header column, header row surface-2 mono 10.5, numbers right-aligned tabular mono, selected cell 2 px accent outline + accent-soft; sheet tabs 30 px; created-in-chat strip; status row.

### 5.6 Sources and citations (card 55)
- Citation marker inline: 16 px min, radius 4, accent-soft ground, accent-ink mono 10 600, vertical-align +2; hover lifts 1 px (press spring) and fills accent with white text, opening the source card.
- Sources card: header 36 px (book icon, "Sources", "· 3 cited · 11 retrieved" ink-3, Show all xs ghost). Tier labels 11 px caps ink-3 with no swatch: Role · name / Project · name / Session. Rows: 18 px numbered square (accent-soft), title 500, meta 11.5 ink-3, confidence bar 36 × 4 (success fill) + value mono 10.5; uncited session row muted with "–".
- Source hover card 300 wide: overlay, radius 12, elevation 3; title 600; quote with a 2 px accent left rail and the matched span on accent-soft; actions Open page N (xs), Insert quote (xs ghost).

---

## 6. Screens (design/reference/screens)
- harness-Main: projects sidebar view, transcript with thought / activity / approval pending, block terminal on the right.
- harness-DiffReview: status sidebar view, summary card, diff pane with one saved and one pending note.
- harness-TerminalMode: date sidebar view, terminal-only centre with docked TUI input, file tree with type icons.
- harness-NewTask: dialog 680 wide (radius 16, elevation 3) over a 45% scrim: prompt textarea, Project / Base branch, Worktree / Agent chips, Model and mode / Fan out stepper, "Where should the agent run?" 2 × 2 radio cards (This laptop, build-box SSH, office-server, Fresh cloud VM with agent counts), action row with hint, Cancel, Create task ⌘↩.
- assistant-Main / Sources / Sheet: light theme, role sections, grounded answer with citations, docx / pdf / xlsx panes.

## 7. Data model the transcript renders (`aui-protocol`)
Session { id, agent, model, mode, cwd, branch, environment } → Turn (User { text, attachments, mentions } | Assistant { blocks, meta }) → Block: Text(stream), Thinking { text, elapsed, summary, state }, Activity { steps[], summary, elapsed, state }, ToolCall { kind: Shell|Read|Edit|Write|Search|Web|Browser|SubAgent|Mcp, target, status, duration, body }, Approval { tool, command, reason, scope, state, rule }, Question { prompt, options[], multi, answer }, Plan { items[], state }, Todo { items[] }, Summary { files[], checks[], cost }, Error { title, detail, retry }, Marker { kind, text }. The library exposes intents back to the app: Send, Queue, Stop, Approve(once|always|deny), Answer, AcceptPlan, OpenFile, OpenDiff, SendNotes, ChangeView, ToggleRightPane.
