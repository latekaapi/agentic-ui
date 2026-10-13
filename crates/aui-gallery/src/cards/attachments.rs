//! `composer/attachments` · the composer's states (D4, docs/41 §2.9, the
//! D4Composer board): the raised composer empty, typed, with attachment
//! chips, with a quotation, with chips uploading and failed, running with a
//! queued line above it — the send button the composer's only solid fill —
//! and the drop overlay over a sheet.

use aui::composer::{composer, composer_state_rows, drop_overlay, queue_strip, ComposerChip, ComposerChipKind, ComposerChipState, QueueStripRow};
use aui_icons::Provider;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::v_flex;

use super::composer::sample_thumbnail;

/// The composers stack 18 apart; the drop sheet is 240 tall.
const GAP: f32 = 18.0;
const SHEET_H: f32 = 240.0;
const NOTE_MEASURE: f32 = 640.0;

const PLACEHOLDER: &str = "Ask Muse, or type / for commands";

fn chip(id: &str, kind: ComposerChipKind, label: &str, detail: Option<&str>, thumb: bool, state: ComposerChipState) -> ComposerChip {
    ComposerChip {
        id: id.to_owned().into(),
        kind,
        label: label.to_owned().into(),
        removable: true,
        thumbnail: thumb.then(sample_thumbnail),
        detail: detail.map(|d| d.to_owned().into()),
        state,
    }
}

fn field(key: &'static str, text: &'static str, window: &mut Window, cx: &mut App) -> Entity<TextareaState> {
    window.use_keyed_state(key, cx, move |window, cx| {
        let mut state = composer_state_rows(PLACEHOLDER, 1, 8, window, cx);
        if !text.is_empty() {
            state.set_value(text, window, cx);
        }
        state
    })
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let empty = field("card42-empty", "", window, cx);
    let typed = field("card42-typed", "Tighten the GB regex so the space form passes too, then rerun the focused tests and show me the diff before committing anything.", window, cx);
    let attached = field("card42-attached", "Tighten the GB regex so the space form passes too.", window, cx);
    let quoted = field("card42-quoted", "Try that again.", window, cx);
    let states = field("card42-states", "Here is the capture and the fixture.", window, cx);
    let running = field("card42-running", "", window, cx);

    let raised = |id: &'static str, state: &Entity<TextareaState>| composer(id, state, Provider::Muse, "Muse Spark 1.3").docked(true).raised(true).mode("Auto").effort("Default");
    let label = |text: &'static str| div().ui(scale::FS_12).text_color(p.ink_3).child(text);
    v_flex()
        .w_full()
        .gap(px(GAP))
        .child(label("Empty"))
        .child(raised("card42-c-empty", &empty).can_send(false))
        .child(label("Typed — the send button takes the one solid fill"))
        .child(raised("card42-c-typed", &typed).can_send(true))
        .child(label("With attachments"))
        .child(raised("card42-c-attached", &attached).can_send(true).chips(vec![
            chip("img", ComposerChipKind::Image, "transcript-real.png", None, true, ComposerChipState::Ready),
            chip("file", ComposerChipKind::File, "synthetic-markdown.jsonl", Some("JSONL · 2.4 KB"), false, ComposerChipState::Ready),
        ]))
        .child(label("With a quotation"))
        .child(raised("card42-c-quote", &quoted).can_send(true).chips(vec![chip(
            "quote",
            ComposerChipKind::Quote,
            "the provider closed the stream before…",
            None,
            false,
            ComposerChipState::Ready,
        )]))
        .child(label("Attachment states: uploading, ready, failed"))
        .child(raised("card42-c-states", &states).can_send(true).chips(vec![
            chip("up", ComposerChipKind::Image, "transcript-real.png", None, true, ComposerChipState::Uploading),
            chip("pdf", ComposerChipKind::File, "notes.pdf", Some("PDF · 1.1 MB"), false, ComposerChipState::Ready),
            chip("bad", ComposerChipKind::File, "archive.zip", None, false, ComposerChipState::Failed("couldn't be read".into())),
        ]))
        .child(label("Running, a message queued"))
        .child(
            v_flex()
                .w_full()
                .gap(px(8.0))
                .child(queue_strip("card42-queue", vec![QueueStripRow::new("q1", "Also update the changelog once the tests pass.")]).on_intent(|_, _, _, _| {}))
                .child(raised("card42-c-running", &running).streaming(true)),
        )
        .child(label("Drop over the sheet"))
        .child(div().relative().w_full().h(px(SHEET_H)).rounded(px(8.0)).bg(p.surface_2).child(drop_overlay("card42-drop", true).at_rest()))
        .child(
            div()
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("The composer's states on the approved card. Chips sit above the field, 6 apart, inset 8: Raised-lit, radius 6, 32 tall; an image leads with its thumbnail, a file with its glyph and type · size, a quote with its first words. Uploading dims the thumbnail and sweeps a 2 px line along the chip's bottom; failed turns the name danger with the reason as its meta. Send is round 28: Floating with a muted arrow while empty, the foreground fill once there is something to send, the same fill with a 10 px square while a turn runs."),
        )
        .into_any_element()
}
