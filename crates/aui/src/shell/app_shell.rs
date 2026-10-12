//! The app shell (canvas "Baaz v3", chrome revised 2026-10-13): one horizontal
//! **bar** across the window's top on the frame colour, the **sidebar column**
//! under it on the frame, and the work area as a **sheet** 6 px in from the
//! bar, the column and the window, radius 8; the right pane is a second sheet
//! beside it.
//!
//! The bar never moves between states: the window's lights at (12, 16), the
//! host's sidebar toggle at x 84 ([`AppShell::chrome_lead`]), the mode
//! switch at the sidebar's right edge ([`AppShell::chrome_switch`]), the
//! title where the sheet's text starts ([`AppShell::chrome_title`]) and the
//! actions at the bar's right ([`AppShell::chrome_right`]). Closing only
//! removes the sidebar column; the sheet grows into the space.
//!
//! Motion (Emil Kowalski's rules, owner 2026-10-13: smooth or none): the
//! column opens and closes on an ease-out tween, 260 / 220 ms, with the
//! panel sliding with the column's edge. While the column moves, the sheet's
//! content is laid out once at the width it will have at rest and slides
//! with the sheet's edge, so no frame re-lays out the transcript — the
//! previous build re-measured everything per frame and managed four frames
//! per close. The peek (the sidebar as a Floating card behind the lights,
//! from a rest on the toggle or the window's left strip) slides on FF
//! `moderate`.

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

/// The sheet's inset from the bar, the window and the sidebar column.
pub const SHEET_INSET: f32 = 6.0;
/// The sheet's corner radius.
pub const SHEET_RADIUS: f32 = 8.0;
/// The scroll fade at the sheet's top (sheet → clear).
pub const SHEET_FADE: f32 = 32.0;
/// The bar: the host's toggle starts here, past the window's lights (the
/// v2 sidebar's title bar put it at the same x). The sidebar's
/// `traffic_light_gap` equals it.
pub const CHROME_LEAD: f32 = 84.0;
/// The mode switch ends this far before the sidebar's right edge.
pub const CHROME_SWITCH_INSET: f32 = 12.0;
/// The title starts this far past the sidebar's edge: the sheet's inset plus
/// the sheet's text inset, so it stands where the transcript's text does.
pub const CHROME_TITLE_INSET: f32 = SHEET_INSET + 16.0;
/// The actions end this far from the window's right edge.
pub const CHROME_ACTIONS_PAD: f32 = 16.0;
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
/// The closed-only chrome fades in once the column has closed (FF popup
/// timing) and leaves before it opens.
const CLOSED_ONLY_IN: std::time::Duration = std::time::Duration::from_millis(80);
/// The title slot keeps this much room for the actions at the bar's right.
const CHROME_ACTIONS_ROOM: f32 = 160.0;

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
    chrome_closed: Option<AnyElement>,
    chrome_right: Option<AnyElement>,
    on_overlay_bounds: Option<OverlayBoundsHandler>,
}

/// The chrome over the sheet's rects with their corner radii.
type OverlayRects = Vec<(gpui::Bounds<Pixels>, Pixels)>;

/// Called each frame with the rects of the shell's chrome over the sheet
/// (window coordinates: the peek card while it shows, with its corner
/// radius) — a host with a native surface under it cuts matching holes.
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
        chrome_closed: None,
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
    pub fn on_overlay_bounds(mut self, f: impl Fn(Vec<(gpui::Bounds<Pixels>, Pixels)>, &mut Window, &mut App) + 'static) -> Self {
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

    /// The mode switch, at the sidebar's right edge (its resting width,
    /// whether or not the column shows, so it never moves).
    pub fn chrome_switch(mut self, el: impl IntoElement) -> Self {
        self.chrome_switch = Some(el.into_any_element());
        self
    }

    /// The title, where the sheet's text starts ([`CHROME_TITLE_INSET`] past
    /// the sidebar's resting edge), in both states.
    pub fn chrome_title(mut self, el: impl IntoElement) -> Self {
        self.chrome_title = Some(el.into_any_element());
        self
    }

    /// Closed-only content in the title's slot — Browse's page controls and
    /// address at rest. Fades in once the column has closed (80 ms, 4 px
    /// rise) and leaves before it opens (60 ms).
    pub fn chrome_closed(mut self, el: impl IntoElement) -> Self {
        self.chrome_closed = Some(el.into_any_element());
        self
    }

    /// The bar's actions, ending [`CHROME_ACTIONS_PAD`] from the window's
    /// right edge in both states.
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
}

