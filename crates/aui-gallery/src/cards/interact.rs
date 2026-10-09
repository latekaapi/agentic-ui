//! Interact cards: the D1 primitives on the v2 tokens — states,
//! fluid-list, rows and surfaces. Dark.
//!
//! The fluid-list card honours `AUI_GALLERY_STEPS` (comma separated):
//! `hover:<n>` highlights that row, `select:<n>` selects it, `key:down` /
//! `key:up` move the highlight, `leave` clears it — so relay can capture
//! the highlight mid-glide with `--screenshot-delay`.

use aui::data::kbd;
use aui::interact::{fluid_list, interactive, menu_surface, nested_radius, row, surface, Axis, Shape};
use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

/// One gallery step from `AUI_GALLERY_STEPS`.
fn gallery_steps() -> Vec<String> {
    std::env::var("AUI_GALLERY_STEPS")
        .map(|value| value.split(',').map(|step| step.trim().to_string()).filter(|step| !step.is_empty()).collect())
        .unwrap_or_default()
}

fn caption(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div().role_in(TextRole::Caption, cx).medium().text_color(cx.aui().colors.fg_muted).child(text.into())
}

fn section(title: &'static str, cx: &App, content: impl IntoElement) -> impl IntoElement {
    let gap = cx.aui().metrics.gap;
    v_flex().w_full().gap(gap).child(caption(title, cx)).child(content)
}

/// `interact/states`: one control in rest / hover / selected / pressed /
/// focus-visible / disabled.
///
/// Hover and pressed are the control's own live state, pre-seeded through
/// the same [`aui::util::interaction`] entity the control reads; the
/// focus-visible tile takes real keyboard focus through the control's focus
/// handle with the keyboard-nav flag armed. Nothing here is a mock: every
/// tile paints through [`interactive`].
pub fn build_states(_window: &mut Window, cx: &mut App) -> AnyElement {
    use aui::interact::InteractState;
    let fixed = |hovered: bool, pressed: bool, focused: bool| InteractState { hovered, pressed, focused };
    let tile = |name: &'static str, cx: &App, control: aui::interact::Interactive| {
        let gap = cx.aui().metrics.gap;
        v_flex().flex_1().min_w(px(0.0)).gap(gap).child(caption(name, cx)).child(control)
    };
    // The control body: a ladder-height, text-axis-padded label, so the
    // card shows the control at its real size (Interactive itself never
    // sizes its child).
    let body = |cx: &App| {
        let metrics = cx.aui().metrics;
        h_flex()
            .h(metrics.control)
            .px(metrics.ctl_px)
            .items_center()
            .role_in(TextRole::Body, cx)
            .child("Archive")
    };
    let gap = cx.aui().metrics.gap;
    v_flex()
        .w_full()
        .gap(gap)
        .child(
            h_flex()
                .w_full()
                .gap(gap)
                .child(tile("rest", cx, interactive("interact-states-rest", body(cx)).label("Archive")))
                .child(tile("hover", cx, interactive("interact-states-hover", body(cx)).preview(fixed(true, false, false)).label("Archive")))
                .child(tile("selected", cx, interactive("interact-states-selected", body(cx)).selected(true).label("Archive"))),
        )
        .child(
            h_flex()
                .w_full()
                .gap(gap)
                .child(tile("pressed", cx, interactive("interact-states-pressed", body(cx)).preview(fixed(true, true, false)).label("Archive")))
                .child(tile("focus-visible", cx, interactive("interact-states-focus", body(cx)).focusable(true).preview(fixed(false, false, true)).label("Archive")))
                .child(tile("disabled", cx, interactive("interact-states-disabled", body(cx)).disabled(true).label("Archive"))),
        )
        .child(caption(
            "Heights follow metrics.control (36 standard / 28 compact) — flip the gallery Density switch.",
            cx,
        ))
        .into_any_element()
}

