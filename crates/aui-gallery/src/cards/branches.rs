//! Branch from here (harness docs/85 16.3, rule 30: the mock before the
//! build): a user message with two branches started at it — the count at
//! rest under the bubble, and the rail's Branch button beside Edit and Copy
//! (shown on hover; drawn here as the app would).

use aui::transcript::{assistant_turn, user_turn, UserTurnAction};
use aui_tokens::{scale, ActiveAui};
use gpui::{prelude::*, px, AnyElement, App, Window};
use gpui_kit::base::v_flex;

/// `branches/message`.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w(px(720.0))
        .p(px(scale::SP_6))
        .gap(px(scale::SP_4))
        .bg(p.bg)
        .child(
            user_turn("branch-msg", "Try the parser with a hand-written state machine instead of the regex.")
                .actions(&[UserTurnAction::Edit, UserTurnAction::Copy, UserTurnAction::Branch])
                .branches(Some("2 branches".into())),
        )
        .child(assistant_turn("branch-reply", "Here is a state machine for the date grammar: three states, no backtracking."))
        .into_any_element()
}
