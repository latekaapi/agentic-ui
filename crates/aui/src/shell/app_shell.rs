//! `.app`: grid 252 | 1fr | 400 (392 in card 10) × 44 | 1fr. Three header
//! cells, one per column, and the three panes beneath them; the dividers
//! between columns run from the top of the header to the bottom of the pane.
//!
//! The right column collapses to zero on the layout spring when
//! [`AppShell::right_open`] is false; the sidebar column shrinks to the rail
//! width when [`AppShell::sidebar_open`] is false.

use aui_motion::{spring_px, SpringKind};
use super::drag_region::drag_region;
use aui_tokens::{scale, ActiveAui};
use gpui::{div, prelude::*, px, AnyElement, App, Div, ElementId, IntoElement, Pixels, Window};
use gpui_kit::base::{h_flex, v_flex};

/// Sidebar column width (`grid-template-columns: 252px …`).
pub const SIDEBAR_WIDTH: f32 = 252.0;
/// Right column width in the app (400; card 10 uses 392).
pub const RIGHT_WIDTH: f32 = 400.0;
/// The collapsed sidebar rail (⌘B).
pub const RAIL_WIDTH: f32 = 48.0;
/// The collapsed sidebar column when the window's controls sit above it: the
/// macOS traffic lights own the top-left of the window whether the shell paints
/// them or the system does, so the rail column widens to clear them.
pub const RAIL_WIDTH_WITH_LIGHTS: f32 = 72.0;
/// Minimum sidebar width while resizing: rows need ~200 px at 12.5 px text.
/// Clamp every drag move with [`clamp_sidebar_width`].
pub const SIDEBAR_MIN_WIDTH: f32 = 180.0;
/// Maximum sidebar width while resizing: keeps the transcript usable.
pub const SIDEBAR_MAX_WIDTH: f32 = 420.0;

/// Clamps a drag width into the resizable range. The shell clamps its own
/// target the same way, but call this on every drag move before notifying so
/// the stored width never leaves the range.
pub fn clamp_sidebar_width(width: f32) -> f32 {
    width.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}
/// Minimum right column width while resizing: below this a diff hunk or a
/// file tree row stops staying readable.
/// Clamp every drag move with [`clamp_right_width`].
pub const RIGHT_MIN_WIDTH: f32 = 280.0;
/// Maximum right column width while resizing: keeps the centre transcript
/// usable on a 13-inch window.
pub const RIGHT_MAX_WIDTH: f32 = 720.0;

/// Clamps a drag width into the resizable range. The shell clamps its own
/// target the same way, but call this on every drag move before notifying so
/// the stored width never leaves the range.
pub fn clamp_right_width(width: f32) -> f32 {
    width.clamp(RIGHT_MIN_WIDTH, RIGHT_MAX_WIDTH)
}

/// The shell. Build with [`app_shell`].
#[derive(IntoElement)]
pub struct AppShell {
    id: ElementId,
    sidebar_width: Pixels,
    sidebar_min: Pixels,
    sidebar_max: Pixels,
    resizing: bool,
    draggable: bool,
    right_width: Pixels,
    right_min: Pixels,
    right_max: Pixels,
    right_open: bool,
    sidebar_open: bool,
    header_follows_sidebar: bool,
    traffic_lights: bool,
    framed: bool,
    header_sidebar: Option<AnyElement>,
    header_centre: Option<AnyElement>,
    header_right: Option<AnyElement>,
    sidebar: Option<AnyElement>,
    rail: Option<AnyElement>,
    centre: Option<AnyElement>,
    right: Option<AnyElement>,
}

