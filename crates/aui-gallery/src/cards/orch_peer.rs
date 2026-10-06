//! Orchestration · Peer message: outgoing waiting, replied collapsed and
//! expanded, queued, unreachable, and the incoming bubble.

use aui::orchestration::peer_message;
use aui::protocol::{sample, Block};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The gap between a caption and its row, and between the six states.
const SECTION_GAP: f32 = 14.0;
const CAPTION_GAP: f32 = 6.0;

/// A small muted caption above each state.
fn caption(text: &str, p: &aui_tokens::Palette) -> impl IntoElement {
    div().mb(px(CAPTION_GAP)).text_role(TextRole::Meta).text_color(p.ink_3).child(text.to_owned())
}

/// One captioned state.
fn state(
    id: &'static str,
    note: &str,
    block: &Block,
    expanded: bool,
    p: &aui_tokens::Palette,
) -> impl IntoElement {
    let Block::PeerMessage(message) = block else { unreachable!("orchestration peer samples are peer messages") };
    v_flex()
        .w_full()
        .child(caption(note, p))
        .child(peer_message(id, message).expanded(expanded).on_intent(|_, _, _| {}))
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let waiting = sample::peer_outgoing_waiting();
    let replied = sample::peer_outgoing_replied();
    let queued = sample::peer_outgoing_queued();
    let unreachable = sample::peer_outgoing_unreachable();
    let incoming = sample::peer_incoming();
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .child(state("orch-peer-waiting", "OUTGOING · WAITING FOR REPLY", &waiting, false, &p))
        .child(state("orch-peer-replied", "REPLIED · COLLAPSED TO TWO LINES", &replied, false, &p))
        .child(state("orch-peer-expanded", "REPLIED · EXPANDED", &replied, true, &p))
        .child(state("orch-peer-queued", "QUEUED · RECEIVER BUSY", &queued, false, &p))
        .child(state("orch-peer-unreachable", "UNREACHABLE · DANGER INK", &unreachable, false, &p))
        .child(state("orch-peer-incoming", "INCOMING · ANOTHER SESSION, NOT YOU", &incoming, false, &p))
        .into_any_element()
}
