//! Orchestration's surfaces (harness docs/85 14.9), the mock before the
//! build (rule 30): the Workers block pinned at the foot of the lead's
//! transcript (open, then folded once every worker settled), a worker open
//! beside its lead with a composer, and the per-criterion summary card.

use aui::data::{button, icon_button};
use aui::transcript::{criteria_card, workers_block, CriteriaTask, CriterionLine, CriterionState, WorkerRow, WorkerState};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, Window};
use gpui_kit::base::{h_flex, v_flex};

fn rows(settled: bool) -> Vec<WorkerRow> {
    let row = |id: &str, title: &str, provider: &str, state, elapsed: &str| WorkerRow { id: id.to_owned().into(), title: title.to_owned().into(), provider: provider.to_owned().into(), state, elapsed: elapsed.to_owned().into() };
    if settled {
        vec![
            row("w-write", "Write 15 questions", "Claude Code", WorkerState::Passed, "6m"),
            row("w-review", "Review them", "Codex", WorkerState::Passed, "3m"),
            row("w-import", "Import the approved ones", "Muse", WorkerState::Passed, "1m"),
        ]
    } else {
        vec![
            row("w-write", "Write 15 questions", "Claude Code", WorkerState::Passed, "6m"),
            row("w-review", "Review them", "Codex", WorkerState::NeedsYou, "3m"),
            row("w-import", "Import the approved ones", "Muse", WorkerState::Waiting, ""),
        ]
    }
}

fn column(cx: &App) -> gpui::Div {
    v_flex().w(px(720.0)).p(px(scale::SP_6)).gap(px(scale::SP_4)).bg(cx.aui().colors.bg)
}

/// `workers/block`: the block while work runs (open), then folded to its
/// one line once every worker has settled.
pub fn build_block(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    column(cx)
        .child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child("While the work runs"))
        .child(workers_block("workers-open", rows(false), true))
        .child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child("Every worker settled: folded"))
        .child(workers_block("workers-folded", rows(true), false))
        .into_any_element()
}

/// `workers/summary`: the card the lead's work ends with, per criterion.
pub fn build_summary(_window: &mut Window, cx: &mut App) -> AnyElement {
    let line = |text: &str, state, by: &str| CriterionLine { text: text.to_owned().into(), state, by: by.to_owned().into() };
    column(cx)
        .child(criteria_card(
            "criteria",
            vec![
                CriteriaTask { title: "Write 15 questions".into(), provider: "Claude Code".into(), lines: vec![line("15 questions on fractions", CriterionState::NotProven, "nothing checks it"), line("The tests pass", CriterionState::Met, "the check")] },
                CriteriaTask { title: "Review them".into(), provider: "Codex".into(), lines: vec![line("Each question reviewed as correct", CriterionState::Met, "the reviewer")] },
                CriteriaTask { title: "Import the approved ones".into(), provider: "Muse".into(), lines: vec![line("14 rows in the questions table", CriterionState::Met, "the check"), line("The wording suits Lakshya", CriterionState::YourCall, "you")] },
            ],
        ))
        .into_any_element()
}

/// `workers/pane`: a worker open beside its lead — its header with "Open in
/// centre", a few of its rows, and a composer that steers it (or queues
/// where its provider does not steer).
pub fn build_pane(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let header = h_flex()
        .w_full()
        .h(px(scale::H_MD))
        .px(px(scale::SP_3))
        .gap(px(scale::SP_2))
        .items_center()
        .border_b_1()
        .border_color(p.line)
        .child(div().flex_1().min_w(px(0.0)).truncate().role_in(TextRole::Body, cx).text_color(p.ink).child("Review them"))
        .child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child("Codex · Read-only"))
        .child(button("pane-open-centre", "Open in centre").ghost().accessibility_label("Open this worker in the centre"))
        .child(icon_button("pane-close", IconName::X).ghost().muted().accessibility_label("Close the worker pane"));
    let line = |who: &str, text: &str| {
        v_flex()
            .gap(px(scale::SP_1))
            .child(div().role_in(TextRole::Micro, cx).text_color(p.ink_4).child(who.to_owned()))
            .child(div().role_in(TextRole::Body, cx).text_color(p.fg).child(text.to_owned()))
    };
    let body = v_flex()
        .flex_1()
        .min_h(px(0.0))
        .p(px(scale::SP_4))
        .gap(px(scale::SP_4))
        .child(line("From session \"Lakshya's 15 questions\" (an agent)", "Review the 15 questions in questions.md against the three criteria."))
        .child(line("Codex", "Questions 1–6 and 8–15 read correctly. Question 7 has two right answers."))
        .child(line("Codex · escalated", "Keep question 7, or drop it?"));
    let composer = div()
        .m(px(scale::SP_3))
        .p(px(scale::SP_3))
        .border_1()
        .border_color(p.line)
        .rounded(px(scale::SP_2))
        .role_in(TextRole::Body, cx)
        .text_color(p.fg_muted)
        .child("Message this worker — steered into its turn");
    h_flex()
        .w(px(1060.0))
        .h(px(560.0))
        .items_start()
        .overflow_hidden()
        .bg(p.bg)
        .child(
            v_flex()
                .flex_1()
                .h_full()
                .p(px(scale::SP_6))
                .gap(px(scale::SP_4))
                .border_r_1()
                .border_color(p.line)
                .child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child("The lead's transcript"))
                .child(workers_block("pane-workers", rows(false), true)),
        )
        .child(v_flex().w(px(400.0)).flex_none().h_full().overflow_hidden().child(header).child(body).child(composer))
        .into_any_element()
}
