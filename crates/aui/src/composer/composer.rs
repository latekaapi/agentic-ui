//! `.cp`: the floating composer card — context chips, the text area, the
//! toolbar with `+`, provider / model / mode / effort chips, context percentage
//! and the send ↔ stop button; plus the optional meta strip above it.

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_motion::{icon_morph, spring_phase, tween, IconMorph, SpringKind, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, radians, relative, App, ElementId, Entity, IntoElement, SharedString, Window};
use gpui_kit::base::input::TextareaState;
use gpui_kit::component::input::Textarea;
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{chip, context_meter, icon_button, ContextMeterState};
use crate::util::{interaction_flags, named_child, TrackInteraction};

/// `.cp{border-radius:14px}`.
const CARD_RADIUS: f32 = 14.0;
/// `.cp.focus{box-shadow:0 0 0 3px var(--accent-ring)}`.
const FOCUS_RING: f32 = 3.0;
/// `.cp .chips{gap:6px;padding:10px 12px 0}`.
const CHIPS_GAP: f32 = 6.0;
const CHIPS_PAD_TOP: f32 = 10.0;
const CHIPS_PAD_X: f32 = 12.0;
/// `.cp textarea{padding:12px 14px 6px;font:14px/1.55;min-height:44px}`; the docked one is 13.5 / min 40.
const TEXT_PAD_TOP: f32 = 12.0;
const TEXT_PAD_X: f32 = 14.0;
const TEXT_PAD_BOTTOM: f32 = 6.0;
const TEXT_SIZE: f32 = 14.0;
const TEXT_LH: f32 = 1.55;
const TEXT_MIN_H: f32 = 44.0;
/// `.cp .bar{gap:6px;padding:6px 10px 10px}`.
const BAR_GAP: f32 = 6.0;
const BAR_PAD_TOP: f32 = 6.0;
const BAR_PAD_X: f32 = 10.0;
const BAR_PAD_BOTTOM: f32 = 10.0;
/// The chip's provider mark (12) and glyph (11).
const CHIP_MARK: f32 = 12.0;
const CHIP_GLYPH: f32 = 11.0;
/// The `x` in a context chip: 10 px at ink-4.
const CHIP_X: f32 = 10.0;
/// D4 attachment chips: 32 px, radius 6; the upload sweep's period.
const ATTACH_CHIP_H: f32 = 32.0;
const ATTACH_CHIP_RADIUS: f32 = 6.0;
const UPLOAD_SWEEP: std::time::Duration = std::time::Duration::from_millis(1600);
/// An image chip with a decoded thumbnail: a 30 px tile in a 32 px pill (the
/// 1 px border on each side); the tile sits 1 px off the border while the gap
/// and the right pad match the text chips.
const CHIP_THUMB: f32 = 30.0;
const IMAGE_CHIP_H: f32 = 32.0;
const IMAGE_CHIP_PAD_L: f32 = 1.0;
const IMAGE_CHIP_GAP: f32 = 5.0;
const IMAGE_CHIP_PAD_R: f32 = 8.0;
/// The send glyph (14) and the press scale (.92, the docked composer only).
const SEND_ICON: f32 = 14.0;
const SEND_PRESS: f32 = 0.92;
/// The raised composer's stop square, the glyph swap's starting scale and the
/// fill's cross-fade (docs/41 §2.9, §3).
const STOP_SQUARE: f32 = 10.0;
const SWAP_FROM: f32 = 0.6;
const SEND_FADE: Tween = Tween::new(std::time::Duration::from_millis(80), aui_tokens::Easing::OUT);
/// The `+` rotates 45° while the menu is open.
const PLUS_TURN: f32 = std::f32::consts::FRAC_PI_4;
/// `.meta{gap:10px;padding:0 4px;font:500 11px/1 mono}` with a 48 × 4 context meter.
const META_GAP: f32 = 10.0;
const META_PAD_X: f32 = 4.0;
const METER_W: f32 = 48.0;
const METER_H: f32 = 4.0;
const METER_GAP: f32 = 6.0;
/// The context percentage in the bar: `.subtle{font-size:11px;margin-left:6px}`.
const CONTEXT_MARGIN: f32 = 6.0;
/// v3 (Composer A, FF InputMessage): card radius 12, padding 8, 4 between
/// regions and controls; the field 8 px in (text at 16), subtitle 14/20,
/// one row of 20 + 16 = 36 at rest; the provider mark 14.
const RAISED_RADIUS: f32 = 12.0;
const RAISED_PAD: f32 = 8.0;
const RAISED_GAP: f32 = 4.0;
const RAISED_TEXT_PAD_X: f32 = 8.0;
const RAISED_TEXT_LH: f32 = 24.0;
const RAISED_TEXT_MIN_H: f32 = 36.0;
/// The field's text: the body's 16 px (design size, before the 1.1 scale).
const RAISED_TEXT_SIZE: f32 = 16.0 / 1.1;
/// The field's padding, and how far the send button sits from its bottom.
const FIELD_PAD: f32 = 4.0;
const FIELD_SEND_INSET: f32 = 4.0;
/// The field's edge at rest and while it holds the keyboard.
const FIELD_EDGE_REST: f32 = 0.08;
const FIELD_EDGE_FOCUSED: f32 = 0.22;
/// The controls row under the field.
const CONTROLS_TOP: f32 = 6.0;
const CONTROLS_PAD_X: f32 = 2.0;
const RAISED_MARK: f32 = 14.0;
/// Text area rows: grows from 2 to 8 (the docked variant starts at 1).
const MIN_ROWS: usize = 2;
const MAX_ROWS: usize = 8;
/// gpui-kit's multi-line input always pads its editor by these amounts
/// (`Size::Medium`: 8 / 10); the wrapper subtracts them to land on the CSS.
const KIT_EDITOR_PAD_Y: f32 = 8.0;
const KIT_EDITOR_PAD_X: f32 = 10.0;
/// Docked variant: `textarea{min-height:40px;padding-left:32px}`, `.bar{padding-left:28px;padding-right:28px}`.
const DOCKED_TEXT_MIN_H: f32 = 40.0;
const DOCKED_TEXT_PAD_L: f32 = 32.0;
const DOCKED_BAR_PAD_X: f32 = 28.0;

