//! Card 31: the user bubble (with attachments and mentions, hover actions)
//! and the assistant turn (full-width markdown, hover toolbar, footer meta,
//! streaming caret).

use aui_icons::{icon, IconName};
use aui_motion::{looping, tween, Loop, Tween};
use aui_protocol::{Attachment, AttachmentKind, TurnMeta};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};

use crate::transcript::{prose, ProseStyle};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, ButtonSize};
use crate::util::{interaction_flags, TrackInteraction};

/// `.u{max-width:78%;gap:6px}`.
const USER_MAX: f32 = 0.78;
const USER_GAP: f32 = 6.0;
/// `.u .bub{border-radius:14px 14px 4px 14px;padding:9px 13px;font-size:13.5px;line-height:1.5}`.
const BUBBLE_RADIUS: f32 = 14.0;
const BUBBLE_PAD_Y: f32 = 9.0;
const BUBBLE_PAD_X: f32 = 13.0;
const BUBBLE_TEXT: f32 = 13.5;
/// `.u .acts{left:-70px;top:4px;gap:2px}`.
const USER_ACTS_LEFT: f32 = -70.0;
const USER_ACTS_TOP: f32 = 4.0;
const ACTS_GAP: f32 = 2.0;
const ACTS_GLYPH: f32 = 12.0;
/// `.fchip{height:30px;padding:0 10px 0 6px;gap:6px;font-size:12px}` with an 18 px icon tile and 11 px glyph.
const CHIP_H: f32 = 30.0;
const CHIP_PAD_L: f32 = 6.0;
const CHIP_PAD_R: f32 = 10.0;
const CHIP_GAP: f32 = 6.0;
const CHIP_TILE: f32 = 18.0;
const CHIP_GLYPH: f32 = 11.0;
/// `.a{font-size:13.5px;line-height:1.65}`; `.a p{margin:0 0 10px}`.
const BODY_TEXT: f32 = 13.5;
const PARAGRAPH_GAP: f32 = 10.0;
/// `.a .tb{right:0;top:-30px;gap:2px;padding:2px}` with a 4 px rise.
const TOOLBAR_TOP: f32 = -30.0;
const TOOLBAR_PAD: f32 = 2.0;
const TOOLBAR_RISE: f32 = 4.0;
/// `.a .ft{gap:10px;font:500 11px/1 mono;margin-top:10px}`.
const FOOTER_GAP: f32 = 10.0;
const FOOTER_TOP: f32 = 10.0;
/// `.caret` blinks every second (steps 2).
const CARET_PERIOD: std::time::Duration = std::time::Duration::from_millis(1000);

/// Actions on a user turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserTurnAction {
    /// Edit and resend.
    Edit,
    /// Copy the text.
    Copy,
    /// Send again as is.
    Resend,
}

/// Actions on an assistant turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantTurnAction {
    /// Copy the text.
    Copy,
    /// Regenerate.
    Retry,
    /// Fork the conversation here.
    Fork,
    /// Bookmark.
    Pin,
}

type UserHandler = std::rc::Rc<dyn Fn(UserTurnAction, &mut Window, &mut App)>;
type AssistantHandler = std::rc::Rc<dyn Fn(AssistantTurnAction, &mut Window, &mut App)>;

/// The person's turn. Build with [`user_turn`].
#[derive(IntoElement)]
pub struct UserTurn {
    id: ElementId,
    markdown: SharedString,
    attachments: Vec<Attachment>,
    on_action: Option<UserHandler>,
}

/// A user turn; `markdown` may carry mentions as inline code (`` `@src/checkout` ``),
/// which render as mention chips.
pub fn user_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> UserTurn {
    UserTurn { id: id.into(), markdown: markdown.into(), attachments: Vec::new(), on_action: None }
}

impl UserTurn {
    /// Attachments shown above the bubble.
    pub fn attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// Hover-action handler.
    pub fn on_action(mut self, f: impl Fn(UserTurnAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }
}

/// Prose style shared by both turns: inline code on `code_bg` in the mono face.
fn prose_style(p: &Palette, size: f32, line_height: f32, code_bg: gpui::Hsla, code_ink: gpui::Hsla) -> ProseStyle {
    ProseStyle { ink: p.ink, code_ink, code_bg, size, line_height, paragraph_gap: PARAGRAPH_GAP }
}

fn attachment_chip(p: &Palette, index: usize, attachment: &Attachment) -> impl IntoElement {
    let (tile_bg, tile_ink, glyph) = match attachment.kind {
        AttachmentKind::Image => (p.surface_3, p.ink_2, IconName::Image),
        _ => (p.info_soft, p.info, IconName::File),
    };
    let mut chip = h_flex()
        .id(("attachment", index))
        .h(px(CHIP_H))
        .pl(px(CHIP_PAD_L))
        .pr(px(CHIP_PAD_R))
        .gap(px(CHIP_GAP))
        .rounded(px(scale::R_MD))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .ui(scale::FS_12)
        .text_color(p.ink)
        .child(
            div()
                .flex_none()
                .size(px(CHIP_TILE))
                .rounded(px(scale::R_XS))
                .bg(tile_bg)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(glyph).size(px(CHIP_GLYPH)).color(tile_ink)),
        )
        .child(attachment.name.clone());
    if let Some(meta) = &attachment.meta {
        chip = chip.child(div().text_color(p.ink_3).child(meta.clone()));
    }
    chip
}

