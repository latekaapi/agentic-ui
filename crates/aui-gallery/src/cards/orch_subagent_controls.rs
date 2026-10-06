//! Orchestration · Subagent controls: the muse row and a provider with no
//! native controls.

use aui::orchestration::subagent_controls;
use aui::protocol::SubagentControls;
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its row, and between the states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    // Muse offers message, interrupt and stop, but no follow-up.
    let muse = SubagentControls { message: true, follow_up: false, interrupt: true, stop: true };
    // Claude and Codex offer no native controls: the row draws nothing.
    let none = SubagentControls { message: false, follow_up: false, interrupt: false, stop: false };
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(
            v_flex()
                .w_full()
                .child(caption("MUSE · MESSAGE, INTERRUPT, STOP", &p))
                .child(subagent_controls("orch-subagent-muse", &muse).on_intent(|_, _, _| {})),
        )
        .child(
            v_flex()
                .w_full()
                .child(caption("NO NATIVE CONTROLS ON THIS PROVIDER", &p))
                .child(subagent_controls("orch-subagent-none", &none).on_intent(|_, _, _| {})),
        )
        .into_any_element()
}
