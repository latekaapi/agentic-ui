//! The agent cursor: where an agent last acted on a page, drawn over it.
//!
//! A dot in the accent colour inside a soft ring, at the point the host
//! gives (the centre of the element the agent clicked, typed into or
//! scrolled), travelling there on the moderate spring when the point moves,
//! so the person sees the agent move from one control to the next. It
//! takes no pointer: the page under it keeps every click. The host draws it
//! in a layer over the page — positioned against that layer's top-left —
//! and names the agent for assistive tech.

use aui_motion::{spring_px, SpringKind};
use aui_tokens::{scale, ActiveAui};
use gpui::{div, point, prelude::*, px, AnyElement, App, ElementId, Pixels, Point, SharedString, Window};

/// The dot's diameter.
pub const AGENT_CURSOR_DOT: f32 = scale::SP_4;
/// The ring's diameter around it.
pub const AGENT_CURSOR_RING: f32 = scale::SP_6;

/// The cursor at `at` (relative to the layer the host draws it in), named
/// `label` (`"<agent> is acting here"`). Absolutely positioned: the host's
/// layer is the containing block.
pub fn agent_cursor(id: impl Into<ElementId>, at: Point<Pixels>, label: impl Into<SharedString>, window: &mut Window, cx: &mut App) -> AnyElement {
    let id: ElementId = id.into();
    let p = cx.aui().colors;
    let x = spring_px((id.clone(), "x"), at.x, SpringKind::Moderate, window, cx);
    let y = spring_px((id.clone(), "y"), at.y, SpringKind::Moderate, window, cx);
    let ring = px(AGENT_CURSOR_RING);
    let mut halo = p.accent;
    halo.a *= 0.22;
    let origin = point(x - ring / 2.0, y - ring / 2.0);
    div()
        .id(id)
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .size(ring)
        .rounded_full()
        .bg(halo)
        .border_1()
        .border_color(p.accent)
        .flex()
        .items_center()
        .justify_center()
        .role(gpui::Role::Image)
        .aria_label(label.into())
        .child(div().size(px(AGENT_CURSOR_DOT)).rounded_full().bg(p.accent).border_2().border_color(p.bg))
        .into_any_element()
}
