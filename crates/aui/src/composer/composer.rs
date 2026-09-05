//! `.cp`: the floating composer card — context chips, the text area, the
//! toolbar with `+`, model / mode / effort chips, context percentage and the
//! send ↔ stop button; plus the optional meta strip above it.

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_motion::{icon_morph, spring_phase, tween, IconMorph, SpringKind, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, radians, relative, App, ElementId, Entity, IntoElement, SharedString, Window};
use gpui_kit::base::input::TextareaState;
use gpui_kit::component::input::Textarea;
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{chip, icon_button};
use crate::util::{interaction_flags, TrackInteraction};

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
/// The send glyph (14) and the press scale (.92).
const SEND_ICON: f32 = 14.0;
const SEND_PRESS: f32 = 0.92;
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
}

/// A chip above the text.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposerChip {
    /// Stable id, reported on removal.
    pub id: SharedString,
    /// What it is.
    pub kind: ComposerChipKind,
    /// Label (`src/checkout`, `form.png`, `test-writer`).
    pub label: SharedString,
    /// Whether it shows the remove `x`.
    pub removable: bool,
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
    /// Open the model picker.
    Model,
    /// Open the mode picker.
    Mode,
    /// Open the effort picker.
    Effort,
    /// Remove a chip.
    RemoveChip(SharedString),
}

type IntentHandler = std::rc::Rc<dyn Fn(ComposerIntent, &mut Window, &mut App)>;

/// Creates the text state a composer renders; keep it in the view (or in
/// window state) and pass it to [`composer`] every frame.
pub fn composer_state(placeholder: impl Into<SharedString>, window: &mut Window, cx: &mut gpui::Context<TextareaState>) -> TextareaState {
    composer_state_rows(placeholder, MIN_ROWS, MAX_ROWS, window, cx)
}

/// Like [`composer_state`] with explicit row bounds (the docked composer starts at one row).
pub fn composer_state_rows(placeholder: impl Into<SharedString>, min_rows: usize, max_rows: usize, window: &mut Window, cx: &mut gpui::Context<TextareaState>) -> TextareaState {
    TextareaState::new(window, cx).placeholder(placeholder).auto_grow(min_rows, max_rows)
}

/// The composer. Build with [`composer`].
#[derive(IntoElement)]
pub struct Composer {
    id: ElementId,
    state: Entity<TextareaState>,
    chips: Vec<ComposerChip>,
    provider: Provider,
    model: SharedString,
    mode: SharedString,
    effort: Option<SharedString>,
    context_percent: Option<u8>,
    streaming: bool,
    can_send: bool,
    focused: bool,
    docked: bool,
    plus_open: bool,
    meta: Option<ComposerMeta>,
    plus_menu: Option<gpui::AnyElement>,
    on_intent: Option<IntentHandler>,
}

/// A composer over `state` for `provider` / `model`.
pub fn composer(id: impl Into<ElementId>, state: &Entity<TextareaState>, provider: Provider, model: impl Into<SharedString>) -> Composer {
    Composer {
        id: id.into(),
        state: state.clone(),
        chips: Vec::new(),
        provider,
        model: model.into(),
        mode: "Plan".into(),
        effort: None,
        context_percent: None,
        streaming: false,
        can_send: true,
        focused: false,
        docked: false,
        plus_open: false,
        meta: None,
        plus_menu: None,
        on_intent: None,
    }
}

impl Composer {
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

