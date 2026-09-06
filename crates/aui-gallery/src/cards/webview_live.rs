//! `workbench/webview` — the browser pane driven by a live `WebBackend`.
//!
//! Card 51 is a still of this pane; this is the pane itself, over
//! [`FakeWebBackend`]. The chrome around it — the frame and the tab strip — is
//! card 51's, so the two entries can be compared side by side, but everything
//! inside comes from `aui-webview`: the nav row acts, the pointer outlines
//! elements as it moves over the page, a click drops a numbered pin and opens
//! its note, and the annotations panel collects them.
//!
//! It opens in the state card 51 froze: annotate mode on, two elements already
//! pinned, the panel open. The seeds go through the same click path a person
//! would, so nothing here is a second way of making an annotation.

use aui::data::{icon_button, ButtonSize};
use aui::shell::{tab_strip, TabItem};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui};
use aui_webview::{webview_pane, FakeWebBackend, WebviewState};
use gpui::*;
use gpui_kit::base::v_flex;

/// The card is 980×640 and the gallery frame adds 20, so the pane pulls in by
/// 8 the way card 51 does and fills the rest.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 980.0 - 24.0;
const FRAME_H: f32 = 610.0;
/// `.tabs .btn.icon.xs svg{width:12px;height:12px}`.
const TAB_GLYPH: f32 = 12.0;

/// The two pins the card opens with, as page coordinates and their notes. The
/// points fall inside the headline and the Starter card of the fake page.
const SEEDS: [((f32, f32), &str); 2] = [
    ((120.0, 44.0), "Headline is fine; subtitle needs the free-trial length."),
    ((110.0, 180.0), "Make the Starter card stand out; it is the plan most people pick."),
];

/// The pane's tabs, as card 51 shows them.
fn tabs() -> Vec<TabItem> {
    vec![
        TabItem::new("checkout", "localhost:3000/checkout", IconName::Globe).closable(false),
        TabItem::new("pricing", "localhost:3000/pricing", IconName::Globe),
    ]
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let action = |name: &'static str, glyph: IconName| icon_button(name, glyph).ghost().size(ButtonSize::Xs).icon_size(px(TAB_GLYPH));

    let state = window.use_keyed_state(SharedString::from("webview-live"), cx, |_, cx| {
        let mut state = WebviewState::new(Box::new(FakeWebBackend::new()), cx);
        state.set_annotate(true);
        state.seed_pins(&SEEDS);
        state
    });

    v_flex()
        .w(px(FRAME_W))
        .h(px(FRAME_H))
        .flex_none()
        .m(px(FRAME_INSET))
        .overflow_hidden()
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line_strong)
        .bg(p.surface_1)
        .child(
            tab_strip("webview-live-tabs", tabs(), 1)
                .after_tabs(action("webview-live-plus", IconName::Plus))
                .trailing(action("webview-live-layout", IconName::Layout))
                .trailing(action("webview-live-link", IconName::Link))
                .trailing(action("webview-live-close", IconName::X)),
        )
        .child(webview_pane("webview-live-pane", &state).on_intent(|_, _, _| {}))
        .into_any_element()
}
