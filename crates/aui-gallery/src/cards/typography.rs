//! Card 02 · Typography. The size/weight scale with a live sample per step,
//! a body paragraph and a tabular-numbers block. Reproduces
//! `design/src/cards/foundations/02-type.html` at 760×560.

use aui_tokens::{ActiveAui, scale, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// Width of the key column: `.scale{grid-template-columns:64px 1fr}`.
const KEY_COL: f32 = 64.0;
/// Column gap of `.scale` (`gap:6px 16px`).
const SCALE_COL_GAP: f32 = 16.0;
/// Row gap of `.scale`.
const SCALE_ROW_GAP: f32 = 6.0;
/// `.scale .k{font:500 11px/1 …}` and the `data-v` labels: line height 1.
const LH_FLAT: f32 = 1.0;
/// Line height of the 24 px display row (`line-height:1.2` on that row only).
const LH_DISPLAY: f32 = 1.2;
/// `.sample{gap:20px;margin-top:18px}`.
const SAMPLE_GAP: f32 = 20.0;
/// `.sample{margin-top:18px}`.
const SAMPLE_TOP: f32 = 18.0;
/// `.sample p{max-width:38ch}` — 38 characters of Geist at 14 px (≈9.2 px/ch).
const SAMPLE_MEASURE: f32 = 350.0;
/// `.sample p{margin:0 0 8px}`.
const SAMPLE_PARA_BOTTOM: f32 = 8.0;
/// `.ds-note{max-width:80ch}` — 80 characters of Geist at 12 px (the `0`
/// advance measures ≈8.1 px in gpui, fixed by matching the reference line breaks).
const NOTE_MEASURE: f32 = 650.0;
/// `.scale{align-items:baseline}` puts the mono key on the sample's baseline.
/// gpui's `items_baseline` bottom-aligns a text run instead of aligning its
/// baseline, so each row states the key's top offset directly: the sample's
/// baseline within the row, less the key's baseline within its own 11 px/1
/// line box. Measured against `design/reference/cards/02-type.png`.
const KEY_BASELINE_OFFSETS: [f32; 9] = [11.8, 12.0, 7.0, 5.0, 4.0, 2.5, 1.5, 4.0, 5.0];
/// `.caps` in this card inherits the body line height (1.5), not the 1.0 of
/// `TextRole::Caps`.
const CAPS_LH: f32 = scale::LH_UI;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .child(scale_grid(p))
        .child(samples(p))
        .child(
            div()
                .mt(px(scale::SP_4))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child(
                    "Weights: 400 body, 500 emphasis and controls, 600 headings and labels. \
                     Never 700 except display. Uppercase only at 11 px with .08 em tracking.",
                ),
        )
        .into_any_element()
}

/// The `.scale` two-column grid: mono key on the left, a live sample on the right.
fn scale_grid(p: Palette) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap(px(SCALE_ROW_GAP))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[0],
            "24 / 600",
            div()
                .ui(scale::FS_24)
                .line_height(relative(LH_DISPLAY))
                .semibold()
                .text_color(p.ink)
                .child("Welcome back, Bharani"),
        ))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[1],
            "20 / 600",
            div()
                .ui(scale::FS_20)
                .semibold()
                .text_color(p.ink)
                .child("Run schema and backfill in order"),
        ))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[2],
            "16 / 500",
            div()
                .ui(scale::FS_16)
                .medium()
                .text_color(p.ink)
                .child("Tighten address validation and add coverage"),
        ))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[3],
            "14 / 400",
            div()
                .ui(scale::FS_14)
                .text_color(p.ink)
                .child("Focused tests pass. Running lint on the touched files before summarising the change."),
        ))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[4],
            "13 / 400",
            div()
                .ui(scale::FS_13)
                .text_color(p.ink)
                .child("Default UI size: sidebar rows, card headers, composer text."),
        ))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[5],
            "12 / 500",
            div()
                .ui(scale::FS_12)
                .medium()
                .text_color(p.ink_2)
                .child("Secondary metadata · 49 m ago · Opus · Claude Max"),
        ))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[6],
            "11 / 600",
            div()
                .ui(scale::FS_11)
                .semibold()
                .text_color(p.ink_3)
                .child("Eyebrow label".to_uppercase()),
        ))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[7],
            "mono 12",
            div()
                .font_family(scale::FONT_MONO)
                .text_px(scale::FS_12)
                .line_height(relative(scale::LH_UI)) // `.scale` rows inherit the body line height
                .text_color(p.ink)
                .child("src/checkout/validators.ts:12 · export function validateAddress"),
        ))
        .child(scale_row(
            p,
            KEY_BASELINE_OFFSETS[8],
            "mono 13",
            h_flex()
                .font_family(scale::FONT_MONO)
                .text_px(scale::FS_13)
                .line_height(relative(scale::LH_UI))
                .text_color(p.term_fg)
                .child(div().child("$ pnpm test --filter web-runtime"))
                .child(div().text_color(p.ansi_green).child("\u{a0}✓ 14 passed")),
        ))
}

/// One `.scale` row: the mono key, then the sample, baseline-aligned.
fn scale_row(p: Palette, key_top: f32, key: &'static str, sample: impl IntoElement) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_start()
        .gap(px(SCALE_COL_GAP))
        .child(
            div()
                .w(px(KEY_COL))
                .flex_none()
                .pt(px(key_top))
                .font_family(scale::FONT_MONO)
                .text_px(scale::FS_11)
                .line_height(relative(LH_FLAT))
                .medium()
                .text_color(p.ink_3)
                .child(key),
        )
        .child(div().flex_1().min_w(px(0.0)).child(sample))
}

/// The `.sample` two-column block: a body paragraph and tabular numbers.
fn samples(p: Palette) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_start()
        .mt(px(SAMPLE_TOP))
        .gap(px(SAMPLE_GAP))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .child(sample_caps(p, "Body, 14 / 1.65"))
                .child(
                    div()
                        .mb(px(SAMPLE_PARA_BOTTOM))
                        .max_w(px(SAMPLE_MEASURE))
                        .ui(scale::FS_14)
                        .line_height(relative(scale::LH_BODY))
                        .text_color(p.ink_2)
                        .child(
                            "Updated validation behaviour and added regression tests for US ZIP+4, \
                             Canadian postal codes, and a missing country. Nothing else in the checkout \
                             copy changed.",
                        ),
                ),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .child(sample_caps(p, "Numbers, tabular"))
                .child(
                    v_flex()
                        .mb(px(SAMPLE_PARA_BOTTOM))
                        .font_family(scale::FONT_MONO)
                        .text_px(scale::FS_13)
                        .line_height(relative(scale::LH_UI))
                        .text_color(p.ink_2)
                        .child("1,284 agents spawned")
                        .child("142 h agent time")
                        .child("\u{a0}\u{a0}\u{a0}96 PRs created"),
                ),
        )
}

/// The caps label above each sample column (`margin-bottom:8px`).
fn sample_caps(p: Palette, label: &'static str) -> impl IntoElement {
    div()
        .mb(px(scale::SP_3))
        .text_role(TextRole::Caps)
        .line_height(relative(CAPS_LH))
        .text_color(p.ink_3)
        .child(label.to_uppercase())
}
