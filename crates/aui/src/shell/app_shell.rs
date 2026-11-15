//! The app shell (docs/83 §3, "lines, not layers"): one flat window of
//! columns. The **sidebar column** runs full height on the `sidebar` fill
//! with a 1 px `line-region` on its right edge; its top row is the chrome
//! row (the window's lights, the host's toggle at x 84, the mode switch
//! ending 12 px before the column's right edge). The centre and the right
//! pane are plain columns on the ground, the right one split from the
//! centre by a 1 px `line-region`. Every column's 44 px header ends in a
//! 1 px `line-region`, and the three lines share one y. No inset, no
//! radius, no contact shadow, no scroll fade.
//!
//! Closing removes the column; the centre grows into the space and nothing
//! else is drawn — no row, no toggle, and the host hides the window's lights
//! (owner 2026-10-13, round 7). A 150 ms rest on the window's top edge or
//! its left edge peeks the whole sidebar as a floating surface flush with
//! the window's left, top and bottom: the `sidebar` fill, a 1 px
//! `line-strong` right edge, the float shadow, rounded at the bottom right,
//! its own row riding in with it; after a toggle the peek waits for the
//! pointer to leave and come back.
//!
//! Motion (Emil Kowalski's rules, owner 2026-10-13: smooth or none): the
//! column opens and closes on an ease-out tween, 260 / 220 ms, the panel
//! sliding with the column's edge; while it moves, the centre's content is
//! laid out once at the width it will have at rest and slides with the
//! column's edge, so no frame re-lays out the transcript (the build before
//! re-measured per frame and managed four frames per close).

use aui_motion::{open_close, open_close_with, spring_px, Enter, SpringKind};
use aui_tokens::{scale, ActiveAui, Easing};
use gpui::{div, prelude::*, px, AnyElement, App, Div, ElementId, IntoElement, Pixels, Window};
use gpui_kit::base::h_flex;

use super::drag_region::drag_region;

/// Sidebar column width (docs/83 §2.5: 256, resizable 220–360).
pub const SIDEBAR_WIDTH: f32 = 256.0;
/// Right column width in the app.
pub const RIGHT_WIDTH: f32 = 400.0;
/// Minimum sidebar width while resizing (docs/83 §2.5).
/// Clamp every drag move with [`clamp_sidebar_width`].
pub const SIDEBAR_MIN_WIDTH: f32 = 220.0;
/// Maximum sidebar width while resizing (docs/83 §2.5).
pub const SIDEBAR_MAX_WIDTH: f32 = 360.0;

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

/// The centre header: the title's inset from the column's left, the
/// actions' from its right.
const HEADER_PAD_LEFT: f32 = 16.0;
const HEADER_PAD_RIGHT: f32 = 10.0;
/// A region divider's thickness (docs/83 §1 rule 2: one 1 px hairline).
const HAIRLINE: f32 = 1.0;
/// The chrome row: the host's toggle starts here, past the window's lights
/// (the v2 sidebar's title bar put it at the same x).
pub const CHROME_LEAD: f32 = 84.0;
/// The mode switch ends this far before the sidebar's right edge.
pub const CHROME_SWITCH_INSET: f32 = 12.0;
/// The peek: its hover strip on the window's left, the band along the
/// window's top edge, arm and dismiss delays.
pub const PEEK_STRIP: f32 = 12.0;
const PEEK_TOP_BAND: f32 = 6.0;
const PEEK_ARM: std::time::Duration = std::time::Duration::from_millis(150);
const PEEK_DISMISS: std::time::Duration = std::time::Duration::from_millis(250);
/// The column's open and close: ease-out (Emil: ease-out for what the
/// person asked for; no bounce on a panel that must land exactly).
pub const COLUMN_EASE: Easing = Easing([0.22, 1.0, 0.36, 1.0]);
const COLUMN_OPEN: std::time::Duration = std::time::Duration::from_millis(260);
const COLUMN_CLOSE: std::time::Duration = std::time::Duration::from_millis(220);
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
    traffic_lights: bool,
    native_lights: bool,
    page_sheet: bool,
    animate: bool,
    peek_open: bool,
    header_right: Option<AnyElement>,
    sidebar: Option<AnyElement>,
    centre: Option<AnyElement>,
    right: Option<AnyElement>,
    peek: Option<AnyElement>,
    chrome_lead: Option<AnyElement>,
    chrome_switch: Option<AnyElement>,
    chrome_title: Option<AnyElement>,
    chrome_right: Option<AnyElement>,
    on_overlay_bounds: Option<OverlayBoundsHandler>,
    on_hover_zones: Option<HoverZonesHandler>,
}

