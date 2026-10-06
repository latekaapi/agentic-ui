//! Orchestration · Run view: the task tree with clickable nodes and the
//! evidence table.

use aui::orchestration::run_view;
use aui::protocol::sample;
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between the caption and the view.
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above the view.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let view = sample::run_view_sample();
    v_flex()
        .w_full()
        .child(caption("LOGIN SCREEN · TASK TREE AND EVIDENCE", &p))
        .child(run_view("orch-run-view", &view).on_intent(|_, _, _| {}))
        .into_any_element()
}
