//! The question card (D4, docs/41 §2.6): a structured question with radio
//! or checkbox options, an optional "Other" input and a footer of only the
//! provider's choices — or, for several questions, one card stepping through
//! them as a wizard — plus the quiet row it collapses to once answered.
//!
//! The card is stateless: it draws the selection it is given and emits the
//! person's intent through closures. The host keeps the selection, builds the
//! [`Answer`] it wants and, once the question is settled, renders
//! [`answered_row`] in the card's place.

use aui_motion::{spring_phase, tween, SpringKind, Tween};
use aui_protocol::{Answer, QuestionOption, QuestionPreview};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::record_ax_label;
use crate::icons::{icon, IconName};
use crate::transcript::controls::{control, control_with, key_hints, ControlKind};
use crate::transcript::quiet_row::{quiet_status_row, status_check, status_cross};
use crate::transcript::ProseStyle;
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// `.q .hd{padding:10px 12px;gap:10px}`.
const HEAD_PAD_Y: f32 = 10.0;
const HEAD_PAD_X: f32 = 12.0;
const HEAD_GAP: f32 = 10.0;
/// `.q .ic{width:26px;height:26px;border-radius:7px}` with a 14 px glyph.
const TILE: f32 = 26.0;
const TILE_RADIUS: f32 = 7.0;
const TILE_GLYPH: f32 = 14.0;
/// Title 600 at body size; the subtitle and the question's header in caption.
const TITLE_TEXT: f32 = 13.0;
const SUB_TEXT: f32 = 12.0;
/// Options: rows 8 / 10 in an 8 px inset, radius 6, gap 10.
const OPT_PAD_Y: f32 = 8.0;
const OPT_PAD_X: f32 = 10.0;
const OPT_MARGIN_X: f32 = 8.0;
const OPT_GAP: f32 = 10.0;
const OPT_RADIUS: f32 = 6.0;
/// FF's fluid hover glide on option rows.
const ROW_GLIDE: Tween = Tween::new(std::time::Duration::from_millis(80), aui_tokens::Easing::OUT);
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
/// The label at body size (600 when picked), the description in caption.
const LABEL_TEXT: f32 = 13.0;
const DESCRIPTION_TEXT: f32 = 12.0;
/// "Other": a 32 px Well input, margins 0 / 8 / 8, the edit glyph.
const OTHER_MARGIN_BOTTOM: f32 = 8.0;
const OTHER_H: f32 = 32.0;
const OTHER_PAD_X: f32 = 10.0;
const OTHER_GAP: f32 = 8.0;
const OTHER_TEXT: f32 = 12.0;
const OTHER_GLYPH: f32 = 12.0;
/// `.q .ft{padding:8px 12px;gap:8px}` with an 11 px hint.
const FOOT_PAD_Y: f32 = 8.0;
const FOOT_PAD_X: f32 = 12.0;
const FOOT_GAP: f32 = 8.0;
/// The "Preview" chevron under an option row, and the body it opens.
const PREVIEW_GAP: f32 = 5.0;
const PREVIEW_GLYPH: f32 = 12.0;
/// A mono preview body sits on the terminal ground, like a tool card's output.
const PREVIEW_PAD_Y: f32 = 8.0;
const PREVIEW_PAD_X: f32 = 10.0;
const PREVIEW_LINE: f32 = scale::LH_MONO;

/// A closure fed the index of the option the person clicked. Shared, because
/// every option row wires the same handler.
type SelectHandler = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;
/// A closure fed the [`Answer`] the person confirmed.
type AnswerHandler = Box<dyn Fn(Answer, &mut Window, &mut App) + 'static>;
/// A closure fed the index of the option whose preview was toggled.
type PreviewHandler = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

/// The pending question card. Build with [`question_card`].
#[derive(IntoElement)]
pub struct QuestionCard {
    id: ElementId,
    header: SharedString,
    prompt: SharedString,
    subtitle: SharedString,
    options: Vec<QuestionOption>,
    multi: bool,
    allow_other: bool,
    selected: Vec<usize>,
    hint: Option<SharedString>,
    limits: (Option<usize>, Option<usize>),
    timeout: Option<(u64, u64)>,
    previews_open: Vec<usize>,
    clarify_open: bool,
    clarify_slot: Option<AnyElement>,
    on_select: Option<SelectHandler>,
    on_other: Option<ClickHandler>,
    on_answer: Option<AnswerHandler>,
    on_skip: Option<ClickHandler>,
    on_toggle_preview: Option<PreviewHandler>,
    on_clarify: Option<ClickHandler>,
    wizard: Option<Wizard>,
    on_back: Option<ClickHandler>,
}