/// What a context chip stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerChipKind {
    /// `@` file or symbol mention.
    Mention,
    /// An image attachment.
    Image,
    /// A file attachment.
    File,
    /// `$` skill.
    Skill,
    /// Words quoted from an answer (D4): `label` holds the first words.
    Quote,
}

/// Where an attachment chip is (docs/41 §2.9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ComposerChipState {
    /// Attached and usable.
    #[default]
    Ready,
    /// Still being read or uploaded: the thumbnail at 50 % and a sweep along
    /// the chip's bottom.
    Uploading,
    /// It could not be attached: the name in danger and the reason as the
    /// chip's meta. ✕ removes it; there is no retry.
    Failed(SharedString),
}

/// Which toolbar control a [`Composer::chip_menu`] hangs off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComposerChipAnchor {
    /// The provider chip.
    Provider,
    /// The model chip.
    Model,
    /// The approval-mode chip.
    Mode,
    /// The reasoning-effort chip.
    Effort,
}

/// A chip above the text.
#[derive(Clone, PartialEq)]
pub struct ComposerChip {
    /// Stable id, reported on removal.
    pub id: SharedString,
    /// What it is.
    pub kind: ComposerChipKind,
    /// Label (`src/checkout`, `form.png`, `test-writer`).
    pub label: SharedString,
    /// Whether it shows the remove `x`.
    pub removable: bool,
    /// A decoded preview for [`ComposerChipKind::Image`], drawn as a rounded
    /// tile in place of the glyph. The caller decodes it once at attach time
    /// and drops it on remove/send, so full bytes never ride along.
    pub thumbnail: Option<std::sync::Arc<gpui::RenderImage>>,
    /// Muted suffix for [`ComposerChipKind::File`] (`PDF · 2.1 MB`).
    pub detail: Option<SharedString>,
    /// Ready, uploading or failed.
    pub state: ComposerChipState,
}

impl std::fmt::Debug for ComposerChip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `RenderImage` has no `Debug`, so the thumbnail reports presence only.
        f.debug_struct("ComposerChip")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("label", &self.label)
            .field("removable", &self.removable)
            .field("thumbnail", &self.thumbnail.is_some())
            .field("detail", &self.detail)
            .field("state", &self.state)
            .finish()
    }
}

/// The optional strip above the card.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposerMeta {
    /// Branch name.
    pub branch: SharedString,
    /// Context used, 0..=1.
    pub context: f32,
    /// Session cost label (`$0.31 this session`).
    pub cost: SharedString,
    /// The right-hand hint (`⌘↩ to queue`).
    pub hint: SharedString,
}

/// What the composer asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum ComposerIntent {
    /// Send the text.
    Send,
    /// Interrupt the running turn.
    Stop,
    /// Toggle the `+` menu.
    TogglePlus,
    /// Open the provider picker.
    Provider,
    /// Open the model picker.
    Model,
    /// Open the mode picker.
    Mode,
    /// Open the effort picker.
    Effort,
    /// Remove a chip.
    RemoveChip(SharedString),
    /// Interject into the turn that is already running instead of queueing
    /// behind it (MSP `turn/steer`; the app binds it to Cmd-Enter).
    Steer,
    /// Leave plan mode (the "Plan" pill in the toolbar).
    ExitPlan,
    /// Attach a file or an image.
    Attach,
    /// Compact the context window now, from the context meter.
    Compact,
}

type IntentHandler = std::rc::Rc<dyn Fn(ComposerIntent, &mut Window, &mut App)>;

/// Creates the text state a composer renders; keep it in the view (or in
/// window state) and pass it to [`composer`] every frame.
pub fn composer_state(placeholder: impl Into<SharedString>, window: &mut Window, cx: &mut gpui::Context<TextareaState>) -> TextareaState {
    composer_state_rows(placeholder, MIN_ROWS, MAX_ROWS, window, cx)
}

/// Like [`composer_state`] with explicit row bounds (the docked composer starts at one row).
pub fn composer_state_rows(placeholder: impl Into<SharedString>, min_rows: usize, max_rows: usize, window: &mut Window, cx: &mut gpui::Context<TextareaState>) -> TextareaState {
    // Enter submits (emits `PressEnter` + propagates, so the app sends) and
    // Shift+Enter still inserts a newline; single-line states never insert,
    // so the flag is a no-op for them.
    TextareaState::new(window, cx).placeholder(placeholder).auto_grow(min_rows, max_rows).submit_on_enter(true)
}

/// The composer. Build with [`composer`].
#[derive(IntoElement)]
pub struct Composer {
    id: ElementId,
    state: Entity<TextareaState>,
    chips: Vec<ComposerChip>,
    quotes: Vec<ComposerQuote>,
    provider: Provider,
    model: SharedString,
    mode: SharedString,
    effort: Option<SharedString>,
    context: Option<ContextMeterState>,
    context_open: Option<bool>,
    plan: bool,
    streaming: bool,
    can_send: bool,
    focused: bool,
    docked: bool,
    raised: bool,
    knowledge_first: bool,
    plus_open: bool,
    meta: Option<ComposerMeta>,
    plus_menu: Option<gpui::AnyElement>,
    chip_menus: Vec<(ComposerChipAnchor, gpui::AnyElement)>,
    on_intent: Option<IntentHandler>,
    model_choices: Option<usize>,
    single_provider: bool,
}

