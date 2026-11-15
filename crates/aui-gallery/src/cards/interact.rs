//! Interact cards: the interaction primitives — states, rows and surfaces.

use aui::data::kbd;
use aui::interact::{interactive, menu_panel, nested_radius, row, surface, Shape};
use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

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
/// Hover, pressed and focus-visible are pinned with `.preview(...)`, the
/// one model's capture hook; every tile paints through [`interactive`].
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
            "Hover, press and selection are the one model (aui::interact::paint): overlays, no tween.",
            cx,
        ))
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

/// `interact/surfaces`: explicit surface steps (no ambient elevation) and
/// the one floating panel, beside the concentric pair (right: inner keeps
/// `nested_radius`; wrong: inner repeats the outer radius).
pub fn build_surfaces(_window: &mut Window, cx: &mut App) -> AnyElement {
    let step_label = |text: &'static str, cx: &App| div().role_in(TextRole::Caption, cx).medium().text_color(cx.aui().colors.fg).child(text);
    let pad = cx.aui().metrics.item_px;
    let steps = h_flex()
        .w_full()
        .gap(cx.aui().metrics.gap)
        .child(surface(1, div().p(pad).child(step_label("surface(1)", cx))).shape(Shape::Container))
        .child(surface(2, div().p(pad).child(step_label("surface(2) · card", cx))).shape(Shape::Container))
        .child(surface(3, div().p(pad).child(step_label("surface(3)", cx))).shape(Shape::Container))
        .child(surface(4, div().p(pad).child(step_label("surface(4) · input", cx))).shape(Shape::Item));
    let floating = div().w(px(240.0)).child(menu_panel(div().p(pad).child(step_label("menu_panel · the one float", cx)), cx));

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
        .child(section("Explicit steps", cx, steps))
        .child(section("Floating", cx, floating))
        .child(section(
            "Concentric: right / wrong",
            cx,
            h_flex()
                .w_full()
                .gap(cx.aui().metrics.gap)
                .child(v_flex().items_start().gap(cx.aui().metrics.gap).child(pair(true, cx)).child(caption("inner = outer − inset", cx)))
                .child(v_flex().items_start().gap(cx.aui().metrics.gap).child(pair(false, cx)).child(caption("inner repeats the outer", cx))),
        ))
        .into_any_element()
}
