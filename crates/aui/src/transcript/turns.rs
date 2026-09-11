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
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::sync::{LazyLock, Mutex};

use crate::transcript::{caret_top_in_line, caret_visible, ProseStyle, CARET_H, CARET_MARGIN_LEFT, CARET_W};
use crate::transcript::{LinkTarget, SelectionHandler, TextSelection, last_block_runs, markdown, markdown_selected_text, LinkHandler};
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
/// `.a .ab{margin-top:8px;gap:2px}`: the in-flow action row under the prose.
/// Muted but visible at rest, full strength on turn hover.
const BOTTOM_TOP: f32 = 8.0;
const BOTTOM_IDLE: f32 = 0.55;


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

impl UserTurnAction {
    /// The full set, in draw order: edit, copy, resend.
    pub const ALL: &'static [UserTurnAction] =
        &[UserTurnAction::Edit, UserTurnAction::Copy, UserTurnAction::Resend];
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

impl AssistantTurnAction {
    /// The full set, in draw order: copy, retry, fork, pin.
    pub const ALL: &'static [AssistantTurnAction] = &[
        AssistantTurnAction::Copy,
        AssistantTurnAction::Retry,
        AssistantTurnAction::Fork,
        AssistantTurnAction::Pin,
    ];
}

/// Element id, glyph and action for one user-turn button, in draw order.
fn user_action_spec(action: UserTurnAction) -> (&'static str, IconName, UserTurnAction) {
    match action {
        UserTurnAction::Edit => ("edit", IconName::Edit, action),
        UserTurnAction::Copy => ("copy", IconName::Copy, action),
        UserTurnAction::Resend => ("resend", IconName::Refresh, action),
    }
}

/// Element id, glyph and action for one assistant-turn button, in draw order.
fn assistant_action_spec(action: AssistantTurnAction) -> (&'static str, IconName, AssistantTurnAction) {
    match action {
        AssistantTurnAction::Copy => ("copy", IconName::Copy, action),
        AssistantTurnAction::Retry => ("retry", IconName::Refresh, action),
        AssistantTurnAction::Fork => ("fork", IconName::Git, action),
        AssistantTurnAction::Pin => ("pin", IconName::Pin, action),
    }
}

type UserHandler = std::rc::Rc<dyn Fn(UserTurnAction, &mut Window, &mut App)>;
type AssistantHandler = std::rc::Rc<dyn Fn(AssistantTurnAction, &mut Window, &mut App)>;

/// The person's turn. Build with [`user_turn`].
#[derive(IntoElement)]
pub struct UserTurn {
    id: ElementId,
    markdown: SharedString,
    attachments: Vec<Attachment>,
    actions: Vec<UserTurnAction>,
    actions_bottom: bool,
    on_action: Option<UserHandler>,
    on_link: Option<LinkHandler>,
    selection: Option<TextSelection>,
    on_selection_change: Option<SelectionHandler>,
}

/// A user turn; `markdown` may carry mentions as inline code (`` `@src/checkout` ``),
/// which render as mention chips.
pub fn user_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> UserTurn {
    UserTurn { id: id.into(), markdown: markdown.into(), attachments: Vec::new(), actions: UserTurnAction::ALL.to_vec(), actions_bottom: false, on_action: None, on_link: None, selection: None, on_selection_change: None }
}

impl UserTurn {
    /// Attachments shown above the bubble.
    pub fn attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// The action buttons, in draw order. Defaults to [`UserTurnAction::ALL`];
    /// pass a smaller slice (or an empty one) to hide actions that have no
    /// meaning for the consumer. Both the hover rail and the
    /// [`UserTurn::actions_bottom`] row honour it; an empty set draws no rail
    /// and no row.
    pub fn actions(mut self, actions: &[UserTurnAction]) -> Self {
        self.actions = actions.to_vec();
        self
    }

