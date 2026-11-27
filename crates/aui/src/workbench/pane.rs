//! The right pane's chrome (D6, docs/43 §2.2 and §2.11; flat since docs/83
//! §3 and §6).
//!
//! Each screen draws its own 44 px row ([`pane_subheader`], on the column
//! header's `line-region` so its line meets the centre's): a view pushed on
//! a screen draws [`pushed_row`] there (←, its title, ≤ 2 actions). A host
//! whose centre header's view icons ([`pane_toggle`]) pick the screen (Baaz's
//! Sessions, owner 2026-11-27) shows no tab row: the screen's row is the
//! pane's title bar, ending in the pane's own controls ([`pane_controls`]:
//! `＋`, maximise, ✕). A host without them seats a tab row ([`pane_tabs`])
//! in the column's 44 px header — one tab per view, the active one an
//! `ov-selected` pill, then the same controls — over the screens' rows.
//! Empty, error and loading states share [`state_block`] and
//! [`skeleton_bar`].

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, Bounds, ClickEvent, ElementId, FontWeight, IntoElement, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex, ElementExt as _};

use crate::interact::paint::{PaintStates, StatePaint};
use crate::data::{icon_button, Button};

/// A screen's own row, its title bar (owner 2026-11-27): the column
/// header's 44 px with the header's `line-region` under it, so in Sessions —
/// where the pane has no tab row and its screens start at the column's top
/// — the row's line meets the centre header's at one y, and the pane reads
/// full bleed. The file preview's path, a pushed view's row, Files' filter,
/// the Page's nav row and the terminal's tabs are built on it; the host adds
/// the pane's own controls ([`pane_controls`]) at its right end.
pub fn pane_subheader(cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    h_flex().w_full().h(cx.aui().metrics.header).flex_none().items_center().bg(p.surface_1).border_b_1().border_color(p.line_region)
}
/// The tab row's inset from the column's edges, and between its controls.
const TABS_PAD_X: f32 = 6.0;
const TABS_GAP: f32 = 2.0;
/// A tab: 28 tall (the dense control), label padded 10 a side.
const TAB_PAD_X: f32 = 10.0;
/// A closable tab's ✕: 16 square, glyph 10, 4 after the label.
const TAB_CLOSE: f32 = 16.0;
const TAB_CLOSE_GLYPH: f32 = 10.0;
/// Every glyph in a row.
const GLYPH: f32 = 14.0;
/// ✕ draws its glyph at 12 in a 28 button.
const CLOSE_GLYPH: f32 = 12.0;
/// The pushed row's padding and the gap between its controls.
const PUSHED_PAD: f32 = 6.0;
const PUSHED_GAP: f32 = 2.0;

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

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type TabHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;
type BoundsHandler = Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static>;

/// One tab of the pane's tab row.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneTab {
    /// Reported by [`PaneTabs::on_select`] and [`PaneTabs::on_close_tab`].
    pub id: SharedString,
    /// The visible label (`Changes`, `Files`, `Page`, a file's name).
    pub label: SharedString,
    /// The view the pane shows.
    pub active: bool,
    /// Carries its own ✕ (an open file's tab).
    pub closable: bool,
}

impl PaneTab {
    /// A tab that cannot be closed (the fixed views).
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, active: bool) -> Self {
        Self { id: id.into(), label: label.into(), active, closable: false }
    }

    /// Gives the tab its own ✕.
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
}

/// The pane's tab row (docs/83 §6), drawn inside the column's 44 px header:
/// the tabs (`t-body`; the active one an `ov-selected` pill, radius 8, in
/// `fg`; the rest `fg-muted`), `＋`, then at the right maximise and ✕. Each
/// control is drawn only when the host wires it.
#[derive(IntoElement)]
pub struct PaneTabs {
    id: ElementId,
    tabs: Vec<PaneTab>,
    on_select: Option<TabHandler>,
    on_close_tab: Option<TabHandler>,
    add: Option<(bool, Handler)>,
    add_bounds: Option<BoundsHandler>,
    maximise: Option<(bool, Handler)>,
    on_close: Option<Handler>,
}

