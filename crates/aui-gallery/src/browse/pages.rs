//! The drawn pages: plain gpui elements on a white page surface, never a
//! webview. Chrome (hairlines, hints) uses theme tokens; the depicted page
//! itself is white with `#0E1116` ink in both themes, like the workbench's
//! document paper.

use aui::data::{button, record_ax_label};
use aui_tokens::{scale, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

use super::story::StoryPage;

/// Depicted-page ink: content, not chrome.
const INK: u32 = 0x0E1116;
/// The page's inner padding and section gap.
const PAGE_PAD: f32 = 28.0;
const PAGE_GAP: f32 = 12.0;

/// The white page surface every page sits on.
fn shell(children: Vec<AnyElement>) -> AnyElement {
    v_flex().size_full().bg(gpui::white()).px(px(PAGE_PAD)).py(px(PAGE_PAD)).gap(px(PAGE_GAP)).children(children).into_any_element()
}

/// A page heading: title in page ink, subtitle faint.
fn heading(title: &str, subtitle: &str) -> AnyElement {
    v_flex()
        .gap(px(2.0))
        .child(div().ui(scale::FS_18).semibold().text_color(gpui::rgb(INK)).child(title.to_string()))
        .child(div().ui(scale::FS_12).text_color(gpui::rgb(INK).opacity(0.6)).child(subtitle.to_string()))
        .into_any_element()
}

/// One table row: cells across, hairline below.
fn row(cells: Vec<(&str, f32)>) -> AnyElement {
    let mut line = h_flex().w_full().py(px(6.0)).gap(px(12.0));
    for (text, width) in cells {
        line = line.child(
            div().w(px(width)).flex_none().truncate().ui(scale::FS_12).text_color(gpui::rgb(INK)).child(text.to_string()),
        );
    }
    v_flex().w_full().child(line).child(div().w_full().h(px(1.0)).bg(gpui::rgb(INK).opacity(0.12))).into_any_element()
}

/// The PGI grades table rows: state, score, grade.
const PGI_ROWS: [(&str, &str, &str); 6] = [
    ("Kerala", "731", "Daksh"),
    ("Tamil Nadu", "702", "Ati-Uttam"),
    ("Karnataka", "668", "Uttam"),
    ("Maharashtra", "661", "Uttam"),
    ("Gujarat", "654", "Uttam"),
    ("Punjab", "647", "Uttam"),
];

/// A plain page link row: left-aligned link-coloured text with a ↗, no
/// fill. Every drawn page carries at least one, all wired the same way:
/// a plain click navigates there, ⇧ peeks, ⌥ splits beside the current
/// page. The screen owns what each gesture does.
pub fn link_row(
    label: &str,
    target: StoryPage,
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    record_ax_label(label);
    let id = SharedString::from(format!("page-link-{label}"));
    let label_text = format!("{label} ↗");
    let aria = label_text.clone();
    div()
        .w_full()
        .pt(px(4.0))
        .child(
            div()
                .id(id)
                .ui(scale::FS_13)
                .text_color(link)
                .cursor_pointer()
                .role(Role::Button)
                .aria_label(aria)
                .child(label_text)
                .on_click(move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                    on_link(&target, window, cx);
                }),
        )
        .into_any_element()
}

/// `pgi_page`: the PGI 2.0 grades table with the methodology link.
pub fn pgi_page(
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut table = v_flex().w_full().child(row(vec![("State", 140.0), ("Score", 80.0), ("Grade", 120.0)]));
    for (state, score, grade) in PGI_ROWS {
        table = table.child(row(vec![(state, 140.0), (score, 80.0), (grade, 120.0)]));
    }
    shell(vec![
        heading("PGI 2.0 · Performance Grading Index · 2023-24", "State scores, all grades"),
        table.into_any_element(),
        link_row("Methodology note", StoryPage::Peek, link, on_link),
    ])
}

/// `udise_page`: PTR bars for five states.
pub fn udise_page(
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    const BARS: [(&str, u32); 5] = [("Karnataka", 22), ("Tamil Nadu", 20), ("Kerala", 18), ("Maharashtra", 24), ("Gujarat", 27)];
    let mut list = v_flex().w_full().gap(px(8.0));
    for (state, value) in BARS {
        list = list.child(
            h_flex()
                .w_full()
                .items_center()
                .gap(px(12.0))
                .child(div().w(px(120.0)).flex_none().truncate().ui(scale::FS_12).text_color(gpui::rgb(INK)).child(state.to_string()))
                .child(div().w(px(value as f32 * 9.0)).h(px(14.0)).rounded(px(4.0)).bg(gpui::rgb(INK).opacity(0.75)))
                .child(div().ui(scale::FS_12).text_color(gpui::rgb(INK)).child(value.to_string())),
        );
    }
    shell(vec![
        heading("UDISE+ 2024-25 · PTR by state", "Pupil–teacher ratio, lower is better"),
        list.into_any_element(),
        link_row("Secondary dropout rates", StoryPage::Dropout, link, on_link),
    ])
}

