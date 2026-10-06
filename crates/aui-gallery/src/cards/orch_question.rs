//! Orchestration · Run question: open with one-tap answers, and answered as
//! one muted line.

use aui::orchestration::run_question;
use aui::protocol::{sample, Block};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its card, and between the two states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let Block::RunQuestion(open) = sample::run_question_open() else {
        unreachable!("sample::run_question_open is a question")
    };
    let Block::RunQuestion(answered) = sample::run_question_answered() else {
        unreachable!("sample::run_question_answered is a question")
    };
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(
            v_flex()
                .w_full()
                .child(caption("OPEN · RECOMMENDED ANSWER IS PRIMARY", &p))
                .child(run_question("orch-question-open", &open).on_intent(|_, _, _| {})),
        )
        .child(
            v_flex()
                .w_full()
                .child(caption("ANSWERED · ONE MUTED LINE", &p))
                .child(run_question("orch-question-answered", &answered).on_intent(|_, _, _| {})),
        )
        .into_any_element()
}