/// The tab row over `tabs`.
pub fn pane_tabs(id: impl Into<ElementId>, tabs: Vec<PaneTab>) -> PaneTabs {
    PaneTabs { id: id.into(), tabs, on_select: None, on_close_tab: None, add: None, add_bounds: None, maximise: None, on_close: None }
}

impl PaneTabs {
    /// A tab was clicked (its id).
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// A closable tab's ✕ was clicked (its id).
    pub fn on_close_tab(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_close_tab = Some(Rc::new(f));
        self
    }

    /// `＋`: opens the host's view menu; `open` holds it lit while the menu
    /// is up.
    pub fn on_add(mut self, open: bool, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.add = Some((open, Rc::new(f)));
        self
    }

    /// Where `＋` was laid out this frame, in window coordinates: the seat
    /// the host anchors its menu to.
    pub fn on_add_bounds(mut self, f: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static) -> Self {
        self.add_bounds = Some(Rc::new(f));
        self
    }

    /// Maximise: `maximised` flips the glyph and the label to Restore.
    pub fn on_maximise(mut self, maximised: bool, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.maximise = Some((maximised, Rc::new(f)));
        self
    }

    /// ✕: close the pane.
    pub fn on_close(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for PaneTabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut tabs = h_flex().id((id.clone(), "tabs")).flex_none().min_w(px(0.0)).gap(px(TABS_GAP)).items_center().role(gpui::Role::TabList).aria_label("Right pane views");
        for tab in self.tabs {
            let tab_id: ElementId = (id.clone(), SharedString::from(format!("tab-{}", tab.id))).into();
            let mut el = h_flex()
                .id(tab_id.clone())
                .h(px(scale::CTL_COMPACT))
                .flex_none()
                .px(px(TAB_PAD_X))
                .gap(px(4.0))
                .items_center()
                .rounded(px(scale::R_8))
                .paint_states(StatePaint::new().selected(tab.active).ink(if tab.active { p.fg } else { p.fg_muted }, p.fg), &p)
                .text_role(TextRole::Body)
                .whitespace_nowrap()
                .cursor_pointer()
                .role(gpui::Role::Tab)
                .aria_label(tab.label.clone())
                .aria_selected(tab.active)
                .child(tab.label.clone());
            if let Some(f) = self.on_select.clone() {
                let tab_key = tab.id.clone();
                el = el.on_click(move |_, w, cx| f(&tab_key, w, cx));
            }
            if tab.closable {
                el = el.pr(px(4.0));
                let mut close = icon_button((tab_id, "close"), IconName::X)
                    .ghost()
                    .icon_size(px(TAB_CLOSE_GLYPH))
                    .no_tooltip()
                    .accessibility_label(SharedString::from(format!("Close {}", tab.label)));
                if let Some(f) = self.on_close_tab.clone() {
                    let tab_key = tab.id.clone();
                    close = close.on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        f(&tab_key, w, cx)
                    });
                }
                el = el.child(div().size(px(TAB_CLOSE)).flex_none().flex().items_center().justify_center().child(close));
            }
            tabs = tabs.child(el);
        }
        // `＋` stays beside the tabs; maximise and ✕ at the row's end.
        let add = self.add.map(|(open, f)| {
            let button = icon_button((id.clone(), "add"), IconName::Plus).ghost().menu_open(open).accessibility_label("Open a view").on_click(move |e, w, cx| f(e, w, cx));
            match self.add_bounds {
                Some(seat) => div().flex_none().child(button).on_prepaint(move |bounds, w, cx| seat(bounds, w, cx)).into_any_element(),
                None => button.into_any_element(),
            }
        });
        let controls = PaneControls { id: (id.clone(), "controls").into(), add: None, add_bounds: None, maximise: self.maximise, on_close: self.on_close, divider: false };
        h_flex()
            .id(id)
            .size_full()
            .min_w(px(0.0))
            .px(px(TABS_PAD_X))
            .gap(px(TABS_GAP))
            .items_center()
            .overflow_hidden()
            .child(tabs)
            .children(add)
            .child(div().flex_1().min_w(px(0.0)))
            .child(controls)
    }
}

