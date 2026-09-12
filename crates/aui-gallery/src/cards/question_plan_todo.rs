//! Card 36 · Question, plan and todo cards: a multi-select question with an
//! "Other" row and its answered state, a proposed plan, and the task list with
//! morphing marks. Reproduces
//! `design/src/cards/transcript/36-question-plan-todo.html` at 800×760.

use aui::protocol::{sample, Block, QuestionOption, TodoItem, TodoState};
use aui::transcript::{answered_row, plan_card, question_card, todo_list, QuestionOutcome};
use gpui::*;
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::v_flex;
use gpui_kit::component::input::Textarea;

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
fn plan_items() -> Vec<SharedString> {
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

/// The countdown pill's sample clock: two minutes granted, 12 seconds left.
const TIMEOUT_TOTAL_MS: u64 = 120_000;
const TIMEOUT_LEFT_MS: u64 = 12_000;

/// The provider-driven question from the sample, as a card.
fn muse_question(id: &'static str) -> aui::transcript::QuestionCard {
    let Block::Question { header, prompt, subtitle, options, multi, allow_other, .. } = sample::muse_question() else {
        unreachable!("sample::muse_question is a question block")
    };
    question_card(id, prompt, &options).header(header).subtitle(subtitle).multi(multi).allow_other(allow_other)
}

/// The sectioned plan's steps.
fn section_items() -> Vec<SharedString> {
    let Block::Plan { items, .. } = sample::muse_plan() else { unreachable!("sample::muse_plan is a plan block") };
    items.into_iter().map(SharedString::from).collect()
}

/// Its heading labels.
fn sections() -> Vec<aui::protocol::PlanSection> {
    let Block::Plan { sections, .. } = sample::muse_plan() else { unreachable!("sample::muse_plan is a plan block") };
    sections
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let selected = window.use_keyed_state("card36-selected", cx, |_, _| vec![0usize, 1]);
    let previews = window.use_keyed_state("card36-previews", cx, |_, _| vec![0usize]);
    let clarify = window.use_keyed_state("card36-clarify-input", cx, |window, cx| {
        TextareaState::new(window, cx).placeholder("Say what you would rather I did").auto_grow(2, 4)
    });
    let toggle_preview = {
        let previews = previews.clone();
        move |index: usize, _: &mut Window, cx: &mut App| {
            previews.update(cx, |open, cx| {
                if let Some(at) = open.iter().position(|i| *i == index) {
                    open.remove(at);
                } else {
                    open.push(index);
                }
                cx.notify();
            })
        }
    };
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
                    question_card("card36-question", "Which postcode formats should validate?", &options())
                        .subtitle("Claude needs this before editing the validator · pick all that apply")
                        .multi(true)
                        .allow_other(true)
                        .selected(current)
                        .on_select(toggle_option),
                )
                .child(answered_row("card36-answered", vec!["US ZIP".into(), "Canadian".into()]))
                // The settlements that are not answers: MSP settles a prompt
                // six ways and only one of them fills the chips.
                .child(answered_row("card36-skipped", Vec::new()).outcome(QuestionOutcome::Skipped))
                .child(
                    answered_row("card36-clarified", Vec::new())
                        .outcome(QuestionOutcome::Clarified("Neither — describe whichever changed most recently.".into())),
                )
                .child(answered_row("card36-timed-out", Vec::new()).outcome(QuestionOutcome::TimedOut))
                // The provider-driven question: a header, a preview open on the
                // first option, the auto-resolution countdown, and the
                // "Explain instead" field the host owns.
                .child(
                    muse_question("card36-muse")
                        .previews_open(previews.read(cx).clone())
                        .timeout(TIMEOUT_LEFT_MS, TIMEOUT_TOTAL_MS)
                        .on_toggle_preview(toggle_preview)
                        .on_clarify(|_, _, _| {})
                        .on_skip(|_, _, _| {}),
                )
                .child(
                    muse_question("card36-muse-clarify")
                        .clarify_open(true)
                        .clarify_slot(Textarea::new(&clarify).text_size(aui_tokens::scaled(aui_tokens::scale::FS_12)))
                        .on_clarify(|_, _, _| {})
                        .on_skip(|_, _, _| {}),
                ),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(STACK_GAP))
                .child(plan_card("card36-plan", &plan_items()))
                // The same plan under its markdown headings: the labels are
                // unnumbered rows and the numbering still counts steps only.
                .child(plan_card("card36-plan-sections", &section_items()).sections(sections()))
                .child(todo_list("card36-todo", tasks()).open(todo_open).on_toggle(toggle_todo)),
        )
        .into_any_element()
}