/// Where a several-question request is (docs/41 §2.6, the owner's wizard,
/// 2026-10-13): one card, one question at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wizard {
    /// The request's topic, the card's title.
    pub topic: SharedString,
    /// Why the agent is asking, under the topic.
    pub why: SharedString,
    /// The current question, from 0.
    pub index: usize,
    /// How many questions there are.
    pub total: usize,
}

/// A question with `prompt` and `options`, drawn as radios; call
/// [`QuestionCard::multi`] for checkboxes.
///
/// The options are borrowed: a caller that owns them in its own state passes a
/// slice rather than cloning the list to hand it over (finding
/// `library-hotpaths-8`).
pub fn question_card(id: impl Into<ElementId>, prompt: impl Into<SharedString>, options: &[QuestionOption]) -> QuestionCard {
    QuestionCard {
        id: id.into(),
        header: SharedString::default(),
        prompt: prompt.into(),
        subtitle: SharedString::default(),
        options: options.to_vec(),
        multi: false,
        allow_other: false,
        selected: Vec::new(),
        hint: None,
        limits: (None, None),
        timeout: None,
        previews_open: Vec::new(),
        clarify_open: false,
        clarify_slot: None,
        on_select: None,
        on_other: None,
        on_answer: None,
        on_skip: None,
        on_toggle_preview: None,
        on_clarify: None,
        wizard: None,
        on_back: None,
    }
}

impl QuestionCard {
    /// The short label above the prompt (MSP `UserInputQuestion.header`).
    pub fn header(mut self, header: impl Into<SharedString>) -> Self {
        self.header = header.into();
        self
    }

    /// The supporting line under the prompt.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// How many options a multi-select must gather: `(min, max)`.
    ///
    /// The confirm button is disabled outside the range and the hint says why,
    /// because a request the server will refuse should never leave the card.
    pub fn limits(mut self, min: Option<usize>, max: Option<usize>) -> Self {
        self.limits = (min, max);
        self
    }

    /// The auto-resolution countdown: how long is left, out of how long there
    /// was. The host ticks the clock; the card only draws the pill.
    pub fn timeout(mut self, remaining_ms: u64, total_ms: u64) -> Self {
        self.timeout = Some((remaining_ms, total_ms));
        self
    }

    /// Which options have their preview expanded, as indices into `options`.
    pub fn previews_open(mut self, open: Vec<usize>) -> Self {
        self.previews_open = open;
        self
    }

    /// Whether the "Explain instead" field is open.
    pub fn clarify_open(mut self, open: bool) -> Self {
        self.clarify_open = open;
        self
    }

    /// The clarification field itself — the host's element, as the approval
    /// card's feedback field is.
    pub fn clarify_slot(mut self, slot: impl IntoElement) -> Self {
        self.clarify_slot = Some(slot.into_any_element());
        self
    }

    /// An option's "Preview" chevron was clicked.
    pub fn on_toggle_preview(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_preview = Some(std::rc::Rc::new(f));
        self
    }

    /// "Explain instead": the person would rather write than pick (MSP
    /// `userInput/clarify`).
    pub fn on_clarify(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_clarify = Some(Box::new(f));
        self
    }

    /// Draws the card as one step of several questions: the topic header,
    /// `n of m`, the progress bar, **Back** and **Next** (**Continue** on
    /// the last).
    pub fn wizard(mut self, wizard: Option<Wizard>) -> Self {
        self.wizard = wizard;
        self
    }

