//! The session goal line: one row on surface-2 with a top hairline — the
//! `Goal` label, a status pill, the truncated objective, the token meter or
//! percent, the elapsed time, and the last reason. There are no buttons.
//!
//! The line renders the [`Block::Goal`](aui_protocol::Block::Goal) fields
//! through a small borrowed struct ([`GoalLineFields`]) rather than the
//! block itself, so callers destructure the block once and the card borrows
//! the strings.

use aui_protocol::Provider as WireProvider;
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, Window};
use gpui_kit::base::h_flex;

use crate::data::{pill, PillVariant};
use crate::icons::{provider_mark, Provider as IconProvider};

use super::common::mark;

/// Row rhythm.
const PAD_X: f32 = 12.0;
const ROW_H: f32 = 30.0;
const ROW_GAP: f32 = 6.0;
/// The token meter: 80x4 px, accent fill on a surface-3 track.
const METER_W: f32 = 80.0;
const METER_H: f32 = 4.0;
/// Provider marks in the row.
const MARK: f32 = 14.0;

/// The [`Block::Goal`](aui_protocol::Block::Goal) fields [`goal_line`]
/// renders, borrowed from the block.
#[derive(Debug, Clone, Copy)]
pub struct GoalLineFields<'a> {
    /// What the agent is trying to achieve.
    pub objective: &'a str,
    /// Free-form status string, verbatim from the provider.
    pub status: &'a str,
    /// Percentage complete, verbatim from the provider.
    pub percent_complete: Option<f32>,
    /// Which provider is doing the work, when known.
    pub provider: Option<WireProvider>,
    /// Tokens spent so far, when the host measured them.
    pub tokens_used: Option<u64>,
    /// Token budget the goal runs against, when there is one.
    pub token_budget: Option<u64>,
    /// How long the goal has been running, e.g. `"7 s"`.
    pub elapsed: Option<&'a str>,
    /// Why the goal is where it is.
    pub last_reason: Option<&'a str>,
    /// Whether Baaz gates this goal.
    pub gated_by_baaz: bool,
    /// Whether the provider said done, the checks disagreed, and the goal
    /// is moving again.
    pub reopened: bool,
}

/// The session goal line. Build with [`goal_line`].
#[derive(IntoElement)]
pub struct GoalLine {
    id: ElementId,
    objective: String,
    status: String,
    percent_complete: Option<f32>,
    provider: Option<WireProvider>,
    tokens_used: Option<u64>,
    token_budget: Option<u64>,
    elapsed: Option<String>,
    last_reason: Option<String>,
    gated_by_baaz: bool,
    reopened: bool,
}

/// A session goal as one muted row.
///
/// The card copies the borrowed fields; it owns no orchestration logic.
pub fn goal_line(id: impl Into<ElementId>, fields: &GoalLineFields<'_>) -> GoalLine {
    GoalLine {
        id: id.into(),
        objective: fields.objective.to_owned(),
        status: fields.status.to_owned(),
        percent_complete: fields.percent_complete,
        provider: fields.provider,
        tokens_used: fields.tokens_used,
        token_budget: fields.token_budget,
        elapsed: fields.elapsed.map(str::to_owned),
        last_reason: fields.last_reason.map(str::to_owned),
        gated_by_baaz: fields.gated_by_baaz,
        reopened: fields.reopened,
    }
}

/// The canonical pill word and variant for a provider status string: active is
/// accent, complete is success, blocked/limited/paused is warning, and a
/// reopened goal always reads `Reopened` in warning.
fn status_pill(status: &str, reopened: bool) -> (&'static str, PillVariant) {
    if reopened {
        return ("Reopened", PillVariant::Warning);
    }
    let lower = status.to_lowercase();
    if lower.contains("complet") {
        ("Complete", PillVariant::Success)
    } else if lower.contains("block") {
        ("Blocked", PillVariant::Warning)
    } else if lower.contains("limit") {
        ("Limited", PillVariant::Warning)
    } else if lower.contains("paus") {
        ("Paused", PillVariant::Warning)
    } else {
        ("Active", PillVariant::Accent)
    }
}

/// `6713` as `6.7k`, `80000` as `80k`, anything under a thousand verbatim.
fn short_tokens(n: u64) -> String {
    if n < 1000 {
        format!("{n}")
    } else {
        let k = n as f64 / 1000.0;
        if k.fract() < 0.05 {
            format!("{}k", k.round() as u64)
        } else {
            format!("{k:.1}k")
        }
    }
}

/// `40.0` as `40%`, `8.4` as `8.4%`: the provider's number, verbatim.
fn short_percent(percent: f32) -> String {
    if percent.fract() == 0.0 {
        format!("{percent:.0}%")
    } else {
        format!("{percent:.1}%")
    }
}

/// Maps the icon provider mark for the row's provider, if any.
fn provider_mark_for(provider: Option<WireProvider>) -> Option<IconProvider> {
    provider.map(mark)
}

impl RenderOnce for GoalLine {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let (pill_word, pill_variant) = status_pill(&self.status, self.reopened);

        let mut row = h_flex()
            .id(self.id)
            .w_full()
            .h(px(ROW_H))
            .items_center()
            .gap(px(ROW_GAP))
            .px(px(PAD_X))
            .border_t_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .child(div().flex_none().ui(scale::FS_12).medium().text_color(p.ink_2).child("Goal"))
            .child(pill(pill_word).variant(pill_variant));
        if let Some(provider) = provider_mark_for(self.provider) {
            row = row.child(provider_mark(provider).size(px(MARK)));
        }
        row = row.child(
            div().min_w(px(0.0)).flex_1().truncate().ui(scale::FS_12).text_color(p.ink).child(self.objective),
        );
        // A set budget draws the 80x4 meter and `used / budget tokens`;
        // otherwise a bare percent stands on its own.
        if let Some(budget) = self.token_budget {
            let used = self.tokens_used.unwrap_or(0);
            let fraction = if budget == 0 { 0.0 } else { (used as f32 / budget as f32).clamp(0.0, 1.0) };
            row = row
                .child(
                    div()
                        .flex_none()
                        .w(px(METER_W))
                        .h(px(METER_H))
                        .rounded_full()
                        .bg(p.surface_3)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(px(METER_W * fraction))
                                .rounded_full()
                                .bg(p.accent),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .text_role(TextRole::MonoSmall)
                        .text_color(p.ink_3)
                        .child(format!("{} / {} tokens", short_tokens(used), short_tokens(budget))),
                );
        } else if let Some(percent) = self.percent_complete {
            row = row.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_role(TextRole::MonoSmall)
                    .text_color(p.ink_3)
                    .child(short_percent(percent)),
            );
        }
        if let Some(elapsed) = self.elapsed {
            row = row.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_role(TextRole::MonoSmall)
                    .text_color(p.ink_3)
                    .child(elapsed),
            );
        }
        if self.gated_by_baaz {
            row = row.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_role(TextRole::Meta)
                    .text_color(p.ink_3)
                    .child("checked by Baaz"),
            );
        }
        if let Some(reason) = self.last_reason {
            // A reopened goal's reason reads in warning: the provider said
            // done and the checks disagreed.
            let ink = if self.reopened { p.warning } else { p.ink_3 };
            row = row.child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .text_role(TextRole::Meta)
                    .text_color(ink)
                    .child(reason),
            );
        }
        row
    }
}
