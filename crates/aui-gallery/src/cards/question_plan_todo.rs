//! Card 36 · Question, plan and todo cards: a multi-select question with an
//! "Other" row and its answered state, a proposed plan, and the task list with
//! morphing marks. Reproduces
//! `design/src/cards/transcript/36-question-plan-todo.html` at 800×760.

use aui::protocol::{QuestionOption, TodoItem, TodoState};
use aui::transcript::{answered_row, plan_card, question_card, todo_list};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.grid{grid-template-columns:1fr 1fr;gap:14px}`.
const COLUMN_GAP: f32 = 14.0;
/// `.col{gap:var(--sp-3)}` — the gap between the cards in one column.
const STACK_GAP: f32 = aui_tokens::scale::SP_3;

fn option(label: &str, description: &str, key: &str) -> QuestionOption {
    QuestionOption { label: label.into(), description: description.into(), key: key.into(), preview: None }
}

fn task(label: &str, state: TodoState, elapsed_ms: Option<u64>) -> TodoItem {
    TodoItem { label: label.into(), state, elapsed_ms }
}

/// The postcode question's options.
fn options() -> Vec<QuestionOption> {
    vec![
        option("US ZIP and ZIP+4", "12345 or 12345-6789", "1"),
        option("Canadian postal", "A1A 1A1, space optional", "2"),
        option("UK postcode", "Outward + inward, e.g. SW1A 1AA", "3"),
    ]
}

/// The plan's steps.
fn plan_items() -> Vec<String> {
    vec![
        "Branch `validateAddress` per country; keep regexes small.".into(),
        "Return a structured `{ ok, field }` instead of boolean.".into(),
        "Add CA / GB / empty-country cases to the test file.".into(),
        "Run focused tests, then lint touched files.".into(),
    ]
}

/// The task list.
fn tasks() -> Vec<TodoItem> {
    vec![
        task("Read validators and the form", TodoState::Done, Some(12_000)),
        task("Branch validation per country", TodoState::Done, Some(48_000)),
        task("Return structured result", TodoState::Done, Some(20_000)),
        task("Add CA, GB and empty-country tests", TodoState::Running, Some(64_000)),
        task("Run focused tests", TodoState::Pending, None),
        task("Lint touched files", TodoState::Pending, None),
        task("Summarise the change", TodoState::Pending, None),
    ]
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let selected = window.use_keyed_state("card36-selected", cx, |_, _| vec![0usize, 1]);
    let open = window.use_keyed_state("card36-todo-open", cx, |_, _| true);
    let current = selected.read(cx).clone();
    let todo_open = *open.read(cx);
    let toggle_option = {
        let selected = selected.clone();
        move |index: usize, _: &mut Window, cx: &mut App| {
            selected.update(cx, |s, cx| {
                if let Some(at) = s.iter().position(|i| *i == index) {
                    s.remove(at);
                } else {
                    s.push(index);
                }
                cx.notify();
            })
        }
    };
    let toggle_todo = {
        let open = open.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            open.update(cx, |o, cx| {
                *o = !*o;
                cx.notify();
            })
        }
    };

    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(COLUMN_GAP))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(STACK_GAP))
                .child(
                    question_card("card36-question", "Which postcode formats should validate?", options())
                        .subtitle("Claude needs this before editing the validator · pick all that apply")
                        .multi(true)
                        .allow_other(true)
                        .selected(current)
                        .on_select(toggle_option),
                )
                .child(answered_row("card36-answered", vec!["US ZIP".into(), "Canadian".into()])),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(STACK_GAP))
                .child(plan_card("card36-plan", plan_items()))
                .child(todo_list("card36-todo", tasks()).open(todo_open).on_toggle(toggle_todo)),
        )
        .into_any_element()
}