    /// **Back** in a wizard past its first question.
    pub fn on_back(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Box::new(f));
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

/// `12 s`, `2 m 05 s` — how the countdown pill spells what is left.
fn countdown_label(remaining_ms: u64) -> String {
    let seconds = remaining_ms / 1000;
    if seconds < 60 {
        format!("Auto-resolves in {seconds} s")
    } else {
        format!("Auto-resolves in {} m {:02} s", seconds / 60, seconds % 60)
    }
}

/// An option's preview, rendered by its declared `format`.
///
/// `markdown` goes through the transcript's own prose renderer, `diff` through
/// the diff block, and anything else — including MSP's `text` — is mono text,
/// because a format this build has never seen must not be guessed at.
fn preview_body(id: impl Into<ElementId>, preview: &QuestionPreview, p: &Palette) -> AnyElement {
    let id: ElementId = id.into();
    match preview.format.as_str() {
        "markdown" => div()
            .w_full()
            .px(px(PREVIEW_PAD_X))
            .py(px(PREVIEW_PAD_Y))
            .rounded(px(scale::R_MD))
            .bg(crate::util::raised_lit(p, p.surface_2))
            .child(crate::transcript::prose(
                id,
                &preview.content,
                ProseStyle { ink: p.ink_2, code_ink: p.ink_2, code_bg: p.surface_3, size: DESCRIPTION_TEXT, line_height: scale::LH_BODY, paragraph_gap: scale::SP_2 },
            ))
            .into_any_element(),
        // MSP hands the diff over as text, not as hunks, so it takes the code
        // block with a `diff` label rather than the structured diff review.
        "diff" => crate::transcript::code_block(id, "preview", preview.content.clone()).language("diff").into_any_element(),
        _ => div()
            .w_full()
            .px(px(PREVIEW_PAD_X))
            .py(px(PREVIEW_PAD_Y))
            .rounded(px(scale::R_MD))
            .bg(p.term_bg)
            .mono(DESCRIPTION_TEXT)
            .line_height(aui_tokens::scaled(DESCRIPTION_TEXT * PREVIEW_LINE))
            .text_color(p.term_fg)
            .child(SharedString::from(preview.content.clone()))
            .into_any_element(),
    }
}

/// The 16 px radio (1.5 muted ring; picked: a foreground ring and an 8 px
/// dot) or checkbox (radius 4; picked: the foreground fill and a
/// frame-coloured tick), the dot popping in on the fast spring.
fn option_mark(id: ElementId, on: bool, multi: bool, p: &Palette, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let phase = spring_phase(id, on, SpringKind::Fast, window, cx).clamp(0.0, 1.0);
    let mark = div().flex_none().mt(px(MARK_OFFSET)).size(px(MARK)).flex().items_center().justify_center();
    if multi {
        mark.rounded(px(CHECKBOX_RADIUS))
            .when(!on, |d| d.border(px(MARK_BORDER)).border_color(p.ink_2))
            .when(on, |d| d.bg(p.ink))
            .when(phase > 0.0, |d| d.child(icon(IconName::CheckBold).size(px(10.0 * (DOT_FROM + (1.0 - DOT_FROM) * phase))).color(p.bg)))
    } else {
        let dot = DOT * (DOT_FROM + (1.0 - DOT_FROM) * phase);
        mark.rounded_full()
            .border(px(MARK_BORDER))
            .border_color(if on { p.ink } else { p.ink_2 })
            .when(phase > 0.0, |d| d.child(div().size(px(dot)).rounded_full().bg(p.ink.alpha(phase))))
    }
}

impl RenderOnce for QuestionCard {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let tile = div()
            .flex_none()
            .size(px(TILE))
            .rounded(px(TILE_RADIUS))
            .bg(crate::util::raised_lit(&p, p.surface_2))
            .shadow(p.edge())
            .flex()
            .items_center()
            .justify_center()
            .child(icon(IconName::Question).size(px(TILE_GLYPH)).color(p.ink_2));
        // The auto-resolve tag: caption, warning on warning-soft, radius 6.
        let countdown = self.timeout.map(|(remaining_ms, _)| {
            div().flex_none().px(px(8.0)).py(px(2.0)).rounded(px(6.0)).bg(p.warning_soft).ui(scale::FS_12).text_color(p.warning).child(countdown_label(remaining_ms))
        });
        // The question's own header (caption muted) over the prompt (600).
        let prompt_block = v_flex()
            .flex_1()
            .min_w(px(0.0))
            .when(!self.header.is_empty(), |d| d.child(div().ui(SUB_TEXT).text_color(p.ink_2).child(self.header.clone())))
            .child(div().ui(TITLE_TEXT).semibold().text_color(p.ink).child(self.prompt.clone()))
            .when(!self.subtitle.is_empty(), |d| d.child(div().ui(SUB_TEXT).text_color(p.ink_2).child(self.subtitle.clone())));

