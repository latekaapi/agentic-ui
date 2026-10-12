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

/// Sidebar column width: Fluid Functionalism's 16rem (owner 2026-10-11).
pub const SIDEBAR_WIDTH: f32 = 256.0;
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
    hide_rail: bool,
    centre: Option<AnyElement>,
    right: Option<AnyElement>,
    full_height_sidebar: bool,
    rail_width: Option<Pixels>,
    centre_clear: Option<Pixels>,
    edge_control: Option<(Pixels, Pixels, AnyElement)>,
    centre_header: bool,
    inset: bool,
    sheets: bool,
    page_sheet: bool,
    native_lights: bool,
    chrome_lead: Option<AnyElement>,
    chrome: Option<AnyElement>,
    chrome_closed: Option<AnyElement>,
    chrome_right: Option<AnyElement>,
    peek: Option<AnyElement>,
    animate: bool,
    on_overlay_bounds: Option<OverlayBoundsHandler>,
    peek_open: bool,
}

/// The floating chrome's rects with their corner radii.
type OverlayRects = Vec<(gpui::Bounds<Pixels>, Pixels)>;

/// Called each frame with the rects of the v3 shell's chrome over the sheet
/// (window coordinates: the corner tabs while they have a ground, the peek
/// card, each with its corner radius) — a host with a native surface under
/// them cuts matching holes.
pub type OverlayBoundsHandler = std::rc::Rc<dyn Fn(Vec<(gpui::Bounds<Pixels>, Pixels)>, &mut Window, &mut App)>;

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
        hide_rail: false,
        centre: None,
        right: None,
        full_height_sidebar: false,
        rail_width: None,
        centre_clear: None,
        edge_control: None,
        centre_header: true,
        inset: false,
        sheets: false,
        page_sheet: false,
        native_lights: false,
        chrome_lead: None,
        chrome: None,
        chrome_closed: None,
        chrome_right: None,
        peek: None,
        animate: true,
        on_overlay_bounds: None,
        peek_open: false,
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

    /// A resize drag is in flight: the dragged column feeds its width straight
    /// through and skips the layout spring so the divider tracks the pointer.
    /// When the drag ends the spring re-arms from the current width, so the
    /// pane settles with no jump. The app sets this from the resize handle's
    /// drag intents (see [`crate::shell::resize_handle`]).
    ///
    /// **This one flag governs both columns.** Only one divider can be under
    /// the pointer at a time, so a single flag is enough for the drag itself —
    /// but it also suppresses the *other* column's spring while it is set. A
    /// host that toggles [`Self::right_open`] during a sidebar drag will see
    /// the right column jump to its new width instead of animating there.
    /// Clear the flag on mouse-up and that window closes.
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

    /// The sidebar column spans the title bar and the body: for a sidebar
    /// that draws its own title bar (the v2 `screens::sidebar`, whose bar
    /// leaves the traffic-light gap and carries the toggle). The centre and
    /// right columns each own their header cell; a centre with no
    /// `header_centre` draws none, so a page can fill the column top to
    /// bottom. Collapsed, the left column is the `rail` at
    /// [`AppShell::rail_width`] (the v2 sidebar's collapsed strip) and the
    /// host draws the toggle in the centre header beside the lights.
    pub fn full_height_sidebar(mut self, on: bool) -> Self {
        self.full_height_sidebar = on;
        self
    }

    /// Whether the centre column draws its header cell (full-height mode
    /// only): off, the centre pane fills the column top to bottom and the
    /// `header_centre` element is dropped.
    pub fn centre_header(mut self, on: bool) -> Self {
        self.centre_header = on;
        self
    }

    /// Full-height layout only: FF's sidebar `variant="inset"`
    /// (sidebar-core.tsx:838-842). The sidebar column draws no right border,
    /// because the centre is an inset card on the same ground that the host
    /// draws, and a hairline beside the card's own edge doubles it.
    pub fn inset(mut self, on: bool) -> Self {
        self.inset = on;
        self
    }

    /// Full-height layout only: the window x the centre header's content
    /// must clear (the traffic lights plus the edge control). The header
    /// pads by `max(0, x − sidebar width)` on the *animated* width every
    /// frame, so its title glides with the sidebar's spring and never sits
    /// under the lights — nothing swaps at the start or end of a collapse.
    pub fn centre_clear(mut self, x: impl Into<Pixels>) -> Self {
        self.centre_clear = Some(x.into());
        self
    }

    /// Full-height layout only: a control pinned in the title strip at window
    /// x `left`, `width` wide (the sidebar toggle at 84). It draws while the
    /// sidebar is closed, and while an opening sidebar's column (on its
    /// animated width) does not yet cover it, so the toggle stands still
    /// while the sidebar slides in and out under it.
    pub fn edge_control(mut self, left: impl Into<Pixels>, width: impl Into<Pixels>, el: impl IntoElement) -> Self {
        self.edge_control = Some((left.into(), width.into(), el.into_any_element()));
        self
    }

    /// The collapsed column's width, in place of the rail constants: the
    /// v2 sidebar collapses to an `SP_4` strip.
    pub fn rail_width(mut self, width: impl Into<Pixels>) -> Self {
        self.rail_width = Some(width.into());
        self
    }

    /// Draws the window frame the design card shows: line-strong border,
    /// radius 12, elevation 3. Apps fill the window instead.
    pub fn framed(mut self, framed: bool) -> Self {
        self.framed = framed;
        self
    }

    /// The v3 layout (canvas "Baaz v3", docs/31 §9, chrome revised
    /// 2026-10-12): the sidebar sits on the frame with no divider; the work
    /// area is a **sheet** 6 px in from the window and the sidebar column,
    /// radius 8, on Sheet with the one edge; the right pane is a second
    /// sheet beside it. The sheet never moves — closing only removes the
    /// sidebar column (FF `slow` in, `slow.exit` out, the panel sliding with
    /// the column's edge). The chrome row — lights, the host's toggle
    /// ([`AppShell::chrome_lead`]) and switch ([`AppShell::chrome`]) — is
    /// the shell's and stays put in both states; closed, its ends become
    /// two corner tabs on the frame colour over the sheet, the left with the
    /// title ([`AppShell::chrome_closed`]), the right with the actions
    /// ([`AppShell::chrome_right`]). Open, the `header_centre` content is
    /// the sheet's own 32 px title row, centred on the lights' line, over a
    /// 64 px fade. The rail, `edge_control` and `centre_clear` are not used.
    pub fn sheets(mut self, on: bool) -> Self {
        self.sheets = on;
        self
    }

    /// v3 only: the sheet is a web page (Browse) — white, its edge a 50 %
    /// black ring (a white highlight cannot show on white), no title row and
    /// no fade; the host's native page covers it.
    pub fn page_sheet(mut self, on: bool) -> Self {
        self.page_sheet = on;
        self
    }

    /// v3 only: the window's own traffic lights. The shell keeps them at the
    /// title bar's spot in every state (`place_native_lights`) and starts
    /// the chrome row past them. With [`AppShell::traffic_lights`] instead,
    /// the shell paints the dots there (gallery).
    pub fn native_lights(mut self, on: bool) -> Self {
        self.native_lights = on;
        self
    }

    /// v3 only: the chrome row's lead — the host's sidebar toggle, drawn at
    /// x [`CHROME_LEAD`] in both states. Resting the pointer on it while the
    /// sidebar is closed arms the peek, like the window's left strip.
    pub fn chrome_lead(mut self, el: impl IntoElement) -> Self {
        self.chrome_lead = Some(el.into_any_element());
        self
    }

    /// v3 only: what follows the lead in both states — the mode switch.
    pub fn chrome(mut self, el: impl IntoElement) -> Self {
        self.chrome = Some(el.into_any_element());
        self
    }

    /// v3 only: the left tab's closed-only content — the title (600), or
    /// Browse's page controls and address at rest. It fades in with a 4 px
    /// rise once the column has closed (FF popup timing, 80 / 60 ms) and
    /// leaves before the column opens.
    pub fn chrome_closed(mut self, el: impl IntoElement) -> Self {
        self.chrome_closed = Some(el.into_any_element());
        self
    }

    /// v3 only: the right tab's content — the header's actions, drawn while
    /// the column is not fully open at the spot the sheet row draws them.
    pub fn chrome_right(mut self, el: impl IntoElement) -> Self {
        self.chrome_right = Some(el.into_any_element());
        self
    }

    /// v3 only: whether the column and the closed-only chrome animate (default true).
    /// Off, open and close are instant: a host whose native page cannot
    /// follow a per-frame resize smoothly turns them off rather than jank
    /// (the peek still slides — only its hole has to follow).
    pub fn animate(mut self, on: bool) -> Self {
        self.animate = on;
        self
    }

    /// v3 only: reports the floating chrome's rects every frame they paint
    /// (see [`OverlayBoundsHandler`]); an empty list when none shows.
    pub fn on_overlay_bounds(mut self, f: impl Fn(Vec<(gpui::Bounds<Pixels>, Pixels)>, &mut Window, &mut App) + 'static) -> Self {
        self.on_overlay_bounds = Some(std::rc::Rc::new(f));
        self
    }

    /// v3 only: holds the peek open while the sidebar is closed, with no
    /// hover — a scripting and capture aid (the person opens it by hover).
    pub fn peek_open(mut self, open: bool) -> Self {
        self.peek_open = open;
        self
    }

    /// v3 only: the peek — the sidebar (without its title bar) as a Floating
    /// card (radius 8, 6 px from the left and bottom, under the chrome row)
    /// that slides in while the pointer rests on the toggle or the window's
    /// left 12 px for 150 ms and leaves 250 ms after it goes (FF
    /// `peek="hover"`, `moderate` / `moderate.exit`).
    pub fn peek(mut self, el: impl IntoElement) -> Self {
        self.peek = Some(el.into_any_element());
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

    /// The collapsed sidebar pane: the rail shown while `sidebar_open` is
    /// false. The rail is mounted beneath the sidebar on every frame, so it
    /// is already underneath while the sidebar clips away during a collapse;
    /// while open the opaque sidebar covers it. Without a rail the column is
    /// simply empty, so full-width content can never be clipped into it.
    pub fn rail(mut self, el: impl IntoElement) -> Self {
        self.rail = Some(el.into_any_element());
        self
    }

    /// With the sidebar closed, reserve no rail column at all: the centre
    /// runs from the window's left edge (a full-width page under a host's
    /// own one-row chrome). The rail element, if any, is not drawn.
    pub fn hide_rail(mut self, hide: bool) -> Self {
        self.hide_rail = hide;
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
        if self.sheets {
            return sheets(self, window, cx);
        }

        let rail_width = if self.hide_rail && !self.sidebar_open {
            px(0.0)
        } else if let Some(width) = self.rail_width {
            width
        } else if self.traffic_lights {
            px(RAIL_WIDTH_WITH_LIGHTS)
        } else {
            px(RAIL_WIDTH)
        };
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
        // Like the sidebar, the closed pane's content unmounts once the spring
        // has settled at zero: clipped-but-mounted children stayed in the
        // accessibility tree as off-screen tabs and buttons.
        let right_mounted = self.right_open || right_w > px(0.5);
        // The pane keeps its resting width while the column springs, so the
        // content slides under the divider instead of reflowing every frame.
        // The rail is mounted beneath the sidebar on every frame: while
        // collapsing, the expanded sidebar stays over it at its resting width
        // and is clipped by the shrinking column (the way a macOS sidebar
        // closes), so the rail is already present underneath when the
        // sidebar's last pixels go and no subtree is swapped in by a
        // threshold mid-motion. The sidebar unmounts only once the spring has
        // settled, so the settled rail column is unchanged; while open the
        // opaque sidebar covers the rail, so that state is unchanged too.
        // Expanding shows the sidebar at its resting width from the first
        // frame.
        let collapsing = !self.sidebar_open && sidebar_w > rail_width + px(1.0);
        let show_sidebar = self.sidebar_open || collapsing;
        let settled_pane_w = if show_sidebar { sidebar_rest } else { rail_width };
        // A steady header row keeps the sidebar cell (and its divider) at the
        // resting width however the pane below moves.
        let (header_w, header_pane_w) =
            if self.header_follows_sidebar { (sidebar_w, settled_pane_w) } else { (sidebar_rest, sidebar_rest) };

        if self.full_height_sidebar {
            return full_height(self, FullHeightGeometry { sidebar_w, sidebar_rest, rail_width, show_sidebar, right_w, right_inner, right_mounted }, cx);
        }

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
                    .child(div().h_full().w(right_inner).flex_none().flex().items_center().border_l_1().border_color(p.line).when(right_mounted, |d| d.children(self.header_right))),
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
                    .relative()
                    .overflow_hidden()
                    .border_r_1()
                    .border_color(p.line)
                    .bg(p.surface_1)
                    .child(div().absolute().left(px(0.0)).top(px(0.0)).bottom(px(0.0)).w(rail_width).children(self.rail))
                    .when(show_sidebar, |d| {
                        // Opaque, or the rail beneath shows through it: the
                        // rail's icons sit at the same left edge as the
                        // sidebar's rows, so a transparent overlay draws a
                        // second `+` through "New session" and a magnifier
                        // through "Add project".
                        d.child(
                            div()
                                .absolute()
                                .left(px(0.0))
                                .top(px(0.0))
                                .bottom(px(0.0))
                                .w(sidebar_rest)
                                .overflow_hidden()
                                .bg(p.surface_1)
                                .children(self.sidebar),
                        )
                    }),
            )
            .child(div().flex_1().h_full().min_w(px(0.0)).overflow_hidden().bg(p.bg).children(self.centre))
            .child(
                div()
                    .w(right_w)
                    .h_full()
                    .flex_none()
                    .overflow_hidden()
                    .child(div().w(right_inner).h_full().flex_none().border_l_1().border_color(p.line).bg(p.surface_1).when(right_mounted, |d| d.children(self.right))),
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

/// The resolved widths [`AppShell::render`] measured, handed to the
/// full-height layout.
struct FullHeightGeometry {
    sidebar_w: Pixels,
    sidebar_rest: Pixels,
    rail_width: Pixels,
    show_sidebar: bool,
    right_w: Pixels,
    right_inner: Pixels,
    right_mounted: bool,
}

/// The full-height layout: `sidebar | centre | right`, where the sidebar
/// column runs from the window's top edge to its bottom and the other
/// two columns stack their own header cell over their pane. The sidebar
/// column's right border is the first divider; the right column's left
/// border the second, from top to bottom like the design's.
fn full_height(shell: AppShell, g: FullHeightGeometry, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let header_h = cx.aui().metrics.header;
    let id = shell.id.clone();
    let header = |content: Option<AnyElement>| {
        div()
            .w_full()
            .h(header_h)
            .flex_none()
            .flex()
            .items_center()
            .min_w(px(0.0))
            .overflow_x_hidden()
            .border_b_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .children(content)
    };
    let sidebar_column = div()
        .w(g.sidebar_w)
        .h_full()
        .flex_none()
        .relative()
        .overflow_hidden()
        .when(!shell.inset, |d| d.border_r_1().border_color(p.line))
        .bg(p.bg)
        .child(div().absolute().left(px(0.0)).top(px(0.0)).bottom(px(0.0)).w(g.rail_width).children(shell.rail))
        .when(g.show_sidebar, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .bottom(px(0.0))
                    .w(g.sidebar_rest)
                    .overflow_hidden()
                    .bg(p.bg)
                    .children(shell.sidebar),
            )
        });
    // The centre header is the window's drag strip (the sidebar's title
    // bar is the sidebar's own); a centre with no header is a page that
    // fills the column, and the window drags from nowhere in it.
    let clear = shell.centre_clear.map_or(px(0.0), |x| (x - g.sidebar_w).max(px(0.0)));
    let centre_header: Option<AnyElement> = shell.header_centre.filter(|_| shell.centre_header).map(|content| {
        let content = div().size_full().flex().items_center().pl(clear).child(content).into_any_element();
        if shell.draggable {
            drag_region((id.clone(), "header-drag")).child(header(Some(content))).into_any_element()
        } else {
            header(Some(content)).into_any_element()
        }
    });
    // Closed, it stands from the first frame (the sidebar's own toggle sits
    // at the same x underneath, so nothing jumps); opening, it stays until
    // the growing column covers it.
    let open = shell.sidebar_open;
    let edge_control = shell.edge_control.filter(|(left, width, _)| !open || g.sidebar_w < *left + *width).map(|(left, _, el)| {
        div().absolute().top(px(0.0)).left(left).h(header_h).flex().items_center().child(el).into_any_element()
    });
    let centre_column = v_flex()
        .flex_1()
        .h_full()
        .min_w(px(0.0))
        .overflow_hidden()
        .bg(p.bg)
        .children(centre_header)
        .child(div().w_full().flex_1().min_h(px(0.0)).overflow_hidden().children(shell.centre));
    let right_column = div().w(g.right_w).h_full().flex_none().overflow_hidden().child(
        v_flex()
            .w(g.right_inner)
            .h_full()
            .flex_none()
            .border_l_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .when(g.right_mounted, |d| d.child(header(shell.header_right)))
            .child(div().w_full().flex_1().min_h(px(0.0)).overflow_hidden().when(g.right_mounted, |d| d.children(shell.right))),
    );
    let mut root = crate::keys::track_pointer(
        h_flex()
            .id(id)
            .relative()
            .size_full()
            .items_stretch()
            .overflow_hidden()
            .bg(p.bg)
            .text_color(p.ink)
            .child(sidebar_column)
            .child(centre_column)
            .child(right_column)
            .children(edge_control),
    );
    if shell.framed {
        root = root.rounded(px(scale::R_LG)).border_1().border_color(p.line_strong).shadow(p.shadow(3));
    }
    root
}

