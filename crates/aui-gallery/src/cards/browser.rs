//! Card 51 · Browser and annotator. The pane's tabs and nav row, a sample
//! page under annotate mode (hover outline, numbered pins, note popover, the
//! agent-action pill) and the annotations side panel. Reproduces
//! `design/src/cards/workbench/51-browser.html` at 980×640 (body padding 12).
//!
//! The page content here is gallery sample content: in the app the page is a
//! native webview and only the chrome and the overlays come from `aui`. The
//! page itself lives in `aui_webview::page`, so this card and the live
//! `workbench/webview` pane paint the same document.

use aui::data::{icon_button, ButtonSize};
use aui::shell::{tab_strip, TabItem};
use aui::workbench::{annotations_panel, browser_nav, Annotation};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui};
use aui_webview::page::page;
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `body.ds{padding:12px}` — the gallery frame adds 20, so the card pulls in
/// by 8 and sizes `.br{height:610px}` to the card width less 24.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 980.0 - 24.0;
const FRAME_H: f32 = 610.0;
/// `.tabs .btn.icon.xs svg{width:12px;height:12px}`.
const TAB_GLYPH: f32 = 12.0;

/// The pane's tabs.
fn tabs() -> Vec<TabItem> {
    vec![
        // The card draws the background tab without a close affordance (it
        // only appears on hover or on the active tab).
        TabItem::new("checkout", "localhost:3000/checkout", IconName::Globe).closable(false),
        TabItem::new("pricing", "localhost:3000/pricing", IconName::Globe),
    ]
}

/// The three annotations the side panel lists.
fn annotations() -> Vec<Annotation> {
    vec![
        Annotation::new(1, "h1 · “Simple pricing”", "Headline is fine; subtitle needs the free-trial length."),
        Annotation::new(2, "div.card.starter", "Make the Starter card stand out; it is the plan most people pick."),
        Annotation::new(3, "div.card.enterprise", "Typing…").pending(true),
    ]
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let action = |name: &'static str, glyph: IconName| icon_button(name, glyph).ghost().size(ButtonSize::Xs).icon_size(px(TAB_GLYPH));

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
            tab_strip("card51-tabs", tabs(), 1)
                .after_tabs(action("card51-plus", IconName::Plus))
                .trailing(action("card51-layout", IconName::Layout))
                .trailing(action("card51-link", IconName::Link))
                .trailing(action("card51-close", IconName::X)),
        )
        .child(browser_nav("card51-nav", "localhost:3000/pricing").annotating(true).on_action(|_, _, _| {}))
        .child(
            h_flex()
                .flex_1()
                .w_full()
                .min_h(px(0.0))
                .items_stretch()
                .child(page())
                .child(
                    annotations_panel("card51-panel", annotations())
                        .selected(Some(1))
                        .screenshot(Some(vec![(0.20, 0.30), (0.38, 0.55), (0.74, 0.55)]))
                        .on_action(|_, _, _| {}),
                ),
        )
        .into_any_element()
}
