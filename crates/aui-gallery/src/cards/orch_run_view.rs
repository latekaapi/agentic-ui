//! Orchestration · Run view: the task tree with clickable nodes and the
//! evidence table.

use aui::orchestration::run_view;
use aui::protocol::{EvidenceKind, EvidenceRow, Provider, RunViewModel, RunViewNode, TaskState};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between the caption and the view.
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above the view.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

fn node(task_id: &str, depth: u32, label: &str, state: TaskState, note: Option<&str>) -> RunViewNode {
    RunViewNode {
        task_id: task_id.into(),
        depth,
        label: label.into(),
        provider: Provider::Muse,
        state,
        note: note.map(Into::into),
    }
}

fn evidence(task_id: &str, text: &str, evidence_kind: EvidenceKind) -> EvidenceRow {
    EvidenceRow { task_id: task_id.into(), text: text.into(), evidence_kind }
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let view = RunViewModel {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        tree: vec![
            node("t-plan", 0, "Plan", TaskState::Accepted, None),
            node("t-impl-form", 1, "Implement · form", TaskState::Accepted, None),
            node("t-impl-auth", 1, "Implement · auth", TaskState::NeedsYou, Some("waiting on you")),
            node("t-review", 0, "Review", TaskState::Queued, Some("waits on both")),
            node("t-fix", 0, "Fix", TaskState::Queued, None),
            node("t-verify", 0, "Verify", TaskState::Queued, None),
        ],
        evidence: vec![
            evidence("t-impl-form", "T1 gate · npm test exited 0 · 14 passed", EvidenceKind::Test),
            evidence("t-impl-form", "T1 visual · 0 findings", EvidenceKind::Screenshot),
            evidence(
                "t-impl-form",
                "T1 scope · base 3db2220..a91f04c inside src/login/**",
                EvidenceKind::Review,
            ),
            evidence("t-impl-auth", "T2 escalated · spec_conflict → you", EvidenceKind::User),
        ],
    };
    v_flex()
        .w_full()
        .child(caption("LOGIN SCREEN · TASK TREE AND EVIDENCE", &p))
        .child(run_view("orch-run-view", &view).on_intent(|_, _, _| {}))
        .into_any_element()
}