/// The human label for a provider on the provider chip (`Muse`,
/// `Claude Code`, `Codex`, …). This is a user-visible string, so it is an
/// explicit table — never the `Debug` form of the enum.
pub fn provider_display_name(provider: Provider) -> SharedString {
    match provider {
        Provider::Muse => "Muse".into(),
        Provider::Claude => "Claude Code".into(),
        Provider::Codex => "Codex".into(),
        Provider::Grok => "Grok".into(),
        Provider::Gemini => "Gemini".into(),
        Provider::Pi => "Pi".into(),
        Provider::Cursor => "Cursor".into(),
    }
}

/// A composer over `state` for `provider` / `model`.
pub fn composer(id: impl Into<ElementId>, state: &Entity<TextareaState>, provider: Provider, model: impl Into<SharedString>) -> Composer {
    Composer {
        id: id.into(),
        state: state.clone(),
        chips: Vec::new(),
        quotes: Vec::new(),
        provider,
        model: model.into(),
        mode: "Plan".into(),
        effort: None,
        context: None,
        context_open: None,
        plan: false,
        streaming: false,
        can_send: true,
        focused: false,
        docked: false,
        raised: false,
        knowledge_first: false,
        plus_open: false,
        meta: None,
        plus_menu: None,
        chip_menus: Vec::new(),
        on_intent: None,
        model_choices: None, single_provider: false,
    }
}

impl Composer {
    /// Quote blocks inside the field, above the text, 8 apart in the order
    /// they were added (the quote flow, docs/43 §0).
    pub fn quotes(mut self, quotes: Vec<ComposerQuote>) -> Self {
        self.quotes = quotes;
        self
    }

    /// Context chips above the text.
    pub fn chips(mut self, chips: Vec<ComposerChip>) -> Self {
        self.chips = chips;
        self
    }

    /// The mode chip label.
    pub fn mode(mut self, mode: impl Into<SharedString>) -> Self {
        self.mode = mode.into();
        self
    }

    /// The effort chip (brain glyph + level).
    pub fn effort(mut self, effort: impl Into<SharedString>) -> Self {
        self.effort = Some(effort.into());
        self
    }

    /// The context meter in the toolbar: the ring, the percentage (or the raw
    /// token count when the basis has no window) and the hover breakdown.
    pub fn context(mut self, context: ContextMeterState) -> Self {
        self.context = Some(context);
        self
    }

    /// Pins the context meter's breakdown open (or shut) instead of letting it
    /// follow the pointer — what the keyboard and a static capture both need.
    pub fn context_open(mut self, open: bool) -> Self {
        self.context_open = Some(open);
        self
    }

    /// Draws the "Plan" pill in the toolbar; clicking it emits
    /// [`ComposerIntent::ExitPlan`].
    ///
    /// Plan mode is a client-side overlay — MSP has no plan mode — so the pill
    /// says what the app is doing, not what the session is.
    pub fn plan(mut self, plan: bool) -> Self {
        self.plan = plan;
        self
    }

    /// A turn is running: the send button shows stop.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Whether the send button is enabled (surface-3 / ink-4 otherwise).
    pub fn can_send(mut self, can_send: bool) -> Self {
        self.can_send = can_send;
        self
    }

    /// Draws the focused ring (accent border + 3 px accent-ring).
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// v3's composer (canvas "Baaz v3", Composer A): one Raised card with
    /// the edge, radius 12, FF InputMessage padding (8 around, the field
    /// padded 8 so the text sits at 16, 4 between field and controls); the
    /// controls are text with chevrons (no bordered chips); one round 28 px
    /// send at Floating with the edge. Overrides [`Composer::docked`].
    pub fn raised(mut self, raised: bool) -> Self {
        self.raised = raised;
        self
    }

    /// The docked variant: full width, top hairline only, no radius or shadow.
    pub fn docked(mut self, docked: bool) -> Self {
        self.docked = docked;
        self
    }

    /// Leads the toolbar with what the answer is grounded in instead of the
    /// model: the mode chip moves before the model chip and reads as a
    /// knowledge chip — book glyph, chevron, never the quiet docked style.
    /// The assistant screens use it; the harness leads with the model.
    pub fn knowledge_first(mut self, knowledge_first: bool) -> Self {
        self.knowledge_first = knowledge_first;
        self
    }

    /// Whether the `+` menu is open (rotates the button); pass the menu element too.
    pub fn plus_menu(mut self, open: bool, menu: Option<impl IntoElement>) -> Self {
        self.plus_open = open;
        self.plus_menu = menu.map(|m| m.into_any_element());
        self
    }

    /// Hangs a picker off one of the toolbar chips. The menu renders inside
    /// that chip's own positioning holder, so it stays anchored to the control
    /// that opened it however wide the composer is; the menu itself is
    /// responsible for lifting onto [`crate::overlay::popover_layer`].
    pub fn chip_menu(mut self, anchor: ComposerChipAnchor, menu: impl IntoElement) -> Self {
        self.chip_menus.push((anchor, menu.into_any_element()));
        self
    }

