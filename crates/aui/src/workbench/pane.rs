//! The right pane's chrome (D6, docs/43 §2.2 and §2.11; canvas "Baaz v3" v49,
//! boards `D6Header` and `D6States*`).
//!
//! There is no tab bar: toggles in the transcript's title row choose the
//! screen ([`pane_toggle`]). Each screen draws one 32 px row — [`pane_row`]
//! (glyph, name, the screen's own actions, ✕) or, for a view pushed on top of
//! it, [`pushed_row`] (←, title, context, ≤ 2 actions, ✕). Empty, error and
//! loading states share [`state_block`] and [`skeleton_bar`].

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, ClickEvent, ElementId, FontWeight, IntoElement, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, Button};

/// The pane row's height (the sheet's own row, on the lights' line).
pub const PANE_ROW_H: f32 = 32.0;
/// The fade under the pane row, to the sheet colour.
pub const PANE_ROW_FADE: f32 = 24.0;
/// The pane row: 12 in on the left (to the glyph), 6 on the right (to ✕).
const ROW_PAD_LEFT: f32 = 12.0;
const ROW_PAD_RIGHT: f32 = 6.0;
/// Glyph to name in the pane row.
const ROW_GAP: f32 = 8.0;
/// Between the pushed row's controls (back, title, actions, ✕).
const PUSHED_GAP: f32 = 2.0;
/// Every glyph in the row.
const GLYPH: f32 = 14.0;
/// ✕ draws its glyph at 12 in a 28 button.
const CLOSE_GLYPH: f32 = 12.0;
/// Name / title: 14.3 / 600; the pushed view's context 13.2 muted.
const NAME_TEXT: f32 = 14.3;
const CONTEXT_TEXT: f32 = 13.2;
/// The pushed row hides its context under this pane width.
pub const CONTEXT_MIN_WIDTH: f32 = 360.0;

/// One of the transcript row's pane toggles (Changes · Files · Page): a 28 px
/// ghost icon button, glyph 14 muted, hover 5 % + foreground, **lit** (the
/// screen the pane shows) 9 % + foreground; it announces itself as pressed.
pub fn pane_toggle(id: impl Into<ElementId>, glyph: IconName, label: impl Into<SharedString>, lit: bool) -> Button {
    icon_button(id, glyph).ghost().toggled(lit).accessibility_label(label)
}

/// One of a row's own actions: a 28 px ghost icon button, glyph 14.
pub fn pane_action(id: impl Into<ElementId>, glyph: IconName, label: impl Into<SharedString>) -> Button {
    icon_button(id, glyph).ghost().accessibility_label(label)
}

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

fn close_button(id: ElementId, on_close: Option<Handler>) -> Button {
    let mut b = icon_button(id, IconName::X).ghost().icon_size(px(CLOSE_GLYPH)).accessibility_label("Close the pane");
    if let Some(f) = on_close {
        b = b.on_click(move |e, w, cx| f(e, w, cx));
    }
    b
}

/// The screen's row: glyph 14 muted, name 14.3 / 600, spacer, the screen's
/// own actions, ✕. Opaque on the sheet; [`pane_row_fade`] goes under it.
#[derive(IntoElement)]
pub struct PaneRow {
    id: ElementId,
    glyph: IconName,
    name: SharedString,
    actions: Vec<AnyElement>,
    on_close: Option<Handler>,
    /// The name as a menu button (Page: "Recent pages"): open, and its click.
    title_menu: Option<(bool, Handler)>,
}

/// The pane row for a screen.
pub fn pane_row(id: impl Into<ElementId>, glyph: IconName, name: impl Into<SharedString>) -> PaneRow {
    PaneRow { id: id.into(), glyph, name: name.into(), actions: Vec::new(), on_close: None, title_menu: None }
}

impl PaneRow {
    /// One of the screen's own actions (Files: filter), before ✕.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// ✕: close the pane.
    pub fn on_close(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }

    /// The name becomes a menu button (D6 Page, owner 2026-10-18): the name
    /// and a 12 px chevron, 28 tall, r 6, hover 5 %, lit 9 % while `open`.
    pub fn title_menu(mut self, open: bool, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.title_menu = Some((open, Box::new(f)));
        self
    }
}