        let mut card = crate::util::raised_block(v_flex().id(id.clone()).w_full().rounded(px(crate::transcript::card::CARD_RADIUS)), &p, p.surface_1)
            .shadow(crate::transcript::card::attention_edge(p.warning))
            .overflow_hidden();
        match &self.wizard {
            // Several questions: one card, one question at a time — the topic
            // and why in the header with `n of m`, a 3 px progress bar, then
            // the current question.
            Some(wizard) => {
                card = card
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap(px(HEAD_GAP))
                            .py(px(HEAD_PAD_Y))
                            .px(px(HEAD_PAD_X))
                            .child(tile)
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(div().ui(TITLE_TEXT).semibold().text_color(p.ink).child(wizard.topic.clone()))
                                    .when(!wizard.why.is_empty(), |d| d.child(div().ui(SUB_TEXT).text_color(p.ink_2).child(wizard.why.clone()))),
                            )
                            .child(div().flex_none().mono(scale::FS_11).text_color(p.ink_4).child(format!("{} of {}", wizard.index + 1, wizard.total)))
                            .children(countdown),
                    )
                    .child(progress_bar(&p, (wizard.index + 1) as f32 / wizard.total.max(1) as f32))
                    .child(div().w_full().pt(px(10.0)).pb(px(6.0)).px(px(HEAD_PAD_X)).child(prompt_block));
            }
            None => {
                card = card.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap(px(HEAD_GAP))
                        .py(px(HEAD_PAD_Y))
                        .px(px(HEAD_PAD_X))
                        .child(tile)
                        .child(prompt_block)
                        .children(countdown),
                );
            }
        }

        let mut options = v_flex().w_full().px(px(OPT_MARGIN_X)).gap(px(2.0));
        for (index, option) in self.options.iter().enumerate() {
            let on = self.selected.contains(&index);
            let row_id: ElementId = (id.clone(), SharedString::from(format!("opt-{index}"))).into();
            let (state, flags) = interaction_flags(row_id.clone(), window, cx);
            // Selected 9 %, hover 5 %, on FF's 80 ms glide.
            let tint = if on { p.selected_overlay() } else if flags.hovered { p.hover_overlay() } else { p.hover_overlay().alpha(0.0) };
            let bg: Hsla = tween((row_id.clone(), "bg"), tint, ROW_GLIDE, window, cx);
            let mut row = h_flex()
                .id(row_id.clone())
                .w_full()
                .items_start()
                .gap(px(OPT_GAP))
                .py(px(OPT_PAD_Y))
                .px(px(OPT_PAD_X))
                .rounded(px(OPT_RADIUS))
                .bg(bg)
                .cursor_pointer()
                .track_interaction(&state)
                .child(option_mark((row_id.clone(), "mark").into(), on, self.multi, &p, window, cx))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(div().ui(LABEL_TEXT).when(on, |d| d.semibold()).text_color(p.ink).child(SharedString::from(option.label.clone())))
                        .when(!option.description.is_empty(), |d| d.child(div().ui(DESCRIPTION_TEXT).text_color(p.ink_2).child(SharedString::from(option.description.clone())))),
                );
            if !option.key.is_empty() {
                row = row.child(key_cap(&p, option.key.clone()));
            }
            if let Some(on_select) = self.on_select.clone() {
                // The row is the radio/checkbox control: it carries the
                // option's label as its accessible name.
                let role = if self.multi { gpui::Role::CheckBox } else { gpui::Role::RadioButton };
                record_ax_label(&option.label);
                row = row.role(role).aria_label(SharedString::from(option.label.clone())).on_click(move |_, w, cx| on_select(index, w, cx));
            }
            options = options.child(row);

            // An option may carry a rendered preview — a diff, a snippet — and
            // the "Preview" disclosure is how the person asks to see it.
            let Some(preview) = &option.preview else { continue };
            let open = self.previews_open.contains(&index);
            let toggle_id: ElementId = (id.clone(), SharedString::from(format!("preview-{index}"))).into();
            let preview_label = SharedString::from(format!("{} preview for {}", if open { "Hide" } else { "Show" }, option.label));
            record_ax_label(&preview_label);
            let mut disclosure = h_flex()
                .id(toggle_id)
                .pt(px(4.0))
                .pr(px(10.0))
                .pb(px(4.0))
                .pl(px(18.0))
                .items_center()
                .gap(px(PREVIEW_GAP))
                .ui(DESCRIPTION_TEXT)
                .text_color(p.ink_2)
                .cursor_pointer()
                .hover(|d| d.text_color(p.ink))
                .role(gpui::Role::Button)
                .aria_label(preview_label)
                .child(icon(if open { IconName::ChevronDown } else { IconName::Chevron }).size(px(PREVIEW_GLYPH)))
                .child("Preview");
            if let Some(on_toggle) = self.on_toggle_preview.clone() {
                disclosure = disclosure.on_click(move |_, w, cx| on_toggle(index, w, cx));
            }
            options = options.child(disclosure);
            if open {
                options = options.child(div().w_full().pb(px(4.0)).child(preview_body((id.clone(), SharedString::from(format!("preview-body-{index}"))), preview, &p)));
            }
        }
        card = card.child(options.pb(px(8.0)));

        // "Explain instead": the field is the host's, the buttons are the card's.
        if self.clarify_open {
            if let Some(slot) = self.clarify_slot.take() {
                card = card.child(div().mx(px(OPT_MARGIN_X)).mb(px(OTHER_MARGIN_BOTTOM)).child(slot));
            }
        }

        // "Other": a 32 px Well input with the edit glyph.
        if self.allow_other {
            let other_id: ElementId = (id.clone(), "other").into();
            record_ax_label("Other, type your own answer");
            let mut other = h_flex()
                .id(other_id)
                .h(px(OTHER_H))
                .items_center()
                .gap(px(OTHER_GAP))
                .px(px(OTHER_PAD_X))
                .mx(px(OPT_MARGIN_X))
                .mb(px(OTHER_MARGIN_BOTTOM))
                .rounded(px(OPT_RADIUS))
                .bg(p.well)
                .ui(OTHER_TEXT)
                .text_color(p.ink_2)
                .cursor_text()
                .role(gpui::Role::Button)
                .aria_label("Other, type your own answer")
                .child(icon(IconName::Edit).size(px(OTHER_GLYPH)).color(p.ink_2))
                .child("Other…");
            if let Some(on_other) = self.on_other {
                other = other.on_click(move |e, w, cx| on_other(e, w, cx));
            }
            card = card.child(other);
        }

        // The footer: key hints (or the limit the selection is outside),
        // a spacer, then only the choices the provider minted.
        let count = self.selected.len();
        let (min, max) = self.limits;
        let under = min.is_some_and(|min| count < min);
        let over = max.is_some_and(|max| count > max);
        let last = self.wizard.as_ref().is_none_or(|w| w.index + 1 >= w.total);
        let advance = if last { "continue" } else { "next" };
        let hint: SharedString = self.hint.clone().unwrap_or_else(|| match (under, over, min, max) {
            (true, _, Some(min), _) => SharedString::from(format!("Pick at least {min}")),
            (_, true, _, Some(max)) => SharedString::from(format!("Pick at most {max}")),
            _ if self.multi => SharedString::from(format!("↑↓ move · space pick · ⏎ {advance}")),
            _ => SharedString::from(format!("↑↓ move · ⏎ {advance}")),
        });
        let answer = Answer { selected: self.selected.clone(), other: None };
        let mut foot = h_flex().w_full().items_center().gap(px(FOOT_GAP)).py(px(FOOT_PAD_Y)).px(px(FOOT_PAD_X)).border_t_1().border_color(p.line);
        if self.wizard.as_ref().is_some_and(|w| w.index > 0) {
            if let Some(on_back) = self.on_back {
                let back = control_with((id.clone(), "back"), Some(IconName::ArrowLeft), "Back", ControlKind::Text, &p)
                    .pl(px(6.0))
                    .on_click(move |e, w, cx| on_back(e, w, cx));
                foot = foot.child(back);
            }
        }
        foot = foot.child(key_hints(hint, &p)).child(div().flex_1());
        if let Some(on_skip) = self.on_skip {
            foot = foot.child(control((id.clone(), "skip"), "Skip", ControlKind::Text, &p).on_click(move |e, w, cx| on_skip(e, w, cx)));
        }
        if let Some(on_clarify) = self.on_clarify {
            foot = foot.child(control((id.clone(), "clarify"), "Explain instead", ControlKind::Secondary, &p).on_click(move |e, w, cx| on_clarify(e, w, cx)));
        }
        let mut confirm = control((id.clone(), "continue"), if last { "Continue" } else { "Next" }, ControlKind::Primary, &p);
        if under || over {
            confirm = confirm.opacity(0.4).cursor_default();
        } else if let Some(on_answer) = self.on_answer {
            confirm = confirm.on_click(move |_, w, cx| on_answer(answer.clone(), w, cx));
        }
        card.child(foot.child(confirm))
    }
}

