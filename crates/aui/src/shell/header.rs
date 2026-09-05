//! The three header cells (`.hd`): sidebar (traffic lights, back/forward,
//! search, sidebar toggle), centre (provider mark + worktree + branch,
//! overflow, right-pane toggle) and right (the pane's tab strip, `+`, close).

use aui_icons::{provider_mark, IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;

use crate::data::{icon_button, tag, ButtonSize};
use crate::shell::TabStrip;
use crate::util::ClickHandler;

/// `.hd{gap:6px;padding:0 10px}`.
const CELL_GAP: f32 = 6.0;
const CELL_PAD: f32 = 10.0;
/// `.hd .ttl{gap:7px}`.
const TITLE_GAP: f32 = 7.0;
/// `.hd.tabs{padding:0 4px 0 6px;gap:2px}`.
const TABS_PAD_LEFT: f32 = 6.0;
const TABS_PAD_RIGHT: f32 = 4.0;
const TABS_GAP: f32 = 2.0;
/// `.lights{gap:7px;margin:0 8px 0 2px} .lights i{width:11px;height:11px}`.
const LIGHT_SIZE: f32 = 11.0;
const LIGHT_GAP: f32 = 7.0;
const LIGHTS_MARGIN_LEFT: f32 = 2.0;
const LIGHTS_MARGIN_RIGHT: f32 = 8.0;
/// macOS traffic-light colours, drawn only when the shell paints its own
/// (the gallery card); real windows show the native ones.
const LIGHT_CLOSE: u32 = 0xFF5F57;
const LIGHT_MIN: u32 = 0xFEBC2E;
const LIGHT_ZOOM: u32 = 0x28C840;
/// Glyph size of the `+` in the right header (`.btn.icon.xs` with a 12 px icon).
const XS_GLYPH: f32 = 12.0;

/// A plain header cell: 44 px row, gap 6, padding 0 10. Build with [`header_cell`].
#[derive(IntoElement)]
pub struct HeaderCell {
    id: ElementId,
    children: Vec<AnyElement>,
}

/// An empty header cell.
pub fn header_cell(id: impl Into<ElementId>) -> HeaderCell {
    HeaderCell { id: id.into(), children: Vec::new() }
}

impl HeaderCell {
    /// Adds a child.
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.children.push(el.into_any_element());
        self
    }
}

impl RenderOnce for HeaderCell {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        header_row(cx).id(self.id).children(self.children)
    }
}

fn header_row(cx: &App) -> gpui::Div {
    h_flex().w_full().h(cx.aui().metrics.header).gap(px(CELL_GAP)).px(px(CELL_PAD)).min_w(px(0.0))
}

/// Sidebar header cell. Build with [`sidebar_header`].
#[derive(IntoElement)]
pub struct SidebarHeader {
    id: ElementId,
    traffic_lights: bool,
    can_go_back: bool,
    can_go_forward: bool,
    on_back: Option<ClickHandler>,
    on_forward: Option<ClickHandler>,
    on_search: Option<ClickHandler>,
    on_toggle_sidebar: Option<ClickHandler>,
}

/// The sidebar header: lights, back, forward, spacer, search, sidebar toggle.
pub fn sidebar_header(id: impl Into<ElementId>) -> SidebarHeader {
    SidebarHeader {
        id: id.into(),
        traffic_lights: false,
        can_go_back: true,
        can_go_forward: false,
        on_back: None,
        on_forward: None,
        on_search: None,
        on_toggle_sidebar: None,
    }
}

impl SidebarHeader {
    /// Paints the three traffic lights (for the gallery; real windows have native ones).
    pub fn traffic_lights(mut self, on: bool) -> Self {
        self.traffic_lights = on;
        self
    }

    /// Enables the back arrow.
    pub fn can_go_back(mut self, on: bool) -> Self {
        self.can_go_back = on;
        self
    }

    /// Enables the forward arrow (disabled at 45 % otherwise).
    pub fn can_go_forward(mut self, on: bool) -> Self {
        self.can_go_forward = on;
        self
    }

    /// Back arrow click.
    pub fn on_back(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Box::new(f));
        self
    }

    /// Forward arrow click.
    pub fn on_forward(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_forward = Some(Box::new(f));
        self
    }

    /// Search click (opens the command palette).
    pub fn on_search(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_search = Some(Box::new(f));
        self
    }

    /// Sidebar toggle click (⌘B).
    pub fn on_toggle_sidebar(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_sidebar = Some(Box::new(f));
        self
    }
}

fn ghost(id: ElementId, name: &'static str, glyph: IconName, handler: Option<ClickHandler>) -> crate::data::Button {
    let mut b = icon_button((id, name), glyph).ghost().muted().size(ButtonSize::Sm);
    if let Some(h) = handler {
        b = b.on_click(move |e, w, cx| h(e, w, cx));
    }
    b
}

