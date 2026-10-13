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
const LIGHTS_MARGIN_RIGHT: f32 = 8.0;
/// Native traffic-light geometry (macOS metrics: each light is 12 pt across,
/// centres 20 pt apart, the first light's left edge 12 pt from the window's
/// left edge). A host that keeps the system's own lights reserves
/// [`NATIVE_LIGHTS_WIDTH`] at the header's leading edge via
/// the shell's bar (`CHROME_LEAD`) and positions the window with
/// [`traffic_light_position`]. The reservation lands on the shell's collapsed
/// rail with lights, so the two must never drift (see the unit test below).
pub const NATIVE_LIGHT_DIAM: f32 = 12.0;
/// Centre-to-centre stride of the native traffic lights.
pub const NATIVE_LIGHT_STRIDE: f32 = 20.0;
/// Left edge of the first native light, from the window's left edge.
pub const NATIVE_LIGHTS_X: f32 = 12.0;
/// Reserved leading footprint: first-light offset + two strides + one
/// diameter + the trailing margin shared with the painted lights.
pub const NATIVE_LIGHTS_WIDTH: f32 =
    NATIVE_LIGHTS_X + 2.0 * NATIVE_LIGHT_STRIDE + NATIVE_LIGHT_DIAM + LIGHTS_MARGIN_RIGHT;
/// Glyph size of the `+` in the right header (`.btn.icon.xs` with a 12 px icon).
const XS_GLYPH: f32 = 12.0;

/// Where a host window should place macOS's native traffic lights so their
/// centre is the header's centre at every density: pass this to
/// `TitlebarOptions.traffic_light_position` when the window keeps the native
/// lights and the sidebar header reserves their footprint with
/// the shell's bar.
pub fn traffic_light_position(cx: &App) -> gpui::Point<gpui::Pixels> {
    gpui::Point::new(px(NATIVE_LIGHTS_X), cx.aui().metrics.header * 0.5 - px(NATIVE_BUTTON_FRAME * 0.5))
}

/// The standard window button's frame is 14 pt square (the 12 pt light sits
/// inside it); gpui places that frame's origin at the position, so a light's
/// centre lands at y + 7. Measured through the live frame 2026-10-13.
pub const NATIVE_BUTTON_FRAME: f32 = 14.0;

/// A plain header cell: 44 px row, gap 6, padding 0 10. Build with [`header_cell`].
#[derive(IntoElement)]
pub struct HeaderCell {
    id: ElementId,
    children: Vec<AnyElement>,
    bare: bool,
}

/// An empty header cell.
pub fn header_cell(id: impl Into<ElementId>) -> HeaderCell {
    HeaderCell { id: id.into(), children: Vec::new(), bare: false }
}

impl HeaderCell {
    /// The v3 sheet's title row ([`crate::shell::AppShell::sheets`]): the
    /// row owns the height (32) and the side insets (16 / 10), so the cell
    /// fills it with no padding of its own.
    pub fn bare(mut self, bare: bool) -> Self {
        self.bare = bare;
        self
    }

    /// Adds a child.
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.children.push(el.into_any_element());
        self
    }
}

impl RenderOnce for HeaderCell {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.bare {
            return h_flex().id(self.id).size_full().min_w(px(0.0)).items_center().gap(px(CELL_GAP)).children(self.children);
        }
        header_row(cx).id(self.id).children(self.children)
    }
}

fn header_row(cx: &App) -> gpui::Div {
    h_flex().w_full().h(cx.aui().metrics.header).gap(px(CELL_GAP)).px(px(CELL_PAD)).min_w(px(0.0))
}


/// A muted ghost icon button in a header cell, wired to an optional handler.
fn ghost(id: ElementId, name: &'static str, glyph: IconName, handler: Option<ClickHandler>) -> crate::data::Button {
    let mut b = icon_button((id, name), glyph).ghost().muted().size(ButtonSize::Sm);
    if let Some(h) = handler {
        b = b.on_click(move |e, w, cx| h(e, w, cx));
    }
    b
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
    on_expand_sidebar: Option<ClickHandler>,
}

/// The centre header: provider mark + worktree name + branch tag, spacer,
/// overflow menu, right-pane toggle. Nothing else lives here.
pub fn centre_header(id: impl Into<ElementId>, title: impl Into<SharedString>) -> CentreHeader {
    CentreHeader {
        id: id.into(),
        provider: None,
        glyph: None,
        title: title.into(),
        branch: None,
        trailing: None,
        on_overflow: None,
        on_toggle_right: None,
        on_expand_sidebar: None,
    }
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

    /// Shows the sidebar toggle at the leading edge of the centre header and
    /// calls `f` when it is clicked. Set this while the sidebar is collapsed:
    /// it is the affordance that brings the sidebar back (⌘B does the same).
    pub fn on_expand_sidebar(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_expand_sidebar = Some(Box::new(f));
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
        let expand = self.on_expand_sidebar.map(|h| ghost(id.clone(), "expand-sidebar", IconName::Sidebar, Some(h)));
        header_row(cx)
            .id(id.clone())
            .children(expand)
            .child(title)
            .child(div().flex_1())
            .child(ghost(id.clone(), "overflow", IconName::Dots, self.on_overflow))
            .child(ghost(id, "toggle-right", IconName::PanelRight, self.on_toggle_right))
    }
}

