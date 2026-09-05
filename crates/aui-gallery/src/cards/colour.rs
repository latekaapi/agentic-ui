//! Card 01 · Colour. Both palettes side by side: swatches, agent states, a
//! diff pair and the 16 ANSI colours. Reproduces
//! `design/src/cards/foundations/01-color.html` at 960×720.

use aui_motion::pulse_ring;
use aui_tokens::{dark, light, scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap(px(16.0))
                .child(theme_section("Dark · harness default", dark()))
                .child(theme_section("Light · assistant default", light())),
        )
        .child(
            div()
                .mt(px(12.0))
                .max_w(px(632.0)) // 80ch at 12 px Geist
                .text_role(TextRole::UiSmall)
                .text_color(cx.aui().colors.ink_3)
                .child(
                    "Accent is iris (blue-violet) and is the only decorative hue. Status colours carry meaning only: \
                     success, warning, danger, info. Agent states map onto them: running = accent, needs-you = warning, \
                     done = success, failed = danger. Neutrals are tinted toward the accent so greys read as chosen.",
                ),
        )
        .into_any_element()
}

fn theme_section(title: &'static str, p: Palette) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w(px(0.0))
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .bg(p.bg)
        .text_color(p.ink)
        .p(px(16.0))
        .child(
            div()
                .mb(px(12.0))
                .text_role(TextRole::Caps)
                .text_px(scale::FS_12)
                .line_height(relative(scale::LH_UI)) // h3 inherits the body line height
                .text_color(p.ink_3)
                .child(title.to_uppercase()),
        )
        .child(swatches(p))
        .child(states(p, title))
        .child(diff_pair(p))
        .child(ansi_grid(p))
}

fn swatches(p: Palette) -> impl IntoElement {
    let rows: [[(&str, Hsla, Hsla); 4]; 3] = [
        [
            ("bg", p.bg, p.ink_2),
            ("surface-1", p.surface_1, p.ink_2),
            ("surface-2", p.surface_2, p.ink_2),
            ("surface-3", p.surface_3, p.ink_2),
        ],
        [
            ("ink", p.ink, p.bg),
            ("ink-2", p.ink_2, gpui::white()),
            ("ink-3", p.ink_3, gpui::white()),
            ("line-strong", p.line_strong, p.ink_2),
        ],
        [
            ("accent", p.accent, gpui::white()),
            ("success", p.success, gpui::white()),
            ("warning", p.warning, gpui::white()),
            ("danger", p.danger, gpui::white()),
        ],
    ];
    let mut grid = v_flex().w_full().gap(px(8.0)).mb(px(12.0));
    for row in rows {
        let mut r = h_flex().w_full().gap(px(8.0));
        for (name, fill, label) in row {
            r = r.child(
                div()
                    .flex_1()
                    .h(px(44.0))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.line)
                    .bg(fill)
                    .flex()
                    .items_end()
                    .px(px(6.0))
                    .py(px(5.0))
                    .font_family(scale::FONT_MONO)
                    .text_px(10.0)
                    .line_height(relative(1.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(label)
                    .child(name),
            );
        }
        grid = grid.child(r);
    }
    grid
}

fn states(p: Palette, section: &'static str) -> impl IntoElement {
    // `.dot.pulse` on running and needs-you only.
    let items = [
        ("running", p.accent, true),
        ("needs you", p.warning, true),
        ("done", p.success, false),
        ("failed", p.danger, false),
        ("idle", p.ink_4, false),
    ];
    let mut row = h_flex().flex_wrap().gap(px(10.0)).mb(px(12.0));
    for (label, color, pulse) in items {
        row = row.child(
            h_flex()
                .gap(px(6.0))
                .text_role(TextRole::UiSmall)
                .text_color(p.ink_2)
                .child(pulse_ring(SharedString::from(format!("{section}-{label}")), px(7.0), color, pulse))
                .child(label),
        );
    }
    row
}

fn diff_pair(p: Palette) -> impl IntoElement {
    let line = |bg: Hsla, gutter_color: Hsla, text: &'static str| {
        h_flex()
            .w_full()
            .bg(bg)
            .child(
                div()
                    .w(px(28.0))
                    .pr(px(10.0))
                    .text_right()
                    .text_color(gutter_color)
                    .child("45"),
            )
            .child(div().whitespace_nowrap().child(text))
    };
    v_flex()
        .w_full()
        .rounded(px(scale::R_SM))
        .overflow_hidden()
        .mb(px(12.0))
        .text_role(TextRole::Mono)
        .text_color(p.ink)
        .child(line(p.diff_add, p.success, "+ if (!values.country) return { ok: false }"))
        .child(line(p.diff_del, p.danger, "- if (!values.country) return true;"))
}

fn ansi_grid(p: Palette) -> impl IntoElement {
    let colors = p.ansi16();
    let mut grid = v_flex().w_full().gap(px(4.0));
    for row in colors.chunks(8) {
        let mut r = h_flex().w_full().gap(px(4.0));
        for c in row {
            r = r.child(div().flex_1().h(px(18.0)).rounded(px(3.0)).bg(*c));
        }
        grid = grid.child(r);
    }
    grid
}
