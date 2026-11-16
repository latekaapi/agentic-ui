//! Card 02 · Typography. The seven v2 text roles at both densities, each with
//! its exact size, whole-pixel line height and weight beside the sample.

use aui_tokens::{ActiveAui, AuiStyled, Density, Palette, TextRole, TextSize, TypeSpec};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// Width of the key column.
const KEY_COL: f32 = 150.0;
/// Gap between the key column and the samples.
const COL_GAP: f32 = 16.0;
/// Gap between rows.
const ROW_GAP: f32 = 10.0;
/// Gap between the standard and compact samples.
const SAMPLE_GAP: f32 = 16.0;
/// `.ds-note` measure.
const NOTE_MEASURE: f32 = 650.0;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .gap(px(ROW_GAP))
        .child(role_row(p, TextRole::Display, "Display", "Welcome back, Alex Rivera"))
        .child(role_row(p, TextRole::Title, "Title", "Run schema and backfill in order"))
        .child(role_row(p, TextRole::Subtitle, "Subtitle", "Tighten address validation and add coverage"))
        .child(role_row(
            p,
            TextRole::Body,
            "Body",
            "Focused tests pass. Running lint on the touched files before summarising the change.",
        ))
        .child(role_row(p, TextRole::Caption, "Caption", "Secondary metadata · 49 m ago · Opus · Claude Max"))
        .child(role_row(p, TextRole::Micro, "Micro", "Synced 2 min ago"))
        .child(role_row(p, TextRole::Mono, "Mono", "src/checkout/validators.ts:12 · export function validateAddress"))
        .child(
            div()
                .mt(px(6.0))
                .max_w(px(NOTE_MEASURE))
                .text_role(TextRole::Caption)
                .text_color(p.fg_muted)
                .child(
                    "Every size is a role: whole-pixel line heights, no global text scale, no px entry point. \
                     Weights: 600 Markdown headings only, 500 display, titles and emphasis, 400 everything else. \
                     Never uppercase a sample: there is no caps role.",
                ),
        )
        .into_any_element()
}

/// One role row: the mono key (`role · fs/lh · weight`, standard then
/// compact), then the standard and compact samples side by side.
fn role_row(p: Palette, role: TextRole, name: &'static str, sample: &'static str) -> impl IntoElement {
    let std = role.spec_in(Density::Comfortable, TextSize::Default);
    let compact = role.spec_in(Density::Compact, TextSize::Default);
    let (std_fs, std_lh, std_weight) = (std.size, std.line_height, std.weight);
    let (c_fs, c_lh) = (compact.size, compact.line_height);
    h_flex()
        .w_full()
        .items_start()
        .gap(px(COL_GAP))
        .child(
            v_flex()
                .w(px(KEY_COL))
                .flex_none()
                .font_family(aui_tokens::scale::FONT_MONO)
                .role_size(TextRole::Micro)
                .line_height(relative(1.4))
                .medium()
                .text_color(p.fg_muted)
                .child(div().text_color(p.ink).child(name))
                .child(format!("{std_fs}/{std_lh} · {}", weight_name(std_weight)))
                .child(format!("c: {c_fs}/{c_lh}")),
        )
        .child(
            h_flex()
                .flex_1()
                .min_w(px(0.0))
                .items_start()
                .gap(px(SAMPLE_GAP))
                .child(at(div().flex_1().min_w(px(0.0)), std).text_color(p.ink).child(sample))
                .child(
                    at(div().flex_1().min_w(px(0.0)), compact)
                        .text_color(p.fg_muted)
                        .child(sample),
                ),
        )
}

/// A sample drawn at an explicit density's spec (the card shows both side by
/// side, whatever the active density is).
fn at(el: gpui::Div, s: TypeSpec) -> gpui::Div {
    el.font_family(s.family).text_size(px(s.size)).line_height(px(s.line_height)).font_weight(s.weight)
}

/// `400` / `500` / `600` / `700` for the key column.
fn weight_name(weight: FontWeight) -> &'static str {
    if weight.0 >= 700.0 {
        "700"
    } else if weight.0 >= 600.0 {
        "600"
    } else if weight.0 >= 500.0 {
        "500"
    } else {
        "400"
    }
}
