//! The handoff confirm dialog shows structured lists, not a paragraph.
//!
//! The confirm reuses the card's list parts over the existing modal: a
//! destination row with the provider mark, the Carried / Not carried rows,
//! the pack size, and the honest note as its own paragraph — so the dialog
//! must carry no single-paragraph body. The model must belong to the
//! destination provider (Claude Code → an Opus/Sonnet id, Codex → gpt-5.6),
//! which the destination line pins per provider.

use aui::protocol::{HandoffItem, Provider};
use aui::transcript::{handoff_confirm, handoff_confirm_destination, HANDOFF_CONFIRM_WIDTH, HANDOFF_FRESH_NOTE};

fn item(label: &str, detail: Option<&str>) -> HandoffItem {
    HandoffItem { label: label.into(), detail: detail.map(Into::into) }
}

fn carried() -> Vec<HandoffItem> {
    vec![
        item("Conversation summary", Some("the thread so far, condensed")),
        item("Recent turns", Some("last 12 turns verbatim")),
    ]
}

fn lost() -> Vec<HandoffItem> {
    vec![item("Tool state", Some("running shells stop here")), item("Pending approvals", None)]
}

/// The destination line names a fresh session on the destination provider
/// with its own model id — never the other provider's.
#[test]
fn destination_line_pairs_each_provider_with_its_own_model() {
    assert_eq!(
        handoff_confirm_destination(Provider::Claude, "opus 4.6").to_string(),
        "Starts a new Claude Code session · opus 4.6"
    );
    assert_eq!(
        handoff_confirm_destination(Provider::Codex, "gpt-5.6").to_string(),
        "Starts a new Codex session · gpt-5.6"
    );
}

/// The confirm draws the shared lists through a structured body, so no
/// single-paragraph body may remain.
#[test]
fn confirm_has_a_structured_body_and_no_paragraph() {
    let dialog = handoff_confirm("handoff-confirm", Provider::Claude, "opus 4.6", &carried(), &lost(), Some(8_400));
    assert!(dialog.has_rich_body(), "the confirm must draw the shared lists, not a paragraph");
    assert!(
        dialog.body_text().is_none(),
        "the old Will carry / Will not carry paragraph must be gone, got {:?}",
        dialog.body_text()
    );
}

/// The dialog is wide enough for the list rows to fit without wrapping
/// their labels.
#[test]
fn confirm_width_fits_the_lists() {
    let dialog = handoff_confirm("handoff-confirm", Provider::Claude, "opus 4.6", &carried(), &lost(), Some(8_400));
    assert!(dialog.width_px() >= 500.0, "the lists need room, got {}", dialog.width_px());
    assert_eq!(dialog.width_px(), HANDOFF_CONFIRM_WIDTH);
}

/// The honest note still travels with the confirm.
#[test]
fn confirm_keeps_the_honest_fresh_start_note() {
    assert!(HANDOFF_FRESH_NOTE.contains("starts fresh"));
}
