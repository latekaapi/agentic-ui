//! Card 30 · Header identity and markers. Two centre-header rows and the
//! four marker kinds. Reproduces
//! `design/src/cards/transcript/30-session-header.html` at 760×380.

use aui::data::{pill, PillVariant};
use aui::shell::centre_header;
use aui::transcript::{marker_row, HandOff};
use aui_icons::{IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.hd{border:1px solid line;border-radius:8px;margin-bottom:18px}` around each header row.
const HEADER_GAP: f32 = 18.0;
/// `.hd{height:44px}` including its border.
const HEADER_H: f32 = 44.0;
/// The first caps label: `margin-bottom:8px`. The second's `margin:6px 0 4px`
/// collapses into the header's 18 px above and the marker's 10 px below.
const CAPS_BOTTOM: f32 = 8.0;
const CAPS2_TOP: f32 = 0.0;
const CAPS2_BOTTOM: f32 = 0.0;
/// `.pill.warning{height:18px}` beside the waiting worktree.
const WAITING_PILL: f32 = 18.0;
/// `.ds-note{max-width:80ch}` ≈ 616 px of Geist 12 (matches the reference wrap).
const NOTE_MEASURE: f32 = 626.0;
/// `.caps` in this card inherits the body line height.
const CAPS_LH: f32 = scale::LH_UI;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .child(caps(p, "Centre header cell").mb(px(CAPS_BOTTOM)))
        .child(header_frame(p, centre_header("card30-hd-1", "checkout-flow-v2").provider(Provider::Claude).branch("feature/checkout-flow-v2")))
        .child(header_frame(
            p,
            centre_header("card30-hd-2", "infra/notifier")
                .provider(Provider::Codex)
                .branch("main")
                .trailing(pill("waiting").variant(PillVariant::Warning).height(WAITING_PILL)),
        ))
        .child(caps(p, "Marker rows").mt(px(CAPS2_TOP)).mb(px(CAPS2_BOTTOM)))
        .child(marker_row("card30-mk-1").glyph(IconName::Clock, None).text("Today · 09:14 · session started · Claude Code v2.1.174"))
        .child(
            marker_row("card30-mk-2")
                .hand_off(HandOff { from: (Provider::Claude, "Claude Code".into()), to: (Provider::Codex, "Codex".into()) })
                .text("handed off with full context"),
        )
        .child(marker_row("card30-mk-3").glyph(IconName::Refresh, None).text("Context compacted · 41k → 12k tokens ·").link("view summary", |_, _, _| {}))
        .child(marker_row("card30-mk-4").glyph(IconName::Shield, Some(p.warning)).text("Permission mode changed to").strong("Bypass").text("for this session"))
        .child(
            div()
                .mt(px(scale::SP_4))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("There is no session header inside the transcript. The provider mark sits beside the worktree name in the centre header, the model and mode chips live in the composer, and facts like the CLI version or a hand-off appear once as marker rows when they change."),
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
