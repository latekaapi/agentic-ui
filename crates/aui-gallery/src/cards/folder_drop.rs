//! `nav/folder-drop` · The folder drop card: dashed, centred, one at rest
//! and one drawn in its drag-over state with the keycap.

use aui::nav::{folder_drop_card, FolderDropCard};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.col{width:280px}`.
const COLUMN_W: f32 = 280.0;
/// `.wrap{gap:18px}`.
const WRAP_GAP: f32 = 18.0;
/// `.ttl{margin:0 0 8px}`.
const TITLE_GAP: f32 = 8.0;

/// One labelled card.
fn column(p: &Palette, title: &'static str, card: FolderDropCard) -> impl IntoElement {
    v_flex()
        .flex_none()
        .w(px(COLUMN_W))
        .child(div().mb(px(TITLE_GAP)).ui(scale::FS_12).text_color(p.ink_3).child(title))
        .child(card)
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    h_flex()
        .w_full()
        .items_start()
        .gap(px(WRAP_GAP))
        .child(column(&p, "At rest", folder_drop_card("folder-drop-rest")))
        .child(column(
            &p,
            "Drag over · ⌘⇧O to choose",
            folder_drop_card("folder-drop-drag").key("⌘⇧O").dragging(true),
        ))
        .into_any_element()
}
