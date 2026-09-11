//! Data · Secret field. The masked field beside its revealed twin, both
//! live: the eye flips the caller's state, which owns the text throughout.

use aui::data::secret_field;
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::base::input::InputState;

/// The sample key behind the dots.
const SAMPLE_KEY: &str = "sk-ant-9f2Kv7Qx3RmZ8tYvB4nM";
/// `.two{gap:24px}`.
const COLUMN_GAP: f32 = 24.0;
/// The fields render at the login card's width.
const FIELD_WIDTH: f32 = 420.0;
/// Caps over each field: 11 px ink-3 with a 10 px gutter below.
const CAPS_TEXT: f32 = scale::FS_11;
const CAPS_BOTTOM: f32 = 10.0;
/// The gap between the two fields.
const FIELD_GAP: f32 = 20.0;
/// `.legend{gap:10px;padding-top:6px;max-width:34ch}` — 34ch of Geist 12 ≈ 250 px.
const LEGEND_GAP: f32 = 10.0;
const LEGEND_TOP: f32 = 6.0;
const LEGEND_MEASURE: f32 = 262.0;

/// One live field: the card owns the state, as the harness does, and the eye
/// flips `set_masked` on it.
fn field(window: &mut Window, cx: &mut App, key: &'static str, masked: bool) -> AnyElement {
    let state = window.use_keyed_state(key, cx, |window, cx| {
        InputState::new(window, cx).masked(masked).placeholder("sk-ant-…").default_value(SAMPLE_KEY)
    });
    let toggle = state.clone();
    secret_field(format!("{key}-field"), &state)
        .placeholder("sk-ant-…")
        .on_toggle_reveal(move |window, cx| {
            let next = !toggle.read(cx).presentation().is_masked();
            toggle.update(cx, |state, cx| state.set_masked(next, window, cx));
        })
        .into_any_element()
}

/// A caps label over a field.
fn caps(p: Palette, text: &'static str) -> impl IntoElement {
    div().mb(px(CAPS_BOTTOM)).ui(CAPS_TEXT).text_color(p.ink_3).child(text)
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    h_flex()
        .w_full()
        .items_start()
        .gap(px(COLUMN_GAP))
        .child(
            v_flex()
                .flex_none()
                .w(px(FIELD_WIDTH))
                .child(caps(p, "Masked"))
                .child(field(window, cx, "data-secret-masked", true))
                .child(div().h(px(FIELD_GAP)))
                .child(caps(p, "Revealed"))
                .child(field(window, cx, "data-secret-revealed", false)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .max_w(px(LEGEND_MEASURE))
                .pt(px(LEGEND_TOP))
                .gap(px(LEGEND_GAP))
                .ui(scale::FS_12)
                .text_color(p.ink_2)
                .child(lead_paragraph(p, "Masked.", " The single-line input in the library's bordered box: 34 px, 6 px radius, surface-2, hairline border, mono text. The dots are the caller's secret rendered masked."))
                .child(lead_paragraph(p, "Reveal.", " The ghost eye reads the presentation snapshot to pick its glyph and reports back; the caller flips the masked flag. The ring lives on the box, never on the input."))
                .child(lead_paragraph(p, "Caller owns.", " The state and its text belong to the caller. The component never logs, copies or formats what is inside.")),
        )
        .into_any_element()
}

/// A paragraph whose first word is ink / 600.
fn lead_paragraph(p: Palette, lead: &'static str, rest: &'static str) -> impl IntoElement {
    let text = format!("{lead}{rest}");
    let highlight = HighlightStyle { color: Some(p.ink), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    div().child(StyledText::new(text).with_highlights([(0..lead.len(), highlight)]))
}
