# API overview

The public surface of every library crate, one section per crate and one
subsection per module. Generated from rustdoc's HTML by `scripts/api-doc.py`;
do not edit by hand.

Regenerate with:

```
cargo doc --workspace --no-deps && python3 scripts/api-doc.py
```

`aui-gallery` is a binary demo and `aui-webview` / `aui-terminal` expose only a
`PHASE` marker constant, so none of the three appear here.

## `aui`

### `aui`

The Agentic UI component library. Two macOS apps share it: an Orca-like agent harness and a day-job assistant with the same three-pane shell.

- **fn** `init` — Initialises gpui-kit, installs the design tokens, fonts and theme, and binds the library’s keyboard actions (`keys`). Call once at application start, before opening any window.
  - `pub fn init(theme: ThemeKind, cx: &mut App)`

### `aui::assets`

The asset source an `aui` application installs: the design’s own glyphs (`icons/aui/*.svg`) layered over gpui-kit’s Lucide set (`icons/*.svg`).

- **struct** `AuiAssets` — Serves `aui-icons` first, then gpui-kit’s bundled icons.

### `aui::composer`

Composer: the auto-growing input with context chips, the `+` menu, model / mode / effort chips, the send ↔ stop morph, queued messages and suggestion chips (cards 40–42, spec §4). [...]

- **fn** `attachment_row` — A ready file row showing `name` over one line of `meta`.
  - `pub fn attachment_row(id: impl Into<ElementId>, name: impl Into<SharedString>, meta: impl Into<SharedString>) -> AttachmentRow`
- **fn** `command_menu` — The command menu at the width of its container. `query` is what has been typed so far (`/re`); its first occurrence in each command is drawn in accent-ink 600. [...]
  - `pub fn command_menu(id: impl Into<ElementId>, query: impl Into<SharedString>, sections: Vec<CommandSection>, selected: usize) -> CommandMenu`
- **fn** `composer` — A composer over `state` for `provider` / `model`.
  - `pub fn composer(id: impl Into<ElementId>, state: &Entity<TextareaState>, provider: Provider, model: impl Into<SharedString>) -> Composer`
- **fn** `composer_state` — Creates the text state a composer renders; keep it in the view (or in window state) and pass it to `composer` every frame.
  - `pub fn composer_state(placeholder: impl Into<SharedString>, window: &mut Window, cx: &mut Context<'_, TextareaState>) -> TextareaState`
- **fn** `composer_state_rows` — Like `composer_state` with explicit row bounds (the docked composer starts at one row).
  - `pub fn composer_state_rows(placeholder: impl Into<SharedString>, min_rows: usize, max_rows: usize, window: &mut Window, cx: &mut Context<'_, TextareaState>) -> TextareaState`
- **fn** `drop_overlay` — The overlay over the pane files are being dragged onto.
  - `pub fn drop_overlay(id: impl Into<ElementId>, visible: bool) -> DropOverlay`
- **fn** `effort_menu` — The reasoning-effort picker.
  - `pub fn effort_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu`
- **fn** `mention_picker` — The mention picker at the width of its container. `query` is what has been typed after the `@`; a row that carries no explicit `MentionItem::matched` range highlights the query wherever it occurs in the label. [...]
  - `pub fn mention_picker(id: impl Into<ElementId>, query: impl Into<SharedString>, sections: Vec<MentionSection>, selected: usize) -> MentionPicker`
- **fn** `mode_menu` — The approval-mode picker.
  - `pub fn mode_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu`
- **fn** `model_menu` — The model picker: rows from the provider catalog, `selected` on the row the session is actually using.
  - `pub fn model_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu`
- **fn** `plus_menu` — A menu over `items`; render it as a child of the `+` button’s holder.
  - `pub fn plus_menu(id: impl Into<ElementId>, items: Vec<PlusMenuItem>, open: bool) -> PlusMenu`
- **fn** `queue_row` — A queued message.
  - `pub fn queue_row(id: impl Into<ElementId>, text: impl Into<SharedString>) -> QueueRow`
- **fn** `queue_strip` — The strip for `rows`, newest last, in server order.
  - `pub fn queue_strip(id: impl Into<ElementId>, rows: Vec<QueueStripRow>) -> QueueStrip`
- **fn** `suggestion_chips` — Chips for `items`; the first carries the sparkle glyph.
  - `pub fn suggestion_chips(id: impl Into<ElementId>, items: Vec<SharedString>) -> SuggestionChips`

