//! D6 · Page (docs/43 §2.7; canvas v49 `D6Page`, `D6StatesPage`): the nav
//! row (wide, loading, narrow), the agent pill, annotating (outline, pins,
//! the notes panel), no page yet, couldn't load. The page itself is a white
//! stand-in here; in Baaz it is the native page window.

use aui::workbench::{
    address_field_frame, agent_pill, annotate_outline, annotate_pin, no_page_state, notes_panel, page_address, page_error_state, page_nav_row,
    pane_row, PageNote,
};
use aui_icons::{IconName, Provider};
use aui_tokens::{ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

const W: f32 = 400.0;
const H: f32 = 520.0;

fn page_ground() -> Div {
    div()
        .relative()
        .flex_1()
        .w_full()
        .bg(gpui::white())
        .p(px(24.0))
        .text_color(gpui::rgb(0x18181B))
        .child(div().text_px(28.0).font_weight(FontWeight::BOLD).child("Simple pricing"))
        .child(div().mt(px(6.0)).text_px(14.0).text_color(gpui::rgb(0x63636D)).child("Start free, upgrade when your team needs it."))
        .child(
            div()
                .mt(px(20.0))
                .w(px(160.0))
                .p(px(14.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(gpui::rgb(0xE4E4E7))
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Starter"))
                .child(div().text_px(13.0).child("$0 / month"))
                .child(div().mt(px(10.0)).px(px(10.0)).py(px(5.0)).rounded(px(6.0)).bg(gpui::rgb(0x18181B)).text_color(gpui::white()).text_px(12.0).child("Try free")),
        )
}

fn screen(label: &str, w: f32, body: impl IntoElement, cx: &App) -> Div {
    let p = cx.aui().colors;
    v_flex().gap(px(6.0)).child(div().text_px(12.0).text_color(p.fg_muted).child(label.to_string())).child(
        div()
            .w(px(w))
            .h(px(H))
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(p.surface_2)
            .shadow(p.edge())
            .flex()
            .flex_col()
            .child(pane_row(SharedString::from(format!("{label}-row")), IconName::Globe, "Page").on_close(|_, _, _| {}))
            .child(body),
    )
}

pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let _ = window;
    let url = "http://localhost:3000/pricing";
    let nav = |id: &str, w: f32, annot: bool, loading: Option<f32>, cx: &App| {
        page_nav_row(SharedString::from(id.to_string()), page_address(SharedString::from(format!("{id}-addr")), url, cx).into_any_element(), true, false, annot, w, loading, |_, _, _| {}, cx)
    };

    let with_pill = v_flex()
        .flex_1()
        .child(nav("d6-nav-1", W, false, None, cx))
        .child(page_ground().child(div().absolute().left(px(0.0)).right(px(0.0)).bottom(px(16.0)).flex().justify_center().child(agent_pill("d6-pill", Provider::Muse, "Muse is clicking “Choose”", cx))));
    let s1 = screen("Page + the agent pill", W, with_pill, cx);

    let notes = [
        PageNote { index: 1, selector: "h1.title".into(), note: "Make the heading one line on desktop.".into() },
        PageNote { index: 2, selector: "button.try".into(), note: "Use the brand blue here.".into() },
    ];
    let annot = v_flex().flex_1().child(nav("d6-nav-2", W, true, None, cx)).child(
        page_ground()
            .child(div().absolute().left(px(22.0)).top(px(22.0)).child(annotate_outline(236.0, 38.0)))
            .child(div().absolute().left(px(246.0)).top(px(10.0)).child(annotate_pin(1)))
            .child(div().absolute().left(px(124.0)).top(px(178.0)).child(annotate_pin(2)))
            .child(div().absolute().left(px(8.0)).right(px(8.0)).bottom(px(8.0)).child(notes_panel("d6-notes", &notes, "Muse", None, None, cx))),
    );
    let s2 = screen("Annotating", W, annot, cx);

    let loading = v_flex().flex_1().child(nav("d6-nav-3", W, false, Some(0.6), cx)).child(page_ground());
    let s3 = screen("Loading", W, loading, cx);

    let empty = no_page_state(
        "d6-nopage",
        address_field_frame(false, cx).child(div().text_color(cx.aui().colors.fg_muted).child("Search or enter an address")),
        "Muse",
        cx,
    );
    let s4 = screen("No page yet", W, empty, cx);
    let err = div().flex_1().child(page_error_state("d6-err", "localhost:3000", "refused the connection. Is the dev server running?", |_, _, _| {}, cx));
    let s5 = screen("Couldn’t load", W, err, cx);
    let narrow = v_flex().flex_1().child(nav("d6-nav-4", 280.0, false, None, cx)).child(page_ground());
    let s6 = screen("Narrow 280", 280.0, narrow, cx);

    v_flex()
        .gap(px(20.0))
        .child(h_flex().gap(px(16.0)).items_start().child(s1).child(s2).child(s3))
        .child(h_flex().gap(px(16.0)).items_start().child(s4).child(s5).child(s6))
        .into_any_element()
}