/// The chrome's rects over the centre with their corner radii.
type OverlayRects = Vec<(gpui::Bounds<Pixels>, Pixels)>;

/// Called each frame with the rects of the shell's chrome over the centre
/// (window coordinates: the peek while it shows, with its corner radius) —
/// a host with a native surface under it cuts matching holes.
pub type OverlayBoundsHandler = std::rc::Rc<dyn Fn(Vec<(gpui::Bounds<Pixels>, Pixels)>, &mut Window, &mut App)>;
/// The host's handler for the shell's hover zones (window coordinates): the
/// regions where a pointer should reach the shell even over a native page
/// that otherwise takes every event — the peek's corner. See
/// [`AppShell::on_hover_zones`].
pub type HoverZonesHandler = std::rc::Rc<dyn Fn(Vec<gpui::Bounds<Pixels>>, &mut Window, &mut App)>;

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
        traffic_lights: false,
        native_lights: false,
        page_sheet: false,
        animate: true,
        peek_open: false,
        header_right: None,
        sidebar: None,
        centre: None,
        right: None,
        peek: None,
        chrome_lead: None,
        chrome_switch: None,
        chrome_title: None,
        chrome_right: None,
        on_overlay_bounds: None,
        on_hover_zones: None,
    }
}

impl AppShell {
    /// The sidebar column's resting width (clamped to the resizable range).
    pub fn sidebar_width(mut self, width: impl Into<Pixels>) -> Self {
        self.sidebar_width = width.into();
        self
    }

    /// Overrides the sidebar's minimum width.
    pub fn sidebar_min_width(mut self, min: impl Into<Pixels>) -> Self {
        self.sidebar_min = min.into();
        self
    }

    /// Overrides the sidebar's maximum width.
    pub fn sidebar_max_width(mut self, max: impl Into<Pixels>) -> Self {
        self.sidebar_max = max.into();
        self
    }

    /// A divider drag is in flight: widths feed straight through with no
    /// motion, so the divider never lags the pointer.
    pub fn resizing(mut self, resizing: bool) -> Self {
        self.resizing = resizing;
        self
    }

    /// Whether the bar moves the window (press-drag) and zooms it
    /// (double-click). On by default.
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    /// The right column's resting width (clamped to the resizable range).
    pub fn right_width(mut self, width: impl Into<Pixels>) -> Self {
        self.right_width = width.into();
        self
    }

    /// Overrides the right column's minimum width.
    pub fn right_min_width(mut self, min: impl Into<Pixels>) -> Self {
        self.right_min = min.into();
        self
    }

    /// Overrides the right column's maximum width.
    pub fn right_max_width(mut self, max: impl Into<Pixels>) -> Self {
        self.right_max = max.into();
        self
    }

    /// Whether the right column shows.
    pub fn right_open(mut self, open: bool) -> Self {
        self.right_open = open;
        self
    }

    /// Whether the sidebar column shows.
    pub fn sidebar_open(mut self, open: bool) -> Self {
        self.sidebar_open = open;
        self
    }