/// The pane's own controls at a screen row's right end (owner 2026-11-27:
/// the tab row is gone in Sessions, the centre header's toggles pick the
/// screen): `＋` (the host's view menu), maximise / restore, and ✕ — after
/// a 1 × 16 hairline that sets them apart from the screen's own actions.
#[derive(IntoElement)]
pub struct PaneControls {
    id: ElementId,
    add: Option<(bool, Handler)>,
    add_bounds: Option<BoundsHandler>,
    maximise: Option<(bool, Handler)>,
    on_close: Option<Handler>,
    divider: bool,
}

/// About how wide [`pane_controls`] stand: the hairline and its margins (9),
/// three 28 px buttons, the gaps between. A row that ends in them has that
/// much less room for its own contents (the Page's nav row narrows sooner).
pub const PANE_CONTROLS_W: f32 = 100.0;

/// The pane's controls (see [`PaneControls`]).
pub fn pane_controls(id: impl Into<ElementId>) -> PaneControls {
    PaneControls { id: id.into(), add: None, add_bounds: None, maximise: None, on_close: None, divider: true }
}

impl PaneControls {
    /// `＋`: opens the host's view menu; `open` holds it lit while the menu
    /// is up.
    pub fn on_add(mut self, open: bool, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.add = Some((open, Rc::new(f)));
        self
    }

    /// Where `＋` was laid out this frame, in window coordinates: the seat
    /// the host anchors its menu to.
    pub fn on_add_bounds(mut self, f: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static) -> Self {
        self.add_bounds = Some(Rc::new(f));
        self
    }

    /// Maximise: `maximised` flips the glyph and the label to Restore.
    pub fn on_maximise(mut self, maximised: bool, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.maximise = Some((maximised, Rc::new(f)));
        self
    }

    /// ✕: close the pane.
    pub fn on_close(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }

    /// Whether the hairline stands before the controls (default true; the
    /// tab row, which has nothing of the screen's before them, has none).
    pub fn divider(mut self, divider: bool) -> Self {
        self.divider = divider;
        self
    }
}

impl RenderOnce for PaneControls {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id;
        let add = self.add.map(|(open, f)| {
            let button = icon_button((id.clone(), "add"), IconName::Plus).ghost().menu_open(open).accessibility_label("Open a view").on_click(move |e, w, cx| f(e, w, cx));
            match self.add_bounds {
                Some(seat) => div().flex_none().child(button).on_prepaint(move |bounds, w, cx| seat(bounds, w, cx)).into_any_element(),
                None => button.into_any_element(),
            }
        });
        let maximise = self.maximise.map(|(maximised, f)| {
            let (glyph, label) = if maximised { (IconName::Minimize, "Restore the pane") } else { (IconName::Maximize, "Maximise the pane") };
            icon_button((id.clone(), "maximise"), glyph).ghost().icon_size(px(GLYPH)).accessibility_label(label).on_click(move |e, w, cx| f(e, w, cx))
        });
        let mut close = icon_button((id.clone(), "close"), IconName::X).ghost().icon_size(px(CLOSE_GLYPH)).accessibility_label("Close the pane");
        if let Some(f) = self.on_close {
            close = close.on_click(move |e, w, cx| f(e, w, cx));
        }
        h_flex()
            .id(id)
            .flex_none()
            .gap(px(TABS_GAP))
            .items_center()
            .when(self.divider, |d| d.child(div().w(px(1.0)).h(px(16.0)).mx(px(4.0)).flex_none().bg(p.line)))
            .children(add)
            .children(maximise)
            .child(close)
    }
}

