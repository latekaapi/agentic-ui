//! Card 42 · Attachments and drop: the five attachment row states beside the
//! transcript pane with the drag overlay on it.
//! Reproduces `design/src/cards/composer/42-attachments.html` at 760×460.

use aui::composer::{attachment_row, drop_overlay, AttachmentRowState, ROW_STACK_GAP};
use aui_icons::IconName;
use aui::protocol::AttachmentKind;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};
use super::composer::sample_thumbnail;

/// `.grid{grid-template-columns:1fr 1fr;gap:16px}`.
const COLUMN_GAP: f32 = scale::SP_5;
/// `.drop{height:300px}`.
const PANE_H: f32 = 300.0;
/// `.drop .lines{inset:16px;gap:12px}` at `opacity:.5`.
const LINES_INSET: f32 = 16.0;
const LINES_GAP: f32 = 12.0;
const LINES_OPACITY: f32 = 0.5;
/// `.drop .lines i{height:10px;border-radius:4px}` with the card's widths.
const LINE_H: f32 = 10.0;
const LINE_WIDTHS: [f32; 6] = [0.60, 0.90, 0.75, 0.40, 0.85, 0.65];
/// The uploading row sits at `width:68%`.
const UPLOAD_PROGRESS: f32 = 0.68;
/// `.ds-note{margin-top:12px;max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_TOP: f32 = 12.0;
const NOTE_MEASURE: f32 = 640.0;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut lines = v_flex()
        .absolute()
        .top(px(LINES_INSET))
        .left(px(LINES_INSET))
        .right(px(LINES_INSET))
        .bottom(px(LINES_INSET))
        .gap(px(LINES_GAP))
        .opacity(LINES_OPACITY);
    for width in LINE_WIDTHS {
        lines = lines.child(div().h(px(LINE_H)).w(relative(width)).rounded(px(scale::R_XS)).bg(p.surface_3));
    }

    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap(px(COLUMN_GAP))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap(px(ROW_STACK_GAP))
                        .child(
                            attachment_row("card42-image", "checkout-form.png", "PNG · 1.2 MB · 1440 × 900")
                                .kind(AttachmentKind::Image)
                                .thumbnail(sample_thumbnail())
                                .on_remove(|_, _| {}),
                        )
                        .child(
                            attachment_row("card42-upload", "design-spec.pdf", "")
                                .kind(AttachmentKind::File)
                                .glyph(IconName::Pdf)
                                .state(AttachmentRowState::Uploading(UPLOAD_PROGRESS))
                                .on_cancel(|_, _| {}),
                        )
                        .child(attachment_row("card42-source", "validators.ts", "from src/checkout · 180 lines").kind(AttachmentKind::Text).on_remove(|_, _| {}))
                        .child(
                            attachment_row("card42-failed", "recording.mov", "")
                                .kind(AttachmentKind::File)
                                .state(AttachmentRowState::Failed("Too large for this provider (max 32 MB)".into()))
                                .on_retry(|_, _| {}),
                        )
                        .child(attachment_row("card42-hint", "Paste an image or drop files", "⌘V with an image on the clipboard").state(AttachmentRowState::Hint)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .relative()
                        .h(px(PANE_H))
                        .rounded(px(scale::R_LG))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.bg)
                        .overflow_hidden()
                        .child(lines)
                        .child(drop_overlay("card42-drop", true).subtitle("3 files · images become screenshots the agent can see").at_rest()),
                ),
        )
        .child(
            div()
                .mt(px(NOTE_TOP))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Rows are 44 px with a 32 px thumb, name and one line of metadata. Uploading dims the thumb, shimmers the name and runs a hairline progress bar. The drop overlay covers the whole transcript pane with a dashed accent border and a bobbing arrow."),
        )
        .into_any_element()
}