    /// Paint the window controls in the bar (a gallery card). A real window
    /// keeps the system's lights and sets [`AppShell::native_lights`].
    pub fn traffic_lights(mut self, on: bool) -> Self {
        self.traffic_lights = on;
        self
    }

    /// The window's own traffic lights: the shell keeps them at the bar's
    /// spot in every state (`place_native_lights`).
    pub fn native_lights(mut self, on: bool) -> Self {
        self.native_lights = on;
        self
    }

    /// The centre is a web page (Browse): no header row; the host's native
    /// page fills the column edge to edge (docs/83 §3).
    pub fn page_sheet(mut self, on: bool) -> Self {
        self.page_sheet = on;
        self
    }

    /// Whether the column animates (default true). Off, open and close are
    /// instant: a host whose native page cannot follow a per-frame resize
    /// smoothly turns them off rather than jank (the peek still slides).
    pub fn animate(mut self, on: bool) -> Self {
        self.animate = on;
        self
    }

    /// Holds the peek open while the sidebar is closed, with no hover — a
    /// scripting and capture aid.
    pub fn peek_open(mut self, open: bool) -> Self {
        self.peek_open = open;
        self
    }

    /// Reports the chrome's rects over the centre every frame they paint
    /// (see [`OverlayBoundsHandler`]); an empty list when none shows.
    pub fn on_overlay_bounds(mut self, f: impl Fn(Vec<(gpui::Bounds<Pixels>, Pixels)>, &mut Window, &mut App) + 'static) -> Self {
        self.on_overlay_bounds = Some(std::rc::Rc::new(f));
        self
    }

