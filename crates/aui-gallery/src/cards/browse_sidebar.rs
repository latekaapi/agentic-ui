//! P1 · Browse sidebar. The whole sidebar composed with sample data at
//! 248 px, plus the five task-folder rows stacked on their own card.

use aui::browse::{
    address_line, browse_footer, browse_nav_cluster, folder_row, pin_grid, tab_row, task_folder_row, Address, Pin,
    TabRowData, TaskFace, BROWSE_SIDEBAR_WIDTH,
};
use aui::data::{kbd, record_ax_label};
use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// Traffic-light geometry, mirroring `aui::shell::header` (the Browse
/// header seats the nav cluster where that row seats search + toggle, so it
/// paints its own lights at the same metrics).
const LIGHT_SIZE: f32 = 11.0;
const LIGHT_GAP: f32 = 7.0;
const LIGHTS_MARGIN_LEFT: f32 = 2.0;
/// macOS traffic-light colours, as in `aui::shell::header`.
const LIGHT_CLOSE: u32 = 0xFF5F57;
const LIGHT_MIN: u32 = 0xFEBC2E;
const LIGHT_ZOOM: u32 = 0x28C840;
/// The header row's horizontal padding and inner gap.
const HEADER_PAD_X: f32 = 10.0;
const HEADER_GAP: f32 = 6.0;
/// The `+ New tab` row height, like the nav rows.
const NEW_TAB_H: f32 = 30.0;
/// The new-tab row's leading box, so the `+` centres where row glyphs do.
const NEW_TAB_LEAD: f32 = 20.0;

/// The traffic-light row: painted lights, spacer, back / forward / reload.
fn header_row(p: Palette) -> impl IntoElement {
    let light = |rgb: u32| div().flex_none().size(px(LIGHT_SIZE)).rounded_full().bg(gpui::rgb(rgb));
    h_flex()
        .w_full()
        .h(px(44.0))
        .flex_none()
        .items_center()
        .gap(px(HEADER_GAP))
        .px(px(HEADER_PAD_X))
        .border_b_1()
        .border_color(p.line)
        .child(
            h_flex()
                .flex_none()
                .gap(px(LIGHT_GAP))
                .ml(px(LIGHTS_MARGIN_LEFT))
                .child(light(LIGHT_CLOSE))
                .child(light(LIGHT_MIN))
                .child(light(LIGHT_ZOOM)),
        )
        .child(div().flex_1())
        .child(
            browse_nav_cluster("browse-nav")
                .can_go_back(true)
                .on_back(|_, _, _| {})
                .on_forward(|_, _, _| {})
                .on_reload(|_, _, _| {}),
        )
}

/// The `+ New tab` row in the nav-row style, with the `⌘T` hint.
fn new_tab_row(p: Palette) -> impl IntoElement {
    record_ax_label("New tab");
    h_flex()
        .id("browse-new-tab")
        .h(px(NEW_TAB_H))
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .pl(px(8.0))
        .pr(px(8.0))
        .mx(px(8.0))
        .rounded(px(scale::R_SM))
        .ui(scale::FS_13)
        .text_color(p.ink_2)
        .cursor_pointer()
        .role(gpui::Role::Button)
        .aria_label("New tab")
        .on_click(|_, _, _| {})
        .child(
            div()
                .flex_none()
                .w(px(NEW_TAB_LEAD))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(IconName::Plus).size(px(14.0)).color(p.ink_3)),
        )
        .child(div().flex_1().min_w(px(0.0)).truncate().child("New tab"))
        .child(kbd("⌘T"))
}

fn sample_pins() -> Vec<Pin> {
    vec![
        Pin::new("u", "U", "UDISE+").position(1),
        Pin::new("p", "P", "PGI 2.0").position(2),
        Pin::new("d", "D", "data.gov.in").position(3),
        Pin::new("m", "M", "NAS 2024").position(4),
    ]
}

fn hairline(p: Palette) -> impl IntoElement {
    div().flex_none().h(px(1.0)).mx(px(8.0)).my(px(4.0)).bg(p.line)
}