    /// In-flow action row under the bubble instead of the hover rail.
    ///
    /// The row stays visible at a muted opacity and goes full strength on
    /// turn hover; the hover rail is off while it is on. Default `false`
    /// keeps the hover rail.
    pub fn actions_bottom(mut self, bottom: bool) -> Self {
        self.actions_bottom = bottom;
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

    /// The stored selection the markdown body highlights: the app owns one
    /// [`Option<TextSelection>`] per turn and passes it back here, passed
    /// straight through to the inner `markdown(...)`.
    pub fn selection(mut self, selection: Option<TextSelection>) -> Self {
        self.selection = selection;
        self
    }

    /// Selection intents, passed straight through to the inner
    /// `markdown(...)`: drags and word / paragraph picks arrive as `Some`,
    /// plain clicks elsewhere in a cell arrive as `None` (clearing).
    pub fn on_selection_change(mut self, f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static) -> Self {
        self.on_selection_change = Some(std::rc::Rc::new(f));
        self
    }
}

/// Copies the selected text out of a turn's `markdown_source` without
/// re-rendering: the slice of the holding cell's shaped text, or `None` when
/// the key addresses no cell or the range is empty. The app puts this on the
/// clipboard on ⌘C; the keybinding stays with the app.
pub fn turn_selected_text(markdown_source: &str, selection: &TextSelection) -> Option<String> {
    markdown_selected_text(markdown_source, selection)
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
        let bottom = self.actions_bottom;
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let acts_opacity = tween((id.clone(), "acts"), if flags.hovered { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);

        let mut col = v_flex()
            .id(id.clone())
            .relative()
            .max_w(relative(USER_MAX))
            .items_end()
            .gap(px(USER_GAP))
            .track_interaction(&state);
        if !bottom && !self.actions.is_empty() {
            let mut acts = h_flex().absolute().left(px(USER_ACTS_LEFT)).top(px(USER_ACTS_TOP)).gap(px(ACTS_GAP)).opacity(acts_opacity);
            for (name, glyph, action) in self.actions.iter().map(|a| user_action_spec(*a)) {
                let mut b = icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(ACTS_GLYPH));
                if let Some(h) = self.on_action.clone() {
                    b = b.on_click(move |_, w, cx| h(action, w, cx));
                }
                acts = acts.child(b);
            }
            if acts_opacity <= 0.001 {
                acts = acts.invisible();
            }
            col = col.child(acts);
        }
        if !self.attachments.is_empty() {
            let mut strip = h_flex().gap(px(USER_GAP));
            for (i, a) in self.attachments.iter().enumerate() {
                strip = strip.child(attachment_chip(&p, i, a));
            }
            col = col.child(strip);
        }
        col = col.child(
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
                    body = body.selection(self.selection.as_ref());
                    if let Some(on_change) = self.on_selection_change.clone() {
                        body = body.on_selection_change(move |next, window, cx| on_change(next, window, cx));
                    }
                    body
                }),
        );
        if bottom && !self.actions.is_empty() {
            let row_opacity = tween((id.clone(), "acts-bottom"), if flags.hovered { 1.0f32 } else { BOTTOM_IDLE }, Tween::FAST, window, cx);
            let mut row = h_flex().gap(px(ACTS_GAP)).opacity(row_opacity);
            for (name, glyph, action) in self.actions.iter().map(|a| user_action_spec(*a)) {
                let mut b = icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(ACTS_GLYPH));
                if let Some(h) = self.on_action.clone() {
                    b = b.on_click(move |_, w, cx| h(action, w, cx));
                }
                row = row.child(b);
            }
            col = col.child(row);
        }
        col
    }
}

/// The assistant's turn. Build with [`assistant_turn`].
#[derive(IntoElement)]
pub struct AssistantTurn {
    id: ElementId,
    markdown: SharedString,
    streaming: bool,
    meta: Option<TurnMeta>,
    actions: Vec<AssistantTurnAction>,
    actions_bottom: bool,
    on_action: Option<AssistantHandler>,
    on_link: Option<LinkHandler>,
    selection: Option<TextSelection>,
    on_selection_change: Option<SelectionHandler>,
}