    /// Reports the shell's hover zones each frame: the rects (window
    /// coordinates) where a resting pointer arms the peek while the sidebar
    /// is closed. A host with a native page in the centre forwards pointer
    /// moves inside them to its own window, so the peek still arms there.
    /// An empty list when the sidebar is open.
    pub fn on_hover_zones(mut self, f: impl Fn(Vec<gpui::Bounds<Pixels>>, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover_zones = Some(std::rc::Rc::new(f));
        self
    }

    /// The bar's lead — the host's sidebar toggle, drawn at x [`CHROME_LEAD`]
    /// in both states. Resting the pointer on it while the sidebar is closed
    /// arms the peek, like the window's left strip.
    pub fn chrome_lead(mut self, el: impl IntoElement) -> Self {
        self.chrome_lead = Some(el.into_any_element());
        self
    }

    /// The mode switch, ending [`CHROME_SWITCH_INSET`] before the sidebar's
    /// right edge (its resting width, whether or not the column shows, so
    /// it never moves).
    pub fn chrome_switch(mut self, el: impl IntoElement) -> Self {
        self.chrome_switch = Some(el.into_any_element());
        self
    }

    /// The title, in the centre column's 44 px header (at 16 from the
    /// column's left), in every sidebar state.
    pub fn chrome_title(mut self, el: impl IntoElement) -> Self {
        self.chrome_title = Some(el.into_any_element());
        self
    }

    /// The actions, at the centre header's right.
    pub fn chrome_right(mut self, el: impl IntoElement) -> Self {
        self.chrome_right = Some(el.into_any_element());
        self
    }

    /// The peek — the sidebar (without its title bar) as a floating surface
    /// (flush with the window's left, top and bottom, rounded at the bottom
    /// right, behind the bar's controls and the window's lights) that slides in while the
    /// pointer rests on the toggle or the window's left 12 px for 150 ms and
    /// leaves 250 ms after it goes (FF `peek="hover"`, `moderate` /
    /// `moderate.exit`).
    pub fn peek(mut self, el: impl IntoElement) -> Self {
        self.peek = Some(el.into_any_element());
        self
    }

    /// The right column's 44 px header row (the pane's tab strip).
    pub fn header_right(mut self, el: impl IntoElement) -> Self {
        self.header_right = Some(el.into_any_element());
        self
    }

    /// The sidebar pane, in the column under the bar.
    pub fn sidebar(mut self, el: impl IntoElement) -> Self {
        self.sidebar = Some(el.into_any_element());
        self
    }

    /// The centre pane, in the centre column under its header.
    pub fn centre(mut self, el: impl IntoElement) -> Self {
        self.centre = Some(el.into_any_element());
        self
    }

    /// The right pane, in the right column under its header row.
    pub fn right(mut self, el: impl IntoElement) -> Self {
        self.right = Some(el.into_any_element());
        self
    }
}

/// The shell paints the window controls at the bar's spot (12, 16) for a
/// gallery card.
fn painted_lights() -> Div {
    let dot = |c: u32| div().size(px(12.0)).rounded_full().bg(gpui::rgb(c));
    h_flex().flex_none().gap(px(8.0)).child(dot(0xFF5F57)).child(dot(0xFEBC2E)).child(dot(0x28C840))
}

/// Puts the window's own traffic lights at the bar's spot — once per window
/// (the position is remembered), so every layout agrees on it. The shell
/// never moves them.
pub fn place_native_lights(window: &mut Window, cx: &mut App) {
    let placed = window.use_keyed_state("aui-native-lights", cx, |_, _| false);
    if !*placed.read(cx) {
        window.set_traffic_light_position(super::header::traffic_light_position(cx));
        placed.update(cx, |p, _| *p = true);
    }
}

#[derive(Clone, Copy, Default)]
struct ShellState {
    /// The column's last sampled openness, so the closed-only chrome can
    /// wait for it.
    column: f32,
    /// What the pointer rests on: the window's left strip, the toggle (the
    /// bar's lead), the peek card.
    strip: bool,
    lead: bool,
    card: bool,
    /// When the pointer arrived on any of the three, and when it left them all.
    since: Option<std::time::Instant>,
    left_at: Option<std::time::Instant>,
    peeking: bool,
    /// The lead's last painted rect: the toggle is the host's element, so
    /// its hover is read from the pointer's position, not from a handler it
    /// may never surface.
    lead_bounds: Option<gpui::Bounds<Pixels>>,
    /// The host's open flag as last seen, and whether a close happened with
    /// the pointer still on the toggle or an edge: the peek then waits for
    /// the pointer to leave and come back.
    open_seen: bool,
    wait_for_leave: bool,
}

#[derive(Clone, Copy)]
enum HoverSource {
    /// The window's top or left edge.
    Strip,
    Lead,
    Card,
}

impl ShellState {
    /// One hover report: the arm clock starts when the pointer first lands
    /// on any peek source and the dismiss clock when it has left them all,
    /// so moving between the toggle, the strip and the card never resets
    /// either.
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

impl RenderOnce for AppShell {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let shell = self;
        let p = cx.aui().colors;
        let row_h = cx.aui().metrics.header;
        let id = shell.id.clone();
        let state = window.use_keyed_state((id.clone(), "shell"), cx, |_, _| ShellState { column: if shell.sidebar_open { 1.0 } else { 0.0 }, open_seen: shell.sidebar_open, ..Default::default() });
        let mut st = *state.read(cx);
        let now = cx.background_executor().now();
        let win_w = window.viewport_size().width;

        let lo = f32::from(shell.sidebar_min);
        let hi = f32::from(shell.sidebar_max).max(lo);
        let rest = px(f32::from(shell.sidebar_width).clamp(lo, hi));

