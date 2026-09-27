//! Handoff: the transcript card in three states, and the confirm dialog.
//!
//! The card column shows a move still preparing (Muse → Claude Code, with
//! Cancel), one that landed (Claude Code → Codex, with Open the new session),
//! and one the destination refused. The confirm page shows the modal the host
//! puts up before starting.

use aui::protocol::{HandoffItem, HandoffState, Provider};
use aui::transcript::{handoff_card, handoff_confirm};
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

/// Builds the card content.
pub fn build(_window: &mut Window, _cx: &mut App) -> AnyElement {
    v_flex()
        .w_full()
        .gap(px(CARD_GAP))
        .child(
            handoff_card("handoff-preparing", Provider::Muse, Provider::Claude, "opus 4.6", HandoffState::Prepared)
                .from_model("muse-spark-1.3")
                .carried(carried())
                .lost(lost())
                .pack_tokens(Some(8_400))
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
        .child(
            handoff_card(
                "handoff-refused",
                Provider::Claude,
                Provider::Codex,
                "gpt-5.6",
                HandoffState::Refused { reason: "A question is waiting for your answer".into() },
            )
            .from_model("opus 4.6")
            .carried(carried())
            .lost(lost())
            .pack_tokens(Some(9_050))
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
