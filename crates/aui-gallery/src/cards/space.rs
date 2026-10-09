//! Card 03 · Size ladder, shape, surface ladder, radius, elevation. The v2
//! foundations: one control height shared by controls and rows, a single
//! shape with its concentric rule, and the eight-step surface ladder.

use aui_tokens::{scale, ActiveAui, AuiStyled, Density, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// Gap below each section heading.
const SECTION_LABEL_GAP: f32 = 10.0;
/// Gap below each section body.
const SECTION_GAP: f32 = 22.0;
/// Gap between tiles in a row.
const SWATCH_GAP: f32 = 14.0;
/// Captions under tiles: mono 10 / 1.
const CAPTION_SIZE: f32 = 10.0;
/// Caption line height.
const LH_FLAT: f32 = 1.0;
/// `.rad div{width:56px;height:40px}`.
const RADIUS_TILE: (f32, f32) = (56.0, 40.0);
/// `.el div{width:120px;height:64px}`.
const ELEVATION_TILE: (f32, f32) = (120.0, 64.0);
/// `.ds-note` measure.
const NOTE_MEASURE: f32 = 650.0;
/// Width of each size-ladder box.
const LADDER_BOX_W: f32 = 120.0;
/// Height of each surface-ladder bar.
const SURFACE_BAR_H: f32 = 30.0;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .child(section_label(p, "Size ladder"))
        .child(size_ladder(p))
        .child(section_label(p, "Shape"))
        .child(shape_demo(p))
        .child(section_label(p, "Surface ladder"))
        .child(surface_ladder(p))
        .child(section_label(p, "Radius"))
        .child(radius_row(p))
        .child(section_label(p, "Elevation"))
        .child(elevation_row(p))
        .child(
            div()
                .mt(px(scale::SP_4))
                .max_w(px(NOTE_MEASURE))
                .text_role_in(TextRole::Caption, Density::Standard)
                .text_color(p.ink_3)
                .child(
                    "One control height (36, compact 28) shared by controls and rows. \
                     Concentric radii: an item inside a container steps down one shape step \
                     with a 2 px inset. Surfaces climb one step per nesting level.",
                ),
        )
        .into_any_element()
}

/// A section heading.
fn section_label(p: Palette, label: &'static str) -> impl IntoElement {
    div()
        .mb(px(SECTION_LABEL_GAP))
        .text_role_in(TextRole::Caption, Density::Standard)
        .medium()
        .text_color(p.ink_3)
        .child(label)
}

/// The v2 size ladder: control, row-2, bar and icon at both densities, each
/// as a labelled box drawn at its true height.
fn size_ladder(p: Palette) -> impl IntoElement {
    let mut row = h_flex().w_full().items_end().mb(px(SECTION_GAP)).gap(px(SWATCH_GAP));
    for (name, std, compact) in [
        ("control", scale::CTL_DEFAULT, scale::CTL_COMPACT),
        ("row-2", scale::ROW_2_DEFAULT, scale::ROW_2_COMPACT),
        ("bar", scale::BAR_DEFAULT, scale::BAR_COMPACT),
        ("icon", scale::ICON_DEFAULT, scale::ICON_COMPACT),
    ] {
        row = row.child(
            h_flex()
                .items_end()
                .gap(px(6.0))
                .child(ladder_box(p, std, format!("{name} {std}")))
                .child(ladder_box(p, compact, format!("{name}-c {compact}"))),
        );
    }
    row
}

/// One labelled box drawn at its true height.
fn ladder_box(p: Palette, height: f32, label: String) -> impl IntoElement {
    v_flex()
        .flex_none()
        .gap(px(4.0))
        .child(
            div()
                .w(px(LADDER_BOX_W))
                .h(px(height))
                .rounded(px(scale::SHAPE_ITEM))
                .border_1()
                .border_color(p.line_strong)
                .bg(p.surface_2),
        )
        .child(
            div()
                .font_family(scale::FONT_MONO)
                .text_px(CAPTION_SIZE)
                .line_height(relative(LH_FLAT))
                .medium()
                .text_color(p.ink_3)
                .child(label),
        )
}