/// The sub-header of a view pushed on a screen (file preview, Create PR,
/// full output, image): ← (label "Back to …") when the host wires it, the
/// title (`t-caption`, `fg-muted`: a file's path), ≤ 2 actions at the right.
/// As the pane's title bar it ends in the pane's controls
/// ([`PushedRow::trailing`]); under a tab row ✕ lives there.
#[derive(IntoElement)]
pub struct PushedRow {
    id: ElementId,
    title: AnyElement,
    back_label: SharedString,
    actions: Vec<AnyElement>,
    trailing: Option<AnyElement>,
    on_back: Option<Handler>,
}

/// A pushed view's sub-header titled with plain text.
pub fn pushed_row(id: impl Into<ElementId>, title: impl Into<SharedString>) -> PushedRow {
    let title: SharedString = title.into();
    pushed_row_with(id, div().child(title))
}

/// A pushed view's sub-header whose title is caller-built (the full
/// output's command in mono with its exit code and time).
pub fn pushed_row_with(id: impl Into<ElementId>, title: impl IntoElement) -> PushedRow {
    PushedRow { id: id.into(), title: title.into_any_element(), back_label: "Back".into(), actions: Vec::new(), trailing: None, on_back: None }
}

impl PushedRow {
    /// ←'s accessible name: "Back to files", "Back to changes".
    pub fn back_label(mut self, label: impl Into<SharedString>) -> Self {
        self.back_label = label.into();
        self
    }

    /// What stands after the actions at the row's end: the pane's own
    /// controls ([`pane_controls`]) when the row is the pane's title bar.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// One of at most two actions at the right.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        debug_assert!(self.actions.len() < 2, "a pushed view's row holds at most two actions");
        self.actions.push(action.into_any_element());
        self
    }

    /// ←: pop the view. Without it the row has no ← (the file preview: its
    /// way back is the Files tab).
    pub fn on_back(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for PushedRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let back = self.on_back.map(|f| icon_button((self.id.clone(), "back"), IconName::ChevronLeft).ghost().accessibility_label(self.back_label).on_click(move |e, w, cx| f(e, w, cx)));
        let has_back = back.is_some();
        let title = div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .when(!has_back, |d| d.pl(px(TAB_PAD_X - PUSHED_PAD)))
            .text_role(TextRole::Caption)
            .text_color(p.fg_muted)
            .child(self.title);
        pane_subheader(cx)
            .id(self.id)
            .px(px(PUSHED_PAD))
            .gap(px(PUSHED_GAP))
            .children(back)
            .child(title)
            .children(self.actions)
            .children(self.trailing)
    }
}

/// An empty / error / not-shown state (docs/43 §2.11): centred, padding 0 32,
/// gap 6 — a 40 px Raised tile r 10 with a `line` border and the glyph 18 muted (6
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
            .border_1()
            .border_color(p.line)
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
            .child(div().role_size(TextRole::Body).font_weight(FontWeight::SEMIBOLD).text_color(p.fg).text_center().child(self.title));
        if let Some(line) = self.line {
            col = col.child(div().max_w(px(260.0)).role_size(TextRole::Body).line_height(px(18.0)).text_color(p.fg_muted).text_center().child(line));
        }
        if let Some(action) = self.action {
            col = col.child(div().mt(px(8.0)).child(action));
        }
        col
    }
}

/// The pane's secondary button: 28 tall, r 6, Raised-lit with a `line` border,
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
        .border_1()
        .border_color(p.line)
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .text_color(p.fg)
        .whitespace_nowrap()
        .paint_states(StatePaint::new().on(p.surface_4), &p)
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

/// The gap between screens on the strip (the column ground shows in it while
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
/// Each screen sits square on the column's ground and carries its own row;
/// screens out
/// of view are not built (pass `None`), so each one keeps its scroll and
/// selection in the host's state, not in the element.
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

/// How far a Shift + sideways swipe travels before it moves one screen.
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