/// Builds the `browse/sidebar` card: the whole sidebar at 248 px.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .flex_none()
        .w(px(BROWSE_SIDEBAR_WIDTH))
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .overflow_hidden()
        .child(header_row(p))
        .child(
            address_line(
                "browse-address",
                Address::new("pgi.udiseplus.gov.in", "/#/home", false, None, true, Some("https".to_owned())),
            )
            .on_click(|_, _, _| {}),
        )
        .child(new_tab_row(p))
        .child(pin_grid("browse-pins", sample_pins()).on_select(|_, _, _| {}))
        .child(
            tab_row("browse-tab-1", TabRowData::new("PGI 2.0 · state scores 2023-24", "P").active().position(1))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(
            tab_row("browse-tab-2", TabRowData::new("UDISE+ 2024-25 · dashboard", "U").position(2))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(
            tab_row("browse-tab-3", TabRowData::new("NAS 2024 · state report cards", "N").position(3))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(folder_row("browse-folder-ka", "Karnataka review", true, 0).on_toggle(|_, _, _| {}))
        .child(
            tab_row("browse-tab-ka-1", TabRowData::new("KSEEB · SSLC results 2025", "K").depth(1).position(4))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(
            tab_row("browse-tab-ka-2", TabRowData::new("Samagra Shiksha · Karnataka", "S").depth(1).position(5))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(hairline(p))
        .child(
            task_folder_row(
                "browse-task-pgi",
                "PGI vs school indicators · 5 states",
                TaskFace::Typing,
                TaskFace::Typing.status_word(),
                true,
            )
            .on_toggle(|_, _, _| {})
            .on_open_chat(|_, _, _| {}),
        )
        .child(
            tab_row("browse-tab-pgi-1", TabRowData::new("PGI 2.0 scores · 5 states", "P").depth(1).position(6))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(
            tab_row("browse-tab-pgi-2", TabRowData::new("UDISE+ · PTR by state", "U").depth(1).position(7))
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(
            tab_row(
                "browse-tab-pgi-3",
                TabRowData::new("UDISE+ · dropout, secondary", "U").depth(1).position(8),
            )
            .on_select(|_, _, _| {})
            .on_close(|_, _, _| {}),
        )
        .child(
            tab_row(
                "browse-tab-pgi-4",
                TabRowData::new("data.gov.in · enrolment 2024-25", "D").depth(1).position(9),
            )
            .on_select(|_, _, _| {})
            .on_close(|_, _, _| {}),
        )
        .child(
            task_folder_row(
                "browse-task-ledger",
                "Fix ledger chart",
                TaskFace::Done,
                SharedString::from(format!("{} · 1 tab", TaskFace::Done.status_word())),
                true,
            )
            .on_toggle(|_, _, _| {})
            .on_open_chat(|_, _, _| {}),
        )
        .child(
            tab_row(
                "browse-tab-ledger-1",
                TabRowData::new("localhost:5173 · Ledger", "L").depth(1).position(10).port(5173),
            )
            .on_select(|_, _, _| {})
            .on_close(|_, _, _| {}),
        )
        .child(div().flex_1())
        .child(
            browse_footer("browse-footer", "Education")
                .on_space(|_, _, _| {})
                .on_downloads(|_, _, _| {})
                .on_extensions(|_, _, _| {}),
        )
        .into_any_element()
}

/// Builds the `browse/task-faces` card: the five task-folder rows stacked.
pub fn build_faces(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let rows = [
        ("browse-faces-reading", "Overnight brief", TaskFace::Reading),
        ("browse-faces-typing", "Ledger rewrite", TaskFace::Typing),
        ("browse-faces-needs", "Release sign-off", TaskFace::NeedsYou),
        ("browse-faces-paused", "Backfill job", TaskFace::Paused),
        ("browse-faces-done", "Ledger chart fix", TaskFace::Done),
    ];
    let mut col =
        v_flex().gap(px(2.0)).p(px(8.0)).rounded(px(scale::R_LG)).border_1().border_color(p.line).bg(p.surface_1);
    for (id, name, face) in rows {
        let status = if face == TaskFace::Done {
            SharedString::from(format!("{} · 3 tabs", face.status_word()))
        } else {
            SharedString::from(face.status_word())
        };
        col = col.child(
            task_folder_row(id, name, face, status, false).on_toggle(|_, _, _| {}).on_open_chat(|_, _, _| {}),
        );
    }
    col.into_any_element()
}