/// v3 geometry (canvas "Baaz v3" v9): the sheet's inset from the window and
/// from the sidebar column, its radius, the title row, the fades, the pills.
pub const SHEET_INSET: f32 = 6.0;
/// The sheet's corner radius.
pub const SHEET_RADIUS: f32 = 8.0;
/// The sheet's own title row, from the sheet's top: 32 px puts its centre on
/// the lights' line (window y 22 = 6 + 16).
pub const SHEET_ROW_H: f32 = 32.0;
/// The title's inset from the sheet's left edge (mirrors the action glyphs,
/// 10 + 6 from the right).
pub const SHEET_ROW_PAD_LEFT: f32 = 16.0;
/// The actions' inset from the sheet's right edge.
pub const SHEET_ROW_PAD_RIGHT: f32 = 10.0;
/// The fade the content scrolls under: open (under the title row) and closed
/// (under the chrome row's tabs).
pub const SHEET_FADE_OPEN: f32 = 64.0;
/// See [`SHEET_FADE_OPEN`].
pub const SHEET_FADE_CLOSED: f32 = 64.0;
/// The chrome row (`metrics.header` tall, 44 at the standard density): the
/// host's toggle starts here, past the window's lights — the v2 sidebar's
/// title bar put it at the same x, so nothing moved when the row became the
/// shell's. The sidebar's `traffic_light_gap` equals it (asserted below).
pub const CHROME_LEAD: f32 = 84.0;
/// Toggle to mode switch, and between the switch and what follows.
pub const CHROME_GAP: f32 = 6.0;
/// A corner tab's end cap: its content ends 14 px before the rounded corner.
pub const CHROME_PAD_END: f32 = 14.0;
/// The right tab's actions end where the sheet row drew them: the sheet's
/// inset plus the row's right padding from the window's edge.
pub const CHROME_ACTIONS_PAD: f32 = SHEET_INSET + SHEET_ROW_PAD_RIGHT;
/// Depth (docs/37): the sheet's contact shadow onto the frame beside the
/// open sidebar — width, and its strength at the sheet's edge per theme
/// (Claude Desktop measures 25 % over 20 pt; the v3 ladder is cooler and
/// already carries the edge highlight, so a touch quieter).
pub const CONTACT_W: f32 = 20.0;
const CONTACT_ALPHA_DARK: f32 = 0.16;
const CONTACT_ALPHA_LIGHT: f32 = 0.06;
/// The fade a list scrolls under at the sidebar's top (frame → clear) and
/// at the peek card's top (floating → clear).
pub const LIST_FADE: f32 = 24.0;
/// The peek: its hover strip, arm and dismiss delays. Its card sits
/// `SHEET_INSET` under the chrome row.
pub const PEEK_STRIP: f32 = 12.0;
const PEEK_ARM: std::time::Duration = std::time::Duration::from_millis(150);
const PEEK_DISMISS: std::time::Duration = std::time::Duration::from_millis(250);

