//! D6 · the right pane's chrome (docs/43 §2.2, §2.3, §2.11; canvas v49
//! `D6Header`, `D6Carousel`). The transcript row's toggles drive a live strip:
//! click Files with the pane closed → it opens on Files; another toggle slides
//! (240 ms ease-out); the lit toggle closes. Beside it: the pushed-view row and
//! the shared state blocks.

use aui::workbench::{
    pane_action, pane_row, pane_row_fade, pane_strip, pane_toggle, pushed_row, secondary_action, skeleton_rows, state_block, strip_position,
    strip_visible, PANE_ROW_H,
};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

const SCREENS: [(IconName, &str); 3] = [(IconName::Git, "Changes"), (IconName::Folder, "Files"), (IconName::Globe, "Page")];
const PANE_W: f32 = 360.0;

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
        .h(px(PANE_ROW_H))
        .pl(px(16.0))
        .pr(px(10.0))
        .items_center()
        .rounded(px(8.0))
        .bg(p.surface_2)
        .shadow(p.edge())
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
                let (glyph, name) = SCREENS[i];
                let state = state.clone();
                let mut row = pane_row(("d6-row", i), glyph, name).on_close(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.open = false;
                        cx.notify();
                    })
                });
                if i == 1 {
                    row = row.action(pane_action("d6-filter", IconName::Search, "Filter files"));
                }
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
                    .child(row)
                    .child(div().flex_1().min_h(px(0.0)).child(body))
                    .child(pane_row_fade(cx))
                    .into_any_element()
            })
        })
        .collect();
    let swipe_state = state.clone();
    let strip = div().w(px(PANE_W)).h(px(320.0)).rounded(px(8.0)).shadow(p.edge()).child(
        pane_strip("d6-strip", pos, px(PANE_W), screens).on_swipe(move |dir, _, cx| {
            swipe_state.update(cx, |s, cx| {
                s.screen = (s.screen as i32 + dir).clamp(0, SCREENS.len() as i32 - 1) as usize;
                cx.notify();
            })
        }),
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
        .shadow(p.edge())
        .overflow_hidden()
        .child(pushed_row("d6-pushed", "right.rs").context("crates/baaz/src").back_label("Back to files").width(px(PANE_W)).action(pane_action(
            "d6-open-ext",
            IconName::External,
            "Open in default app",
        )));
    let not_shown = div().w(px(PANE_W)).h(px(220.0)).rounded(px(8.0)).bg(p.surface_2).shadow(p.edge()).child(
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
