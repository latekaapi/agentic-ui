//! Handoff: the transcript card in three states, and the confirm dialog.
//!
//! The card column shows a move in progress (Muse → Claude Code, with the
//! step list, a spinning current step and its elapsed counter, plus Cancel),
//! one that failed (Claude Code → Codex, the failed step carrying its reason),
//! and one that landed (Claude Code → Codex, settled back to the state line
//! and the shared lists, with Open the new session). The confirm page shows
//! the modal the host puts up before starting.

use aui::protocol::{HandoffItem, HandoffState, Provider};
use aui::transcript::{default_handoff_steps, handoff_card, handoff_confirm, HandoffStep, HandoffStepState};
use gpui::*;
use gpui_kit::base::v_flex;

/// The column rhythm between the three cards.
const CARD_GAP: f32 = 14.0;

fn item(label: &str, detail: Option<&str>) -> HandoffItem {
    HandoffItem { label: label.into(), detail: detail.map(Into::into) }
}

fn carried() -> Vec<HandoffItem> {
    vec![
        item("Conversation summary", Some("the thread so far, condensed")),
        item("Recent turns", Some("last 12 turns verbatim")),
        item("Open todos", Some("3 of 7 done")),
        item("Touched files", Some("src/checkout/validators.ts +2")),
    ]
}

fn lost() -> Vec<HandoffItem> {
    vec![
        item("Tool state", Some("running shells stop here")),
        item("Pending approvals", None),
        item("Provider memory", Some("the old agent's own notes")),
    ]
}

/// The in-progress steps: the pack done, the summary running with its live
/// elapsed counter, the rest pending.
fn summarising_steps() -> Vec<HandoffStep> {
    let mut steps = default_handoff_steps(Provider::Claude);
    steps[0].state = HandoffStepState::Done;
    steps[1].state = HandoffStepState::Current;
    steps[1].detail = Some("6 s".into());
    steps
}

/// The failed steps: the pack done, the summary skipped as redundant, the
/// start failed with its reason, the confirm never reached.
fn failed_steps() -> Vec<HandoffStep> {
    let mut steps = default_handoff_steps(Provider::Codex);
    steps[0].state = HandoffStepState::Done;
    steps[1].state = HandoffStepState::Skipped;
    steps[2].state = HandoffStepState::Failed;
    steps[2].detail = Some("the destination was unreachable".into());
    steps
}

/// Builds the card content.
pub fn build(_window: &mut Window, _cx: &mut App) -> AnyElement {
    v_flex()
        .w_full()
        .gap(px(CARD_GAP))
        .child(
            handoff_card("handoff-progress", Provider::Muse, Provider::Claude, "opus 4.6", HandoffState::Checkpointed)
                .from_model("muse-spark-1.3")
                .carried(carried())
                .lost(lost())
                .pack_tokens(Some(8_400))
                .steps(summarising_steps())
                .on_intent(|_, _, _| {}),
        )
        .child(
            handoff_card(
                "handoff-failed",
                Provider::Claude,
                Provider::Codex,
                "gpt-5.6",
                HandoffState::Failed { reason: "the destination was unreachable".into() },
            )
            .from_model("opus 4.6")
            .carried(carried())
            .lost(lost())
            .pack_tokens(Some(9_050))
            .steps(failed_steps())
            .on_intent(|_, _, _| {}),
        )
        .child(
            handoff_card("handoff-active", Provider::Claude, Provider::Codex, "gpt-5.6", HandoffState::Activated)
                .from_model("opus 4.6")
                .carried(carried())
                .lost(lost())
                .pack_tokens(Some(11_200))
                .destination_session(Some("sess-codex-9f2".into()))
                .on_intent(|_, _, _| {}),
        )
        .into_any_element()
}

/// Builds the confirm dialog content: the modal over its scrim, static for
/// the capture — like the dialog card, the focus trap and `esc` belong to
/// the caller, so this composition has neither.
pub fn build_confirm(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div()
        .relative()
        .size_full()
        .child(
            handoff_confirm("handoff-confirm", Provider::Claude, "opus 4.6", &carried(), &lost(), Some(8_400))
                .at_rest()
                .on_primary(|_, _| {})
                .on_secondary(|_, _| {})
                .on_dismiss(|_, _| {}),
        )
        .into_any_element()
}
