//! Card 04 · Motion. The four springs, the easings and the composite
//! primitives as live samples, each driven by the real `aui-motion`
//! primitive rather than a canned animation. Reproduces
//! `design/src/cards/foundations/04-motion.html` at 900×520.
//!
//! Every tile loops the way the CSS sample does (`infinite alternate`), so the
//! springs are exercised by toggling their target on a timer and letting the
//! solver travel — which is exactly what a button press or a chevron does.

use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_motion::{icon_morph, looping, spring_phase, CheckDraw, Easing, IconMorph, Loop, SpringKind};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .child(
            v_flex()
                .w_full()
                .gap(px(14.0))
                .child(
                    h_flex()
                        .w_full()
                        .items_stretch()
                        .gap(px(14.0))
                        .child(tile(&p, "spring · press", SpringKind::Press.describe(), press_stage(&p, window, cx)))
                        .child(tile(&p, "spring · swap", SpringKind::Swap.describe(), swap_stage(&p, window, cx)))
                        .child(tile(&p, "spring · layout", SpringKind::Layout.describe(), layout_stage(&p, window, cx)))
                        .child(tile(&p, "spring · gentle", SpringKind::Gentle.describe(), gentle_stage(&p, window, cx))),
                )
                .child(
                    h_flex()
                        .w_full()
                        .items_stretch()
                        .gap(px(14.0))
                        .child(tile(&p, "ease-out", "enter 220 ms · cubic(.16,1,.3,1)".into(), ease_out_stage(&p, window, cx)))
                        .child(tile(&p, "fade-rise", "base 180 ms · +4 px".into(), fade_rise_stage(&p, window, cx)))
                        .child(tile(&p, "icon morph", "send ↔ stop".into(), morph_stage(&p, window, cx)))
                        .child(tile(&p, "check draw · shake", "success 320 ms · error 500 ms".into(), check_shake_stage(&p, window, cx))),
                ),
        )
        .child(durations_row(&p))
        .child(
            div()
                .mt(px(12.0))
                .max_w(px(632.0))
                .text_role(TextRole::UiSmall)
                .text_color(p.ink_3)
                .child("Rules: animate transform and opacity, never layout when a fade will do; every animation has a readable resting state; motion explains change and is never ambient."),
        )
        .into_any_element()
}

/// `.m`: surface-1, 1 px line, radius 8, padding 12, min-height 120.
fn tile(p: &Palette, title: &'static str, meta: String, stage: AnyElement) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(120.0))
        .rounded(px(scale::R_MD))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .p(px(12.0))
        .child(
            div()
                .mb(px(2.0))
                .text_role(TextRole::Title)
                .text_px(scale::FS_12)
                .line_height(relative(1.2))
                .text_color(p.ink)
                .child(title),
        )
        .child(
            div()
                .mb(px(10.0))
                .font_family(scale::FONT_MONO)
                .text_px(scale::FS_11)
                .line_height(relative(1.3))
                .text_color(p.ink_3)
                .child(meta),
        )
        .child(h_flex().h(px(56.0)).items_center().gap(px(10.0)).child(stage))
}

fn caption(p: &Palette, text: &'static str) -> impl IntoElement {
    div().text_role(TextRole::Meta).text_color(p.ink_3).child(text)
}

fn ball(p: &Palette, size: Pixels) -> Div {
    div().flex_none().size(size).rounded_full().bg(p.accent)
}

/// Toggles a boolean every `half_period`, which is how the CSS
/// `infinite alternate` samples are reproduced with a real spring.
fn alternating(id: &'static str, half_period: Duration, window: &mut Window, cx: &mut App) -> bool {
    looping((id, "clock"), Loop::linear(half_period * 2), window, cx) >= 0.5
}

// spring · press: scale 1 → .8
fn press_stage(p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let on = alternating("m-press", Duration::from_millis(1600), window, cx);
    let t = spring_phase("m-press", on, SpringKind::Press, window, cx);
    let size = px(18.0 * (1.0 - 0.2 * t));
    h_flex()
        .gap(px(10.0))
        .child(div().flex_none().size(px(18.0)).flex().items_center().justify_center().child(ball(p, size)))
        .child(caption(p, "buttons, chips, rows"))
        .into_any_element()
}

// spring · swap: translateY 6 → −6, opacity .2 → 1
fn swap_stage(p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let on = alternating("m-swap", Duration::from_millis(1400), window, cx);
    let t = spring_phase("m-swap", on, SpringKind::Swap, window, cx);
    h_flex()
        .gap(px(10.0))
        .child(ball(p, px(18.0)).relative().top(px(6.0 - 12.0 * t)).opacity(0.2 + 0.8 * t.clamp(0.0, 1.0)))
        .child(caption(p, "icon morph, chevrons, tab indicator"))
        .into_any_element()
}

// spring · layout: width 40 → 160
fn layout_stage(p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let on = alternating("m-layout", Duration::from_millis(1800), window, cx);
    let t = spring_phase("m-layout", on, SpringKind::Layout, window, cx);
    div()
        .h(px(18.0))
        .w(px(40.0 + 120.0 * t))
        .rounded(px(scale::R_SM))
        .bg(p.accent_soft)
        .border_1()
        .border_color(p.accent)
        .into_any_element()
}

