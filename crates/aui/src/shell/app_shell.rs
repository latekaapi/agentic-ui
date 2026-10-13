//! The app shell (canvas "Baaz v3", chrome revised 2026-10-13, round 3): the
//! **sidebar column** full height on the frame, its top row the chrome row
//! (the window's lights, the host's toggle at x 84, the mode switch ending
//! 12 px before the column's right edge), and the work area as a **sheet**
//! 6 px in from the window and the column, radius 8, with its own 32 px
//! title row (title at 16, actions at the right) centred on the lights'
//! line; the right pane is a second sheet beside it.
//!
//! Closing removes the column; the sheet grows into the space. What stays is
//! the chrome row, now a **pill**: the sidebar's own top strip — a plain
//! rectangle flush with the window's top and left, the sidebar's width, the
//! row's height, the frame's own colour with no edge and no drop — holding
//! exactly lights · toggle · switch at the x and y they had open. A 150 ms
//! rest on the pill, the toggle or the window's left strip peeks the whole
//! sidebar as the frame-coloured column flush with the window's left, top
//! and bottom behind the pill (one surface with it, rounded 8 at the bottom
//! right, a drop to its right); after a toggle the peek waits for the
//! pointer to leave and come back.
//!
//! Motion (Emil Kowalski's rules, owner 2026-10-13: smooth or none): the
//! column opens and closes on an ease-out tween, 260 / 220 ms, the panel
//! sliding with the column's edge; while it moves, the sheet's content is
//! laid out once at the width it will have at rest and slides with the
//! sheet's edge, so no frame re-lays out the transcript (the build before
//! re-measured per frame and managed four frames per close).

use aui_motion::{open_close, open_close_with, spring_px, Enter, SpringKind, EXIT_EASING};
use aui_tokens::{ActiveAui, Easing};
use gpui::{div, prelude::*, px, AnyElement, App, Div, ElementId, IntoElement, Pixels, Window};
use gpui_kit::base::h_flex;

use super::drag_region::drag_region;

/// Sidebar column width: Fluid Functionalism's 16rem (owner 2026-10-11).
pub const SIDEBAR_WIDTH: f32 = 256.0;
/// Right column width in the app.
pub const RIGHT_WIDTH: f32 = 400.0;
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

/// The sheet's inset from the window and the sidebar column.
pub const SHEET_INSET: f32 = 6.0;
/// The sheet's corner radius.
pub const SHEET_RADIUS: f32 = 8.0;
/// The sheet's own title row (open): height, the title's inset from the
/// sheet's left, the actions' from its right; its centre is the lights'
/// line (window y 22 = 6 + 16).
pub const SHEET_ROW_H: f32 = 32.0;
/// See [`SHEET_ROW_H`].
pub const SHEET_ROW_PAD_LEFT: f32 = 16.0;
/// See [`SHEET_ROW_H`].
pub const SHEET_ROW_PAD_RIGHT: f32 = 10.0;
/// The scroll fade at the sheet's top: under the title row (open) and under
/// the pill (closed).
pub const SHEET_FADE_OPEN: f32 = 64.0;
/// See [`SHEET_FADE_OPEN`].
pub const SHEET_FADE_CLOSED: f32 = 92.0;
/// The chrome row: the host's toggle starts here, past the window's lights
/// (the v2 sidebar's title bar put it at the same x).
pub const CHROME_LEAD: f32 = 84.0;
/// The mode switch ends this far before the sidebar's right edge.
pub const CHROME_SWITCH_INSET: f32 = 12.0;
/// Depth (docs/37): the sheet's contact shadow onto the frame beside the
/// open sidebar — width, and its strength at the sheet's edge per theme.
pub const CONTACT_W: f32 = 20.0;
const CONTACT_ALPHA_DARK: f32 = 0.16;
const CONTACT_ALPHA_LIGHT: f32 = 0.06;
/// The fade a list scrolls under at the sidebar column's top and at the peek
/// card's content top.
pub const LIST_FADE: f32 = 24.0;
/// The peek: its hover strip on the window's left, arm and dismiss delays.
pub const PEEK_STRIP: f32 = 12.0;
const PEEK_ARM: std::time::Duration = std::time::Duration::from_millis(150);
const PEEK_DISMISS: std::time::Duration = std::time::Duration::from_millis(250);
/// The column's open and close: ease-out (Emil: ease-out for what the
/// person asked for; no bounce on a panel that must land exactly).
pub const COLUMN_EASE: Easing = Easing([0.22, 1.0, 0.36, 1.0]);
const COLUMN_OPEN: std::time::Duration = std::time::Duration::from_millis(260);
const COLUMN_CLOSE: std::time::Duration = std::time::Duration::from_millis(220);
/// The pill's ground fades in once the column has closed (FF popup timing)
/// and leaves before it opens.
const PILL_IN: std::time::Duration = std::time::Duration::from_millis(80);
/// Browse closed (owner 2026-10-13): the strip is not always there — a native
/// page cannot run under it the way the transcript does. It fades in when the
/// pointer reaches the window's top edge (the frame band above the page: the
/// page window takes every event over itself, so that band is the only place
/// a pointer near the top can be seen), right after the sidebar closes and
/// while the sidebar peeks; it fades out `STRIP_LINGER` after the pointer
/// last left it. The hole under it cross-fades with it.
pub const STRIP_LINGER: std::time::Duration = std::time::Duration::from_millis(2000);
const STRIP_OUT: std::time::Duration = std::time::Duration::from_millis(200);