impl RenderOnce for UserTurn {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let acts_opacity = tween((id.clone(), "acts"), if flags.hovered { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);

        let mut acts = h_flex().absolute().left(px(USER_ACTS_LEFT)).top(px(USER_ACTS_TOP)).gap(px(ACTS_GAP)).opacity(acts_opacity);
        for (name, glyph, action) in [
            ("edit", IconName::Edit, UserTurnAction::Edit),
            ("copy", IconName::Copy, UserTurnAction::Copy),
            ("resend", IconName::Refresh, UserTurnAction::Resend),
        ] {
            let mut b = icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(ACTS_GLYPH));
            if let Some(h) = self.on_action.clone() {
                b = b.on_click(move |_, w, cx| h(action, w, cx));
            }
            acts = acts.child(b);
        }
        if acts_opacity <= 0.001 {
            acts = acts.invisible();
        }

        let mut col = v_flex()
            .id(id.clone())
            .relative()
            .max_w(relative(USER_MAX))
            .items_end()
            .gap(px(USER_GAP))
            .track_interaction(&state)
            .child(acts);
        if !self.attachments.is_empty() {
            let mut strip = h_flex().gap(px(USER_GAP));
            for (i, a) in self.attachments.iter().enumerate() {
                strip = strip.child(attachment_chip(&p, i, a));
            }
            col = col.child(strip);
        }
        col.child(
            div()
                .py(px(BUBBLE_PAD_Y))
                .px(px(BUBBLE_PAD_X))
                .rounded(px(BUBBLE_RADIUS))
                .rounded_br(px(scale::R_XS))
                .bg(p.surface_3)
                .ui(BUBBLE_TEXT)
                .text_color(p.ink)
                .child(prose((id, "text"), &self.markdown, prose_style(&p, BUBBLE_TEXT, scale::LH_UI, p.surface_3, p.accent_ink))),
        )
    }
}

/// The assistant's turn. Build with [`assistant_turn`].
#[derive(IntoElement)]
pub struct AssistantTurn {
    id: ElementId,
    markdown: SharedString,
    streaming: bool,
    meta: Option<TurnMeta>,
    on_action: Option<AssistantHandler>,
}

/// An assistant turn rendering `markdown`.
pub fn assistant_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> AssistantTurn {
    AssistantTurn { id: id.into(), markdown: markdown.into(), streaming: false, meta: None, on_action: None }
}

impl AssistantTurn {
    /// Shows the blinking caret after the text while chunks arrive.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// The footer: model · duration · tokens · cost.
    pub fn meta(mut self, meta: TurnMeta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Toolbar handler.
    pub fn on_action(mut self, f: impl Fn(AssistantTurnAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }
}

/// `2.4k tokens` / `$0.04` / `3.1 s` formatting for the footer.
fn footer_items(meta: &TurnMeta) -> Vec<String> {
    let tokens = meta.tokens_in + meta.tokens_out;
    let tokens = if tokens >= 1000 { format!("{:.1}k tokens", tokens as f64 / 1000.0) } else { format!("{tokens} tokens") };
    vec![meta.model.clone(), format!("{:.1} s", meta.duration_ms as f64 / 1000.0), tokens, format!("${:.2}", meta.cost_usd)]
}

impl RenderOnce for AssistantTurn {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let tb_opacity = tween((id.clone(), "tb-opacity"), if flags.hovered { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);
        let tb_rise = tween((id.clone(), "tb-rise"), if flags.hovered { 0.0f32 } else { TOOLBAR_RISE }, Tween::BASE.with_easing(aui_tokens::Easing::OUT), window, cx);

        let mut toolbar = h_flex()
            .absolute()
            .right(px(0.0))
            .top(px(TOOLBAR_TOP + tb_rise))
            .gap(px(ACTS_GAP))
            .p(px(TOOLBAR_PAD))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .opacity(tb_opacity);
        for (name, glyph, action) in [
            ("copy", IconName::Copy, AssistantTurnAction::Copy),
            ("retry", IconName::Refresh, AssistantTurnAction::Retry),
            ("fork", IconName::Git, AssistantTurnAction::Fork),
            ("pin", IconName::Pin, AssistantTurnAction::Pin),
        ] {
            let mut b = icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(ACTS_GLYPH));
            if let Some(h) = self.on_action.clone() {
                b = b.on_click(move |_, w, cx| h(action, w, cx));
            }
            toolbar = toolbar.child(b);
        }
        if tb_opacity <= 0.001 {
            toolbar = toolbar.invisible();
        }

        let mut turn = v_flex()
            .id(id.clone())
            .relative()
            .w_full()
            .ui(BODY_TEXT)
            .line_height(relative(scale::LH_BODY))
            .text_color(p.ink)
            .track_interaction(&state)
            .child(toolbar)
            .child(prose((id.clone(), "text"), &self.markdown, prose_style(&p, BODY_TEXT, scale::LH_BODY, p.surface_2, p.ink)));

        if self.streaming {
            // The caret cannot sit inline after the last glyph yet (gpui text
            // hosts no inline elements); the composer phase measures the last
            // line. Until then a streaming turn shows no caret.
            let _ = looping((id.clone(), "caret"), Loop::linear(CARET_PERIOD).resting(1.0), window, cx);
        }

        if let Some(meta) = &self.meta {
            let mut footer = h_flex().mt(px(FOOTER_TOP)).gap(px(FOOTER_GAP)).font_family(scale::FONT_MONO).text_px(scale::FS_11).line_height(relative(1.0)).medium().text_color(p.ink_4);
            for (i, item) in footer_items(meta).into_iter().enumerate() {
                if i > 0 {
                    footer = footer.child("·");
                }
                footer = footer.child(item);
            }
            turn = turn.child(footer);
        }
        turn
    }
}
