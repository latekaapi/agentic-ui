//! Card 36: the question card — a structured question with radio or checkbox
//! options, an optional free-text "Other" row and an action row, plus the
//! one-line answered state it collapses to once the person has replied.
//!
//! The card is stateless: it draws the selection it is given and emits the
//! person's intent through closures. The host keeps the selection, builds the
//! [`Answer`] it wants and, once the question is settled, renders
//! [`answered_row`] in the card's place.

use aui_motion::{spring_phase, tween, SpringKind, Tween};
use aui_protocol::{Answer, QuestionOption};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, chip, glyph_ok, kbd};
use crate::icons::{icon, IconName};
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// `.q .hd{padding:10px 12px;gap:10px}`.
const HEAD_PAD_Y: f32 = 10.0;
const HEAD_PAD_X: f32 = 12.0;
const HEAD_GAP: f32 = 10.0;
/// `.q .ic{width:26px;height:26px;border-radius:7px}` with a 14 px glyph.
const TILE: f32 = 26.0;
const TILE_RADIUS: f32 = 7.0;
const TILE_GLYPH: f32 = 14.0;
/// `.q .ttl{font-size:13px;font-weight:600}` and `.q .sub{font-size:12px}`.
const TITLE_TEXT: f32 = 13.0;
const SUB_TEXT: f32 = 12.0;
/// `.opt{padding:8px 10px;margin:0 8px 6px;gap:10px}`.
const OPT_PAD_Y: f32 = 8.0;
const OPT_PAD_X: f32 = 10.0;
const OPT_MARGIN_X: f32 = 8.0;
const OPT_MARGIN_BOTTOM: f32 = 6.0;
const OPT_GAP: f32 = 10.0;
/// `.opt .rb{width:16px;height:16px;border:1.5px;margin-top:1px}`.
const MARK: f32 = 16.0;
const MARK_BORDER: f32 = 1.5;
const MARK_OFFSET: f32 = 1.0;
/// `.opt.on .rb::after{width:8px;height:8px}` — the dot, which pops in from
/// 40 % on the swap spring (gpui has no transform, so the size is animated).
const DOT: f32 = 8.0;
const DOT_FROM: f32 = 0.4;
/// `.opt .cb{border-radius:4px}` and `.opt.on .cb::after{border-radius:1px}`.
const CHECKBOX_RADIUS: f32 = 4.0;
const CHECKBOX_DOT_RADIUS: f32 = 1.0;
/// `.opt b{font-size:12.5px;font-weight:500}` and `.opt span{font-size:12px}`.
const LABEL_TEXT: f32 = 12.5;
const DESCRIPTION_TEXT: f32 = 12.0;
/// The description is an inline `span`, so in CSS its line box is the row's
/// inherited 13 px strut rather than its own 12 px one.
const DESCRIPTION_LINE: f32 = scale::FS_13 * scale::LH_UI;
/// `.other{margin:0 8px 8px;padding:6px 10px;gap:8px;font-size:12px}` with a
/// 12 px glyph and a dashed 1 px line-strong border.
const OTHER_MARGIN_BOTTOM: f32 = 8.0;
const OTHER_PAD_Y: f32 = 6.0;
const OTHER_PAD_X: f32 = 10.0;
const OTHER_GAP: f32 = 8.0;
const OTHER_TEXT: f32 = 12.0;
const OTHER_GLYPH: f32 = 12.0;
/// `.q .ft{padding:8px 12px;gap:8px}` with an 11 px hint.
const FOOT_PAD_Y: f32 = 8.0;
const FOOT_PAD_X: f32 = 12.0;
const FOOT_GAP: f32 = 8.0;
const HINT_TEXT: f32 = 11.0;
/// `.answered{padding:8px 12px;gap:8px;font-size:12.5px}`.
const ANSWERED_PAD_Y: f32 = 8.0;
const ANSWERED_PAD_X: f32 = 12.0;
const ANSWERED_GAP: f32 = 8.0;
const ANSWERED_TEXT: f32 = 12.5;

/// A closure fed the index of the option the person clicked. Shared, because
/// every option row wires the same handler.
type SelectHandler = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;
/// A closure fed the [`Answer`] the person confirmed.
type AnswerHandler = Box<dyn Fn(Answer, &mut Window, &mut App) + 'static>;

/// The pending question card. Build with [`question_card`].
#[derive(IntoElement)]
pub struct QuestionCard {
    id: ElementId,
    prompt: SharedString,
    subtitle: SharedString,
    options: Vec<QuestionOption>,
    multi: bool,
    allow_other: bool,
    selected: Vec<usize>,
    hint: Option<SharedString>,
    on_select: Option<SelectHandler>,
    on_other: Option<ClickHandler>,
    on_answer: Option<AnswerHandler>,
    on_skip: Option<ClickHandler>,
}