    /// The meta strip above the card.
    pub fn meta(mut self, meta: ComposerMeta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// How many models the model chip can pick from. The chip shows its
    /// chevron and stays clickable only with more than one choice; unset
    /// keeps the current behaviour (chevron shown, clickable).
    pub fn model_choices(mut self, n: usize) -> Self {
        self.model_choices = Some(n);
        self
    }

    /// A host with exactly one provider: no provider chip (there is nothing to
    /// pick), and the model chip carries the provider mark instead.
    pub fn single_provider(mut self, single: bool) -> Self {
        self.single_provider = single;
        self
    }

    /// Whether the model chip opens the model picker: more than one choice,
    /// or unset (which keeps the current behaviour).
    fn model_selectable(&self) -> bool {
        self.model_choices.map(|n| n > 1).unwrap_or(true)
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(ComposerIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }
}

/// A quote waiting in the composer (the quote flow, docs/43 §0): the
/// selection's text, flattened for display; `code` = (language, lines) for a
/// selection inside one code block or Well output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerQuote {
    /// Stable id; ✕ reports [`ComposerIntent::RemoveChip`] with it.
    pub id: SharedString,
    /// The quoted text as shown (prose flattened; code as written).
    pub text: SharedString,
    /// A code quote: its language and line count.
    pub code: Option<(SharedString, usize)>,
}

/// The rule's alpha on a quote (white 18 % in dark: the foreground at 18 %).
const QUOTE_RULE_ALPHA: f32 = 0.18;

/// One quote block inside the composer field (docs/43 §0): a 2 px rule at
/// 18 %, padding 1 / 28 / 1 / 12, text 14.3 / 22 muted clamped to 2 lines,
/// ✕ (20, r 4) top-right "Remove quote". A code quote: mono 12.65 / 20,
/// 3 lines, then `rust · 2 lines` in faint 12.1.
fn composer_quote_block(id: &ElementId, q: &ComposerQuote, p: &aui_tokens::Palette, remove: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> gpui::AnyElement {
    // Between FG and muted: #C9C9D0 on dark, #3F3F46 on light (docs/43 §0).
    let code_ink: gpui::Hsla = if p.is_dark() { gpui::rgb(0xC9C9D0).into() } else { gpui::rgb(0x3F3F46).into() };
    let body = match &q.code {
        Some((lang, lines)) => v_flex()
            .child(
                div()
                    .font_family(scale::FONT_MONO)
                    .text_px(12.65)
                    .line_height(px(20.0))
                    .text_color(code_ink)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .line_clamp(3)
                    .max_h(px(60.0))
                    .child(q.text.clone()),
            )
            .child(
                div()
                    .mt(px(2.0))
                    .font_family(scale::FONT_UI)
                    .text_px(12.1)
                    .line_height(px(16.0))
                    .text_color(p.ink_4)
                    .child(format!("{lang} · {lines} line{}", if *lines == 1 { "" } else { "s" })),
            ),
        None => v_flex().child(
            div().font_family(scale::FONT_UI).text_px(14.3).line_height(px(22.0)).text_color(p.fg_muted).line_clamp(2).max_h(px(44.0)).overflow_hidden().child(q.text.clone()),
        ),
    };
    div()
        .id(named_child(id, "quote-", &q.id))
        .relative()
        .w_full()
        .pt(px(1.0))
        .pb(px(1.0))
        .pl(px(12.0))
        .pr(px(28.0))
        .border_l(px(2.0))
        .border_color(p.fg.alpha(QUOTE_RULE_ALPHA))
        .role(gpui::Role::Group)
        .aria_label(SharedString::from(format!("Quote: {}", q.text)))
        .child(body)
        .child(
            div()
                .id(named_child(id, "quote-x-", &q.id))
                .absolute()
                .right(px(0.0))
                .top(px(1.0))
                .size(px(20.0))
                .rounded(px(4.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(p.hover_overlay()))
                .role(gpui::Role::Button)
                .aria_label("Remove quote")
                .on_click(remove)
                .child(icon(IconName::X).size(px(11.0)).color(p.fg_muted)),
        )
        .into_any_element()
}

/// One attachment chip in the raised composer (D4, docs/41 §2.9):
/// Raised-lit, radius 6, the edge, 32 tall. An image leads with its 30 px
/// thumbnail (radius 5, 1 px in); a file with its glyph and `TYPE · size`
/// muted; a quote with the edit glyph, its first words and `quote`. While it
/// uploads the thumbnail dims to 50 % and a 2 px foreground sweep runs along
/// the chip's bottom; failed, the name turns danger and the reason is the
/// meta. ✕ removes it.
fn attachment_chip(id: &ElementId, c: &ComposerChip, p: &aui_tokens::Palette, remove: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
    let uploading = c.state == ComposerChipState::Uploading;
    let failed = match &c.state {
        ComposerChipState::Failed(reason) => Some(reason.clone()),
        _ => None,
    };
    let thumb = c.thumbnail.clone().filter(|_| c.kind == ComposerChipKind::Image);
    let mut chip = h_flex()
        .id(named_child(id, "chip-", &c.id))
        .relative()
        .flex_none()
        .h(px(ATTACH_CHIP_H))
        .pl(px(if thumb.is_some() { 1.0 } else { 8.0 }))
        .pr(px(4.0))
        .gap(px(6.0))
        .items_center()
        .rounded(px(ATTACH_CHIP_RADIUS))
        .bg(crate::util::raised_lit(p, p.surface_2))
        .shadow(p.edge())
        .overflow_hidden()
        .font_family(scale::FONT_UI)
        .text_px(scale::FS_13)
        .whitespace_nowrap()
        .text_color(p.ink);
    chip = match (&thumb, c.kind) {
        (Some(image), _) => chip.child(
            div()
                .flex_none()
                .size(px(CHIP_THUMB))
                .rounded(px(5.0))
                .overflow_hidden()
                .when(uploading, |d| d.opacity(0.5))
                .child(gpui::img(image.clone()).object_fit(gpui::ObjectFit::Cover).w_full().h_full()),
        ),
        (None, ComposerChipKind::Image) => chip.child(icon(IconName::Image).size(px(13.0)).color(p.ink_2)),
        (None, ComposerChipKind::File) => chip.child(icon(IconName::File).size(px(13.0)).color(p.ink_2)),
        (None, ComposerChipKind::Quote) => chip.child(icon(IconName::Edit).size(px(13.0)).color(p.ink_2)),
        (None, ComposerChipKind::Mention) => chip.child(div().mono(scale::FS_12).text_color(p.ink_2).child("@")),
        (None, ComposerChipKind::Skill) => chip.child(div().mono(scale::FS_12).text_color(p.ink_2).child("$")),
    };
    let label: SharedString = if c.kind == ComposerChipKind::Quote { format!("“{}”", c.label).into() } else { c.label.clone() };
    chip = chip.child(div().max_w(px(260.0)).truncate().when(failed.is_some(), |d| d.text_color(p.danger)).child(label));
    let meta: Option<SharedString> = match (&failed, c.kind) {
        (Some(reason), _) => Some(reason.clone()),
        (None, ComposerChipKind::Quote) => Some("quote".into()),
        (None, ComposerChipKind::File) => c.detail.clone(),
        _ => None,
    };
    if let Some(meta) = meta {
        chip = chip.child(div().flex_none().text_px(scale::FS_12).text_color(p.ink_2).child(meta));
    }
    if c.removable {
        chip = chip.child(
            div()
                .id(named_child(id, "chip-x-", &c.id))
                .flex_none()
                .size(px(20.0))
                .rounded(px(4.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(p.hover_overlay()))
                .role(gpui::Role::Button)
                .aria_label(SharedString::from(format!("Remove {}", c.label)))
                .on_click(remove)
                .child(icon(IconName::X).size(px(CHIP_X)).color(p.ink_2)),
        );
    }
    if uploading {
        // A 2 px foreground sweep, 35 % of the chip wide, every 1.6 s.
        let phase = aui_motion::looping((named_child(id, "chip-sweep-", &c.id), "sweep"), aui_motion::Loop::linear(UPLOAD_SWEEP), window, cx);
        chip = chip.child(div().absolute().bottom_0().left(gpui::relative(phase * 1.35 - 0.35)).w(gpui::relative(0.35)).h(px(2.0)).bg(p.ink));
    }
    chip.into_any_element()
}

impl RenderOnce for Composer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let model_selectable = self.model_selectable();
        let control = cx.aui().metrics.control_md;
        let handler = self.on_intent.clone();
        let emit = move |intent: ComposerIntent| {
            let handler = handler.clone();
            move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(intent.clone(), w, cx)
                }
            }
        };

        // Chips row.
        let mut chips_row = h_flex().w_full().flex_wrap().gap(px(CHIPS_GAP));
        // v3: the card's 8 px padding already holds the row; the field's 8 px
        // inset lines the chips up with the text.
        chips_row = if self.raised { chips_row.px(px(RAISED_TEXT_PAD_X)).pt(px(RAISED_TEXT_PAD_X)) } else { chips_row.pt(px(CHIPS_PAD_TOP)).px(px(CHIPS_PAD_X)) };
        for c in self.chips.iter().filter(|_| self.raised) {
            chips_row = chips_row.child(attachment_chip(&id, c, &p, emit(ComposerIntent::RemoveChip(c.id.clone())), window, cx));
        }
        for c in self.chips.iter().filter(|_| !self.raised) {
            // An image with a decoded preview wears its thumbnail: a rounded
            // tile in place of the glyph, in a pill sized to the tile.
            if c.kind == ComposerChipKind::Image {
                if let Some(thumb) = &c.thumbnail {
                    let tile = div()
                        .size(px(CHIP_THUMB))
                        .rounded(px(scale::R_SM))
                        .overflow_hidden()
                        .child(gpui::img(thumb.clone()).object_fit(gpui::ObjectFit::Cover).w_full().h_full());
                    let mut pill = div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .h(px(IMAGE_CHIP_H))
                        .pl(px(IMAGE_CHIP_PAD_L))
                        .pr(px(IMAGE_CHIP_PAD_R))
                        .gap(px(IMAGE_CHIP_GAP))
                        .rounded(px(scale::R_SM))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.surface_1)
                        .font_family(scale::FONT_UI)
                        .text_px(scale::FS_12)
                        .line_height(relative(1.0))
                        .medium()
                        .whitespace_nowrap()
                        .text_color(p.ink_2)
                        .child(tile)
                        .child(c.label.clone());
                    if c.removable {
                        let remove = emit(ComposerIntent::RemoveChip(c.id.clone()));
                        pill = pill.child(
                            div()
                                .id(named_child(&id, "chip-x-", &c.id))
                                .cursor_pointer()
                                .role(gpui::Role::Button)
                                .aria_label(SharedString::from(format!("Remove {}", c.label)))
                                .on_click(remove)
                                .child(icon(IconName::X).size(px(CHIP_X)).color(p.ink_4)),
                        );
                    }
                    chips_row = chips_row.child(pill);
                    continue;
                }
            }
            let leading: gpui::AnyElement = match c.kind {
                ComposerChipKind::Mention => div().mono(scale::FS_12).line_height(relative(1.0)).text_color(p.accent_ink).child("@").into_any_element(),
                ComposerChipKind::Skill => div().mono(scale::FS_12).line_height(relative(1.0)).text_color(p.accent_ink).child("$").into_any_element(),
                ComposerChipKind::Image => icon(IconName::Image).size(px(CHIP_GLYPH)).into_any_element(),
                ComposerChipKind::File => icon(IconName::File).size(px(CHIP_GLYPH)).into_any_element(),
                ComposerChipKind::Quote => icon(IconName::Edit).size(px(CHIP_GLYPH)).into_any_element(),
            };
            let mut el = chip(named_child(&id, "chip-", &c.id), c.label.clone()).leading(leading);
            // A file names its kind and size after the glyph, muted.
            if c.kind == ComposerChipKind::File {
                if let Some(detail) = &c.detail {
                    el = el.detail(detail.clone());
                }
            }
            if c.removable {
                let remove = emit(ComposerIntent::RemoveChip(c.id.clone()));
                el = el.trailing(
                    div()
                        .id(named_child(&id, "chip-x-", &c.id))
                        .cursor_pointer()
                        .role(gpui::Role::Button)
                        .aria_label(SharedString::from(format!("Remove {}", c.label)))
                        .on_click(remove).child(icon(IconName::X).size(px(CHIP_X)).color(p.ink_4)));
            }
            chips_row = chips_row.child(el);
        }