/// The pane row's name as a menu button ([`PaneRow::title_menu`]).
fn title_menu_button(id: ElementId, name: SharedString, open: bool, on_click: Handler, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    h_flex()
        .id(id)
        .flex_none()
        .h(px(28.0))
        .ml(px(-4.0))
        .pl(px(4.0))
        .pr(px(6.0))
        .gap(px(4.0))
        .items_center()
        .rounded(px(6.0))
        .when(open, |d| d.bg(p.selected_overlay()))
        .when(!open, |d| d.hover(|d| d.bg(p.hover_overlay())))
        .cursor_pointer()
        .role(gpui::Role::Button)
        .aria_label(SharedString::from("Recent pages"))
        .aria_expanded(open)
        .font_family(scale::FONT_UI)
        .text_px(NAME_TEXT)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.fg)
        .whitespace_nowrap()
        .child(name)
        .child(icon(IconName::ChevronDown).size(px(12.0)).color(p.fg_muted))
        .on_click(move |e, w, cx| on_click(e, w, cx))
}

impl RenderOnce for PaneRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        h_flex()
            .id(self.id.clone())
            .w_full()
            .h(px(PANE_ROW_H))
            .flex_none()
            .pl(px(ROW_PAD_LEFT))
            .pr(px(ROW_PAD_RIGHT))
            .gap(px(ROW_GAP))
            .items_center()
            .bg(p.surface_2)
            .child(icon(self.glyph).size(px(GLYPH)).color(p.fg_muted))
            .child(match self.title_menu {
                Some((open, on_click)) => h_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(title_menu_button((self.id.clone(), "title").into(), self.name, open, on_click, cx))
                    .into_any_element(),
                None => div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font_family(scale::FONT_UI)
                    .text_px(NAME_TEXT)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.fg)
                    .child(self.name)
                    .into_any_element(),
            })
            .child(h_flex().flex_none().gap(px(PUSHED_GAP)).items_center().children(self.actions))
            .child(close_button((self.id, "close").into(), self.on_close))
    }
}

/// The row of a view pushed on a screen (file preview, Create PR, full
/// output, image): ← (label "Back to …"), title 14.3 / 600, context 13.2
/// muted (hidden under [`CONTEXT_MIN_WIDTH`]), ≤ 2 actions, ✕.
#[derive(IntoElement)]
pub struct PushedRow {
    id: ElementId,
    title: AnyElement,
    context: Option<SharedString>,
    back_label: SharedString,
    actions: Vec<AnyElement>,
    width: Option<Pixels>,
    on_back: Option<Handler>,
    on_close: Option<Handler>,
}

/// A pushed view's row titled with plain text.
pub fn pushed_row(id: impl Into<ElementId>, title: impl Into<SharedString>) -> PushedRow {
    let title: SharedString = title.into();
    pushed_row_with(id, div().font_weight(FontWeight::SEMIBOLD).child(title))
}

/// A pushed view's row whose title is caller-built (the full output's
/// command in mono with its exit code and time).
pub fn pushed_row_with(id: impl Into<ElementId>, title: impl IntoElement) -> PushedRow {
    PushedRow {
        id: id.into(),
        title: title.into_any_element(),
        context: None,
        back_label: "Back".into(),
        actions: Vec::new(),
        width: None,
        on_back: None,
        on_close: None,
    }
}

impl PushedRow {
    /// The muted context after the title (a file's folder).
    pub fn context(mut self, context: impl Into<SharedString>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// ←'s accessible name: "Back to files", "Back to changes".
    pub fn back_label(mut self, label: impl Into<SharedString>) -> Self {
        self.back_label = label.into();
        self
    }

    /// One of at most two actions before ✕.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        debug_assert!(self.actions.len() < 2, "a pushed view's row holds at most two actions");
        self.actions.push(action.into_any_element());
        self
    }

    /// The pane's width, so the context can leave under 360.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// ←: pop the view.
    pub fn on_back(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Box::new(f));
        self
    }

    /// ✕: close the pane.
    pub fn on_close(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }
}

impl RenderOnce for PushedRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let show_context = self.width.is_none_or(|w| f32::from(w) >= CONTEXT_MIN_WIDTH);
        let mut back = icon_button((self.id.clone(), "back"), IconName::ChevronLeft).ghost().accessibility_label(self.back_label);
        if let Some(f) = self.on_back {
            back = back.on_click(move |e, w, cx| f(e, w, cx));
        }
        let mut title = h_flex()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .pl(px(2.0))
            .gap(px(8.0))
            .items_baseline()
            .whitespace_nowrap()
            .font_family(scale::FONT_UI)
            .text_px(NAME_TEXT)
            .text_color(p.fg)
            .child(div().flex_none().max_w_full().overflow_hidden().text_ellipsis().child(self.title));
        if let Some(context) = self.context.filter(|_| show_context) {
            title = title.child(div().min_w(px(0.0)).overflow_hidden().text_ellipsis().text_px(CONTEXT_TEXT).text_color(p.fg_muted).child(context));
        }
        h_flex()
            .id(self.id.clone())
            .w_full()
            .h(px(PANE_ROW_H))
            .flex_none()
            .px(px(ROW_PAD_RIGHT))
            .gap(px(PUSHED_GAP))
            .items_center()
            .bg(p.surface_2)
            .child(back)
            .child(title)
            .children(self.actions)
            .child(close_button((self.id, "close").into(), self.on_close))
    }
}

