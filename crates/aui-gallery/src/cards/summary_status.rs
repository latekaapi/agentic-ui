//! Card 38 · Summary, error and status rows: the turn summary with its changed
//! files and PR call to action, the retryable error card, two working lines,
//! the needs-you banner and the jump-to-latest pill. Reproduces
//! `design/src/cards/transcript/38-summary-error-status.html` at 760×600.

use aui::protocol::{sample, Block, ChangeKind, Check, FileChange};
use aui::transcript::{error_card, generic_item_card, goal_card, jump_pill, needs_you_banner, retry_row, status_row, summary_card, StatusLead};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// The card's block rhythm: `.sm{margin-bottom:14px}`, `.err{margin-bottom:18px}`,
/// the caps label's 8 px, `.status{margin-bottom:10px}` and `.need{margin-bottom:14px}`.
/// gpui does not collapse vertical margins, so the column spaces its own children.
const SUMMARY_GAP: f32 = 14.0;
const ERROR_GAP: f32 = 18.0;
const CAPS_GAP: f32 = 8.0;
const STATUS_GAP: f32 = 10.0;
const NEED_GAP: f32 = 14.0;
/// `.caps` here inherits the body line height.
const CAPS_LH: f32 = scale::LH_UI;
/// `.row{gap:var(--sp-3)}` around the jump pill and its note.
const ROW_GAP: f32 = scale::SP_3;

fn file(path: &str, change: ChangeKind, added: u32, removed: u32) -> FileChange {
    FileChange { path: path.into(), change, added, removed }
}

fn check(label: &str) -> Check {
    Check { label: label.into(), passed: true }
}

/// One sample [`Block::Goal`] as a card.
fn goal(id: &'static str, block: &Block) -> impl IntoElement {
    let Block::Goal { objective, status, percent_complete, current_work, next_work } = block else {
        unreachable!("sample::muse_goals yields goal blocks only")
    };
    let mut card = goal_card(id, objective.clone(), status.clone()).percent(*percent_complete);
    if let Some(work) = current_work {
        card = card.current_work(work.clone());
    }
    if let Some(work) = next_work {
        card = card.next_work(work.clone());
    }
    card
}

/// The mandated fallback for an item kind this build does not model.
fn generic(id: &'static str) -> impl IntoElement {
    let Block::Generic { kind, status, text } = sample::muse_generic_item() else {
        unreachable!("sample::muse_generic_item is a generic block")
    };
    generic_item_card(id, kind, status, text)
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let goals = sample::muse_goals();
    v_flex()
        .w_full()
        .child(
            div().mb(px(SUMMARY_GAP)).child(
                summary_card("card38-summary", "Done · address validation tightened", "4 m 12 s · $0.31")
                    .files(vec![
                        file("src/checkout/validators.ts", ChangeKind::Modified, 8, 3),
                        file("src/checkout/validators.test.ts", ChangeKind::Added, 41, 0),
                        file("src/checkout/AddressForm.tsx", ChangeKind::Modified, 2, 1),
                    ])
                    .checks(vec![check("27 tests pass"), check("lint clean"), check("typecheck")])
                    .on_action(|_, _, _| {}),
            ),
        )
        .child(
            div().mb(px(ERROR_GAP)).child(
                error_card("card38-error", "Anthropic API rate limited", "429 after 3 attempts · retrying automatically in 20 s ·")
                    .link("details", |_, _, _| {})
                    .on_retry(|_, _, _| {}),
            ),
        )
        .child(div().mb(px(CAPS_GAP)).text_role(TextRole::Caps).line_height(relative(CAPS_LH)).text_color(p.ink_3).child("STATUS ROWS"))
        .child(
            div().mb(px(STATUS_GAP)).child(
                status_row("card38-working", "Working…").lead(StatusLead::Spinner).shimmer(true).elapsed("12 s").key_hint("esc", "to interrupt"),
            ),
        )
        .child(div().mb(px(STATUS_GAP)).child(status_row("card38-tests", "Running tests").lead(StatusLead::Braille).elapsed("38 s").note("1 queued message")))
        .child(
            div().mb(px(NEED_GAP)).child(
                needs_you_banner("card38-need", "Claude is waiting for you.", "One approval and one question above.").at_rest().on_jump(|_, _, _| {}),
            ),
        )
        // The retry the provider scheduled: the same primitives as the working
        // line, with the countdown the host ticks and the reason it gave.
        .child(div().mb(px(STATUS_GAP)).child(retry_row("card38-retry", 2, 5, 4_200, "rate limited by the provider")))
        .child(
            h_flex()
                .w_full()
                .mb(px(NEED_GAP))
                .gap(px(ROW_GAP))
                .child(jump_pill("card38-jump", "Jump to latest").count(3).on_jump(|_, _, _| {}))
                .child(div().ui(scale::FS_12).text_color(p.ink_3).child("appears when the reader scrolls away from the tail; counts new turns")),
        )
        .child(div().mb(px(CAPS_GAP)).text_role(TextRole::Caps).line_height(relative(CAPS_LH)).text_color(p.ink_3).child("GOAL AND UNKNOWN ITEMS"))
        .child(div().mb(px(STATUS_GAP)).child(goal("card38-goal", &goals[0])))
        // A provider that reports 120 % has said something about itself worth
        // seeing: the number is verbatim and only the bar is clamped.
        .child(div().mb(px(STATUS_GAP)).child(goal("card38-goal-over", &goals[1])))
        .child(div().child(generic("card38-generic")))
        .into_any_element()
}
