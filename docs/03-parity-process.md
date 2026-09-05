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

## Tooling
- `scripts/parity.sh <entry-id> <reference-stem>` builds the gallery, renders the entry, writes `target/parity/<stem>.png` and `<stem>-diff.png`, and prints the ImageMagick RMSE and the count of pixels that differ at 6 % fuzz. Card 01 sits at 0.7 % differing pixels; anything above ~2 % on a static card needs a look.
- `aui-gallery --screenshot-window <out.png>` captures the whole gallery window (chrome included) for shell reviews.
- References are re-rendered with headless Chrome when a design file changes: `python3 design/build.py`, then `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --window-size=<w>,<h> --virtual-time-budget=5000 --screenshot=design/reference/cards/<stem>.png "file://$PWD/design/dist/components/<group>/<stem>.html"`.

## Known, accepted gaps (gpui cannot express these today)
- Caps labels lose their `.08em` letter-spacing: gpui has no letter-spacing text style.
- SVG `<text>` is not rasterised, so the `ft-ts` / `ft-tsx` glyph labels do not render until the sprite carries them as paths.
- Animated states (pulse rings, spinners, shimmer) are compared at their resting frame; the motion itself is checked by hand in the gallery.
- No element rotation: the drag ghost tab (card 11) is drawn straight instead of at −2°.
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
| 10 App shell | shell::AppShell, HeaderCells, DockedComposer | shell/app-shell | e8ceba3 · 1.0 % |
| 11 Panel chrome | shell::PanelHeader, TabStrip, DropZones | shell/panel-chrome | e8ceba3 · 0.7 % (ghost rotation) |
| 12 Command palette | overlay::CommandPalette | shell/command-palette | |
| 13 Toasts and banners | feedback::ToastStack, Banner | shell/toasts | |
| 20 Worktree rows | nav::SessionRow | sidebar/rows | e8ceba3 · 1.4 % |
| 21 Sidebar | nav::Sidebar, Rail | sidebar/sidebar | e8ceba3 · 1.0 % |
| 22 Assistant sidebar | nav::RoleSections | sidebar/assistant | e8ceba3 · 1.6 % (tracking) |
| 23 Sidebar views | nav::SidebarView (status/project/date), ViewMenu | sidebar/views | e8ceba3 · 1.0 % |
| 30 Header identity and markers | shell::HeaderIdentity, transcript::Marker | transcript/markers | |
| 31 Turns | transcript::UserTurn, AssistantTurn, StreamText | transcript/turns | |
| 32 Thinking | transcript::ThinkingBlock | transcript/thinking | |
| 33 Activity group | transcript::ActivityGroup | transcript/activity | |
| 34 Tool cards | transcript::ToolCard (+ bodies) | transcript/tool-cards | |
| 35 Approval | transcript::ApprovalCard | transcript/approval | |
| 36 Question, plan, todo | transcript::QuestionCard, PlanCard, TodoList | transcript/question-plan-todo | |
| 37 Code and diff blocks | transcript::CodeBlock, DiffBlock, Note | transcript/code-diff | |
| 38 Summary, error, status | transcript::SummaryCard, ErrorCard, StatusRow, NeedsYouBanner, JumpPill | transcript/summary-status | |
| 40 Composer | composer::Composer (docked + floating), PlusMenu, SendStop | composer/composer | |
| 41 Slash and mentions | composer::CommandMenu, MentionPicker | composer/menus | |
| 42 Attachments | composer::AttachmentRow, DropOverlay | composer/attachments | |
| 50 Terminal | workbench::BlockTerminal, TuiPane | workbench/terminal | |
| 51 Browser and annotator | workbench::BrowserPane, Annotator | workbench/browser | |
| 52 Diff review | workbench::DiffReview | workbench/diff | |
| 53 Git and PR | workbench::GitPanel, PrForm | workbench/git | |
| 54 Files and documents | workbench::FileTree, DocPane, PdfPane, SheetPane | workbench/files-docs | |
| 55 Sources and citations | transcript::Citation, SourcesCard, SourceHover | transcript/citations | |
| Screens (7) | assembled in the assistant mock and gallery "screens" page | screens/* | |
