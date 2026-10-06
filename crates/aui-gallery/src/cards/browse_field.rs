//! Browse card · the Field. The browser's single input floating over a
//! dimmed grey stand-in page, frozen on a week-old NAS question: Ask
//! selected, then Search and Task, matching tabs, history with relative read
//! words, and commands. A static composition — the rows are hand-built so the
//! card shows exactly this state.

use aui::browse::field::{field, FieldModel, FieldRow, FieldRowKind};
use aui_tokens::{scale, ActiveAui};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// The query the card is frozen on.
const QUERY: &str = "that NAS report card I read last week";
/// The row the card shows highlighted: Ask.
const SELECTED: usize = 0;
/// The card stage: 760 × 520, the Field's 640 px card centred on it.
const STAGE_H: f32 = 480.0;
/// How far below the stage top the Field sits.
const FIELD_PAD_TOP: f32 = 40.0;
/// The stand-in page's fake chrome and skeleton bars.
const PAGE_BAR_H: f32 = 36.0;
const PAGE_LINE_H: f32 = 8.0;
const PAGE_GAP: f32 = 10.0;
const PAGE_PAD: f32 = 16.0;

/// The card's rows, in display order.
fn rows() -> Vec<FieldRow> {
    vec![
        FieldRow {
            kind: FieldRowKind::Ask,
            title: "Ask Claude — answers here".into(),
            detail: None,
            hint: "↩".into(),
        },
        FieldRow {
            kind: FieldRowKind::Search,
            title: "Search Google".into(),
            detail: None,
            hint: "⌥↩".into(),
        },
        FieldRow {
            kind: FieldRowKind::Task,
            title: "Task for Claude — in its own tabs".into(),
            detail: None,
            hint: "⌘↩".into(),
        },
        FieldRow {
            kind: FieldRowKind::Tab { id: "tab-nas".into() },
            title: "NAS 2024 · state report cards".into(),
            detail: Some("ncert.nic.in".into()),
            hint: "↩".into(),
        },
        FieldRow {
            kind: FieldRowKind::Tab { id: "tab-udise".into() },
            title: "UDISE+ 2024-25 · dashboard".into(),
            detail: Some("udiseplus.gov.in".into()),
            hint: "↩".into(),
        },
        FieldRow {
            kind: FieldRowKind::History { url: "ncert.nic.in/nas".into() },
            title: "NAS 2024 · Karnataka report card".into(),
            detail: Some("ncert.nic.in/nas".into()),
            hint: "read Tuesday".into(),
        },
        FieldRow {
            kind: FieldRowKind::History { url: "udiseplus.gov.in".into() },
            title: "District report cards 2024-25".into(),
            detail: Some("udiseplus.gov.in".into()),
            hint: "read Monday".into(),
        },
        FieldRow {
            kind: FieldRowKind::Command { id: "split".into() },
            title: "Split View".into(),
            detail: None,
            hint: "↩".into(),
        },
        FieldRow {
            kind: FieldRowKind::Command { id: "reading".into() },
            title: "Reading mode".into(),
            detail: None,
            hint: "↩".into(),
        },
        FieldRow {
            kind: FieldRowKind::Command { id: "space".into() },
            title: "New Space…".into(),
            detail: None,
            hint: "↩".into(),
        },
    ]
}

/// The dimmed grey stand-in page: a fake nav bar and skeleton text bars in
/// surface steps, dimmed the way the palette scrim dims its ground.
fn stand_in_page(p: aui_tokens::Palette) -> AnyElement {
    let bar = |width: f32| {
        div().flex_none().h(px(PAGE_LINE_H)).w(px(width)).rounded(px(scale::R_XS)).bg(p.surface_3)
    };
    v_flex()
        .absolute()
        .inset_0()
        .bg(p.surface_1)
        .child(
            h_flex()
                .flex_none()
                .w_full()
                .h(px(PAGE_BAR_H))
                .items_center()
                .gap(px(PAGE_GAP))
                .px(px(PAGE_PAD))
                .border_b_1()
                .border_color(p.line)
                .bg(p.surface_2)
                .child(div().flex_none().size(px(10.0)).rounded_full().bg(p.ink_4))
                .child(div().flex_none().size(px(10.0)).rounded_full().bg(p.ink_4))
                .child(div().flex_none().size(px(10.0)).rounded_full().bg(p.ink_4))
                .child(div().flex_none().h(px(PAGE_LINE_H)).w(px(220.0)).rounded(px(scale::R_XS)).bg(p.surface_3)),
        )
        .child(
            v_flex()
                .flex_1()
                .w_full()
                .gap(px(PAGE_GAP))
                .p(px(PAGE_PAD))
                .child(bar(300.0))
                .child(bar(420.0))
                .child(bar(260.0))
                .child(bar(380.0))
                .child(bar(200.0)),
        )
        .child(div().absolute().inset_0().bg(black().opacity(0.35)))
        .into_any_element()
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut model = FieldModel::new(QUERY, rows());
    model.selected = SELECTED;
    div()
        .relative()
        .w_full()
        .h(px(STAGE_H))
        .flex_none()
        .overflow_hidden()
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .child(stand_in_page(p))
        .child(
            h_flex()
                .relative()
                .w_full()
                .justify_center()
                .pt(px(FIELD_PAD_TOP))
                .child(field("card-field", &model).placeholder("Search or type a URL").on_choose(|_, _, _| {}).on_close(|_, _| {})),
        )
        .into_any_element()
}