/// The 3 px progress bar under a wizard's or the tasks card's header: a 7 %
/// track, the muted fill clamped at 100 %.
pub(crate) fn progress_bar(p: &Palette, fraction: f32) -> impl IntoElement {
    div()
        .relative()
        .w_full()
        .h(px(3.0))
        .bg(p.line)
        .child(div().absolute().top_0().left_0().h_full().w(gpui::relative(fraction.clamp(0.0, 1.0))).bg(p.ink_2))
}

/// The key cap at an option's right: faint mono on white 7 %, radius 4, 1 / 6.
fn key_cap(p: &Palette, key: impl Into<SharedString>) -> impl IntoElement {
    div().flex_none().px(px(6.0)).py(px(1.0)).rounded(px(4.0)).bg(p.line).mono(scale::FS_11).text_color(p.ink_4).child(key.into())
}

/// How a question settled, which is what the collapsed row says it did.
///
/// MSP settles a prompt six ways and only one of them is an answer; a cancelled
/// or timed-out prompt drawn as "Answered:" with no chips would be a lie.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QuestionOutcome {
    /// The person picked something; the chips are the labels.
    #[default]
    Answered,
    /// The person declined (MSP `cancelled`).
    Skipped,
    /// The person wrote an explanation instead (MSP `clarified`).
    Clarified(SharedString),
    /// Nobody answered in time (MSP `timedOut`).
    TimedOut,
    /// The turn ended under it (MSP `interrupted` / `aborted`).
    Interrupted,
}