#[derive(Clone, Copy)]
enum HoverSource {
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
        let bar_h = cx.aui().metrics.header;
        let id = shell.id.clone();
        let state = window.use_keyed_state((id.clone(), "shell"), cx, |_, _| ShellState { column: if shell.sidebar_open { 1.0 } else { 0.0 }, ..Default::default() });
        let mut st = *state.read(cx);
        let now = cx.background_executor().now();
        let win_w = window.viewport_size().width;

        let lo = f32::from(shell.sidebar_min);
        let hi = f32::from(shell.sidebar_max).max(lo);
        let rest = px(f32::from(shell.sidebar_width).clamp(lo, hi));

        // The toggle's hover, from the pointer against its last rect (the
        // host's button re-renders on its own hover, which is when this runs;
        // frames keep coming while the pointer is on it, so leaving is seen).
        let mouse = window.mouse_position();
        let over_lead = st.lead_bounds.is_some_and(|b| b.contains(&mouse));
        if over_lead != st.lead {
            st.set_hover(HoverSource::Lead, over_lead, now);
            state.update(cx, |s, _| *s = st);
        }
        if st.lead {
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
            aui_motion::open_close_jump((id.clone(), "closed-only"), closed, window, cx);
        }
        let column = if shell.resizing || !shell.animate {
            if shell.sidebar_open { 1.0 } else { 0.0 }
        } else {
            open_close_with((id.clone(), "column"), shell.sidebar_open, Enter::Tween(COLUMN_OPEN, COLUMN_EASE), COLUMN_CLOSE, COLUMN_EASE, window, cx)
        };
        let sidebar_w = rest * column.clamp(0.0, 1.0);
        let settled_closed = closed && column <= 0.0005;
        let closed_only_wanted = closed && st.column <= 0.0005;
        let closed_k = if !shell.animate {
            if closed { 1.0 } else { 0.0 }
        } else {
            open_close_with((id.clone(), "closed-only"), closed_only_wanted, Enter::Tween(CLOSED_ONLY_IN, Easing::OUT), aui_motion::durations::FAST_EXIT, EXIT_EASING, window, cx)
        };
        if shell.animate && settled_closed && !closed_only_wanted {
            window.request_animation_frame();
        }

        if shell.native_lights {
            place_native_lights(window, cx);
        }

        // The peek: armed by a 150 ms rest on the toggle or the strip,
        // dismissed 250 ms after the pointer has left those and the card.
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

        // The sidebar column: the panel keeps its resting width and slides
        // with the column's edge; rows fade under the bar.
        let show_sidebar = shell.sidebar_open || column > 0.0005;
        let sidebar_column = div()
            .w(sidebar_w)
            .h_full()
            .flex_none()
            .relative()
            .overflow_hidden()
            .when(show_sidebar, |d| {
                d.child(div().absolute().top(px(0.0)).bottom(px(0.0)).left(sidebar_w - rest).w(rest).children(shell.sidebar))
                    .child(fade(p.surface_1, LIST_FADE).left(sidebar_w - rest).w(rest))
            });