impl RenderOnce for SidebarHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let mut row = header_row(cx).id(id.clone());
        if self.traffic_lights {
            let light = |rgb: u32| div().flex_none().size(px(LIGHT_SIZE)).rounded_full().bg(gpui::rgb(rgb));
            row = row.child(
                h_flex()
                    .flex_none()
                    .gap(px(LIGHT_GAP))
                    .ml(px(LIGHTS_MARGIN_LEFT))
                    .mr(px(LIGHTS_MARGIN_RIGHT) - px(CELL_GAP))
                    .child(light(LIGHT_CLOSE))
                    .child(light(LIGHT_MIN))
                    .child(light(LIGHT_ZOOM)),
            );
        }
        row.child(ghost(id.clone(), "back", IconName::ArrowLeft, self.on_back).disabled(!self.can_go_back))
            .child(ghost(id.clone(), "forward", IconName::ArrowRight, self.on_forward).disabled(!self.can_go_forward))
            .child(div().flex_1())
            .child(ghost(id.clone(), "search", IconName::Search, self.on_search))
            .child(ghost(id, "toggle-sidebar", IconName::Sidebar, self.on_toggle_sidebar))
    }
}

/// Centre header cell. Build with [`centre_header`].
#[derive(IntoElement)]
pub struct CentreHeader {
    id: ElementId,
    provider: Option<Provider>,
    glyph: Option<IconName>,
    title: SharedString,
    branch: Option<SharedString>,
    trailing: Option<AnyElement>,
    on_overflow: Option<ClickHandler>,
    on_toggle_right: Option<ClickHandler>,
}

/// The centre header: provider mark + worktree name + branch tag, spacer,
/// overflow menu, right-pane toggle. Nothing else lives here.
pub fn centre_header(id: impl Into<ElementId>, title: impl Into<SharedString>) -> CentreHeader {
    CentreHeader { id: id.into(), provider: None, glyph: None, title: title.into(), branch: None, trailing: None, on_overflow: None, on_toggle_right: None }
}

impl CentreHeader {
    /// The 16 px provider mark before the title.
    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = Some(provider);
        self
    }

    /// A 14 px ink-3 glyph before the title instead of a provider mark (the assistant's role icon).
    pub fn glyph(mut self, glyph: IconName) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// The branch tag after the title.
    pub fn branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// A status pill after the branch (`waiting`).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    /// Overflow (dots) click.
    pub fn on_overflow(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_overflow = Some(Box::new(f));
        self
    }

    /// Right-pane toggle click.
    pub fn on_toggle_right(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_right = Some(Box::new(f));
        self
    }
}

impl RenderOnce for CentreHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut title = h_flex().min_w(px(0.0)).gap(px(TITLE_GAP)).text_color(p.ink).ui(scale::FS_13).semibold();
        if let Some(provider) = self.provider {
            title = title.child(provider_mark(provider));
        }
        if let Some(glyph) = self.glyph {
            title = title.child(aui_icons::icon(glyph).color(p.ink_3));
        }
        title = title.child(div().min_w(px(0.0)).truncate().child(self.title));
        if let Some(branch) = self.branch {
            title = title.child(tag(branch));
        }
        if let Some(trailing) = self.trailing {
            title = title.child(trailing);
        }
        header_row(cx)
            .id(id.clone())
            .child(title)
            .child(div().flex_1())
            .child(ghost(id.clone(), "overflow", IconName::Dots, self.on_overflow))
            .child(ghost(id, "toggle-right", IconName::PanelRight, self.on_toggle_right))
    }
}

/// Right header cell. Build with [`right_header`].
#[derive(IntoElement)]
pub struct RightHeader {
    id: ElementId,
    tabs: Option<TabStrip>,
    on_add: Option<ClickHandler>,
    on_close: Option<ClickHandler>,
}

/// The right header: the pane's tab strip, `+`, spacer, close.
pub fn right_header(id: impl Into<ElementId>) -> RightHeader {
    RightHeader { id: id.into(), tabs: None, on_add: None, on_close: None }
}

impl RightHeader {
    /// The tab strip (rendered at the shell height).
    pub fn tabs(mut self, tabs: TabStrip) -> Self {
        self.tabs = Some(tabs.in_shell_header());
        self
    }

    /// `+` click (new tab).
    pub fn on_add(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_add = Some(Box::new(f));
        self
    }

    /// Close click (collapses the pane).
    pub fn on_close(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }
}

impl RenderOnce for RightHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let mut add = icon_button((id.clone(), "add"), IconName::Plus).ghost().muted().size(ButtonSize::Xs).icon_size(px(XS_GLYPH));
        if let Some(h) = self.on_add {
            add = add.on_click(move |e, w, cx| h(e, w, cx));
        }
        h_flex()
            .id(id.clone())
            .w_full()
            .h(cx.aui().metrics.header)
            .gap(px(TABS_GAP))
            .pl(px(TABS_PAD_LEFT))
            .pr(px(TABS_PAD_RIGHT))
            .min_w(px(0.0))
            .children(self.tabs)
            .child(add)
            .child(div().flex_1())
            .child(ghost(id, "close", IconName::X, self.on_close))
    }
}