/// The 24 px fade from the sheet colour under a pane row; place it absolutely
/// at the row's foot, over the body.
pub fn pane_row_fade(cx: &App) -> gpui::Div {
    let ground = cx.aui().colors.surface_2;
    div()
        .absolute()
        .top(px(PANE_ROW_H))
        .left(px(0.0))
        .right(px(0.0))
        .h(px(PANE_ROW_FADE))
        .bg(gpui::linear_gradient(180.0, gpui::linear_color_stop(ground, 0.0), gpui::linear_color_stop(ground.alpha(0.0), 1.0)))
}

/// An empty / error / not-shown state (docs/43 §2.11): centred, padding 0 32,
/// gap 6 — a 40 px Raised tile r 10 with the edge and the glyph 18 muted (6
/// more below it), the title 600, a line 13.2 / 18 muted (max 260), then at
/// most one secondary action 8 below.
#[derive(IntoElement)]
pub struct StateBlock {
    id: ElementId,
    glyph: IconName,
    title: SharedString,
    line: Option<AnyElement>,
    action: Option<AnyElement>,
}

/// A state block.
pub fn state_block(id: impl Into<ElementId>, glyph: IconName, title: impl Into<SharedString>) -> StateBlock {
    StateBlock { id: id.into(), glyph, title: title.into(), line: None, action: None }
}

impl StateBlock {
    /// The muted line under the title.
    pub fn line(mut self, line: impl Into<SharedString>) -> Self {
        let line: SharedString = line.into();
        self.line = Some(line.into_any_element());
        self
    }

    /// A caller-built line (a path in mono inside the sentence).
    pub fn line_with(mut self, line: impl IntoElement) -> Self {
        self.line = Some(line.into_any_element());
        self
    }

    /// The one secondary action (see [`secondary_action`]).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for StateBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let tile = div()
            .size(px(40.0))
            .mb(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(10.0))
            .bg(p.surface_3)
            .shadow(p.edge())
            .child(icon(self.glyph).size(px(18.0)).color(p.fg_muted));
        let mut col = v_flex()
            .id(self.id)
            .role(gpui::Role::Group)
            .aria_label(self.title.clone())
            .size_full()
            .px(px(32.0))
            .gap(px(6.0))
            .items_center()
            .justify_center()
            .font_family(scale::FONT_UI)
            .child(tile)
            .child(div().text_px(NAME_TEXT).font_weight(FontWeight::SEMIBOLD).text_color(p.fg).text_center().child(self.title));
        if let Some(line) = self.line {
            col = col.child(div().max_w(px(260.0)).text_px(CONTEXT_TEXT).line_height(px(18.0)).text_color(p.fg_muted).text_center().child(line));
        }
        if let Some(action) = self.action {
            col = col.child(div().mt(px(8.0)).child(action));
        }
        col
    }
}

/// The pane's secondary button: 28 tall, r 6, Raised-lit with the edge,
/// 14.3 foreground, an optional leading glyph 14 ("Open in default app",
/// "Try again").
pub fn secondary_action(id: impl Into<ElementId>, label: impl Into<SharedString>, glyph: Option<IconName>, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let label: SharedString = label.into();
    h_flex()
        .id(id.into())
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .cursor_pointer()
        .h(px(28.0))
        .px(px(12.0))
        .gap(px(6.0))
        .items_center()
        .rounded(px(6.0))
        .bg(p.surface_4)
        .shadow(p.edge())
        .font_family(scale::FONT_UI)
        .text_px(NAME_TEXT)
        .text_color(p.fg)
        .whitespace_nowrap()
        .hover(|s| s.bg(p.surface_4.blend(p.hover_overlay())))
        .when_some(glyph, |d, g| d.child(icon(g).size(px(GLYPH)).color(p.fg_muted)))
        .child(label)
}