/// `interact/fluid-list`: an 8-row list with one selected, an `X` strip of
/// 4 chips, and an `XY` grid of 9 tiles.
pub fn build_fluid_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let mut highlighted: Option<usize> = None;
    let mut selected: Option<usize> = Some(2);
    for step in gallery_steps() {
        if let Some(rest) = step.strip_prefix("hover:") {
            highlighted = rest.parse::<usize>().ok();
        } else if let Some(rest) = step.strip_prefix("select:") {
            selected = rest.parse::<usize>().ok();
        } else if step == "key:down" {
            let at = highlighted.or(selected).unwrap_or(0);
            highlighted = Some((at + 1).min(7));
        } else if step == "key:up" {
            let at = highlighted.or(selected).unwrap_or(0);
            highlighted = Some(at.saturating_sub(1));
        } else if step == "leave" {
            highlighted = None;
        }
    }

    let mut list = fluid_list("interact-fluid-rows", Axis::Y)
        .gap_click(true)
        .label("Fluid rows")
        .selected(selected)
        .on_highlight(|_, _, _| {})
        .on_activate(|_, _, _| {});
    if let Some(at) = highlighted {
        list = list.highlighted(Some(at));
    }
    let titles = ["Checkout flow", "Notifier", "Auth refresh", "Observability", "Webhook retry", "Rate limits", "Migrations", "Docs pass"];
    let bylines = ["2 files changed", "awaiting approval", "PR #2491 open", "2 tests failed", "backoff 30 s", "900 req/min", "14 applied", "4 pages"];
    for (index, (title, byline)) in titles.iter().zip(bylines.iter()).enumerate() {
        let id = SharedString::from(format!("interact-fluid-row-{index}"));
        list = list.item(index, row(id, *title).two_line(*byline).selected(selected == Some(index)));
    }

    let mut strip = fluid_list("interact-fluid-chips", Axis::X).label("Fluid chips");
    for (index, name) in ["Agent", "Harness", "Docs", "Infra"].iter().enumerate() {
        let p = cx.aui().colors;
        let chip = div()
            .flex_none()
            .px(cx.aui().metrics.ctl_px)
            .py(cx.aui().metrics.item_px)
            .rounded(px(scale::SHAPE_ITEM))
            .border_1()
            .border_color(p.line)
            .role_in(TextRole::Caption, cx)
            .text_color(p.fg)
            .child(*name);
        strip = strip.item(index, chip);
    }

    let mut grid = fluid_list("interact-fluid-grid", Axis::XY).label("Fluid tiles");
    for index in 0..9 {
        let tile = surface(
            2,
            div()
                .px(cx.aui().metrics.item_px)
                .py(cx.aui().metrics.item_px)
                .child(div().role_in(TextRole::Caption, cx).text_color(cx.aui().colors.fg).child(format!("Tile {index}"))),
        )
        .shape(Shape::Item);
        grid = grid.item(index, tile);
    }

    v_flex()
        .w_full()
        .gap(cx.aui().metrics.inset)
        .child(section("Y · one highlight glides between rows", cx, list))
        .child(section("X · a chip strip", cx, strip))
        .child(section("XY · a tile grid", cx, grid))
        .into_any_element()
}

