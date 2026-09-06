//! The scripted fake page: the "Simple pricing" mock the gallery browses.
//!
//! This is the page card 51 (`design/src/cards/workbench/51-browser.html`) is
//! drawn over, moved here so that the live pane and the static card paint the
//! same pixels from the same source. [`page`] is card 51's whole picture —
//! the document plus the annotator overlays the design froze on top of it;
//! [`page_body`] is the document on its own, which is what the live pane
//! renders before it adds overlays of its own.
//!
//! Page content is a document, not app chrome: it is painted from
//! [`aui_tokens::light`] whatever the app's theme is, exactly as a real
//! webview would be.
//!
//! [`fake_elements`] exposes the boxes of the elements the document draws, so
//! [`crate::fake::FakeWebBackend`] can hit-test a pointer against real
//! geometry instead of pretending.

use aui::workbench::{agent_action_pill, annotation_pin, element_outline, note_popover};
use aui_tokens::{light, scale, AuiStyled, Palette};
use gpui::{div, prelude::*, px, Div, FontWeight, IntoElement, SharedString};
use gpui_kit::base::{h_flex, v_flex};

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

/// The `h1`'s box: it starts at the page's padding and its right edge is where
/// the design drops the first pin (`PIN_1.0`), one `h1` line tall.
const H1_TOP: f32 = PAGE_PAD_Y;
const H1_W: f32 = PIN_1.0 - PAGE_PAD_X;
const H1_H: f32 = 34.0;

/// One element of the fake document, as the annotator sees it.
///
/// The boxes are the design's own numbers — the same constants the overlays in
/// [`page`] are placed with — so a pointer hit-test lands exactly where card 51
/// draws its outline.
#[derive(Debug, Clone, PartialEq)]
pub struct FakeElement {
    /// The CSS path (`div.card.starter`).
    pub selector: SharedString,
    /// The tag the outline's label leads with (`p`).
    pub label: SharedString,
    /// The top-left corner in page coordinates.
    pub origin: (f32, f32),
    /// The box in page pixels.
    pub size: (f32, f32),
    /// The source location a mapped dev server would report.
    pub source: SharedString,
}

/// The elements the fake page offers the annotator: the headline, the lead
/// paragraph and the three plan cards, front to back.
pub fn fake_elements() -> Vec<FakeElement> {
    let element = |selector: &'static str, label: &'static str, origin: (f32, f32), size: (f32, f32), source: &'static str| FakeElement {
        selector: SharedString::from(selector),
        label: SharedString::from(label),
        origin,
        size,
        source: SharedString::from(source),
    };
    let card_left = |column: f32| PAGE_PAD_X + column * (CARD_W + CARDS_GAP);
    vec![
        element("h1", "h1", (PAGE_PAD_X, H1_TOP), (H1_W, H1_H), "pricing.tsx:31"),
        element("p.lead", "p", (HOVER_LEFT, HOVER_TOP), (HOVER_W, HOVER_H), "pricing.tsx:34"),
        element("div.card.starter", "div", (card_left(0.0), SEL_TOP), (SEL_W, SEL_H), "pricing.tsx:42"),
        element("div.card.team", "div", (card_left(1.0), SEL_TOP), (SEL_W, SEL_H), "pricing.tsx:43"),
        element("div.card.enterprise", "div", (card_left(2.0), SEL_TOP), (SEL_W, SEL_H), "pricing.tsx:44"),
    ]
}

/// `.page`: the sample web page with the annotator's overlays on top. Page
/// content is a document of its own and always light, so it is painted from
/// the light palette (the design's `#fff` / `#1a1c22` / `#e2e5eb`).
///
/// This is card 51's frozen picture: the hovered paragraph, the selected
/// Starter card, three pins, the open note and the agent-action pill.
pub fn page() -> impl IntoElement {
    page_body()
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

/// The document on its own: the headline, the lead and the three plan cards,
/// with no annotator overlays. The caller positions overlays against it in the
/// same coordinates [`fake_elements`] reports.
pub fn page_body() -> Div {
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
///
/// `at` is the element's top-right corner in page coordinates; the pin's
/// bottom-left point lands on it.
pub fn pin_at(at: (f32, f32), index: usize, selected: bool) -> impl IntoElement {
    let ring = if selected { PIN_RING_INSET } else { 0.0 };
    div()
        .absolute()
        .left(px(at.0 + PIN_SHIFT_X - ring))
        .top(px(at.1 + PIN_SHIFT_Y - ring))
        .child(annotation_pin(index).selected(selected))
}