/// A skeleton bar (docs/43 §2.11): 10 tall, r 5, at the selected overlay.
pub fn skeleton_bar(width: Pixels, cx: &App) -> gpui::Div {
    div().h(px(10.0)).w(width).flex_none().rounded(px(5.0)).bg(cx.aui().colors.selected_overlay())
}

/// Lines of a loading tree or list: a 14 px glyph block and a bar per row, at
/// the given depths and widths, rows 28 apart (Files · loading).
pub fn skeleton_rows(rows: &[(usize, f32)], cx: &App) -> gpui::Div {
    let sel = cx.aui().colors.selected_overlay();
    v_flex().w_full().pt(px(4.0)).px(px(6.0)).children(rows.iter().map(|&(depth, w)| {
        h_flex()
            .h(px(28.0))
            .pl(px(2.0 + 16.0 + 16.0 * depth as f32))
            .gap(px(6.0))
            .items_center()
            .child(div().size(px(14.0)).rounded(px(3.0)).bg(sel))
            .child(skeleton_bar(px(w), cx))
    }))
}

/// The gap between screens on the strip (the frame colour shows in it while
/// the strip moves).
pub const STRIP_GAP: f32 = 6.0;
/// The slide: 240 ms ease-out (docs/43 §3).
pub const STRIP_SLIDE: std::time::Duration = std::time::Duration::from_millis(240);

/// Where the strip is, in screens (0.0 = the first screen), sliding toward
/// `index` on the 240 ms ease-out; reduced motion jumps. Read it before
/// building the screens so a screen that cannot move (a native page) can draw
/// its still picture while `strip_moving` says the strip is between screens.
pub fn strip_position(id: impl Into<ElementId>, index: usize, window: &mut Window, cx: &mut App) -> f32 {
    let id: ElementId = id.into();
    aui_motion::tween((id, "strip-pos"), index as f32, aui_motion::Tween::new(STRIP_SLIDE, aui_motion::Easing::OUT), window, cx)
}

/// The strip is between screens.
pub fn strip_moving(position: f32, index: usize) -> bool {
    (position - index as f32).abs() > 0.001
}

/// Changes · Files · Page as screens side by side on one strip, 6 apart; the
/// pane is a window onto the one at `position` (from [`strip_position`]).
/// Each screen is a sheet of its own (rounded, the sheet colour) and carries
/// its own row; screens out of view are not built (pass `None`), so each one
/// keeps its scroll and selection in the host's state, not in the element.
#[derive(IntoElement)]
pub struct PaneStrip {
    id: ElementId,
    position: f32,
    width: Pixels,
    screens: Vec<Option<AnyElement>>,
    on_swipe: Option<SwipeHandler>,
}

/// A swipe's handler: `+1` next screen, `-1` previous.
type SwipeHandler = Box<dyn Fn(i32, &mut Window, &mut App) + 'static>;

/// How far a two-finger sideways swipe travels before it moves one screen.
const SWIPE_DISTANCE: f32 = 48.0;

/// One gesture's travel; `fired` once it has moved a screen, so the rest of
/// the gesture (and its momentum) moves nothing more.
#[derive(Default)]
struct Swipe {
    travel: f32,
    fired: bool,
    /// The last wheel event: a pause longer than [`SWIPE_GAP`] starts a new
    /// gesture, for a wheel that never reports touch phases (a mouse).
    last: Option<std::time::Instant>,
}

/// A pause between wheel events that ends a swipe gesture.
const SWIPE_GAP: std::time::Duration = std::time::Duration::from_millis(250);

/// A strip of screens. `width` is the pane's (the sheet's) width.
pub fn pane_strip(id: impl Into<ElementId>, position: f32, width: Pixels, screens: Vec<Option<AnyElement>>) -> PaneStrip {
    PaneStrip { id: id.into(), position, width, screens, on_swipe: None }
}

impl PaneStrip {
    /// A two-finger sideways swipe in the pane: `+1` toward the next screen,
    /// `-1` toward the previous — once per gesture (snapping, one screen per
    /// swipe). Vertical scrolling passes through untouched.
    pub fn on_swipe(mut self, f: impl Fn(i32, &mut Window, &mut App) + 'static) -> Self {
        self.on_swipe = Some(Box::new(f));
        self
    }
}

/// The screens a strip at `position` shows: the one it rests on, or the two
/// it is between.
pub fn strip_visible(position: f32, count: usize) -> impl Fn(usize) -> bool {
    move |i: usize| i < count && (i as f32 - position).abs() < 1.0
}

