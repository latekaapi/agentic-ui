//! Card 33 · Agent activity group: live with its timeline open, done, and
//! failed. Reproduces `design/src/cards/transcript/33-activity-group.html` at 760×560.

use aui::protocol::{ActivityState, Step, StepState};
use aui::transcript::activity_group;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.ag{margin-bottom:16px}`.
const BLOCK_GAP: f32 = 16.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

fn step(verb: &str, target: &str, state: StepState, result: Option<&str>) -> Step {
    Step { verb: verb.into(), target: target.into(), state, result: result.map(Into::into) }
}

/// The steps of the live group.
pub fn live_steps() -> Vec<Step> {
    vec![
        step("Searched", "validateAddress in src/checkout", StepState::Done, Some("2 hits")),
        step("Read", "src/checkout/validators.ts", StepState::Done, Some("180 lines")),
        step("Edited", "src/checkout/validators.ts", StepState::Done, Some("+8 −3")),
        step("Running", "pnpm vitest run src/checkout", StepState::Running, Some("12 s")),
        step("Lint", "eslint on touched files", StepState::Pending, Some("queued")),
    ]
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let open = window.use_keyed_state("card33-open", cx, |_, _| [true, false, false]);
    let current = *open.read(cx);
    let toggle = |index: usize| {
        let open = open.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            open.update(cx, |o, cx| {
                o[index] = !o[index];
                cx.notify();
            })
        }
    };
    let done: Vec<Step> = (0..6).map(|i| step("Step", &format!("{i}"), StepState::Done, None)).collect();
    let failed = vec![
        step("Step", "1", StepState::Done, None),
        step("Step", "2", StepState::Done, None),
        step("Step", "3", StepState::Failed, None),
        step("Step", "4", StepState::Done, None),
    ];
    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(activity_group("card33-live", live_steps(), "Running tests", "38 s", ActivityState::Working).open(current[0]).on_toggle(toggle(0)))
        .child(
            activity_group("card33-done", done, "Ran 3 commands", "1 m 04 s", ActivityState::Done)
                .detail("· read 2 files · edited 1 file")
                .open(current[1])
                .on_toggle(toggle(1)),
        )
        .child(activity_group("card33-failed", failed, "Tests failed", "52 s", ActivityState::Failed).detail("· 2 of 27 · ran 4 commands").open(current[2]).on_toggle(toggle(2)))
        .child(
            div()
                .mt(px(scale::SP_4) - px(BLOCK_GAP))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("The header shows a shimmering status while working and a plain count when done; the row of step glyphs is the whole run at a glance. Expanded, each step is one line with verb, mono target and a right-aligned result. Steps pulse in accent while running and never show raw output here, that lives in the tool card."),
        )
        .into_any_element()
}
