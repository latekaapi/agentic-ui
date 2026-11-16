//! D6 · the right pane's chrome (docs/43 §2.2, §2.3, §2.11; canvas v49
//! `D6Header`, `D6Carousel`). The transcript row's toggles drive a live strip:
//! click Files with the pane closed → it opens on Files; another toggle slides
//! (240 ms ease-out) under the pane's tab row (docs/83 §6); the lit toggle
//! closes. Beside it: an open file's tab and sub-header and the shared state
//! blocks.

use aui::workbench::{
    pane_action, pane_strip, pane_subheader, pane_tabs, pane_toggle, pushed_row, secondary_action, skeleton_rows, state_block, strip_position,
    strip_visible, PaneTab,
};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

const SCREENS: [(IconName, &str); 3] = [(IconName::Git, "Changes"), (IconName::Folder, "Files"), (IconName::Globe, "Page")];
const PANE_W: f32 = 360.0;

/// The pane's tab header as Baaz draws it (docs/83 §6): the bar's 44 with its
/// `line-region`, the tab row inside — `active` lit, or the file's tab when
/// one is open.
pub fn tab_header(id: &str, active: &str, file: Option<&str>, cx: &App) -> Div {
    let p = cx.aui().colors;
    let mut tabs: Vec<PaneTab> = SCREENS.iter().map(|(_, n)| PaneTab::new(*n, *n, file.is_none() && *n == active)).collect();
    if let Some(f) = file {
        tabs.push(PaneTab::new("file", f.to_string(), true).closable());
    }
    div()
        .w_full()
        .h(cx.aui().metrics.bar)
        .flex_none()
        .border_b_1()
        .border_color(p.line_region)
        .child(pane_tabs(SharedString::from(id.to_string()), tabs).on_add(false, |_, _, _| {}).on_maximise(false, |_, _, _| {}).on_close(|_, _, _| {}))
}

#[derive(Clone, Copy)]
struct Pane {
    open: bool,
    screen: usize,
}

pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let state = window.use_keyed_state("d6-pane-state", cx, |_, _| Pane { open: true, screen: 1 });
    let pane = *state.read(cx);
    let p = cx.aui().colors;

    // The transcript's title row: title, then Changes · Files · Page,
    // Terminal, a hairline, the session menu.
    let mut toggles = h_flex().gap(px(2.0)).items_center();
    for (i, (glyph, label)) in SCREENS.iter().enumerate() {
        let state = state.clone();
        toggles = toggles.child(pane_toggle(("d6-toggle", i), *glyph, *label, pane.open && pane.screen == i).on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                if s.open && s.screen == i {
                    s.open = false;
                } else {
                    s.open = true;
                    s.screen = i;
                }
                cx.notify();
            });
        }));
    }
    let title_row = h_flex()
        .w_full()
        .h(cx.aui().metrics.bar)
        .pl(px(16.0))
        .pr(px(10.0))
        .items_center()
        .rounded(px(8.0))
        .bg(p.surface_2)
        .border_1()
        .border_color(p.line)
        .child(div().flex_1().text_px(13.0).font_weight(FontWeight::SEMIBOLD).text_color(p.fg).child("Merge the Diff and Git kinds into one Changes view."))
        .child(toggles)
        .child(pane_action("d6-terminal", IconName::Terminal, "Terminal"))
        .child(div().mx(px(4.0)).w(px(1.0)).h(px(16.0)).bg(p.line))
        .child(pane_action("d6-menu", IconName::Dots, "Open session menu"));

    // The strip, a window onto one screen.
    let pos = strip_position("d6-strip", pane.screen, window, cx);
    let visible = strip_visible(pos, SCREENS.len());
    let screens = (0..SCREENS.len())
        .map(|i| {
            visible(i).then(|| {
                let row = (i == 1).then(|| {
                    pane_subheader(cx)
                        .px(px(6.0))
                        .child(div().flex_1().pl(px(4.0)).text_px(12.0).text_color(p.fg_muted).child("baaz"))
                        .child(pane_action("d6-filter", IconName::Search, "Filter files"))
                });
                let body: AnyElement = match i {
                    0 => state_block("d6-clean", IconName::Check, "No changes").line("The working tree is clean.").into_any_element(),
                    1 => skeleton_rows(&[(0, 64.0), (0, 88.0), (0, 56.0), (0, 112.0), (1, 72.0), (1, 96.0), (0, 60.0)], cx).into_any_element(),
                    _ => state_block("d6-nopage", IconName::Globe, "No page yet")
                        .line("When Muse opens a page for this session, it shows here. Or type an address.")
                        .into_any_element(),
                };
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .children(row)
                    .child(div().flex_1().min_h(px(0.0)).child(body))
                    .into_any_element()
            })
        })
        .collect();
    let swipe_state = state.clone();
    // The tab row stays put over the strip; only the screens slide.
    let tabs = {
        let (select, close) = (state.clone(), state.clone());
        let tabs: Vec<PaneTab> = SCREENS.iter().enumerate().map(|(i, (_, n))| PaneTab::new(*n, *n, i == pane.screen)).collect();
        div().w_full().h(cx.aui().metrics.bar).flex_none().border_b_1().border_color(p.line_region).child(
            pane_tabs("d6-tabs", tabs)
                .on_select(move |id, _, cx| {
                    let i = SCREENS.iter().position(|(_, n)| *n == id.as_ref()).unwrap_or(0);
                    select.update(cx, |s, cx| {
                        s.screen = i;
                        cx.notify();
                    })
                })
                .on_close(move |_, _, cx| {
                    close.update(cx, |s, cx| {
                        s.open = false;
                        cx.notify();
                    })
                }),
        )
    };
    let strip = v_flex().w(px(PANE_W)).h(px(320.0)).rounded(px(8.0)).border_1().border_color(p.line).overflow_hidden().child(tabs).child(
        div().flex_1().min_h(px(0.0)).child(pane_strip("d6-strip", pos, px(PANE_W), screens).on_swipe(move |dir, _, cx| {
            swipe_state.update(cx, |s, cx| {
                s.screen = (s.screen as i32 + dir).clamp(0, SCREENS.len() as i32 - 1) as usize;
                cx.notify();
            })
        })),
    );
    let pane_el = if pane.open {
        strip.into_any_element()
    } else {
        div().w(px(PANE_W)).h(px(320.0)).flex().items_center().justify_center().text_color(p.fg_muted).text_px(13.2).child("Pane closed").into_any_element()
    };

    // The pushed view's row and the shared states, at rest.
    let pushed = div()
        .w(px(PANE_W))
        .rounded(px(8.0))
        .bg(p.surface_2)
        .border_1()
        .border_color(p.line)
        .overflow_hidden()
        .child(tab_header("d6-pushed-tabs", "Files", Some("right.rs"), cx))
        .child(pushed_row("d6-pushed", "crates/baaz/src/right.rs").action(pane_action(
            "d6-open-ext",
            IconName::External,
            "Open in default app",
        )));
    let not_shown = div().w(px(PANE_W)).h(px(220.0)).rounded(px(8.0)).bg(p.surface_2).border_1().border_color(p.line).child(
        state_block("d6-binary", IconName::File, "app.db")
            .line("SQLite database · 4.2 MB. Baaz shows text, Markdown and images here.")
            .action(secondary_action("d6-open-default", "Open in default app", Some(IconName::External), cx)),
    );

    v_flex()
        .gap(px(16.0))
        .font_family(scale::FONT_UI)
        .child(title_row)
        .child(h_flex().gap(px(16.0)).items_start().child(pane_el).child(v_flex().gap(px(16.0)).child(pushed).child(not_shown)))
        .into_any_element()
}