// spring · gentle: translateX 0 → 120
fn gentle_stage(p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let on = alternating("m-gentle", Duration::from_millis(2000), window, cx);
    let t = spring_phase("m-gentle", on, SpringKind::Gentle, window, cx);
    ball(p, px(18.0)).relative().left(px(120.0 * t)).into_any_element()
}

// ease-out: translateX 0 → 120 over 900 ms, alternating
fn ease_out_stage(p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = looping("m-ease-out", Loop::eased(Duration::from_millis(900), Easing::OUT).alternate(), window, cx);
    ball(p, px(18.0)).relative().left(px(120.0 * t)).into_any_element()
}

// fade-rise: opacity 0 → 1, translateY 4 → 0 over 1.2 s (std), alternating
fn fade_rise_stage(p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = looping("m-fade", Loop::eased(Duration::from_millis(1200), Easing::STD).alternate().resting(1.0), window, cx);
    h_flex()
        .gap(px(10.0))
        .child(ball(p, px(18.0)).relative().top(px(4.0 * (1.0 - t))).opacity(t))
        .child(caption(p, "streamed chunks, list rows"))
        .into_any_element()
}

// icon morph: 28 px accent square, arrow-up ↔ stop on the swap spring, 2.4 s cycle
fn morph_stage(p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let phase = looping("m-morph-clock", Loop::linear(Duration::from_millis(2400)), window, cx);
    let show_stop = (0.45..0.95).contains(&phase);
    let sample = icon_morph("m-morph", show_stop, window, cx);
    let glyph = |name: IconName| icon(name).size(px(14.0)).color(gpui::white());
    h_flex()
        .gap(px(10.0))
        .child(
            div()
                .flex_none()
                .size(px(28.0))
                .rounded(px(scale::R_SM))
                .bg(p.accent)
                .flex()
                .items_center()
                .justify_center()
                .child(IconMorph::new(sample, px(14.0), glyph(IconName::ArrowUp), glyph(IconName::Stop))),
        )
        .child(caption(p, "opacity + 3 px + scale .8"))
        .into_any_element()
}

// check draw (2 s cycle: draw over the first 30 %, hold, undraw) · shake (500 ms, every 1.1 s)
fn check_shake_stage(p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let phase = looping("m-check-clock", Loop::linear(Duration::from_millis(2000)).resting(0.5), window, cx);
    let progress = if phase < 0.3 {
        Easing::OUT.sample(phase / 0.3)
    } else if phase < 0.8 {
        1.0
    } else {
        1.0 - (phase - 0.8) / 0.2
    };
    let shake_phase = looping("m-shake-clock", Loop::linear(Duration::from_millis(1100)), window, cx);
    // `.shake`: 0/100 % 0, 20 % −4, 40 % +4, 60 % −3, 80 % +3, over the first 500 ms of each 1.1 s.
    let s = (shake_phase * 1100.0 / 500.0).min(1.0);
    let shake_x = if s >= 1.0 {
        0.0
    } else if s < 0.2 {
        -4.0 * (s / 0.2)
    } else if s < 0.4 {
        -4.0 + 8.0 * ((s - 0.2) / 0.2)
    } else if s < 0.6 {
        4.0 - 7.0 * ((s - 0.4) / 0.2)
    } else if s < 0.8 {
        -3.0 + 6.0 * ((s - 0.6) / 0.2)
    } else {
        3.0 - 3.0 * ((s - 0.8) / 0.2)
    };
    h_flex()
        .gap(px(10.0))
        .child(
            div()
                .flex_none()
                .size(px(22.0))
                .rounded_full()
                .bg(p.success_soft)
                .flex()
                .items_center()
                .justify_center()
                .child(CheckDraw::new(px(12.0), progress, icon(IconName::Check).size(px(12.0)).color(p.success))),
        )
        .child(
            // `.btn.danger.sm`: 24 px, padding 0 9, 12 px 500, danger text, danger-soft border.
            div()
                .relative()
                .left(px(shake_x))
                .h(px(scale::H_SM))
                .px(px(9.0))
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(p.danger_soft)
                .flex()
                .items_center()
                .text_role(TextRole::UiMedium)
                .text_px(scale::FS_12)
                .line_height(relative(1.0))
                .text_color(p.danger)
                .child("Deny"),
        )
        .into_any_element()
}

/// `.durs`: mono 11 / 1.4 ink-3 with ink 500 numbers, gap 18, margin-top 14.
fn durations_row(p: &Palette) -> impl IntoElement {
    let items = [
        ("fast 120", "hover tints"),
        ("base 180", "toggles, tabs"),
        ("enter 220", "cards, menus"),
        ("exit 160", "dismiss"),
        ("slow 280", "panel width"),
        ("reduced motion", "all 0"),
    ];
    let mut row = h_flex()
        .mt(px(14.0))
        .gap(px(18.0))
        .font_family(scale::FONT_MONO)
        .text_px(scale::FS_11)
        .line_height(relative(1.4))
        .text_color(p.ink_3);
    for (bold, rest) in items {
        // Items shrink and their trailing words wrap, as flex items do in the CSS.
        row = row.child(
            h_flex()
                .flex_1()
                .min_w(px(0.0))
                .flex_wrap()
                .gap(px(4.0))
                .child(div().whitespace_nowrap().font_weight(FontWeight::MEDIUM).text_color(p.ink).child(bold))
                .child(div().min_w(px(0.0)).child(rest)),
        );
    }
    row
}
