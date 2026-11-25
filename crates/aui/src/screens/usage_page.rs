//! Settings › Usage (harness docs/85 16.1, W7): one page for what the
//! agents cost. At the top each provider's limits — its plan, a bar per
//! window with when it resets ([`usage_card`]); below, tokens, cache % and
//! cost from Baaz's own ledger over 7, 30 or 90 days, split by day, model,
//! project or provider. Built from the Settings parts (`section`,
//! `settings_row`, `segmented`) and the account menu's usage card, so it
//! reads like the pages beside it (docs/83: flat, hairlines, no new chrome).

use std::rc::Rc;

use aui_icons::Provider;
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{record_ax_label, section, segmented, settings_row};
use crate::screens::{usage_card, UsageWindow};

/// One provider's limits, as its card shows them.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageLimits {
    /// Which provider.
    pub provider: Provider,
    /// Its plan (`Pro`, `prolite`), when known.
    pub plan: Option<SharedString>,
    /// One bar per window (5-hour, weekly), with when each resets.
    pub windows: Vec<UsageWindow>,
    /// When the numbers were read (`2m ago`; the card says "as of").
    pub as_of: Option<SharedString>,
}

/// One row of the ledger table.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageLine {
    /// The day, model, project or provider.
    pub key: SharedString,
    /// `1.2M tokens · 64% cached · 38 turns`.
    pub detail: SharedString,
    /// `$3.20`, or `included` on a plan.
    pub cost: SharedString,
}

/// The ranges the table offers, in order.
pub const USAGE_RANGES: [&str; 3] = ["7 days", "30 days", "90 days"];
/// The splits the table offers, in order.
pub const USAGE_SPLITS: [&str; 4] = ["Day", "Model", "Project", "Provider"];

/// What the page shows.
#[derive(Clone, Debug, PartialEq)]
pub struct UsagePageModel {
    /// Each connected provider's limits.
    pub limits: Vec<UsageLimits>,
    /// Index into [`USAGE_RANGES`].
    pub range: usize,
    /// Index into [`USAGE_SPLITS`].
    pub split: usize,
    /// The table, already split and summed.
    pub lines: Vec<UsageLine>,
    /// The range's total, in one line.
    pub total: SharedString,
}

/// What the page's controls ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageIntent {
    /// A range was picked (index into [`USAGE_RANGES`]).
    Range(usize),
    /// A split was picked (index into [`USAGE_SPLITS`]).
    Split(usize),
}

type Handler = Rc<dyn Fn(UsageIntent, &mut Window, &mut App)>;

/// The page. Build with [`usage_page`].
#[derive(IntoElement)]
pub struct UsagePage {
    id: ElementId,
    model: UsagePageModel,
    on_intent: Option<Handler>,
}

/// The Usage page over `model`.
pub fn usage_page(id: impl Into<ElementId>, model: UsagePageModel) -> UsagePage {
    UsagePage { id: id.into(), model, on_intent: None }
}

impl UsagePage {
    /// What a control asks for.
    pub fn on_intent(mut self, f: impl Fn(UsageIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for UsagePage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let m = self.model;
        let id = |s: &str| ElementId::from((self.id.clone(), SharedString::from(s.to_owned())));
        let mut cards = h_flex().w_full().flex_wrap().items_start().gap(px(scale::SP_3));
        for (i, l) in m.limits.iter().enumerate() {
            let mut card = usage_card(id(&format!("limits-{i}")), l.provider, l.windows.clone());
            if let Some(plan) = &l.plan {
                card = card.plan(plan.clone());
            }
            if let Some(as_of) = &l.as_of {
                card = card.as_of(as_of.clone());
            }
            // Three to a row on the Settings measure.
            cards = cards.child(div().w(px(scale::SP_7 * 9.0)).child(card));
        }
        let handler = self.on_intent.clone();
        let range = segmented(id("range"), USAGE_RANGES.iter().map(|s| SharedString::from(*s)).collect(), m.range)
            .accessibility_label("Usage over")
            .on_select(move |i, w, cx| {
                if let Some(h) = handler.as_ref() {
                    h(UsageIntent::Range(i), w, cx);
                }
            });
        let handler = self.on_intent.clone();
        let split = segmented(id("split"), USAGE_SPLITS.iter().map(|s| SharedString::from(*s)).collect(), m.split)
            .accessibility_label("Split usage by")
            .on_select(move |i, w, cx| {
                if let Some(h) = handler.as_ref() {
                    h(UsageIntent::Split(i), w, cx);
                }
            });
        record_ax_label("Usage over");
        record_ax_label("Split usage by");
        let mut table = section(format!("By {}", USAGE_SPLITS[m.split.min(USAGE_SPLITS.len() - 1)].to_lowercase())).description(m.total.clone());
        if m.lines.is_empty() {
            table = table.note("Nothing yet in this range: rows arrive as turns finish.");
        }
        for line in &m.lines {
            table = table.row(
                settings_row(line.key.clone())
                    .description(line.detail.clone())
                    .control(div().role_in(TextRole::Body, cx).text_color(p.fg).child(line.cost.clone())),
            );
        }
        v_flex()
            .w_full()
            .px(px(scale::SP_7))
            .pt(px(scale::SP_6))
            .gap(px(scale::SP_7))
            .child(
                v_flex()
                    .gap(px(scale::SP_1))
                    .child(div().role_in(TextRole::Title, cx).text_color(p.ink).child("Usage"))
                    .child(div().role_in(TextRole::Body, cx).text_color(p.fg_muted).child("Each provider's limits, and what your sessions spent, from Baaz's own record.")),
            )
            .child(v_flex().gap(px(scale::SP_3)).child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child("Limits")).child(cards))
            .child(h_flex().gap(px(scale::SP_3)).items_center().child(range).child(split))
            .child(table)
            .into_any_element()
    }
}
