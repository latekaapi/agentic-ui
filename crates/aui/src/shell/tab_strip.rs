//! `.strip`: a 34 px tab strip (44 px inside the shell header) whose 2 px
//! ink indicator slides under the active tab on the swap spring. The close
//! affordance appears on hover or on the active tab; dirty tabs show a 6 px
//! ink-3 dot.

use std::{cell::RefCell, rc::Rc};

use aui_icons::{icon, IconName};
use aui_motion::{spring_px, tween, SpringKind, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, Bounds, ElementId, IntoElement, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, ElementExt};

use crate::util::{interaction_flags, TrackInteraction};

/// `.strip .t{gap:7px;padding:0 12px;font-size:12px}`; `.hd .tab{padding:0 10px}`.
const TAB_GAP: f32 = 7.0;
const TAB_PAD: f32 = 12.0;
const SHELL_TAB_PAD: f32 = 10.0;
/// Tab glyphs are 12 px.
const TAB_ICON: f32 = 12.0;
/// `.strip{padding:0 6px;gap:2px}`.
const STRIP_PAD: f32 = 6.0;
const STRIP_GAP: f32 = 2.0;
/// `.ind{height:2px;border-radius:2px 2px 0 0}` inset 8 px from the tab edges.
const INDICATOR_HEIGHT: f32 = 2.0;
/// The 1 px hairline the shell header draws under every cell.
const HAIRLINE: f32 = 1.0;
const INDICATOR_INSET: f32 = 8.0;
/// `.cl{width:14px;height:14px;border-radius:3px}` with a 10 px x.
const CLOSE_SIZE: f32 = 14.0;
const CLOSE_RADIUS: f32 = 3.0;
const CLOSE_ICON: f32 = 10.0;
/// `.dirty{width:6px;height:6px}`.
const DIRTY_SIZE: f32 = 6.0;
/// The badge pill inside a tab: `height:14px;padding:0 5px;font-size:9px`.
const BADGE_H: f32 = 14.0;
const BADGE_PAD: f32 = 5.0;
const BADGE_TEXT: f32 = 9.0;

/// One tab.
#[derive(Debug, Clone, PartialEq)]
pub struct TabItem {
    /// Stable id, reported by the select / close handlers.
    pub id: SharedString,
    /// Label.
    pub label: SharedString,
    /// Leading 12 px glyph.
    pub icon: Option<IconName>,
    /// A 12 px provider mark instead of the glyph (agent tabs).
    pub mark: Option<aui_icons::Provider>,
    /// A tiny pill after the label (`2 splits`).
    pub badge: Option<SharedString>,
    /// Unsaved changes: shows the dirty dot.
    pub dirty: bool,
    /// Shows the close affordance.
    pub closable: bool,
}

impl TabItem {
    /// A closable tab with a glyph.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, icon: IconName) -> Self {
        Self { id: id.into(), label: label.into(), icon: Some(icon), mark: None, badge: None, dirty: false, closable: true }
    }

    /// A tab led by a provider mark (an agent's TUI).
    pub fn with_mark(id: impl Into<SharedString>, label: impl Into<SharedString>, provider: aui_icons::Provider) -> Self {
        Self { id: id.into(), label: label.into(), icon: None, mark: Some(provider), badge: None, dirty: false, closable: true }
    }

    /// A tiny pill after the label.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Marks the tab dirty.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// Hides the close affordance (fixed panes).
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

type TabHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Previous-frame geometry the indicator is positioned from.
#[derive(Default)]
struct StripGeometry {
    strip: Option<Bounds<Pixels>>,
    tabs: Vec<Option<Bounds<Pixels>>>,
}

/// The tab strip. Build with [`tab_strip`].
#[derive(IntoElement)]
pub struct TabStrip {
    id: ElementId,
    tabs: Vec<TabItem>,
    active: usize,
    in_shell_header: bool,
    after_tabs: Vec<gpui::AnyElement>,
    trailing: Vec<gpui::AnyElement>,
    on_select: Option<TabHandler>,
    on_close: Option<TabHandler>,
}

/// A strip over `tabs` with `active` selected.
pub fn tab_strip(id: impl Into<ElementId>, tabs: Vec<TabItem>, active: usize) -> TabStrip {
    TabStrip { id: id.into(), tabs, active, in_shell_header: false, after_tabs: Vec::new(), trailing: Vec::new(), on_select: None, on_close: None }
}

impl TabStrip {
    /// The 44 px shell-header variant: no strip padding, 10 px tab padding.
    pub fn in_shell_header(mut self) -> Self {
        self.in_shell_header = true;
        self
    }

    /// A control placed right after the tabs (the `+`).
    pub fn after_tabs(mut self, el: impl IntoElement) -> Self {
        self.after_tabs.push(el.into_any_element());
        self
    }