        // Hover by geometry: the toggle is the host's element, which
        // re-renders on its own hover (when this runs); frames keep coming
        // while the pointer is on it, so leaving is seen too.
        let mouse = window.mouse_position();
        let over_lead = st.lead_bounds.is_some_and(|b| b.contains(&mouse));
        let mut changed = false;
        if over_lead != st.lead {
            st.set_hover(HoverSource::Lead, over_lead, now);
            changed = true;
        }
        // A toggle leaves the pointer on the button it just pressed: the peek
        // waits until the pointer has gone and come back (owner 2026-10-13).
        if shell.sidebar_open != st.open_seen {
            st.open_seen = shell.sidebar_open;
            st.wait_for_leave = !shell.sidebar_open && (st.lead || st.strip);
            changed = true;
        }
        if st.wait_for_leave && !(st.lead || st.strip) {
            st.wait_for_leave = false;
            changed = true;
        }
        if changed {
            state.update(cx, |s, _| *s = st);
        }
        if st.lead {
            aui_motion::request_frame(window);
        }

        // The column: ease-out both ways, the panel sliding with its edge.
        let closed = !shell.sidebar_open;
        if shell.sidebar_open && st.peeking {
            // Opening out of a peek: the sidebar is already in view as the
            // card, so the column starts at rest.
            aui_motion::open_close_jump((id.clone(), "column"), true, window, cx);
        }
        if !shell.animate || shell.resizing {
            aui_motion::open_close_jump((id.clone(), "column"), shell.sidebar_open, window, cx);
        }
        let column = if shell.resizing || !shell.animate {
            if shell.sidebar_open { 1.0 } else { 0.0 }
        } else {
            open_close_with((id.clone(), "column"), shell.sidebar_open, Enter::Tween(COLUMN_OPEN, COLUMN_EASE), COLUMN_CLOSE, COLUMN_EASE, window, cx)
        };
        let sidebar_w = rest * column.clamp(0.0, 1.0);
        let settled_closed = closed && column <= 0.0005;
        if shell.native_lights {
            place_native_lights(window, cx);
        }

