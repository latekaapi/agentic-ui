//! Orchestration · Sidebar nesting: a gallery-only composition built with
//! the existing sidebar row API — no new library code. A lead run with its
//! nested workers, the peer row while pulsing and afterwards unread, and the
//! waiting sender row.
//!
//! `RowStatusKind` has no grey waiting kind, so the sender's waiting line is
//! composed with the existing muted placeholder slot.

use aui::nav::{compact_session_row, RowDensity, RowStatusKind, SessionSummary};
use aui_tokens::{ActiveAui, AgentState, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its rows, and between the states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// The lead run with its nested workers and the open-question status.
fn lead() -> SessionSummary {
    SessionSummary::new("login", "Login screen", AgentState::Waiting, "31m")
        .status(RowStatusKind::Asked, "1 question")
        .child(SessionSummary::new("plan", "Plan", AgentState::Done, "6m"))
        .child(
            SessionSummary::new("impl-form", "Implement · form", AgentState::Running, "12m").pulse(),
        )
        .child(
            SessionSummary::new("impl-auth", "Implement · auth", AgentState::Waiting, "4m")
                .status(RowStatusKind::Asked, ""),
        )
        .child(SessionSummary::new("review", "Review", AgentState::Idle, "0m"))
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    // The peer while it handles an incoming message, and the same row
    // afterwards with the unread dot.
    let peer_live =
        SessionSummary::new("audit-live", "Browser audit", AgentState::Running, "2m").pulse();
    let peer_after =
        SessionSummary::new("audit-after", "Browser audit", AgentState::Running, "6m").unread();
    let sender =
        SessionSummary::new("baaz", "Orchestration in Baaz", AgentState::Idle, "14:18")
            .placeholder("waiting");
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(
            v_flex()
                .w_full()
                .child(caption("LEAD · NEEDS-YOU WITH NESTED WORKERS", &p))
                .child(compact_session_row("orch-nesting-lead", lead()).density(RowDensity::One)),
        )
        .child(
            v_flex()
                .w_full()
                .child(caption("PEER · HANDLING AN INCOMING MESSAGE", &p))
                .child(compact_session_row("orch-nesting-peer-live", peer_live).density(RowDensity::One)),
        )
        .child(
            v_flex()
                .w_full()
                .child(caption("PEER · AFTERWARDS WITH THE UNREAD DOT", &p))
                .child(compact_session_row("orch-nesting-peer-after", peer_after).density(RowDensity::One)),
        )
        .child(
            v_flex()
                .w_full()
                .child(caption("SENDER · GREY WAITING STATUS", &p))
                .child(compact_session_row("orch-nesting-sender", sender).density(RowDensity::Two)),
        )
        .into_any_element()
}