/// Whether the fading strip is due now: never gone when it does not fade;
/// otherwise while hovered or peeking, and for [`STRIP_LINGER`] after the
/// pointer last touched the top edge or the strip, or the sidebar closed.
fn strip_due(fades: bool, hovered: bool, peeking: bool, top_seen: Option<std::time::Instant>, closed_at: Option<std::time::Instant>, now: std::time::Instant) -> bool {
    let recent = |t: Option<std::time::Instant>| t.is_some_and(|t| now.duration_since(t) < STRIP_LINGER);
    !fades || hovered || peeking || recent(top_seen) || recent(closed_at)
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
    traffic_lights: bool,
    native_lights: bool,
    page_sheet: bool,
    /// The closed strip fades (Browse); see [`STRIP_LINGER`].
    strip_fades: bool,
    /// Holds the fading strip shown (captures).
    strip_shown: bool,
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
}

/// The chrome over the sheet's rects with their corner radii.
type OverlayRects = Vec<(gpui::Bounds<Pixels>, Pixels, f32)>;

/// Called each frame with the rects of the shell's chrome over the sheet
/// (window coordinates: the peek card while it shows, with its corner
/// radius) — a host with a native surface under it cuts matching holes.
pub type OverlayBoundsHandler = std::rc::Rc<dyn Fn(Vec<(gpui::Bounds<Pixels>, Pixels, f32)>, &mut Window, &mut App)>;

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
        strip_fades: false,
        strip_shown: false,
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

    /// Whether the right sheet shows.
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

    /// The sheet is a web page (Browse): white, its edge a 50 % black ring (a
    /// white highlight cannot show on white), no fade; the host's native page
    /// covers it.
    pub fn page_sheet(mut self, on: bool) -> Self {
        self.page_sheet = on;
        self
    }
    /// The closed strip fades instead of staying (Browse, owner 2026-10-13):
    /// in on a pointer at the window's top edge, after a close and during a
    /// peek; out [`STRIP_LINGER`] after the pointer last left it. The hole
    /// the host cuts for it carries the same opacity, so it cross-fades with
    /// the page.
    pub fn strip_fades(mut self, on: bool) -> Self {
        self.strip_fades = on;
        self
    }

    /// Holds the fading strip shown with no pointer (captures).
    pub fn strip_shown(mut self, on: bool) -> Self {
        self.strip_shown = on;
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

    /// Reports the chrome's rects over the sheet every frame they paint
    /// (see [`OverlayBoundsHandler`]); an empty list when none shows.
    pub fn on_overlay_bounds(mut self, f: impl Fn(Vec<(gpui::Bounds<Pixels>, Pixels, f32)>, &mut Window, &mut App) + 'static) -> Self {
        self.on_overlay_bounds = Some(std::rc::Rc::new(f));
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

    /// The title, in the sheet's own title row (at 16 from the sheet's
    /// left) while the sidebar is open.
    pub fn chrome_title(mut self, el: impl IntoElement) -> Self {
        self.chrome_title = Some(el.into_any_element());
        self
    }

    /// The actions, at the sheet row's right while the sidebar is open.
    pub fn chrome_right(mut self, el: impl IntoElement) -> Self {
        self.chrome_right = Some(el.into_any_element());
        self
    }

    /// The peek — the sidebar (without its title bar) as a Floating card
    /// (radius 8, 6 px from the window's left, top and bottom, behind the
    /// bar's controls and the window's lights) that slides in while the
    /// pointer rests on the toggle or the window's left 12 px for 150 ms and
    /// leaves 250 ms after it goes (FF `peek="hover"`, `moderate` /
    /// `moderate.exit`).
    pub fn peek(mut self, el: impl IntoElement) -> Self {
        self.peek = Some(el.into_any_element());
        self
    }

    /// The right sheet's header row (the pane's tab strip).
    pub fn header_right(mut self, el: impl IntoElement) -> Self {
        self.header_right = Some(el.into_any_element());
        self
    }

    /// The sidebar pane, in the column under the bar.
    pub fn sidebar(mut self, el: impl IntoElement) -> Self {
        self.sidebar = Some(el.into_any_element());
        self
    }

    /// The centre pane, in the sheet.
    pub fn centre(mut self, el: impl IntoElement) -> Self {
        self.centre = Some(el.into_any_element());
        self
    }

    /// The right pane, in the right sheet under its header row.
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

/// The peek card's shadow (docs/37 G2): one drop cast to the right, so the
/// card's edge over the sheet reads like Claude Desktop's hover panel (no
/// edge ring: the card is the frame's own surface); light keeps FF's float
/// recipe.
fn peek_shadow(p: &aui_tokens::Palette) -> Vec<gpui::BoxShadow> {
    if p.is_dark() {
        vec![gpui::BoxShadow {
            color: gpui::black().alpha(0.45),
            offset: gpui::point(px(8.0), px(0.0)),
            blur_radius: px(24.0),
            spread_radius: px(-8.0),
            inset: false,
        }]
    } else {
        p.floating_shadow()
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
    /// may never surface. The pills likewise.
    lead_bounds: Option<gpui::Bounds<Pixels>>,
    pill_bounds: Option<gpui::Bounds<Pixels>>,
    /// The pointer rests on the pill.
    pill: bool,
    /// The host's open flag as last seen, and whether a close happened with
    /// the pointer still on the toggle / pill / strip: the peek then waits
    /// for the pointer to leave and come back.
    open_seen: bool,
    wait_for_leave: bool,
    /// The fading strip: the pointer rests on the window's top edge; when
    /// it last touched the edge or the strip; when the sidebar last closed.
    top: bool,
    top_seen: Option<std::time::Instant>,
    closed_at: Option<std::time::Instant>,
}

#[derive(Clone, Copy)]
enum HoverSource {
    Strip,
    Lead,
    Pill,
    Card,
}

impl ShellState {
    /// One hover report: the arm clock starts when the pointer first lands
    /// on any peek source and the dismiss clock when it has left them all,
    /// so moving between the toggle, the strip and the card never resets
    /// either.
    fn set_hover(&mut self, source: HoverSource, hovered: bool, now: std::time::Instant) {
        let before = self.strip || self.lead || self.pill || self.card;
        match source {
            HoverSource::Strip => self.strip = hovered,
            HoverSource::Lead => self.lead = hovered,
            HoverSource::Pill => self.pill = hovered,
            HoverSource::Card => self.card = hovered,
        }
        let after = self.strip || self.lead || self.pill || self.card;
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

        // Hover by geometry: the toggle and the pills are the host's
        // elements, which re-render on their own hover (when this runs);
        // frames keep coming while the pointer is on them, so leaving is
        // seen too.
        let mouse = window.mouse_position();
        let over_lead = st.lead_bounds.is_some_and(|b| b.contains(&mouse));
        let over_pill = st.pill_bounds.is_some_and(|b| b.contains(&mouse));
        let mut changed = false;
        if over_lead != st.lead {
            st.set_hover(HoverSource::Lead, over_lead, now);
            changed = true;
        }
        if over_pill != st.pill {
            st.set_hover(HoverSource::Pill, over_pill, now);
            changed = true;
        }
        // A toggle leaves the pointer on the button it just pressed: the peek
        // waits until the pointer has gone and come back (owner 2026-10-13).
        if shell.sidebar_open != st.open_seen {
            st.open_seen = shell.sidebar_open;
            st.wait_for_leave = !shell.sidebar_open && (st.lead || st.pill || st.strip);
            if !shell.sidebar_open {
                st.closed_at = Some(now);
            }
            changed = true;
        }
        // The fading strip stays while the pointer is on it or its toggle;
        // the clock restarts when the pointer leaves.
        if shell.strip_fades && (st.pill || st.lead) {
            st.top_seen = Some(now);
            changed = true;
        }
        if st.wait_for_leave && !(st.lead || st.pill || st.strip) {
            st.wait_for_leave = false;
            changed = true;
        }
        if changed {
            state.update(cx, |s, _| *s = st);
        }
        if st.lead || st.pill {
            window.request_animation_frame();
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
            aui_motion::open_close_jump((id.clone(), "pill"), closed, window, cx);
        }
        let column = if shell.resizing || !shell.animate {
            if shell.sidebar_open { 1.0 } else { 0.0 }
        } else {
            open_close_with((id.clone(), "column"), shell.sidebar_open, Enter::Tween(COLUMN_OPEN, COLUMN_EASE), COLUMN_CLOSE, COLUMN_EASE, window, cx)
        };
        let sidebar_w = rest * column.clamp(0.0, 1.0);
        let settled_closed = closed && column <= 0.0005;
        let settled_open = shell.sidebar_open && column >= 0.9995;
        // The pill's ground arrives once the column has settled closed (FF
        // popup timing, 80 ms, a 4 px rise) and leaves before it opens.
        let strip_hovered = st.top || st.pill || st.lead || st.strip;
        let due = shell.strip_shown || strip_due(shell.strip_fades, strip_hovered, st.peeking || shell.peek_open, st.top_seen, st.closed_at, now);
        let pill_wanted = closed && st.column <= 0.0005 && due;
        let pill_k = if !shell.animate && !shell.strip_fades {
            if closed { 1.0 } else { 0.0 }
        } else {
            let out = if shell.strip_fades { STRIP_OUT } else { aui_motion::durations::FAST_EXIT };
            open_close_with((id.clone(), "pill"), pill_wanted, Enter::Tween(PILL_IN, Easing::OUT), out, EXIT_EASING, window, cx)
        };
        if (shell.animate || shell.strip_fades) && settled_closed && !pill_wanted && pill_k > 0.0 {
            window.request_animation_frame();
        }
        // A lingering strip needs a frame when its time is up.
        if shell.strip_fades && closed && due && !strip_hovered && !shell.strip_shown {
            window.request_animation_frame();
        }
        if shell.native_lights {
            place_native_lights(window, cx);
        }

        // The peek: armed by a 150 ms rest on the toggle or the strip,
        // dismissed 250 ms after the pointer has left those and the card.
        let any_hovered = st.strip || st.lead || st.pill || st.card;
        let mut peeking = (st.peeking || shell.peek_open) && settled_closed;
        let mut pending = false;
        if settled_closed && shell.peek.is_some() {
            if !peeking {
                if let (Some(since), true, false) = (st.since, st.strip || st.lead || st.pill, st.wait_for_leave) {
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
                s.since = if s.lead || s.pill { Some(now) } else { None };
                s.left_at = None;
            });
        }
        if pending {
            window.request_animation_frame();
        }
        if column != st.column || peeking != st.peeking {
            state.update(cx, |s, _| {
                s.column = column;
                s.peeking = peeking;
            });
        }

        // The right sheet on the moderate tier (FF: panel width).
        let right_lo = f32::from(shell.right_min);
        let right_rest = px(f32::from(shell.right_width).clamp(right_lo, f32::from(shell.right_max).max(right_lo)));
        // The right column: its gap from the centre sheet, the sheet at the
        // pane's width, its gap from the window.
        let right_target = if shell.right_open { right_rest + px(2.0 * SHEET_INSET) } else { px(0.0) };
        let right_w = if shell.resizing || !shell.animate {
            right_target
        } else {
            spring_px((id.clone(), "right-width"), right_target, SpringKind::Moderate, window, cx).max(px(0.0))
        };
        let right_mounted = shell.right_open || right_w > px(0.5);

        // The sheet's content width at rest for the state the shell is
        // heading to: laid out once, it slides with the sheet's edge instead
        // of re-measuring on every frame of the column's motion.
        let left_at_rest = if shell.sidebar_open { rest } else { px(0.0) };
        let right_at_rest = if shell.right_open { right_rest + px(2.0 * SHEET_INSET) } else { px(SHEET_INSET) };
        let content_w = (win_w - left_at_rest - px(SHEET_INSET) - right_at_rest).max(px(0.0));

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
        let fade = |ground: gpui::Hsla, h: f32| {
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .h(px(h))
                .bg(gpui::linear_gradient(180.0, gpui::linear_color_stop(ground, 0.0), gpui::linear_color_stop(ground.alpha(0.0), 1.0)))
        };

        // The sidebar column, full height: its top row is the chrome row
        // (drawn by the shell over everything, below); the panel keeps its
        // resting width and slides with the column's edge; rows fade under
        // the row.
        let show_sidebar = shell.sidebar_open || column > 0.0005;
        let sidebar_column = div()
            .w(sidebar_w)
            .h_full()
            .flex_none()
            .relative()
            .overflow_hidden()
            .when(show_sidebar, |d| {
                d.child(div().absolute().top(px(0.0)).bottom(px(0.0)).left(sidebar_w - rest).w(rest).pt(row_h).children(shell.sidebar))
                    .child(fade(p.surface_1, LIST_FADE).top(row_h).left(sidebar_w - rest).w(rest))
            });

        // The centre sheet, its content at the resting width; fully open it
        // carries its own title row (title at 16, the actions at the right)
        // over a fade; closed, the content runs under the pill through a
        // taller fade and the pill alone stays.
        let fade_h = if settled_open && !shell.page_sheet { SHEET_FADE_OPEN } else { SHEET_FADE_CLOSED };
        let mut centre_sheet = sheet_box(shell.page_sheet).child(div().absolute().top(px(0.0)).bottom(px(0.0)).left(px(0.0)).w(content_w).children(shell.centre));
        if !shell.page_sheet {
            centre_sheet = centre_sheet.child(fade(p.surface_2, fade_h));
        }
        let row_shown = settled_open && !shell.page_sheet;
        if row_shown {
            let row = h_flex()
                .w_full()
                .h(px(SHEET_ROW_H))
                .pl(px(SHEET_ROW_PAD_LEFT))
                .pr(px(SHEET_ROW_PAD_RIGHT))
                .items_center()
                .child(div().flex_1().min_w(px(0.0)).overflow_hidden().flex().items_center().children(shell.chrome_title))
                .child(div().flex_none().flex().items_center().children(shell.chrome_right));
            let row = if shell.draggable { drag_region((id.clone(), "row-drag")).child(row).into_any_element() } else { row.into_any_element() };
            centre_sheet = centre_sheet.child(div().absolute().top(px(0.0)).left(px(0.0)).right(px(0.0)).h(px(SHEET_ROW_H)).child(row));
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
                div().w(right_rest + px(2.0 * SHEET_INSET)).h_full().pl(px(SHEET_INSET)).pt(px(SHEET_INSET)).pb(px(SHEET_INSET)).pr(px(SHEET_INSET)).child(
                    sheet_box(false).child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .child(div().w_full().h(row_h).flex_none().flex().items_center().children(shell.header_right))
                            .child(div().w_full().flex_1().min_h(px(0.0)).overflow_hidden().children(shell.right)),
                    ),
                ),
            )
        });

        // The sheet's contact shadow onto the frame (docs/37 G1): beside the
        // sheet only, riding and fading with the column.
        let contact = show_sidebar.then(|| {
            let a = if p.is_dark() { CONTACT_ALPHA_DARK } else { CONTACT_ALPHA_LIGHT };
            div()
                .absolute()
                .top(px(SHEET_INSET))
                .bottom(px(SHEET_INSET))
                .left(sidebar_w + px(SHEET_INSET) - px(CONTACT_W))
                .w(px(CONTACT_W))
                .opacity(column.clamp(0.0, 1.0))
                .bg(gpui::linear_gradient(90.0, gpui::linear_color_stop(gpui::black().alpha(0.0), 0.0), gpui::linear_color_stop(gpui::black().alpha(a), 1.0)))
        });

        let body = h_flex().absolute().inset_0().items_stretch().child(sidebar_column).child(centre_column).child(right_column);

        let mut root = div().id(id.clone()).relative().size_full().overflow_hidden().bg(p.surface_1).text_color(p.ink).child(body).children(contact);

        // The chrome's rects over the sheet, collected while it prepaints and
        // handed to the host by the last child (see `AppShell::on_overlay_bounds`).
        use gpui_kit::base::ElementExt as _;
        let rects: std::rc::Rc<std::cell::RefCell<OverlayRects>> = Default::default();

        // The peek strip and card, under the chrome row.
        if settled_closed && shell.strip_fades {
            // The window's top edge: the frame band above the page, the one
            // place a pointer near the top reaches the shell. Resting there
            // brings the strip; leaving starts its linger.
            let top_state = state.clone();
            root = root.child(
                div()
                    .id((id.clone(), "strip-reveal"))
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .right(px(0.0))
                    .h(px(SHEET_INSET))
                    .on_hover(move |hovered, _, cx| {
                        let hovered = *hovered;
                        top_state.update(cx, |s, cx| {
                            s.top = hovered;
                            s.top_seen = Some(cx.background_executor().now());
                            cx.notify();
                        });
                    }),
            );
        }
        if settled_closed {
            if let Some(peek_el) = shell.peek {
                let k = open_close((id.clone(), "peek"), peeking, SpringKind::Moderate, aui_motion::durations::MODERATE_EXIT, window, cx);
                let strip_state = state.clone();
                root = root.child(
                    div()
                        .id((id.clone(), "peek-strip"))
                        .absolute()
                        .left(px(0.0))
                        .top(row_h)
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
                    let report = rects.clone();
                    // From −108 % of its width to its resting place: the
                    // sidebar's own column in the frame's colour, flush with
                    // the window's left, top and bottom, the sidebar's width,
                    // rounded 8 at the bottom right, one surface with the pill
                    // above it (owner 2026-10-13); its rows start under the
                    // pill. Its drop to the right is what lifts it off the
                    // sheet.
                    let travel = rest * 1.08;
                    root = root.child(
                        div()
                            .id((id.clone(), "peek-card"))
                            .absolute()
                            .top(px(0.0))
                            .bottom(px(0.0))
                            .left(px(0.0) - travel * (1.0 - k))
                            .w(rest)
                            .rounded_br(px(SHEET_RADIUS))
                            .overflow_hidden()
                            .bg(p.surface_1)
                            .shadow(peek_shadow(&p))
                            .occlude()
                            .on_hover(move |hovered, _, cx| {
                                let hovered = *hovered;
                                card_state.update(cx, |s, cx| {
                                    s.set_hover(HoverSource::Card, hovered, cx.background_executor().now());
                                    cx.notify();
                                });
                            })
                            .child(div().absolute().top(row_h).bottom(px(0.0)).left(px(0.0)).right(px(0.0)).child(peek_el))
                            .child(fade(p.surface_1, LIST_FADE).top(row_h))
                            .on_prepaint(move |bounds, _, _| report.borrow_mut().push((bounds, px(SHEET_RADIUS), 1.0))),
                    );
                }
            }
        }

        // The chrome row: lights, the host's toggle at CHROME_LEAD, the mode
        // switch ending CHROME_SWITCH_INSET before the sidebar's right edge —
        // at the same x and the same line (y 22) in both states. Open it is
        // the sidebar column's title row, on the frame; closed, its ground is
        // the pill: the sidebar's own top strip on the frame's colour.
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
        let mut row = div().absolute().top(px(0.0)).left(px(0.0)).w(rest).h(row_h).child(switch).child(lead);
        if pill_k > 0.0 {
            // The pill's ground: the frame's own colour — the surface under
            // everything else (owner 2026-10-13) — a plain strip flush with
            // the window's top and left, exactly the sidebar's width and the
            // row's height, so the peek card below it continues it as one
            // surface; it rises 4 px as it fades in. Over a native page the
            // host cuts it as a hole.
            let pill_state = state.clone();
            let report = rects.clone();
            let ground = div()
                .absolute()
                .left(px(0.0))
                .top(px(4.0 * (1.0 - pill_k)))
                .w(rest)
                .h(row_h)
                .bg(p.surface_1)
                .opacity(pill_k)
                .on_prepaint(move |bounds, _, cx| {
                    pill_state.update(cx, |s, _| s.pill_bounds = Some(bounds));
                    // The hole cross-fades with the strip.
                    report.borrow_mut().push((bounds, px(0.0), pill_k));
                });
            row = div().absolute().top(px(0.0)).left(px(0.0)).w(rest).h(row_h).child(ground).child(row);
        } else if st.pill_bounds.is_some() {
            state.update(cx, |s, _| s.pill_bounds = None);
        }
        if shell.strip_fades && closed {
            // The controls go with the strip's ground.
            row = row.opacity(pill_k);
        }
        let row = if shell.draggable { drag_region((id.clone(), "row-drag")).child(row).into_any_element() } else { row.into_any_element() };
        root = root.child(row);
        if shell.traffic_lights {
            let at = super::header::traffic_light_position(cx);
            root = root.child(painted_lights().absolute().left(at.x).top(at.y));
        }

        if let Some(handler) = shell.on_overlay_bounds {
            let rects = rects.clone();
            root = root.child(div().absolute().size_0().on_prepaint(move |_, window, cx| handler(rects.take(), window, cx)));
        }

        crate::keys::track_pointer(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fading_strip_lingers_two_seconds_after_the_pointer() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        // Not fading: always due.
        assert!(strip_due(false, false, false, None, None, t0));
        // Fading, nothing seen: gone.
        assert!(!strip_due(true, false, false, None, None, t0));
        // Hovered or peeking: due.
        assert!(strip_due(true, true, false, None, None, t0));
        assert!(strip_due(true, false, true, None, None, t0));
        // The pointer left the edge: due for STRIP_LINGER, then gone.
        assert!(strip_due(true, false, false, Some(t0), None, t0 + STRIP_LINGER - Duration::from_millis(1)));
        assert!(!strip_due(true, false, false, Some(t0), None, t0 + STRIP_LINGER));
        // The sidebar just closed: the same linger.
        assert!(strip_due(true, false, false, None, Some(t0), t0 + Duration::from_millis(1999)));
        assert!(!strip_due(true, false, false, None, Some(t0), t0 + Duration::from_millis(2000)));
    }

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