impl QuestionOutcome {
    /// The lead-in the row shows before the chips.
    fn label(&self) -> SharedString {
        match self {
            QuestionOutcome::Answered => "Answered".into(),
            QuestionOutcome::Skipped => "Skipped".into(),
            QuestionOutcome::Clarified(_) => "Clarified".into(),
            QuestionOutcome::TimedOut => "Timed out".into(),
            QuestionOutcome::Interrupted => "Interrupted".into(),
        }
    }

    /// Whether the outcome is a success (the glyph) or a quiet non-answer.
    fn answered(&self) -> bool {
        matches!(self, QuestionOutcome::Answered | QuestionOutcome::Clarified(_))
    }
}

/// The answered state of a question. Build with [`answered_row`].
#[derive(IntoElement)]
pub struct AnsweredRow {
    id: ElementId,
    chips: Vec<SharedString>,
    outcome: QuestionOutcome,
}

/// The quiet row a settled question leaves (docs/41 §2.4): `✓ Answered
/// <pick as inline code>`, `✓ Clarified <text>`; a non-answer reads what
/// it was with a muted ✕. Not clickable.
pub fn answered_row(id: impl Into<ElementId>, chips: Vec<SharedString>) -> AnsweredRow {
    AnsweredRow { id: id.into(), chips, outcome: QuestionOutcome::Answered }
}

impl AnsweredRow {
    /// How the question settled; the default is [`QuestionOutcome::Answered`].
    pub fn outcome(mut self, outcome: QuestionOutcome) -> Self {
        self.outcome = outcome;
        self
    }
}

impl RenderOnce for AnsweredRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let glyph = if self.outcome.answered() { status_check(p.success).into_any_element() } else { status_cross(p.ink_3).into_any_element() };
        let mut row = quiet_status_row(self.id.clone(), glyph).part(div().flex_none().child(self.outcome.label()));
        if let QuestionOutcome::Clarified(text) = &self.outcome {
            row = row.part(div().min_w(px(0.0)).truncate().text_color(p.ink).child(text.clone()));
        }
        for chip in self.chips {
            row = row.part(div().flex_none().px(px(5.0)).py(px(1.0)).rounded(px(4.0)).bg(p.line).mono(scale::FS_12).child(chip));
        }
        row
    }
}