/// Right header cell. Build with [`right_header`].
///
/// The cell shows either a tab strip or a title (the tabs win when both are
/// set), or neither: a tabbed right pane needs no title. The trailing `+`,
/// spacer and close are unchanged by that choice.
#[derive(IntoElement)]
pub struct RightHeader {
    id: ElementId,
    title: Option<SharedString>,
    tabs: Option<TabStrip>,
    tabs_label: Option<SharedString>,
    on_add: Option<ClickHandler>,
    on_close: Option<ClickHandler>,
}

/// The right header: the pane's title or tab strip, `+`, spacer, close.
pub fn right_header(id: impl Into<ElementId>) -> RightHeader {
    RightHeader {
        id: id.into(),
        title: None,
        tabs: None,
        tabs_label: Some("Right pane".into()),
        on_add: None,
        on_close: None,
    }
}

impl RightHeader {
    /// The title shown when there is no tab strip. Either may be set; the
    /// tabs win. A tabbed pane sets no title.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The tab strip (rendered at the shell height), in place of the title.
    pub fn tabs(mut self, tabs: TabStrip) -> Self {
        self.tabs = Some(tabs.in_shell_header());
        self
    }

    /// The accessible name of the tab strip. It applies only when the strip
    /// carries no [`TabStrip::accessibility_label`] of its own.
    pub fn tabs_label(mut self, label: impl Into<SharedString>) -> Self {
        self.tabs_label = Some(label.into());
        self
    }

    /// Whether a tab strip was set.
    pub fn has_tabs(&self) -> bool {
        self.tabs.is_some()
    }

    /// Whether a title was set.
    pub fn has_title(&self) -> bool {
        self.title.is_some()
    }

    /// The selected tab's id, or `None` when there is no strip (or its
    /// `active` points past the tabs).
    pub fn selected_tab(&self) -> Option<&SharedString> {
        self.tabs.as_ref().and_then(|tabs| tabs.selected_id())
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
        let p = cx.aui().colors;
        let id = self.id.clone();
        // The strip carries the header's default label unless it names
        // itself; the title (or nothing) shows when there is no strip.
        let tabs = self.tabs.map(|strip| {
            if strip.has_accessibility_label() {
                strip
            } else {
                match self.tabs_label {
                    Some(label) => strip.accessibility_label(label),
                    None => strip,
                }
            }
        });
        // The tabs stand in place of the title: with a strip set, a title
        // would double the header's subject, so it is not drawn.
        let title = if tabs.is_none() {
            self.title.map(|title| {
                div().min_w(px(0.0)).truncate().text_color(p.ink).ui(scale::FS_13).semibold().child(title).into_any_element()
            })
        } else {
            None
        };
        // `+` draws only when someone handles it: a pane with fixed tabs has
        // nothing to add, and a dead button is worse than none.
        let add = self.on_add.map(|h| {
            icon_button((id.clone(), "add"), IconName::Plus)
                .ghost()
                .muted()
                .size(ButtonSize::Xs)
                .icon_size(px(XS_GLYPH))
                .on_click(move |e, w, cx| h(e, w, cx))
        });
        h_flex()
            .id(id.clone())
            .w_full()
            .h(cx.aui().metrics.header)
            .gap(px(TABS_GAP))
            .pl(px(TABS_PAD_LEFT))
            .pr(px(TABS_PAD_RIGHT))
            .min_w(px(0.0))
            .children(tabs)
            .children(title)
            .children(add)
            .child(div().flex_1())
            .child(ghost(id, "close", IconName::X, self.on_close))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{tab_strip, TabItem};
    use aui_icons::IconName;

    #[test]
    fn native_lights_reservation_is_the_bar_lead_minus_the_gap() {
        // 12 pt offset + two 20 pt strides + 12 pt light + the 8 pt trailing
        // margin shared with the painted lights.
        assert_eq!(NATIVE_LIGHT_DIAM, 12.0);
        assert_eq!(NATIVE_LIGHT_STRIDE, 20.0);
        assert_eq!(NATIVE_LIGHTS_X, 12.0);
        assert_eq!(NATIVE_LIGHTS_WIDTH, 72.0);
    }

    fn right_pane_tabs(active: usize) -> TabStrip {
        tab_strip(
            "hd-tabs",
            vec![
                TabItem::new("changes", "Changes", IconName::Git).closable(false),
                TabItem::new("files", "Files", IconName::Folder).closable(false),
                TabItem::new("browser", "Browser", IconName::Globe).closable(false),
            ],
            active,
        )
    }

    #[test]
    fn right_header_without_tabs_or_title_has_neither() {
        // The tabbed right pane needs no title: a bare header carries no
        // strip, no title and no selection, with the trailing controls alone.
        let header = right_header("hd-right");
        assert!(!header.has_tabs());
        assert!(!header.has_title());
        assert_eq!(header.selected_tab(), None);
    }

    #[test]
    fn right_header_title_shows_without_tabs() {
        let header = right_header("hd-right").title("Files");
        assert!(!header.has_tabs());
        assert!(header.has_title());
        assert_eq!(header.selected_tab(), None);
    }

    #[test]
    fn right_header_selected_tab_tracks_the_strip() {
        let header = right_header("hd-right").tabs(right_pane_tabs(2));
        assert!(header.has_tabs());
        assert_eq!(header.selected_tab(), Some(&SharedString::from("browser")));
    }

    #[test]
    fn right_header_out_of_range_strip_selects_nothing() {
        let header = right_header("hd-right").tabs(right_pane_tabs(9));
        assert!(header.has_tabs());
        assert_eq!(header.selected_tab(), None);
    }
}