/// The shape pair: a container (8) holding an item (6) at a 2 px inset, and
/// beside it the wrong same-radius pair labelled accordingly.
fn shape_demo(p: Palette) -> impl IntoElement {
    h_flex()
        .mb(px(SECTION_GAP))
        .gap(px(SWATCH_GAP))
        .child(
            v_flex()
                .gap(px(4.0))
                .child(
                    div()
                        .w(px(160.0))
                        .rounded(px(scale::SHAPE_CONTAINER))
                        .border_1()
                        .border_color(p.line_strong)
                        .bg(p.surface_2)
                        .p(px(2.0))
                        .child(
                            div()
                                .w_full()
                                .h(px(44.0))
                                .rounded(px(scale::SHAPE_ITEM))
                                .bg(p.surface_4)
                                .border_1()
                                .border_color(p.line),
                        ),
                )
                .child(shape_caption(p, "container 8 · item 6 · 2 px inset")),
        )
        .child(
            v_flex()
                .gap(px(4.0))
                .child(
                    div()
                        .w(px(160.0))
                        .rounded(px(scale::SHAPE_CONTAINER))
                        .border_1()
                        .border_color(p.line_strong)
                        .bg(p.surface_2)
                        .p(px(2.0))
                        .child(
                            div()
                                .w_full()
                                .h(px(44.0))
                                .rounded(px(scale::SHAPE_CONTAINER))
                                .bg(p.surface_4)
                                .border_1()
                                .border_color(p.danger),
                        ),
                )
                .child(shape_caption(p, "✗ same radius")),
        )
}

/// Mono caption under a shape tile.
fn shape_caption(p: Palette, label: &'static str) -> impl IntoElement {
    div()
        .font_family(scale::FONT_MONO)
        .text_px(CAPTION_SIZE)
        .line_height(relative(LH_FLAT))
        .medium()
        .text_color(p.ink_3)
        .child(label)
}

/// The eight-step surface ladder as nested bars, each labelled with its hex.
fn surface_ladder(p: Palette) -> impl IntoElement {
    let hex_of = |step: u8| {
        let c: Rgba = p.surface(step).into();
        let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", ch(c.r), ch(c.g), ch(c.b))
    };
    // Innermost first, then wrap outwards: step 8 sits inside step 7, and so on.
    let mut bar: AnyElement = div()
        .w_full()
        .h(px(SURFACE_BAR_H))
        .rounded(px(scale::SHAPE_ITEM))
        .bg(p.surface(8))
        .border_1()
        .border_color(p.line)
        .flex()
        .items_center()
        .px(px(8.0))
        .font_family(scale::FONT_MONO)
        .text_px(CAPTION_SIZE)
        .line_height(relative(LH_FLAT))
        .medium()
        .text_color(p.ink_3)
        .child(format!("surface-8 · {}", hex_of(8)))
        .into_any_element();
    for step in (1..=7).rev() {
        bar = div()
            .w_full()
            .rounded(px(scale::SHAPE_ITEM))
            .bg(p.surface(step))
            .border_1()
            .border_color(p.line)
            .p(px(6.0))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .font_family(scale::FONT_MONO)
                            .text_px(CAPTION_SIZE)
                            .line_height(relative(LH_FLAT))
                            .medium()
                            .text_color(p.ink_3)
                            .child(format!("surface-{step} · {}", hex_of(step))),
                    )
                    .child(div().flex_1().child(bar)),
            )
            .into_any_element();
    }
    div().w_full().mb(px(SECTION_GAP)).child(bar)
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
                .text_px(CAPTION_SIZE)
                .line_height(relative(LH_FLAT))
                .medium()
                .text_color(p.ink_3)
                .child(div().max_w(px(RADIUS_TILE.0)).child(label)),
        );
    }
    row
}

/// `.el`: the three shadow levels plus the flat docked-panel ground.
fn elevation_row(p: Palette) -> impl IntoElement {
    let mut row = h_flex().gap(px(18.0));
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
        .text_px(ELEVATION_LABEL_SIZE)
        .line_height(relative(LH_FLAT))
        .medium()
        .text_color(p.ink_3)
        .child(div().max_w(px(ELEVATION_TILE.0)).child(label))
}

/// `.el div` label size.
const ELEVATION_LABEL_SIZE: f32 = 11.0;