/// `datagov_page`: the enrolment dataset behind a sign-in box (inert).
pub fn datagov_page(
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    shell(vec![
        heading("data.gov.in · School education indicators 2024-25", "State indicators spreadsheet · 14 MB"),
        div()
            .mt(px(8.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(gpui::rgb(INK).opacity(0.2))
            .p(px(16.0))
            .child(div().ui(scale::FS_13).medium().text_color(gpui::rgb(INK)).child("Sign in to download".to_string()))
            .child(
                div()
                    .pt(px(4.0))
                    .ui(scale::FS_12)
                    .text_color(gpui::rgb(INK).opacity(0.6))
                    .child("The file stays on data.gov.in until you sign in.".to_string()),
            )
            .into_any_element(),
        link_row("Enrolment data dictionary", StoryPage::Generic("Enrolment data dictionary".into()), link, on_link),
    ])
}

/// The comparison table rows: state, grade, PTR, dropout, GER.
const ANSWER_ROWS: [(&str, &str, &str, &str, &str); 5] = [
    ("Karnataka", "Uttam", "22", "3.1 %", "92.4"),
    ("Tamil Nadu", "Ati-Uttam", "20", "2.4 %", "96.1"),
    ("Kerala", "Daksh", "18", "1.2 %", "101.3"),
    ("Maharashtra", "Uttam", "24", "4.8 %", "90.2"),
    ("Gujarat", "Uttam", "27", "6.9 %", "88.7"),
];

/// `answer_page`: the built comparison with the action row.
pub fn answer_page(
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
    on_ask_more: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_done: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut table =
        v_flex().w_full().child(row(vec![("State", 110.0), ("PGI 2.0", 90.0), ("PTR", 50.0), ("Dropout", 70.0), ("Girls' GER", 80.0)]));
    for (state, grade, ptr, dropout, ger) in ANSWER_ROWS {
        table = table.child(row(vec![(state, 110.0), (grade, 90.0), (ptr, 50.0), (dropout, 70.0), (ger, 80.0)]));
    }
    shell(vec![
        heading("PGI 2.0 vs school indicators, 2023-24 / 2024-25", "5 states · 4 sources · 2 skipped"),
        table.into_any_element(),
        link_row("Methodology note", StoryPage::Peek, link, on_link),
        h_flex()
            .w_full()
            .items_center()
            .gap(px(8.0))
            .pt(px(8.0))
            .child(div().ui(scale::FS_11).text_color(gpui::rgb(INK).opacity(0.55)).child("Steers queue behind this turn".to_string()))
            .child(div().flex_1())
            .child(button("answer-ask", "Ask more").on_click(on_ask_more))
            .child(button("answer-done", "Done").primary().on_click(on_done))
            .into_any_element(),
    ])
}

/// `peek_page`: how the grades are computed, three short paragraphs.
pub fn peek_page(
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    const PARAS: [&str; 3] = [
        "PGI 2.0 grades states across outcomes, governance and infrastructure, then bands the total into Daksh, Ati-Uttam and Uttam.",
        "Each domain contributes a fixed weight, so a state can lead on access while trailing on learning outcomes.",
        "The 2023-24 round keeps the 2021 weights, which is why Gujarat's 3.5-point gain reads as real progress.",
    ];
    let mut body = v_flex().w_full().gap(px(8.0));
    for para in PARAS {
        body = body.child(div().w_full().ui(scale::FS_13).text_color(gpui::rgb(INK)).child(para.to_string()));
    }
    shell(vec![
        heading("PGI 2.0: how the grades are computed", "Methodology note"),
        body.into_any_element(),
        link_row("PGI 2.0 state scores", StoryPage::Pgi, link, on_link),
    ])
}

/// A small drawn page: title plus one or two quiet lines and its link.
fn simple_page(title: &str, lines: &[&str], link: Hsla, on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static) -> AnyElement {
    let mut body = v_flex().w_full().gap(px(6.0));
    for line in lines {
        body = body.child(div().w_full().ui(scale::FS_13).text_color(gpui::rgb(INK)).child(line.to_string()));
    }
    shell(vec![
        heading(title, "A saved page from the story"),
        body.into_any_element(),
        link_row("Methodology note", StoryPage::Peek, link, on_link),
    ])
}

/// `nas_page`: state report cards.
pub fn nas_page(
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    simple_page(
        "NAS 2024 · state report cards",
        &["Five states, one card each.", "Reading and arithmetic trail in the same two states as ever."],
        link,
        on_link,
    )
}

/// `ledger_page`: the local ledger app.
pub fn ledger_page(
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    simple_page("localhost:5173 · Ledger", &["The chart fix renders here.", "Hot-reloads on every save."], link, on_link)
}

/// `weather_page`: the weekend weather card (Personal space).
pub fn weather_page(
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    simple_page("Weekend weather · Bengaluru", &["Saturday 29° · scattered showers.", "Sunday 27° · clearing by noon."], link, on_link)
}

/// `generic_page`: any other saved tab, by title.
pub fn generic_page(
    title: &str,
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
) -> AnyElement {
    simple_page(title, &["A saved page from the story."], link, on_link)
}

/// Renders any story page. Page links and the answer action row report
/// through the screen's handlers, so clicks take the same path as the
/// keyboard.
pub fn render_page(
    page: &StoryPage,
    link: Hsla,
    on_link: impl Fn(&StoryPage, &mut Window, &mut App) + 'static,
    on_ask_more: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_done: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    match page {
        StoryPage::Pgi => pgi_page(link, on_link),
        StoryPage::Udise => udise_page(link, on_link),
        StoryPage::Dropout => generic_page("UDISE+ · dropout, secondary", link, on_link),
        StoryPage::Datagov => datagov_page(link, on_link),
        StoryPage::Answer => answer_page(link, on_link, on_ask_more, on_done),
        StoryPage::Peek => peek_page(link, on_link),
        StoryPage::Nas => nas_page(link, on_link),
        StoryPage::Ledger => ledger_page(link, on_link),
        StoryPage::Weather => weather_page(link, on_link),
        StoryPage::Generic(title) => generic_page(title, link, on_link),
    }
}
