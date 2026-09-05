//! Card 03 · Spacing, radius, elevation. The seven spacing steps drawn to
//! scale, the six radii and the three shadow levels beside a flat panel.
//! Reproduces `design/src/cards/foundations/03-space.html` at 760×420.

use aui_tokens::{dark, scale, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.caps{margin-bottom:10px}` above every section of this card.
const SECTION_LABEL_GAP: f32 = 10.0;
/// `.sp`, `.rad`{margin-bottom:22px}.
const SECTION_GAP: f32 = 22.0;
/// `.sp{gap:14px}` and `.rad{gap:14px}`.
const SWATCH_GAP: f32 = 14.0;
/// `.el{gap:18px}`.
const ELEVATION_GAP: f32 = 18.0;
/// `.sp div{height:24px}` — the spacing bars.
const BAR_HEIGHT: f32 = 24.0;
/// `.sp div{border-radius:2px}` — a one-off hairline radius below `--r-xs`.
const BAR_RADIUS: f32 = 2.0;
/// `.sp div::after{margin-top:4px}` — the value caption under each bar.
const BAR_LABEL_GAP: f32 = 4.0;
/// `.sp div::after`, `.rad div{font:500 10px/1 var(--font-mono)}`.
const CAPTION_SIZE: f32 = 10.0;
/// `.el div{font:500 11px/1 var(--font-mono)}`.
const ELEVATION_LABEL_SIZE: f32 = 11.0;
/// Captions are set solid (`/1`).
const LH_FLAT: f32 = 1.0;
/// `.rad div{width:56px;height:40px}`.
const RADIUS_TILE: (f32, f32) = (56.0, 40.0);
/// `.el div{width:120px;height:64px}`.
const ELEVATION_TILE: (f32, f32) = (120.0, 64.0);
/// The subtitle beside the spacing bars: `font-size:12px;margin-left:12px`.
const SP_NOTE_INDENT: f32 = 12.0;
/// `.ds-note{max-width:80ch}` — 80 characters of Geist at 12 px (the `0`
/// advance measures ≈8.1 px in gpui, fixed by matching the reference line breaks).
const NOTE_MEASURE: f32 = 650.0;
/// `.caps` inherits the body line height (1.5) in this card.
const CAPS_LH: f32 = scale::LH_UI;

/// Builds the card content.
pub fn build(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let p = dark();
    v_flex()
        .w_full()
        .child(section_label(p, "Spacing"))
        .child(spacing_row(p))
        .child(section_label(p, "Radius"))
        .child(radius_row(p))
        .child(section_label(p, "Elevation"))
        .child(elevation_row(p))
        .child(
            div()
                .mt(px(scale::SP_4))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child(
                    "Control heights: 20 / 24 / 28 / 32 / 40. Sidebar rows 56 (two lines), \
                     transcript card headers 32, status bar 24, panel tab strip 32.",
                ),
        )
        .into_any_element()
}

/// A `.caps` section heading with its 10 px bottom margin.
fn section_label(p: Palette, label: &'static str) -> impl IntoElement {
    div()
        .mb(px(SECTION_LABEL_GAP))
        .text_role(TextRole::Caps)
        .line_height(relative(CAPS_LH))
        .text_color(p.ink_3)
        .child(label.to_uppercase())
}

/// `.sp`: the seven spacing steps drawn at their true width, each captioned
/// underneath by an absolutely positioned label (the CSS `::after`).
fn spacing_row(p: Palette) -> impl IntoElement {
    let steps = [
        scale::SP_1,
        scale::SP_2,
        scale::SP_3,
        scale::SP_4,
        scale::SP_5,
        scale::SP_6,
        scale::SP_7,
    ];
    let mut row = h_flex()
        .w_full()
        .items_end()
        .mb(px(SECTION_GAP))
        .gap(px(SWATCH_GAP));
    for step in steps {
        row = row.child(
            div()
                .relative()
                .flex_none()
                .w(px(step))
                .h(px(BAR_HEIGHT))
                .rounded(px(BAR_RADIUS))
                .border_1()
                .border_color(p.accent)
                .bg(p.accent_soft)
                .child(
                    div()
                        .absolute()
                        .top(px(BAR_HEIGHT + BAR_LABEL_GAP))
                        .left_0()
                        .font_family(scale::FONT_MONO)
                        .text_size(px(CAPTION_SIZE))
                        .line_height(relative(LH_FLAT))
                        .medium()
                        .whitespace_nowrap()
                        .text_color(p.ink_3)
                        .child(format!("{step}")),
                ),
        );
    }
    row.child(
        div()
            .flex_1()
            .min_w(px(0.0))
            .ml(px(SP_NOTE_INDENT))
            .ui(scale::FS_12)
            .text_color(p.ink_3)
            .child(
                "Padding belongs to the component; gaps to the parent. \
                 Never double up between nested containers.",
            ),
    )
}

/// `.rad`: six 56×40 tiles, one per radius step.
fn radius_row(p: Palette) -> impl IntoElement {
    let tiles = [
        (scale::R_XS, "4 chips"),
        (scale::R_SM, "6 controls"),
        (scale::R_MD, "8 cards"),
        (scale::R_LG, "12 panels"),
        (scale::R_XL, "16 dialogs"),
        (scale::R_FULL, "pill"),
    ];
    let mut row = h_flex().mb(px(SECTION_GAP)).gap(px(SWATCH_GAP));
    for (radius, label) in tiles {
        row = row.child(
            div()
                .flex_none()
                .w(px(RADIUS_TILE.0))
                .h(px(RADIUS_TILE.1))
                .rounded(px(radius))
                .border_1()
                .border_color(p.line_strong)
                .bg(p.surface_3)
                .flex()
                .items_center()
                .justify_center()
                .font_family(scale::FONT_MONO)
                .text_size(px(CAPTION_SIZE))
                .line_height(relative(LH_FLAT))
                .medium()
                .text_color(p.ink_3)
                // The tile is a grid cell in the CSS, so the caption wraps at
                // the tile width and the wrapped block stays centred.
                .child(div().max_w(px(RADIUS_TILE.0)).child(label)),
        );
    }
    row
}

/// `.el`: the three shadow levels plus the flat docked-panel ground.
fn elevation_row(p: Palette) -> impl IntoElement {
    let mut row = h_flex().gap(px(ELEVATION_GAP));
    for (level, label) in [
        (1u8, "1 · rows, chips"),
        (2, "2 · popovers, toasts"),
        (3, "3 · palette, dialogs"),
    ] {
        row = row.child(elevation_tile(p, label).bg(p.surface_1).shadow(p.shadow(level)));
    }
    row.child(elevation_tile(p, "flat · docked panels").bg(p.surface_2))
}

/// One 120×64 elevation tile without its ground, so the caller can pick the
/// surface and the shadow.
fn elevation_tile(p: Palette, label: &'static str) -> Div {
    div()
        .flex_none()
        .w(px(ELEVATION_TILE.0))
        .h(px(ELEVATION_TILE.1))
        .rounded(px(scale::R_MD))
        .border_1()
        .border_color(p.line)
        .flex()
        .items_center()
        .justify_center()
        .font_family(scale::FONT_MONO)
        .text_size(px(ELEVATION_LABEL_SIZE))
        .line_height(relative(LH_FLAT))
        .medium()
        .text_color(p.ink_3)
        .child(div().max_w(px(ELEVATION_TILE.0)).child(label))
}