/// The peek card's shadow (docs/37 G2): the edge plus one drop cast to the
/// right, so the card's edge over the sheet reads like Claude Desktop's
/// hover panel; light keeps FF's float recipe.
fn peek_shadow(p: &aui_tokens::Palette) -> Vec<gpui::BoxShadow> {
    if p.is_dark() {
        let mut layers = p.edge();
        layers.push(gpui::BoxShadow {
            color: gpui::black().alpha(0.45),
            offset: gpui::point(px(8.0), px(0.0)),
            blur_radius: px(24.0),
            spread_radius: px(-8.0),
            inset: false,
        });
        layers
    } else {
        p.floating_shadow()
    }
}

/// The shell paints the window controls at the title bar's spot (the v2
/// sidebar's: 12, 16) for a gallery card.
fn painted_lights() -> Div {
    let dot = |c: u32| div().size(px(12.0)).rounded_full().bg(gpui::rgb(c));
    h_flex().flex_none().gap(px(8.0)).child(dot(0xFF5F57)).child(dot(0xFEBC2E)).child(dot(0x28C840))
}

/// Puts the window's own traffic lights at the title bar's spot — once per
/// window (the position is remembered), so every layout agrees on it. The
/// v3 shell never moves them: the chrome row they sit in stays put in both
/// sidebar states (owner, 2026-10-12: the lights and the toggle must not
/// jump between states).
pub fn place_native_lights(window: &mut Window, cx: &mut App) {
    let placed = window.use_keyed_state("aui-native-lights", cx, |_, _| false);
    if !*placed.read(cx) {
        window.set_traffic_light_position(lights_position(cx));
        placed.update(cx, |p, _| *p = true);
    }
}

