//! Two transcript cards a provider-driven session cannot do without: the
//! mandated fallback for an item kind this build does not model, and the
//! session goal.
//!
//! Both are deliberately literal. MSP's `ItemKind` is an **open** enum and the
//! protocol requires a client that meets an unknown kind to show the kind, the
//! status and the server's own `fallbackText` rather than guess a richer card;
//! a goal's `status` and `percentComplete` are the provider's own strings and
//! numbers, so the card shows what it was handed — including a percentage over
//! 100 — and only clamps the *bar*, which cannot draw past its end.

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{pill, PillVariant};
use crate::transcript::transcript_card;

/// The body of both cards keeps the transcript's own card padding.
const BODY_PAD: f32 = scale::SP_4;
/// The header row's gap, matching every other transcript card header.
const HEAD_GAP: f32 = scale::SP_2;
/// The definition rows under a goal: label column, gap, row rhythm.
const DL_LABEL_W: f32 = 84.0;
const DL_COL_GAP: f32 = scale::SP_3;
const DL_ROW_GAP: f32 = scale::SP_2;
/// The progress bar: full width of the body, 4 px tall, its own radius.
const BAR_HEIGHT: f32 = 4.0;
const BAR_RADIUS: f32 = 2.0;
/// The bar and the number it belongs to sit on one row.
const BAR_GAP: f32 = scale::SP_3;
const BAR_NUMBER_W: f32 = 44.0;

/// The fallback card for an unknown item kind. Build with
/// [`generic_item_card`].
#[derive(IntoElement)]
pub struct GenericItemCard {
    id: ElementId,
    kind: SharedString,
    status: SharedString,
    text: SharedString,
}

/// The kind name, the item's status and the server's `fallbackText`.
///
/// This is the whole card on purpose: a client that invented a richer rendering
/// for a kind it does not model would be guessing at the provider's meaning.
pub fn generic_item_card(id: impl Into<ElementId>, kind: impl Into<SharedString>, status: impl Into<SharedString>, text: impl Into<SharedString>) -> GenericItemCard {
    GenericItemCard { id: id.into(), kind: kind.into(), status: status.into(), text: text.into() }
}

impl RenderOnce for GenericItemCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut card = transcript_card(id, true).chevron(false).hover_tint(false).header(
            h_flex()
                .w_full()
                .items_center()
                .gap(px(HEAD_GAP))
                .child(div().flex_none().ui(scale::FS_13).semibold().text_color(p.ink).child(self.kind))
                .child(div().flex_1().min_w(px(0.0)))
                .child(pill(self.status).variant(PillVariant::Quiet)),
        );
        // An item with no fallback text is a header and nothing else; an empty
        // body would draw a stray border under it.
        if !self.text.is_empty() {
            card = card.body(div().w_full().p(px(BODY_PAD)).ui(scale::FS_12).text_color(p.ink_2).child(self.text));
        }
        card
    }
}

/// The session goal. Build with [`goal_card`].
#[derive(IntoElement)]
pub struct GoalCard {
    id: ElementId,
    objective: SharedString,
    status: SharedString,
    percent: Option<f32>,
    current_work: Option<SharedString>,
    next_work: Option<SharedString>,
}

/// The objective and the provider's own status string.
///
/// Add the progress with [`GoalCard::percent`] and the two work lines with
/// [`GoalCard::current_work`] / [`GoalCard::next_work`].
pub fn goal_card(id: impl Into<ElementId>, objective: impl Into<SharedString>, status: impl Into<SharedString>) -> GoalCard {
    GoalCard { id: id.into(), objective: objective.into(), status: status.into(), percent: None, current_work: None, next_work: None }
}

impl GoalCard {
    /// How far along the provider says it is, **verbatim**.
    ///
    /// The number is printed as it arrived: a provider that reports 120 % has
    /// said something about itself worth seeing. Only the bar is clamped, since
    /// it has nowhere further to fill.
    pub fn percent(mut self, percent: Option<f32>) -> Self {
        self.percent = percent;
        self
    }

    /// What the agent says it is doing now.
    pub fn current_work(mut self, work: impl Into<SharedString>) -> Self {
        self.current_work = Some(work.into());
        self
    }

    /// What it says it will do next.
    pub fn next_work(mut self, work: impl Into<SharedString>) -> Self {
        self.next_work = Some(work.into());
        self
    }
}

impl RenderOnce for GoalCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let header = h_flex()
            .w_full()
            .items_center()
            .gap(px(HEAD_GAP))
            .child(div().flex_1().min_w(px(0.0)).truncate().ui(scale::FS_13).semibold().text_color(p.ink).child(self.objective))
            .child(pill(self.status).variant(PillVariant::Line));

        let mut body = v_flex().w_full().p(px(BODY_PAD)).gap(px(DL_ROW_GAP));
        if let Some(percent) = self.percent {
            let fraction = (percent / 100.0).clamp(0.0, 1.0);
            body = body.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(BAR_GAP))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .h(px(BAR_HEIGHT))
                            .rounded(px(BAR_RADIUS))
                            .bg(p.surface_3)
                            .overflow_hidden()
                            .child(div().h_full().w(gpui::relative(fraction)).rounded(px(BAR_RADIUS)).bg(p.accent)),
                    )
                    .child(div().flex_none().w(px(BAR_NUMBER_W)).flex().justify_end().mono(scale::FS_11).text_color(p.ink_3).child(format!("{percent:.0}%"))),
            );
        }
        for (label, value) in [("Doing now", self.current_work), ("Next", self.next_work)] {
            let Some(value) = value else { continue };
            body = body.child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap(px(DL_COL_GAP))
                    .ui(scale::FS_12)
                    .child(div().flex_none().w(px(DL_LABEL_W)).text_color(p.ink_3).child(label))
                    .child(div().flex_1().min_w(px(0.0)).text_color(p.ink_2).child(value)),
            );
        }

        transcript_card(id, true).chevron(false).hover_tint(false).header(header).body(body)
    }
}