        // Text area. The a11y frame is the node assistive technology sees:
        // role, the "Message" label and the live value, with SetValue /
        // ReplaceSelectedText / Focus routed into the state so dictation,
        // VoiceOver and automation write through it instead of into the void.
        let raised = self.raised;
        let docked = self.docked && !raised;
        let text = crate::a11y_text::a11y_text_field((id.clone(), "text"), "Message", &self.state, true, cx)
            .w_full()
            .min_h(px(if raised { RAISED_TEXT_MIN_H } else if docked { DOCKED_TEXT_MIN_H } else { TEXT_MIN_H }))
            .when(!raised, |d| {
                d.pt(px((TEXT_PAD_TOP - KIT_EDITOR_PAD_Y).max(0.0)))
                    .pl(px((if docked { DOCKED_TEXT_PAD_L } else { TEXT_PAD_X }) - KIT_EDITOR_PAD_X))
                    .pr(px(TEXT_PAD_X - KIT_EDITOR_PAD_X))
                    .pb(px((TEXT_PAD_BOTTOM - KIT_EDITOR_PAD_Y).max(0.0)))
                    // The kit pads more than the CSS asks for at the bottom; pull the bar up by the difference.
                    .mb(px((TEXT_PAD_BOTTOM - KIT_EDITOR_PAD_Y).min(0.0)))
            })
            // v3: the kit's own editor padding (8 / 10) is the field's 8 /
            // 8, less 2 px each side, so the text lands at 16 from the card.
            .when(raised, |d| d.mx(px(RAISED_TEXT_PAD_X - KIT_EDITOR_PAD_X)))
            .ui(if raised { RAISED_TEXT_SIZE } else { TEXT_SIZE })
            .line_height(if raised { relative(RAISED_TEXT_LH / RAISED_TEXT_SIZE) } else { relative(TEXT_LH) })
            .text_color(p.ink)
            .child(
                Textarea::new(&self.state)
                    .appearance(false)
                    .bordered(false)
                    .text_size(aui_tokens::scaled(if raised { RAISED_TEXT_SIZE } else { TEXT_SIZE }))
                    .line_height(if raised { relative(RAISED_TEXT_LH / RAISED_TEXT_SIZE) } else { relative(TEXT_LH) }),
            );

