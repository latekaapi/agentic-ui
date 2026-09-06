# Parity process: making the gpui components match the design

The design is the contract. Three artefacts define it, in this order of authority:
1. `design/src/cards/*.html` + `design/src/base.css` + `design/tokens/tokens.css` (exact values).
2. `docs/02-component-spec.md` (anatomy, states, motion, keys, data) — corrected when it disagrees with the CSS.
3. `design/reference/cards/*.png` and `design/reference/screens/*.png` (what it must look like, rendered at the card's declared size).

## Per-component loop
1. Read the card HTML/CSS and the spec entry. List every state the card shows.
2. Implement the component in `aui` using tokens from `aui-tokens` and motion from `aui-motion`. No literal colours, sizes or durations in components.
3. Add a gallery entry in `aui-gallery` that reproduces the card: same states, same sample content, same frame size.
4. Render the gallery entry to PNG at the card's size (the gallery has a `--screenshot <entry> <out.png>` flag) and place it beside the reference PNG. Compare: alignment, sizes, colours, type, spacing. A one-pixel misalignment on an intended shared edge is a defect.
5. Exercise every interaction listed in the spec (hover, press, expand, stream, drag, keys) and check the motion: correct spring, correct duration, readable resting state, reduced-motion path.
6. Tick the row in the checklist below with the commit hash.

## Rules that every component must satisfy
- Light and dark both correct; theme switch is live.
- Keyboard operable; focus ring is the accent ring; focus restores after overlays.
- Text never clips: truncate with ellipsis or wrap in its own container.
- Class-name-style collisions do not exist in Rust, but the equivalent does: shared style helpers must be named for their purpose (a file-type icon helper must not be reused for a footer).
- Every component takes data in and emits intents out; no I/O inside `aui`.
- gpui paints an element's border *after* its children, so an absolutely positioned overlay can never sit on top of its own container's border. A per-side border colour (CSS `border-left: 2px solid var(--accent)` on an otherwise `--line-strong` box) has to be drawn as a wrapper whose left border carries the accent, with the card inside dropping its left border — not as a rail laid over the border. Line-note cards no longer need this: every one of them (cards 10, 37, 52) is a single box with a plain 1 px `line` border all the way round, matching the approval card. Pending and saved notes differ only in their actions.
- One implementation of the note card: `aui::transcript::diff_note` (and `diff_note_inset`, which takes the host's margins and body metrics). Card 10's mock in `aui-gallery` calls it rather than hand-building a note.

## Tooling
- `scripts/parity.sh <entry-id> <reference-stem>` builds the gallery, renders the entry, writes `target/parity/<stem>.png` and `<stem>-diff.png`, and prints the ImageMagick RMSE and the count of pixels that differ at 6 % fuzz. Card 01 sits at 0.7 % differing pixels; anything above ~2 % on a static card needs a look.
- `aui-gallery --screenshot-window <out.png>` captures the whole gallery window (chrome included) for shell reviews.
- References are re-rendered with headless Chrome when a design file changes: `python3 design/build.py`, then `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --window-size=<w>,<h> --virtual-time-budget=5000 --screenshot=design/reference/cards/<stem>.png "file://$PWD/design/dist/components/<group>/<stem>.html"`.

## Live behaviour (phase 4 step 2)

The `screens/assistant` mock is no longer a still. `assistant/model.rs` holds the
transcript (`Vec<Block>`: user turn, activity, answer, approval, question, file
card, sources) and `view.rs` renders only from it, so the three parity screens
and a running turn go through the same code. `assistant/script.rs` is a scripted
fake backend driven by `gpui` timers on the window executor.

- **Send.** `ComposerIntent::Send` appends the user's turn and picks a scenario
  from the text: a `?` asks a question card first; a prompt starting `write` /
  `update` / `change` asks for approval first; anything else runs the default
  scenario straight away.
- **Default scenario.** An activity group goes live for ~1.2 s with its three
  steps flipping to done one at a time, then it folds and an answer streams word
  groups at ~40 ms each through `CitedAnswer::streaming` (the model reveals a
  prefix; the caret is the component's own), then the status row reads "Done".
- **Stop.** `ComposerIntent::Stop` clears the answer's streaming flag, which
  freezes it at the group it had reached and ends the turn.
- **Approval.** Y / allow-once and A / always resolve the card, raise a
  "Saved vendor-scoring.xlsx" toast and continue into the default scenario; N
  denies it and answers "Left the sheet unchanged."
- **Question.** Single select, three options plus Other; choosing one collapses
  the card to an answered row and starts the default scenario.
- **Overlays.** The ⌘K palette ("Toggle right pane", "Collapse sidebar",
  "New session", which appends a session to the open project) and the composer's
  `+` menu both go through `overlay::popover_layer`. The composer's textarea is
  the resting focus; opening the palette moves the keyboard to it and closing
  restores whatever had it (`window.focused(cx)` on open, `window.focus` on
  close).
- **Toasts.** `feedback::ToastStack` in the shell's bottom-right corner, entering
  and leaving on the components' own presence, auto-dismissing after 4 s, with
  the timer held while the pointer is over the stack.
- **Sidebar views.** The `sidebar/views` entry (card 23) is live: the sliders
  button opens `nav::ViewMenu` through the popover layer and "Group by" switches
  the rendered grouping. The fourth column keeps the at-rest specimens the
  reference shows.

### Keys

Bound once in `aui::keys::bind`, which `aui::init` calls. Components declare a
`key_context` and handle the actions; nothing reads raw keystrokes.

| keys | action | context |
|---|---|---|
| `⌘B` | `ToggleSidebar` | anywhere |
| `⌘K` | `TogglePalette` | anywhere |
| `⌘\` | `ToggleRightPane` | anywhere |
| `↑` / `↓` | `SelectPrev` / `SelectNext` | `AuiMenu` (palette, view menu, question card) |
| `↩` | `Confirm` | `AuiMenu` |
| `esc` | `Cancel` | `AuiMenu`, `AuiApproval`, `AuiRoot` |
| `Y` / `A` / `N` | `ApproveOnce` / `ApproveAlways` / `Deny` | `AuiApproval` (the newest pending card, which takes focus when it appears) |
| `Tab` / `⇧Tab` | `FocusNext` / `FocusPrev` | `AuiRoot` |

`data::Button` and `data::icon_button` are tab stops and draw the accent focus
ring from `.btn:focus-visible` (`box-shadow:0 0 0 3px var(--accent-ring)`).

### `AUI_GALLERY_STEPS`

The gallery cannot receive keys from the command line, so the `screens/assistant`
entry accepts a comma-separated script that is applied after the first frame.
Every step is dispatched as the action or intent the UI itself would produce,
never straight into the model. Combine it with `--screenshot-delay <ms>` to
capture a frame mid-flight.

| step | what it does |
|---|---|
| `send:<text>` | puts `<text>` in the composer and sends it |
| `answer:<n>` | picks option `n` (1-based) on the pending question card |
| `wait:<ms>` | delays the script |
| `cmdk` / `cmdb` / `cmdright` | dispatches `TogglePalette` / `ToggleSidebar` / `ToggleRightPane` |
| `up` / `down` / `enter` / `esc` | dispatches `SelectPrev` / `SelectNext` / `Confirm` / `Cancel` |
| `tab` / `shift-tab` | dispatches `FocusNext` / `FocusPrev` |
| `approve` / `always` / `deny` | dispatches `ApproveOnce` / `ApproveAlways` / `Deny` |
| `stop` / `plus` | the composer's Stop and `+` intents |
| `shot` | a no-op marker for readability |

```bash
AUI_GALLERY_STEPS='send:write the weights,wait:600,approve,wait:1500,cmdk,wait:200,down,shot' \
  cargo run -p aui-gallery -- --screenshot screens/assistant /tmp/step.png --screenshot-delay 3000
```

The `sidebar/views` entry has its own small hook, `AUI_GALLERY_VIEWS_STEPS`,
documented in `crates/aui-gallery/src/cards/views.rs`.

## Known, accepted gaps (gpui cannot express these today)
- Caps labels lose their `.08em` letter-spacing: gpui has no letter-spacing text style.
- SVG `<text>` is not rasterised, so the `ft-ts` / `ft-tsx` glyph labels do not render until the sprite carries them as paths.
- Animated states (pulse rings, spinners, shimmer) are compared at their resting frame; the motion itself is checked by hand in the gallery.
- No element rotation: the drag ghost tab (card 11) is drawn straight instead of at −2°.
- Inline code inside prose runs at the body size (13.5 px) rather than 12 px, and without the 1 × 5 px padding: a `TextRun` cannot change size, and a run background hugs the glyphs. Bullet lists use an 18 px indent as designed.
- The composer embeds gpui-kit's multi-line input, which always pads its editor by 8 / 10 px and sets its own line box; the composer subtracts the padding and pushes the design's 14 px / 1.55 text onto it, landing within ~3 px of the CSS.
- In the slash menu the fixed 100 px command column breaks `/release-notes` mid-word where Chrome breaks at the hyphen.
- gpui rasterises Geist visibly heavier than headless Chrome; prose-heavy cards (31, 55) carry ~1 % of residue from that alone, with geometry within a pixel.
- Card 54's in-pane document tabs keep the design's accent bar on top (`DocTabs`); in the shell the pane tabs live in the header cell and use the ink indicator.
- The segmented control (card 52) fades its surface-1 thumb per segment instead of sliding one thumb: segments have text-measured widths gpui cannot read at build time.
- The streaming caret is drawn: the prose is wrapped in a relative holder, an `on_prepaint` child captures its bounds, and the last paragraph is re-shaped with `TextSystem::shape_text` at that wrap width so the caret lands after the last visual line (`transcript::last_paragraph_runs` hands out the same runs the prose paints). `CitedAnswer` is a wrapping row of word groups, so its caret is simply the last group.
- gpui wraps the document paper's Georgia body a word earlier than Chrome on a line that fills the measure to within a pixel (`assistant-Main`, section 2): the shaped run is ~1 % wider. Everything after that line sits one line lower, which is most of what is left in the screen numbers.
- The screen sources under `design/screens/` were an earlier draft of several cards and disagreed with them (row heights, composer padding, paper type scale, artifact chip and hover-card geometry, toolbar buttons). They are now aligned to `design/src/cards/*` — the card is authority #1 — and the references re-rendered with `design/screens/render.py`.
- `aui_motion::presence` was seen frozen mid-enter in a static capture (card 51's popover); it does request frames while running, so the cause is unconfirmed — static compositions use the components' `.at_rest()`.
- Syntax colours in code blocks come from a small lexer (keywords, calls, strings, numbers, comments); tree-sitter grammars are optional features of gpui-kit and not enabled yet.
- Tabs in a strip keep their natural width (the card lets `Terminal 1` wrap onto two lines when the strip is short of room); a strip short of room clips at its edge. Shrinking tabs with truncated labels made taffy collapse the labels entirely and is left for a later pass.
- CSS collapses the vertical margins between siblings; gpui does not. Rows carry a top margin only (`session_row`), which puts the last row 2 px closer to its container's bottom edge than the CSS.
- Parity renders open at the display's top-left corner so the pointer does not hover anything; keep the pointer away from that corner while `scripts/parity.sh` runs.
- The sprite carries four symbols the HTML cards never use: `spinner-ring` / `spinner-arc` (gpui rotates SVGs, not divs, so the spinner's accent arc is an SVG), `check-bold` / `x-bold` (the stroke-3 glyph marks), and `chev` (the stroke-2 chevron).

## Checklist (card → component → gallery entry → parity commit)
| Card | Component(s) | Gallery entry | Parity |
|---|---|---|---|
| 01 Colour | aui-tokens theme | foundations/colour | 0b8a535 · 0.7 % |
| 02 Typography | aui-tokens type | foundations/type | 0b8a535 · 1.8 % (tracking) |
| 03 Space/radius/elevation | aui-tokens | foundations/space | 0b8a535 · 0.6 % |
| 04 Motion | aui-motion | foundations/motion (playground) | 0b8a535 · 1.2 % (frames) |
| 05 Icons and marks | aui-icons | foundations/icons | 0b8a535 · 1.7 % (ts/tsx text) |
| 10 App shell | shell::AppShell, HeaderCells, DockedComposer (note via transcript::diff_note_inset) | shell/app-shell | dc89a4a · 1.0 % |
| 11 Panel chrome | shell::PanelHeader, TabStrip, DropZones | shell/panel-chrome | dc89a4a · 0.7 % (ghost rotation) |
| 12 Command palette | overlay::CommandPalette | shell/command-palette | a70ab8a · 0.4 % |
| 13 Toasts and banners | feedback::ToastStack, Banner | shell/toasts | a70ab8a · 1.1 % |
| 20 Worktree rows | nav::SessionRow | sidebar/rows | e8ceba3 · 1.4 % |
| 21 Sidebar | nav::Sidebar, Rail | sidebar/sidebar | e8ceba3 · 1.0 % |
| 22 Assistant sidebar | nav::RoleSections | sidebar/assistant | e8ceba3 · 1.6 % (tracking) |
| 23 Sidebar views | nav::SidebarView (status/project/date), ViewMenu | sidebar/views | e8ceba3 · 1.0 % |
| 30 Header identity and markers | shell::CentreHeader, transcript::MarkerRow | transcript/markers | a70ab8a · 1.0 % |
| 31 Turns | transcript::UserTurn, AssistantTurn, prose | transcript/turns | dc89a4a · 1.7 % (inline code size) |
| 32 Thinking | transcript::ThinkingBlock | transcript/thinking | a70ab8a · 1.1 % |
| 33 Activity group | transcript::ActivityGroup | transcript/activity | a70ab8a · 1.0 % |
| 34 Tool cards | transcript::ToolCard (+ bodies) | transcript/tool-cards | cc4d03f · 1.6 % (code indentation) |
| 35 Approval | transcript::ApprovalCard | transcript/approval | cc4d03f · 1.1 % (26 px tile kept) |
| 36 Question, plan, todo | transcript::QuestionCard, PlanCard, TodoList | transcript/question-plan-todo | cc4d03f · 1.0 % |
| 37 Code and diff blocks | transcript::CodeBlock, DiffBlock, DiffNote | transcript/code-diff | dc89a4a · 2.0 % (diff keeps code indentation the HTML collapsed) |
| 38 Summary, error, status | transcript::SummaryCard, ErrorCard, StatusRow, NeedsYouBanner, JumpPill | transcript/summary-status | cc4d03f · 0.9 % (design `.sm` collision fixed) |
| 40 Composer | composer::Composer (docked + floating), PlusMenu, queue, suggestions | composer/composer | dc89a4a · 2.3 % (kit textarea line box) |
| 41 Slash and mentions | composer::CommandMenu, MentionPicker | composer/menus | f94bc7c · 0.9 % |
| 42 Attachments | composer::AttachmentRow, DropOverlay | composer/attachments | f94bc7c · 1.1 % |
| 50 Terminal | workbench::BlockTerminal, TuiPane | workbench/terminal | 19bcd60 · 1.4 % (tab indicator moved to ink/bottom per the rules) |
| 51 Browser and annotator | workbench::BrowserNav, AnnotationsPanel, pins, NotePopover | workbench/browser | a447df3 · 1.0 % |
| 52 Diff review | workbench::DiffReview, Segmented | workbench/diff | dc89a4a · 1.6 % (code indentation) |
| 53 Git and PR | workbench::GitChanges, PrForm | workbench/git | 6157e0f · 1.0 % (description flows as prose; design fixed) |
| 54 Files and documents | workbench::FileTree, DocPane, ArtifactStrip (+ PdfPane, SheetPane used by the screens) | workbench/files-docs | dc89a4a · 1.2 % dark / 1.2 % light |
| 55 Sources and citations | workbench::Citation, CitedAnswer, SourcesCard, SourceHoverCard | transcript/citations | dc89a4a · 2.9 % (glyph weight; geometry within 1 px) |
| Screens (3 assistant) | the three assistant screens assembled live; `AUI_GALLERY_SCREEN=<Main\|Sources\|Sheet>` picks one | screens/assistant | dc89a4a · 2.5 % Main / 2.4 % Sources / 1.1 % Sheet (Geist and Georgia rasterise wider than Chrome, so the doc paper wraps one word early) |
| Screens page | the three assistant screens side by side at 1440×900, horizontally scrolling | screens/all | dc89a4a · no reference (a gallery page, not a design card) |
| Screens (4 harness) | harness screens not assembled | — | — |
