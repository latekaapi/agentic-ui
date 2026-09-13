//! Project marks. The eight label colours at the three sizes — 14 for menu
//! rows and the palette, 18 for the header and group rows, 22 for the rail —
//! in both themes.

use aui::nav::project_mark;
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// The ramp's initials, red through pink.
const INITIALS: [&str; 8] = ["R", "O", "Y", "G", "T", "B", "V", "P"];
/// The three sizes and where each is used.
const SIZES: [(f32, &str); 3] = [
    (14.0, "14 · menu rows and the palette"),
    (18.0, "18 · the header and group rows"),
    (22.0, "22 · the rail"),
];
/// The gap between marks, and between a caption and its marks.
const MARK_GAP: f32 = 8.0;
const ROW_GAP: f32 = 16.0;

/// One size row: its caption and the eight marks.
fn size_row(p: Palette, size: f32, caption: &'static str) -> impl IntoElement {
    let mut marks = h_flex().gap(px(MARK_GAP));
    for (i, initial) in INITIALS.into_iter().enumerate() {
        marks = marks.child(project_mark(initial, p.label(i as u8)).size(px(size)));
    }
    v_flex()
        .gap(px(MARK_GAP))
        .child(div().ui(scale::FS_12).text_color(p.ink_3).child(caption))
        .child(marks)
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut col = v_flex().gap(px(ROW_GAP));
    for (size, caption) in SIZES {
        col = col.child(size_row(p, size, caption));
    }
    col.child(
        div()
            .mt(px(ROW_GAP))
            .max_w(px(632.0))
            .text_role(TextRole::UiSmall)
            .text_color(cx.aui().colors.ink_3)
            .child(
                "One shape identifies a project everywhere: the header switcher, the group rows, the palette and the rail all draw this mark. \
                 The colour is identity, never status — it comes from the eight-colour label ramp, assigned round-robin and changeable from the project menu.",
            ),
    )
    .into_any_element()
}