/// One wheel event of a Shift-swipe: the screen step it fires, once per
/// gesture (a pause longer than [`SWIPE_GAP`] or a new touch starts the next).
fn swipe_step(swipe: &gpui::Entity<Swipe>, phase: gpui::TouchPhase, dx: f32, cx: &mut App) -> Option<i32> {
    swipe.update(cx, |s, _| {
        let now = std::time::Instant::now();
        let paused = s.last.is_none_or(|t| now.duration_since(t) > SWIPE_GAP);
        if phase == gpui::TouchPhase::Started || paused {
            *s = Swipe::default();
        }
        s.last = Some(now);
        if s.fired {
            return None;
        }
        s.travel += dx;
        if s.travel.abs() < SWIPE_DISTANCE {
            return None;
        }
        s.fired = true;
        // Fingers moving left bring the next screen in.
        Some(if s.travel < 0.0 { 1 } else { -1 })
    })
}

/// A strip of screens. `width` is the pane's (the sheet's) width.
pub fn pane_strip(id: impl Into<ElementId>, position: f32, width: Pixels, screens: Vec<Option<AnyElement>>) -> PaneStrip {
    PaneStrip { id: id.into(), position, width, screens, on_swipe: None }
}

impl PaneStrip {
    /// Shift + a two-finger sideways swipe in the pane: `+1` toward the next
    /// screen, `-1` toward the previous — once per gesture (snapping, one
    /// screen per swipe), and the content under it does not scroll. Without
    /// Shift every swipe scrolls the content and never switches screens.
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
        // Flat (docs/83 §3): the screens sit square on the column's ground,
        // with the gap between two in motion showing that same ground.
        div()
            .id(self.id)
            .relative()
            .size_full()
            .overflow_hidden()
            // Owner, 2026-10-21: a plain sideways swipe scrolls what is under
            // the pointer (a long code line, a diff, an image at 100 %) and
            // never switches screens. Shift + a sideways swipe moves one
            // screen, and only that: the listener runs in the capture phase
            // and stops the event, so the content under it does not scroll.
            .when_some(self.on_swipe, |d, on_swipe| {
                let on_swipe = std::rc::Rc::new(on_swipe);
                d.child(
                    gpui::canvas(
                        |bounds, _, _| bounds,
                        move |bounds, _, window, _| {
                            let on_swipe = on_swipe.clone();
                            let swipe = swipe.clone();
                            window.on_mouse_event(move |e: &gpui::ScrollEvent, phase, window, cx| {
                                if phase != gpui::DispatchPhase::Capture || !e.modifiers.shift || !bounds.contains(&e.position) {
                                    return;
                                }
                                let delta = match e.delta {
                                    gpui::ScrollDelta::Pixels(d) => gpui::point(f32::from(d.x), f32::from(d.y)),
                                    gpui::ScrollDelta::Lines(d) => gpui::point(d.x * 20.0, d.y * 20.0),
                                };
                                // Sideways only; Shift + a vertical swipe scrolls as usual.
                                if delta.x.abs() <= 2.0 * delta.y.abs() {
                                    return;
                                }
                                cx.stop_propagation();
                                if let Some(dir) = swipe_step(&swipe, e.touch_phase, delta.x, cx) {
                                    on_swipe(dir, window, cx);
                                }
                            });
                        },
                    )
                    .absolute()
                    .size_full(),
                )
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
                        .overflow_hidden()
                        .bg(p.surface_1)
                        .child(s),
                )
            }))
    }
}

/// A field's 1 px border (docs/83 §2.1): `line-strong` at rest, the focus
/// colour (`accent-ring`) while focused. Inputs keep a border in the flat
/// design; the old inset edge and its top highlight are gone.
pub fn field_border(focused: bool, cx: &App) -> gpui::Hsla {
    let p = cx.aui().colors;
    if focused { p.accent_ring } else { p.line_strong }
}