        // The peek: armed by a 150 ms rest on the toggle or an edge,
        // dismissed 250 ms after the pointer has left those and the card.
        let any_hovered = st.strip || st.lead || st.card;
        let mut peeking = (st.peeking || shell.peek_open) && settled_closed;
        let mut pending = false;
        if settled_closed && shell.peek.is_some() {
            if !peeking {
                if let (Some(since), true, false) = (st.since, st.strip || st.lead, st.wait_for_leave) {
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
        if !settled_closed && (st.strip || st.card || st.since.is_some() || st.left_at.is_some()) {
            state.update(cx, |s, _| {
                s.strip = false;
                s.card = false;
                s.since = if s.lead { Some(now) } else { None };
                s.left_at = None;
            });
        }
        if pending {
            aui_motion::request_frame(window);
        }
        if column != st.column || peeking != st.peeking {
            state.update(cx, |s, _| {
                s.column = column;
                s.peeking = peeking;
            });
        }

        // The right column on the moderate tier (FF: panel width): exactly
        // the pane's width, its left edge the divider.
        let right_lo = f32::from(shell.right_min);
        let right_rest = px(f32::from(shell.right_width).clamp(right_lo, f32::from(shell.right_max).max(right_lo)));
        let right_target = if shell.right_open { right_rest } else { px(0.0) };
        let right_w = if shell.resizing || !shell.animate {
            right_target
        } else {
            spring_px((id.clone(), "right-width"), right_target, SpringKind::Moderate, window, cx).max(px(0.0))
        };
        let right_mounted = shell.right_open || right_w > px(0.5);

        // The centre's content width at rest for the state the shell is
        // heading to: laid out once, it slides with the column's edge instead
        // of re-measuring on every frame of the column's motion.
        let left_at_rest = if shell.sidebar_open { rest } else { px(0.0) };
        let right_at_rest = if shell.right_open { right_rest } else { px(0.0) };
        let content_w = (win_w - left_at_rest - right_at_rest).max(px(0.0));

        // Region dividers (docs/83 §1 rule 2): one 1 px `line-region`.
        let hairline = px(HAIRLINE);
        // A header's foot: the line over the last pixel of the 44 px row
        // (docs/83 §2.5: 44 incl. its line), so every column's line sits at
        // the same y and every row's content stays centred on the lights.
        let header_line = |left: Pixels, w: Option<Pixels>| {
            let line = div().absolute().top(row_h - hairline).h(hairline).left(left).bg(p.line_region);
            match w {
                Some(w) => line.w(w),
                None => line.right(px(0.0)),
            }
        };

        // The sidebar column, full height on the `sidebar` fill: its top row
        // is the chrome row (drawn by the shell over everything, below) with
        // its header line; the panel keeps its resting width and slides with
        // the column's edge; the right edge is the divider, full height.
        let show_sidebar = shell.sidebar_open || column > 0.0005;
        let sidebar_column = div()
            .w(sidebar_w)
            .h_full()
            .flex_none()
            .relative()
            .overflow_hidden()
            .when(show_sidebar, |d| {
                d.bg(p.sidebar)
                    .child(div().absolute().top(px(0.0)).bottom(px(0.0)).left(sidebar_w - rest).w(rest).pt(row_h).children(shell.sidebar))
                    .child(header_line(px(0.0), None))
                    .child(div().absolute().top(px(0.0)).bottom(px(0.0)).right(px(0.0)).w(hairline).bg(p.line_region))
            });

        // The centre column on the ground, its content at the resting
        // width. A work column carries its 44 px header (title at 16, the
        // actions at the right) ending in its line, in every sidebar state
        // (owner 2026-10-14: "bring back the title and the icons as seen when
        // sidebar is expanded"), and its content starts under it. A page
        // column (Browse) has no header: the page is the column, edge to
        // edge.
        let row_shown = !shell.page_sheet;
        let content_top = if row_shown { row_h } else { px(0.0) };
        let mut centre_column = div()
            .flex_1()
            .h_full()
            .min_w(px(0.0))
            .relative()
            .overflow_hidden()
            .child(div().absolute().top(content_top).bottom(px(0.0)).left(px(0.0)).w(content_w).children(shell.centre));
        if row_shown {
            let row = h_flex()
                .w_full()
                .h(row_h)
                .pl(px(HEADER_PAD_LEFT))
                .pr(px(HEADER_PAD_RIGHT))
                .items_center()
                .child(div().flex_1().min_w(px(0.0)).overflow_hidden().flex().items_center().children(shell.chrome_title))
                .child(div().flex_none().flex().items_center().children(shell.chrome_right));
            let row = if shell.draggable { drag_region((id.clone(), "title-drag")).child(row).into_any_element() } else { row.into_any_element() };
            centre_column = centre_column
                .child(div().absolute().top(px(0.0)).left(px(0.0)).right(px(0.0)).h(row_h).child(row))
                .child(header_line(px(0.0), None));
        }

        // The right column on the ground, its left edge the divider; its
        // header row (Browse: the pane's tab strip) ends in its line. A pane
        // whose screens carry their own rows (D6, the strip) passes no
        // header, and its body fills the column; those rows are the same
        // 44 px with the same line.
        let right_column = div().w(right_w).h_full().flex_none().overflow_hidden().when(right_mounted, |d| {
            d.child(
                div()
                    .w(right_rest)
                    .h_full()
                    .relative()
                    .flex()
                    .flex_col()
                    .border_l_1()
                    .border_color(p.line_region)
                    .when(shell.header_right.is_some(), |d| {
                        d.child(
                            div()
                                .w_full()
                                .h(row_h)
                                .flex_none()
                                .flex()
                                .items_center()
                                .border_b_1()
                                .border_color(p.line_region)
                                .children(shell.header_right),
                        )
                    })
                    .child(div().w_full().flex_1().min_h(px(0.0)).overflow_hidden().children(shell.right)),
            )
        });

        let body = h_flex().absolute().inset_0().items_stretch().child(sidebar_column).child(centre_column).child(right_column);

        let mut root = div().id(id.clone()).relative().size_full().overflow_hidden().bg(p.surface_1).text_color(p.ink).child(body);

        // The chrome's rects over the centre, collected while it prepaints and
        // handed to the host by the last child (see `AppShell::on_overlay_bounds`).
        use gpui_kit::base::ElementExt as _;
        let rects: std::rc::Rc<std::cell::RefCell<OverlayRects>> = Default::default();
        let hover_zones: std::rc::Rc<std::cell::RefCell<Vec<gpui::Bounds<Pixels>>>> = Default::default();

        // The peek: closed, nothing is drawn (owner 2026-10-13: not even the
        // lights); a rest on the window's top edge or its left edge brings
        // the sidebar as a floating surface, its own row riding in with it.
        let mut peek_k: f32 = 0.0;
        if settled_closed {
            if let Some(peek_el) = shell.peek {
                let k = open_close((id.clone(), "peek"), peeking, SpringKind::Moderate, aui_motion::durations::MODERATE_EXIT, window, cx);
                peek_k = k;
                // The peek's sources: the window's top-left corner at the row's
                // size (owner 2026-10-13: the 6 px band alone needed the pointer
                // "all the way to the top corner"), the top band across the
                // window and the left strip down it.
                let corner = div().absolute().left(px(0.0)).top(px(0.0)).w(rest).h(row_h);
                let zones = hover_zones.clone();
                let corner = corner.on_prepaint(move |bounds, _, _| zones.borrow_mut().push(bounds));
                for (name, el) in [
                    ("peek-corner", corner),
                    ("peek-top", div().absolute().left(px(0.0)).top(px(0.0)).right(px(0.0)).h(px(PEEK_TOP_BAND))),
                    ("peek-left", div().absolute().left(px(0.0)).top(px(PEEK_TOP_BAND)).bottom(px(0.0)).w(px(PEEK_STRIP))),
                ] {
                    let edge_state = state.clone();
                    root = root.child(el.id((id.clone(), name)).on_hover(move |hovered, _, cx| {
                        let hovered = *hovered;
                        edge_state.update(cx, |s, cx| {
                            s.set_hover(HoverSource::Strip, hovered, cx.background_executor().now());
                            cx.notify();
                        });
                    }));
                }
                if k > 0.0 {
                    let card_state = state.clone();
                    let report = rects.clone();
                    // From −108 % of its width to its resting place: the
                    // sidebar's own column on the `sidebar` fill, flush with
                    // the window's left, top and bottom, the sidebar's width,
                    // rounded at the bottom right because it floats; its rows
                    // start under its row and that row's line. Its
                    // `line-strong` right edge and the float shadow are what
                    // lift it off the centre (docs/83 §3).
                    let travel = rest * 1.08;
                    root = root.child(
                        div()
                            .id((id.clone(), "peek-card"))
                            .absolute()
                            .top(px(0.0))
                            .bottom(px(0.0))
                            .left(px(0.0) - travel * (1.0 - k))
                            .w(rest)
                            .rounded_br(px(scale::SHAPE_CONTAINER))
                            .overflow_hidden()
                            .bg(p.sidebar)
                            // It floats: a `line-strong` right edge and the
                            // one float shadow (docs/83 §2.3, §3).
                            .border_r_1()
                            .border_color(p.line_strong)
                            .shadow(p.floating_shadow())
                            .occlude()
                            .on_hover(move |hovered, _, cx| {
                                let hovered = *hovered;
                                card_state.update(cx, |s, cx| {
                                    s.set_hover(HoverSource::Card, hovered, cx.background_executor().now());
                                    cx.notify();
                                });
                            })
                            .child(div().absolute().top(row_h).bottom(px(0.0)).left(px(0.0)).right(px(0.0)).child(peek_el))
                            .child(header_line(px(0.0), None))
                            .on_prepaint(move |bounds, _, _| report.borrow_mut().push((bounds, px(scale::SHAPE_CONTAINER)))),
                    );
                }
            }
        }

        // The chrome row: lights, the host's toggle at CHROME_LEAD, the mode
        // switch ending CHROME_SWITCH_INSET before the sidebar's right edge —
        // the sidebar column's own title row. Open, it stands at
        // the window's corner; closed it is gone, and it rides in with the
        // peek card. The lights are the host's to hide and show with it.
        let row_left = if !settled_closed {
            Some(px(0.0))
        } else if peek_k > 0.0 {
            Some(px(0.0) - rest * 1.08 * (1.0 - peek_k))
        } else {
            None
        };
        if let Some(row_left) = row_left {
            let lead_state = state.clone();
            let lead = div()
                .absolute()
                .left(px(CHROME_LEAD))
                .top(px(0.0))
                .h(row_h)
                .flex()
                .items_center()
                .children(shell.chrome_lead)
                .on_prepaint(move |bounds, _, cx| {
                    lead_state.update(cx, |s, _| s.lead_bounds = Some(bounds));
                });
            let switch = div().absolute().left(px(0.0)).top(px(0.0)).w(rest).h(row_h).flex().items_center().justify_end().pr(px(CHROME_SWITCH_INSET)).children(shell.chrome_switch);
            // The drag region wraps a row with a real box: around an absolutely
            // placed row it had no height, so a press or a double-click on the
            // row's empty ground never reached it and the window could not be
            // moved or zoomed from it (owner 2026-10-13).
            let inner = div().relative().w(rest).h(row_h).child(switch).child(lead);
            let inner = if shell.draggable { drag_region((id.clone(), "row-drag")).child(inner).into_any_element() } else { inner.into_any_element() };
            root = root.child(div().absolute().top(px(0.0)).left(row_left).w(rest).h(row_h).child(inner));
        } else if st.lead_bounds.is_some() {
            // No toggle on screen: its last rect must not keep arming the peek.
            state.update(cx, |s, _| s.lead_bounds = None);
        }
        if shell.traffic_lights {
            let at = super::header::traffic_light_position(cx);
            root = root.child(painted_lights().absolute().left(at.x).top(at.y));
        }

        if let Some(handler) = shell.on_overlay_bounds {
            let rects = rects.clone();
            root = root.child(div().absolute().size_0().on_prepaint(move |_, window, cx| handler(rects.take(), window, cx)));
        }
        if let Some(handler) = shell.on_hover_zones {
            let zones = hover_zones.clone();
            root = root.child(div().absolute().size_0().on_prepaint(move |_, window, cx| handler(zones.take(), window, cx)));
        }

        crate::keys::track_pointer(root)
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
    }

    #[test]
    fn clamp_right_keeps_drag_widths_in_range() {
        assert_eq!(clamp_right_width(400.0), 400.0);
        assert_eq!(clamp_right_width(0.0), RIGHT_MIN_WIDTH);
        assert_eq!(clamp_right_width(10_000.0), RIGHT_MAX_WIDTH);
    }

    /// One arm clock across the toggle, the strip and the card: moving from
    /// the toggle onto the card keeps the peek, and the dismiss clock only
    /// starts once the pointer has left all three.
    #[test]
    fn peek_hover_clocks_span_all_three_sources() {
        let t0 = std::time::Instant::now();
        let later = |ms: u64| t0 + std::time::Duration::from_millis(ms);
        let mut s = ShellState::default();
        s.set_hover(HoverSource::Lead, true, t0);
        assert_eq!(s.since, Some(t0));
        assert_eq!(s.left_at, None);
        s.set_hover(HoverSource::Card, true, later(200));
        s.set_hover(HoverSource::Lead, false, later(201));
        assert_eq!(s.since, Some(t0), "the arm clock is not reset by a move within the sources");
        assert_eq!(s.left_at, None);
        s.set_hover(HoverSource::Card, false, later(900));
        assert_eq!(s.since, None);
        assert_eq!(s.left_at, Some(later(900)));
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