/// Where the macOS lights go: the title bar's spot, centred on the chrome row.
fn lights_position(cx: &App) -> gpui::Point<Pixels> {
    super::header::traffic_light_position(cx)
}

#[derive(Clone, Copy, Default)]
struct SheetsState {
    /// The column's last sampled openness, so the closed-only chrome can
    /// wait for it.
    column: f32,
    /// What the pointer rests on: the window's left strip, the toggle (the
    /// chrome lead), the peek card.
    strip: bool,
    lead: bool,
    card: bool,
    /// When the pointer arrived on any of the three, and when it left them all.
    since: Option<std::time::Instant>,
    left_at: Option<std::time::Instant>,
    peeking: bool,
}

#[derive(Clone, Copy)]
enum HoverSource {
    Strip,
    Lead,
    Card,
}

impl SheetsState {
    /// One hover report: the arm clock starts when the pointer first lands
    /// on any peek source and the dismiss clock when it has left them all,
    /// so moving between the toggle, the strip and the card never resets
    /// either (two sources each clearing one shared time did).
    fn set_hover(&mut self, source: HoverSource, hovered: bool, now: std::time::Instant) {
        let before = self.strip || self.lead || self.card;
        match source {
            HoverSource::Strip => self.strip = hovered,
            HoverSource::Lead => self.lead = hovered,
            HoverSource::Card => self.card = hovered,
        }
        let after = self.strip || self.lead || self.card;
        if !before && after {
            self.since = Some(now);
            self.left_at = None;
        } else if before && !after {
            self.since = None;
            self.left_at = Some(now);
        }
    }
}