/// A question with `prompt` and `options`, drawn as radios; call
/// [`QuestionCard::multi`] for checkboxes.
pub fn question_card(id: impl Into<ElementId>, prompt: impl Into<SharedString>, options: Vec<QuestionOption>) -> QuestionCard {
    QuestionCard {
        id: id.into(),
        prompt: prompt.into(),
        subtitle: SharedString::default(),
        options,
        multi: false,
        allow_other: false,
        selected: Vec::new(),
        hint: None,
        on_select: None,
        on_other: None,
        on_answer: None,
        on_skip: None,
    }
}

impl QuestionCard {
    /// The supporting line under the prompt.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// Checkboxes instead of radios (`Block::Question::multi`).
    pub fn multi(mut self, multi: bool) -> Self {
        self.multi = multi;
        self
    }

    /// Whether the dashed "Other, type your own…" row is offered.
    pub fn allow_other(mut self, allow: bool) -> Self {
        self.allow_other = allow;
        self
    }

    /// Which options are currently selected, as indices into `options`.
    pub fn selected(mut self, selected: Vec<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Overrides the action-row hint (the default counts the selection).
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// An option row was clicked; the host toggles or replaces the selection.
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }

    /// The "Other" row was clicked; the host opens a free-text field.
    pub fn on_other(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_other = Some(Box::new(f));
        self
    }

    /// "Continue": the current selection, as an [`Answer`].
    pub fn on_answer(mut self, f: impl Fn(Answer, &mut Window, &mut App) + 'static) -> Self {
        self.on_answer = Some(Box::new(f));
        self
    }

    /// "Skip": the person declined to answer.
    pub fn on_skip(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_skip = Some(Box::new(f));
        self
    }
}

/// The 16 px radio or checkbox, with its dot popping in on the swap spring.
fn option_mark(id: ElementId, on: bool, multi: bool, p: &Palette, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let phase = spring_phase(id, on, SpringKind::Swap, window, cx).clamp(0.0, 1.0);
    let dot = DOT * (DOT_FROM + (1.0 - DOT_FROM) * phase);
    let radius = if multi { px(CHECKBOX_RADIUS) } else { px(scale::R_FULL) };
    let dot_radius = if multi { px(CHECKBOX_DOT_RADIUS) } else { px(scale::R_FULL) };
    div()
        .flex_none()
        .mt(px(MARK_OFFSET))
        .size(px(MARK))
        .rounded(radius)
        .border(px(MARK_BORDER))
        .border_color(if on { p.accent } else { p.line_strong })
        .flex()
        .items_center()
        .justify_center()
        .when(phase > 0.0, |d| d.child(div().size(px(dot)).rounded(dot_radius).bg(p.accent.alpha(phase))))
}