/// Files' filter (owner 2026-10-21, canvas `D6OneBar`; docs/83 §6): while
/// filtering, Files' sub-header IS the field — the Well (28, r 6, a
/// [`field_border`]; the focus colour while focused), search glyph 12, the
/// query, the match count faint mono at the right — then a 1 × 16 hairline
/// and ✕, which closes the filter (as Esc does).
pub fn filter_bar(
    id: impl Into<ElementId>,
    state: &gpui::Entity<gpui_kit::base::input::InputState>,
    count: Option<SharedString>,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    window: &Window,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let focused = gpui::Focusable::focus_handle(state.read(cx), cx).is_focused(window);
    pane_subheader(cx)
        .id(id.clone())
        .px(px(6.0))
        .gap(px(6.0))
        .child(
            h_flex()
                .flex_1()
                .min_w(px(0.0))
                .h(px(28.0))
                .px(px(8.0))
                .gap(px(8.0))
                .items_center()
                .rounded(px(6.0))
                .bg(p.well)
                .border_1()
                .border_color(field_border(focused, cx))
                .font_family(scale::FONT_UI)
                .role_size(TextRole::Body)
                .text_color(p.fg)
                .child(icon(IconName::Search).size(px(12.0)).color(p.fg_muted))
                .child(crate::a11y_text::a11y_text_input((id.clone(), "input"), "Filter files", state, cx).flex_1().min_w(px(0.0)))
                .when_some(count, |d, c| d.child(div().flex_none().font_family(scale::FONT_MONO).role_size(TextRole::Caption).text_color(p.ink_4).child(c))),
        )
        .child(div().w(px(1.0)).h(px(16.0)).mx(px(4.0)).flex_none().bg(p.line))
        .child(icon_button((id, "close"), IconName::X).ghost().icon_size(px(12.0)).accessibility_label("Close the filter").on_click(move |e, w, cx| on_close(e, w, cx)))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{point, px, Modifiers, Render, ScrollDelta, ScrollEvent, TouchPhase};

    use super::*;

    /// A strip of two screens; the first counts the wheel events that reach
    /// its content, the strip counts the screen steps it fires.
    struct StripHost {
        steps: Rc<Cell<i32>>,
        content: Rc<Cell<u32>>,
    }

    impl Render for StripHost {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let (steps, content) = (self.steps.clone(), self.content.clone());
            let screen = div().id("content").size_full().on_scroll(move |_, _, _| content.set(content.get() + 1));
            pane_strip("strip", 0.0, px(300.0), vec![Some(screen.into_any_element()), Some(div().into_any_element())])
                .on_swipe(move |dir, _, _| steps.set(steps.get() + dir))
        }
    }

    fn wheel(dx: f32, shift: bool, phase: TouchPhase) -> ScrollEvent {
        ScrollEvent {
            position: point(px(150.0), px(100.0)),
            delta: ScrollDelta::Pixels(point(px(dx), px(0.0))),
            modifiers: Modifiers { shift, ..Default::default() },
            touch_phase: phase,
            source: Default::default(),
        }
    }

    /// Owner, 2026-10-21: a plain sideways swipe scrolls the content and never
    /// moves screens; Shift + a sideways swipe moves exactly one screen per
    /// gesture and does not reach the content.
    #[gpui::test]
    fn only_a_shift_swipe_moves_screens(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let (steps, content) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0u32)));
        let (_host, vcx) = cx.add_window_view({
            let (steps, content) = (steps.clone(), content.clone());
            move |_, _| StripHost { steps, content }
        });
        vcx.simulate_resize(gpui::size(px(300.0), px(200.0)));
        vcx.run_until_parked();

        for phase in [TouchPhase::Started, TouchPhase::Moved, TouchPhase::Moved, TouchPhase::Ended] {
            vcx.simulate_event(wheel(-40.0, false, phase));
        }
        assert_eq!(steps.get(), 0, "a plain sideways swipe never moves screens");
        assert_eq!(content.get(), 4, "and every event reaches the content");

        for phase in [TouchPhase::Started, TouchPhase::Moved, TouchPhase::Moved, TouchPhase::Ended] {
            vcx.simulate_event(wheel(-40.0, true, phase));
        }
        assert_eq!(steps.get(), 1, "Shift + a swipe moves one screen, once per gesture");
        assert_eq!(content.get(), 4, "a Shift-swipe does not also scroll the content");
    }
}