        // The centre sheet, its content at the resting width.
        let mut centre_sheet = sheet_box(shell.page_sheet).child(div().absolute().top(px(0.0)).bottom(px(0.0)).left(px(0.0)).w(content_w).children(shell.centre));
        if !shell.page_sheet {
            centre_sheet = centre_sheet.child(fade(p.surface_2, SHEET_FADE));
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
                            .child(div().w_full().h(bar_h).flex_none().flex().items_center().children(shell.header_right))
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
                .top(bar_h + px(SHEET_INSET))
                .bottom(px(SHEET_INSET))
                .left(sidebar_w + px(SHEET_INSET) - px(CONTACT_W))
                .w(px(CONTACT_W))
                .opacity(column.clamp(0.0, 1.0))
                .bg(gpui::linear_gradient(90.0, gpui::linear_color_stop(gpui::black().alpha(0.0), 0.0), gpui::linear_color_stop(gpui::black().alpha(a), 1.0)))
        });

        let body = h_flex().absolute().top(bar_h).bottom(px(0.0)).left(px(0.0)).right(px(0.0)).items_stretch().child(sidebar_column).child(centre_column).child(right_column);

        let mut root = div().id(id.clone()).relative().size_full().overflow_hidden().bg(p.surface_1).text_color(p.ink).child(body).children(contact);

        // The chrome's rects, collected while it prepaints and handed to the
        // host by the last child (see `AppShell::on_overlay_bounds`).
        use gpui_kit::base::ElementExt as _;
        let rects: std::rc::Rc<std::cell::RefCell<OverlayRects>> = Default::default();

        // The peek strip and card, under the bar's controls.
        if settled_closed {
            if let Some(peek_el) = shell.peek {
                let k = open_close((id.clone(), "peek"), peeking, SpringKind::Moderate, aui_motion::durations::MODERATE_EXIT, window, cx);
                let strip_state = state.clone();
                root = root.child(
                    div()
                        .id((id.clone(), "peek-strip"))
                        .absolute()
                        .left(px(0.0))
                        .top(bar_h)
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
                    // From −108 % of its width to its resting place, 6 px in;
                    // its rows start under the bar.
                    let travel = (rest + px(SHEET_INSET)) * 1.08;
                    root = root.child(
                        div()
                            .id((id.clone(), "peek-card"))
                            .absolute()
                            .top(px(SHEET_INSET))
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
                            .child(div().absolute().top(bar_h - px(SHEET_INSET)).bottom(px(0.0)).left(px(0.0)).right(px(0.0)).child(crate::interact::AmbientScope::filled(aui_tokens::Palette::FLOATING_LEVEL, peek_el)))
                            .child(fade(p.overlay, LIST_FADE).top(bar_h - px(SHEET_INSET)))
                            .on_prepaint(move |bounds, _, _| report.borrow_mut().push((bounds, px(SHEET_RADIUS)))),
                    );
                }
            }
        }

        // The bar's controls, over everything: lights, lead, switch at the
        // sidebar's edge, the title (or the closed-only content) where the
        // sheet's text starts, the actions at the right. No ground of their
        // own, so the peek card shows through behind them.
        let lead_state = state.clone();
        let lead = div()
            .absolute()
            .left(px(CHROME_LEAD))
            .top(px(0.0))
            .h(bar_h)
            .flex()
            .items_center()
            .children(shell.chrome_lead)
            .on_prepaint(move |bounds, _, cx| {
                lead_state.update(cx, |s, _| s.lead_bounds = Some(bounds));
            });
        let switch = div().absolute().left(px(0.0)).top(px(0.0)).w(rest).h(bar_h).flex().items_center().justify_end().pr(px(CHROME_SWITCH_INSET)).children(shell.chrome_switch);
        let slot_left = rest + px(CHROME_TITLE_INSET);
        let title = div().absolute().left(slot_left).right(px(CHROME_ACTIONS_PAD + CHROME_ACTIONS_ROOM)).top(px(0.0)).h(bar_h).flex().items_center().overflow_hidden().children(shell.chrome_title);
        let rise = px(4.0 * (1.0 - closed_k));
        let closed_only = (closed_k > 0.0).then(|| div().absolute().left(slot_left).top(rise).h(bar_h).flex().items_center().opacity(closed_k).children(shell.chrome_closed));
        let actions = div().absolute().right(px(CHROME_ACTIONS_PAD)).top(px(0.0)).h(bar_h).flex().items_center().children(shell.chrome_right);
        let mut bar = div().absolute().top(px(0.0)).left(px(0.0)).right(px(0.0)).h(bar_h).child(switch).child(title).children(closed_only).child(actions).child(lead);
        if shell.traffic_lights {
            let at = super::header::traffic_light_position(cx);
            bar = bar.child(painted_lights().absolute().left(at.x).top(at.y));
        }
        let bar: AnyElement = if shell.draggable { drag_region((id.clone(), "bar-drag")).child(bar).into_any_element() } else { bar.into_any_element() };
        root = root.child(bar);

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
