# Agentic UI — Research & Plan

_Date: 2026-09-05. Status: draft for review. Source of truth for the component inventory; the HTML artifact is a rendered copy._

## 1. What we are building

Two macOS apps on one shared Rust/gpui UI library:

| App | Shape | Notes |
|---|---|---|
| **Harness** | Orca-style agent development environment | Sidebar of projects → worktrees/tasks; native streaming transcript; right workbench (terminal, browser, files, diff, editor, git). v1 mirrors onorca.dev. |
| **Assistant** | Day-job AI assistant | Same 3-pane shell. Right pane hosts browsers wired into chat plus editable docx / xlsx / pdf renders of files the chat creates. |

The library ("**aui**" as a working name, rename later) supplies: tokens + theme, motion engine, shell/dock/layout, sidebar, transcript + agent cards, composer, workbench panels, feedback/overlay primitives, gallery app.

## 2. Research summary

### 2.1 gpui (Zed's framework)
- Hybrid immediate/retained, GPU (Metal on macOS), Taffy flexbox/grid layout, Tailwind-style `Styled` API, `Entity<T>` + `Render` views, `Element` trait for custom low-level elements, `uniform_list`/`list` virtualization, `canvas`, `img`, `svg`, `anchored`/`deferred` for overlays, actions + keymap DSL, `FocusHandle`, `Task`/executors for async, `#[gpui::test]`.
- Animation: `Animation::new(Duration).with_easing(..).repeat()` + `element.with_animation(id, animation, |el, delta| ..)`; easing fns `linear`, `quadratic`, `ease_in_out`, `ease_out_quint`, `bounce`. It is a *time-driven re-render* model (delta 0→1), no springs, no enter/exit, no property interpolation. We build those.
- Versions: **gpui 0.2.2** (stable, Aug 2026) and **gpui-pre 0.3.3** (snapshot of zed main, Sep 3 2026). Pre-1.0; breaking changes are routine.
- Platform deps on macOS: Xcode (installed: 26.6), Metal, font-kit. Rust: stable 1.98.1 via rustup (present in `~/.cargo/bin`, not on the non-login PATH).

### 2.2 Component libraries on gpui
| Library | Version / base | What it gives us | Gaps |
|---|---|---|---|
| **gpui-kit** (Longbridge, ex gpui-component) | 0.6.0 on gpui-pre 0.3.1; Apache-2.0; 27 shipping apps | 60+ components: **Dock** (tabs/splits/edge docks/tiles, `DockAreaState` persistence, `DockSkin`), **Resizable**, **Sidebar**, **Command** palette, **TextView** (markdown/html, selectable, code-block actions, plugins), **Editor** (tree-sitter, LSP, 200K lines), **VirtualList**, **DataTable**, **Tree**, chat primitives **Message / Bubble / MessageScroller / Attachment / Marker** (tail-following virtual list with jump button + bottom fade, upload states, shimmer), Shimmer, Skeleton, Spinner, Notification, Dialog, Sheet, Popover, Menu, Tooltip, HoverCard, Charts, Theme (ActiveTheme, ThemeRegistry, gradient tokens, 20+ themes), design guide (spacing 2/4/8/12/16/24/32, sizes xs–lg). | No animation/transition system beyond gpui's; no agent-specific cards (tool call, approval, reasoning, diff), no terminal, no browser (separate crate). |
| **gpui-wry** | 0.6.0, same repo, gpui-pre 0.3.1 | Embed wry/WKWebView as an element (`WebView`, `WebViewElement`, `WebViewHandle`). | Native view overlays the GPU surface: nothing gpui-drawn can sit on top of it (popovers must avoid its bounds); experimental. |
| **guise-ui** | 1.5, MIT, gpui **0.2.2** | 130+ components, anime.js-style motion (Motion/Sequence/Stagger/Presence/Collapse, springs), AI chat kit (transcript, composer, streaming text, reasoning, tool calls, citations, cost meter), webview, DevTools. | Different gpui line than gpui-kit; 116 stars, single maintainer; no dock/editor of gpui-kit's depth. Good *reference* for motion API design. |
| **gpui-animation** | 0.2.63, gpui 0.2.2 | `with_transition`, `transition_on_hover/click`, `transition_when` interpolating bg/border/size/padding/opacity/radius/shadow/font. | Linear + custom easing only; gpui 0.2 line. Reference for property-tweening design. |
| **Zed `agent_ui`** | in-tree | Production gpui agent panel: thread view, tool cards, agent diff, message editor with @mentions, model/mode/profile selectors, terminal codegen. | Not a library; read for patterns. |

