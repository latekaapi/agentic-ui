//! `.app`: grid 252 | 1fr | 400 (392 in card 10) × 44 | 1fr. Three header
//! cells, one per column, and the three panes beneath them; the dividers
//! between columns run from the top of the header to the bottom of the pane.
//!
//! The right column collapses to zero on the layout spring when
//! [`AppShell::right_open`] is false; the sidebar column shrinks to the rail
//! width when [`AppShell::sidebar_open`] is false.

use aui_motion::{spring_px, SpringKind};
use aui_tokens::{scale, ActiveAui};
use gpui::{div, prelude::*, px, AnyElement, App, Div, ElementId, IntoElement, Pixels, Window};
use gpui_kit::base::{h_flex, v_flex};

/// Sidebar column width (`grid-template-columns: 252px …`).
pub const SIDEBAR_WIDTH: f32 = 252.0;
/// Right column width in the app (400; card 10 uses 392).
pub const RIGHT_WIDTH: f32 = 400.0;
/// The collapsed sidebar rail (⌘B).
pub const RAIL_WIDTH: f32 = 48.0;

/// The shell. Build with [`app_shell`].
#[derive(IntoElement)]
pub struct AppShell {
    id: ElementId,
    sidebar_width: Pixels,
    right_width: Pixels,
    right_open: bool,
    sidebar_open: bool,
    framed: bool,
    header_sidebar: Option<AnyElement>,
    header_centre: Option<AnyElement>,
    header_right: Option<AnyElement>,
    sidebar: Option<AnyElement>,
    centre: Option<AnyElement>,
    right: Option<AnyElement>,
}

/// An empty shell with the default column widths.
pub fn app_shell(id: impl Into<ElementId>) -> AppShell {
    AppShell {
        id: id.into(),
        sidebar_width: px(SIDEBAR_WIDTH),
        right_width: px(RIGHT_WIDTH),
        right_open: true,
        sidebar_open: true,
        framed: false,
        header_sidebar: None,
        header_centre: None,
        header_right: None,
        sidebar: None,
        centre: None,
        right: None,
    }
}

impl AppShell {
    /// Overrides the sidebar column width.
    pub fn sidebar_width(mut self, width: impl Into<Pixels>) -> Self {
        self.sidebar_width = width.into();
        self
    }

    /// Overrides the right column width.
    pub fn right_width(mut self, width: impl Into<Pixels>) -> Self {
        self.right_width = width.into();
        self
    }

    /// Whether the right pane is open; the column animates on the layout spring.
    pub fn right_open(mut self, open: bool) -> Self {
        self.right_open = open;
        self
    }

    /// Whether the sidebar is expanded (false = the 48 px rail).
    pub fn sidebar_open(mut self, open: bool) -> Self {
        self.sidebar_open = open;
        self
    }

    /// Draws the window frame the design card shows: line-strong border,
    /// radius 12, elevation 3. Apps fill the window instead.
    pub fn framed(mut self, framed: bool) -> Self {
        self.framed = framed;
        self
    }

    /// The sidebar header cell content.
    pub fn header_sidebar(mut self, el: impl IntoElement) -> Self {
        self.header_sidebar = Some(el.into_any_element());
        self
    }

    /// The centre header cell content.
    pub fn header_centre(mut self, el: impl IntoElement) -> Self {
        self.header_centre = Some(el.into_any_element());
        self
    }

    /// The right header cell content (the pane's tab strip).
    pub fn header_right(mut self, el: impl IntoElement) -> Self {
        self.header_right = Some(el.into_any_element());
        self
    }

    /// The sidebar pane.
    pub fn sidebar(mut self, el: impl IntoElement) -> Self {
        self.sidebar = Some(el.into_any_element());
        self
    }

    /// The centre pane (transcript + composer).
    pub fn centre(mut self, el: impl IntoElement) -> Self {
        self.centre = Some(el.into_any_element());
        self
    }

    /// The right pane (workbench).
    pub fn right(mut self, el: impl IntoElement) -> Self {
        self.right = Some(el.into_any_element());
        self
    }
}

impl RenderOnce for AppShell {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let header_h = cx.aui().metrics.header;
        let id = self.id.clone();

        let sidebar_target = if self.sidebar_open { self.sidebar_width } else { px(RAIL_WIDTH) };
        let sidebar_w = spring_px((id.clone(), "sidebar-width"), sidebar_target, SpringKind::Layout, window, cx).max(px(0.0));
        let right_target = if self.right_open { self.right_width } else { px(0.0) };
        let right_w = spring_px((id.clone(), "right-width"), right_target, SpringKind::Layout, window, cx).max(px(0.0));
        let right_inner = self.right_width;

        // Header cells: surface-1, bottom hairline. The sidebar cell owns the
        // first divider (its right border) and the right cell the second (its
        // left border), so each lines up with the pane border beneath it.
        let cell = |d: Div| d.h_full().flex_none().flex().items_center().min_w(px(0.0)).overflow_hidden().bg(p.surface_1);
        let header = h_flex()
            .w_full()
            .h(header_h)
            .flex_none()
            .border_b_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .child(cell(div()).w(sidebar_w).border_r_1().border_color(p.line).children(self.header_sidebar))
            .child(cell(div()).flex_1().children(self.header_centre))
            .child(
                cell(div())
                    .w(right_w)
                    .child(div().h_full().w(right_inner).flex_none().flex().items_center().border_l_1().border_color(p.line).children(self.header_right)),
            );

        let panes = h_flex()
            .w_full()
            .flex_1()
            .min_h(px(0.0))
            .items_stretch()
            .child(
                div()
                    .w(sidebar_w)
                    .h_full()
                    .flex_none()
                    .overflow_hidden()
                    .border_r_1()
                    .border_color(p.line)
                    .bg(p.surface_1)
                    .children(self.sidebar),
            )
            .child(div().flex_1().h_full().min_w(px(0.0)).overflow_hidden().bg(p.bg).children(self.centre))
            .child(
                div()
                    .w(right_w)
                    .h_full()
                    .flex_none()
                    .overflow_hidden()
                    .child(div().w(right_inner).h_full().flex_none().border_l_1().border_color(p.line).bg(p.surface_1).children(self.right)),
            );

        let mut root = v_flex().id(id).size_full().overflow_hidden().bg(p.bg).text_color(p.ink).child(header).child(panes);
        if self.framed {
            root = root.rounded(px(scale::R_LG)).border_1().border_color(p.line_strong).shadow(p.shadow(3));
        }
        root
    }
}
