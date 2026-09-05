//! Card 51 · Browser and annotator. The pane's tabs and nav row, a sample
//! page under annotate mode (hover outline, numbered pins, note popover, the
//! agent-action pill) and the annotations side panel. Reproduces
//! `design/src/cards/workbench/51-browser.html` at 980×640 (body padding 12).
//!
//! The page content here is gallery sample content: in the app the page is a
//! native webview and only the chrome and the overlays come from `aui`.

use aui::data::{icon_button, ButtonSize};
use aui::shell::{tab_strip, TabItem};
use aui::workbench::{agent_action_pill, annotation_pin, annotations_panel, browser_nav, element_outline, note_popover, Annotation};
use aui_icons::IconName;
use aui_tokens::{light, scale, ActiveAui, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `body.ds{padding:12px}` — the gallery frame adds 20, so the card pulls in
/// by 8 and sizes `.br{height:610px}` to the card width less 24.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 980.0 - 24.0;
const FRAME_H: f32 = 610.0;
/// `.tabs .btn.icon.xs svg{width:12px;height:12px}`.
const TAB_GLYPH: f32 = 12.0;

/// `.page{padding:30px 36px}`.
const PAGE_PAD_Y: f32 = 30.0;
const PAGE_PAD_X: f32 = 36.0;
/// `.page h1{font-size:24px;margin:0 0 6px}`.
const H1_SIZE: f32 = 24.0;
const H1_GAP: f32 = 6.0;
/// `.page p{font-size:13px;margin:0 0 18px;max-width:50ch}` — 50ch of Geist 13.
const LEAD_GAP: f32 = 18.0;
const LEAD_MEASURE: f32 = 445.0;
/// `.cards{gap:14px}`; `.pc{width:150px;border-radius:10px;padding:14px;font-size:12px}`.
const CARDS_GAP: f32 = 14.0;
const CARD_W: f32 = 150.0;
const CARD_RADIUS: f32 = 10.0;
const CARD_PAD: f32 = 14.0;
const CARD_TEXT: f32 = 12.0;
/// `.pc b{font-size:14px;margin-bottom:6px}`.
const CARD_TITLE: f32 = 14.0;
const CARD_TITLE_GAP: f32 = 6.0;
/// `.pc .cta{margin-top:10px;padding:6px 10px;border-radius:6px}`.
const CTA_TOP: f32 = 10.0;
const CTA_PAD_Y: f32 = 6.0;
const CTA_PAD_X: f32 = 10.0;
const CTA_RADIUS: f32 = 6.0;

/// `.hover{left:36px;top:70px;width:392px;height:34px}`.
const HOVER_LEFT: f32 = 36.0;
const HOVER_TOP: f32 = 70.0;
const HOVER_W: f32 = 392.0;
const HOVER_H: f32 = 34.0;
/// `.sel{left:36px;top:126px;width:150px;height:118px}`.
const SEL_LEFT: f32 = 36.0;
const SEL_TOP: f32 = 126.0;
const SEL_W: f32 = 150.0;
const SEL_H: f32 = 118.0;
/// `.pin{transform:translate(-4px,-22px)}` applied to the three `left/top` pairs.
const PIN_SHIFT_X: f32 = -4.0;
const PIN_SHIFT_Y: f32 = -22.0;
const PIN_1: (f32, f32) = (222.0, 30.0);
const PIN_2: (f32, f32) = (186.0, 126.0);
const PIN_3: (f32, f32) = (456.0, 126.0);
/// The selected pin's white ring lies outside its box (2 px at 1 px offset).
const PIN_RING_INSET: f32 = 3.0;
/// `.pop{left:250px;top:140px}`.
const POP_LEFT: f32 = 250.0;
const POP_TOP: f32 = 140.0;
/// `.agent{left:36px;bottom:16px}`.
const AGENT_LEFT: f32 = 36.0;
const AGENT_BOTTOM: f32 = 16.0;

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

/// `.page`: the sample web page with the annotator's overlays on top. Page
/// content is a document of its own and always light, so it is painted from
/// the light palette (the design's `#fff` / `#1a1c22` / `#e2e5eb`).
fn page() -> impl IntoElement {
    let w = light();
    div()
        .relative()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .py(px(PAGE_PAD_Y))
        .px(px(PAGE_PAD_X))
        .bg(w.surface_1)
        .text_color(w.ink)
        .ui(scale::FS_13)
        .child(div().mb(px(H1_GAP)).ui(H1_SIZE).font_weight(FontWeight::BOLD).child("Simple pricing"))
        .child(
            div()
                .mb(px(LEAD_GAP))
                .max_w(px(LEAD_MEASURE))
                .ui(scale::FS_13)
                .text_color(w.ink_2)
                .child("Start free, upgrade when your team needs it. All plans include unlimited worktrees."),
        )
        .child(
            h_flex()
                .items_start()
                .gap(px(CARDS_GAP))
                .child(plan_card(w, "Starter", "$0 / month", "3 agents, local only", "Try free", true))
                .child(plan_card(w, "Team", "$24 / seat", "SSH + cloud VMs", "Choose", false))
                .child(plan_card(w, "Enterprise", "Custom", "SSO, audit, support", "Contact", false)),
        )
        // The hovered element and its tag label.
        .child(
            div()
                .absolute()
                .left(px(HOVER_LEFT))
                .top(px(HOVER_TOP))
                .child(element_outline("p · 392 × 34").size(px(HOVER_W), px(HOVER_H))),
        )
        // The selected element keeps its 1.5 px outline.
        .child(
            div()
                .absolute()
                .left(px(SEL_LEFT))
                .top(px(SEL_TOP))
                .child(element_outline("div.card.starter").selected(true).size(px(SEL_W), px(SEL_H))),
        )
        .child(pin_at(PIN_1, 1, false))
        .child(pin_at(PIN_2, 2, true))
        .child(pin_at(PIN_3, 3, false))
        .child(
            div().absolute().left(px(POP_LEFT)).top(px(POP_TOP)).child(
                note_popover(
                    "card51-note",
                    "div.card.starter · 150 × 118 · pricing.tsx:42",
                    "Make the Starter card stand out; it is the plan most people pick.",
                )
                .on_action(|_, _, _| {}),
            ),
        )
        .child(
            div()
                .absolute()
                .left(px(AGENT_LEFT))
                .bottom(px(AGENT_BOTTOM))
                .child(agent_action_pill("card51-agent", "Clicking “Try free”").detail("· verifying signup flow")),
        )
}

/// `.pc`: one pricing card of the sample page.
fn plan_card(w: Palette, title: &'static str, price: &'static str, detail: &'static str, cta: &'static str, primary: bool) -> impl IntoElement {
    let (cta_bg, cta_ink) = if primary { (w.accent, gpui::white()) } else { (w.surface_2, w.ink) };
    v_flex()
        .flex_none()
        .w(px(CARD_W))
        .p(px(CARD_PAD))
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_color(w.line)
        .ui(CARD_TEXT)
        .child(div().mb(px(CARD_TITLE_GAP)).ui(CARD_TITLE).font_weight(FontWeight::BOLD).child(title))
        .child(div().child(price))
        .child(div().child(detail))
        // The CTA is an inline-block: it hugs its label on its own line.
        .child(
            h_flex().mt(px(CTA_TOP)).child(
            div()
                .py(px(CTA_PAD_Y))
                .px(px(CTA_PAD_X))
                .rounded(px(CTA_RADIUS))
                .bg(cta_bg)
                .text_color(cta_ink)
                .medium()
                .child(cta),
        ))
}

/// A pin at one of the design's `left/top` pairs, after its translate.
fn pin_at(at: (f32, f32), index: usize, selected: bool) -> impl IntoElement {
    let ring = if selected { PIN_RING_INSET } else { 0.0 };
    div()
        .absolute()
        .left(px(at.0 + PIN_SHIFT_X - ring))
        .top(px(at.1 + PIN_SHIFT_Y - ring))
        .child(annotation_pin(index).selected(selected))
}
