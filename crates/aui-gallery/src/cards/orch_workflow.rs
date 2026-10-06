//! Orchestration · Native workflow: muse with a failed child carrying Retry
//! and Skip, and claude phases with the provisional pill.

use aui::orchestration::native_workflow;
use aui::protocol::{sample, Block};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its card, and between the states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let Block::NativeWorkflow(muse) = sample::workflow_muse_with_failed_child() else {
        unreachable!("sample::workflow_muse_with_failed_child is a workflow")
    };
    let Block::NativeWorkflow(claude) = sample::workflow_claude_phases() else {
        unreachable!("sample::workflow_claude_phases is a workflow")
    };
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(
            v_flex()
                .w_full()
                .child(caption("MUSE · FAILED CHILD WITH RETRY AND SKIP", &p))
                .child(native_workflow("orch-workflow-muse", &muse).on_intent(|_, _, _| {})),
        )
        .child(
            v_flex()
                .w_full()
                .child(caption("CLAUDE · PHASES WITH THE PROVISIONAL PILL", &p))
                .child(native_workflow("orch-workflow-claude", &claude).on_intent(|_, _, _| {})),
        )
        .into_any_element()
}