- **struct** `AttachmentRow` — One attachment row (`.a`). Build with `attachment_row`.
  - `pub fn glyph(self, glyph: IconName) -> Self` — Overrides the tile glyph the kind would pick (a PDF is a file).
  - `pub fn kind(self, kind: AttachmentKind) -> Self` — What the thumb tile shows: an image and a file sit on the gradient placeholder, a promoted text excerpt on the info tint.
  - `pub fn on_cancel(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — Abandon the upload (`Cancel`).
  - `pub fn on_remove(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — Drop the attachment (the `x` on a ready row).
  - `pub fn on_retry(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — Try the failed upload again (`Retry`).
  - `pub fn state(self, state: AttachmentRowState) -> Self` — Sets the row state.
  - `pub fn thumbnail(self, thumbnail: Arc<RenderImage>) -> Self` — A decoded preview for `AttachmentKind::Image`: drawn as the tile in place of the glyph, like the composer image chip.
- **struct** `CommandItem` — One row of the `/` menu.
  - fields: `id`, `command`, `description`, `key`, `source_tag`
  - `pub fn key(self, key: impl Into<SharedString>) -> Self` — Adds the keycap at the right edge.
  - `pub fn new(id: impl Into<SharedString>, command: impl Into<SharedString>, description: impl Into<SharedString>) -> Self` — A command row.
  - `pub fn source_tag(self, source_tag: impl Into<SharedString>) -> Self` — Adds the source tag at the right edge.
- **struct** `CommandMenu` — The `/` command menu. Build with `command_menu`.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the menu is drawn at rest on its first frame, for a static composition (the design card) rather than one the person just opened by typing `/`.
  - `pub fn on_hover(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — The pointer entered a row; the argument is its index across all sections, so the caller can move the selection to it.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A row was clicked; the argument is its `CommandItem::id`.
  - `pub fn present(self, present: bool) -> Self` — Whether the menu is open; `false` plays the exit.
- **struct** `CommandSection` — A titled block of command rows (`Built-in`, `Skills`, `Custom`).
  - fields: `title`, `items`
  - `pub fn new(title: impl Into<SharedString>, items: Vec<CommandItem>) -> Self` — A section with its header and rows.
- **struct** `Composer` — The composer. Build with `composer`.
  - `pub fn can_send(self, can_send: bool) -> Self` — Whether the send button is enabled (surface-3 / ink-4 otherwise).
  - `pub fn chip_menu(self, anchor: ComposerChipAnchor, menu: impl IntoElement) -> Self` — Hangs a picker off one of the toolbar chips. The menu renders inside that chip’s own positioning holder, so it stays anchored to the control that opened it however wide the composer is; the menu itsel [...]
  - `pub fn chips(self, chips: Vec<ComposerChip>) -> Self` — Context chips above the text.
  - `pub fn context(self, context: ContextMeterState) -> Self` — The context meter in the toolbar: the ring, the percentage (or the raw token count when the basis has no window) and the hover breakdown.
  - `pub fn context_open(self, open: bool) -> Self` — Pins the context meter’s breakdown open (or shut) instead of letting it follow the pointer — what the keyboard and a static capture both need.
  - `pub fn docked(self, docked: bool) -> Self` — The docked variant: full width, top hairline only, no radius or shadow.
  - `pub fn effort(self, effort: impl Into<SharedString>) -> Self` — The effort chip (brain glyph + level).
  - `pub fn focused(self, focused: bool) -> Self` — Draws the focused ring (accent border + 3 px accent-ring).
  - `pub fn knowledge_first(self, knowledge_first: bool) -> Self` — Leads the toolbar with what the answer is grounded in instead of the model: the mode chip moves before the model chip and reads as a knowledge chip — book glyph, chevron, never the quiet docked style. [...]
  - `pub fn meta(self, meta: ComposerMeta) -> Self` — The meta strip above the card.
  - `pub fn mode(self, mode: impl Into<SharedString>) -> Self` — The mode chip label.
  - `pub fn on_intent(self, f: impl Fn(ComposerIntent, &mut Window, &mut App) + 'static) -> Self` — Intent handler.
  - `pub fn plan(self, plan: bool) -> Self` — Draws the “Plan” pill in the toolbar; clicking it emits `ComposerIntent::ExitPlan`.
  - `pub fn plus_menu(self, open: bool, menu: Option<impl IntoElement>) -> Self` — Whether the `+` menu is open (rotates the button); pass the menu element too.
  - `pub fn streaming(self, streaming: bool) -> Self` — A turn is running: the send button shows stop.
- **struct** `ComposerChip` — A chip above the text.
  - fields: `id`, `kind`, `label`, `removable`, `thumbnail`, `detail`
- **struct** `ComposerMeta` — The optional strip above the card.
  - fields: `branch`, `context`, `cost`, `hint`
- **struct** `DropOverlay` — The drag-over overlay (`.ov`). Build with `drop_overlay`.
  - `pub fn at_rest(self) -> Self` — Skips the enter fade and scale (static captures).
  - `pub fn subtitle(self, subtitle: impl Into<SharedString>) -> Self` — The 12 px line under it, usually a file count.
  - `pub fn title(self, title: impl Into<SharedString>) -> Self` — The 14 px headline.
- **struct** `MentionItem` — One row of the `@` picker.
  - fields: `id`, `icon`, `label`, `matched`, `detail`, `detail_mono`, `key`
  - `pub fn detail_mono(self) -> Self` — Draws the detail line as mono 11 (a path, a `file:line`).
  - `pub fn key(self, key: impl Into<SharedString>) -> Self` — Adds the keycap at the right edge.
  - `pub fn matched(self, range: Range<usize>) -> Self` — Marks a byte range of the label as the query match.
  - `pub fn matching(self, needle: &str) -> Self` — Marks the first occurrence of `needle` in the label as the query match.
  - `pub fn new(id: impl Into<SharedString>, icon: MentionIcon, label: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self` — A mention row with a UI-face detail line.
- **struct** `MentionPicker` — The `@` mention picker. Build with `mention_picker`.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the picker is drawn at rest on its first frame, for a static composition (the design card).
  - `pub fn on_hover(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — The pointer entered a row; the argument is its index across all sections.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A row was clicked; the argument is its `MentionItem::id`.
  - `pub fn present(self, present: bool) -> Self` — Whether the picker is open; `false` plays the exit.
- **struct** `MentionSection` — A titled block of mention rows (`Files in acme-web`, `Symbols`).
  - fields: `title`, `items`
  - `pub fn new(title: impl Into<SharedString>, items: Vec<MentionItem>) -> Self` — A section with its header and rows.
- **struct** `PickerMenu` — A composer chip menu. Build with `model_menu`, `effort_menu` or `mode_menu`.
  - `pub fn at_rest(self) -> Self` — Skips the enter (static captures).
  - `pub fn on_close(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — A click outside the menu.
  - `pub fn on_hover(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — The pointer entered a row; the argument is its index, so the caller can move the selection to it and keep one highlight on screen.
  - `pub fn on_pick(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A row was activated; the argument is its `PickerRow::id`.
  - `pub fn title(self, title: impl Into<SharedString>) -> Self` — Replaces the caps header.
- **struct** `PickerRow` — One row of a composer picker.
  - fields: `id`, `label`, `detail`, `meta`, `badges`
  - `pub fn badge(self, badge: impl Into<SharedString>) -> Self` — Adds one badge at the right edge.
  - `pub fn meta(self, meta: impl Into<SharedString>) -> Self` — The trailing mono fact.
  - `pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self` — A row with a label and a description.
- **struct** `PlusMenu` — The menu. Build with `plus_menu`.
  - `pub fn at_rest(self) -> Self` — Skips the enter (static captures).
  - `pub fn on_activate(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — Activation handler.
- **struct** `PlusMenuItem` — One row of the menu.
  - fields: `id`, `icon`, `label`, `key`
  - `pub fn key(self, key: impl Into<SharedString>) -> Self` — Adds the keycap.
  - `pub fn new(id: impl Into<SharedString>, icon: IconName, label: impl Into<SharedString>) -> Self` — An item.
- **struct** `QueueRow` — A queued message row. Build with `queue_row`.
  - `pub fn editing(self) -> Self` — The row whose text is in the composer, waiting for the server’s `turn/unqueued` to take it out of the queue. The badge says so; nothing is removed until the wire says it was.
  - `pub fn on_intent(self, f: impl Fn(QueueIntent, &mut Window, &mut App) + 'static) -> Self` — Intent handler.
- **struct** `QueueStrip` — The queued submissions above the docked composer. Build with `queue_strip`.
  - `pub fn on_intent(self, f: impl Fn(&SharedString, QueueIntent, &mut Window, &mut App) + 'static) -> Self` — Intent handler; the first argument is the row’s `QueueStripRow::id`.
- **struct** `QueueStripRow` — One row of a `QueueStrip`: the server’s turn id and the text it queued.
  - fields: `id`, `text`, `editing`
  - `pub fn editing(self) -> Self` — Marks the row as the one being edited.
  - `pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self` — A queued row.
- **struct** `SuggestionChips` — Suggestion chips. Build with `suggestion_chips`.
  - `pub fn at_rest(self) -> Self` — Skips the staggered enter (static captures).
  - `pub fn on_pick(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — Pick handler with the chip index.

- **enum** `AttachmentRowState` — Where one attachment row is in its life.
  - variants: `Ready`, `Uploading`, `Failed`, `Hint`
  - `pub fn from_upload(state: &UploadState) -> Self` — Maps the protocol’s `UploadState` onto a row state.
- **enum** `ComposerChipAnchor` — Which toolbar control a `Composer::chip_menu` hangs off.
  - variants: `Model`, `Mode`, `Effort`
- **enum** `ComposerChipKind` — What a context chip stands for.
  - variants: `Mention`, `Image`, `File`, `Skill`
- **enum** `ComposerIntent` — What the composer asks for.
  - variants: `Send`, `Stop`, `TogglePlus`, `Model`, `Mode`, `Effort`, `RemoveChip`, `Steer`,
    `ExitPlan`, `Attach`, `Compact`
- **enum** `MentionIcon` — The leading glyph of a mention row.
  - variants: `Glyph`, `Symbol`, `Dot`
- **enum** `QueueIntent` — What a queue row asks for.
  - variants: `Edit`, `Remove`, `Steer`

- **const** `ROW_STACK_GAP` — `.att{gap:6px}` — the gap between rows in the column.
  - `pub const ROW_STACK_GAP: f32 = 6.0;`

### `aui::data`

Shared data-display primitives every card is built from (`base.css`): buttons, chips, pills, tags, status dots, kbd, avatars, status glyphs, the provider usage meter and the context-window meter. [...]

- **fn** `avatar` — The person’s initial in a circle.
  - `pub fn avatar(initial: impl Into<SharedString>) -> Avatar`
- **fn** `button` — A labelled button (secondary by default).
  - `pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button`
- **fn** `chip` — A chip with a label.
  - `pub fn chip(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Chip`
- **fn** `context_meter` — The meter for `state`; hover reveals the breakdown.
  - `pub fn context_meter(id: impl Into<ElementId>, state: ContextMeterState) -> ContextMeter`
- **fn** `glyph_err` — The error glyph.
  - `pub fn glyph_err() -> Glyph`
- **fn** `glyph_ok` — The success glyph.
  - `pub fn glyph_ok() -> Glyph`
- **fn** `icon_button` — A square icon button (`.btn.icon`), secondary by default; call `Button::ghost` for the quiet header / row kind.
  - `pub fn icon_button(id: impl Into<ElementId>, glyph: IconName) -> Button`
- **fn** `kbd` — A keycap such as `esc` or `⌘K`.
  - `pub fn kbd(keys: impl Into<SharedString>) -> Kbd`
- **fn** `pill` — A quiet pill.
  - `pub fn pill(label: impl Into<SharedString>) -> Pill`
- **fn** `secret_field` — The caller’s `state` in the library’s bordered control box, with a trailing ghost eye button that reports back through `.on_toggle_reveal`.
  - `pub fn secret_field(id: impl Into<ElementId>, state: &Entity<InputState>) -> SecretField`
- **fn** `spinner` — A spinning ring; `id` keys its rotation.
  - `pub fn spinner(id: impl Into<ElementId>) -> Spinner`
- **fn** `status_dot` — A dot in the state’s colour.
  - `pub fn status_dot(id: impl Into<ElementId>, state: AgentState) -> StatusDot`
- **fn** `tag` — Mono 10.5 / 500 / ink-3.
  - `pub fn tag(text: impl Into<SharedString>) -> Tag`
- **fn** `usage_meter` — `fraction` in `0..=1`.
  - `pub fn usage_meter(provider: Provider, fraction: f32) -> UsageMeter`

- **struct** `Avatar` — An avatar. Build with `avatar`.
  - `pub fn size(self, size: impl Into<Pixels>) -> Self` — Overrides the diameter (the rail uses 24); the initial scales with it.
- **struct** `Button` — A button. Build with `button` or `icon_button`.
  - `pub fn danger(self) -> Self` — Outlined in danger.
  - `pub fn disabled(self, disabled: bool) -> Self` — Non-interactive at 45 % opacity.
  - `pub fn ghost(self) -> Self` — Quiet, transparent.
  - `pub fn icon(self, glyph: IconName) -> Self` — A leading glyph before the label.
  - `pub fn icon_size(self, size: impl Into<Pixels>) -> Self` — Overrides the glyph size (the shell header uses 12 px glyphs in 20 px buttons).
  - `pub fn muted(self) -> Self` — Ghost buttons in headers are ink-3 instead of ink-2 (`.hd .btn.ghost`).
  - `pub fn on_click(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Click handler.
  - `pub fn outline(self) -> Self` — Outlined, no fill.
  - `pub fn primary(self) -> Self` — Accent fill.
  - `pub fn size(self, size: ButtonSize) -> Self` — Sets the height.
  - `pub fn sm(self) -> Self` — 24 px.
  - `pub fn trailing(self, element: impl IntoElement) -> Self` — Something after the label: a kbd, a chevron.
  - `pub fn variant(self, variant: ButtonVariant) -> Self` — Sets the variant.
  - `pub fn xs(self) -> Self` — 20 px.
- **struct** `Chip` — A chip. Build with `chip`.
  - `pub fn accent(self) -> Self` — Accent-ink text (the “+ add” chip).
  - `pub fn active(self, active: bool) -> Self` — Selected: ink text and the line-strong border.
  - `pub fn chevron(self) -> Self` — A trailing chevron-down, for chips that open a menu.
  - `pub fn composer(self) -> Self` — The 28 px composer toolbar size.
  - `pub fn detail(self, detail: impl Into<SharedString>) -> Self` — A muted suffix after the label (a file chip’s kind and size).
  - `pub fn icon(self, glyph: IconName) -> Self` — A leading 11 px glyph.
  - `pub fn leading(self, element: impl IntoElement) -> Self` — Any leading element (a provider mark, a file-type icon).
  - `pub fn on_click(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Click handler.
  - `pub fn trailing(self, element: impl IntoElement) -> Self` — A trailing element (the remove `x` of a context chip).
- **struct** `ContextMeter` — The context meter. Build with `context_meter`.
  - `pub fn on_compact(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The Compact action, in the breakdown and — when the pressure is `ContextPressure::Blocked` — inline beside the number.
  - `pub fn open(self, open: bool) -> Self` — Forces the breakdown open (or shut) instead of following the pointer — what a static capture and the keyboard both need.
- **struct** `ContextMeterState` — Everything the meter draws.
  - fields: `used_tokens`, `window_tokens`, `pressure`, `prompt_tokens`, `output_tokens`,
    `total_tokens`
  - `pub fn fraction(&self) -> Option<f32>` — Occupancy in `0..=1`, or `None` when there is no denominator.
  - `pub fn label(&self) -> String` — The label beside the ring: `62%`, or `19.3k tokens` with no window.
- **struct** `Glyph` — A status glyph. Build with `glyph_ok` / `glyph_err`.
  - `pub fn size(self, size: impl Into<Pixels>) -> Self` — Overrides the diameter; the mark scales with it.
- **struct** `Kbd` — A keycap. Build with `kbd`.
- **struct** `Pill` — A pill. Build with `pill`.
  - `pub fn font_size(self, size: f32) -> Self` — Overrides the 11 px text (tab badges use 9).
  - `pub fn height(self, height: f32) -> Self` — Overrides the 20 px height (the assistant footer uses 16).
  - `pub fn leading(self, element: impl IntoElement) -> Self` — A leading element (dot, glyph).
  - `pub fn padding_x(self, pad: f32) -> Self` — Overrides the 7 px horizontal padding.
  - `pub fn variant(self, variant: PillVariant) -> Self` — Sets the status variant.
- **struct** `SecretField` — A masked single-line field for `state`. Build with `secret_field`.
  - `pub fn disabled(self, disabled: bool) -> Self` — Non-interactive at 45 % opacity; passed through to the caller’s state when drawn.
  - `pub fn on_toggle_reveal(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The eye button was pressed. The component never flips the masked flag itself: read the presentation snapshot and call `set_masked` on the caller’s state.
  - `pub fn placeholder(self, text: impl Into<SharedString>) -> Self` — The greyed hint shown while the field is empty; passed through to the caller’s state when drawn.
- **struct** `Spinner` — The spinner. Build with `spinner`.
  - `pub fn size(self, size: impl Into<Pixels>) -> Self` — Overrides the diameter (rows use 10 and 11).
- **struct** `StatusDot` — A status dot. Build with `status_dot`.
  - `pub fn pulse(self, pulse: bool) -> Self` — Adds the expanding ring (`.dot.pulse`).
  - `pub fn size(self, size: impl Into<Pixels>) -> Self` — Overrides the diameter (the rail uses 8).
- **struct** `Tag` — A tag. Build with `tag`.
  - `pub fn color(self, color: Hsla) -> Self` — Overrides ink-3 (a `+8` in success, a `−3` in danger).
  - `pub fn truncate(self, max_width: f32) -> Self` — Truncates with an ellipsis at `max_width` px (`.tag.trunc{max-width:…}`).
- **struct** `UsageMeter` — The usage meter. Build with `usage_meter`.

- **enum** `ButtonSize` — `.btn` heights: md 28 (padding 11, 13 px), sm 24 (9, 12 px), xs 20 (7, 11 px).
  - variants: `Md`, `Sm`, `Xs`
- **enum** `ButtonVariant` — `.btn` variants.
  - variants: `Secondary`, `Primary`, `Ghost`, `Outline`, `Danger`
- **enum** `ContextPressure` — How much context pressure the server reports.
  - variants: `Normal`, `Warning`, `Blocked`
- **enum** `GlyphKind` — Which status a glyph shows.
  - variants: `Ok`, `Err`
- **enum** `PillVariant` — `.pill.*` variants.
  - variants: `Quiet`, `Accent`, `Success`, `Warning`, `Danger`, `Info`, `Line`

### `aui::feedback`

Feedback: toasts and banners (card 13).

- **fn** `banner` — A one-line banner in `kind`’s colours, carrying `runs` as its message.
  - `pub fn banner(id: impl Into<ElementId>, kind: BannerKind, runs: Vec<BannerRun>) -> Banner`
- **fn** `toast` — A toast card that fills the width it is given.
  - `pub fn toast(id: impl Into<ElementId>, data: &ToastData) -> Toast`
- **fn** `toast_stack` — The toast stack: `toasts` oldest first, so the last one is the newest and is drawn in front at full size.
  - `pub fn toast_stack(id: impl Into<ElementId>, toasts: Vec<ToastData>) -> ToastStack`

- **struct** `Banner` — An inline banner. Build with `banner`.
  - `pub fn action(self, label: impl Into<SharedString>, style: BannerActionStyle) -> Self` — The single xs button at the right end of the row.
  - `pub fn on_action(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The button was pressed.
  - `pub fn on_secondary(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The second button was pressed.
  - `pub fn secondary_action(self, label: impl Into<SharedString>, style: BannerActionStyle) -> Self` — A second xs button, drawn to the left of `Banner::action`.
- **struct** `Toast` — A single toast. Build with `toast`.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the toast is drawn at rest on its first frame, for a static composition (the design card, a restored stack) rather than one that just arrived.
  - `pub fn on_action(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — An action button was pressed; the argument is `ToastAction::id`.
  - `pub fn on_close(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The close glyph was pressed.
  - `pub fn present(self, present: bool) -> Self` — Whether the toast is on screen; `false` plays the 160 ms fade out.
- **struct** `ToastAction` — One xs button in a toast’s action row.
  - fields: `id`, `label`, `primary`
  - `pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self` — A secondary action.
  - `pub fn primary(self) -> Self` — Marks the action as the primary (accent) one.
- **struct** `ToastData` — Everything a toast draws. The app owns these; the component is stateless.
  - fields: `id`, `kind`, `title`, `body`, `actions`, `progress`
  - `pub fn action(self, action: ToastAction) -> Self` — Adds an action button.
  - `pub fn height(&self, text_scale: f32) -> f32` — The toast’s resting height, derived from its CSS box: 10 px padding top and bottom, the 13/1.3 title, the 2 px row gap, the 12/1.5 body, the 1 px borders and — when it carries actions — 10 px plus the 20 px xs control. [...]
  - `pub fn kind(self, kind: ToastKind) -> Self` — Sets the icon tile.
  - `pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>, body: impl Into<SharedString>) -> Self` — A toast with a title and a body line.
  - `pub fn progress(self, progress: f32) -> Self` — Shows the auto-dismiss hairline at `progress` (`0..=1`).
- **struct** `ToastStack` — The stack of toasts. Build with `toast_stack`.
  - `pub fn at_rest(self) -> Self` — Draws every toast at rest on the first frame, with no enter.
  - `pub fn hovered(self, hovered: Option<bool>) -> Self` — Forces the fanned-out state on or off instead of following the pointer (`None`, the default). A gallery or a test can hold the stack open.
  - `pub fn on_action(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — An action button was pressed on one of the toasts; the argument is the `ToastAction::id`.
  - `pub fn on_close(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — A close glyph was pressed.

- **enum** `BannerActionStyle` — How the banner’s single button is drawn.
  - variants: `Primary`, `Secondary`, `Ghost`
- **enum** `BannerKind` — Which state a banner reports. The kind picks the icon, the ink and whether the row is tinted.
  - variants: `Waiting`, `Info`, `Error`, `Success`
- **enum** `BannerRun` — One run of a banner’s message. A banner line mixes weights and faces (“Waiting for you. Allow `pnpm test` to run?”).
  - variants: `Text`, `Bold`, `Mono`
- **enum** `ToastKind` — The icon tile of a toast: neutral, success or warning.
  - variants: `Neutral`, `Ok`, `Warn`

### `aui::keys`

The library’s keyboard actions and their default bindings.

- **fn** `bind` — Installs the default bindings. Called by `crate::init`; an application that wants a different keymap can rebind the same actions afterwards.
  - `pub fn bind(cx: &mut App)`
- **fn** `keyboard_nav` — Whether the keyboard, rather than the mouse, last moved the focus.
  - `pub fn keyboard_nav(cx: &mut App) -> bool`
- **fn** `set_keyboard_nav` — Arms or disarms the keyboard-focus flag. Call it with `true` from anything that moves focus with the keyboard; the focus ring follows.
  - `pub fn set_keyboard_nav(on: bool, cx: &mut App)`
- **fn** `track_pointer` — Keeps the `keyboard_nav` flag honest for a whole window: any mouse press anywhere under `el` disarms it, and any key press re-arms it. [...]
  - `pub fn track_pointer<E: InteractiveElement>(el: E) -> E`

- **struct** `ApproveAlways` — Allow it and remember the rule.
- **struct** `ApproveOnce` — Allow the pending request once.
- **struct** `Cancel` — Close the overlay that has the keyboard.
- **struct** `ChooseNth` — Pick the n-th choice of a card that carries a server-minted choice list.
  - fields: `index`
- **struct** `Confirm` — Run the highlighted row of a menu or the palette.
- **struct** `Deny` — Refuse the pending request.
- **struct** `FocusNext` — Move the keyboard to the next tab stop.
- **struct** `FocusPrev` — Move the keyboard to the previous tab stop.
- **struct** `SelectNext` — Move the highlight down one row.
- **struct** `SelectPrev` — Move the highlight up one row.
- **struct** `TogglePalette` — Open or close the command palette.
- **struct** `ToggleRightPane` — Open or close the right pane.
- **struct** `ToggleSidebar` — Swap the sidebar for its collapsed rail, and back.

- **const** `APPROVAL_CONTEXT` — The context a pending approval card puts on itself while it is focused: it owns Y, A and N.
  - `pub const APPROVAL_CONTEXT: &str = "AuiApproval";`
- **const** `MENU_CONTEXT` — The context a menu, picker or the command palette puts on itself while it holds the keyboard: it owns the arrows, return and escape.
  - `pub const MENU_CONTEXT: &str = "AuiMenu";`
- **const** `ROOT_CONTEXT` — The context a screen puts on its outermost element: it owns Tab and the escape that closes whatever is open.
  - `pub const ROOT_CONTEXT: &str = "AuiRoot";`

### `aui::nav`

Sidebar: session rows in every state, the sidebar and its collapsed rail, the three groupings (status / project / date) with the view-options menu, and the assistant’s role sections (cards 20–23, spec [...]

- **fn** `chevron` — A `.chev` glyph rotated 0° (closed) → 90° (open) on the swap spring, at the default 12 px.
  - `pub fn chevron(id: impl Into<ElementId>, open: bool, color: Hsla, window: &mut Window, cx: &mut App) -> impl IntoElement`
- **fn** `chevron_sized` — `chevron` at an explicit `size` in design px, for the rows whose CSS narrows the glyph (`.pj .chev` 11 px, the file tree’s `.n .chev` 10 px).
  - `pub fn chevron_sized(id: impl Into<ElementId>, open: bool, color: Hsla, size: f32, window: &mut Window, cx: &mut App) -> impl IntoElement`
- **fn** `compact_session_row` — A compact row for `session`; children nest beneath with a hairline rail.
  - `pub fn compact_session_row(id: impl Into<ElementId>, session: SessionSummary) -> CompactSessionRow`
- **fn** `date_group_header` — `TODAY`, `YESTERDAY`, `THIS WEEK` with the hairline rule after them.
  - `pub fn date_group_header(label: impl Into<SharedString>) -> DateGroupHeader`
- **fn** `dense_field` — The dense single-line field for `CompactSessionRow::editor`: the row-title size, no appearance and no border, fixed to one line. [...]
  - `pub fn dense_field(state: &Entity<TextareaState>) -> Textarea`
- **fn** `folder_drop_card` — A card reading “Drop a folder here” / “or click to choose one”.
  - `pub fn folder_drop_card(id: impl Into<ElementId>) -> FolderDropCard`
- **fn** `group_header` — A group header (`Pinned 3`, `In progress 17`).
  - `pub fn group_header(id: impl Into<ElementId>, label: impl Into<SharedString>, open: bool) -> GroupHeader`
- **fn** `group_row` — A caps group row.
  - `pub fn group_row(id: impl Into<ElementId>, label: impl Into<SharedString>) -> GroupRow`
- **fn** `knowledge_card` — A knowledge card for `role_id` listing `sources`.
  - `pub fn knowledge_card(id: impl Into<ElementId>, role_id: impl Into<SharedString>, sources: Vec<SharedString>) -> KnowledgeCard`
- **fn** `nav_item` — `Tasks`, `Automations`, `Inbox`…
  - `pub fn nav_item(id: impl Into<ElementId>, glyph: IconName, label: impl Into<SharedString>) -> NavItem`
- **fn** `project_group_row` — A project row: chevron, folder mark, name, count.
  - `pub fn project_group_row(id: impl Into<ElementId>, name: impl Into<SharedString>, count: impl Into<SharedString>, open: bool) -> ProjectGroupRow`
- **fn** `project_mark` — A rounded square in `colour` with `initial` centred in the theme background colour. Stateless; no click handling — the caller owns the interaction.
  - `pub fn project_mark(initial: impl Into<SharedString>, colour: Hsla) -> ProjectMark`
- **fn** `project_row` — A project row: folder glyph, name, session count.
  - `pub fn project_row(id: impl Into<ElementId>, project: &Project) -> ProjectRow`
- **fn** `rail` — A rail showing `items`, top to bottom.
  - `pub fn rail(id: impl Into<ElementId>, items: Vec<RailItem>) -> Rail`
- **fn** `role_section` — A role section for `role`.
  - `pub fn role_section(id: impl Into<ElementId>, role: &Role) -> RoleSection`
- **fn** `role_session_row` — A session row: the hairline rail, the kind glyph, the name, the elapsed time.
  - `pub fn role_session_row(id: impl Into<ElementId>, session: RoleSession) -> RoleSessionRow`
- **fn** `session_row` — A row for `session`. Children are rendered beneath it, nested.
  - `pub fn session_row(id: impl Into<ElementId>, session: SessionSummary) -> SessionRow`
- **fn** `sidebar` — A sidebar rendering `nav`.
  - `pub fn sidebar(id: impl Into<ElementId>, nav: SidebarNav) -> Sidebar`
- **fn** `sidebar_footer` — A footer for `name` with `initial` in the avatar.
  - `pub fn sidebar_footer(id: impl Into<ElementId>, initial: impl Into<SharedString>, name: impl Into<SharedString>) -> SidebarFooter`
- **fn** `sidebar_search` — A search row wrapping `field`.
  - `pub fn sidebar_search(id: impl Into<ElementId>, field: impl IntoElement) -> SidebarSearch`
- **fn** `sidebar_view` — The sessions of a sidebar, grouped by `grouping`.
  - `pub fn sidebar_view(id: impl Into<ElementId>, grouping: impl Into<Rc<Grouping>>) -> SidebarView`
- **fn** `view_menu` — The 250 px menu reached from the sliders icon on a group row.
  - `pub fn view_menu(id: impl Into<ElementId>, rows: Vec<MenuRow>) -> ViewMenu`
- **fn** `view_submenu` — A submenu listing `items`, with `selected` marked by an accent-ink check.
  - `pub fn view_submenu(id: impl Into<ElementId>, items: Vec<SharedString>, selected: Option<usize>) -> ViewSubmenu`
- **fn** `view_submenu_rows` — A submenu of menu rows, each carrying its own check (a `Colour` submenu of `MenuRow::Swatch` rows). Indices reported by `on_activate` count across the plain `items` first, then these rows.
  - `pub fn view_submenu_rows(id: impl Into<ElementId>, rows: Vec<MenuRow>) -> ViewSubmenu`

- **struct** `Activity` — The live third line of a row.
  - fields: `kind`, `text`
- **struct** `CompactSessionRow` — The compact row (`.sr`). Build with `compact_session_row`.
  - `pub fn actions(self, actions: Vec<RowAction>) -> Self` — The hover action tray, off by default on a compact row.
  - `pub fn editor(self, editor: impl IntoElement) -> Self` — Replace the name with a field the caller owns: an inline rename.
  - `pub fn on_action(self, f: impl Fn(&SharedString, RowAction, &mut Window, &mut App) + 'static) -> Self` — Hover-action click.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — Row click.
  - `pub fn selected(self, selected: bool) -> Self` — Selected: surface-3 ground and ink text.
- **struct** `DateGroup` — A date group: the `.dg` caps header with its hairline rule, and the flat rows under it.
  - fields: `label`, `sessions`
  - `pub fn new(label: impl Into<SharedString>, sessions: Vec<SessionSummary>) -> Self` — A date group.
- **struct** `DateGroupHeader` — A date header (`.dg`). Build with `date_group_header`.
- **struct** `FolderDropCard` — A full-width card that takes a folder. Build with `folder_drop_card`.
  - `pub fn dragging(self, dragging: bool) -> Self` — Draws the drag-over state statically: accent border on accent-soft.
  - `pub fn key(self, key: impl Into<SharedString>) -> Self` — A keycap at the card’s right (`⌘⇧O`): the shortcut that chooses.
  - `pub fn on_click(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — Click on the card (choose a folder).
  - `pub fn on_drop(self, f: impl Fn(Vec<PathBuf>, &mut Window, &mut App) + 'static) -> Self` — A drop landed: every dropped path that is a directory.
  - `pub fn subtitle(self, subtitle: impl Into<SharedString>) -> Self` — Overrides the subtitle (`or click to choose one`).
  - `pub fn title(self, title: impl Into<SharedString>) -> Self` — Overrides the title (`Drop a folder here`).
- **struct** `GroupHeader` — A collapsible group header: chevron, 12 px / 600 label, mono count, optional trailing element. Build with `group_header`.
  - `pub fn count(self, count: impl Into<SharedString>) -> Self` — The mono count after the label.
  - `pub fn margin_top(self, margin: f32) -> Self` — Overrides the 8 px top margin.
  - `pub fn on_toggle(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Toggle click.
  - `pub fn trailing(self, el: impl IntoElement) -> Self` — Something at the far right (the `main` branch tag).
- **struct** `GroupRow` — The caps group row (`Workspaces`, `Projects`, `Recent`) with the view options (sliders) and `+` actions. Build with `group_row`.
  - `pub fn margin_top(self, margin: f32) -> Self` — Overrides the 8 px top margin (card 21 has none).
  - `pub fn on_add(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Shows the `+` and handles its click.
  - `pub fn on_view_options(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Shows the sliders icon and handles its click.
- **struct** `KnowledgeCard` — The knowledge sources of a role, as a bordered card of chips (`.kb`). Build with `knowledge_card`.
  - `pub fn on_add(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — Intent for the `+ add` chip, carrying the role id.
- **struct** `NavItem` — A primary nav row. Build with `nav_item`.
  - `pub fn count(self, count: impl Into<SharedString>) -> Self` — The mono count at the right.
  - `pub fn count_warning(self) -> Self` — Colours the count in warning (the inbox with items that need the person).
  - `pub fn on_click(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Click handler.
- **struct** `Project` — One project inside a role (`.proj` plus the `.sess` rows under it).
  - fields: `id`, `name`, `count`, `sessions`
  - `pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, count: usize) -> Self` — A project with `count` sessions and no expanded children.
  - `pub fn session(self, session: RoleSession) -> Self` — Adds a session row under the project.
- **struct** `ProjectGroup` — A project group: the `.pj` row and the sessions (and their children) below.
  - fields: `id`, `name`, `count`, `open`, `muted`, `mark`, `trailing`, `state`, `sessions`,
    `fold`
  - `pub fn folded(self, hidden: usize, expanded: bool) -> Self` — Folds a long group: `hidden` rows are held back and `expanded` picks the row’s label (“Show {hidden} more” / “Show less”). [...]
  - `pub fn mark(self, initial: impl Into<SharedString>, colour: Hsla) -> Self` — Draws the project’s mark in place of the folder glyph.
  - `pub fn muted(self) -> Self` — Mutes the row (`Archived`, `Other workspaces`).
  - `pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, count: impl Into<SharedString>) -> Self` — A closed, unmuted project.
  - `pub fn open(self, sessions: Vec<SessionSummary>) -> Self` — Opens the project and gives it its sessions.
  - `pub fn state(self, state: AgentState) -> Self` — Sets the rolled-up agent state: a status dot after the name, pulsing while a session runs.
  - `pub fn trailing(self, text: impl Into<SharedString>) -> Self` — Sets the trailing mono text before the count (the branch).
- **struct** `ProjectGroupRow` — A project row (`.pj`). Build with `project_group_row`.
  - `pub fn mark(self, initial: impl Into<SharedString>, colour: Hsla) -> Self` — Draws the project’s mark in place of the folder glyph. Muted groups keep the folder glyph.
  - `pub fn muted(self) -> Self` — `.pj{color:var(--ink-3);font-weight:500}` — the archived project.
  - `pub fn on_group_action(self, f: impl Fn(GroupAction, &mut Window, &mut App) + 'static) -> Self` — A hover-tray button was clicked; the argument is what it asked for.
  - `pub fn on_toggle(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Toggle click.
  - `pub fn state(self, state: AgentState) -> Self` — Sets the rolled-up agent state: a status dot after the name, pulsing while a session runs.
  - `pub fn trailing(self, text: impl Into<SharedString>) -> Self` — Sets the trailing mono text before the count (the branch).
- **struct** `ProjectMark` — A project mark. Build with `project_mark`.
  - `pub fn size(self, size: impl Into<Pixels>) -> Self` — Overrides the square’s side: 14 for menu rows and the palette, 18 for the header and group rows, 22 for the rail. The initial scales with it.
- **struct** `ProjectRow` — A project row (`.proj`). Build with `project_row`.
  - `pub fn on_select(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — Selection intent, carrying the project id.
- **struct** `Rail` — The collapsed sidebar. Build with `rail`.
  - `pub fn avatar(self, initial: impl Into<SharedString>) -> Self` — The account avatar pinned to the bottom.
  - `pub fn flat(self, flat: bool) -> Self` — Drops the rail’s own card (border, radius, ground) and lets it fill the column it is given: the shell already paints the sidebar column’s surface and divider, so the standalone card would double them.
  - `pub fn on_action(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — A nav cell (or `"account"`) was clicked; the argument is its name.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A session cell was clicked; the argument is the session id.
- **struct** `Role` — One role of the assistant (`.sec`): the person’s hat, its projects and the knowledge sources the assistant is grounded in while wearing it.
  - fields: `id`, `name`, `icon`, `open`, `count`, `projects`, `knowledge`
  - `pub fn knowledge(self, source: impl Into<SharedString>) -> Self` — Adds a knowledge source.
  - `pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, glyph: RoleIcon, count: usize) -> Self` — A closed role with `count` sessions.
  - `pub fn open(self) -> Self` — Expands the section.
  - `pub fn project(self, project: Project) -> Self` — Adds a project.
- **struct** `RoleSection` — A bordered role section (`.sec`): the 44 px role header and, when open, the Projects and Knowledge groups. Build with `role_section`.
  - `pub fn active_session(self, id: Option<&str>) -> Self` — The id of the session that is open, highlighted with `.sess.on`.
  - `pub fn on_add_knowledge(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — Add-knowledge intent, carrying the role id.
  - `pub fn on_select_project(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — Project selection intent, carrying the project id.
  - `pub fn on_select_session(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — Session selection intent, carrying the session id.
  - `pub fn on_toggle_role(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — Open / close intent, carrying the role id.
- **struct** `RoleSession` — One session inside a project (`.sess`).
  - fields: `id`, `name`, `kind`, `elapsed`
  - `pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, kind: SessionKind, elapsed: impl Into<SharedString>) -> Self` — A session of `kind` last touched `elapsed` ago.
- **struct** `RoleSessionRow` — A session row under a project (`.sess`). Build with `role_session_row`.
  - `pub fn active(self, active: bool) -> Self` — Marks the row as the open session (`.sess.on`).
  - `pub fn on_select(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — Selection intent, carrying the session id.
- **struct** `SessionRow` — The full session row. Build with `session_row`.
  - `pub fn actions(self, actions: Vec<RowAction>) -> Self` — Which actions the tray carries; `RowAction::ALL` when unset.
  - `pub fn activity_max(self, max: f32) -> Self` — Max width of the activity sentence (defaults to the branch max).
  - `pub fn branch_max(self, max: f32) -> Self` — Max width of the truncated branch tag.
  - `pub fn collapse_margins(self) -> Self` — For rows in block flow, where CSS collapses adjacent 2 px margins into one: keeps the top margin only.
  - `pub fn margin_x(self, margin: f32) -> Self` — Horizontal margin (8 inside a sidebar, 0 inside a padded list).
  - `pub fn on_action(self, f: impl Fn(&SharedString, RowAction, &mut Window, &mut App) + 'static) -> Self` — Hover-action click.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — Row click.
  - `pub fn row_gap(self, gap: f32) -> Self` — Gap between the row’s lines (2 in card 20, 3 in the shell and sidebar).
  - `pub fn selected(self, selected: bool) -> Self` — Selected: surface-3 ground.
  - `pub fn show_actions(self, show: bool) -> Self` — Whether the hover action tray exists.
  - `pub fn text_sizes(self, name: f32, meta: f32) -> Self` — Name and meta sizes (13 / 12 by default; the sidebar card uses 12.5 / 11.5).
- **struct** `SessionSummary` — One session / worktree as the sidebar shows it.
  - fields: `id`, `name`, `state`, `pulse`, `elapsed`, `repo`, `branch`, `providers`, `meta`,
    `activity`, `unread`, `pinned`, `children`
  - `pub fn activity(self, kind: ActivityKind, text: impl Into<SharedString>) -> Self` — Sets the activity line.
  - `pub fn branch(self, branch: impl Into<SharedString>) -> Self` — Sets the branch tag.
  - `pub fn child(self, child: SessionSummary) -> Self` — Adds a child session.
  - `pub fn meta(self, item: MetaItem) -> Self` — Adds a meta item.
  - `pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, state: AgentState, elapsed: impl Into<SharedString>) -> Self` — A minimal summary; fill the rest with the builder methods.
  - `pub fn pinned(self) -> Self` — Pins the session: date groupings render it in the leading `Pinned` group, excluded from the date buckets.
  - `pub fn provider(self, provider: Provider) -> Self` — Adds a provider mark.
  - `pub fn pulse(self) -> Self` — Pulses the dot.
  - `pub fn repo(self, repo: impl Into<SharedString>) -> Self` — Sets the repo tag.
  - `pub fn unread(self) -> Self` — Marks unread.
- **struct** `Sidebar` — The sidebar. Build with `sidebar`.
  - `pub fn collapsed(self, collapsed: bool) -> Self` — Collapsed (⌘B): renders the `crate::nav::Rail` instead. The width change itself is the caller’s (the shell animates the column).
  - `pub fn on_action(self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self` — A named action: the nav rows report their own name; the header reports `"workspace"`, `"search"`, `"new"` and `"collapse"`; the group row reports `"view-options"` and `"add"`; the footer `"account"`.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A session row was clicked; the argument is the session id.
  - `pub fn on_toggle_group(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A group header was clicked; the argument is the group id.
- **struct** `SidebarAccount` — The account footer’s data.
  - fields: `initial`, `name`, `plan`, `provider`, `usage`
  - `pub fn new(initial: impl Into<SharedString>, name: impl Into<SharedString>, provider: Provider, usage: f32) -> Self` — A footer for `name`.
  - `pub fn plan(self, plan: impl Into<SharedString>, warning: bool) -> Self` — The plan label the footer draws after the name.
- **struct** `SidebarFooter` — The account footer: avatar, name, provider usage meter, chevron. Build with `sidebar_footer`.
  - `pub fn detail(self, detail: impl Into<SharedString>) -> Self` — A second, quieter line under the name: the account’s email, the identity a “signed in as” footer is really reporting.
  - `pub fn meter(self, provider: Provider, fraction: f32) -> Self` — Shows the provider usage meter (`fraction` in 0..=1) and the chevron.
  - `pub fn on_click(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Click on the footer (account menu).
  - `pub fn pad_y(self, pad: f32) -> Self` — Vertical padding: 10 in the shell, 8 in the sidebar cards.
  - `pub fn plan(self, plan: impl Into<SharedString>, warning: bool) -> Self` — A small label after the name on the name’s row: what the account is entitled to, e.g. [...]
  - `pub fn plan_trailing(self, el: impl IntoElement) -> Self` — One quiet control after the plan label.
  - `pub fn trailing(self, el: impl IntoElement) -> Self` — Replaces the meter + chevron with another element (the assistant’s pill).
- **struct** `SidebarGroup` — A collapsible group of sessions.
  - fields: `id`, `label`, `count`, `open`, `trailing`, `sessions`
  - `pub fn closed(self) -> Self` — Closes the group.
  - `pub fn count(self, count: impl Into<SharedString>) -> Self` — Sets the count.
  - `pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self` — An open group with no sessions.
  - `pub fn session(self, session: SessionSummary) -> Self` — Adds a session.
  - `pub fn trailing(self, text: impl Into<SharedString>) -> Self` — Sets the trailing mono text.
- **struct** `SidebarNav` — Everything the sidebar renders. An app derives this from its own state; the sidebar reads nothing else.
  - fields: `workspace`, `items`, `groups_label`, `groups`, `selected`, `footer`
  - `pub fn group(self, group: SidebarGroup) -> Self` — Adds a group.
  - `pub fn groups_label(self, label: impl Into<SharedString>) -> Self` — Overrides the caps label of the group row.
  - `pub fn item(self, item: SidebarNavItem) -> Self` — Adds a primary nav row.
  - `pub fn new(workspace: impl Into<SharedString>, footer: SidebarAccount) -> Self` — A sidebar for `workspace` with the given footer; add items and groups with the builder methods.
  - `pub fn rail_items(&self) -> Vec<RailItem>` — The collapsed form of this data: the nav glyphs (the warning count becomes the badge), the separator, then one cell per active session — every session that is not `AgentState::Idle`, in group order.
  - `pub fn selected(self, id: impl Into<SharedString>) -> Self` — Selects a session.
- **struct** `SidebarNavItem` — One primary nav row (`Tasks`, `Automations`, `Inbox`).
  - fields: `name`, `label`, `icon`, `count`, `warning`
  - `pub fn count(self, count: impl Into<SharedString>) -> Self` — Sets the mono count.
  - `pub fn new(name: impl Into<SharedString>, label: impl Into<SharedString>, glyph: IconName) -> Self` — A nav row with no count.
  - `pub fn warning(self) -> Self` — Colours the count in warning.
- **struct** `SidebarSearch` — The sidebar’s search row: a magnifier, a field the caller owns, and a clear button that only exists while there is something to clear. Build with `sidebar_search`.
  - `pub fn clearable(self, clearable: bool) -> Self` — Whether the clear button is drawn: there is text to clear.
  - `pub fn on_clear(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — The clear button was pressed.
- **struct** `SidebarView` — The body of a sidebar panel in one grouping. Build with `sidebar_view`.
  - `pub fn caption(self, caption: impl Into<SharedString>) -> Self` — The caps group row above the groups (`Workspaces`, `Projects`, `Recent`). It carries the sliders icon when `Self::on_view_options` is set.
  - `pub fn editing(self, session_id: impl Into<SharedString>, editor: impl IntoElement) -> Self` — One row is being renamed: draw `editor` in place of its name.
  - `pub fn on_action(self, f: impl Fn(&SharedString, RowAction, &mut Window, &mut App) + 'static) -> Self` — A row’s hover action was clicked.
  - `pub fn on_group_action(self, f: impl Fn(&SharedString, GroupAction, &mut Window, &mut App) + 'static) -> Self` — A project group row’s hover action was clicked; the arguments are the group id and what the tray button asked for.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A row was clicked; the argument is the session id.
  - `pub fn on_toggle(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A group header or project row was clicked; the argument is the group id.
  - `pub fn on_view_options(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The sliders icon on the caption row was clicked: open the view menu.
  - `pub fn row_actions(self, actions: Vec<RowAction>) -> Self` — The hover actions every row carries; none by default.
  - `pub fn selected(self, id: impl Into<SharedString>) -> Self` — The id of the selected session.
- **struct** `StatusGroup` — A status group: `Needs you 1`, `Running 3`, `Done 2`.
  - fields: `id`, `label`, `count`, `open`, `sessions`
  - `pub fn closed(self) -> Self` — Closes the group.
  - `pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, count: impl Into<SharedString>, sessions: Vec<SessionSummary>) -> Self` — An open group with `sessions` under it.
- **struct** `ViewMenu` — The view-options menu. Build with `view_menu`.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the menu is drawn at rest on its first frame. For a menu that is part of a static composition (the design card, a restored panel) rather than one the person just opened.
  - `pub fn on_activate(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — A row was clicked; the argument is its index in `rows`.
  - `pub fn present(self, present: bool) -> Self` — Whether the menu is open; `false` plays the exit.
- **struct** `ViewSubmenu` — The 170 px submenu of the `Group by` row. Build with `view_submenu`, or with `view_submenu_rows` for rows that carry their own checks (the project colour swatches).
  - `pub fn at_rest(self) -> Self` — Skips the enter: the submenu is drawn at rest on its first frame.
  - `pub fn on_activate(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — An item was clicked; the argument is its index.
  - `pub fn present(self, present: bool) -> Self` — Whether the submenu is open; `false` plays the exit.
  - `pub fn separator_before(self, index: usize) -> Self` — Draws a hairline above the item at `index` (`None` sits below the grouping choices).

- **enum** `ActivityKind` — How the activity line is drawn.
  - variants: `Working`, `Waiting`, `Failed`, `Plain`
- **enum** `GroupAction` — What a project group row’s hover tray asks for.
  - variants: `New`, `Menu`, `ToggleMore`
- **enum** `Grouping` — How a `SidebarView` groups its sessions. The variant carries the groups, because each grouping has its own header shape.
  - variants: `Status`, `Project`, `Date`
- **enum** `MenuRow` — One row of the view-options menu.
  - variants: `Submenu`, `Toggle`, `Swatch`, `Separator`
- **enum** `MetaItem` — An item on the meta line.
  - variants: `Text`, `Tag`, `Danger`, `Warning`
- **enum** `RailItem` — One cell of the rail. Build with `RailItem::nav`, `RailItem::separator` or `RailItem::session`.
  - variants: `Nav`, `Separator`, `Session`
  - `pub fn badge(self) -> Self` — Adds the warning badge to a `RailItem::Nav`; ignored by other kinds.
  - `pub fn current(self, is_current: bool) -> Self` — Marks a `RailItem::Nav` as the cell the screen is showing; ignored by other kinds (`RailItem::selected` does the same for a session).
  - `pub fn label(self, title: impl Into<SharedString>) -> Self` — Titles a `RailItem::Session`: the tile shows its initial and the tooltip the whole title; ignored by other kinds.
  - `pub fn nav(name: impl Into<SharedString>, glyph: IconName) -> Self` — A nav glyph named `name` (the name `on_action` reports).
  - `pub fn pulse(self) -> Self` — Pulses a `RailItem::Session` dot; ignored by other kinds.
  - `pub fn selected(self, is_selected: bool) -> Self` — Marks a `RailItem::Session` as the current one; ignored by other kinds.
  - `pub fn separator() -> Self` — The hairline separator.
  - `pub fn session(id: impl Into<SharedString>, state: AgentState) -> Self` — A session cell.
  - `pub fn tint(self, colour: Hsla) -> Self` — Draws a `RailItem::Session` tile’s initial in `colour` (the project label) instead of the default ink; the state dot and everything else are unchanged. [...]
- **enum** `RowAction` — The hover actions on a row.
  - variants: `Terminal`, `Browser`, `Pin`, `Rename`, `Hide`, `Archive`, `More`
- **enum** `SessionKind` — What kind of work a session is, which fixes its glyph.
  - variants: `Chat`, `Document`, `Sheet`
  - `pub fn icon(self) -> IconName` — The glyph for this kind.

- **const** `DENSE_FIELD_H` — Height of the dense rename field: the compact row is 30 px with 4 px of vertical padding, so the field gets 20 px inside a 22 px bordered wrapper.
  - `pub const DENSE_FIELD_H: f32 = 20.0;`
- **const** `RAIL_WIDTH` — `.rail{width:48px;padding:8px 0;gap:4px}`.
  - `pub const RAIL_WIDTH: f32 = 48.0;`
- **const** `SIDEBAR_WIDTH` — `.side{width:256px}`.
  - `pub const SIDEBAR_WIDTH: f32 = 256.0;`

- **type** `RoleIntent` — Handler for an intent that carries the id of the row it came from.
  - `pub type RoleIntent = Rc<dyn Fn(&str, &mut Window, &mut App)>;`

### `aui::overlay`

Overlays: the command palette (card 12), the modal dialog, and later menus and popovers. Overlays render inside the window; the app decides when they are present and positions them.

- **fn** `command_palette` — The 560 px palette. `selected` indexes the rows of all sections in order, as the arrow keys walk them.
  - `pub fn command_palette(id: impl Into<ElementId>, query: impl Into<SharedString>, sections: Vec<PaletteSection>, selected: usize) -> CommandPalette`
- **fn** `dialog` — A modal dialog headed `title`, with an `OK` primary until one is set.
  - `pub fn dialog(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Dialog`
- **fn** `palette_scrim` — `.scrim`: 470 px tall, a black .25 → .45 gradient over the window ground, with `child` centred 56 px below the top edge.
  - `pub fn palette_scrim(child: impl IntoElement) -> PaletteScrim`
- **fn** `popover_layer` — Lifts an anchored overlay — a menu, a popover, a hover card — out of the paint order of the surface it hangs off.
  - `pub fn popover_layer(child: impl IntoElement) -> Deferred`

- **struct** `CommandPalette` — The command palette. Build with `command_palette`.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the palette is drawn at rest on its first frame. For a palette that is part of a static composition (the design card) rather than one the person just opened.
  - `pub fn on_dismiss(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The `esc` keycap was clicked.
  - `pub fn on_hover(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — The pointer entered a row; the argument is its index across all sections, so the caller can move the selection to it.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — A row was clicked; the argument is its `PaletteItem::id`.
  - `pub fn placeholder(self, placeholder: impl Into<SharedString>) -> Self` — The ink-4 text shown in the query row while the query is empty.
  - `pub fn present(self, present: bool) -> Self` — Whether the palette is open; `false` plays the exit.
  - `pub fn query_slot(self, editor: impl IntoElement) -> Self` — A real editor for the query row: the caller’s own field, drawn chromeless where the query text would be, so the palette is one surface with the field inside it rather than a field floating above.
- **struct** `Dialog` — A modal dialog. Build with `dialog`.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the dialog is drawn at rest on its first frame, for a static capture.
  - `pub fn body(self, text: impl Into<SharedString>) -> Self` — The body paragraph, in the muted body ink.
  - `pub fn danger(self, danger: bool) -> Self` — Draws the primary as the outlined danger button instead of the accent fill, for an action that destroys something.
  - `pub fn detail(self, text: impl Into<SharedString>) -> Self` — An optional mono detail line under the body (a path, an id, an error code).
  - `pub fn kind(self, kind: DialogKind) -> Self` — What the dialog is about; picks the tile.
  - `pub fn on_dismiss(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The scrim was clicked. A click on the card itself does not reach this.
  - `pub fn on_primary(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The primary was pressed.
  - `pub fn on_secondary(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The secondary was pressed.
  - `pub fn present(self, present: bool) -> Self` — Whether the dialog is open; `false` plays the exit.
  - `pub fn primary(self, label: impl Into<SharedString>) -> Self` — The label of the primary button at the far right.
  - `pub fn secondary(self, label: impl Into<SharedString>) -> Self` — The label of the secondary button; without one only the primary is drawn.
  - `pub fn width(self, width: f32) -> Self` — Overrides the card width.
- **struct** `PaletteItem` — One row of the palette.
  - fields: `id`, `icon`, `label`, `matched`, `context`, `badge`, `keys`
  - `pub fn badge(self, badge: impl Into<SharedString>) -> Self` — Adds the status pill after the label.
  - `pub fn context(self, context: impl Into<SharedString>) -> Self` — Adds the muted mono context after the label.
  - `pub fn key(self, key: impl Into<SharedString>) -> Self` — Adds one keycap to the right-hand hint.
  - `pub fn matched(self, range: Range<usize>) -> Self` — Marks a byte range of the label as a query match.
  - `pub fn matching(self, needle: &str) -> Self` — Marks the first occurrence of `needle` in the label as a query match.
  - `pub fn new(id: impl Into<SharedString>, icon: PaletteIcon, label: impl Into<SharedString>) -> Self` — A row with an icon and a label and nothing else.
- **struct** `PaletteScrim` — The dimmed ground the palette floats on. Build with `palette_scrim`.
- **struct** `PaletteSection` — A titled block of rows (`Worktrees`, `Actions`, `Files`).
  - fields: `title`, `items`
  - `pub fn lead(self, el: impl IntoElement) -> Self` — A non-row element drawn under the section title and before the rows, with the rows’ horizontal padding and a `SP_2` gap below it. [...]
  - `pub fn new(title: impl Into<SharedString>, items: Vec<PaletteItem>) -> Self` — A section with its header and rows.

- **enum** `DialogKind` — What a dialog is about. The kind picks the tile’s glyph and its tint; nothing else in the card is coloured.
  - variants: `Info`, `Warning`, `Error`
- **enum** `PaletteIcon` — The leading glyph of a palette row: an icon for actions and files, a status dot for worktrees, a project mark for projects.
  - variants: `Glyph`, `Dot`, `Mark`

- **const** `POPOVER_LAYER` — The priority every popover in the library paints at. Deferred draws are painted in priority order, so a menu opened from inside another popover can ask for `POPOVER_LAYER` + 1 and land on top of it.
  - `pub const POPOVER_LAYER: usize = 1;`

### `aui::screens`

Full-window screens: the whole window is the component, not a card inside one.

- **fn** `login` — The sign-in screen in `state`. It fills the window it is given and centres its card on the window ground.
  - `pub fn login(id: impl Into<ElementId>, state: LoginState) -> Login`

- **struct** `Login` — The sign-in screen. Build with `login`.
  - `pub fn api_key_field(self, field: impl IntoElement) -> Self` — The element drawn in the API-key form’s field slot — the harness passes a `secret_field`. The screen owns no text: Enter inside the field and the reveal toggle stay the caller’s business.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the card is drawn at rest on its first frame, for a static capture.
  - `pub fn headline(self, text: impl Into<SharedString>) -> Self` — Overrides the headline (`"Sign in to <product>"` by default).
  - `pub fn on_intent(self, f: impl Fn(LoginIntent, &mut Window, &mut App) + 'static) -> Self` — A button was pressed.
  - `pub fn product(self, name: impl Into<SharedString>) -> Self` — The product being signed in to; the default headline is built from it.
  - `pub fn provider(self, provider: Provider) -> Self` — The product mark above the headline.
  - `pub fn subtitle(self, text: impl Into<SharedString>) -> Self` — The muted line under the headline.

- **enum** `LoginIntent` — What the person asked the app to do.
  - variants: `StartAccount`, `UseApiKey`, `SubmitApiKey`, `ToggleReveal`, `Back`,
    `OpenBrowser`, `CopyCode`, `Retry`, `ChooseAnother`, `Cancel`
- **enum** `LoginMethod` — Which way in the person chose.
  - variants: `Account`, `ApiKey`
- **enum** `LoginState` — What the sign-in screen is showing.
  - variants: `Choose`, `Starting`, `Device`, `ApiKey`, `Validating`, `Success`, `Error`

### `aui::shell`

App shell: the three-column layout with one 44 px header cell per column, continuous dividers, the docked composer, panel chrome, tab strips with the sliding indicator and drop zones (cards 10–11, spe [...]

- **fn** `app_shell` — An empty shell with the default column widths.
  - `pub fn app_shell(id: impl Into<ElementId>) -> AppShell`
- **fn** `centre_header` — The centre header: provider mark + worktree name + branch tag, spacer, overflow menu, right-pane toggle. Nothing else lives here.
  - `pub fn centre_header(id: impl Into<ElementId>, title: impl Into<SharedString>) -> CentreHeader`
- **fn** `clamp_sidebar_width` — Clamps a drag width into the resizable range. The shell clamps its own target the same way, but call this on every drag move before notifying so the stored width never leaves the range.
  - `pub fn clamp_sidebar_width(width: f32) -> f32`
- **fn** `docked_composer` — A composer for `provider` / `model`.
  - `pub fn docked_composer(id: impl Into<ElementId>, provider: Provider, model: impl Into<SharedString>) -> DockedComposer`
- **fn** `drag_capture_overlay` — A transparent layer over the window that forwards every move and the release to the drag intents. [...]
  - `pub fn drag_capture_overlay(id: impl Into<ElementId>) -> DragCaptureOverlay`
- **fn** `drag_region` — Wraps a header row so press-drag moves the window and double-click zooms.
  - `pub fn drag_region(id: impl Into<ElementId>) -> DragRegion`
- **fn** `drop_zones` — An overlay to place inside a `relative` panel body.
  - `pub fn drop_zones(id: impl Into<ElementId>, visible: bool) -> DropZones`
- **fn** `header_cell` — An empty header cell.
  - `pub fn header_cell(id: impl Into<ElementId>) -> HeaderCell`
- **fn** `panel_header` — A header with a title.
  - `pub fn panel_header(id: impl Into<ElementId>, title: impl Into<SharedString>) -> PanelHeader`
- **fn** `resize_handle` — A 6 px transparent strip, full height, with the horizontal-resize cursor.
  - `pub fn resize_handle(id: impl Into<ElementId>) -> ResizeHandle`
- **fn** `right_header` — The right header: the pane’s tab strip, `+`, spacer, close.
  - `pub fn right_header(id: impl Into<ElementId>) -> RightHeader`
- **fn** `sidebar_header` — The sidebar header: lights, back, forward, spacer, search, sidebar toggle.
  - `pub fn sidebar_header(id: impl Into<ElementId>) -> SidebarHeader`
- **fn** `tab_ghost` — A ghost for a tab.
  - `pub fn tab_ghost(icon: Option<IconName>, label: impl Into<SharedString>) -> TabGhost`
- **fn** `tab_strip` — A strip over `tabs` with `active` selected.
  - `pub fn tab_strip(id: impl Into<ElementId>, tabs: Vec<TabItem>, active: usize) -> TabStrip`
- **fn** `traffic_light_position` — Where a host window should place macOS’s native traffic lights so their centre is the header’s centre at every density: pass this to `TitlebarOptions.traffic_light_position` when the window keeps the [...]
  - `pub fn traffic_light_position(cx: &App) -> Point<Pixels>`

- **struct** `AppShell` — The shell. Build with `app_shell`.
  - `pub fn centre(self, el: impl IntoElement) -> Self` — The centre pane (transcript + composer).
  - `pub fn draggable(self, draggable: bool) -> Self` — Wraps the header row in a window drag region: press-drag moves the window, double-click zooms (macOS titlebar behaviour, `zoom_window` elsewhere). [...]
  - `pub fn framed(self, framed: bool) -> Self` — Draws the window frame the design card shows: line-strong border, radius 12, elevation 3. Apps fill the window instead.
  - `pub fn header_centre(self, el: impl IntoElement) -> Self` — The centre header cell content.
  - `pub fn header_follows_sidebar(self, follows: bool) -> Self` — Whether the header row’s sidebar cell follows the collapse (default true, today’s behaviour: the cell shrinks to the rail with the pane). [...]
  - `pub fn header_right(self, el: impl IntoElement) -> Self` — The right header cell content (the pane’s tab strip).
  - `pub fn header_sidebar(self, el: impl IntoElement) -> Self` — The sidebar header cell content.
  - `pub fn rail(self, el: impl IntoElement) -> Self` — The collapsed sidebar pane: the rail that replaces `AppShell::sidebar` while `sidebar_open` is false. [...]
  - `pub fn resizing(self, resizing: bool) -> Self` — A resize drag is in flight: the column feeds the width straight through and skips the layout spring so the divider tracks the pointer. [...]
  - `pub fn right(self, el: impl IntoElement) -> Self` — The right pane (workbench).
  - `pub fn right_open(self, open: bool) -> Self` — Whether the right pane is open; the column animates on the layout spring.
  - `pub fn right_width(self, width: impl Into<Pixels>) -> Self` — Overrides the right column width.
  - `pub fn sidebar(self, el: impl IntoElement) -> Self` — The sidebar pane.
  - `pub fn sidebar_max_width(self, max: impl Into<Pixels>) -> Self` — Maximum sidebar width; the render target never goes above it while the sidebar is open. Defaults to `SIDEBAR_MAX_WIDTH`.
  - `pub fn sidebar_min_width(self, min: impl Into<Pixels>) -> Self` — Minimum sidebar width; the render target never goes below it while the sidebar is open. Defaults to `SIDEBAR_MIN_WIDTH`.
  - `pub fn sidebar_open(self, open: bool) -> Self` — Whether the sidebar is expanded (false = the rail).
  - `pub fn sidebar_width(self, width: impl Into<Pixels>) -> Self` — Overrides the sidebar column width.
  - `pub fn traffic_lights(self, on: bool) -> Self` — The window’s controls sit in the shell’s top-left corner (the gallery paints them; a real window’s are native). [...]
- **struct** `CentreHeader` — Centre header cell. Build with `centre_header`.
  - `pub fn branch(self, branch: impl Into<SharedString>) -> Self` — The branch tag after the title.
  - `pub fn glyph(self, glyph: IconName) -> Self` — A 14 px ink-3 glyph before the title instead of a provider mark (the assistant’s role icon).
  - `pub fn on_expand_sidebar(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Shows the sidebar toggle at the leading edge of the centre header and calls `f` when it is clicked. [...]
  - `pub fn on_overflow(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Overflow (dots) click.
  - `pub fn on_toggle_right(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Right-pane toggle click.
  - `pub fn provider(self, provider: Provider) -> Self` — The 16 px provider mark before the title.
  - `pub fn trailing(self, el: impl IntoElement) -> Self` — A status pill after the branch (`waiting`).
- **struct** `DockedComposer` — The docked composer. Build with `docked_composer`.
  - `pub fn can_send(self, can_send: bool) -> Self` — Whether the send button is live (surface-3 / ink-4 when the draft is empty).
  - `pub fn context_percent(self, percent: u8) -> Self` — Shows `context N%` after the chips.
  - `pub fn mode(self, mode: impl Into<SharedString>) -> Self` — The mode chip label.
  - `pub fn on_intent(self, f: impl Fn(DockedComposerIntent, &mut Window, &mut App) + 'static) -> Self` — Intent handler.
  - `pub fn pad_x(self, pad: f32) -> Self` — Overrides the horizontal padding.
  - `pub fn placeholder(self, text: impl Into<SharedString>) -> Self` — The placeholder text.
  - `pub fn streaming(self, streaming: bool) -> Self` — A turn is running: the send button morphs to stop.
- **struct** `DragCaptureOverlay` — The full-window capture layer for an in-flight resize drag. Build with `drag_capture_overlay`.
  - `pub fn on_drag(self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self` — A move anywhere in the window; same update as the handle’s `on_drag`.
  - `pub fn on_drag_end(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The button was released; same teardown as the handle’s `on_drag_end`.
- **struct** `DragRegion` — A window drag region over its children. Build with `drag_region`.
  - `pub fn child(self, el: impl IntoElement) -> Self` — Adds wrapped content (usually one header row).
  - `pub fn enabled(self, enabled: bool) -> Self` — Whether press-drag and double-click are armed. On by default; pass false to keep the (layout-identical) wrapper without the behaviour.
- **struct** `DropZones` — The overlay. Build with `drop_zones`.
  - `pub fn hot(self, zone: Option<DropZone>) -> Self` — The zone under the pointer (drawn solid at full opacity).
- **struct** `HeaderCell` — A plain header cell: 44 px row, gap 6, padding 0 10. Build with `header_cell`.
  - `pub fn child(self, el: impl IntoElement) -> Self` — Adds a child.
- **struct** `PanelHeader` — A panel header. Build with `panel_header`.
  - `pub fn action(self, name: &'static str, glyph: IconName, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — A quiet xs icon action at the right.
  - `pub fn grip(self, grip: bool) -> Self` — Whether to show the drag grip (floating panels).
  - `pub fn icon(self, glyph: IconName) -> Self` — The 14 px glyph before the title.
  - `pub fn subtitle(self, subtitle: impl Into<SharedString>) -> Self` — The ink-3 subtitle after the title.
- **struct** `ResizeHandle` — The resize strip over the sidebar/centre divider. Build with `resize_handle`.
  - `pub fn on_drag(self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self` — The pointer moved with the left button held; the argument is the current x in window pixels. Set `width = clamp(start_w + (x - grab_x))`.
  - `pub fn on_drag_end(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The button was released (inside or outside the strip). Disarm the drag and persist the width.
  - `pub fn on_drag_start(self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self` — The left button went down on the strip; the argument is the grab x in window pixels. Arm the drag and remember `grab_x` and `start_w`.
- **struct** `RightHeader` — Right header cell. Build with `right_header`.
  - `pub fn on_add(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — `+` click (new tab).
  - `pub fn on_close(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Close click (collapses the pane).
  - `pub fn tabs(self, tabs: TabStrip) -> Self` — The tab strip (rendered at the shell height).
- **struct** `SidebarHeader` — Sidebar header cell. Build with `sidebar_header`.
  - `pub fn can_go_back(self, on: bool) -> Self` — Enables the back arrow.
  - `pub fn can_go_forward(self, on: bool) -> Self` — Enables the forward arrow (disabled at 45 % otherwise).
  - `pub fn collapsed(self, collapsed: bool) -> Self` — The sidebar is collapsed to the rail: the cell is only as wide as the rail, so it keeps the window controls (the top-left of a macOS window is theirs whether or not the shell paints them) and drops the navigation actions, which move to the leading edge of the centre header (`CentreHeader::on_expand_sidebar`). [...]
  - `pub fn native_lights(self, on: bool) -> Self` — Reserves the footprint of macOS’s own traffic lights: a leading spacer `NATIVE_LIGHTS_WIDTH` wide with nothing painted in it. [...]
  - `pub fn on_back(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Back arrow click.
  - `pub fn on_forward(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Forward arrow click.
  - `pub fn on_search(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Search click (opens the command palette).
  - `pub fn on_toggle_sidebar(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Sidebar toggle click (⌘B).
  - `pub fn traffic_lights(self, on: bool) -> Self` — Paints the three traffic lights (for the gallery; real windows have native ones).
- **struct** `TabGhost` — The ghost tab that follows the pointer while dragging. Build with `tab_ghost`.
- **struct** `TabItem` — One tab.
  - fields: `id`, `label`, `icon`, `mark`, `badge`, `dirty`, `closable`
  - `pub fn badge(self, badge: impl Into<SharedString>) -> Self` — A tiny pill after the label.
  - `pub fn closable(self, closable: bool) -> Self` — Hides the close affordance (fixed panes).
  - `pub fn dirty(self, dirty: bool) -> Self` — Marks the tab dirty.
  - `pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, icon: IconName) -> Self` — A closable tab with a glyph.
  - `pub fn with_mark(id: impl Into<SharedString>, label: impl Into<SharedString>, provider: Provider) -> Self` — A tab led by a provider mark (an agent’s TUI).
- **struct** `TabStrip` — The tab strip. Build with `tab_strip`.
  - `pub fn after_tabs(self, el: impl IntoElement) -> Self` — A control placed right after the tabs (the `+`).
  - `pub fn in_shell_header(self) -> Self` — The 44 px shell-header variant: no strip padding, 10 px tab padding.
  - `pub fn on_close(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — Called with the tab id when a tab’s close affordance is clicked.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — Called with the tab id when a tab is clicked.
  - `pub fn trailing(self, el: impl IntoElement) -> Self` — A control at the far right of the band (split, overflow).

- **enum** `DockedComposerIntent` — What the composer toolbar asks for.
  - variants: `Send`, `Stop`, `Plus`, `Model`, `Mode`
- **enum** `DropZone` — Where a dragged tab would land.
  - variants: `Left`, `Right`, `Top`, `Bottom`, `Centre`

- **const** `NATIVE_LIGHTS_WIDTH` — Reserved leading footprint: first-light offset + two strides + one diameter + the trailing margin shared with the painted lights.
  - `pub const NATIVE_LIGHTS_WIDTH: f32 = _; // 72f32`
- **const** `NATIVE_LIGHTS_X` — Left edge of the first native light, from the window’s left edge.
  - `pub const NATIVE_LIGHTS_X: f32 = 12.0;`
- **const** `NATIVE_LIGHT_DIAM` — Native traffic-light geometry (macOS metrics: each light is 12 pt across, centres 20 pt apart, the first light’s left edge 12 pt from the window’s left edge). [...]
  - `pub const NATIVE_LIGHT_DIAM: f32 = 12.0;`
- **const** `NATIVE_LIGHT_STRIDE` — Centre-to-centre stride of the native traffic lights.
  - `pub const NATIVE_LIGHT_STRIDE: f32 = 20.0;`
- **const** `RAIL_WIDTH` — The collapsed sidebar rail (⌘B).
  - `pub const RAIL_WIDTH: f32 = 48.0;`
- **const** `RAIL_WIDTH_WITH_LIGHTS` — The collapsed sidebar column when the window’s controls sit above it: the macOS traffic lights own the top-left of the window whether the shell paints them or the system does, so the rail column widen [...]
  - `pub const RAIL_WIDTH_WITH_LIGHTS: f32 = 72.0;`
- **const** `RESIZE_HANDLE_W` — Width of the resize strip: wide enough to grab, transparent so the divider beneath it keeps its own paint.
  - `pub const RESIZE_HANDLE_W: f32 = 6.0;`
- **const** `RIGHT_WIDTH` — Right column width in the app (400; card 10 uses 392).
  - `pub const RIGHT_WIDTH: f32 = 400.0;`
- **const** `SIDEBAR_MAX_WIDTH` — Maximum sidebar width while resizing: keeps the transcript usable.
  - `pub const SIDEBAR_MAX_WIDTH: f32 = 420.0;`
- **const** `SIDEBAR_MIN_WIDTH` — Minimum sidebar width while resizing: rows need ~200 px at 12.5 px text. Clamp every drag move with `clamp_sidebar_width`.
  - `pub const SIDEBAR_MIN_WIDTH: f32 = 180.0;`
- **const** `SIDEBAR_WIDTH` — Sidebar column width (`grid-template-columns: 252px …`).
  - `pub const SIDEBAR_WIDTH: f32 = 252.0;`

### `aui::transcript`

Transcript: markers, user and assistant turns with streaming reveal, thinking block, activity group, tool cards and bodies, approval, question / plan / todo, code and diff blocks, summary / error / st [...]

- **fn** `activity_group` — A group of `steps` with the header `summary` (`Running tests`, `Ran 3 commands`).
  - `pub fn activity_group(id: impl Into<ElementId>, steps: Vec<Step>, summary: impl Into<SharedString>, elapsed: impl Into<SharedString>, state: ActivityState) -> ActivityGroup`
- **fn** `ansi_runs` — Turns spans into text runs in the mono face on the terminal palette.
  - `pub fn ansi_runs(spans: &[AnsiSpan], p: &Palette, mono_family: &'static str) -> (String, Vec<TextRun>)`
- **fn** `answered_row` — The one-line card a settled question collapses to: a success glyph, the chosen answers as chips and a “Change” action. `chips` are the short labels of the chosen options.
  - `pub fn answered_row(id: impl Into<ElementId>, chips: Vec<SharedString>) -> AnsweredRow`
- **fn** `approval_card` — A permission request for `command` run through `tool`, in `state`.
  - `pub fn approval_card(id: impl Into<ElementId>, tool: impl Into<SharedString>, command: impl Into<SharedString>, state: ApprovalState) -> ApprovalCard`
- **fn** `assistant_turn` — An assistant turn rendering `markdown`.
  - `pub fn assistant_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> AssistantTurn`
- **fn** `caret_top_in_line` — The caret’s offset from the top of the line box it closes: centred in the line, then dropped by `vertical-align`.
  - `pub fn caret_top_in_line(line_height: Pixels, caret_height: Pixels, text_scale: f32) -> Pixels`
- **fn** `caret_visible` — Whether the streaming caret is on this frame. `blink 1s steps(2)` is on for the first half of every second; reduced motion holds it on.
  - `pub fn caret_visible(id: impl Into<TransitionId>, window: &mut Window, cx: &mut App) -> bool`
- **fn** `code_block` — A block showing `code` from `path`.
  - `pub fn code_block(id: impl Into<ElementId>, path: impl Into<SharedString>, code: impl Into<SharedString>) -> CodeBlock`
- **fn** `count_label` — `1 call` / `N calls` for the muted header count.
  - `pub fn count_label(calls: usize) -> String`
- **fn** `diff_block` — A unified diff block for `diff`.
  - `pub fn diff_block(id: impl Into<ElementId>, diff: Diff) -> DiffBlock`
- **fn** `diff_note` — `.note`: the note under a diff line, saved or pending.
  - `pub fn diff_note(p: &Palette, id: ElementId, note: &DiffNote, on_action: Option<Rc<dyn Fn(DiffBlockAction, &mut Window, &mut App)>>) -> impl IntoElement`
- **fn** `diff_note_inset` — `diff_note` with host-supplied margins and body metrics, so a pane that insets its notes differently still draws the one note card.
  - `pub fn diff_note_inset(p: &Palette, id: ElementId, note: &DiffNote, insets: NoteInsets, on_action: Option<Rc<dyn Fn(DiffBlockAction, &mut Window, &mut App)>>) -> impl IntoElement`
- **fn** `error_card` — An error card: `title` in 600 over the 12 px `detail` line, framed by the attention border (danger at 70 %, no halo).
  - `pub fn error_card(id: impl Into<ElementId>, title: impl Into<SharedString>, detail: impl Into<SharedString>) -> ErrorCard`
- **fn** `footer_items` — `2.4k tokens` / `$0.04` / `3.1 s` formatting for the footer.
  - `pub fn footer_items(meta: &TurnMeta) -> Vec<SharedString>`
- **fn** `format_duration` — `12.4 s`, `1 m 12 s`, `0.3 s`.
  - `pub fn format_duration(ms: u64) -> SharedString`
- **fn** `generic_item_card` — The kind name, the item’s status and the server’s `fallbackText`.
  - `pub fn generic_item_card(id: impl Into<ElementId>, kind: impl Into<SharedString>, status: impl Into<SharedString>, text: impl Into<SharedString>) -> GenericItemCard`
- **fn** `goal_card` — The objective and the provider’s own status string.
  - `pub fn goal_card(id: impl Into<ElementId>, objective: impl Into<SharedString>, status: impl Into<SharedString>) -> GoalCard`
- **fn** `jump_pill` — A jump pill with `label` (`Jump to latest`); `JumpPill::count` adds the new-turn badge.
  - `pub fn jump_pill(id: impl Into<ElementId>, label: impl Into<SharedString>) -> JumpPill`
- **fn** `last_block_runs` — The text and runs of the block that closes `source`, built exactly as `Markdown` builds them, so a caller that has to measure where the text ends (the streaming caret) shapes the same glyphs that are painted. [...]
  - `pub fn last_block_runs(source: &str, style: &ProseStyle, link_ink: Hsla) -> Option<(String, Vec<TextRun>)>`
- **fn** `markdown` — Renders `source` as markdown blocks in `style`.
  - `pub fn markdown(id: impl Into<ElementId>, source: impl Into<SharedString>, style: ProseStyle) -> Markdown`
- **fn** `markdown_selected_text` — Copies the selected text out of `source` without building a view: the slice of the holding cell’s shaped text, or `None` when the key addresses no cell or the range is empty. [...]
  - `pub fn markdown_selected_text(source: &str, selection: &TextSelection) -> Option<String>`
- **fn** `marker_row` — A marker with plain text; add emphasis, links or a hand-off with the builders.
  - `pub fn marker_row(id: impl Into<ElementId>) -> MarkerRow`
- **fn** `more_label` — `+3 more` for the collapsed preview’s overflow row.
  - `pub fn more_label(hidden: usize) -> String`
- **fn** `needs_you_banner` — A needs-you banner: bold `headline` then plain `detail`, with a jump action.
  - `pub fn needs_you_banner(id: impl Into<ElementId>, headline: impl Into<SharedString>, detail: impl Into<SharedString>) -> NeedsYouBanner`
- **fn** `parse_ansi` — Splits `line` into spans at its SGR escapes.
  - `pub fn parse_ansi(line: &str) -> Vec<AnsiSpan>`
- **fn** `parse_markdown` — Parses `source` into blocks, uncached. Prefer `parsed_markdown`, which memoises this across frames.
  - `pub fn parse_markdown(source: &str) -> Vec<Block>`
- **fn** `parsed_markdown` — Parses `source` into shared blocks, memoised across frames: every render of the same turn hits the memo instead of re-running the parser, which is the per-delta re-parse the transcript diagnosis attributes the scroll jank to. [...]
  - `pub fn parsed_markdown(source: &str, style: &ProseStyle) -> Arc<Vec<Block>> ⓘ`
- **fn** `plan_card` — A proposed plan over `items`; each item is the small markdown subset, so `code` spans render in the mono face.
  - `pub fn plan_card(id: impl Into<ElementId>, items: &[SharedString]) -> PlanCard`
- **fn** `preview_hidden` — Preview rows past the first `GROUP_PREVIEW` collapse into the more row.
  - `pub fn preview_hidden(total: usize) -> usize`
- **fn** `prose` — Renders `markdown` as prose blocks.
  - `pub fn prose(id: impl Into<ElementId>, markdown: &str, style: ProseStyle) -> impl IntoElement`
- **fn** `question_card` — A question with `prompt` and `options`, drawn as radios; call `QuestionCard::multi` for checkboxes.
  - `pub fn question_card(id: impl Into<ElementId>, prompt: impl Into<SharedString>, options: &[QuestionOption]) -> QuestionCard`
- **fn** `retry_row` — The live row a scheduled retry draws while the provider waits out its backoff: `Attempt 2/5 · retrying in 4 s · rate limited`.
  - `pub fn retry_row(id: impl Into<ElementId>, attempt: u32, max: u32, remaining_ms: u64, reason: impl Into<SharedString>) -> StatusRow`
- **fn** `selectable_text` — Builds a selectable text cell: `key` scopes the selection, `text` is the shaped string. [...]
  - `pub fn selectable_text(id: impl Into<ElementId>, key: SelectionKey, text: impl Into<SharedString>) -> SelectableText`
- **fn** `span_runs` — Builds the shaped text, runs and link ranges for `spans`. This is the one run builder for both `prose` and `Markdown`: `link_ink` colours link runs, and callers that never emit links pass a shaping-on [...]
  - `pub fn span_runs(spans: &[Span], style: &ProseStyle, link_ink: Hsla) -> (String, Vec<TextRun>, Vec<LinkRange>)`
- **fn** `status_row` — A status row: `Working… · 12 s · esc to interrupt`.
  - `pub fn status_row(id: impl Into<ElementId>, label: impl Into<SharedString>) -> StatusRow`
- **fn** `summary_card` — A summary headed by `title` (`Done · address validation tightened`) with `meta` on the right (`4 m 12 s · $0.31`).
  - `pub fn summary_card(id: impl Into<ElementId>, title: impl Into<SharedString>, meta: impl Into<SharedString>) -> SummaryCard`
- **fn** `syntax_runs` — Text runs for one line in the mono face, classified by the lexer.
  - `pub fn syntax_runs(line: &str, p: &Palette, mono_family: &'static str) -> Vec<TextRun>`
- **fn** `syntax_runs_in` — `syntax_runs` for a line whose `language` is known; see `tokenize_line_in`.
  - `pub fn syntax_runs_in(line: &str, language: Option<&str>, p: &Palette, mono_family: &'static str) -> Vec<TextRun>`
- **fn** `thinking_block` — A block over the trace `text`, with `elapsed` as shown (`14 s`).
  - `pub fn thinking_block(id: impl Into<ElementId>, text: impl Into<SharedString>, elapsed: impl Into<SharedString>, state: ThinkingState) -> ThinkingBlock`
- **fn** `todo_list` — The agent’s task list over `items`.
  - `pub fn todo_list(id: impl Into<ElementId>, items: Vec<TodoItem>) -> TodoList`
- **fn** `token_color` — The colour for a token class.
  - `pub fn token_color(kind: TokenKind, p: &Palette) -> Hsla`
- **fn** `tokenize_line` — Tokenises one line of `code` into `(byte range, kind)` pairs covering it fully.
  - `pub fn tokenize_line(line: &str) -> Vec<(Range<usize>, TokenKind)>`
- **fn** `tokenize_line_in` — `tokenize_line` for a line whose `language` is known.
  - `pub fn tokenize_line_in(line: &str, language: Option<&str>) -> Vec<(Range<usize>, TokenKind)>`
- **fn** `tool_card` — A card for one tool call.
  - `pub fn tool_card(id: impl Into<ElementId>, verb: impl Into<SharedString>, target: impl Into<SharedString>, status: ToolStatus, body: ToolBody) -> ToolCard`
- **fn** `tool_group` — Consecutive `group` calls under one summary; `open` picks preview rows (`false`) or every call as a full `tool_card` (`true`).
  - `pub fn tool_group(id: impl Into<ElementId>, group: &ToolGroupData, open: bool) -> ToolGroup`
- **fn** `transcript_card` — An empty card; add header parts with `TranscriptCard::header` and a body with `TranscriptCard::body`.
  - `pub fn transcript_card(id: impl Into<ElementId>, open: bool) -> TranscriptCard`
- **fn** `ts_language` — The grammar name gpui-kit’s highlighter knows for `language`, for the six languages the `tree-sitter` feature ships grammars for. [...]
  - `pub fn ts_language(language: &str) -> Option<&'static str>`
- **fn** `turn_selected_text` — Copies the selected text out of a turn’s `markdown_source` without re-rendering: the slice of the holding cell’s shaped text, or `None` when the key addresses no cell or the range is empty. [...]
  - `pub fn turn_selected_text(markdown_source: &str, selection: &TextSelection) -> Option<String>`
- **fn** `user_turn` — A user turn; `markdown` may carry mentions as inline code (`@src/checkout`), which render as mention chips.
  - `pub fn user_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> UserTurn`

- **struct** `ActivityGroup` — The activity group. Build with `activity_group`.
  - `pub fn detail(self, detail: impl Into<SharedString>) -> Self` — The ink-3 detail after the summary (`· read 2 files · edited 1 file`).
  - `pub fn on_toggle(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Header click.
  - `pub fn open(self, open: bool) -> Self` — Whether the timeline is shown.
- **struct** `AnsiSpan` — One coloured run of a line.
  - fields: `text`, `color`, `bold`, `dim`
- **struct** `AnsweredRow` — The answered state of a question. Build with `answered_row`.
  - `pub fn on_change(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — “Change”: the person wants to answer again.
  - `pub fn outcome(self, outcome: QuestionOutcome) -> Self` — How the question settled; the default is `QuestionOutcome::Answered`.
- **struct** `ApprovalCard` — The approval card. Build with `approval_card`.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the card and its buttons are drawn at rest on the first frame (parity captures, restored transcripts).
  - `pub fn badges(self, badges: ApprovalBadges) -> Self` — The header badges: a protected write, an escalation from the judge.
  - `pub fn capabilities(self, capabilities: impl IntoIterator<Item = impl Into<SharedString>>) -> Self` — The capabilities being granted, e.g. `["modify files", "network"]`.
  - `pub fn choices(self, choices: Vec<ApprovalChoice>) -> Self` — The server’s own choice list, in the server’s order.
  - `pub fn cwd(self, cwd: impl Into<SharedString>) -> Self` — The directory the command would run in.
  - `pub fn feedback(self, feedback: impl Into<SharedString>) -> Self` — The feedback that went out with a refusal, quoted on the resolved card.
  - `pub fn feedback_open(self, choice_id: Option<String>) -> Self` — Which choice’s feedback field is open, by `ApprovalChoice::id`.
  - `pub fn feedback_slot(self, slot: impl IntoElement) -> Self` — The feedback field itself — the host’s element, because the card never owns text. The same division as the composer’s editor.
  - `pub fn feedback_text(self, text: impl Into<String>) -> Self` — What the open field currently holds. Data in, so that “Send” can hand it straight back through `ApprovalCard::on_choose` without the card ever keeping a character of it.
  - `pub fn on_choose(self, f: impl Fn(String, Option<String>, &mut Window, &mut App) + 'static) -> Self` — A server-minted choice was pressed: its id, and the feedback typed for it when the open field was confirmed.
  - `pub fn on_decide(self, f: impl Fn(ApprovalDecision, &mut Window, &mut App) + 'static) -> Self` — The person pressed `Deny`, `Always allow` or `Allow once`.
  - `pub fn on_feedback_toggle(self, f: impl Fn(Option<String>, &mut Window, &mut App) + 'static) -> Self` — Open the feedback field for a choice (`Some(id)`) or close it (`None`).
  - `pub fn on_manage_rules(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — The “manage rules” link on an auto-allowed card was clicked.
  - `pub fn present(self, present: bool) -> Self` — Whether the card is on screen; `false` plays the exit.
  - `pub fn reason(self, reason: impl Into<SharedString>) -> Self` — Why the agent wants it, in its own words.
  - `pub fn resolved_by(self, resolved_by: Option<ResolvedBy>) -> Self` — Who settled the request, which is what makes a policy or judge resolution read as one and never actionable.
  - `pub fn rule(self, rule: impl Into<SharedString>) -> Self` — The rule an “always allow” would remember, spelled out on the button.
  - `pub fn scope(self, scope: ApprovalScope) -> Self` — How far an “always allow” would reach.
  - `pub fn stages(self, stages: Vec<ApprovalStage>, current: Option<usize>) -> Self` — The subject’s stages and which one is awaiting a decision.
  - `pub fn title(self, title: impl Into<SharedString>) -> Self` — The pending question, e.g. `"Allow Muse to run this command?"`.
- **struct** `AssistantTurn` — The assistant’s turn. Build with `assistant_turn`.
  - `pub fn actions(self, actions: &[AssistantTurnAction]) -> Self` — The toolbar buttons, in draw order. Defaults to `AssistantTurnAction::ALL`; pass a smaller slice to hide actions that have no meaning for the consumer (a turn without pinning keeps `&[Copy, Retry, Fork]`). [...]
  - `pub fn actions_bottom(self, bottom: bool) -> Self` — In-flow action row under the prose instead of the hover toolbar.
  - `pub fn footer(self, cells: Vec<SharedString>) -> Self` — The footer cells, already formatted, for a caller that keeps them.
  - `pub fn meta(self, meta: TurnMeta) -> Self` — The footer: model · duration · tokens · cost.
  - `pub fn on_action(self, f: impl Fn(AssistantTurnAction, &mut Window, &mut App) + 'static) -> Self` — Toolbar handler.
  - `pub fn on_link(self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self` — Link-click handler, passed through to the markdown body.
  - `pub fn on_selection_change(self, f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static) -> Self` — Selection intents, passed straight through to the inner `markdown(...)`: drags and word / paragraph picks arrive as `Some`, plain clicks elsewhere in a cell arrive as `None` (clearing).
  - `pub fn selection(self, selection: Option<&TextSelection>) -> Self` — The stored selection the markdown body highlights: the app owns one `Option<TextSelection>` per turn and passes it back here, passed straight through to the inner `markdown(...)`.
  - `pub fn streaming(self, streaming: bool) -> Self` — Shows the blinking caret after the text while chunks arrive.
- **struct** `CodeBlock` — A code block. Build with `code_block`.
  - `pub fn hidden_lines(self, count: usize) -> Self` — How many more lines the fold row offers.
  - `pub fn language(self, language: impl Into<SharedString>) -> Self` — The language label after the filename.
  - `pub fn on_action(self, f: impl Fn(CodeBlockAction, &mut Window, &mut App) + 'static) -> Self` — Action handler.
  - `pub fn on_selection_change(self, f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static) -> Self` — Selection intents from any line, translated to block-wide indices. Empty lines carry no bytes, so presses there clear instead.
  - `pub fn selection(self, range: Option<Range<usize>>) -> Self` — The visible selection, in block-wide byte indices. Only the lines it overlaps highlight.
  - `pub fn selection_color(self, color: Hsla) -> Self` — The highlight colour behind selected glyphs. Defaults to the theme’s `selection` token.
  - `pub fn selection_key(self, key: SelectionKey) -> Self` — The selection cell key this block’s lines share. Ranges are byte offsets over the whole block text, newlines included.
  - `pub fn start_line(self, line: u32) -> Self` — The first line number.
- **struct** `DiffBlock` — A diff block. Build with `diff_block`.
  - `pub fn notes(self, notes: Vec<DiffNote>) -> Self` — Notes shown under their lines.
  - `pub fn on_action(self, f: impl Fn(DiffBlockAction, &mut Window, &mut App) + 'static) -> Self` — Action handler.
- **struct** `DiffNote` — A note attached to a diff line.
  - fields: `line`, `text`, `pending`
- **struct** `ErrorCard` — A failure the person may want to retry. Build with `error_card`.
  - `pub fn link(self, label: impl Into<SharedString>, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — A danger-coloured link at the end of the detail line (`details`).
  - `pub fn on_retry(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Shows the retry button and reports its press.
  - `pub fn retry_label(self, label: impl Into<SharedString>) -> Self` — Overrides the retry button’s label.
- **struct** `GenericItemCard` — The fallback card for an unknown item kind. Build with `generic_item_card`.
- **struct** `GoalCard` — The session goal. Build with `goal_card`.
  - `pub fn current_work(self, work: impl Into<SharedString>) -> Self` — What the agent says it is doing now.
  - `pub fn next_work(self, work: impl Into<SharedString>) -> Self` — What it says it will do next.
  - `pub fn percent(self, percent: Option<f32>) -> Self` — How far along the provider says it is, verbatim.
- **struct** `HandOff` — A hand-off between two agents, shown as a pill with both marks.
  - fields: `from`, `to`
- **struct** `JumpPill` — The floating jump-to-latest pill. Build with `jump_pill`.
  - `pub fn count(self, count: u32) -> Self` — The number of new turns below the reader.
  - `pub fn on_jump(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — The pill was pressed.
- **struct** `LinkRange` — A link’s byte range inside its paragraph’s shaped text, with its target.
  - fields: `range`, `target`
- **struct** `Markdown` — A markdown block column. Build with `markdown`.
  - `pub fn on_link(self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self` — Click handler for links: the argument is the clicked range’s target.
  - `pub fn on_selection_change(self, f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static) -> Self` — Selection intents: drags and word / paragraph picks arrive as `Some`, plain clicks elsewhere in a cell arrive as `None` (clearing).
  - `pub fn selected_text(&self, selection: &TextSelection) -> Option<String>` — Copies the selected text out of `selection`: the slice of the holding cell’s shaped text (code spans and link labels read as plain words), or `None` when the key addresses no cell or the range is empty. [...]
  - `pub fn selection(self, selection: Option<&TextSelection>) -> Self` — The stored selection this render highlights: the app owns one `Option<TextSelection>` per markdown view and passes it back here.
- **struct** `MarkerRow` — A marker row. Build with `marker_row`.
  - `pub fn glyph(self, name: IconName, color: Option<Hsla>) -> Self` — The 12 px leading glyph, optionally tinted (a warning shield).
  - `pub fn hand_off(self, hand_off: HandOff) -> Self` — The hand-off pill before the text.
  - `pub fn link(self, text: impl Into<SharedString>, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — An accent-ink link.
  - `pub fn strong(self, text: impl Into<SharedString>) -> Self` — Emphasised text: ink, weight 500.
  - `pub fn text(self, text: impl Into<SharedString>) -> Self` — Plain ink-3 text.
- **struct** `NeedsYouBanner` — The banner pinned above the composer when the agent is blocked on the person. Build with `needs_you_banner`.
  - `pub fn action_label(self, label: impl Into<SharedString>) -> Self` — Overrides the action’s label.
  - `pub fn at_rest(self) -> Self` — Skips the enter animation and draws the settled banner (static captures).
  - `pub fn on_jump(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — The action was pressed.
- **struct** `NoteInsets` — Geometry a host can override on `diff_note_inset`. The defaults are the transcript diff block’s own values (card 37).
  - fields: `margin`, `line_height`, `caps_gap`
- **struct** `PlanCard` — The plan card. Build with `plan_card`.
  - `pub fn on_accept(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — “Accept and run”.
  - `pub fn on_edit(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — “Edit”: the person wants to change the plan text first.
  - `pub fn on_reject(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — “Reject”: the agent should propose something else.
  - `pub fn sections(self, sections: Vec<PlanSection>) -> Self` — The unnumbered labels that group the steps.
  - `pub fn state(self, state: PlanState) -> Self` — Where the plan is in its lifecycle; the action row is only drawn while the plan is `PlanState::Proposed`.
- **struct** `ProseStyle` — Colours and sizes for a prose block.
  - fields: `ink`, `code_ink`, `code_bg`, `size`, `line_height`, `paragraph_gap`
- **struct** `QuestionCard` — The pending question card. Build with `question_card`.
  - `pub fn allow_other(self, allow: bool) -> Self` — Whether the dashed “Other, type your own…” row is offered.
  - `pub fn clarify_open(self, open: bool) -> Self` — Whether the “Explain instead” field is open.
  - `pub fn clarify_slot(self, slot: impl IntoElement) -> Self` — The clarification field itself — the host’s element, as the approval card’s feedback field is.
  - `pub fn header(self, header: impl Into<SharedString>) -> Self` — The short label above the prompt (MSP `UserInputQuestion.header`).
  - `pub fn hint(self, hint: impl Into<SharedString>) -> Self` — Overrides the action-row hint (the default counts the selection).
  - `pub fn limits(self, min: Option<usize>, max: Option<usize>) -> Self` — How many options a multi-select must gather: `(min, max)`.
  - `pub fn multi(self, multi: bool) -> Self` — Checkboxes instead of radios (`Block::Question::multi`).
  - `pub fn on_answer(self, f: impl Fn(Answer, &mut Window, &mut App) + 'static) -> Self` — “Continue”: the current selection, as an `Answer`.
  - `pub fn on_clarify(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — “Explain instead”: the person would rather write than pick (MSP `userInput/clarify`).
  - `pub fn on_other(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — The “Other” row was clicked; the host opens a free-text field.
  - `pub fn on_select(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — An option row was clicked; the host toggles or replaces the selection.
  - `pub fn on_skip(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — “Skip”: the person declined to answer.
  - `pub fn on_toggle_preview(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — An option’s “Preview” chevron was clicked.
  - `pub fn previews_open(self, open: Vec<usize>) -> Self` — Which options have their preview expanded, as indices into `options`.
  - `pub fn selected(self, selected: Vec<usize>) -> Self` — Which options are currently selected, as indices into `options`.
  - `pub fn subtitle(self, subtitle: impl Into<SharedString>) -> Self` — The supporting line under the prompt.
  - `pub fn timeout(self, remaining_ms: u64, total_ms: u64) -> Self` — The auto-resolution countdown: how long is left, out of how long there was. The host ticks the clock; the card only draws the pill.
- **struct** `SelectableText` — One selectable run of shaped text. Build with `selectable_text`.
  - `pub fn links(self, links: Vec<LinkRange>) -> Self` — Clickable link ranges with their targets, in local byte indices. A press-release without movement on one fires `SelectableText::on_link`; movement starts a selection instead.
  - `pub fn on_link(self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self` — Fires when a press-release without movement lands on a link range.
  - `pub fn on_selection_change(self, f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static) -> Self` — Fires on drags and word / paragraph picks (`Some`) and on plain clicks elsewhere in the cell (`None`, clearing the selection).
  - `pub fn runs(self, runs: Vec<TextRun>) -> Self` — The text runs (same shape as `StyledText::with_runs`).
  - `pub fn selection(self, range: Option<Range<usize>>) -> Self` — The visible selection, in local byte indices; `None` (the default) paints plain text. The range splits the runs around it at paint time.
  - `pub fn selection_color(self, color: Hsla) -> Self` — The highlight colour behind selected glyphs — the theme’s `selection` token. Without it a selection range paints nothing.
- **struct** `SelectionKey` — Identifies one selectable cell inside a `Markdown` render: a paragraph, heading, list item, table cell or fenced code block. [...]
  - `pub fn as_str(&self) -> &str` — The encoded key.
  - `pub fn code(prefix: &str, index: usize) -> Self` — Key of the `index`-th fenced code block under `prefix`. Every line of the block shares this key; ranges are byte offsets over the whole block text (newlines included).
  - `pub fn heading(prefix: &str, index: usize) -> Self` — Key of the `index`-th heading under `prefix`.
  - `pub fn list_item(prefix: &str, index: usize, ordered: bool, item: usize) -> Self` — Key of one list item: `index` is the block, `item` the row; `ordered` picks the `o` (numbered) or `b` (bullet) arm.
  - `pub fn new(name: impl Into<String>) -> Self` — Builds a key from its encoded form (`p0`, `q2-p1`, …).
  - `pub fn paragraph(prefix: &str, index: usize) -> Self` — Key of the `index`-th paragraph under `prefix`.
  - `pub fn quote_prefix(prefix: &str, index: usize) -> String` — The key prefix for blocks nested inside the `index`-th quote: inner keys read `q{index}-…`, so quote cells never collide with siblings.
  - `pub fn table_cell(prefix: &str, index: usize, row: Option<usize>, col: usize) -> Self` — Key of one table cell: `index` is the block, `row` is `None` for a header cell and `Some` for a body row, `col` the column.
- **struct** `StatusRow` — One live status line under the transcript. Build with `status_row`.
  - `pub fn elapsed(self, elapsed: impl Into<SharedString>) -> Self` — The mono elapsed time after the label.
  - `pub fn key_hint(self, key: impl Into<SharedString>, text: impl Into<SharedString>) -> Self` — A keycap and its trailing text after the separator (`esc to interrupt`).
  - `pub fn lead(self, lead: StatusLead) -> Self` — The leading glyph.
  - `pub fn note(self, note: impl Into<SharedString>) -> Self` — A trailing note after the separator (`1 queued message`).
  - `pub fn shimmer(self, shimmer: bool) -> Self` — Whether the label shimmers (it does while the agent is working).
- **struct** `SummaryCard` — The end-of-work summary card. Build with `summary_card`.
  - `pub fn checks(self, checks: Vec<Check>) -> Self` — The verifications that ran.
  - `pub fn files(self, files: Vec<FileChange>) -> Self` — The changed files, in display order.
  - `pub fn on_action(self, f: impl Fn(SummaryAction, &mut Window, &mut App) + 'static) -> Self` — The action row’s intent.
- **struct** `TextSelection` — A text selection inside one markdown cell: which cell, and the byte range over that cell’s shaped text. An empty range is never stored — clearing is `None`.
  - fields: `cell`, `range`
- **struct** `ThinkingBlock` — The thinking block. Build with `thinking_block`.
  - `pub fn expanded(self, expanded: bool) -> Self` — Whether a finished trace is expanded to its full text.
  - `pub fn on_toggle(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Header click.
  - `pub fn summary(self, summary: impl Into<SharedString>) -> Self` — The one-line summary shown once done.
- **struct** `TodoList` — The todo list. Build with `todo_list`.
  - `pub fn on_toggle(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Header click: collapse or expand the list.
  - `pub fn open(self, open: bool) -> Self` — Whether the rows are shown.
  - `pub fn title(self, title: impl Into<SharedString>) -> Self` — Overrides the header label (`Tasks`).
- **struct** `ToolCard` — A tool call card. Build with `tool_card`.
  - `pub fn duration_ms(self, ms: Option<u64>) -> Self` — The duration shown in the header, formatted by `format_duration` once per card rather than once per frame.
  - `pub fn on_intent(self, f: impl Fn(ToolCardIntent, &mut Window, &mut App) + 'static) -> Self` — Intent handler.
  - `pub fn open(self, open: bool) -> Self` — Whether the body is shown.
- **struct** `ToolGroup` — A group of tool calls. Build with `tool_group`.
  - `pub fn call_open(self, index: usize, open: bool) -> Self` — Whether one call’s full card is open (all are, by default).
  - `pub fn on_intent(self, f: impl Fn(ToolGroupIntent, &mut Window, &mut App) + 'static) -> Self` — Intent handler: `ToolGroupIntent::Toggle` for the header, and one `ToolGroupIntent::Call` per call card.
- **struct** `ToolGroupData` — The data a tool group renders.
  - fields: `calls`, `summary`, `state`
  - `pub fn from_block(block: &Block) -> Option<Self>` — The data of a `Block::ToolGroup`; `None` for any other variant.
- **struct** `TranscriptCard` — A collapsible transcript card. Build with `transcript_card`.
  - `pub fn body(self, el: impl IntoElement) -> Self` — The body, drawn behind a 1 px top border and collapsed when closed.
  - `pub fn body_border(self, border: bool) -> Self` — Drops the 1 px line between header and body (the thinking block).
  - `pub fn chevron(self, show: bool) -> Self` — Hides the chevron (cards that never collapse).
  - `pub fn header(self, el: impl IntoElement) -> Self` — Appends a header part (glyph, label, spacer, meta).
  - `pub fn header_height(self, height: impl Into<Pixels>) -> Self` — Overrides the 34 px header (the thinking block and todo list use 32).
  - `pub fn hover_tint(self, tint: bool) -> Self` — Disables the header hover tint (non-interactive headers).
  - `pub fn on_toggle(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` — Header click.
- **struct** `UserTurn` — The person’s turn. Build with `user_turn`.
  - `pub fn actions(self, actions: &[UserTurnAction]) -> Self` — The action buttons, in draw order. Defaults to `UserTurnAction::ALL`; pass a smaller slice (or an empty one) to hide actions that have no meaning for the consumer. [...]
  - `pub fn actions_bottom(self, bottom: bool) -> Self` — In-flow action row under the bubble instead of the hover rail.
  - `pub fn attachments(self, attachments: Vec<Attachment>) -> Self` — Attachments shown above the bubble.
  - `pub fn on_action(self, f: impl Fn(UserTurnAction, &mut Window, &mut App) + 'static) -> Self` — Hover-action handler.
  - `pub fn on_link(self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self` — Link-click handler, passed through to the markdown body.
  - `pub fn on_selection_change(self, f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static) -> Self` — Selection intents, passed straight through to the inner `markdown(...)`: drags and word / paragraph picks arrive as `Some`, plain clicks elsewhere in a cell arrive as `None` (clearing).
  - `pub fn selection(self, selection: Option<&TextSelection>) -> Self` — The stored selection the markdown body highlights: the app owns one `Option<TextSelection>` per turn and passes it back here, passed straight through to the inner `markdown(...)`.

- **enum** `AssistantTurnAction` — Actions on an assistant turn.
  - variants: `Copy`, `Retry`, `Fork`, `Pin`
- **enum** `Block` — One block of a parsed transcript.
  - variants: `Paragraph`, `Heading`, `BulletList`, `OrderedList`, `CodeBlock`, `Table`,
    `Quote`, `Rule`, `Image`
- **enum** `CodeBlockAction` — Actions on a code block header.
  - variants: `Wrap`, `Open`, `Copy`, `Unfold`
- **enum** `DiffBlockAction` — Actions on a diff block.
  - variants: `Unified`, `Split`, `OpenInDiff`, `AddNote`, `SaveNote`, `CancelNote`
- **enum** `LinkTarget` — Where a link points.
  - variants: `Url`, `Path`
- **enum** `MarkdownBlock` — One block of a parsed transcript.
  - variants: `Paragraph`, `Heading`, `BulletList`, `OrderedList`, `CodeBlock`, `Table`,
    `Quote`, `Rule`, `Image`
- **enum** `MarkdownSpan` — One inline segment.
  - variants: `Text`, `Code`, `Bold`, `Italic`, `Strikethrough`, `Link`
- **enum** `QuestionOutcome` — How a question settled, which is what the collapsed row says it did.
  - variants: `Answered`, `Skipped`, `Clarified`, `TimedOut`, `Interrupted`
- **enum** `Span` — One inline segment.
  - variants: `Text`, `Code`, `Bold`, `Italic`, `Strikethrough`, `Link`
- **enum** `StatusLead` — The glyph that leads a `StatusRow`.
  - variants: `None`, `Spinner`, `Braille`
- **enum** `SummaryAction` — What the summary card’s action row asks for.
  - variants: `CreatePr`, `Commit`, `ReviewDiff`
- **enum** `TableAlign` — Column alignment of a table, from the delimiter row.
  - variants: `None`, `Left`, `Center`, `Right`
- **enum** `TokenKind` — A token class.
  - variants: `Plain`, `Keyword`, `Function`, `String`, `Number`, `Comment`
- **enum** `ToolCardIntent` — What the card’s fold rows and header ask for.
  - variants: `Toggle`, `Unfold`, `OpenInPane`
- **enum** `ToolGroupIntent` — What a tool group asks for.
  - variants: `Toggle`, `Call`
- **enum** `UserTurnAction` — Actions on a user turn.
  - variants: `Edit`, `Copy`, `Resend`

- **const** `CARET_BASELINE_DROP` — `vertical-align:-3px`: the caret’s box hangs 3 px below the text baseline.
  - `pub const CARET_BASELINE_DROP: f32 = 3.0;`
- **const** `CARET_H` — The caret’s height.
  - `pub const CARET_H: f32 = 15.0;`
- **const** `CARET_MARGIN_LEFT` — The caret’s gap from the last glyph.
  - `pub const CARET_MARGIN_LEFT: f32 = 1.0;`
- **const** `CARET_W` — `.caret{width:2px;height:15px;background:var(--accent);vertical-align:-3px; margin-left:1px;animation:blink 1s steps(2) infinite}` — the streaming caret’s geometry, shared by every turn that can strea [...]
  - `pub const CARET_W: f32 = 2.0;`
- **const** `GROUP_PREVIEW` — Collapsed preview rows before the `+k more` row (the image3 idiom: two rows, then the overflow count).
  - `pub const GROUP_PREVIEW: usize = 2;`
- **const** `SHELL_FOLD` — Shell output folds after this many lines (the card shows six, then `14 more lines`).
  - `pub const SHELL_FOLD: usize = 6;`

- **type** `LinkHandler` — What `Markdown::on_link` receives.
  - `pub type LinkHandler = Rc<dyn Fn(LinkTarget, &mut Window, &mut App)>;`
- **type** `SelectionHandler` — A selection intent: `Some` replaces the app’s stored selection, `None` clears it.
  - `pub type SelectionHandler = Rc<dyn Fn(Option<TextSelection>, &mut Window, &mut App)>;`

### `aui::util`

Small helpers shared by the components: per-element interaction state (hover / press) kept in the window, keyed by element id.

- **fn** `interaction` — The interaction state for `id`, created on first use.
  - `pub fn interaction(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Entity<Interaction>`
- **fn** `interaction_flags` — Reads the current flags without keeping the entity.
  - `pub fn interaction_flags(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> (Entity<Interaction>, Interaction)`

- **struct** `Interaction` — Hover and press flags for one interactive element. Lives in the window’s element state, so stateless (`RenderOnce`) components can animate hover tints and press springs without owning a view.
  - fields: `hovered`, `pressed`

- **trait** `TrackInteraction` — Wires hover / press tracking into a stateful element.
  - `fn track_interaction(self, state: &Entity<Interaction>) -> Self` — Updates `state` from hover, mouse-down and mouse-up events.

- **type** `ClickHandler` — A click handler stored by a component builder.
  - `pub type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;`

### `aui::workbench`

Workbench: the block terminal and TUI pane, browser with annotator, diff review, git and PR forms, file tree and document panes, sources and citations (cards 50–55, spec §5).

- **fn** `agent_action_pill` — The pill shown over the page while an agent drives it; `text` shimmers.
  - `pub fn agent_action_pill(id: impl Into<ElementId>, text: impl Into<SharedString>) -> AgentActionPill`
- **fn** `annotation_pin` — The 20 px teardrop pin carrying `index`, dropped at an element’s top-right.
  - `pub fn annotation_pin(index: usize) -> AnnotationPin`
- **fn** `annotations_panel` — The 272 px panel collecting `annotations`, with the screenshot preview, the metadata note and the send action.
  - `pub fn annotations_panel(id: impl Into<ElementId>, annotations: Vec<Annotation>) -> AnnotationsPanel`
- **fn** `artifact_strip` — The strip of artifacts the chat produced, under the page.
  - `pub fn artifact_strip(id: impl Into<ElementId>, artifacts: Vec<Artifact>) -> ArtifactStrip`
- **fn** `block_terminal` — A terminal pane over `blocks`.
  - `pub fn block_terminal(id: impl Into<ElementId>, blocks: Vec<TermBlock>) -> BlockTerminal`
- **fn** `browser_nav` — The nav row for `url`: history controls, the URL field, the annotate toggle, screenshot and console.
  - `pub fn browser_nav(id: impl Into<ElementId>, url: impl Into<SharedString>) -> BrowserNav`
- **fn** `citation` — A citation marker for source `index`. Its interaction state is keyed by the index, so one answer shows each number once.
  - `pub fn citation(index: u8) -> Citation`
- **fn** `cited_answer` — An answer paragraph; write markers as `[[1]]` where the HTML has a `<span class="cite">`.
  - `pub fn cited_answer(id: impl Into<ElementId>, text: impl Into<SharedString>, style: ProseStyle) -> CitedAnswer`
- **fn** `diff_review` — A review pane showing `diff` for the file selected in `files`, with `notes` collected across the change set.
  - `pub fn diff_review(id: impl Into<ElementId>, files: Vec<ReviewFile>, diff: Diff, notes: Vec<ReviewNote>, scope: DiffScope, view: DiffView) -> DiffReview`
- **fn** `doc_pane` — The pane’s page on its surface-2 ground.
  - `pub fn doc_pane(id: impl Into<ElementId>, page: DocPage) -> DocPane`
- **fn** `doc_saved_hint` — The “Saved” hint that sits at the right of the tab band (`.subtle`, 11 px).
  - `pub fn doc_saved_hint(text: impl Into<SharedString>, cx: &App) -> impl IntoElement`
- **fn** `doc_tabs` — The pane’s tabs, one per open document, with `active` selected. [...]
  - `pub fn doc_tabs(id: impl Into<ElementId>, tabs: Vec<TabItem>, active: usize) -> DocTabs`
- **fn** `doc_toolbar` — The toolbar with the paragraph style (“Body text”) and font (“Georgia · 11”) selects; the marks, insert actions, ask chip and export button are fixed by the design.
  - `pub fn doc_toolbar(id: impl Into<ElementId>, style_label: impl Into<SharedString>, font_label: impl Into<SharedString>) -> DocToolbar`
- **fn** `element_outline` — An outline over a page element, labelled `label` (`p · 392 × 34`). [...]
  - `pub fn element_outline(label: impl Into<SharedString>) -> ElementOutline`
- **fn** `file_tree` — A file tree over a flat list of pre-expanded rows.
  - `pub fn file_tree(id: impl Into<ElementId>, nodes: Vec<FileNode>) -> FileTree`
- **fn** `git_changes` — The Changes panel: every changed file with its staged flag, the drafted commit `message`, and how far the branch is `ahead` of and `behind` its remote.
  - `pub fn git_changes(id: impl Into<ElementId>, files: Vec<(FileChange, bool)>, message: impl Into<SharedString>, ahead: u32, behind: u32) -> GitChanges`
- **fn** `note_popover` — The 236 px popover that opens on a pinned element: its path, the note being typed, Cancel and Save note.
  - `pub fn note_popover(id: impl Into<ElementId>, path: impl Into<SharedString>, draft: impl Into<SharedString>) -> NotePopover`
- **fn** `pane_status` — A plain status item.
  - `pub fn pane_status(text: impl Into<SharedString>) -> PaneStatusItem`
- **fn** `pane_status_row` — A 28 px status row with items on the left and on the right of a spacer.
  - `pub fn pane_status_row(id: impl Into<ElementId>, left: Vec<PaneStatusItem>, right: Vec<PaneStatusItem>) -> PaneStatusRow`
- **fn** `pdf_pane` — A pane showing `page`, page `page_no` of `page_count`.
  - `pub fn pdf_pane(id: impl Into<ElementId>, page: PdfPage, page_no: u32, page_count: u32) -> PdfPane`
- **fn** `pr_check` — A finished check.
  - `pub fn pr_check(label: impl Into<String>, passed: bool) -> PrCheck`
- **fn** `pr_form` — The pull-request form: the `base` branch the PR targets, its `title`, the `description` runs and the `checks` from the last push.
  - `pub fn pr_form(id: impl Into<ElementId>, base: impl Into<SharedString>, title: impl Into<SharedString>, description: Vec<PrDescription>, checks: Vec<PrCheck>) -> PrForm`
- **fn** `segmented` — A segmented control over `labels` with `active` selected.
  - `pub fn segmented(id: impl Into<ElementId>, labels: Vec<SharedString>, active: usize) -> Segmented`
- **fn** `sheet_pane` — A pane over `rows` (row 0 is the bold header row when `header_row`).
  - `pub fn sheet_pane(id: impl Into<ElementId>, columns: Vec<SharedString>, rows: Vec<Vec<SheetCell>>) -> SheetPane`
- **fn** `source_hover_card` — A hover card for one source: its title, the quoted passage with the matched span (a byte range into `quote`) on accent-soft, and the two actions.
  - `pub fn source_hover_card(id: impl Into<ElementId>, title: impl Into<SharedString>, quote: impl Into<SharedString>, highlight: Range<usize>, page: u32) -> SourceHoverCard`
- **fn** `sources_card` — The sources card for `tiers`.
  - `pub fn sources_card(id: impl Into<ElementId>, tiers: Vec<SourceTier>) -> SourcesCard`
- **fn** `tui_pane` — A pane over the TUI’s `lines` (ANSI escapes allowed; bold via SGR 1).
  - `pub fn tui_pane(id: impl Into<ElementId>, lines: Vec<String>) -> TuiPane`

- **struct** `AgentActionPill` — The agent-action pill. Build with `agent_action_pill`.
  - `pub fn detail(self, detail: impl Into<SharedString>) -> Self` — The quiet trailing clause (`· verifying signup flow`).
  - `pub fn provider(self, provider: Provider) -> Self` — Which agent is driving (default Claude).
- **struct** `Annotation` — One annotation in the side panel.
  - fields: `index`, `selector`, `note`, `pending`
  - `pub fn new(index: usize, selector: impl Into<SharedString>, note: impl Into<SharedString>) -> Self` — A saved annotation.
  - `pub fn pending(self, pending: bool) -> Self` — Marks it as still being written.
- **struct** `AnnotationPin` — A numbered pin. Build with `annotation_pin`.
  - `pub fn selected(self, selected: bool) -> Self` — The pin of the selected annotation: a white ring around the teardrop.
- **struct** `AnnotationsPanel` — The annotations side panel. Build with `annotations_panel`.
  - `pub fn agent(self, provider: Provider, name: impl Into<SharedString>) -> Self` — The agent the batch is sent to (mark and name on the primary button).
  - `pub fn on_action(self, f: impl Fn(AnnotatorAction, &mut Window, &mut App) + 'static) -> Self` — Called with the `AnnotatorAction` a control stands for.
  - `pub fn screenshot(self, pins: Option<Vec<(f32, f32)>>) -> Self` — The screenshot preview tile: the pins’ positions as fractions of the tile (`None` hides the tile).
  - `pub fn selected(self, selected: Option<usize>) -> Self` — The highlighted row.
- **struct** `Artifact` — One artifact the chat created.
  - fields: `name`, `kind`, `version`, `active`
  - `pub fn active(self, active: bool) -> Self` — Marks the artifact as the one open in the pane.
  - `pub fn new(name: impl Into<SharedString>, kind: ArtifactKind) -> Self` — An artifact chip.
  - `pub fn version(self, version: impl Into<SharedString>) -> Self` — Sets the version tag.
- **struct** `ArtifactStrip` — The “Created in chat” strip (`.arts`). Build with `artifact_strip`.
  - `pub fn label(self, label: impl Into<SharedString>) -> Self` — Overrides the caps label.
  - `pub fn max_visible(self, n: usize) -> Self` — Shows at most `n` chips and gathers the rest behind a `+N` chip (`.art.more`). [...]
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — Called with the artifact’s name when a chip is clicked.
- **struct** `BlockTerminal` — The block terminal. Build with `block_terminal`.
  - `pub fn marker(self, text: impl Into<SharedString>) -> Self` — The restored-scrollback marker at the top.
  - `pub fn on_action(self, f: impl Fn(TerminalAction, &mut Window, &mut App) + 'static) -> Self` — Action handler.
  - `pub fn prompt(self, prompt: TermPrompt) -> Self` — The prompt row at the bottom.
  - `pub fn track_scroll(self, handle: ScrollHandle) -> Self` — Track the block list’s scroll position with `handle`, so a live session can follow its tail (`handle.scroll_to_bottom()`) and a caller can tell whether the user has scrolled away from it. [...]
- **struct** `BrowserNav` — The 38 px nav row. Build with `browser_nav`.
  - `pub fn annotating(self, annotating: bool) -> Self` — Annotate mode: the toggle is ink-filled and shows the `esc` cap.
  - `pub fn can_go_back(self, can: bool) -> Self` — Enables the back control (default true).
  - `pub fn can_go_forward(self, can: bool) -> Self` — Enables the forward control (default false: disabled at 45 %).
  - `pub fn loading(self, progress: f32) -> Self` — Load progress 0–1; anything above 0 draws the accent hairline.
  - `pub fn on_action(self, f: impl Fn(BrowserAction, &mut Window, &mut App) + 'static) -> Self` — Called with the `BrowserAction` a control stands for.
  - `pub fn secure(self, secure: bool) -> Self` — Whether the URL field shows the success shield (default true).
- **struct** `Citation` — An inline citation marker (`.cite`): the small accent square carrying a source number. Build with `citation`.
  - `pub fn on_open(self, f: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self` — The marker was clicked; the argument is the source number.
- **struct** `CitedAnswer` — An assistant answer with inline citation markers (`.a p`). Build with `cited_answer`.
  - `pub fn on_open(self, f: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self` — A marker was clicked; the argument is the source number.
  - `pub fn streaming(self, streaming: bool) -> Self` — Shows the blinking caret after the last word while chunks arrive. The answer is a wrapping row of word groups, so the caret is simply the last group and needs no measuring.
- **struct** `DiffHighlight` — A word-level `diff-add-strong` / `diff-del-strong` span inside one row, addressed by hunk and row index with byte offsets into `aui_protocol::DiffLine::text`.
  - fields: `hunk`, `line`, `start`, `end`
- **struct** `DiffReview` — The diff review pane. Build with `diff_review`.
  - `pub fn highlights(self, highlights: Vec<DiffHighlight>) -> Self` — Word-level highlights inside diff rows.
  - `pub fn on_action(self, f: impl Fn(DiffReviewAction, &mut Window, &mut App) + 'static) -> Self` — Action handler.
  - `pub fn summary(self, lead: impl Into<SharedString>, added: u32, removed: u32) -> Self` — The summary beside the scope control: a lead (`"vs main · 3 files"`) followed by the totals, tinted success and danger.
- **struct** `DocPage` — The page the document pane shows.
  - fields: `title`, `subtitle`, `blocks`
  - `pub fn new(title: impl Into<SharedString>, subtitle: impl Into<SharedString>, blocks: Vec<DocBlock>) -> Self` — A page.
- **struct** `DocPane` — The paper area of the document pane (`.doc` + `.paper`). Build with `doc_pane`.
  - `pub fn paper(self, width: f32, pad_y: f32, pad_x: f32) -> Self` — Overrides the 520 px page (the assistant screens use 340 with 34 / 36 padding).
- **struct** `DocTabs` — The document pane’s tab band (`.dtabs`). Build with `doc_tabs`.
  - `pub fn on_select(self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self` — Called with the tab id when a tab is clicked.
  - `pub fn trailing(self, el: impl IntoElement) -> Self` — A control at the far right of the band (the saved hint, the overflow button).
- **struct** `DocToolbar` — The document toolbar (`.tool`). Build with `doc_toolbar`.
  - `pub fn ask_label(self, label: impl Into<SharedString>) -> Self` — Overrides the sparkle chip’s label.
  - `pub fn export_label(self, label: impl Into<SharedString>) -> Self` — Overrides the export button’s label.
  - `pub fn on_action(self, f: impl Fn(&DocToolbarAction, &mut Window, &mut App) + 'static) -> Self` — Called with every intent the toolbar emits.
  - `pub fn without_export(self) -> Self` — Drops the export button, the way the assistant screens’ narrow pane does: export lives in the tab band’s overflow menu there, and the row keeps every remaining control at its designed width.
  - `pub fn without_font_select(self) -> Self` — Drops the font select. The toolbar’s controls keep their width rather than shrinking, so in a pane as narrow as the shell’s right pane (`shell::RIGHT_WIDTH`) the row cannot hold both selects and the a [...]
- **struct** `ElementOutline` — The hovered / selected element overlay. Build with `element_outline`.
  - `pub fn selected(self, selected: bool) -> Self` — The clicked element: a 1.5 px outline, no fill and no label.
  - `pub fn size(self, width: impl Into<Pixels>, height: impl Into<Pixels>) -> Self` — The element’s box.
- **struct** `FileNode` — One row of the tree. Directories carry `open`; files leave it `None`.
  - fields: `id`, `name`, `kind`, `depth`, `open`, `badge`, `selected`
  - `pub fn badge(self, badge: GitBadge) -> Self` — Sets the git badge.
  - `pub fn depth(self, depth: usize) -> Self` — Sets the nesting level.
  - `pub fn dir(id: impl Into<SharedString>, name: impl Into<SharedString>, open: bool) -> Self` — A directory row; `open` picks the open / closed folder icon and the chevron’s rotation.
  - `pub fn file(id: impl Into<SharedString>, name: impl Into<SharedString>, kind: FileType) -> Self` — A file row.
  - `pub fn is_dir(&self) -> bool` — Whether the row is a directory.
  - `pub fn selected(self, selected: bool) -> Self` — Marks the row selected.
- **struct** `FileTree` — The file tree. Build with `file_tree`.
  - `pub fn footer(self, text: impl Into<SharedString>) -> Self` — The footer status line (“worktree checkout-flow-v2 · 2 modified · 1 added”).
  - `pub fn header(self, name: impl Into<SharedString>) -> Self` — The header’s repository or root name; without it the header is hidden.
  - `pub fn on_action(self, f: impl Fn(&FileTreeAction, &mut Window, &mut App) + 'static) -> Self` — Called with every intent the tree emits.
- **struct** `GitChanges` — The Changes panel. Build with `git_changes`.
  - `pub fn branch(self, branch: impl Into<SharedString>) -> Self` — The branch name shown in the ahead / behind row.
  - `pub fn on_action(self, f: impl Fn(GitAction, &mut Window, &mut App) + 'static) -> Self` — The panel’s intents: toggling a file, redrafting, amending, committing and pushing.
- **struct** `NotePopover` — The note popover. Build with `note_popover`.
  - `pub fn on_action(self, f: impl Fn(NoteAction, &mut Window, &mut App) + 'static) -> Self` — Called with `NoteAction` when Cancel or Save note is clicked.
  - `pub fn visible(self, visible: bool) -> Self` — Whether the popover is open; it fades and rises 4 px on the enter timing.
- **struct** `PaneStatusItem` — One item of a pane status row.
  - fields: `text`, `accent`
  - `pub fn accent(self) -> Self` — Colours the item accent-ink.
- **struct** `PaneStatusRow` — The pane status row (`.stat`). Build with `pane_status_row`.
- **struct** `PdfPage` — A page: heading, paragraphs, footer.
  - fields: `heading`, `paragraphs`, `footer`
- **struct** `PdfPane` — The PDF pane. Build with `pdf_pane`.
  - `pub fn cited_as(self, n: u8) -> Self` — Shows the `cited as N` pill.
  - `pub fn on_action(self, f: impl Fn(PdfAction, &mut Window, &mut App) + 'static) -> Self` — Action handler.
  - `pub fn zoom(self, zoom: impl Into<SharedString>) -> Self` — The zoom label.
- **struct** `PrCheck` — One row of the “checks on last push” list: a protocol `Check`, whether it is still running, and the trailing detail the card prints after a `·`.
  - fields: `check`, `running`, `detail`
  - `pub fn detail(self, detail: impl Into<SharedString>) -> Self` — The detail after the name.
  - `pub fn running(self) -> Self` — Marks the check as still running.
- **struct** `PrForm` — The Create pull request form. Build with `pr_form`.
  - `pub fn issue(self, issue: impl Into<SharedString>) -> Self` — The tracker issue chip in the footer (`Linear ACM-412`).
  - `pub fn on_action(self, f: impl Fn(PrAction, &mut Window, &mut App) + 'static) -> Self` — The form’s intents: picking the base branch, cancelling, creating, and opening the linked issue.
  - `pub fn provider(self, provider: impl Into<SharedString>) -> Self` — The forge chip in the header (`GitHub`).
- **struct** `ReviewFile` — One row of the changed-files column.
  - fields: `change`, `notes`, `selected`
- **struct** `ReviewNote` — One note the reviewer left on a line.
  - fields: `file`, `line`, `text`, `summary`, `pending`
- **struct** `Segmented` — `.seg`: a surface-2 track of 24 px segments; the active one wears a single surface-1 thumb at elevation 1 that slides between segments on the layout spring rather than cross-fading. [...]
  - `pub fn on_select(self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self` — Called with the index of the segment that was clicked.
- **struct** `SheetCell` — One cell.
  - fields: `text`, `numeric`, `bold`
  - `pub fn bold(self) -> Self` — Bold.
  - `pub fn num(text: impl Into<SharedString>) -> Self` — A number cell.
  - `pub fn text(text: impl Into<SharedString>) -> Self` — A text cell.
- **struct** `SheetPane` — The sheet pane. Build with `sheet_pane`.
  - `pub fn formula(self, cell: impl Into<SharedString>, formula: impl Into<SharedString>) -> Self` — The formula bar: cell reference and formula.
  - `pub fn selected(self, row: usize, column: usize) -> Self` — The selected cell `(row, column)` (0-based data indices).
  - `pub fn tabs(self, tabs: Vec<SharedString>, active: usize) -> Self` — Sheet tabs at the bottom.
  - `pub fn tabs_note(self, note: impl Into<SharedString>) -> Self` — The mono note at the right of the tabs (`weighted by Annex A`).
- **struct** `Source` — One retrieved source (`.s`).
  - fields: `index`, `title`, `meta`, `confidence`
  - `pub fn cited(index: u8, title: impl Into<SharedString>, meta: impl Into<SharedString>, confidence: f32) -> Self` — A cited source with its number and confidence.
  - `pub fn uncited(title: impl Into<SharedString>, meta: impl Into<SharedString>) -> Self` — A retrieved but uncited source: muted row, no number, no bar.
- **struct** `SourceHoverCard` — The passage popover (`.hover`). Build with `source_hover_card`.
  - `pub fn at_rest(self) -> Self` — Skips the enter: the card is drawn at rest on its first frame.
  - `pub fn on_insert(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — `Insert quote` was pressed.
  - `pub fn on_open(self, f: impl Fn(u32, &mut Window, &mut App) + 'static) -> Self` — `Open page N` was pressed; the argument is the page.
  - `pub fn present(self, present: bool) -> Self` — Whether the card is shown; `false` plays the exit.
- **struct** `SourceTier` — A retrieval tier (`.tier`): the caps label, an optional name after the separator, and the sources found in it.
  - fields: `label`, `name`, `sources`
  - `pub fn name(self, name: impl Into<SharedString>) -> Self` — The role or project name shown after the separator.
  - `pub fn new(label: impl Into<SharedString>) -> Self` — A tier with no sources yet.
  - `pub fn source(self, source: Source) -> Self` — Appends a source.
- **struct** `SourcesCard` — The sources list (`.src`). Build with `sources_card`.
  - `pub fn cited(self, count: usize) -> Self` — How many sources the answer cites (the header count).
  - `pub fn hover_card(self, row: usize, card: impl IntoElement) -> Self` — Anchors a `SourceHoverCard` to the row at `row` (counted over all tiers, in render order), as the HTML nests `.hover` inside a `.s`.
  - `pub fn on_open(self, f: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self` — A source row was opened; the argument is its citation number.
  - `pub fn on_show_all(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self` — `Show all` was pressed.
  - `pub fn retrieved(self, count: usize) -> Self` — How many sources retrieval returned (the header count).
- **struct** `TermBlock` — One command block.
  - fields: `id`, `command`, `state`, `output`, `folded`, `agent`, `duration`, `old`
  - `pub fn agent(self, provider: Provider) -> Self` — The agent tag.
  - `pub fn folded(self, folded: usize) -> Self` — Folded line count.
  - `pub fn new(id: impl Into<SharedString>, command: impl Into<SharedString>, state: BlockState, duration: impl Into<SharedString>) -> Self` — A finished block.
  - `pub fn old(self) -> Self` — Dims the block.
  - `pub fn output(self, lines: Vec<String>) -> Self` — Output lines.
- **struct** `TermPrompt` — The prompt row’s state.
  - fields: `text`, `context`
- **struct** `TuiPane` — The TUI pane. Build with `tui_pane`.
  - `pub fn footer(self, text: impl Into<SharedString>) -> Self` — The dim hint line under the input.
  - `pub fn hint_key(self, key: impl Into<SharedString>) -> Self` — Keycaps pinned top-right (`⌘⇧D`).
  - `pub fn input(self, text: impl Into<SharedString>) -> Self` — The docked input box with its text (the cursor follows it).

- **enum** `AnnotatorAction` — What the annotations panel reports.
  - variants: `Close`, `Clear`, `Send`, `Select`
- **enum** `ArtifactKind` — The kind of an artifact chip: its glyph and, for the two office kinds, the status hue the design tints the glyph with.
  - variants: `Doc`, `Sheet`, `Pdf`, `Note`, `Image`
  - `pub fn icon(self) -> IconName` — The glyph for this kind.
  - `pub fn tint(self, colors: &Palette) -> Option<Hsla>` — The glyph’s tint; `None` inherits the chip’s text colour.
- **enum** `BlockState` — How a block ended.
  - variants: `Done`, `Failed`, `Running`
- **enum** `BrowserAction` — A control in the nav row.
  - variants: `Back`, `Forward`, `Reload`, `FocusUrl`, `ToggleAnnotate`, `Screenshot`, `Console`
- **enum** `DiffReviewAction` — What the pane asks the app to do.
  - variants: `Scope`, `View`, `Search`, `CollapseAll`, `SelectFile`, `OpenInEditor`, `Stage`,
    `AddNote`, `EditNote`, `DeleteNote`, `SaveNote`, `CancelNote`, `Clear`, `Send`
- **enum** `DiffScope` — Which change set the pane reviews.
  - variants: `ThisTurn`, `Branch`, `Unstaged`
  - `pub fn label(self) -> &'static str` — The segment label.
- **enum** `DiffView` — How the diff is laid out.
  - variants: `Unified`, `Split`
  - `pub fn label(self) -> &'static str` — The segment label.
- **enum** `DocBlock` — One block of the page.
  - variants: `Paragraph`, `List`
  - `pub fn list(items: impl IntoIterator<Item = &'static str>) -> Self` — A numbered list of plain items.
  - `pub fn text(s: impl Into<SharedString>) -> Self` — A paragraph of plain text.
- **enum** `DocRun` — One inline run of the page’s body text.
  - variants: `Text`, `Bold`, `Changed`
  - `pub fn bold(s: impl Into<SharedString>) -> Self` — Bold text.
  - `pub fn changed(s: impl Into<SharedString>) -> Self` — A span the chat changed.
  - `pub fn text(s: impl Into<SharedString>) -> Self` — Plain text.
- **enum** `DocToolbarAction` — What a toolbar control asks the application to do.
  - variants: `Style`, `Font`, `Mark`, `InsertList`, `InsertLink`, `InsertImage`,
    `AskAboutSelection`, `Export`
- **enum** `FileTreeAction` — What a tree row or header control asks the application to do.
  - variants: `Select`, `Toggle`, `Search`, `Refresh`
- **enum** `FormatMark` — The character marks of the toolbar (`<b>`, `<i>`, `<u>`).
  - variants: `Bold`, `Italic`, `Underline`
  - `pub fn letter(self) -> &'static str` — The letter drawn in the button.
- **enum** `GitAction` — What the Changes panel asks the host to do.
  - variants: `Toggle`, `Regenerate`, `Amend`, `Commit`, `Push`
- **enum** `GitBadge` — The git status of a tree row, shown as a mono letter at the right of the row (`.n .b`).
  - variants: `M`, `A`, `D`, `U`
  - `pub fn color(self, colors: &Palette) -> Hsla` — The badge colour: `.b.m` warning, `.b.a` success, deleted danger, `.b.u` ink-4.
  - `pub fn letter(self) -> &'static str` — The letter drawn in the badge.
- **enum** `NoteAction` — What the note popover reports.
  - variants: `Cancel`, `Save`
- **enum** `PdfRun` — One run of page text.
  - variants: `Text`, `Bold`, `Highlight`
- **enum** `PrAction` — What the pull-request form asks the host to do.
  - variants: `PickBase`, `Cancel`, `Create`, `Linear`
- **enum** `PrDescription` — One run of the PR description: prose in ink-2 or an identifier in the mono face at the field’s ink. Runs flow together and wrap; `\n\n` inside a text run starts a new paragraph.
  - variants: `Text`, `Mono`
- **enum** `TerminalAction` — What a block asks for.
  - variants: `Copy`, `Ask`, `Unfold`

- **const** `ANSWER_MARGIN` — `.a{margin-bottom:14px}` — the gap between the answer and the sources card.
  - `pub const ANSWER_MARGIN: f32 = 14.0;`

## `aui-motion`

### `aui-motion`

The motion layer of the Agentic UI library (design card 04, spec §0.4). [...]

- **fn** `child_id` — Derives a child id from a parent id and an index or generation, so one element can own several independently keyed motions.
  - `pub fn child_id(parent: impl Into<ElementId>, index: usize) -> ElementId`
- **fn** `collapse` — Wraps `child` in a measured, clipped reveal whose height follows the layout spring from 0 (closed) to the child’s natural height (open). [...]
  - `pub fn collapse(id: impl Into<ElementId>, open: bool, child: AnyElement, window: &mut Window, cx: &mut App) -> (Reveal, f32)`
- **fn** `icon_morph` — Samples the morph phase for `id`: `show_second` picks the resting glyph.
  - `pub fn icon_morph(id: impl Into<ElementId>, show_second: bool, window: &mut Window, cx: &mut App) -> MorphSample`
- **fn** `looping` — The current phase in `0..=1` of a loop keyed by `id`.
  - `pub fn looping(id: impl Into<TransitionId>, config: Loop, window: &mut Window, cx: &mut App) -> f32`
- **fn** `presence` — Samples the presence of an element. `present` is the desired state; the sample says whether to render at all and how far along the enter/exit is (`progress` runs 0→1 on enter and 1→0 on exit).
  - `pub fn presence(id: impl Into<TransitionId>, present: bool, timing: EnterExit, window: &mut Window, cx: &mut App) -> PresenceSample`
- **fn** `spring` — A spring-driven `f32`. The first call adopts `target`; later calls travel toward it with the spring’s velocity preserved. [...]
  - `pub fn spring(id: impl Into<TransitionId>, target: f32, kind: SpringKind, window: &mut Window, cx: &mut App) -> f32`
- **fn** `spring_phase` — A spring-driven phase for a boolean state: `0.0` when `on` is false, `1.0` when true, travelling between them. This is the building block for chevron rotation, checkbox pops and the send/stop morph.
  - `pub fn spring_phase(id: impl Into<TransitionId>, on: bool, kind: SpringKind, window: &mut Window, cx: &mut App) -> f32`
- **fn** `spring_px` — A spring-driven length in pixels.
  - `pub fn spring_px(id: impl Into<TransitionId>, target: Pixels, kind: SpringKind, window: &mut Window, cx: &mut App) -> Pixels`
- **fn** `tint_fade` — Fades a tint in and out over whatever ground is behind it: the highlight a row wears while it is hovered or selected.
  - `pub fn tint_fade(id: impl Into<TransitionId>, on: bool, tint: Hsla, policy: Tween, window: &mut Window, cx: &mut App) -> Hsla`
- **fn** `tween` — Transitions a value toward `target`. The first call adopts the target; later target changes start from the value sampled at that moment, and a reversal mid-flight takes proportionally less time. [...]
  - `pub fn tween<T>(id: impl Into<TransitionId>, target: T, policy: Tween, window: &mut Window, cx: &mut App) -> Twhere T: Interpolate + PartialEq + 'static,`

- **struct** `Easing` — One of the three named easings (`out`, `inout`, `std`) as a cubic bezier.
  - fields: `0`
  - `pub fn function(self) -> impl Fn(f32) + 'static` — The easing as a boxed function usable with gpui’s `Animation::with_easing`.
  - `pub fn sample(self, t: f32) -> f32` — Evaluates the bezier at `t` in `0..=1` (x is time, y is progress).
- **struct** `EnterExit` — Enter and exit timings.
  - fields: `enter`, `exit`, `delay`
  - `pub const fn with_delay(self, delay: Duration) -> Self` — Adds an enter delay.
- **struct** `IconMorph` — A ready-made morph element: both glyphs stacked in a `size` × `size` box, styled from a `MorphSample`. [...]
  - `pub fn new(sample: MorphSample, size: Pixels, first: impl IntoElement, second: impl IntoElement) -> Self` — Builds the element from a sample and the two glyph elements. Each glyph should size itself to fill its box (`size_full`) so the scale applies.
- **struct** `Loop` — Loop parameters.
  - fields: `period`, `easing`, `alternate`, `resting`
  - `pub const fn alternate(self) -> Self` — Ping-pong.
  - `pub const fn eased(period: Duration, easing: Easing) -> Self` — An eased loop.
  - `pub const fn linear(period: Duration) -> Self` — A linear loop.
  - `pub const fn resting(self, resting: f32) -> Self` — The value to hold under reduced motion.
- **struct** `MorphSample` — How far the morph has progressed and the per-glyph styles.
  - fields: `progress`, `a_opacity`, `a_offset`, `a_scale`, `b_opacity`, `b_offset`, `b_scale`
  - `pub fn from_progress(progress: f32) -> Self` — Derives the glyph styles from a 0..1 phase (values outside the range come from spring overshoot and are clamped per property).
- **struct** `PresenceSample`
  - fields: `phase`, `progress`, `status`
  - `pub const fn should_render(self) -> bool`
- **struct** `PresenceStyle` — The standard visual for an entering/leaving element, derived from a `PresenceSample`: opacity, a vertical rise and a size scale.
  - fields: `opacity`, `offset_y`, `scale`
  - `pub fn fade(sample: PresenceSample) -> Self` — Plain fade.
  - `pub fn fade_rise(sample: PresenceSample, rise: f32) -> Self` — Fade + rise by `rise` px (cards: 6, toolbars: 4, chunks: 3).
  - `pub fn fade_rise_scale(sample: PresenceSample, rise: f32, from_scale: f32) -> Self` — Fade + rise + a slight scale (command palette: 6 px, .985).
  - `pub fn settled(sample: PresenceSample) -> bool` — Whether the element is fully at rest and visible.
- **struct** `Reveal` — A measured, clipped vertical reveal driven by normalized progress.
  - `pub fn new(id: impl Into<ElementId>, progress: f32, child: AnyElement) -> Self` — A reveal of `child` at normalized `progress` (clamped to 0..=1).
- **struct** `Tween` — A named timing policy: duration + easing (+ optional delay).
  - fields: `duration`, `easing`, `delay`
  - `pub const fn new(duration: Duration, easing: Easing) -> Self` — A custom policy.
  - `pub fn policy(self) -> Transition` — The equivalent gpui-base policy.
  - `pub const fn with_delay(self, delay: Duration) -> Self` — Adds a start delay (used by staggers).
  - `pub const fn with_easing(self, easing: Easing) -> Self` — Swaps the easing (a base-length slide that eases out).

- **enum** `SpringKind` — Which of the design’s four springs to use (`motion.json`).
  - variants: `Press`, `Swap`, `Layout`, `Gentle`
  - `pub fn config(self) -> SpringConfig` — The physical parameters from the design tokens.
  - `pub fn describe(self) -> String` — `stiffness / damping / mass` as printed on the motion card.
  - `pub fn label(self) -> &'static str` — Human-readable label, as printed on the motion card.
  - `pub fn policy(self) -> Spring` — The equivalent gpui-base policy for a value in `0..=1`.
  - `pub fn policy_px(self) -> Spring` — The policy for a value in pixels, with a coarser settling tolerance so sub-pixel motion stops requesting frames.
  - `pub fn settle_time(self) -> Duration` — How long the spring takes to settle from rest to a unit step, for display and for choosing exit holds.

- **const** `PHASE` — Delivery phase of this crate, from `docs/01-research-and-plan.md`.
  - `pub const PHASE: &str = "phase 2 · motion engine";`

### `aui-motion::check`

Check draw: a success check mark that draws itself in 320 ms (ease-out). [...]

- **fn** `check_draw` — Progress 0..=1 of the draw for `generation` under `id` (0 = not started).
  - `pub fn check_draw(id: impl Into<ElementId>, generation: u32, window: &mut Window, cx: &mut App) -> f32`

- **struct** `CheckDraw` — Clips `glyph` (a `size` × `size` element) to `progress` of its width.
  - `pub fn new(size: Pixels, progress: f32, glyph: impl IntoElement) -> Self` — Wraps a glyph.

- **const** `DURATION` — Draw duration.
  - `pub const DURATION: Duration;`

### `aui-motion::durations`

Motion durations from `motion.json`, mirrored for convenience.

- **const** `BASE`
  - `pub const BASE: Duration;`
- **const** `ENTER`
  - `pub const ENTER: Duration;`
- **const** `EXIT`
  - `pub const EXIT: Duration;`
- **const** `FAST`
  - `pub const FAST: Duration;`
- **const** `QUICK_ENTER`
  - `pub const QUICK_ENTER: Duration;`
- **const** `QUICK_EXIT`
  - `pub const QUICK_EXIT: Duration;`
- **const** `SLOW`
  - `pub const SLOW: Duration;`

### `aui-motion::pulse`

Pulse: the ring that expands out of a running / waiting status dot every 2 s (`.dot.pulse::after` in base.css: inset −4 px, 1.5 px border, scale .6 → 1.5, opacity .7 → 0, ease-out).

- **fn** `pulse_ring` — Builds a pulsing dot. With `active = false` it is a plain dot.
  - `pub fn pulse_ring(id: impl Into<ElementId>, dot: Pixels, color: Hsla, active: bool) -> PulseRing`

- **struct** `PulseRing` — A status dot with its pulsing ring. `dot` is the dot diameter (7 px in rows). The ring is drawn as a sibling so the dot itself never moves.

- **const** `PERIOD` — One pulse cycle.
  - `pub const PERIOD: Duration;`

### `aui-motion::reveal`

Stream reveal: each newly arrived chunk of assistant text (or each new timeline row) fades in and rises 3 px over the base duration.

- **fn** `stream_reveal` — Samples the reveal for chunk `index` under `id`. A chunk that was already present when its element mounted (e.g. history) can pass `settled = true` to skip the animation.
  - `pub fn stream_reveal(id: impl Into<ElementId>, index: usize, settled: bool, window: &mut Window, cx: &mut App) -> RevealSample`

- **struct** `RevealSample` — The style for one revealed chunk.
  - fields: `opacity`, `offset_y`

### `aui-motion::shake`

Shake: the ±4 px horizontal error shake (500 ms, std easing) that a denied action or a failed send plays once.

- **fn** `shake_offset` — The horizontal offset to apply (as a relative `left`) for shake number `generation` under `id`. [...]
  - `pub fn shake_offset(id: impl Into<ElementId>, generation: u32, window: &mut Window, cx: &mut App) -> Pixels`

- **const** `DURATION` — One shake.
  - `pub const DURATION: Duration;`

### `aui-motion::shimmer`

Shimmer: the “Thinking…” / “Working…” text sweep (1.8 s linear) and the surface skeleton (1.4 s). [...]

- **fn** `shimmer_text` — A shimmering label: ink-3 base with an ink highlight sweeping left to right every 1.8 s (`.shimmer` in base.css). The caller applies size and weight.
  - `pub fn shimmer_text(id: impl Into<ElementId>, text: impl Into<SharedString>, cx: &App) -> ShimmerText`
- **fn** `skeleton` — A skeleton block: surface-3 with a line-strong band sweeping across every 1.4 s (`.sk` in base.css). Returns a `div` sized by the caller.
  - `pub fn skeleton(id: impl Into<ElementId>, width: Pixels, height: Pixels, window: &mut Window, cx: &mut App) -> Div`

- **const** `SKELETON_PERIOD` — One sweep of the surface skeleton.
  - `pub const SKELETON_PERIOD: Duration;`
- **const** `TEXT_PERIOD` — One sweep of the text shimmer.
  - `pub const TEXT_PERIOD: Duration;`

### `aui-motion::springs`

The four springs from `design/tokens/motion.json` (stiffness / damping / mass).

- **const** `GENTLE` — `gentle` spring — popover morph, drop-zone overlay.
  - `pub const GENTLE: SpringConfig;`
- **const** `LAYOUT` — `layout` spring — collapse/expand, panel width, card resize.
  - `pub const LAYOUT: SpringConfig;`
- **const** `PRESS` — `press` spring — buttons, chips, rows on press.
  - `pub const PRESS: SpringConfig;`
- **const** `SWAP` — `swap` spring — icon morph, chevrons, tab indicator, checkbox.
  - `pub const SWAP: SpringConfig;`

### `aui-motion::stagger`

Per-child delays for lists that enter together: approval buttons (40 ms), suggestion chips (60 ms), timeline rows.

- **fn** `stagger_delay` — The delay for child `index` of `count`, `interval` apart, counting from the first child.
  - `pub fn stagger_delay(index: usize, count: usize, interval: Duration) -> Duration`

- **const** `DEFAULT_INTERVAL` — Default interval between children.
  - `pub const DEFAULT_INTERVAL: Duration;`

### `aui-motion::ticker`

Number ticker: counts, percentages and costs glide to their new value over the base duration instead of jumping.

- **fn** `number_ticker` — The displayed value for `target`, easing from the previous value.
  - `pub fn number_ticker(id: impl Into<TransitionId>, target: f32, window: &mut Window, cx: &mut App) -> f32`

## `aui-tokens`

### `aui-tokens`

Design tokens for the Agentic UI (`aui`) library. Everything visual that a component needs comes from here: the two colour palettes, the type scale, spacing, radii, control heights, elevation, motion [...]

- **fn** `dark` — The dark palette.
  - `pub fn dark() -> Palette`
- **fn** `light` — The light palette.
  - `pub fn light() -> Palette`
- **fn** `load_fonts` — Registers the embedded faces with gpui’s text system so that the families `Geist` and `Geist Mono` resolve without a system install.
  - `pub fn load_fonts(cx: &App) -> Result<()>`
- **fn** `scaled` — Converts a design pixel size into rems against the 13 px base, so the window’s rem size (base × `crate::AuiTheme::text_scale`) scales it.
  - `pub fn scaled(size_px: f32) -> Rems`
- **fn** `theme_set_json` — The generated gpui-kit `ThemeSet` as pretty JSON.
  - `pub fn theme_set_json() -> String`
- **fn** `theme_set_value` — The generated gpui-kit `ThemeSet` as a JSON value.
  - `pub fn theme_set_value() -> Value`

- **struct** `AuiTheme` — The active token set. Lives as a gpui [`Global`]; read it with `ActiveAui::aui`.
  - fields: `kind`, `density`, `colors`, `metrics`, `text_scale`
  - `pub fn init(kind: ThemeKind, cx: &mut App)` — Installs the tokens: loads the bundled fonts, registers the generated gpui-kit theme set, makes it gpui-kit’s light/dark pair and activates `kind`. Call once after `gpui_kit::init(cx)`.
  - `pub fn new(kind: ThemeKind, density: Density) -> Self` — Builds a theme without touching gpui globals (for tests and tooling).
  - `pub fn reduce_motion(cx: &App) -> bool` — Whether the person asked the OS for reduced motion.
  - `pub fn set_density(density: Density, window: Option<&mut Window>, cx: &mut App)` — Switches the row density.
  - `pub fn set_kind(kind: ThemeKind, window: Option<&mut Window>, cx: &mut App)` — Switches the theme (both the token palette and gpui-kit’s theme).
  - `pub fn set_text_scale(text_scale: f32, window: Option<&mut Window>, cx: &mut App)` — Sets the text scale (0.9 … 1.3) and pushes the resulting base size to gpui-kit, whose `Root` sets the window rem size from it every frame.
  - `pub fn toggle_kind(window: Option<&mut Window>, cx: &mut App)` — Flips between light and dark.
- **struct** `Easing` — One of the three named easings (`out`, `inout`, `std`) as a cubic bezier.
  - fields: `0`
  - `pub fn function(self) -> impl Fn(f32) -> f32 + 'static` — The easing as a boxed function usable with gpui’s `Animation::with_easing`.
  - `pub fn sample(self, t: f32) -> f32` — Evaluates the bezier at `t` in `0..=1` (x is time, y is progress).
- **struct** `Metrics` — Heights that depend on density. Everything else (spacing, radii, type) comes straight from `scale`.
  - fields: `header`, `panel_header`, `tab_strip`, `card_header`, `row`, `row_sm`, `control_xs`,
    `control_sm`, `control_md`, `control_lg`, `control_xl`
  - `pub fn for_density(density: Density) -> Self` — Metrics for a density.
- **struct** `Palette` — The full colour palette of one theme. Field names mirror the CSS custom properties in `design/tokens/tokens.css` (`--surface-1` → `surface_1`).
  - fields: `bg`, `surface_1`, `surface_2`, `surface_3`, `overlay`, `line`, `line_strong`,
    `ink`, `ink_2`, `ink_3`, `ink_4`, `accent`, `accent_strong`, `accent_ink`, `accent_soft`,
    `accent_ring`, `success`, `success_soft`, `warning`, `warning_soft`, `danger`,
    `danger_soft`, `info`, `info_soft`, `label_1`, `label_2`, `label_3`, `label_4`, `label_5`,
    `label_6`, `label_7`, `label_8`, `diff_add`, `diff_add_strong`, `diff_del`,
    `diff_del_strong`, `selection`, `term_bg`, `term_fg`, `term_dim`, `term_cursor`,
    `ansi_black`, `ansi_red`, `ansi_green`, `ansi_yellow`, `ansi_blue`, `ansi_magenta`,
    `ansi_cyan`, `ansi_white`, `ansi_bblack`, `ansi_bred`, `ansi_bgreen`, `ansi_byellow`,
    `ansi_bblue`, `ansi_bmagenta`, `ansi_bcyan`, `ansi_bwhite`, `shadow_1`, `shadow_2`,
    `shadow_3`
  - `pub fn agent_state(&self, state: AgentState) -> Hsla` — The colour that carries an agent state (running → accent, waiting → warning, done → success, failed → danger, idle → ink-4).
  - `pub fn ansi16(&self) -> [Hsla; 16]` — The 16 ANSI colours in terminal order (black, red, green, yellow, blue, magenta, cyan, white, then the bright variants).
  - `pub fn attention_border(&self, color: Hsla) -> Hsla` — The pending-dialog attention border: the status colour at 70 % alpha, as the design rules require (no halo, no glow).
  - `pub fn color(&self, name: &str) -> Option<Hsla>` — Looks a colour up by its token name (`"surface-1"`).
  - `pub fn for_kind(kind: ThemeKind) -> Palette` — The palette for a theme kind.
  - `pub fn label(&self, index: u8) -> Hsla` — One of the eight project label colours, red through pink in token order (`label-1` … `label-8`). [...]
  - `pub fn shadow(&self, level: u8) -> Vec<BoxShadow>` — Converts one of the three elevation levels into gpui box shadows.
- **struct** `ShadowLayer` — One layer of a box shadow, in logical pixels.
  - fields: `x`, `y`, `blur`, `spread`, `color`

- **enum** `AgentState` — The five agent states the sidebar and status rows colour.
  - variants: `Running`, `Waiting`, `Done`, `Failed`, `Idle`
- **enum** `Density` — How tightly rows are packed. Standard is the design; compact trims the row and header heights the way a “compact” preference would, without changing type sizes.
  - variants: `Standard`, `Compact`
  - `pub fn label(self) -> &'static str` — Human-readable name.
  - `pub fn toggled(self) -> Self` — The other density.
- **enum** `TextRole` — The named text roles of the design. Each role fixes family, size, line height and weight; colour is applied separately from the palette.
  - variants: `Ui`, `UiMedium`, `UiSmall`, `Body`, `Caps`, `Meta`, `Mono`, `MonoSmall`, `Tag`,
    `Title`
  - `pub fn spec(self) -> (&'static str, f32, f32, FontWeight)` — `(family, size px, line-height ratio, weight)`.
- **enum** `ThemeKind` — Which of the two first-class themes is active. The harness defaults to dark, the assistant to light.
  - variants: `Light`, `Dark`
  - `pub fn is_dark(self) -> bool` — Whether this is the dark theme.
  - `pub fn label(self) -> &'static str` — Human-readable name (“Light” / “Dark”).
  - `pub fn parse(s: &str) -> Option<Self>` — Parses `light` / `dark` (case-insensitive).
  - `pub fn toggled(self) -> Self` — The opposite theme.

- **trait** `ActiveAui` — Access to the active `AuiTheme` from any context that derefs to [`App`].
  - `fn aui(&self) -> &AuiTheme` — The active token set.
- **trait** `AuiStyled` — Fluent helpers for applying the type scale. Sizes are expressed in rems against the 13 px base so the text-scale preference applies to all of them.
  - `fn medium(self) -> Self` — Weight 500.
  - `fn mono(self, size: f32) -> Self` — Mono face at a size from the scale with the mono line height (1.6).
  - `fn semibold(self) -> Self` — Weight 600.
  - `fn text_px(self, size: f32) -> Self` — A design pixel text size that follows the text-scale preference.
  - `fn text_role(self, role: TextRole) -> Self` — Applies a `TextRole` (family, size, line height, weight).
  - `fn ui(self, size: f32) -> Self` — UI face at a size from the scale with the UI line height (1.5).

- **const** `FONT_FILES` — File names of the embedded faces (Regular, Medium, SemiBold, Bold and the two italics for Geist; Regular, Medium, SemiBold, Bold for Geist Mono).
  - `pub const FONT_FILES: &[&str];`
- **const** `KIT_THEME_DARK` — Name of the generated dark theme inside gpui-kit’s registry.
  - `pub const KIT_THEME_DARK: &str = "Agentic UI Dark";`
- **const** `KIT_THEME_LIGHT` — Name of the generated light theme inside gpui-kit’s registry.
  - `pub const KIT_THEME_LIGHT: &str = "Agentic UI Light";`
- **const** `MOTION_JSON` — Raw JSON of `design/tokens/motion.json`.
  - `pub const MOTION_JSON: &str = ...;`
- **const** `TOKENS_JSON` — Raw JSON of `design/tokens/tokens.json`, embedded for tooling that wants the original values (the gallery’s colour card lists them).
  - `pub const TOKENS_JSON: &str = ...;`

### `aui-tokens::durations`

Motion durations from `motion.json`, mirrored for convenience.

- **const** `BASE`
  - `pub const BASE: Duration;`
- **const** `ENTER`
  - `pub const ENTER: Duration;`
- **const** `EXIT`
  - `pub const EXIT: Duration;`
- **const** `FAST`
  - `pub const FAST: Duration;`
- **const** `QUICK_ENTER`
  - `pub const QUICK_ENTER: Duration;`
- **const** `QUICK_EXIT`
  - `pub const QUICK_EXIT: Duration;`
- **const** `SLOW`
  - `pub const SLOW: Duration;`

### `aui-tokens::scale`

Shared (theme-independent) metrics: type scale, line heights, spacing, radii, control heights, durations, easings and font families.

- **const** `D_BASE`
  - `pub const D_BASE: Duration;`
- **const** `D_ENTER`
  - `pub const D_ENTER: Duration;`
- **const** `D_EXIT`
  - `pub const D_EXIT: Duration;`
- **const** `D_FAST`
  - `pub const D_FAST: Duration;`
- **const** `D_SLOW`
  - `pub const D_SLOW: Duration;`
- **const** `E_INOUT`
  - `pub const E_INOUT: [f32; 4];`
- **const** `E_OUT`
  - `pub const E_OUT: [f32; 4];`
- **const** `E_STD`
  - `pub const E_STD: [f32; 4];`
- **const** `FONT_MONO`
  - `pub const FONT_MONO: &str = "Geist Mono";`
- **const** `FONT_UI`
  - `pub const FONT_UI: &str = "Geist";`
- **const** `FS_11`
  - `pub const FS_11: f32 = 11.0;`
- **const** `FS_12`
  - `pub const FS_12: f32 = 12.0;`
- **const** `FS_13`
  - `pub const FS_13: f32 = 13.0;`
- **const** `FS_14`
  - `pub const FS_14: f32 = 14.0;`
- **const** `FS_16`
  - `pub const FS_16: f32 = 16.0;`
- **const** `FS_18`
  - `pub const FS_18: f32 = 18.0;`
- **const** `FS_20`
  - `pub const FS_20: f32 = 20.0;`
- **const** `FS_24`
  - `pub const FS_24: f32 = 24.0;`
- **const** `H_LG`
  - `pub const H_LG: f32 = 32.0;`
- **const** `H_MD`
  - `pub const H_MD: f32 = 28.0;`
- **const** `H_SM`
  - `pub const H_SM: f32 = 24.0;`
- **const** `H_XL`
  - `pub const H_XL: f32 = 40.0;`
- **const** `H_XS`
  - `pub const H_XS: f32 = 20.0;`
- **const** `LH_BODY`
  - `pub const LH_BODY: f32 = 1.65;`
- **const** `LH_MONO`
  - `pub const LH_MONO: f32 = 1.6;`
- **const** `LH_TIGHT`
  - `pub const LH_TIGHT: f32 = 1.3;`
- **const** `LH_UI`
  - `pub const LH_UI: f32 = 1.5;`
- **const** `R_FULL`
  - `pub const R_FULL: f32 = 999.0;`
- **const** `R_LG`
  - `pub const R_LG: f32 = 12.0;`
- **const** `R_MD`
  - `pub const R_MD: f32 = 8.0;`
- **const** `R_SM`
  - `pub const R_SM: f32 = 6.0;`
- **const** `R_XL`
  - `pub const R_XL: f32 = 16.0;`
- **const** `R_XS`
  - `pub const R_XS: f32 = 4.0;`
- **const** `SP_1`
  - `pub const SP_1: f32 = 2.0;`
- **const** `SP_2`
  - `pub const SP_2: f32 = 4.0;`
- **const** `SP_3`
  - `pub const SP_3: f32 = 8.0;`
- **const** `SP_4`
  - `pub const SP_4: f32 = 12.0;`
- **const** `SP_5`
  - `pub const SP_5: f32 = 16.0;`
- **const** `SP_6`
  - `pub const SP_6: f32 = 24.0;`
- **const** `SP_7`
  - `pub const SP_7: f32 = 32.0;`
- **const** `TEXT_SCALE`
  - `pub const TEXT_SCALE: f32 = 1.1;`
- **const** `TRACK_CAPS_EM`
  - `pub const TRACK_CAPS_EM: f32 = 0.08;`

### `aui-tokens::springs`

The four springs from `design/tokens/motion.json` (stiffness / damping / mass).

- **const** `GENTLE` — `gentle` spring — popover morph, drop-zone overlay.
  - `pub const GENTLE: SpringConfig;`
- **const** `LAYOUT` — `layout` spring — collapse/expand, panel width, card resize.
  - `pub const LAYOUT: SpringConfig;`
- **const** `PRESS` — `press` spring — buttons, chips, rows on press.
  - `pub const PRESS: SpringConfig;`
- **const** `SWAP` — `swap` spring — icon morph, chevrons, tab indicator, checkbox.
  - `pub const SWAP: SpringConfig;`

## `aui-icons`

### `aui-icons`

Icon glyphs, provider marks and file-type icon mapping for the Agentic UI library.

- **fn** `icon` — Create an `Icon` element for the given glyph.
  - `pub fn icon(name: IconName) -> Icon`
- **fn** `lookup` — Resolve an `icons/aui/<id>.svg` asset path to its SVG bytes.
  - `pub fn lookup(path: &str) -> Option<&'static [u8]>`
- **fn** `provider_mark` — Create a `ProviderMark` at the default 16 px size.
  - `pub fn provider_mark(provider: Provider) -> ProviderMark`

- **struct** `Assets` — A [`gpui::AssetSource`] that serves the bundled Agentic UI icons.
- **struct** `Icon` — A single stroked glyph from the sprite sheet.
  - `pub fn color(self, color: impl Into<Hsla>) -> Self` — Tint the glyph. Without this the glyph inherits the parent’s text colour.
  - `pub fn lg(self) -> Self` — Use the 16 px header size.
  - `pub fn name(&self) -> IconName` — The glyph this icon renders.
  - `pub fn rotate(self, radians: impl Into<Radians>) -> Self` — Rotate the glyph about its centre, in radians.
  - `pub fn size(self, size: impl Into<Pixels>) -> Self` — Override the square size of the glyph.
- **struct** `ProviderMark` — The rounded-square provider mark (`.mark`).
  - `pub fn provider(&self) -> Provider` — The provider this mark stands for.
  - `pub fn size(self, size: impl Into<Pixels>) -> Self` — Override the square size. The spec uses 16 (default), 13, 12 and 11 px.

- **enum** `FileType` — The file-type icon family (`fic`) used by trees, mentions and diff headers.
  - variants: `Folder`, `FolderOpen`, `Ts`, `Tsx`, `Json`, `Md`, `Test`, `Css`, `Lock`, `Image`,
    `File`
  - `pub fn from_path(path: &str) -> FileType` — Classify a path by its file name.
  - `pub fn hue(self) -> Option<Rgba>` — The muted hue this file type is tinted with.
  - `pub fn icon(self) -> IconName` — The `ft-*` glyph for this file type.
- **enum** `IconName` — Every glyph in the Agentic UI sprite sheet.
  - variants: `Chevron`, `ChevronDown`, `Terminal`, `File`, `Folder`, `Search`, `Globe`,
    `Check`, `X`, `Plus`, `ArrowUp`, `Stop`, `Sparkle`, `Edit`, `Eye`, `EyeOff`, `Git`, `Copy`,
    `Pin`, `PinOff`, `Bell`, `Layout`, `Split`, `Refresh`, `ArrowLeft`, `ArrowRight`, `Cursor`,
    `Camera`, `Paperclip`, `Slash`, `At`, `Brain`, `List`, `Shield`, `Question`, `Doc`, `Sheet`,
    `Pdf`, `Image`, `Play`, `Dots`, `Inbox`, `Zap`, `Book`, `Link`, `Sidebar`, `Clock`,
    `PanelRight`, `GradCap`, `Scale`, `Gear`, `FtFolder`, `FtFolderOpen`, `FtTs`, `FtTsx`,
    `FtJson`, `FtMd`, `FtTest`, `FtCss`, `FtLock`, `FtImage`, `FtFile`, `Sliders`,
    `SpinnerRing`, `SpinnerArc`, `CheckBold`, `XBold`, `Chev`, `Archive`
  - `pub fn bytes(self) -> &'static [u8] ⓘ` — The standalone SVG bytes for this glyph.
  - `pub fn id(self) -> &'static str` — The `id` of this glyph’s `<symbol>` in the sprite sheet.
  - `pub fn path(self) -> &'static str` — The asset path this glyph is served at by `crate::Assets`.
- **enum** `Provider` — The agent providers the harness shows a mark for.
  - variants: `Claude`, `Codex`, `Grok`, `Gemini`, `Pi`, `Cursor`, `Muse`
  - `pub fn color(self) -> Rgba` — The mark’s background colour, from the `.mark.*` rules in `design/src/base.css`.
  - `pub fn letter(self) -> &'static str` — The glyph shown inside the mark, as used by `design/src/cards/foundations/05-icons.html`.
- **enum** `RoleIcon` — The role icons used by the assistant sidebar’s role headers.
  - variants: `GradCap`, `Scale`, `Gear`
  - `pub fn icon(self) -> IconName` — The glyph for this role.

- **const** `ICON_SIZE` — The default glyph size: 14 px, per spec section 0.5.
  - `pub const ICON_SIZE: Pixels;`
- **const** `ICON_SIZE_LG` — The larger glyph size used in headers: 16 px.
  - `pub const ICON_SIZE_LG: Pixels;`
- **const** `MARK_SIZE` — The default provider mark size: 16 px.
  - `pub const MARK_SIZE: Pixels;`

## `aui-protocol`

### `aui-protocol`

`aui-protocol` — the transport-agnostic session model that the Agentic UI transcript renders.

- **struct** `Answer` — The person’s reply to a `Block::Question`.
  - fields: `selected`, `other`
- **struct** `ApprovalBadges` — The badges an approval card shows beside its title.
  - fields: `protected_write`, `judge_escalated`
- **struct** `ApprovalChoice` — One server-minted choice on an approval card (MSP `ApprovalChoice`).
  - fields: `id`, `label`, `decision`, `scope`, `rule_preview`, `accepts_feedback`
- **struct** `ApprovalStage` — One stage of a staged approval subject (MSP `ApprovalStage`).
  - fields: `position`, `total`, `argv`, `argv_complete`, `resolved`, `suggested_rule`
- **struct** `Attachment` — A file or image sent with a user turn, shown as a chip above the bubble.
  - fields: `name`, `kind`, `size_bytes`, `meta`, `state`
- **struct** `Check` — One verification in a `Block::Summary`, e.g. `"27 tests pass"`.
  - fields: `label`, `passed`
- **struct** `Diff` — A unified diff for one file.
  - fields: `path`, `hunks`, `added`, `removed`
- **struct** `DiffLine` — One row of a `Hunk`.
  - fields: `kind`, `old_no`, `new_no`, `text`
- **struct** `FileChange` — One file row in a `Block::Summary`.
  - fields: `path`, `change`, `added`, `removed`
- **struct** `Hunk` — One `@@` hunk of a `Diff`.
  - fields: `header`, `lines`
- **struct** `Mention` — An `@` mention chip inside a user turn.
  - fields: `kind`, `label`
- **struct** `Note` — One review note anchored to a file and line.
  - fields: `path`, `line`, `text`
- **struct** `PlanSection` — One unnumbered section label inside a `Block::Plan`.
  - fields: `label`, `first_item`
- **struct** `QuestionOption` — One choice in a `Block::Question`.
  - fields: `label`, `description`, `key`, `preview`
- **struct** `QuestionPreview` — A rendered preview attached to a `QuestionOption` (MSP `UserInputOption.preview`).
  - fields: `content`, `format`
- **struct** `SearchHit` — One code-search hit.
  - fields: `path`, `line`, `snippet`
- **struct** `Session` — One agent conversation, with everything the shell chrome needs to describe it (provider mark, worktree, branch, permission mode) plus the transcript.
  - fields: `id`, `agent`, `model`, `mode`, `plan`, `cwd`, `branch`, `environment`, `turns`
  - `pub fn apply(&mut self, delta: Delta) -> bool` — Fold a streaming update into the transcript.
  - `pub fn last_turn_id(&self) -> Option<&str>` — The id of the last turn, if any.
  - `pub fn new(id: impl Into<String>, agent: Provider, model: impl Into<String>, cwd: impl Into<String>) -> Self` — A session with no turns yet, running locally in `cwd`.
  - `pub fn turn(&self, turn_id: &str) -> Option<&Turn>` — The turn with this id, if it is still in the transcript.
  - `pub fn turn_mut(&mut self, turn_id: &str) -> Option<&mut Turn>` — Mutable access to the turn with this id.
- **struct** `Step` — One row in an activity group’s timeline.
  - fields: `verb`, `target`, `state`, `result`
- **struct** `TodoItem` — One row in a `Block::Todo` list.
  - fields: `label`, `state`, `elapsed_ms`
- **struct** `ToolCall` — One tool invocation: what ran, how it went, and the payload the card renders.
  - fields: `id`, `kind`, `verb`, `target`, `status`, `duration_ms`, `body`
- **struct** `TurnMeta` — The mono footer under an assistant turn: `model · duration · tokens · cost`.
  - fields: `model`, `duration_ms`, `tokens_in`, `tokens_out`, `reasoning_tokens`, `cost_usd`
- **struct** `WebResult` — One web result row.
  - fields: `title`, `url`, `domain`

- **enum** `ActivityState` — Whether an activity group is still running.
  - variants: `Working`, `Done`, `Failed`
- **enum** `ApprovalDecision` — What the person chose on an approval card.
  - variants: `Once`, `Always`, `Deny`, `ApprovedForSession`, `PolicyAmendment`,
    `DeniedPolicyAmendment`, `TimedOut`, `Abort`
- **enum** `ApprovalScope` — How far an “always allow” decision reaches.
  - variants: `ThisCommand`, `ThisWorktree`, `ThisSession`, `Global`
- **enum** `ApprovalState` — Where an approval request is in its lifecycle.
  - variants: `Pending`, `Approving`, `AllowedOnce`, `Denied`, `AutoAllowed`, `AutoDenied`
- **enum** `AttachmentKind` — The three attachment shapes the composer accepts.
  - variants: `Image`, `File`, `Text`
- **enum** `Block` — One renderable unit inside an assistant turn.
  - variants: `Text`, `Thinking`, `Activity`, `ToolCall`, `ToolGroup`, `Approval`, `Question`,
    `Plan`, `Todo`, `Summary`, `Error`, `Goal`, `Generic`, `Marker`
  - `pub fn approval(id: impl Into<String>, tool: impl Into<String>, command: impl Into<String>, reason: impl Into<String>, cwd: impl Into<String>, capabilities: Vec<String>, scope: ApprovalScope, state: ApprovalState, rule: Option<String>) -> Self` — A pending `Block::Approval` with the MSP-only fields left empty.
  - `pub fn as_tool_call(&self) -> Option<ToolCall>` — The `ToolCall` shape of a `Block::ToolCall`; `None` for any other variant, so a grouping pass can collect runs of tool calls without matching the variant itself.
  - `pub fn complete_approval(&mut self, exit_code: i32, duration_ms: u64) -> bool` — Settle an approval that was `ApprovalState::Approving` once the command has exited. Returns `false` if the block was in any other state.
  - `pub fn decide_approval(&mut self, decision: ApprovalDecision, remembered_rule: Option<String>) -> bool` — Record a decision on an `Block::Approval` block.
  - `pub fn text(text: impl Into<String>) -> Self` — A finished, non-streaming text block.
  - `pub fn tool_call(call: ToolCall) -> Self` — A `Block::ToolCall` built from its struct shape.
- **enum** `ChangeKind` — How a file changed.
  - variants: `Modified`, `Added`, `Deleted`
- **enum** `Delta` — One incremental change to a session, as an adapter emits it while the agent runs.
  - variants: `TurnStarted`, `TextDelta`, `BlockAdded`, `BlockUpdated`, `ThinkingDelta`,
    `ToolOutputDelta`, `TurnRemoved`, `BlockRemoved`, `TurnFinished`
- **enum** `DiffKind` — The three diff row kinds.
  - variants: `Context`, `Add`, `Del`
- **enum** `Environment` — Where the agent process runs.
  - variants: `Local`, `Ssh`, `Server`, `CloudVm`
- **enum** `Intent` — Something the person did that the app must act on.
  - variants: `Send`, `Queue`, `Stop`, `Approve`, `Answer`, `AcceptPlan`, `RejectPlan`,
    `EditPlan`, `OpenFile`, `OpenDiff`, `SendNotes`, `ChangeView`, `ToggleRightPane`, `Steer`,
    `Unqueue`, `EditQueued`, `Compact`, `SetModel`, `SetEffort`, `SetMode`, `Fork`, `Clarify`,
    `CancelQuestion`, `Login`, `Logout`
- **enum** `MarkerKind` — What a `Block::Marker` row announces.
  - variants: `SessionStarted`, `HandOff`, `ContextCompacted`, `PermissionModeChanged`,
    `TurnCancelled`, `TurnRetracted`, `RetryScheduled`, `ViewGap`, `ForkedFrom`
- **enum** `MentionKind` — The things the composer can mention.
  - variants: `File`, `Symbol`, `Worktree`, `Url`, `Skill`
- **enum** `PermissionMode` — How the session gates tool calls that need permission.
  - variants: `AllowAll`, `OnRequest`, `PromptUnmatched`, `DenyUnmatched`
  - `pub fn description(&self) -> &'static str` — The one-line description shown under the label in the mode picker.
  - `pub fn label(&self) -> &'static str` — The picker label, from harness spec §3.6.
- **enum** `PlanState` — Whether a proposed plan has been decided.
  - variants: `Proposed`, `Accepted`, `Rejected`, `Editing`
- **enum** `Provider` — The agent CLIs the harness can drive.
  - variants: `Claude`, `Codex`, `Grok`, `Gemini`, `Pi`, `Cursor`, `Muse`
- **enum** `ReasoningEffort` — How much reasoning the provider should spend on a turn.
  - variants: `None`, `Minimal`, `Low`, `Medium`, `High`, `Xhigh`, `Max`, `Ultra`
  - `pub fn label(&self) -> &'static str` — The picker label for this tier.
- **enum** `ResolvedBy` — Who settled an approval request (MSP `ApprovalResolved.resolvedBy`).
  - variants: `User`, `Policy`, `LlmJudge`
- **enum** `StepState` — The state of one activity step.
  - variants: `Pending`, `Running`, `Done`, `Failed`
- **enum** `ThinkingState` — Whether a reasoning trace is still growing.
  - variants: `Thinking`, `Done`
- **enum** `TodoState` — The state of one todo row.
  - variants: `Pending`, `Running`, `Done`
- **enum** `ToolBody` — The tool-specific payload rendered under a tool call’s header.
  - variants: `Shell`, `Read`, `Edit`, `Search`, `Web`, `Browser`, `SubAgent`, `Mcp`, `None`
- **enum** `ToolKind` — Which tool produced a `crate::Block::ToolCall`.
  - variants: `Shell`, `Read`, `Edit`, `Write`, `Search`, `Web`, `Browser`, `SubAgent`, `Mcp`
- **enum** `ToolStatus` — The lifecycle of a tool call, which picks the header glyph and result pill.
  - variants: `Pending`, `Running`, `Success`, `Error`, `Cancelled`
- **enum** `Turn` — One entry in the transcript.
  - variants: `User`, `Assistant`
  - `pub fn blocks(&self) -> &[Block]` — The blocks of an assistant turn; empty for a user turn.
  - `pub fn blocks_mut(&mut self) -> Option<&mut Vec<Block>>` — Mutable blocks of an assistant turn, or `None` for a user turn.
  - `pub fn id(&self) -> &str` — The turn’s id, whichever variant it is.
- **enum** `UploadState` — Where an attachment is in its upload.
  - variants: `Ready`, `Uploading`, `Failed`
- **enum** `WorkbenchView` — The panes the workbench can show.
  - variants: `Diff`, `Files`, `Terminal`, `Browser`, `Git`, `Docs`

- **type** `AnsiLine` — One line of terminal output.
  - `pub type AnsiLine = String;`

### `aui-protocol::sample`

A realistic sample session, used by `aui-gallery` and the parity screenshots.

- **fn** `approvals` — The five approval states from card 35, for the gallery’s state matrix.
  - `pub fn approvals() -> Vec<Block>`
- **fn** `muse_approvals` — The approval shapes a provider-minted request adds to card 35: server choices, a staged subject, badges, and the resolutions nobody was asked for.
  - `pub fn muse_approvals() -> Vec<Block>`
- **fn** `muse_generic_item` — An item kind this build does not model, drawn the way MSP mandates.
  - `pub fn muse_generic_item() -> Block`
- **fn** `muse_goals` — The session goal, twice: an honest 40 % and a provider that reports 120 %.
  - `pub fn muse_goals() -> Vec<Block>`
- **fn** `muse_plan` — A plan with markdown headings over its steps, for the sections row.
  - `pub fn muse_plan() -> Block`
- **fn** `muse_question` — A question with everything MSP’s `userInput/request` can attach: a header, per-option previews in two formats, and an auto-resolution deadline.
  - `pub fn muse_question() -> Block`
- **fn** `session` — The full sample session: the checkout-flow-v2 worktree from the harness reference screen, with one block of every kind in the order the design cards present them.
  - `pub fn session() -> Session`
- **fn** `tool_call_sample` — One `ToolCall` struct for the gallery: the read call as data.
  - `pub fn tool_call_sample() -> ToolCall`
- **fn** `tool_calls` — Every tool-call body from card 34, for the gallery’s tool-card entry.
  - `pub fn tool_calls() -> Vec<Block>`
- **fn** `tool_groups` — Consecutive tool calls folded into one card, for the gallery’s tool-group entry: a finished group of five and a running group of two.
  - `pub fn tool_groups() -> Vec<Block>`