/// The v3 layout: see [`AppShell::sheets`].
fn sheets(shell: AppShell, window: &mut Window, cx: &mut App) -> gpui::Stateful<gpui::Div> {
    use aui_motion::{open_close, open_close_with, Enter, Easing, EXIT_EASING};
    let p = cx.aui().colors;
    let id = shell.id.clone();
    let state = window.use_keyed_state((id.clone(), "v3"), cx, |_, _| SheetsState { column: if shell.sidebar_open { 1.0 } else { 0.0 }, ..Default::default() });
    let st = *state.read(cx);
    let now = cx.background_executor().now();

    let lo = f32::from(shell.sidebar_min);
    let hi = f32::from(shell.sidebar_max).max(lo);
    let rest = px(f32::from(shell.sidebar_width).clamp(lo, hi));

    // The chrome row — the lights, the host's toggle and its mode switch —
    // never moves: it is drawn by the shell at the window's top-left in
    // both states (the open sidebar has no title bar of its own; the shell
    // pads the column by the row's height). Closed, the row's two ends become
    // corner tabs on the frame colour over the sheet: the left one gains the
    // title (or Browse's page controls), the right one takes the sheet row's
    // actions at the very spot the row drew them. The tabs' ground shows
    // whenever the column is not fully open, so the sheet slides under it
    // during a close and back out during an open.
    let closed = !shell.sidebar_open;
    let header_h = cx.aui().metrics.header;
    // Opening out of a peek: the sidebar is already in view as the card, so
    // the column starts at rest instead of sliding in from the left.
    if shell.sidebar_open && st.peeking {
        aui_motion::open_close_jump((id.clone(), "column"), true, window, cx);
    }
    // Motion off (an instant layout) or a drag: keep the keyed runs at rest
    // where the layout is, so turning motion back on never replays a stale run.
    if !shell.animate || shell.resizing {
        aui_motion::open_close_jump((id.clone(), "column"), shell.sidebar_open, window, cx);
        aui_motion::open_close_jump((id.clone(), "closed-only"), closed, window, cx);
    }
    let column = if shell.resizing || !shell.animate {
        if shell.sidebar_open { 1.0 } else { 0.0 }
    } else {
        open_close((id.clone(), "column"), shell.sidebar_open, aui_motion::SpringKind::Slow, aui_motion::durations::SLOW_EXIT, window, cx)
    };
    let sidebar_w = rest * column.max(0.0);
    let settled_closed = closed && column <= 0.0005;
    // The closed-only content (the title, Browse's page controls) fades in
    // with a 4 px rise once the column has settled closed (FF popup timing,
    // 80 ms ease-out) and leaves before the column opens (60 ms).
    let closed_only_wanted = closed && st.column <= 0.0005;
    let closed_k = if !shell.animate {
        if closed { 1.0 } else { 0.0 }
    } else {
        open_close_with(
            (id.clone(), "closed-only"),
            closed_only_wanted,
            Enter::Tween(std::time::Duration::from_millis(80), Easing::OUT),
            aui_motion::durations::FAST_EXIT,
            EXIT_EASING,
            window,
            cx,
        )
    };
    // The column settled closed on this frame: the fade reads it on the
    // next, so ask for one (nothing else is moving to request it).
    if shell.animate && settled_closed && !closed_only_wanted {
        window.request_animation_frame();
    }
    let in_motion = column > 0.0005 && column < 0.9995;
    let tabs_grounded = closed || in_motion;

    // The window's own lights stay at the title bar's spot in every state.
    if shell.native_lights {
        place_native_lights(window, cx);
    }

    // The peek: armed by a 150 ms rest on the toggle or the window's left
    // strip, dismissed 250 ms after the pointer has left those and the card.
    let any_hovered = st.strip || st.lead || st.card;
    let mut peeking = (st.peeking || shell.peek_open) && settled_closed;
    let mut pending = false;
    if settled_closed && shell.peek.is_some() {
        if !peeking {
            if let (Some(since), true) = (st.since, st.strip || st.lead) {
                if now.duration_since(since) >= PEEK_ARM { peeking = true } else { pending = true }
            }
        } else if !any_hovered {
            match st.left_at {
                Some(at) if now.duration_since(at) >= PEEK_DISMISS => peeking = false,
                Some(_) => pending = true,
                None => {}
            }
        }
    } else {
        peeking = false;
    }
    // Hover reports stop when the strip and card unmount: clear what they
    // left behind, so a later close never opens a peek nobody asked for.
    if !settled_closed && (st.strip || st.card || st.since.is_some() || st.left_at.is_some()) {
        state.update(cx, |s, _| {
            s.strip = false;
            s.card = false;
            s.since = if s.lead { Some(now) } else { None };
            s.left_at = None;
        });
    }
    if pending {
        window.request_animation_frame();
    }
    if column != st.column || peeking != st.peeking {
        state.update(cx, |s, _| *s = SheetsState { column, peeking, ..*s });
    }
    // Right sheet on the moderate tier (FF: panel width).
    let right_lo = f32::from(shell.right_min);
    let right_rest = px(f32::from(shell.right_width).clamp(right_lo, f32::from(shell.right_max).max(right_lo)));
    let right_target = if shell.right_open { right_rest + px(SHEET_INSET) } else { px(0.0) };
    let right_w = if shell.resizing || !shell.animate {
        right_target
    } else {
        aui_motion::spring_px((id.clone(), "right-width"), right_target, aui_motion::SpringKind::Moderate, window, cx).max(px(0.0))
    };
    let right_mounted = shell.right_open || right_w > px(0.5);

    let edge_of = |page: bool| if page {
        vec![gpui::BoxShadow { color: gpui::black().alpha(0.5), offset: gpui::point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(1.0), inset: false }]
    } else {
        p.elevation_shadow(2)
    };
    let sheet_box = |page: bool| {
        div()
            .relative()
            .size_full()
            .rounded(px(SHEET_RADIUS))
            .overflow_hidden()
            .bg(if page { gpui::Hsla::from(gpui::rgb(0xFAFAFA)) } else { p.surface_2 })
            .shadow(edge_of(page))
    };

    // The sidebar column: the panel keeps its resting width and slides with
    // the column's edge (FF: width 0 ↔ w and x −100 % ↔ 0 on one transition).
    let show_sidebar = shell.sidebar_open || column > 0.0005;
    // Depth (docs/37, 2026-10-12): rows scrolling under the chrome row fade
    // into the frame over `LIST_FADE`, like the transcript under the title
    // row — the one gradient Claude Desktop draws under its bar.
    let list_fade = |ground: gpui::Hsla| {
        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .h(px(LIST_FADE))
            .bg(gpui::linear_gradient(180.0, gpui::linear_color_stop(ground, 0.0), gpui::linear_color_stop(ground.alpha(0.0), 1.0)))
    };
    let sidebar_column = div()
        .w(sidebar_w)
        .h_full()
        .flex_none()
        .relative()
        .overflow_hidden()
        .when(show_sidebar, |d| {
            d.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .bottom(px(0.0))
                    .left(sidebar_w - rest)
                    .w(rest)
                    .pt(header_h)
                    .relative()
                    .children(shell.sidebar)
                    .child(list_fade(p.surface_1).top(header_h)),
            )
        });
    // The sheet's contact shadow onto the frame (docs/37 G1): the sheet is
    // the lighter surface, so where it meets the sidebar it casts one soft
    // shadow leftwards — `CONTACT_W` wide, darkest at the sheet's edge, gone
    // at the strip's left — beside the sheet only (never into the 6 px gaps,
    // where it would read as a border). It rides the column's edge and fades
    // with the column, so a closed window has none.
    let contact = if show_sidebar {
        let a = if p.is_dark() { CONTACT_ALPHA_DARK } else { CONTACT_ALPHA_LIGHT };
        Some(
            div()
                .absolute()
                .top(px(SHEET_INSET))
                .bottom(px(SHEET_INSET))
                .left(sidebar_w + px(SHEET_INSET) - px(CONTACT_W))
                .w(px(CONTACT_W))
                .opacity(column.clamp(0.0, 1.0))
                .bg(gpui::linear_gradient(90.0, gpui::linear_color_stop(gpui::black().alpha(0.0), 0.0), gpui::linear_color_stop(gpui::black().alpha(a), 1.0))),
        )
    } else {
        None
    };

    // The centre sheet: the title row and its fade only while fully open
    // (the corner tabs carry the title and the actions otherwise); a page
    // sheet has neither.
    let row_shown = !shell.page_sheet && !tabs_grounded;
    let fade_h = if row_shown { SHEET_FADE_OPEN } else { SHEET_FADE_CLOSED };
    let ground = p.surface_2;
    let mut centre_sheet = sheet_box(shell.page_sheet).child(div().absolute().inset_0().children(shell.centre));
    if !shell.page_sheet {
        centre_sheet = centre_sheet.child(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .h(px(fade_h))
                .bg(gpui::linear_gradient(180.0, gpui::linear_color_stop(ground, 0.2), gpui::linear_color_stop(ground.alpha(0.0), 1.0))),
        );
    }
    if row_shown && shell.centre_header {
        if let Some(content) = shell.header_centre {
            let row = h_flex()
                .w_full()
                .h(px(SHEET_ROW_H))
                .pl(px(SHEET_ROW_PAD_LEFT))
                .pr(px(SHEET_ROW_PAD_RIGHT))
                .items_center()
                .child(div().size_full().flex().items_center().child(content));
            let row = if shell.draggable { drag_region((id.clone(), "row-drag")).child(row).into_any_element() } else { row.into_any_element() };
            centre_sheet = centre_sheet.child(div().absolute().top(px(0.0)).left(px(0.0)).right(px(0.0)).h(px(SHEET_ROW_H)).child(row));
        }
    }
    let centre_column = div()
        .flex_1()
        .h_full()
        .min_w(px(0.0))
        .pt(px(SHEET_INSET))
        .pb(px(SHEET_INSET))
        .pl(px(SHEET_INSET))
        .pr(if right_mounted { px(0.0) } else { px(SHEET_INSET) })
        .child(crate::interact::AmbientScope::filled(2, centre_sheet));

    // The right sheet, 6 px beside the centre; its header row inside it.
    let right_column = div().w(right_w).h_full().flex_none().overflow_hidden().when(right_mounted, |d| {
        d.child(
            div().w(right_rest + px(SHEET_INSET)).h_full().pl(px(SHEET_INSET)).pt(px(SHEET_INSET)).pb(px(SHEET_INSET)).pr(px(SHEET_INSET)).child(
                sheet_box(false).child(
                    v_flex()
                        .size_full()
                        .child(div().w_full().h(cx.aui().metrics.header).flex_none().flex().items_center().children(shell.header_right))
                        .child(div().w_full().flex_1().min_h(px(0.0)).overflow_hidden().children(shell.right)),
                ),
            ),
        )
    });

    let mut root = h_flex()
        .id(id.clone())
        .relative()
        .size_full()
        .items_stretch()
        .overflow_hidden()
        .bg(p.surface_1)
        .text_color(p.ink)
        .child(sidebar_column)
        .children(contact)
        .child(centre_column)
        .child(right_column);

    // The floating chrome's rects, collected while it prepaints and handed
    // to the host by the last child (see `AppShell::on_overlay_bounds`).
    use gpui_kit::base::ElementExt as _;
    let rects: std::rc::Rc<std::cell::RefCell<OverlayRects>> = Default::default();
    let report = |radius: Pixels| {
        let rects = rects.clone();
        move |bounds: gpui::Bounds<Pixels>, _: &mut Window, _: &mut App| rects.borrow_mut().push((bounds, radius))
    };

    // A corner tab's rect, grown by its radius on the sides that meet the
    // window's edges, so the hole shows only the one rounded corner.
    let report_corner = |rects: std::rc::Rc<std::cell::RefCell<OverlayRects>>, left: bool| {
        move |bounds: gpui::Bounds<Pixels>, _: &mut Window, _: &mut App| {
            let r = px(SHEET_RADIUS);
            let grown = gpui::Bounds {
                origin: gpui::point(if left { bounds.origin.x - r } else { bounds.origin.x }, bounds.origin.y - r),
                size: gpui::size(bounds.size.width + r, bounds.size.height + r),
            };
            rects.borrow_mut().push((grown, r))
        }
    };

    // The chrome row's two ends, over everything but the peek. The ends'
    // rects reach the host as holes with their rounding pushed outside the
    // window, so a native page shows the one rounded corner each tab has.
    let edge_line = if p.is_dark() { gpui::white().alpha(0.06) } else { gpui::black().alpha(0.06) };
    let hover_state = state.clone();
    let lead = div()
        .id((id.clone(), "chrome-lead"))
        .flex_none()
        .on_hover(move |hovered, _, cx| {
            let hovered = *hovered;
            hover_state.update(cx, |s, cx| {
                s.set_hover(HoverSource::Lead, hovered, cx.background_executor().now());
                cx.notify();
            });
        })
        .children(shell.chrome_lead);
    let rise = px(4.0 * (1.0 - closed_k));
    let left_tab = h_flex()
        .id((id.clone(), "chrome-left"))
        .h(header_h)
        .min_w(sidebar_w)
        .pl(px(CHROME_LEAD))
        .pr(px(CHROME_PAD_END))
        .gap(px(CHROME_GAP))
        .items_center()
        .when(tabs_grounded, |d| d.bg(p.surface_1).rounded_br(px(SHEET_RADIUS)).border_b_1().border_r_1().border_color(edge_line))
        .child(lead)
        .children(shell.chrome)
        .when(closed_k > 0.0, |d| d.children(shell.chrome_closed.map(|el| div().flex_none().opacity(closed_k).mt(rise).child(el))));
    let left_tab = if tabs_grounded { left_tab.on_prepaint(report_corner(rects.clone(), true)) } else { left_tab };
    let left_tab = if shell.draggable { drag_region((id.clone(), "chrome-drag")).child(left_tab).into_any_element() } else { left_tab.into_any_element() };
    root = root.child(div().absolute().top(px(0.0)).left(px(0.0)).child(left_tab));
    if tabs_grounded {
        if let Some(actions) = shell.chrome_right {
            let right_tab = h_flex()
                .id((id.clone(), "chrome-right"))
                .h(header_h)
                .pl(px(CHROME_PAD_END))
                .pr(px(CHROME_ACTIONS_PAD))
                .items_center()
                .bg(p.surface_1)
                .rounded_bl(px(SHEET_RADIUS))
                .border_b_1()
                .border_l_1()
                .border_color(edge_line)
                .child(actions)
                .on_prepaint(report_corner(rects.clone(), false));
            root = root.child(div().absolute().top(px(0.0)).right(right_w).child(right_tab));
        }
    }

    // The gallery paints the lights where the window's would be, over the tab.
    if shell.traffic_lights {
        let at = lights_position(cx);
        root = root.child(painted_lights().absolute().left(at.x).top(at.y));
    }

    // The peek strip and card, under the chrome row.
    if settled_closed {
        if let Some(peek_el) = shell.peek {
            let peek_top = header_h + px(SHEET_INSET);
            let k = open_close((id.clone(), "peek"), peeking, aui_motion::SpringKind::Moderate, aui_motion::durations::MODERATE_EXIT, window, cx);
            let strip_state = state.clone();
            root = root.child(
                div()
                    .id((id.clone(), "peek-strip"))
                    .absolute()
                    .left(px(0.0))
                    .top(peek_top)
                    .bottom(px(0.0))
                    .w(px(PEEK_STRIP))
                    .on_hover(move |hovered, _, cx| {
                        let hovered = *hovered;
                        strip_state.update(cx, |s, cx| {
                            s.set_hover(HoverSource::Strip, hovered, cx.background_executor().now());
                            cx.notify();
                        });
                    }),
            );
            if k > 0.0 {
                let card_state = state.clone();
                // From −108 % of its width to its resting place, 6 px in.
                let travel = (rest + px(SHEET_INSET)) * 1.08;
                root = root.child(
                    div()
                        .id((id.clone(), "peek-card"))
                        .absolute()
                        .top(peek_top)
                        .bottom(px(SHEET_INSET))
                        .left(px(SHEET_INSET) - travel * (1.0 - k))
                        .w(rest)
                        .rounded(px(SHEET_RADIUS))
                        .overflow_hidden()
                        .bg(p.overlay)
                        .shadow(peek_shadow(&p))
                        .occlude()
                        .on_hover(move |hovered, _, cx| {
                            let hovered = *hovered;
                            card_state.update(cx, |s, cx| {
                                s.set_hover(HoverSource::Card, hovered, cx.background_executor().now());
                                cx.notify();
                            });
                        })
                        .child(crate::interact::AmbientScope::filled(aui_tokens::Palette::FLOATING_LEVEL, peek_el))
                        .child(list_fade(p.overlay))
                        .on_prepaint(report(px(SHEET_RADIUS))),
                );
            }
        }
    }
    if let Some(handler) = shell.on_overlay_bounds {
        let rects = rects.clone();
        root = root.child(div().absolute().size_0().on_prepaint(move |_, window, cx| handler(rects.take(), window, cx)));
    }

    let mut root = crate::keys::track_pointer(root);
    if shell.framed {
        root = root.rounded(px(scale::R_LG)).shadow(p.dialog_shadow());
    }
    root
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

    /// One arm clock across the toggle, the strip and the card: moving from
    /// the toggle onto the card keeps the peek, and the dismiss clock only
    /// starts once the pointer has left all three.
    #[test]
    fn peek_hover_clocks_span_all_three_sources() {
        let t0 = std::time::Instant::now();
        let later = |ms: u64| t0 + std::time::Duration::from_millis(ms);
        let mut s = SheetsState::default();
        s.set_hover(HoverSource::Lead, true, t0);
        assert_eq!(s.since, Some(t0), "landing on the toggle starts the arm clock");
        assert_eq!(s.left_at, None);
        // Onto the card (the card reports first, then the toggle releases).
        s.set_hover(HoverSource::Card, true, later(200));
        s.set_hover(HoverSource::Lead, false, later(201));
        assert_eq!(s.since, Some(t0), "the arm clock is not reset by a move within the sources");
        assert_eq!(s.left_at, None, "no dismiss while the card is hovered");
        // Off everything: the dismiss clock starts once.
        s.set_hover(HoverSource::Card, false, later(900));
        assert_eq!(s.since, None);
        assert_eq!(s.left_at, Some(later(900)));
        // Back onto the strip before the dismiss: the dismiss clock clears.
        s.set_hover(HoverSource::Strip, true, later(1000));
        assert_eq!(s.left_at, None);
        assert_eq!(s.since, Some(later(1000)));
    }

    const _: () = {
        assert!(SIDEBAR_MIN_WIDTH < SIDEBAR_WIDTH);
        assert!(SIDEBAR_WIDTH < SIDEBAR_MAX_WIDTH);
        assert!(RIGHT_MIN_WIDTH < RIGHT_WIDTH);
        assert!(RIGHT_WIDTH < RIGHT_MAX_WIDTH);
    };
}