impl RenderOnce for PaneStrip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let step = f32::from(self.width) + STRIP_GAP;
        let swipe = window.use_keyed_state((self.id.clone(), "swipe"), cx, |_, _| Swipe::default());
        let moving = self.screens.iter().enumerate().any(|(i, s)| s.is_some() && (i as f32 - self.position).abs() > 0.001);
        div()
            .id(self.id)
            .relative()
            .size_full()
            .overflow_hidden()
            .rounded(px(8.0))
            .when(moving, |d| d.bg(p.surface_1))
            .when_some(self.on_swipe, |d, on_swipe| {
                d.on_scroll_wheel(move |e, window, cx| {
                    use gpui::TouchPhase;
                    let delta = match e.delta {
                        gpui::ScrollDelta::Pixels(d) => gpui::point(f32::from(d.x), f32::from(d.y)),
                        gpui::ScrollDelta::Lines(d) => gpui::point(d.x * 20.0, d.y * 20.0),
                    };
                    let fire = swipe.update(cx, |s, _| {
                        let now = std::time::Instant::now();
                        let paused = s.last.is_none_or(|t| now.duration_since(t) > SWIPE_GAP);
                        if e.touch_phase == TouchPhase::Started || paused {
                            *s = Swipe::default();
                        }
                        s.last = Some(now);
                        // Sideways only: a mostly vertical gesture scrolls the screen.
                        if s.fired || delta.x.abs() <= 2.0 * delta.y.abs() {
                            return None;
                        }
                        s.travel += delta.x;
                        if s.travel.abs() < SWIPE_DISTANCE {
                            return None;
                        }
                        s.fired = true;
                        // Fingers moving left bring the next screen in.
                        Some(if s.travel < 0.0 { 1 } else { -1 })
                    });
                    if let Some(dir) = fire {
                        on_swipe(dir, window, cx);
                    }
                })
            })
            .children(self.screens.into_iter().enumerate().filter_map(|(i, s)| {
                let s = s?;
                let left = (i as f32 - self.position) * step;
                Some(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(left))
                        .w(self.width)
                        .rounded(px(8.0))
                        .overflow_hidden()
                        .bg(p.surface_2)
                        .child(s),
                )
            }))
    }
}

/// A focused field's edge (docs/43 §2.4, §2.6): dark lifts the edge's ring to
/// white 22 % under the top highlight; light draws one black ring at 28 %.
pub fn focus_edge(cx: &App) -> Vec<gpui::BoxShadow> {
    let p = cx.aui().colors;
    let ring = |color: gpui::Hsla| gpui::BoxShadow {
        color,
        offset: gpui::point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(1.0),
        inset: true,
    };
    if p.is_dark() {
        vec![
            gpui::BoxShadow { color: gpui::white().alpha(0.07), offset: gpui::point(px(0.0), px(1.0)), blur_radius: px(0.0), spread_radius: px(0.0), inset: true },
            ring(gpui::white().alpha(0.22)),
        ]
    } else {
        vec![ring(gpui::black().alpha(0.28))]
    }
}

/// Files' filter (docs/43 §2.4): a 32 px Well field under the pane row (inset
/// 6 above, 8 at the sides, 2 below), the edge (22 % while focused), the
/// search glyph 14 muted, the query, and `3 of 1,204` in faint mono 12.1 at
/// the right.
pub fn filter_field(
    id: impl Into<ElementId>,
    state: &gpui::Entity<gpui_kit::base::input::InputState>,
    count: Option<SharedString>,
    window: &Window,
    cx: &App,
) -> gpui::Div {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let focused = gpui::Focusable::focus_handle(state.read(cx), cx).is_focused(window);
    div().w_full().flex_none().pt(px(6.0)).px(px(8.0)).pb(px(2.0)).child(
        h_flex()
            .h(px(32.0))
            .px(px(10.0))
            .gap(px(8.0))
            .items_center()
            .rounded(px(6.0))
            .bg(p.well)
            .shadow(if focused { focus_edge(cx) } else { p.edge() })
            .font_family(scale::FONT_UI)
            .text_px(NAME_TEXT)
            .text_color(p.fg)
            .child(icon(IconName::Search).size(px(GLYPH)).color(p.fg_muted))
            .child(crate::a11y_text::a11y_text_input(id, "Filter files", state, cx).flex_1().min_w(px(0.0)))
            .when_some(count, |d, c| d.child(div().flex_none().font_family(scale::FONT_MONO).text_px(12.1).text_color(p.ink_4).child(c))),
    )
}