/// `interact/rows`: one-line and two-line rows with leading icons, counts,
/// kbd, actions and depths 0–2.
pub fn build_rows(_window: &mut Window, cx: &mut App) -> AnyElement {
    let count = |text: &'static str, cx: &App| div().role_in(TextRole::Caption, cx).text_color(cx.aui().colors.fg_muted).child(text);
    let lead = |glyph: IconName, cx: &App| icon(glyph).size(cx.aui().metrics.icon).color(cx.aui().colors.fg_muted);
    let action = |id: &'static str, glyph: IconName, cx: &App| {
        interactive(id, icon(glyph).size(cx.aui().metrics.icon).color(cx.aui().colors.fg_muted))
            .label(id)
            .into_any_element()
    };
    v_flex()
        .w_full()
        .gap(cx.aui().metrics.inset)
        .child(section(
            "One line",
            cx,
            v_flex().w_full().child(row("interact-row-one", "Checkout flow").leading(lead(IconName::Folder, cx)).trailing(count("12", cx))),
        ))
        .child(section(
            "Two lines",
            cx,
            v_flex().w_full().child(
                row("interact-row-two", "Auth refresh").two_line("PR #2491 open · 3 ahead").leading(lead(IconName::Git, cx)).trailing(kbd("↵")),
            ),
        ))
        .child(section(
            "Actions + selected",
            cx,
            v_flex().w_full().child(
                row("interact-row-actions", "Observability").leading(lead(IconName::Inbox, cx)).trailing(h_flex().gap(cx.aui().metrics.gap).child(action("interact-row-check", IconName::Check, cx))).selected(true),
            ),
        ))
        .child(section(
            "Depth 0 / 1 / 2",
            cx,
            v_flex()
                .w_full()
                .child(row("interact-row-d0", "acme-web").leading(lead(IconName::Folder, cx)))
                .child(row("interact-row-d1", "feature/checkout-flow").leading(lead(IconName::Folder, cx)).depth(1))
                .child(row("interact-row-d2", "withSession middleware").leading(lead(IconName::Doc, cx)).depth(2)),
        ))
        .into_any_element()
}

/// `interact/surfaces`: page → card → popover → menu nested with
/// `elevated`, each labelled with its step, beside the concentric pair
/// (right: inner keeps `nested_radius`; wrong: inner repeats the outer 8).
pub fn build_surfaces(_window: &mut Window, cx: &mut App) -> AnyElement {
    let step_label = |step: u8, name: &'static str, cx: &App| {
        div().role_in(TextRole::Caption, cx).medium().text_color(cx.aui().colors.fg).child(format!("step {step} · {name}"))
    };
    let nested = surface(
        1,
        div()
            .p(cx.aui().metrics.item_px)
            .child(step_label(1, "page", cx))
            .child(
                surface(
                    1,
                    div()
                        .p(cx.aui().metrics.item_px)
                        .child(step_label(3, "card · elevated(2)", cx))
                        .child(
                            surface(
                                1,
                                div()
                                    .p(cx.aui().metrics.item_px)
                                    .child(step_label(5, "popover · elevated(2)", cx))
                                    .child(menu_surface(step_label(7, "menu · elevated(2)", cx))),
                            )
                            .elevated(2)
                            .shape(Shape::Item)
                            .shadow(true)
                            .ring(true),
                        ),
                )
                .elevated(2)
                .shape(Shape::Container)
                .shadow(true),
            ),
    )
    .shape(Shape::Container);

    let pair = |right: bool, cx: &App| {
        let outer = px(scale::SHAPE_CONTAINER);
        let inset = px(scale::SP_2);
        let inner = if right { nested_radius(outer, inset) } else { outer };
        div()
            .flex_none()
            .rounded(outer)
            .bg(cx.aui().colors.surface(3))
            .p(inset)
            .child(div().w(px(scale::SP_7)).h(px(scale::SP_6)).rounded(inner).bg(cx.aui().colors.surface(5)))
    };
    v_flex()
        .w_full()
        .gap(cx.aui().metrics.inset)
        .child(section("Nested elevation", cx, nested))
        .child(section(
            "Concentric: right / wrong",
            cx,
            h_flex()
                .w_full()
                .gap(cx.aui().metrics.gap)
                .child(v_flex().gap(cx.aui().metrics.gap).child(pair(true, cx)).child(caption("inner = outer − inset", cx)))
                .child(v_flex().gap(cx.aui().metrics.gap).child(pair(false, cx)).child(caption("inner repeats the outer 8", cx))),
        ))
        .into_any_element()
}