    /// Shows `context N%` after the chips.
    pub fn context_percent(mut self, percent: u8) -> Self {
        self.context_percent = Some(percent);
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

    /// The docked variant: full width, top hairline only, no radius or shadow.
    pub fn docked(mut self, docked: bool) -> Self {
        self.docked = docked;
        self
    }

    /// Whether the `+` menu is open (rotates the button); pass the menu element too.
    pub fn plus_menu(mut self, open: bool, menu: Option<impl IntoElement>) -> Self {
        self.plus_open = open;
        self.plus_menu = menu.map(|m| m.into_any_element());
        self
    }

    /// The meta strip above the card.
    pub fn meta(mut self, meta: ComposerMeta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(ComposerIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for Composer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
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
        let mut chips_row = h_flex().w_full().flex_wrap().gap(px(CHIPS_GAP)).pt(px(CHIPS_PAD_TOP)).px(px(CHIPS_PAD_X));
        for c in &self.chips {
            let leading: gpui::AnyElement = match c.kind {
                ComposerChipKind::Mention => div().mono(scale::FS_12).line_height(relative(1.0)).text_color(p.accent_ink).child("@").into_any_element(),
                ComposerChipKind::Skill => div().mono(scale::FS_12).line_height(relative(1.0)).text_color(p.accent_ink).child("$").into_any_element(),
                ComposerChipKind::Image => icon(IconName::Image).size(px(CHIP_GLYPH)).into_any_element(),
                ComposerChipKind::File => icon(IconName::File).size(px(CHIP_GLYPH)).into_any_element(),
            };
            let mut el = chip((id.clone(), SharedString::from(format!("chip-{}", c.id))), c.label.clone()).leading(leading);
            if c.removable {
                let remove = emit(ComposerIntent::RemoveChip(c.id.clone()));
                el = el.trailing(div().id((id.clone(), SharedString::from(format!("chip-x-{}", c.id)))).cursor_pointer().on_click(remove).child(icon(IconName::X).size(px(CHIP_X)).color(p.ink_4)));
            }
            chips_row = chips_row.child(el);
        }

        // Text area.
        let text = div()
            .w_full()
            .min_h(px(if self.docked { DOCKED_TEXT_MIN_H } else { TEXT_MIN_H }))
            .pt(px((TEXT_PAD_TOP - KIT_EDITOR_PAD_Y).max(0.0)))
            .pl(px((if self.docked { DOCKED_TEXT_PAD_L } else { TEXT_PAD_X }) - KIT_EDITOR_PAD_X))
            .pr(px(TEXT_PAD_X - KIT_EDITOR_PAD_X))
            .pb(px((TEXT_PAD_BOTTOM - KIT_EDITOR_PAD_Y).max(0.0)))
            // The kit pads more than the CSS asks for at the bottom; pull the bar up by the difference.
            .mb(px((TEXT_PAD_BOTTOM - KIT_EDITOR_PAD_Y).min(0.0)))
            .ui(TEXT_SIZE)
            .line_height(relative(TEXT_LH))
            .text_color(p.ink)
            .child(Textarea::new(&self.state).appearance(false).bordered(false).text_size(aui_tokens::scaled(TEXT_SIZE)).line_height(relative(TEXT_LH)));

        // Toolbar.
        let plus_turn = spring_phase((id.clone(), "plus"), self.plus_open, SpringKind::Swap, window, cx);
        let plus = icon_button((id.clone(), "plus"), IconName::Plus).on_click(emit(ComposerIntent::TogglePlus));
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
                    .child(icon(IconName::Plus).color(p.ink).rotate(plus_glyph_turn)),
            )
            .children(self.plus_menu);

        let (send_state, send_flags) = interaction_flags((id.clone(), "send"), window, cx);
        let press = spring_phase((id.clone(), "send-press"), send_flags.pressed, SpringKind::Press, window, cx).clamp(0.0, 1.0);
        let send_size = control * (1.0 - (1.0 - SEND_PRESS) * press);
        let sample = icon_morph((id.clone(), "send-stop"), self.streaming, window, cx);
        let enabled = self.streaming || self.can_send;
        let glyph_color = if enabled { gpui::white() } else { p.ink_4 };
        let send_bg = tween((id.clone(), "send-bg"), if enabled { p.accent } else { p.surface_3 }, Tween::FAST, window, cx);
        let glyph = |name: IconName| icon(name).size(px(SEND_ICON)).color(glyph_color);
        let send = div()
            .flex_none()
            .size(control)
            .flex()
            .items_center()
            .justify_center()
            .child(
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
                    .on_click(emit(if self.streaming { ComposerIntent::Stop } else { ComposerIntent::Send }))
                    .child(IconMorph::new(sample, px(SEND_ICON), glyph(IconName::ArrowUp), glyph(IconName::Stop))),
            );

        let mut bar = h_flex()
            .relative()
            .w_full()
            .gap(px(BAR_GAP))
            .pt(px(BAR_PAD_TOP))
            .px(px(if self.docked { DOCKED_BAR_PAD_X } else { BAR_PAD_X }))
            .pb(px(BAR_PAD_BOTTOM))
            .child(plus_holder)
            .child(chip((id.clone(), "model"), self.model.clone()).composer().leading(provider_mark(self.provider).size(px(CHIP_MARK))).chevron().on_click(emit(ComposerIntent::Model)))
            .child({
                // The floating card's mode chip is active with a chevron; the docked one is quiet.
                let mode = chip((id.clone(), "mode"), self.mode.clone()).composer().active(!self.docked).on_click(emit(ComposerIntent::Mode));
                if self.docked { mode } else { mode.chevron() }
            });
        if let Some(effort) = &self.effort {
            bar = bar.child(chip((id.clone(), "effort"), effort.clone()).composer().leading(icon(IconName::Brain).size(px(CHIP_GLYPH))).on_click(emit(ComposerIntent::Effort)));
        }
        if let Some(percent) = self.context_percent {
            bar = bar.child(div().ml(px(CONTEXT_MARGIN)).ui(scale::FS_11).text_color(p.ink_3).whitespace_nowrap().child(format!("context {percent}%")));
        }
        bar = bar.child(div().flex_1()).child(send);

        let mut card = v_flex().id(id.clone()).w_full().bg(p.surface_1);
        card = if self.docked {
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
        root.child(card)
    }
}
