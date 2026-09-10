//! Card 31: the user bubble (with attachments and mentions, hover actions)
//! and the assistant turn (full-width markdown, hover toolbar, footer meta,
//! streaming caret).

use aui_icons::{icon, IconName};
use aui_motion::{tween, Tween};
use aui_protocol::{Attachment, AttachmentKind, TurnMeta};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, relative, App, Bounds, ElementId, IntoElement, Pixels, SharedString, TextRun, Window};
use gpui_kit::base::ElementExt;
use std::cell::RefCell;
use std::rc::Rc;

use crate::transcript::{caret_top_in_line, caret_visible, ProseStyle, CARET_H, CARET_MARGIN_LEFT, CARET_W};
use crate::transcript::{LinkTarget, last_block_runs, markdown, LinkHandler};
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
    on_link: Option<LinkHandler>,
}

/// A user turn; `markdown` may carry mentions as inline code (`` `@src/checkout` ``),
/// which render as mention chips.
pub fn user_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> UserTurn {
    UserTurn { id: id.into(), markdown: markdown.into(), attachments: Vec::new(), on_action: None, on_link: None }
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

    /// Link-click handler, passed through to the markdown body.
    pub fn on_link(mut self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(std::rc::Rc::new(f));
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
                .child({
                    let mut body = markdown((id.clone(), "text"), self.markdown.clone(), prose_style(&p, BUBBLE_TEXT, scale::LH_UI, p.surface_3, p.accent_ink));
                    if let Some(on_link) = self.on_link.clone() {
                        body = body.on_link(move |target, window, cx| on_link(target, window, cx));
                    }
                    body
                }),
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
    on_link: Option<LinkHandler>,
}

/// An assistant turn rendering `markdown`.
pub fn assistant_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> AssistantTurn {
    AssistantTurn { id: id.into(), markdown: markdown.into(), streaming: false, meta: None, on_action: None, on_link: None }
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

    /// Link-click handler, passed through to the markdown body.
    pub fn on_link(mut self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(std::rc::Rc::new(f));
        self
    }
}

/// The width of the last *visual* line of `text` once it is wrapped at
/// `wrap_width`: the whole line when it never wrapped, otherwise the shaped
/// width from the last wrap boundary to the end.
fn last_line_width(window: &Window, text: SharedString, font_size: Pixels, runs: &[TextRun], wrap_width: Pixels) -> Option<Pixels> {
    let lines = window.text_system().shape_text(text, font_size, runs, Some(wrap_width), None).ok()?;
    let line = lines.last()?;
    let layout = &line.unwrapped_layout;
    match line.wrap_boundaries.last() {
        None => Some(layout.width),
        Some(boundary) => {
            let index = layout.runs.get(boundary.run_ix)?.glyphs.get(boundary.glyph_ix)?.index;
            Some(layout.width - layout.x_for_index(index))
        }
    }
}

/// `2.4k tokens` / `$0.04` / `3.1 s` formatting for the footer.
///
/// Reasoning tokens get their own cell and only when there are any: a provider
/// can bill a reasoning budget and emit no reasoning item at all, so the number
/// is the one place a person can see that thinking happened, and a `0` on every
/// turn that did none would be noise.
fn footer_items(meta: &TurnMeta) -> Vec<String> {
    let tokens = meta.tokens_in + meta.tokens_out;
    let tokens = if tokens >= 1000 { format!("{:.1}k tokens", tokens as f64 / 1000.0) } else { format!("{tokens} tokens") };
    let mut items = vec![meta.model.clone(), format!("{:.1} s", meta.duration_ms as f64 / 1000.0), tokens];
    // A provider that reports no model label leaves an empty cell, and an empty
    // cell renders as a stray separator; drop it rather than draw it.
    items.retain(|item| !item.is_empty());
    if meta.reasoning_tokens > 0 {
        items.push(format!("{} reasoning", meta.reasoning_tokens));
    }
    // A catalog that reports no price (MSP's `cost` is `null` on every row of
    // a subscription catalog) leaves the cost at zero; `$0.00` under every turn
    // is a number the app cannot stand behind, so it is not drawn at all.
    if meta.cost_usd > 0.0 {
        items.push(format!("${:.2}", meta.cost_usd));
    }
    items
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

        let style = prose_style(&p, BODY_TEXT, scale::LH_BODY, p.surface_2, p.ink);
        // gpui text hosts no inline elements, so the caret is an absolutely
        // positioned sibling of the prose: last frame's prose bounds give the
        // wrap width, the last paragraph is re-shaped with the same runs the
        // prose paints, and the caret lands after the final visual line.
        let bounds: Rc<RefCell<Option<Bounds<Pixels>>>> =
            window.use_keyed_state((id.clone(), "caret-bounds"), cx, |_, _| Rc::new(RefCell::new(None))).read(cx).clone();
        let scale_factor = cx.aui().text_scale;
        let caret = if self.streaming {
            let visible = caret_visible((id.clone(), "caret"), window, cx);
            let measured = *bounds.borrow();
            match measured {
                Some(b) if b.size.width > px(0.0) => {
                    let font_size = window.rem_size() * (BODY_TEXT / scale::FS_13);
                    let line_height = font_size * scale::LH_BODY;
                    last_block_runs(&self.markdown, &style, p.accent)
                        .and_then(|(text, runs)| last_line_width(window, text.into(), font_size, &runs, b.size.width))
                        .map(|x| {
                            let height = px(CARET_H * scale_factor);
                            let top = b.size.height - line_height + caret_top_in_line(line_height, height, scale_factor);
                            div()
                                .absolute()
                                .left(x + px(CARET_MARGIN_LEFT * scale_factor))
                                .top(top)
                                .w(px(CARET_W * scale_factor))
                                .h(height)
                                .bg(p.accent)
                                .opacity(if visible { 1.0 } else { 0.0 })
                        })
                }
                _ => {
                    // Nothing measured yet: draw no caret and ask for the frame
                    // that will have the bounds.
                    window.request_animation_frame();
                    None
                }
            }
        } else {
            None
        };

        let mut turn = v_flex()
            .id(id.clone())
            .relative()
            .w_full()
            .ui(BODY_TEXT)
            .line_height(relative(scale::LH_BODY))
            .text_color(p.ink)
            .track_interaction(&state)
            .child(toolbar)
            .child(
                div()
                    .relative()
                    .w_full()
                    .child(div().w_full().on_prepaint(move |b, _, _| *bounds.borrow_mut() = Some(b)).child({
                        let mut body = markdown((id.clone(), "text"), self.markdown.clone(), style);
                        if let Some(on_link) = self.on_link.clone() {
                            body = body.on_link(move |target, window, cx| on_link(target, window, cx));
                        }
                        body
                    }))
                    .children(caret),
            );

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