impl RenderOnce for QuestionCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        let head = h_flex()
            .w_full()
            .items_center()
            .gap(px(HEAD_GAP))
            .py(px(HEAD_PAD_Y))
            .px(px(HEAD_PAD_X))
            .child(
                div()
                    .flex_none()
                    .size(px(TILE))
                    .rounded(px(TILE_RADIUS))
                    .bg(p.accent_soft)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(IconName::Question).size(px(TILE_GLYPH)).color(p.accent_ink)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().ui(TITLE_TEXT).semibold().text_color(p.ink).child(self.prompt.clone()))
                    .when(!self.subtitle.is_empty(), |d| d.child(div().ui(SUB_TEXT).text_color(p.ink_2).child(self.subtitle.clone()))),
            );

        let mut card = v_flex()
            .id(id.clone())
            .w_full()
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.attention_border(p.accent))
            .bg(p.surface_1)
            .overflow_hidden()
            .child(head);

        for (index, option) in self.options.iter().enumerate() {
            let on = self.selected.contains(&index);
            let row_id: ElementId = (id.clone(), SharedString::from(format!("opt-{index}"))).into();
            let (state, flags) = interaction_flags(row_id.clone(), window, cx);
            // `.opt:hover{background:surface-2}` and `.opt.on{border-color:accent;background:surface-2}`.
            let tint = on || flags.hovered;
            let bg: Hsla = tween((row_id.clone(), "bg"), if tint { p.surface_2 } else { p.surface_1 }, Tween::FAST, window, cx);
            let border: Hsla = tween((row_id.clone(), "border"), if on { p.accent } else { p.line }, Tween::FAST, window, cx);
            let mut row = h_flex()
                .id(row_id.clone())
                .items_start()
                .gap(px(OPT_GAP))
                .py(px(OPT_PAD_Y))
                .px(px(OPT_PAD_X))
                .mx(px(OPT_MARGIN_X))
                .mb(px(OPT_MARGIN_BOTTOM))
                .rounded(px(scale::R_MD))
                .border_1()
                .border_color(border)
                .bg(bg)
                .cursor_pointer()
                .track_interaction(&state)
                .child(option_mark((row_id.clone(), "mark").into(), on, self.multi, &p, window, cx))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(div().ui(LABEL_TEXT).medium().text_color(p.ink).child(SharedString::from(option.label.clone())))
                        .child(div().text_px(DESCRIPTION_TEXT).font_family(scale::FONT_UI).line_height(aui_tokens::scaled(DESCRIPTION_LINE)).text_color(p.ink_2).child(SharedString::from(option.description.clone()))),
                );
            if !option.key.is_empty() {
                row = row.child(kbd(option.key.clone()));
            }
            if let Some(on_select) = self.on_select.clone() {
                row = row.on_click(move |_, w, cx| on_select(index, w, cx));
            }
            card = card.child(row);
        }

        if self.allow_other {
            let other_id: ElementId = (id.clone(), "other").into();
            let mut other = h_flex()
                .id(other_id)
                .items_center()
                .gap(px(OTHER_GAP))
                .py(px(OTHER_PAD_Y))
                .px(px(OTHER_PAD_X))
                .mx(px(OPT_MARGIN_X))
                .mb(px(OTHER_MARGIN_BOTTOM))
                .rounded(px(scale::R_MD))
                .border_dashed()
                .border_1()
                .border_color(p.line_strong)
                .ui(OTHER_TEXT)
                .text_color(p.ink_3)
                .cursor_pointer()
                .child(icon(IconName::Edit).size(px(OTHER_GLYPH)).color(p.ink_3))
                .child("Other, type your own…");
            if let Some(on_other) = self.on_other {
                other = other.on_click(move |e, w, cx| on_other(e, w, cx));
            }
            card = card.child(other);
        }

        let count = self.selected.len();
        let hint = self.hint.clone().unwrap_or_else(|| SharedString::from(format!("{count} selected")));
        let answer = Answer { selected: self.selected.clone(), other: None };
        let mut skip = button((id.clone(), "skip"), "Skip").sm().ghost();
        if let Some(on_skip) = self.on_skip {
            skip = skip.on_click(move |e, w, cx| on_skip(e, w, cx));
        }
        let mut confirm = button((id.clone(), "continue"), "Continue").sm().primary();
        if let Some(on_answer) = self.on_answer {
            confirm = confirm.on_click(move |_, w, cx| on_answer(answer.clone(), w, cx));
        }
        card = card.child(
            h_flex()
                .w_full()
                .items_center()
                .gap(px(FOOT_GAP))
                .py(px(FOOT_PAD_Y))
                .px(px(FOOT_PAD_X))
                .border_t_1()
                .border_color(p.line)
                .bg(p.surface_2)
                .child(div().flex_none().ui(HINT_TEXT).text_color(p.ink_3).child(hint))
                .child(div().flex_1())
                .child(skip)
                .child(confirm),
        );
        card
    }
}

/// The answered state of a question. Build with [`answered_row`].
#[derive(IntoElement)]
pub struct AnsweredRow {
    id: ElementId,
    chips: Vec<SharedString>,
    on_change: Option<ClickHandler>,
}

/// The one-line card a settled question collapses to: a success glyph, the
/// chosen answers as chips and a "Change" action. `chips` are the short
/// labels of the chosen options.
pub fn answered_row(id: impl Into<ElementId>, chips: Vec<SharedString>) -> AnsweredRow {
    AnsweredRow { id: id.into(), chips, on_change: None }
}

impl AnsweredRow {
    /// "Change": the person wants to answer again.
    pub fn on_change(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}

impl RenderOnce for AnsweredRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut change = button((id.clone(), "change"), "Change").xs().ghost();
        if let Some(on_change) = self.on_change {
            change = change.on_click(move |e, w, cx| on_change(e, w, cx));
        }
        h_flex()
            .id(id.clone())
            .w_full()
            .items_center()
            .gap(px(ANSWERED_GAP))
            .py(px(ANSWERED_PAD_Y))
            .px(px(ANSWERED_PAD_X))
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .ui(ANSWERED_TEXT)
            .text_color(p.ink)
            .child(glyph_ok())
            .child(div().flex_none().text_color(p.ink_3).child("Answered:"))
            .children(self.chips.into_iter().enumerate().map(|(i, label)| chip((id.clone(), SharedString::from(format!("chip-{i}"))), label)))
            .child(div().flex_1())
            .child(change)
    }
}
