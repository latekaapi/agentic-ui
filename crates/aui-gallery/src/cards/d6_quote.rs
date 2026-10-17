//! The quote flow (docs/43 §0; canvas v49 `D4QuoteSelect`,
//! `D4QuoteCompose`, `D4Quote`): the selection bar over a selected passage,
//! quote blocks in the composer (prose and code), and the sent bubble's quote.

use aui::composer::{composer, composer_state_rows, ComposerQuote};
use aui::protocol::{Attachment, AttachmentKind, UploadState};
use aui::transcript::{quote_bar, user_turn};
use aui_icons::Provider;
use aui_tokens::{ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

fn quote(name: &str, meta: Option<&str>) -> Attachment {
    Attachment { name: name.into(), kind: AttachmentKind::Quote, size_bytes: None, meta: meta.map(Into::into), state: UploadState::Ready, path: None, quote_of: Some("turn-1".into()) }
}

pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let label = |t: &'static str| div().text_px(12.0).text_color(p.fg_muted).child(t);
    let bar = quote_bar("d6-qbar", true, |_, _, _| {}, window, cx);
    let selected = div()
        .relative()
        .w(px(560.0))
        .pt(px(44.0))
        .text_px(16.0)
        .line_height(px(26.0))
        .text_color(p.fg)
        .child(div().absolute().left(px(0.0)).top(px(4.0)).children(bar))
        .child(div().child("CA and GB now branch per country, ").child(div().bg(gpui::rgba(0x3D72F245)).child("with focused tests over both.")));

    let state = window.use_keyed_state("d6-q-field", cx, |w, cx| {
        let mut s = composer_state_rows("Ask Muse, or type / for commands", 1, 8, w, cx);
        s.set_value("Why does GB still fail on the space? Fix it the same way as CA.", w, cx);
        s
    });
    let composer = composer("d6-q-composer", &state, Provider::Muse, "Muse Spark 1.3")
        .docked(true)
        .raised(true)
        .mode("Auto")
        .effort("Default")
        .can_send(true)
        .quotes(vec![
            ComposerQuote { id: "q1".into(), text: "with focused tests over both. • CA outward codes accept the space form • GB outward codes still fail on the space".into(), code: None },
            ComposerQuote { id: "q2".into(), text: "Country::Ca => CA_RE.is_match(code),\nCountry::Gb => GB_RE.is_match(code),".into(), code: Some(("rust".into(), 2)) },
        ]);

    let sent = v_flex()
        .w(px(560.0))
        .gap(px(16.0))
        .child(user_turn("d6-q-sent", "Why does GB still fail on the space? Fix it the same way as CA.").attachments(vec![quote(
            "with focused tests over both. • CA outward codes accept the space form • GB outward codes still fail on the space",
            None,
        )]).on_quote(|_, _, _| {}))
        .child(user_turn("d6-q-sent-code", "Is the order of these two arms right?").attachments(vec![quote("Country::Ca => CA_RE.is_match(code),\nCountry::Gb => GB_RE.is_match(code),", Some("code:rust"))]).on_quote(|_, _, _| {}));

    v_flex()
        .gap(px(20.0))
        .child(label("The selection bar"))
        .child(selected)
        .child(label("Quotes in the composer"))
        .child(div().w(px(760.0)).child(composer))
        .child(label("Sent"))
        .child(h_flex().child(sent))
        .into_any_element()
}