/// An empty shell with the default column widths.
pub fn app_shell(id: impl Into<ElementId>) -> AppShell {
    AppShell {
        id: id.into(),
        sidebar_width: px(SIDEBAR_WIDTH),
        sidebar_min: px(SIDEBAR_MIN_WIDTH),
        sidebar_max: px(SIDEBAR_MAX_WIDTH),
        resizing: false,
        draggable: true,
        right_width: px(RIGHT_WIDTH),
        right_min: px(RIGHT_MIN_WIDTH),
        right_max: px(RIGHT_MAX_WIDTH),
        right_open: true,
        sidebar_open: true,
        header_follows_sidebar: true,
        traffic_lights: false,
        framed: false,
        header_sidebar: None,
        header_centre: None,
        header_right: None,
        sidebar: None,
        rail: None,
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

    /// Minimum sidebar width; the render target never goes below it while the
    /// sidebar is open. Defaults to [`SIDEBAR_MIN_WIDTH`].
    pub fn sidebar_min_width(mut self, min: impl Into<Pixels>) -> Self {
        self.sidebar_min = min.into();
        self
    }

    /// Maximum sidebar width; the render target never goes above it while the
    /// sidebar is open. Defaults to [`SIDEBAR_MAX_WIDTH`].
    pub fn sidebar_max_width(mut self, max: impl Into<Pixels>) -> Self {
        self.sidebar_max = max.into();
        self
    }

    /// A resize drag is in flight: the column feeds the width straight through
    /// and skips the layout spring so the divider tracks the pointer. When the
    /// drag ends the spring re-arms from the current width, so the pane
    /// settles with no jump. The app sets this from the resize handle's
    /// drag intents (see [`crate::shell::resize_handle`]).
    pub fn resizing(mut self, resizing: bool) -> Self {
        self.resizing = resizing;
        self
    }

    /// Wraps the header row in a window drag region: press-drag moves the
    /// window, double-click zooms (macOS titlebar behaviour, `zoom_window`
    /// elsewhere). On by default; interactive children keep their clicks.
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    /// Overrides the right column width.
    pub fn right_width(mut self, width: impl Into<Pixels>) -> Self {
        self.right_width = width.into();
        self
    }

    /// Minimum right column width; the render target never goes below it while
    /// the right column is open. Defaults to [`RIGHT_MIN_WIDTH`].
    pub fn right_min_width(mut self, min: impl Into<Pixels>) -> Self {
        self.right_min = min.into();
        self
    }

    /// Maximum right column width; the render target never goes above it while
    /// the right column is open. Defaults to [`RIGHT_MAX_WIDTH`].
    pub fn right_max_width(mut self, max: impl Into<Pixels>) -> Self {
        self.right_max = max.into();
        self
    }

    /// Whether the right pane is open; the column animates on the layout spring.
    pub fn right_open(mut self, open: bool) -> Self {
        self.right_open = open;
        self
    }

    /// Whether the sidebar is expanded (false = the rail).
    pub fn sidebar_open(mut self, open: bool) -> Self {
        self.sidebar_open = open;
        self
    }

    /// Whether the header row's sidebar cell follows the collapse (default
    /// true, today's behaviour: the cell shrinks to the rail with the pane).
    /// When false the cell keeps `sidebar_rest` width — and the divider under
    /// it — whether or not `sidebar_open`, so back/forward, search and the
    /// toggle stay where they are; only the pane below collapses to the rail
    /// and the centre header begins at the same x in both states.
    pub fn header_follows_sidebar(mut self, follows: bool) -> Self {
        self.header_follows_sidebar = follows;
        self
    }

    /// The window's controls sit in the shell's top-left corner (the gallery
    /// paints them; a real window's are native). The collapsed column widens to
    /// [`RAIL_WIDTH_WITH_LIGHTS`] so the lights never sit over the rail's cells.
    pub fn traffic_lights(mut self, on: bool) -> Self {
        self.traffic_lights = on;
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

    /// The collapsed sidebar pane: the rail that replaces [`AppShell::sidebar`]
    /// while `sidebar_open` is false. The expanded sidebar is never drawn into
    /// the rail column — without a rail the column is simply empty, so
    /// full-width content can never be clipped into it.
    pub fn rail(mut self, el: impl IntoElement) -> Self {
        self.rail = Some(el.into_any_element());
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

        let rail_width = if self.traffic_lights { px(RAIL_WIDTH_WITH_LIGHTS) } else { px(RAIL_WIDTH) };
        // The app clamps every drag move (see `clamp_sidebar_width`); the
        // shell clamps the target again so a stale or hand-set width can never
        // push the rows under their minimum. The rail is exempt: it owns the
        // collapsed column.
        let lo = f32::from(self.sidebar_min);
        let hi = f32::from(self.sidebar_max).max(lo);
        let sidebar_rest = px(f32::from(self.sidebar_width).clamp(lo, hi));
        let sidebar_target = if self.sidebar_open { sidebar_rest } else { rail_width };
        // Mid-drag the width feeds straight through: `spring_px` would chase a
        // moving target and the divider would lag the pointer. On mouse-up the
        // spring re-arms from the current width, so there is no jump.
        let sidebar_w = if self.resizing {
            sidebar_target
        } else {
            spring_px((id.clone(), "sidebar-width"), sidebar_target, SpringKind::Layout, window, cx).max(px(0.0))
        };
        let right_lo = f32::from(self.right_min);
        let right_hi = f32::from(self.right_max).max(right_lo);
        let right_rest = px(f32::from(self.right_width).clamp(right_lo, right_hi));
        let right_target = if self.right_open { right_rest } else { px(0.0) };
        let right_w = if self.resizing {
            right_target
        } else {
            spring_px((id.clone(), "right-width"), right_target, SpringKind::Layout, window, cx).max(px(0.0))
        };
        let right_inner = right_rest;
        // The pane keeps its resting width while the column springs, so the
        // content slides under the divider instead of reflowing every frame.
        // While collapsing, the expanded sidebar stays in the column and is
        // clipped by the shrinking width (the way a macOS sidebar closes); the
        // rail replaces it only once the spring has settled, so the column is
        // never an empty surface mid-motion. Expanding shows the sidebar at its
        // resting width from the first frame.
        let collapsing = !self.sidebar_open && sidebar_w > rail_width + px(1.0);
        let (pane, pane_width) = if self.sidebar_open || collapsing { (self.sidebar, sidebar_rest) } else { (self.rail, rail_width) };
        // A steady header row keeps the sidebar cell (and its divider) at the
        // resting width however the pane below moves.
        let (header_w, header_pane_w) =
            if self.header_follows_sidebar { (sidebar_w, pane_width) } else { (sidebar_rest, sidebar_rest) };

        // Header cells: surface-1, bottom hairline. The sidebar cell owns the
        // first divider (its right border) and the right cell the second (its
        // left border), so each lines up with the pane border beneath it.
        // Cells clip horizontally only: the right cell's tab strip draws its ink
        // indicator over the header's bottom hairline, the way `.tab.on::after`
        // does in the design, and a vertical clip would cut it in half.
        let cell = |d: Div| d.h_full().flex_none().flex().items_center().min_w(px(0.0)).overflow_x_hidden().bg(p.surface_1);
        let header_row = h_flex()
            .w_full()
            .h(header_h)
            .flex_none()
            .border_b_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .child(
                cell(div())
                    .w(header_w)
                    .border_r_1()
                    .border_color(p.line)
                    .child(div().h_full().w(header_pane_w).flex_none().flex().items_center().children(self.header_sidebar)),
            )
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
                    .child(div().w(pane_width).h_full().flex_none().overflow_hidden().children(pane)),
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

        // The window-wide `:focus-visible` approximation: a mouse press anywhere
        // in the shell disarms the focus ring, the next key press re-arms it.
        // The header drags the window by default (see `AppShell::draggable`);
        // the wrapper is unstyled, so on/off screenshots are identical.
        let header: AnyElement = if self.draggable {
            drag_region((id.clone(), "header-drag")).child(header_row).into_any_element()
        } else {
            header_row.into_any_element()
        };
        let mut root = crate::keys::track_pointer(v_flex().id(id).size_full().overflow_hidden().bg(p.bg).text_color(p.ink).child(header).child(panes));
        if self.framed {
            root = root.rounded(px(scale::R_LG)).border_1().border_color(p.line_strong).shadow(p.shadow(3));
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_drag_widths_in_range() {
        assert_eq!(clamp_sidebar_width(252.0), 252.0);
        assert_eq!(clamp_sidebar_width(0.0), SIDEBAR_MIN_WIDTH);
        assert_eq!(clamp_sidebar_width(10_000.0), SIDEBAR_MAX_WIDTH);
        assert_eq!(clamp_sidebar_width(SIDEBAR_MIN_WIDTH - 0.5), SIDEBAR_MIN_WIDTH);
        assert_eq!(clamp_sidebar_width(SIDEBAR_MAX_WIDTH + 0.5), SIDEBAR_MAX_WIDTH);
    }

    #[test]
    fn clamp_right_keeps_drag_widths_in_range() {
        assert_eq!(clamp_right_width(400.0), 400.0);
        assert_eq!(clamp_right_width(0.0), RIGHT_MIN_WIDTH);
        assert_eq!(clamp_right_width(10_000.0), RIGHT_MAX_WIDTH);
        assert_eq!(clamp_right_width(RIGHT_MIN_WIDTH - 0.5), RIGHT_MIN_WIDTH);
        assert_eq!(clamp_right_width(RIGHT_MAX_WIDTH + 0.5), RIGHT_MAX_WIDTH);
    }

    #[test]
    fn right_and_sidebar_clamps_are_independent() {
        assert_eq!(clamp_sidebar_width(200.0), 200.0);
        assert_eq!(clamp_right_width(200.0), RIGHT_MIN_WIDTH);
        assert_eq!(clamp_right_width(500.0), 500.0);
        assert_eq!(clamp_sidebar_width(500.0), SIDEBAR_MAX_WIDTH);
    }

    const _: () = {
        assert!(SIDEBAR_MIN_WIDTH < SIDEBAR_WIDTH);
        assert!(SIDEBAR_WIDTH < SIDEBAR_MAX_WIDTH);
        assert!(RIGHT_MIN_WIDTH < RIGHT_WIDTH);
        assert!(RIGHT_WIDTH < RIGHT_MAX_WIDTH);
    };
}