/// An assistant turn rendering `markdown`.
pub fn assistant_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> AssistantTurn {
    AssistantTurn { id: id.into(), markdown: markdown.into(), streaming: false, meta: None, actions: AssistantTurnAction::ALL.to_vec(), actions_bottom: false, on_action: None, on_link: None, selection: None, on_selection_change: None }
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

    /// The toolbar buttons, in draw order. Defaults to
    /// [`AssistantTurnAction::ALL`]; pass a smaller slice to hide actions
    /// that have no meaning for the consumer (a turn without pinning keeps
    /// `&[Copy, Retry, Fork]`). Both the hover toolbar and the
    /// [`AssistantTurn::actions_bottom`] row honour it; an empty set draws no
    /// toolbar and no row.
    pub fn actions(mut self, actions: &[AssistantTurnAction]) -> Self {
        self.actions = actions.to_vec();
        self
    }

    /// In-flow action row under the prose instead of the hover toolbar.
    ///
    /// The row stays visible at a muted opacity and goes full strength on
    /// turn hover; the hover toolbar is off while it is on. Default `false`
    /// keeps the hover toolbar.
    pub fn actions_bottom(mut self, bottom: bool) -> Self {
        self.actions_bottom = bottom;
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

    /// The stored selection the markdown body highlights: the app owns one
    /// [`Option<TextSelection>`] per turn and passes it back here, passed
    /// straight through to the inner `markdown(...)`.
    pub fn selection(mut self, selection: Option<TextSelection>) -> Self {
        self.selection = selection;
        self
    }

    /// Selection intents, passed straight through to the inner
    /// `markdown(...)`: drags and word / paragraph picks arrive as `Some`,
    /// plain clicks elsewhere in a cell arrive as `None` (clearing).
    pub fn on_selection_change(mut self, f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static) -> Self {
        self.on_selection_change = Some(std::rc::Rc::new(f));
        self
    }
}

/// How many measured trailing widths [`last_line_width`] keeps. One entry per
/// streaming turn is the realistic load (only the closing block of a turn
/// that is still streaming is measured); the bound leaves room for a
/// transcript that has several in flight and for the last few shapes of each.
const CARET_MEASURE_CAP: usize = 16;

/// Measured trailing widths, keyed by what the shape depends on: the text,
/// the font size, the wrap width and the shaping-relevant parts of the runs.
/// Colour is deliberately not in the key — it does not move a glyph.
static CARET_WIDTHS: LazyLock<Mutex<Vec<CaretMeasure>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// One cached measurement: the key and the trailing width it shaped to.
type CaretMeasure = (u64, Option<Pixels>);

/// The key for one measurement. Fonts are compared by family, weight and
/// style; a run's colour, underline and strikethrough do not affect shaping.
fn caret_key(text: &str, font_size: Pixels, runs: &[TextRun], wrap_width: Pixels) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    f32::from(font_size).to_bits().hash(&mut hasher);
    f32::from(wrap_width).to_bits().hash(&mut hasher);
    for run in runs {
        run.len.hash(&mut hasher);
        run.font.family.hash(&mut hasher);
        run.font.weight.0.to_bits().hash(&mut hasher);
        matches!(run.font.style, gpui::FontStyle::Italic).hash(&mut hasher);
    }
    hasher.finish()
}

/// The width of the last *visual* line of `text` once it is wrapped at
/// `wrap_width`: the whole line when it never wrapped, otherwise the shaped
/// width from the last wrap boundary to the end.
///
/// Memoised: a streaming turn re-renders at frame rate but only grows its
/// text when a chunk lands, so without a cache this shapes the closing block
/// again for every frame in between. The cache holds
/// [`CARET_MEASURE_CAP`] entries, most recent last.
fn last_line_width(window: &Window, text: SharedString, font_size: Pixels, runs: &[TextRun], wrap_width: Pixels) -> Option<Pixels> {
    let key = caret_key(&text, font_size, runs, wrap_width);
    if let Ok(cache) = CARET_WIDTHS.lock() {
        if let Some((_, width)) = cache.iter().rev().find(|(cached, _)| *cached == key) {
            return *width;
        }
    }
    let width = shape_last_line_width(window, text, font_size, runs, wrap_width);
    if let Ok(mut cache) = CARET_WIDTHS.lock() {
        if cache.len() >= CARET_MEASURE_CAP {
            cache.remove(0);
        }
        cache.push((key, width));
    }
    width
}

