//! `.zones`: the 3 × 3 drop grid shown over a panel while a tab is dragged
//! (edges dashed accent-ring on accent-soft at .55, centre solid at .9), and
//! the ghost tab that follows the pointer.

use aui_icons::{icon, IconName};
use aui_motion::{spring_phase, SpringKind};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, Div, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

/// `.zones{inset:8px;gap:4px}`.
const INSET: f32 = 8.0;
const GAP: f32 = 4.0;
/// `.zones i{border:1.5px dashed;opacity:.55}` / `.c{opacity:.9}`.
const ZONE_BORDER: f32 = 1.5;
const EDGE_OPACITY: f32 = 0.55;
const CENTRE_OPACITY: f32 = 0.9;
/// `.ghost{width:150px;height:34px;padding:0 10px;gap:7px}`.
const GHOST_W: f32 = 150.0;
const GHOST_H: f32 = 34.0;
const GHOST_PAD: f32 = 10.0;
const GHOST_GAP: f32 = 7.0;
const GHOST_ICON: f32 = 12.0;

/// Where a dragged tab would land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Split to the left.
    Left,
    /// Split to the right.
    Right,
    /// Split above.
    Top,
    /// Split below.
    Bottom,
    /// Merge as a tab.
    Centre,
}

/// The overlay. Build with [`drop_zones`].
#[derive(IntoElement)]
pub struct DropZones {
    id: ElementId,
    visible: bool,
    hot: Option<DropZone>,
}

/// An overlay to place inside a `relative` panel body.
pub fn drop_zones(id: impl Into<ElementId>, visible: bool) -> DropZones {
    DropZones { id: id.into(), visible, hot: None }
}

impl DropZones {
    /// The zone under the pointer (drawn solid at full opacity).
    pub fn hot(mut self, zone: Option<DropZone>) -> Self {
        self.hot = zone;
        self
    }
}

impl RenderOnce for DropZones {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let show = spring_phase((self.id.clone(), "show"), self.visible, SpringKind::Gentle, window, cx).clamp(0.0, 1.0);
        if show <= 0.0 && !self.visible {
            return div().absolute().inset_0().invisible().into_any_element();
        }
        let zone = |kind: Option<DropZone>, grow: f32| {
            let centre = kind == Some(DropZone::Centre);
            let hot = kind.is_some() && kind == self.hot;
            let mut z: Div = div()
                .h_full()
                .min_w(px(0.0))
                .rounded(px(scale::R_SM))
                .bg(p.accent_soft)
                .border(px(ZONE_BORDER))
                .border_color(p.accent_ring)
                .opacity(if hot { 1.0 } else if centre { CENTRE_OPACITY } else { EDGE_OPACITY });
            // 1fr 2fr 1fr: the middle track is twice the outer ones.
            z = z.flex_grow(grow).flex_basis(px(0.0));
            if !(centre || hot) {
                z = z.border_dashed();
            }
            z
        };
        let row = |left: Option<DropZone>, mid: Option<DropZone>, right: Option<DropZone>, weight: f32| {
            h_flex()
                .w_full()
                .gap(px(GAP))
                .flex_basis(px(0.0))
                .flex_grow(weight)
                .child(zone(left, 1.0))
                .child(zone(mid, 2.0))
                .child(zone(right, 1.0))
        };
        v_flex()
            .id(self.id)
            .absolute()
            .inset(px(INSET))
            .gap(px(GAP))
            .opacity(show)
            .child(row(None, Some(DropZone::Top), None, 1.0))
            .child(row(Some(DropZone::Left), Some(DropZone::Centre), Some(DropZone::Right), 2.0))
            .child(row(None, Some(DropZone::Bottom), None, 1.0))
            .into_any_element()
    }
}

/// The ghost tab that follows the pointer while dragging. Build with [`tab_ghost`].
///
/// The design rotates it −2°; gpui has no element rotation, so the ghost is
/// drawn straight (recorded in `docs/03` as a known gap).
#[derive(IntoElement)]
pub struct TabGhost {
    icon: Option<IconName>,
    label: SharedString,
}

/// A ghost for a tab.
pub fn tab_ghost(icon: Option<IconName>, label: impl Into<SharedString>) -> TabGhost {
    TabGhost { icon, label: label.into() }
}

impl RenderOnce for TabGhost {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut g = h_flex()
            .w(px(GHOST_W))
            .h(px(GHOST_H))
            .flex_none()
            .px(px(GHOST_PAD))
            .gap(px(GHOST_GAP))
            .rounded(px(scale::R_SM))
            .bg(p.surface_3)
            .border_1()
            .border_color(p.line_strong)
            .shadow(p.shadow(2))
            .text_color(p.ink)
            .ui(scale::FS_12)
            .whitespace_nowrap();
        if let Some(glyph) = self.icon {
            g = g.child(icon(glyph).size(px(GHOST_ICON)).color(p.ink));
        }
        g.child(self.label)
    }
}
