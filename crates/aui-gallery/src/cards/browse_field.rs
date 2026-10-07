//! Browse card · the Field. The browser's single input floating over a
//! dimmed grey stand-in page, frozen on a "nas" question: Ask selected, then
//! Search and Task, the matching NAS tab, and the matching sampling note
//! under their captions. A static composition — the rows come from
//! `rank` over real `FieldSources`,
//! so the card shows exactly what the Field shows for this query.

use aui::browse::field::{field, rank, FieldCommand, FieldHistory, FieldModel, FieldSources, FieldTab};
use aui_tokens::{scale, ActiveAui};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// The query the card is frozen on: tabs and history have matches for it, so
/// both captioned groups show beside the agent rows.
const QUERY: &str = "nas";
/// The row the card shows highlighted: Ask.
const SELECTED: usize = 0;
/// The card stage: full width × 480, the Field's 640 px card centred on it.
const STAGE_H: f32 = 480.0;
/// How far below the stage top the Field sits.
const FIELD_PAD_TOP: f32 = 40.0;
/// The stand-in page's fake chrome and skeleton bars.
const PAGE_BAR_H: f32 = 36.0;
const PAGE_LINE_H: f32 = 8.0;
const PAGE_GAP: f32 = 10.0;
const PAGE_PAD: f32 = 16.0;

/// The sources the card ranks: the open tabs, the history with relative read
/// words, and the commands.
fn sources() -> FieldSources {
    let mut out = FieldSources::new("Claude", "Google");
    out.tabs = vec![
        FieldTab { id: "tab-nas".into(), title: "NAS 2024 · state report cards".into(), host: "ncert.nic.in".into() },
        FieldTab { id: "tab-udise".into(), title: "UDISE+ 2024-25 · dashboard".into(), host: "udiseplus.gov.in".into() },
        FieldTab { id: "tab-pgi".into(), title: "PGI 2.0 · state scores 2023-24".into(), host: "pgi.udiseplus.gov.in".into() },
    ];
    out.history = vec![
        FieldHistory { title: "NAS 2024 · Karnataka report card".into(), url: "ncert.nic.in/nas".into(), read: "read Tuesday".into() },
        FieldHistory { title: "District report cards 2024-25".into(), url: "udiseplus.gov.in/reports".into(), read: "read Monday".into() },
        FieldHistory { title: "NAS 2024 · sampling note".into(), url: "notes.example.com/nas-sampling".into(), read: "read Sunday".into() },
    ];
    out.commands = vec![
        FieldCommand { id: "split".into(), name: "Split View".into() },
        FieldCommand { id: "reading".into(), name: "Reading mode".into() },
        FieldCommand { id: "space".into(), name: "New Space…".into() },
    ];
    out
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
    let mut model = FieldModel::new(QUERY, rank(QUERY, &sources()));
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
