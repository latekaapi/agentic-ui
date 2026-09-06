---
name: feedback-design-taste
description: The user's design taste for the agentic-ui apps — calm, refined, gold-standard polish; specific patterns they asked for
metadata:
  type: feedback
---

Feedback from the first Claude Design review (2026-09-05):
- Wants "gold standard" polish and a visually calmer, less colourful UI. Colour only where something needs attention; muted pills/tags; no halos/glows; thin separators and surface steps instead of borders everywhere.
- Buttons must read as buttons (clear fill + border); one consistent action-row pattern on every card: hint left, spacer, secondary, primary on the right; buttons never wrap.
- App shell header must be split in three like the Claude Code desktop app: each column (sidebar / centre / right) owns its own header row, dividers run continuously top to bottom.
- Browser pane: NOT a "design mode" element picker. Wanted an annotator: hover → select element → add comment pin → list of annotations → send batch (selector, box, HTML, screenshot with pins) to the agent in the transcript.
- Static design cards must not contain mid-gesture artifacts (e.g. a floating drag ghost) without a label; it reads as cruft.
- Hates walls of text in terminals; block-based terminal design was welcomed.

**Why:** the user judges the product primarily on visual refinement; loud colour and inconsistent controls read as unfinished.
**How to apply:** before showing any new card or screen, check the four things above (calm colour, real buttons, action-row pattern, no unexplained artifacts). See [[project-decisions-2026-09-05]].

Round 2 feedback (2026-09-05, screens): assistant text and cards must span the full pane width; composer is docked full-width at the bottom (top hairline, no floating card); the right pane's tabs live in its header cell with one close control, and the centre header has only a pane toggle + overflow menu; file trees need muted file-type icons (ts/tsx/json/md/test/lock/folder); role rows use muted line icons with no coloured swatch, hierarchy via weight and ink level. Technical: the design canvas collapses newlines inside white-space:pre, so terminal/TUI output must be one element per line; never reuse generic class names (.in) across components in one page.

Round 3 (2026-09-05): no bottom status bar in either app (provider usage meter lives in the sidebar footer instead); assistant sidebar = bordered role sections in the SoloTerm style (44 px header row with chevron + muted icon, hairline-separated, group labels "Projects"/"Knowledge" with a rule and count), no background pill on the active role; only the active session gets a highlight. The user likes SoloTerm's sidebar as a reference.

Round 4 (2026-09-05): no session-header row in the transcript (provider mark goes in the centre header title; model/mode live in the composer); dialog accent = a plain 1 px border in the status colour (70% alpha), no halo/glow and no left edge — the user found the glow "too much"; composer "+" is a bordered button like the chips; the sidebar must support several groupings (status / project→sessions→children / date) behind a sliders "view options" menu (Status, Environment, Group by, Sort by, Show empty groups, Show PR status). User references: T3 Code, Amp, Chief, the Claude desktop app sidebar. Every shell change must be mirrored in the component library the same turn. Watch class-name collisions in base.css (.ft/.in bit us twice).

Round 5 (2026-09-05, first gpui gallery): light mode must render light (no card pinned to its design theme) and no extra frames around content. The 13 px design grid reads too small on the user's Mac; 110 % text ("Text: 110%") was "perfect" → baked as `text-scale: 1.1` in tokens.json (UI 14.3, body 14.85, spacing unchanged). Parity screenshots stay at 1.0.

**Added 2026-09-06 (bug-fix pass on Phase 3):**
- Inline note cards (diff notes, review notes, shell diff-panel note) are one plain hairline box: `border_1` in `line`, `R_SM`, `surface_2`. No left accent rail, no dashed "pending" variant. Design source and references were changed to match. **Why:** the user saw three different note styles and wants the same calm frame as the approval card. **How to apply:** any new inset card gets the plain hairline; accent rails only on quotes (card 55) and for nothing else without asking.
- Hover/selection tints must fade alpha only via `aui_motion::tint_fade` (never tween a colour to `transparent_black()`, which dips through dark green). Popovers and anything that overflows its box go through `aui::overlay::popover_layer`. **Why:** the palette hover looked like a flash; the composer menu sat under the card border.