**Decision (proposed):** base on **gpui-pre 0.3.x + gpui-kit 0.6** (Dock, Editor, TextView, MessageScroller, Command, theme are exactly what we need and are maintained). Write our own **motion engine** (springs, presence, collapse, stagger, property tweens) since nothing on that gpui line provides it. Borrow API ideas from guise/gpui-animation/beui.

### 2.3 Reference apps
- **Orca** (Electron/React, MIT, 61.6k★). Layout: left sidebar (Tasks, Automations, Orca Mobile, Search; *Workspaces* grouped Pinned / In progress with counts; rows = status dot, name, repo chip, branch, agent-provider icons, elapsed, PR#, expander), centre = tab strip per worktree with split columns (agent session, terminal, another agent), right = file tree with icon toolbar, bottom status bar with provider usage meters (78% 5h · 94% wk). Session header: agent name + version, model + plan, cwd. Real app runs the agent **TUI inside a PTY**; the marketing site shows a *native* transcript (Read/Update cards, "Ran 1 shell command", diff snippets, ⠼ Thinking…). Other screens: Welcome-back dashboard (stats tiles, desktops, resume, quick actions, account usage), diff review with line notes → "Send notes to Claude Code / Codex", commit + Create PR forms, terminal splits (⌘D / ⌘⇧D), browser with design-mode element picker, "Where should the agent run?" (laptop / SSH / server / cloud VM), mobile pairing.
- **Synara**: task-owns-everything model (conversation, provider session, worktree, terminal, browser, diff, delivery), provider hand-off menu, split view of two threads, kanban.
- **T3 Code**: provider adapters → common event stream; inline approval cards; composer with `/commands`, `$skills`, images, model picker grouped by provider with favourites; per-turn / branch / unstaged diff scopes with line comments; terminal drawer; preview panel; ⌘↵ sends to background thread.

### 2.4 Web UI libraries (what to port, in gpui terms)
- **AI Elements**: Conversation, Message (+Response markdown, Actions, Branch, Toolbar), PromptInput, Reasoning, Tool, Task, Sources/InlineCitation, Context (usage), CodeBlock, Artifact, WebPreview, Image, Suggestion, Loader, Attachments (grid/inline/list + hover card), Canvas/Node/Edge workflow.
- **beui agents**: Prompt Input (auto-grow 2–8 rows, send↔stop icon morph with SPRING_SWAP 460/30/0.55, plus-icon 45° rotate, morphing popover), Agent Activity (reasoning/steps/search/tools/trace in a capped masked viewport, SPRING_LAYOUT 360/32, auto-collapse to summary), Tool Approval (7 states: pending→approving→approved/denied→running→complete/error; Allow once / Always allow / Deny; 220 ms ease-out entrance), Approval Card (choices + custom answer), Todo List (morphing status marks), Tool Result, Code Block (stable streaming), File Diff (progressive rows, live counts), Streaming Response, Citations, Loading States (shimmer / progress / cycling phrases), Message Scroller (reader-aware).
- **transitions.dev catalogue**: thinking states, reasoning stream, streaming text, spinner→check morph, icon swap, tabs sliding indicator, accordion, panel reveal, modal/toast/tooltip open-close, dropdown morph, checkbox check, toggle thumb, number pop-in, notification badge, skeleton reveal, error shake, success check, card resize, text state swap, input clear dissolve, gooey plus menu, drag & drop, banner stacking, matrix dot loader, shimmer/gradient text. Principle: composite (transform/opacity) over repaint.
- **beautifului.dev**: Approval Card, Thinking trace, Streaming text with sources, Task Rows, Tool Chips, Prompt Bar (@ sources, / commands, model, dictation), Records/Diff/Filter tables, Context cards, Recommendation card with confidence, Sidebar nav, Code block with unified diff, Agent screen.
- **gpui-kit design guide**: spacing xxs–xxl = 2/4/8/12/16/24/32; button frames ≈20/24/32; inputs to 44; type 12/14/16/18/20; semantic colour roles only for meaning; "motion explains change, it is not decoration"; alignment to the pixel is a quality invariant.

## 3. Architecture

```
agentic-ui/                      Cargo workspace
├─ crates/
│  ├─ aui-tokens      colour/space/radius/type/shadow/motion tokens; light+dark theme JSON for gpui-kit ThemeRegistry
│  ├─ aui-motion      spring solver, Transition (property tween on state change), Presence (enter/exit), Collapse,
│  │                  Stagger, Shimmer, IconMorph, NumberTicker, StreamReveal, reduced-motion switch
│  ├─ aui-icons       Lucide (via gpui-kit assets) + agent/provider/file-type/status icon set
│  ├─ aui             the component library (modules below); re-exports gpui-kit where we wrap it
│  ├─ aui-protocol    transport-agnostic session model: Session, Turn, Block (Text/Reasoning/ToolCall/Approval/
│  │                  Question/Plan/Error/Marker), streaming deltas, usage; adapters map Claude Code stream-json / ACP
│  ├─ aui-webview     wry/WKWebView pane wrapper (tabs, nav, JS bridge, element picker, screenshot → chat)
│  ├─ aui-terminal    PTY + alacritty_terminal grid rendered as a gpui Element (phase 3)
│  └─ aui-gallery     storybook app: every component × state, motion playground, theme switcher (also the
│                     source of the Claude Design design-system previews)
└─ apps/
   ├─ harness         Orca-like ADE
   └─ assistant       day-job assistant
```

`aui` module map: `shell` (window chrome, 3-pane, dock skin, panel chrome, status bar), `nav` (sidebar, project/task rows, filters), `transcript` (scroller, turns, blocks, cards), `composer`, `workbench` (terminal, browser, files, diff, git, editor, docs), `home` (dashboard), `tasks` (new-task, environment picker, handoff), `data` (status, badges, chips, meters, tables, timeline), `feedback` (toast, banner, skeleton, empty/error), `overlay` (dialog, sheet, popover, menu, command palette, tooltip, hover card).

## 4. Component inventory (everything to design and build)

Legend: **[kit]** wrap/skin gpui-kit · **[new]** build in aui · **[motion]** has a defined micro-interaction.

### 4.0 Foundations
1. Colour tokens [new]: bg/surface-0..3, fg/fg-muted/fg-subtle, border/border-strong, accent (+hover/active/soft), focus ring, selection; status success/warning/danger/info; **agent states** running / waiting-for-you / needs-approval / done / failed / idle; diff add/remove/modify; terminal ANSI 16; provider brand hues. Light + dark, hue-tinted neutrals.
2. Spacing 2/4/8/12/16/24/32 · radii xs 4 / sm 6 / md 8 / lg 12 / xl 16 / full · elevation 0–3 · type scale 11/12/13/14/16/18/20/24 (UI sans + mono) · density (compact/standard).
3. Motion tokens [motion]: durations fast 120 / base 180 / enter 220 / exit 160 / slow 280 ms; easings ease-out-cubic, ease-in-out, ease-out-quint; springs press (500/30/0.6), swap (460/30/0.55), layout (360/32/1), gentle (200/26/1); reduced-motion = 0 ms.
4. Motion primitives [new][motion]: Transition (tween bg/fg/border/opacity/size/padding/radius/shadow/transform on state), Presence (enter/exit with exit hold), Collapse (height 0↔auto), Stagger, Shimmer (text + surface), IconMorph (swap with scale/fade/±3 px y), NumberTicker, StreamReveal (per-chunk fade/rise), Pulse, Shake, Check-draw.
5. Typography primitives: Text, Label, Eyebrow (uppercase + tracking), Mono, Kbd [kit], Link [kit], Markdown view [kit TextView] with code-block actions, Heading scale.
6. Icons [new]: Lucide set + provider marks (Claude, Codex, Grok, Gemini, Cursor, Copilot, OpenCode…) + status glyphs + file-type glyphs.
7. Keyboard: keymap file, action registry, context predicates, shortcut hints; focus rings everywhere.
8. Theme switcher (system/light/dark) [kit ThemeRegistry] + theme JSON export for Claude Design.

### 4.1 App shell & layout
9. Window chrome (macOS traffic lights, draggable title region, sidebar toggle, back/forward, run/dev button, overflow menu) [kit TitleBar].
10. Three-pane shell [kit Resizable]: sidebar | centre | right; drag handles with hover highlight, collapse/expand with width tween [motion], persisted sizes, min/max, keyboard toggles ⌘B / ⌘J / ⌘⇧B.
11. Dock [kit Dock + DockSkin]: tabs, h/v splits, edge docks, drag-to-redock with drop-zone overlay [motion], zoom/maximise, close/restore, layout persistence per workspace.
12. Panel chrome [new]: header (icon, title, subtitle, actions), tab strip with sliding indicator [motion], overflow menu, drag handle, close.
13. Centre tab strip [new]: per-worktree tabs (icon, title, dirty/unread dot, close), "+" new, reorder drag, split column button.
14. Status bar [kit StatusBar]: provider usage meters (5h / wk) with animated fill [motion], connection state, notifications count, branch, sync.
15. Command palette ⌘K [kit Command]: sections (actions, worktrees, files, commands, recents), fuzzy, shortcut hints, open/close scale-fade [motion].
16. Quick open ⌘P (files / worktrees / repo context) [kit Command].
17. Toast stack [kit Notification + motion]: stacked cards, hover-expand, swipe/close, progress bar.
18. Banner (inline, stackable, dismissible) [new][motion].
19. Dialog / confirm [kit Dialog] with open-close spring; Sheet (side) [kit]; Drawer (bottom terminal drawer) [new].
20. Popover, Menu, Context menu, Dropdown [kit] with morph-from-trigger [motion]; Tooltip [kit] 0 delay in groups; Hover card [kit].
21. Empty states (illustrated, action), error states (retry), skeleton screens [kit Skeleton][motion reveal].
22. Settings screen (sections list + form) [kit Form/Settings].
23. Onboarding / welcome [new].

### 4.2 Sidebar / navigation
24. Sidebar container [kit Sidebar]: header (workspace switcher, add, filter, collapse), scroll region, footer (account/usage, help).
25. Primary nav items: Tasks, Automations, Search, Mobile/Devices, Inbox (needs-you count) [new].
26. Group header (Pinned / In progress / Done / project name) with count pill and collapse chevron [motion].
27. **Worktree/task row** [new][motion]: status dot (pulse when running), name, repo chip, branch, PR # chip, provider icon cluster, current-activity line (truncated, streaming update), elapsed/relative time, "needs you" badge, expander, hover actions (open, pin, terminal, browser, archive), selection state, unread indicator, drag reorder.
28. Child task tree (parent → children, "2 children", PR 1/2) [new].
29. Sidebar filters/search (Recent / Repo / Pinned / Active) [new].
30. Collapsed rail mode (icons + tooltips) [new][motion].
31. Account/usage footer: provider avatar, 5h / 7d meters, quota warnings [new].

### 4.3 Transcript (conversation pane)
32. Message scroller [kit MessageScroller]: virtualised, tail-follow, jump-to-latest pill [motion], bottom fade, prepend history, unread/needs-attention jump.
33. Session header [new]: agent name + version, model + plan chip, cwd, branch, environment chip (Local / SSH / Server / Cloud), permission mode, "hand off to…" menu.
34. System/marker rows [kit Marker]: date separators, session started, model changed, context compacted, handoff.
35. User turn [new]: bubble (secondary), attachments strip, @mention chips, edit / re-send / copy actions on hover [motion].
36. Assistant turn [new]: full-width ghost surface, markdown (TextView) with **streaming cursor + chunk reveal** [motion], incomplete-markdown tolerant, hover toolbar (copy, retry, fork/branch, bookmark), footer meta (model, duration, tokens, cost).
37. **Thinking / reasoning block** [new][motion]: shimmer "Thinking…" + elapsed timer, capped masked viewport that streams, collapsible, auto-collapses to a one-line summary when done, expand shows full trace.
38. **Agent activity group** [new][motion]: consecutive tool calls fold into "Ran 3 commands · read 2 files · edited 1 file" summary row with step glyphs; expandable timeline; live shimmer status while working.
39. **Tool call card (base)** [new][motion]: icon, verb + target (mono), status pill (pending / running / success / error / cancelled) with spinner→check morph, duration, chevron, collapsible body, copy, "open in panel" link.
40. Shell command card: `$ command`, live output tail (ANSI), exit code, expand full output, "Ran 1 shell command" collapsed label.
41. File read card: path, "Read 180 lines", peek.
42. File edit card: path, "+8 −3" counts, inline unified diff (gutter numbers, add/remove tint), open in Diff panel.
43. File write/create card; delete card.
44. Search/grep card: query, result rows path:line, count.
45. Web search / fetch card: query, result rows with favicon/title/domain, sources list.
46. Browser action card: navigate / click / fill / screenshot with thumbnail → lightbox.
47. Sub-agent / task card: nested collapsible transcript with its own status.
48. MCP / generic tool card: parameter definition list, JSON result viewer (tree, copy).
49. **Todo / plan list** [new][motion]: items with morphing status marks (pending → running → done), progress "3/7", collapsible.
50. **Approval card** [new][motion]: tool + description + parameters (collapsible), Allow once / Always allow / Deny buttons (spring press, 220 ms entrance), states pending → approving → approved / denied → running → complete / error, keyboard (Y / A / N), "remembered" note.
51. **Question card** [new][motion]: single / multi-select options with descriptions, Other free text, submit; answered state collapses to chip.
52. Plan proposal card: plan markdown, Accept / Edit / Reject.
53. Turn summary card: changed files list (M/A/D), tests run, PR link, "review diff" CTA.
54. Error card: API/tool error, retry, details.
55. Code block [new over TextView]: language label, line numbers, copy with check morph [motion], wrap toggle, expand-collapse for long blocks, filename header, "open in editor".
56. Diff block (inline) [new]: unified / split, hunk headers, add/remove, note affordance per line.
57. Tables, images (lightbox), attachments in messages [kit Attachment], citations / sources popover [new].
58. Context/usage meter (context window %, compaction warning) [new][motion fill].
59. Working status line ("Working… 12 s · esc to interrupt") with braille/dot spinner [motion].
60. Needs-you banner pinned above composer when approval/question pending [motion].
61. Message branch selector (< 1/2 >) [new].
62. Selection → "ask about this" floating action [new].

### 4.4 Composer
63. Auto-growing input (2→8 rows), placeholder, Enter / ⇧Enter / ⌘Enter-queue [kit Textarea + new].
64. Slash-command menu [new][motion]: `/` opens list (built-in, skills, custom) with descriptions + shortcuts, fuzzy, ↑↓ nav, sections.
65. @-mention menu + chips [new]: files, folders, symbols, worktrees, URLs, agents; drag file from tree drops a chip.
66. `$skill` attachment syntax [new].
67. Attachments row [kit Attachment]: image thumbs, files, progress, remove; drag-and-drop overlay over the pane [motion]; paste image.
68. Toolbar: **Actions "+" gooey menu** (attach, skill, context, screenshot) [motion], **model picker** (grouped by provider, search, favourites) [new], **mode selector** (Ask / Plan / Auto / Bypass permissions) [new], effort/thinking toggle, **agent picker / hand-off** [new], environment picker.
69. Send ↔ Stop icon morph button [motion]; queue indicator; queued messages list (edit/remove) [new].
70. Draft persistence, ↑ history, token count, dictation button (optional).
71. Suggestion chips (follow-ups) [new][motion stagger].
72. Inline meta strip (branch, context %, cost) above the box [new].

### 4.5 Workbench (right / centre panels)
73. Panel tab strip for the workbench (Terminal 1, Browser, Files, Diff, Editor…) [kit Dock tabs].
74. **Terminal** [new, phase 3]: tabs + splits (⌘D / ⌘⇧D) with split animation, GPU grid, scrollback restore, search, link detection, "session started" markers, agent-driven commands highlighted.
75. **Browser** [aui-webview]: tabs, nav bar (back / forward / reload / URL / recent), loading bar [motion], **design-mode element picker** (hover highlight → HTML/CSS/screenshot into composer), screenshot-to-chat, console/network drawer, agent-action overlay ("clicking Try free"), popovers avoid webview bounds.
76. **File explorer** [kit Tree]: worktree-aware, git status badges, drag to composer, quick actions, filter.
77. **Editor** [kit Editor]: tabs, tree-sitter, dirty state, find, go-to-line; LSP later.
78. **Diff viewer** [new]: scope switch (this turn / branch / unstaged), file list with M/A/D and +/−, unified/split, hunk collapse, **line notes** ("NOTE · LINE 43", add/cancel, batch) → "Send notes to Claude Code / Codex" [motion].
79. **Git panel** [new]: changes, stage, commit form (message, AI draft), ahead/behind, push/pull, **Create PR form** (base, title, description), PR status.
80. Markdown preview/editor, image preview, PDF preview [new].
81. Live processes list (dev servers, ports, logs) [new].
82. Usage / telemetry panel (agents spawned, agent time, PRs; per-provider quotas) [new].
83. **Assistant-only document panes**: docx, xlsx, pdf (and csv/md/pptx?) viewer-editors with "created in chat" artifact list, open-from-chat chip, edit-sync back to chat, version chips. (Implementation strategy is an open question, see §6.)

### 4.6 Home / dashboard / tasks
84. Welcome-back dashboard: stat tiles with NumberTicker [motion], desktops/hosts (connected / disconnected), Resume card, Tasks (GitHub · Linear), Quick actions, Account usage meters.
85. New task dialog: prompt, project, base branch, worktree name, agent + model, fan-out N, **"Where should the agent run?"** environment picker (laptop / SSH / server / cloud VM with agent counts) [new].
86. Task detail header, hand-off menu, automations list, GitHub / Linear task browser, inbox of needs-attention items, notifications centre.

### 4.7 Data-display primitives
87. Status dot / pill (pulse variants), Badge [kit], Chip/Tag [kit], Avatar + provider mark [kit], Kbd [kit], Progress bar/ring [kit], Meter (quota) [new], Sparkline [kit Chart], DataTable [kit], Tree [kit], Description list [kit], Timeline [new], Stat tile [new], Elapsed timer / relative time [new], Number ticker [motion], Breadcrumb [kit], Stepper [kit].

### 4.8 Feedback
88. Skeleton + shimmer reveal, spinners (ring, dots, braille), spinner→check morph, success check draw, error shake, toast/banner, confetti (PR created), empty/error states.

### 4.9 Gallery / docs
89. `aui-gallery`: sidebar of components, each with every state and a motion playground; theme + density switch; screenshot export used to seed Claude Design.

## 5. Delivery phases
0. **Bootstrap**: workspace, gpui-pre + gpui-kit hello world, gallery skeleton, tokens, light/dark themes, CI build.
1. **Foundations + shell**: motion engine, typography/icons, 3-pane + dock skin, sidebar rows, command palette, toasts, overlays.
2. **Transcript + composer**: protocol model, scroller, turns, reasoning, activity group, all tool cards, approval/question/plan cards, code/diff blocks, composer with slash/@/model/mode/send-stop.
3. **Workbench**: files, editor, diff + notes, git, browser (webview), terminal.
4. **Harness app**: sessions via Claude Code stream-json (and/or ACP), worktrees, home dashboard, new-task flow, persistence.
5. **Assistant app**: browser↔chat integration, document panes, artifact list.

Design happens before each build phase: Claude Design screens for phase 1–2 first (shell, sidebar, transcript, composer), then workbench, then app-specific screens.

## 6. Open questions (answers change the work)
1. **Base stack**: gpui-pre 0.3 + gpui-kit 0.6 (recommended) vs stable gpui 0.2.2 + guise/gpui-animation.
2. **Claude Design workflow**: (a) I draft a design canvas here (artboards you refine in the Claude Design canvas editor) and/or (b) I push the gallery as a Claude Design *design-system* project (DesignSync) and you compose screens in claude.ai/design, which I then implement.
3. **Transcript model**: native structured transcript (recommended; needs Claude Code `stream-json` / ACP events) vs Orca's approach of running the agent TUI inside a terminal.
4. **Agent backends for v1**: Claude Code only, or multi-provider via ACP from day one.
5. **Document editing in the assistant**: native gpui editors for docx/xlsx/pdf (very large) vs web editors (Univer for xlsx, ProseMirror/SuperDoc-style for docx, PDF.js/pdfium) inside the wry webview with a native file model (recommended). Which formats are must-have?
6. **v1 scope of Orca**: core (sidebar, transcript, terminal/browser/files/diff, review notes, commit/PR) vs everything (dashboard, SSH/cloud VMs, GitHub/Linear, mobile pairing, automations).
7. **Terminal**: build a GPU terminal element (alacritty_terminal, like Zed) in phase 3, or defer.
8. **macOS-only** (can use AppKit/WKWebView directly, simpler) vs keep cross-platform.
9. **Assistant domain**: what "manage my day job" covers (email, calendar, docs, tickets…) to shape its sidebar and cards.
10. Names for the library and the two apps; license; single monorepo.