        // Toolbar.
        let plus_turn = spring_phase((id.clone(), "plus"), self.plus_open, SpringKind::Swap, window, cx);
        let plus = icon_button((id.clone(), "plus"), IconName::Plus)
            .accessibility_label("Attach or add")
            .on_click(emit(ComposerIntent::TogglePlus));
        // v3: the `+` is a ghost icon button on the card, like every control.
        let plus = if raised { plus.ghost() } else { plus };
        let plus_glyph_turn = radians(plus_turn * PLUS_TURN);
        let plus_holder = div()
            .relative()
            .flex_none()
            .child(plus.icon_size(px(0.0)))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(IconName::Plus).color(if raised { p.ink_2 } else { p.ink }).rotate(plus_glyph_turn)),
            )
            .children(self.plus_menu);

        let (send_state, send_flags) = interaction_flags((id.clone(), "send"), window, cx);
        let enabled = self.streaming || self.can_send;
        let send = if raised {
            // D4 (docs/41 §2.9): the composer's only solid fill. Empty: the
            // round 28 px button at Floating with the edge and a muted arrow.
            // Ready (a draft, an attachment or a queued line): the
            // foreground fill, a frame-coloured arrow, no edge. Running: the
            // same fill with a 10 px frame-coloured square. The fill
            // cross-fades in 80 ms; the glyph swaps on spring.fast (scale
            // 0.6 → 1 and opacity). Nothing moves.
            let bg: gpui::Hsla = tween((id.clone(), "send-fill"), if enabled { p.ink } else { p.overlay }, SEND_FADE, window, cx);
            let ink = if enabled { p.bg } else { p.ink_2 };
            let swap = spring_phase((id.clone(), "send-swap"), self.streaming, SpringKind::Fast, window, cx).clamp(0.0, 1.0);
            let arrow = 1.0 - swap;
            div().flex_none().size(control).flex().items_center().justify_center().child(
                div()
                    .id((id.clone(), "send"))
                    .relative()
                    .size(control)
                    .rounded_full()
                    .when(!enabled, |d| d.shadow(p.edge()))
                    .bg(bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .track_interaction(&send_state)
                    .role(gpui::Role::Button)
                    .aria_label(if self.streaming { "Stop" } else { "Send" })
                    .on_click(emit(if self.streaming { ComposerIntent::Stop } else { ComposerIntent::Send }))
                    .when(arrow > 0.001, |d| {
                        d.child(
                            div().absolute().inset_0().flex().items_center().justify_center().opacity(arrow).child(
                                icon(IconName::ArrowUp).size(px(SEND_ICON * (SWAP_FROM + (1.0 - SWAP_FROM) * arrow))).color(ink).lit(true),
                            ),
                        )
                    })
                    .when(swap > 0.001, |d| {
                        let side = STOP_SQUARE * (SWAP_FROM + (1.0 - SWAP_FROM) * swap);
                        d.child(div().absolute().inset_0().flex().items_center().justify_center().opacity(swap).child(div().size(px(side)).rounded(px(2.0)).bg(ink)))
                    }),
            )
        } else {
            let press = spring_phase((id.clone(), "send-press"), send_flags.pressed, SpringKind::Press, window, cx).clamp(0.0, 1.0);
            let send_size = control * (1.0 - (1.0 - SEND_PRESS) * press);
            let sample = icon_morph((id.clone(), "send-stop"), self.streaming, window, cx);
            let glyph_color = if enabled { gpui::white() } else { p.ink_4 };
            let send_bg = tween((id.clone(), "send-bg"), if enabled { p.accent } else { p.surface_3 }, Tween::FAST, window, cx);
            let glyph = |name: IconName| icon(name).size(px(SEND_ICON)).color(glyph_color);
            div().flex_none().size(control).flex().items_center().justify_center().child(
                div()
                    .id((id.clone(), "send"))
                    .size(send_size)
                    .rounded(px(scale::R_SM))
                    .bg(send_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .track_interaction(&send_state)
                    .role(gpui::Role::Button)
                    .aria_label(if self.streaming { "Stop" } else { "Send" })
                    .on_click(emit(if self.streaming { ComposerIntent::Stop } else { ComposerIntent::Send }))
                    .child(IconMorph::new(sample, px(SEND_ICON), glyph(IconName::ArrowUp), glyph(IconName::Stop))),
            )
        };

        let mut bar = h_flex().relative().w_full();
        bar = if raised {
            bar.gap(px(RAISED_GAP))
        } else {
            bar.gap(px(BAR_GAP)).pt(px(BAR_PAD_TOP)).px(px(if docked { DOCKED_BAR_PAD_X } else { BAR_PAD_X })).pb(px(BAR_PAD_BOTTOM))
        };
        bar = bar.child(plus_holder);
        // v3: every control is text with a chevron; the provider lit.
        let style = |c: crate::data::Chip| if raised { c.text_control() } else { c.composer() };
        // The provider chip carries the mark; the model chip carries only the
        // model name, so the mark is never drawn twice in one row.
        let provider_label = provider_display_name(self.provider);
        let provider_chip = style(chip((id.clone(), "provider"), provider_label.clone()))
            .active(raised)
            .leading(provider_mark(self.provider).size(px(if raised { RAISED_MARK } else { CHIP_MARK })))
            .chevron()
            .accessibility_label(SharedString::from(format!("{provider_label}, provider")))
            .on_click(emit(ComposerIntent::Provider));
        let mut model_chip = style(chip((id.clone(), "model"), self.model.clone()))
            .accessibility_label(SharedString::from(format!("{}, model", self.model)));
        if self.single_provider {
            model_chip = model_chip.leading(provider_mark(self.provider).size(px(CHIP_MARK)));
        }
        if model_selectable {
            model_chip = model_chip.chevron().on_click(emit(ComposerIntent::Model));
        }
        let mode_chip = {
            let mode = style(chip((id.clone(), "mode"), self.mode.clone()))
                .accessibility_label(SharedString::from(format!(
                    "{}, {}",
                    self.mode,
                    if self.knowledge_first { "grounding" } else { "approval mode" }
                )))
                .on_click(emit(ComposerIntent::Mode));
            if self.knowledge_first {
                // The grounding chip: what the answer is grounded in, read left to right.
                mode.leading(icon(IconName::Book).size(px(CHIP_GLYPH))).chevron()
            } else {
                // The floating card's mode chip is active with a chevron; the
                // docked one is quiet; v3's is muted text with a chevron.
                if raised {
                    mode.chevron()
                } else {
                    let mode = mode.active(!docked);
                    if docked { mode } else { mode.chevron() }
                }
            }
        };
        // Each chip sits in its own positioning holder so the picker that
        // hangs off it is anchored to the control, not to the toolbar.
        let mut menus = self.chip_menus;
        let mut anchored = |anchor: ComposerChipAnchor, chip: gpui::AnyElement| {
            let mut holder = div().relative().flex_none().child(chip);
            let mut rest = Vec::new();
            for (at, menu) in menus.drain(..) {
                if at == anchor {
                    holder = holder.child(menu);
                } else {
                    rest.push((at, menu));
                }
            }
            menus = rest;
            holder
        };
        let provider_chip = anchored(ComposerChipAnchor::Provider, provider_chip.into_any_element());
        let model_chip = anchored(ComposerChipAnchor::Model, model_chip.into_any_element());
        let mode_chip = anchored(ComposerChipAnchor::Mode, mode_chip.into_any_element());
        let effort_chip = self.effort.as_ref().map(|effort| {
            let mut c = style(chip((id.clone(), "effort"), effort.clone()))
                .accessibility_label(SharedString::from(format!("{effort}, reasoning effort")))
                .on_click(emit(ComposerIntent::Effort));
            c = if raised { c.chevron() } else { c.leading(icon(IconName::Brain).size(px(CHIP_GLYPH))) };
            anchored(ComposerChipAnchor::Effort, c.into_any_element())
        });
        // v3 (Composer A): left = + · provider · the count; right = model ·
        // mode · effort · send.
        let mut right = h_flex().flex_none().items_center().gap(px(RAISED_GAP));
        if raised {
            (bar, right) = match (self.knowledge_first, self.single_provider) {
                (true, false) => (bar.child(mode_chip).child(provider_chip), right.child(model_chip)),
                (true, true) => (bar.child(mode_chip), right.child(model_chip)),
                (false, false) => (bar.child(provider_chip), right.child(model_chip).child(mode_chip)),
                (false, true) => (bar, right.child(model_chip).child(mode_chip)),
            };
            right = right.children(effort_chip);
        } else {
            bar = match (self.knowledge_first, self.single_provider) {
                (true, false) => bar.child(mode_chip).child(provider_chip).child(model_chip),
                (true, true) => bar.child(mode_chip).child(model_chip),
                (false, false) => bar.child(provider_chip).child(model_chip).child(mode_chip),
                (false, true) => bar.child(model_chip).child(mode_chip),
            };
            bar = bar.children(effort_chip);
        }
        if self.plan {
            // Plan mode is the app's own overlay, so its pill is an active chip
            // that turns itself off rather than a status pill.
            bar = bar.child(
                style(chip((id.clone(), "plan"), SharedString::from("Plan")))
                    .active(true)
                    .leading(icon(IconName::List).size(px(CHIP_GLYPH)))
                    .trailing(icon(IconName::X).size(px(CHIP_X)).color(p.ink_4))
                    .accessibility_label("Plan mode on, turn off")
                    .on_click(emit(ComposerIntent::ExitPlan)),
            );
        }
        if let Some(context) = self.context {
            let handler = self.on_intent.clone();
            let mut meter = context_meter((id.clone(), "context"), context).on_compact(move |w, cx| {
                if let Some(h) = &handler {
                    h(ComposerIntent::Compact, w, cx)
                }
            });
            if let Some(open) = self.context_open {
                meter = meter.open(open);
            }
            bar = bar.child(div().ml(px(if raised { RAISED_GAP } else { CONTEXT_MARGIN })).relative().flex_none().child(meter));
        }
        // Claude's shape (owner 2026-10-14): the send button lives in the
        // field; the controls row stands outside it.
        let mut send = Some(send);
        bar = if raised {
            bar.child(div().flex_1()).child(right)
        } else {
            bar.child(div().flex_1()).children(send.take())
        };

        if raised {
            // The field: its own rounded box, the text and the send button,
            // chips above them; its edge brightens while it holds the
            // keyboard (white 8 % → 22 %), read from the field's own focus
            // handle so no host has to report it.
            use gpui::Focusable as _;
            let focused = self.focused || self.state.read(cx).focus_handle(cx).contains_focused(window, cx);
            let edge = tween((id.clone(), "field-edge"), if focused { p.ink.alpha(FIELD_EDGE_FOCUSED) } else { p.ink.alpha(FIELD_EDGE_REST) }, SEND_FADE, window, cx);
            let mut field = v_flex()
                .id(id.clone())
                .w_full()
                .bg(p.surface_3)
                .rounded(px(RAISED_RADIUS))
                .border_1()
                .border_color(edge)
                // It floats over the transcript (owner 2026-10-15): the
                // floating drop under it, as menus and the jump pill wear.
                .shadow(p.floating_shadow())
                .p(px(FIELD_PAD));
            if !self.quotes.is_empty() {
                let mut stack = v_flex().w_full().gap(px(8.0)).px(px(RAISED_TEXT_PAD_X)).pt(px(RAISED_TEXT_PAD_X));
                for q in &self.quotes {
                    stack = stack.child(composer_quote_block(&id, q, &p, emit(ComposerIntent::RemoveChip(q.id.clone()))));
                }
                field = field.child(stack);
            }
            if !self.chips.is_empty() {
                field = field.child(chips_row);
            }
            field = field.child(
                h_flex()
                    .w_full()
                    .items_end()
                    .gap(px(RAISED_GAP))
                    .child(div().flex_1().min_w(px(0.0)).child(text))
                    .child(div().flex_none().pb(px(FIELD_SEND_INSET)).children(send.take())),
            );
            let controls = bar.mt(px(CONTROLS_TOP)).px(px(CONTROLS_PAD_X));
            return v_flex().w_full().child(field).child(controls).into_any_element();
        }

        let mut card = v_flex().id(id.clone()).w_full().bg(p.surface_1);
        card = if raised {
            card.bg(p.surface_3).rounded(px(RAISED_RADIUS)).shadow(p.elevation_shadow(3)).p(px(RAISED_PAD)).gap(px(RAISED_GAP))
        } else if docked {
            card.border_t_1().border_color(p.line)
        } else {
            card.rounded(px(CARD_RADIUS)).border_1().border_color(if self.focused { p.accent } else { p.line_strong }).shadow(if self.focused {
                vec![gpui::BoxShadow { color: p.accent_ring, offset: gpui::point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(FOCUS_RING), inset: false }]
            } else {
                p.shadow(1)
            })
        };
        if !self.chips.is_empty() {
            card = card.child(chips_row);
        }
        card = card.child(text).child(bar);

        let mut root = v_flex().w_full();
        if let Some(meta) = self.meta {
            root = root.child(
                h_flex()
                    .w_full()
                    .gap(px(META_GAP))
                    .px(px(META_PAD_X))
                    .text_role(TextRole::MonoSmall)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(p.ink_3)
                    .whitespace_nowrap()
                    .child(format!("⎇ {}", meta.branch))
                    .child(
                        h_flex()
                            .gap(px(METER_GAP))
                            .child("context")
                            .child(div().w(px(METER_W)).h(px(METER_H)).rounded(px(METER_H / 2.0)).bg(p.surface_3).overflow_hidden().child(div().h_full().w(px(METER_W * meta.context.clamp(0.0, 1.0))).bg(p.accent)))
                            .child(format!("{}%", (meta.context * 100.0).round())),
                    )
                    .child(meta.cost)
                    .child(div().flex_1())
                    .child(meta.hint),
            );
        }
        root.child(card).into_any_element()
    }
}