    /// A control at the far right of the band (split, overflow).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing.push(el.into_any_element());
        self
    }

    /// Called with the tab id when a tab is clicked.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// Called with the tab id when a tab's close affordance is clicked.
    pub fn on_close(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TabStrip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let in_shell_header = self.in_shell_header;
        // In the shell header the strip fills the cell's *content* box: the
        // header's bottom hairline is not part of it, and the indicator hangs
        // below the strip to cover that hairline the way `.tab.on::after` does.
        let height = if self.in_shell_header { cx.aui().metrics.header - px(HAIRLINE) } else { cx.aui().metrics.tab_strip };
        let tab_pad = if self.in_shell_header { SHELL_TAB_PAD } else { TAB_PAD };

        let geometry = window
            .use_keyed_state((id.clone(), "geometry"), cx, |_, _| Rc::new(RefCell::new(StripGeometry::default())))
            .read(cx)
            .clone();

        // The indicator is placed from last frame's bounds; the first frame
        // draws none and asks for another frame. The captured strip bounds are
        // its border box while an absolute `left` is measured from the content
        // box, so the strip's own padding is taken back out.
        let strip_pad = if self.in_shell_header { 0.0 } else { STRIP_PAD };
        let target = {
            let g = geometry.borrow();
            match (g.strip, g.tabs.get(self.active).copied().flatten()) {
                (Some(strip), Some(tab)) => {
                    Some((tab.origin.x - strip.origin.x + px(INDICATOR_INSET - strip_pad), tab.size.width - px(2.0 * INDICATOR_INSET)))
                }
                _ => None,
            }
        };
        let indicator = match target {
            Some((left, width)) => {
                let left = spring_px((id.clone(), "indicator-left"), left, SpringKind::Swap, window, cx);
                let width = spring_px((id.clone(), "indicator-width"), width, SpringKind::Swap, window, cx);
                let bar = div()
                    .absolute()
                    .bottom(px(if in_shell_header { -INDICATOR_HEIGHT } else { -HAIRLINE }))
                    .left(left)
                    .w(width.max(px(0.0)))
                    .h(px(INDICATOR_HEIGHT))
                    .rounded_t(px(INDICATOR_HEIGHT))
                    .bg(p.ink);
                // In the shell header the bar straddles the header's bottom
                // hairline, and gpui paints a border after its children, so
                // the bar has to leave the surrounding paint order.
                Some(if in_shell_header { crate::overlay::popover_layer(bar).into_any_element() } else { bar.into_any_element() })
            }
            None => {
                window.request_animation_frame();
                None
            }
        };

        let mut strip = h_flex()
            .id(id.clone())
            .relative()
            .h(height)
            .flex_none()
            .gap(px(STRIP_GAP))
            .min_w(px(0.0))
            // `.strip{border-bottom:1px solid var(--line)}`; inside the shell the header cell owns it.
            .when(!self.in_shell_header, |d| d.px(px(STRIP_PAD)).border_b_1().border_color(p.line))
            .on_prepaint({
                let geometry = geometry.clone();
                move |bounds, _, _| geometry.borrow_mut().strip = Some(bounds)
            });

        let count = self.tabs.len();
        for (index, tab) in self.tabs.into_iter().enumerate() {
            let tab_id: ElementId = (id.clone(), SharedString::from(format!("tab-{}", tab.id))).into();
            let active = index == self.active;
            let (state, flags) = interaction_flags(tab_id.clone(), window, cx);
            let color = tween((tab_id.clone(), "color"), if active || flags.hovered { p.ink } else { p.ink_3 }, Tween::FAST, window, cx);
            let show_close = tab.closable && (active || flags.hovered);
            let close_opacity = tween((tab_id.clone(), "close"), if show_close { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);

            // Tabs keep their natural width; a strip short of room clips at
            // its edge (the design lets labels wrap; see docs/03 known gaps).
            let mut el = h_flex()
                .id(tab_id.clone())
                .relative()
                .h_full()
                .flex_none()
                .gap(px(TAB_GAP))
                .px(px(tab_pad))
                .text_color(color)
                .ui(scale::FS_12)
                .whitespace_nowrap()
                .cursor_pointer()
                .track_interaction(&state)
                .on_prepaint({
                    let geometry = geometry.clone();
                    move |bounds, _, _| {
                        let mut g = geometry.borrow_mut();
                        if g.tabs.len() < count {
                            g.tabs.resize(count, None);
                        }
                        g.tabs[index] = Some(bounds);
                    }
                });
            if let Some(glyph) = tab.icon {
                el = el.child(icon(glyph).size(px(TAB_ICON)).color(color));
            }
            if let Some(provider) = tab.mark {
                el = el.child(aui_icons::provider_mark(provider).size(px(TAB_ICON)));
            }
            el = el.child(tab.label.clone());
            if let Some(badge) = &tab.badge {
                el = el.child(crate::data::pill(badge.clone()).height(BADGE_H).font_size(BADGE_TEXT).padding_x(BADGE_PAD));
            }
            if tab.dirty {
                el = el.child(div().flex_none().size(px(DIRTY_SIZE)).rounded_full().bg(p.ink_3));
            }
            if tab.closable {
                let close_id: ElementId = (tab_id.clone(), "close").into();
                let mut close = div()
                    .id(close_id)
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(CLOSE_SIZE))
                    .rounded(px(CLOSE_RADIUS))
                    .opacity(close_opacity)
                    .hover(|s| s.bg(p.surface_3))
                    .child(icon(IconName::X).size(px(CLOSE_ICON)).color(color));
                if let Some(on_close) = self.on_close.clone() {
                    let tab_key = tab.id.clone();
                    close = close.on_click(move |_, w, cx| on_close(&tab_key, w, cx));
                }
                el = el.child(close);
            }
            if let Some(on_select) = self.on_select.clone() {
                let tab_key = tab.id.clone();
                el = el.on_click(move |_, w, cx| on_select(&tab_key, w, cx));
            }
            strip = strip.child(el);
        }
        strip = strip.children(self.after_tabs);
        if !self.trailing.is_empty() {
            strip = strip.child(div().flex_1().min_w(px(0.0))).children(self.trailing);
        }
        strip.children(indicator)
    }
}