/// [`last_line_width`] without the memo: the shaping pass itself.
fn shape_last_line_width(window: &Window, text: SharedString, font_size: Pixels, runs: &[TextRun], wrap_width: Pixels) -> Option<Pixels> {
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
pub(super) fn footer_items(meta: &TurnMeta) -> Vec<String> {
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
        let bottom = self.actions_bottom;
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
        for (name, glyph, action) in self.actions.iter().map(|a| assistant_action_spec(*a)) {
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
            .track_interaction(&state);
        if !bottom && !self.actions.is_empty() {
            turn = turn.child(toolbar);
        }
        turn = turn.child(
            div()
                .relative()
                .w_full()
                .child(div().w_full().on_prepaint(move |b, _, _| *bounds.borrow_mut() = Some(b)).child({
                    let mut body = markdown((id.clone(), "text"), self.markdown.clone(), style);
                    if let Some(on_link) = self.on_link.clone() {
                        body = body.on_link(move |target, window, cx| on_link(target, window, cx));
                    }
                    body = body.selection(self.selection.as_ref());
                    if let Some(on_change) = self.on_selection_change.clone() {
                        body = body.on_selection_change(move |next, window, cx| on_change(next, window, cx));
                    }
                    body
                }))
                .children(caret),
        );
        if bottom && !self.actions.is_empty() {
            let row_opacity = tween((id.clone(), "tb-bottom"), if flags.hovered { 1.0f32 } else { BOTTOM_IDLE }, Tween::FAST, window, cx);
            let mut row = h_flex().mt(px(BOTTOM_TOP)).gap(px(ACTS_GAP)).opacity(row_opacity);
            for (name, glyph, action) in self.actions.iter().map(|a| assistant_action_spec(*a)) {
                let mut b = icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(ACTS_GLYPH));
                if let Some(h) = self.on_action.clone() {
                    b = b.on_click(move |_, w, cx| h(action, w, cx));
                }
                row = row.child(b);
            }
            turn = turn.child(row);
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

#[cfg(test)]
mod tests {
    use super::{assistant_turn, turn_selected_text, user_turn, AssistantTurnAction, UserTurnAction};
    use crate::transcript::{SelectionKey, TextSelection};

    #[test]
    fn user_actions_default_to_all() {
        assert_eq!(user_turn("t", "hi").actions, UserTurnAction::ALL.to_vec());
    }

    #[test]
    fn user_actions_keeps_a_reduced_set() {
        let turn = user_turn("t", "hi").actions(&[UserTurnAction::Copy]);
        assert_eq!(turn.actions, vec![UserTurnAction::Copy]);
    }

    #[test]
    fn assistant_actions_default_to_all() {
        assert_eq!(
            assistant_turn("t", "hi").actions,
            AssistantTurnAction::ALL.to_vec()
        );
        assert_eq!(AssistantTurnAction::ALL.len(), 4);
    }

    #[test]
    fn assistant_actions_can_hide_pin() {
        let turn = assistant_turn("t", "hi").actions(&[
            AssistantTurnAction::Copy,
            AssistantTurnAction::Retry,
            AssistantTurnAction::Fork,
        ]);
        assert!(!turn.actions.contains(&AssistantTurnAction::Pin));
        assert_eq!(turn.actions.len(), 3);
    }

    #[test]
    fn turn_selected_text_reads_a_paragraph_slice() {
        // Code spans read as plain words in the shaped text.
        let selection = TextSelection {
            cell: SelectionKey::paragraph("", 0),
            range: 0..7,
        };
        assert_eq!(
            turn_selected_text("Tighten `validateAddress` now", &selection).as_deref(),
            Some("Tighten")
        );
    }

    #[test]
    fn turn_selected_text_rejects_unknown_cells() {
        let selection = TextSelection {
            cell: SelectionKey::paragraph("", 9),
            range: 0..7,
        };
        assert_eq!(turn_selected_text("Hello", &selection), None);
    }
}
