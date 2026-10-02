//! Shell · right header tabs. The right header cell carrying a tab strip in
//! place of a title — Changes · Files · Browser with a close control — and,
//! below it, the title variant with no tabs.

use aui::shell::{right_header, tab_strip, TabItem};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.hd{border:1px solid line;border-radius:8px;margin-bottom:18px}` around each header row.
const HEADER_GAP: f32 = 18.0;
/// `.hd{height:44px}` including its border.
const HEADER_H: f32 = 44.0;
/// The caps labels' `margin-bottom:8px`.
const CAPS_BOTTOM: f32 = 8.0;
/// `.ds-note{max-width:80ch}` ≈ 616 px of Geist 12.
const NOTE_MEASURE: f32 = 626.0;
/// `.caps` in this card inherits the body line height.
const CAPS_LH: f32 = scale::LH_UI;

const TAB_IDS: [&str; 3] = ["changes", "files", "browser"];

fn tabs() -> Vec<TabItem> {
    vec![
        TabItem::new("changes", "Changes", IconName::Git).closable(false),
        TabItem::new("files", "Files", IconName::Folder).closable(false),
        TabItem::new("browser", "Browser", IconName::Globe).closable(false),
    ]
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let active = window.use_keyed_state("card-rh-tab", cx, |_, _| 0usize);
    let current = *active.read(cx);
    let select = active.clone();
    let strip = tab_strip("card-rh-tabs", tabs(), current)
        .accessibility_label("Right pane")
        .on_select(move |id, _, cx| {
            let index = TAB_IDS.iter().position(|t| *t == id.as_ref()).unwrap_or(0);
            select.update(cx, |value, cx| {
                if *value != index {
                    *value = index;
                    cx.notify();
                }
            });
        });
    v_flex()
        .w_full()
        .child(caps(p, "Right header · tab strip, no title").mb(px(CAPS_BOTTOM)))
        .child(header_frame(p, right_header("card-rh-hd-tabs").tabs(strip).on_close(|_, _, _| {})))
        .child(caps(p, "Right header · title, no tabs").mb(px(CAPS_BOTTOM)))
        .child(header_frame(p, right_header("card-rh-hd-title").title("Files").on_close(|_, _, _| {})))
        .child(
            div()
                .mt(px(scale::SP_4))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("A tabbed right pane needs no title: the strip stands in its place and the trailing close is unchanged. The strip is one tab stop with left and right arrows moving the selection like a click; it announces as a “Right pane” tab list and each tab announces its label and selected state."),
        )
        .into_any_element()
}

fn caps(p: Palette, label: &'static str) -> Div {
    div().text_role(TextRole::Caps).line_height(relative(CAPS_LH)).text_color(p.ink_3).child(label.to_uppercase())
}

/// The card frames each header row like a cell: line border, radius 8,
/// surface-1, 44 px border-box (so the row inside is clipped by the border).
fn header_frame(p: Palette, header: impl IntoElement) -> impl IntoElement {
    div()
        .w_full()
        .h(px(HEADER_H))
        .mb(px(HEADER_GAP))
        .rounded(px(scale::R_MD))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .overflow_hidden()
        .child(header)
}
