//! The three composer chip menus: **model**, **effort** and **approval mode**.
//!
//! One component with three constructors, because they are one control with
//! three data sets: a list of rows with a label, a one-line detail, an optional
//! trailing meta (a model's context limit) and any number of badges
//! (`default`, `active`), plus a check on the selected row.
//!
//! They are the one menu panel ([`crate::interact::Dropdown`], FF's dropdown),
//! painted through [`crate::overlay::popover_layer`] and anchored by the
//! composer to the chip that opened them
//! ([`crate::composer::Composer::chip_menu`]).
//!
//! Stateless: rows in, `selected` in, `open` in; pick, hover and close out. In
//! particular **the selection is not a fact about the rows**. MSP says a client
//! must not assume exactly one `isActive` or `isDefault` model, so "active" is a
//! badge that any number of rows may wear, and `selected` is only where the
//! keyboard is.

use std::rc::Rc;

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use gpui_kit::base::ElementExt;
use crate::interact::{dropdown, dropdown_present, DropdownEntry, DropdownItem, DropdownSide, DropdownTrailing};
use crate::overlay::popover_layer;

/// `.pick{bottom:28px;left:0;padding:6px}` — measured from the chip it hangs
/// off, which is 28 px tall like every composer toolbar control.
const MENU_BOTTOM: f32 = 28.0;
/// The model menu carries a context limit and two badges, so it is wider than
/// the `+` menu's 200; effort and mode share the floor so the three read as one
/// control.
///
/// It is a **floor**, not a width. A provider names its models what it likes and
/// MSP's catalog is discovered at runtime, so a fixed width would have to cut a
/// name off to fit — and half a model name is not a model name (finding F8).
/// Nothing here ever ellipsises: the menu is at least this wide, grows with its
/// content up to the ceiling, and past that the label wraps rather than being
/// cut.
///
/// The floor is set to the widest name Muse's own catalog ships
/// (`muse-spark-1.2-contributor` with its context limit and two badges) because
/// gpui's absolute layout does not shrink-to-fit a column of stretched rows: a
/// row that fills its parent and a parent that sizes to its rows is circular,
/// and taffy resolves that circle at the floor. So the floor has to be a width
/// that is actually right, not a token minimum nobody expects to see.
const MENU_W_MIN: f32 = 360.0;
const MENU_W_MAX: f32 = 520.0;
/// The gap between a row's meta and its badges.
const ITEM_GAP: f32 = 8.0;
/// Rows padded 10 at the sides (docs/41 §2.10).
const ROW_PAD_X: f32 = 10.0;
/// The provider picker's width.
const PROVIDER_W: f32 = 240.0;
/// The approval-mode and effort lists: 320, their meanings wrapping under the
/// name (owner 2026-10-14: the mode menu ran 520 wide, past the sheet).
const NARROW_W: f32 = 320.0;
/// The nested submenu: 4 px right of the panel, 300 wide, opening after a
/// 150 ms rest on its row.
const SUBMENU_GAP: f32 = 4.0;
const SUBMENU_W: f32 = 300.0;
/// The submenu's caption header, above its first row.
const SUB_HEADER_H: f32 = 28.0;
const SUBMENU_DELAY: std::time::Duration = std::time::Duration::from_millis(150);
/// The meta's mono size.
const DETAIL_TEXT: f32 = scale::FS_11;

/// One row of a composer picker.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerRow {
    /// Stable identity, handed back by [`PickerMenu::on_pick`].
    pub id: SharedString,
    /// The row's name (`muse-spark-1.3`, `Extra high`, `Read-only`).
    pub label: SharedString,
    /// The one-line description under it; empty draws no second line.
    pub detail: SharedString,
    /// A trailing mono fact — a model's context limit, say. `None` draws none.
    pub meta: Option<SharedString>,
    /// Tags at the right edge (`default`, `active`). A model row may carry
    /// none, one or both, and several rows may carry the same one.
    pub badges: Vec<SharedString>,
    /// A provider's mark before the label (the provider picker).
    pub mark: Option<aui_icons::Provider>,
    /// A nested submenu this row opens (a model's effort levels): its title,
    /// its rows and the row that is current. The row draws a chevron.
    pub submenu: Option<PickerSubmenu>,
}

/// A row's nested submenu (docs/41 §2.10): drawn beside its row, 4 px to the
/// right, top-aligned, inside the menu's own box so a pick in it is never an
/// outside press.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerSubmenu {
    /// The submenu's caption header (`Reasoning effort · muse-spark-1.3`).
    pub title: SharedString,
    /// Its rows.
    pub rows: Vec<PickerRow>,
    /// The current row, which wears the check.
    pub selected: Option<usize>,
}

impl PickerRow {
    /// A row with a label and a description.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), detail: detail.into(), meta: None, badges: Vec::new(), mark: None, submenu: None }
    }

    /// The provider's mark before the label.
    pub fn mark(mut self, provider: aui_icons::Provider) -> Self {
        self.mark = Some(provider);
        self
    }

    /// A nested submenu this row opens.
    pub fn submenu(mut self, submenu: PickerSubmenu) -> Self {
        self.submenu = Some(submenu);
        self
    }

    /// The trailing mono fact.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Adds one badge at the right edge.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badges.push(badge.into());
        self
    }
}

type PickHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type NestedPickHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A composer chip menu. Build with [`model_menu`], [`effort_menu`] or
/// [`mode_menu`].
#[derive(IntoElement)]
pub struct PickerMenu {
    id: ElementId,
    title: SharedString,
    rows: Vec<PickerRow>,
    selected: usize,
    open: bool,
    at_rest: bool,
    on_pick: Option<PickHandler>,
    on_pick_nested: Option<NestedPickHandler>,
    on_hover: Option<HoverHandler>,
    on_close: Option<CloseHandler>,
    width: f32,
    max_width: f32,
    align_end: bool,
    force_sub: Option<usize>,
}

/// The model picker: rows from the provider catalog, `selected` on the row the
/// session is actually using.
///
/// `meta` is where the context limit goes and `badges` where `default` and
/// `active` go; both are the caller's to fill, because MSP's catalog can flag
/// any number of rows either way.
pub fn model_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    menu(id, "Model", rows, selected, open)
}

/// The reasoning-effort picker.
pub fn effort_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    let mut menu = menu(id, "Reasoning effort", rows, selected, open);
    (menu.width, menu.max_width) = (NARROW_W, NARROW_W);
    menu
}

/// The approval-mode picker.
pub fn mode_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    let mut menu = menu(id, "Approval mode", rows, selected, open);
    (menu.width, menu.max_width) = (NARROW_W, NARROW_W);
    menu
}

fn menu(id: impl Into<ElementId>, title: &'static str, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    PickerMenu {
        id: id.into(),
        title: title.into(),
        rows,
        selected,
        open,
        at_rest: false,
        on_pick: None,
        on_pick_nested: None,
        on_hover: None,
        on_close: None,
        width: MENU_W_MIN,
        max_width: MENU_W_MAX,
        align_end: false,
        force_sub: None,
    }
}

/// The provider picker (docs/41 §2.10, the owner, 2026-10-13): just the
/// providers — mark and name, the current one checked — 240 wide.
pub fn provider_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    let mut menu = menu(id, "Provider", rows, selected, open);
    menu.width = PROVIDER_W;
    menu
}

impl PickerMenu {
    /// Hang from the chip's right edge instead of its left — for chips at
    /// the right of the controls row, so the panel grows into the sheet and
    /// never past it (where a native page in the pane draws above it); the
    /// submenu then opens to the panel's left.
    pub fn align_end(mut self, end: bool) -> Self {
        self.align_end = end;
        self
    }

    /// Replaces the caps header.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Skips the enter (static captures).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// A row was activated; the argument is its [`PickerRow::id`].
    pub fn on_pick(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(f));
        self
    }

    /// Opens a row's submenu from the host (the → key, a capture); `None`
    /// leaves it to the pointer.
    pub fn open_submenu(mut self, row: Option<usize>) -> Self {
        self.force_sub = row;
        self
    }

    /// A row of a nested submenu was picked: the parent row's id and the
    /// submenu row's id.
    pub fn on_pick_nested(mut self, f: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick_nested = Some(Rc::new(f));
        self
    }

    /// The pointer entered a row; the argument is its index, so the caller can
    /// move the selection to it and keep one highlight on screen.
    pub fn on_hover(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    /// A click outside the menu.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

/// A row's trailing cluster: the meta in faint mono, the badges as 9 % caps.
fn row_trailing(row: &PickerRow, p: &aui_tokens::Palette) -> Option<gpui::AnyElement> {
    if row.meta.is_none() && row.badges.is_empty() {
        return None;
    }
    let mut trailing = h_flex().gap(px(ITEM_GAP)).items_center();
    if let Some(meta) = &row.meta {
        trailing = trailing.child(div().flex_none().mono(DETAIL_TEXT).text_color(p.ink_4).child(meta.clone()));
    }
    for badge in &row.badges {
        trailing = trailing.child(div().flex_none().px(px(6.0)).rounded_full().bg(p.selected_overlay()).ui(DETAIL_TEXT).line_height(px(16.0)).text_color(p.ink_2).child(badge.clone()));
    }
    Some(trailing.into_any_element())
}

/// One picker row as a dropdown item.
fn picker_item(row: &PickerRow, checked: bool, p: &aui_tokens::Palette) -> DropdownItem {
    picker_item_placed(row, checked, p, None::<fn(gpui::Pixels, &mut App)>)
}

/// [`picker_item`], reporting the row's vertical centre each frame through
/// `place` (a submenu's parent row).
fn picker_item_placed(row: &PickerRow, checked: bool, p: &aui_tokens::Palette, place: Option<impl Fn(gpui::Pixels, &mut App) + 'static>) -> DropdownItem {
    let mut item = DropdownItem::new(row.label.clone()).detail(row.detail.clone()).wrap().checked(checked);
    if let Some(provider) = row.mark {
        item = item.leading(div().size(px(16.0)).rounded(px(4.0)).overflow_hidden().child(aui_icons::provider_mark(provider).size(px(16.0))));
    }
    let chevron_placed = || {
        let marker = div().flex_none().child(chevron(p));
        match place {
            Some(place) => marker.on_prepaint(move |bounds, _, cx| place(bounds.center().y, cx)).into_any_element(),
            None => marker.into_any_element(),
        }
    };
    match (row_trailing(row, p), &row.submenu) {
        (Some(trailing), Some(_)) => item = item.trailing(DropdownTrailing::Element(h_flex().gap(px(ITEM_GAP)).items_center().child(trailing).child(chevron_placed()).into_any_element())),
        (Some(trailing), None) => item = item.trailing(DropdownTrailing::Element(trailing)),
        (None, Some(_)) => item = item.trailing(DropdownTrailing::Element(chevron_placed())),
        (None, None) => {}
    }
    item
}

fn chevron(p: &aui_tokens::Palette) -> impl IntoElement {
    aui_icons::icon(aui_icons::IconName::Chev).size(px(12.0)).color(p.ink_2)
}

/// Where the open submenu is: which row, since when the pointer has rested
/// on it, and the measured geometry it hangs from.
#[derive(Default, Clone)]
struct Nest {
    hover: Option<(usize, std::time::Instant)>,
    open: Option<usize>,
    panel: Option<gpui::Bounds<gpui::Pixels>>,
    rows: Vec<(usize, gpui::Pixels)>,
}

impl RenderOnce for PickerMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // At rest a closed menu draws nothing (static captures).
        let present = if self.at_rest { self.open } else { dropdown_present((id.clone(), "panel"), self.open, window, cx) };
        if !present {
            return div().invisible().into_any_element();
        }
        // The nested submenu opens after a 150 ms rest on its row.
        let nest = window.use_keyed_state((id.clone(), "nest"), cx, |_, _| Nest::default());
        let state = nest.read(cx).clone();
        if let (Some((row, since)), None) = (state.hover, state.open.filter(|o| Some(*o) == state.hover.map(|h| h.0))) {
            if since.elapsed() >= SUBMENU_DELAY {
                nest.update(cx, |n, _| n.open = Some(row));
            } else {
                window.request_animation_frame();
            }
        }
        let open_sub = self.force_sub.or(nest.read(cx).open).filter(|i| self.rows.get(*i).is_some_and(|r| r.submenu.is_some()));

        // The one menu panel (FF's dropdown): the title as FF's group label,
        // then the rows padded 10; `selected` is the keyboard's row and wears
        // the check, the pointer drives the same gliding block.
        let mut panel = dropdown((id.clone(), "panel"), self.title.clone())
            .min_width(px(self.width))
            .row_padding(px(ROW_PAD_X))
            .side(DropdownSide::Above)
            .open(self.open)
            .highlighted(Some(self.selected + 1))
            .entry(DropdownEntry::Label(self.title.clone()));
        if self.at_rest {
            panel = panel.at_rest();
        }
        {
            let hover = self.on_hover.clone();
            let nest = nest.clone();
            let has_sub: Vec<bool> = self.rows.iter().map(|r| r.submenu.is_some()).collect();
            panel = panel.on_highlight(move |index, w, cx| {
                let row = index.and_then(|i| i.checked_sub(1));
                if let (Some(row), Some(hover)) = (row, hover.as_ref()) {
                    hover(row, w, cx);
                }
                nest.update(cx, |n, _| match row {
                    Some(r) if has_sub.get(r).copied().unwrap_or(false) => {
                        if n.hover.map(|h| h.0) != Some(r) {
                            n.hover = Some((r, std::time::Instant::now()));
                        }
                    }
                    Some(r) => {
                        n.hover = None;
                        if n.open != Some(r) {
                            n.open = None;
                        }
                    }
                    None => n.hover = None,
                });
                w.refresh();
            });
        }
        for (index, row) in self.rows.iter().enumerate() {
            // A row with a submenu reports where it is (its trailing
            // chevron's centre), for the submenu to hang from.
            let place = row.submenu.is_some().then(|| {
                let nest = nest.clone();
                move |y: gpui::Pixels, cx: &mut App| {
                    nest.update(cx, |n, _| {
                        n.rows.retain(|(i, _)| *i != index);
                        n.rows.push((index, y));
                    })
                }
            });
            let mut item = picker_item_placed(row, index == self.selected, &p, place);
            if let Some(handler) = self.on_pick.clone() {
                let row_id = row.id.clone();
                item = item.on_select(move |w, cx| handler(&row_id, w, cx));
            }
            panel = panel.entry(item);
        }
        let menu = div().id(id.clone()).absolute().bottom(px(MENU_BOTTOM)).max_w(px(self.max_width));
        let mut menu = if self.align_end { menu.right(px(0.0)) } else { menu.left(px(0.0)) }.child({
            let nest = nest.clone();
            div().on_prepaint(move |bounds, _, cx| nest.update(cx, |n, _| n.panel = Some(bounds))).child(panel)
        });
        // The open submenu: 4 px right of the panel, its top on its row's.
        if let Some(index) = open_sub {
            let geometry = nest.read(cx).clone();
            if let (Some(sub), Some(panel_bounds)) = (self.rows[index].submenu.clone(), geometry.panel) {
                let row_center = geometry.rows.iter().find(|(i, _)| *i == index).map(|(_, y)| *y).unwrap_or(panel_bounds.top());
                // The submenu's first row lines up with its parent row: its
                // top sits the panel padding and the caption header above.
                let top = (row_center - px(18.0) - panel_bounds.top() - px(crate::interact::PANEL_PAD + SUB_HEADER_H)).max(px(0.0));
                let parent = self.rows[index].id.clone();
                let mut subpanel = dropdown((id.clone(), "sub"), sub.title.clone())
                    .width(px(SUBMENU_W))
                    .row_padding(px(ROW_PAD_X))
                    .side(DropdownSide::Beside)
                    .at_rest()
                    .entry(DropdownEntry::Label(sub.title.clone()));
                for (i, row) in sub.rows.iter().enumerate() {
                    let mut item = picker_item(row, sub.selected == Some(i), &p);
                    if let Some(handler) = self.on_pick_nested.clone() {
                        let (parent, child) = (parent.clone(), row.id.clone());
                        item = item.on_select(move |w, cx| handler(&parent, &child, w, cx));
                    }
                    subpanel = subpanel.entry(item);
                }
                let beside = div().absolute().top(top);
                let beside = if self.align_end { beside.right(panel_bounds.size.width + px(SUBMENU_GAP)) } else { beside.left(panel_bounds.size.width + px(SUBMENU_GAP)) };
                menu = menu.child(beside.child(subpanel));
            }
        }

        // A click anywhere else closes it. The catcher is a sibling of the menu
        // inside the same deferred draw, so it covers the window without
        // covering the menu.
        let mut layer = div().absolute().top(px(0.0)).left(px(0.0)).right(px(0.0)).child(menu);
        if let Some(close) = self.on_close.clone() {
            // The catcher's wrapper spans the chip too, or the panel's
            // `right(0)` resolves against a zero-width box (the chip's
            // left edge) instead of the chip's right.
            layer = div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .child(
                    div()
                        .id((id.clone(), "scrim"))
                        .occlude()
                        .absolute()
                        // The catcher is anchored to the chip, so it has to
                        // reach far enough in every direction to cover a
                        // 1440 × 900 window from wherever the chip sits.
                        .top(px(-SCRIM_REACH))
                        .left(px(-SCRIM_REACH))
                        .w(px(SCRIM_REACH * 2.0))
                        .h(px(SCRIM_REACH * 2.0))
                        .on_click(move |_, w, cx| close(w, cx)),
                )
                .child(layer);
        }
        popover_layer(layer).into_any_element()
    }
}

/// How far the click-catcher reaches from the chip in each direction; a window
/// is at most 1440 × 900 and the chip can be anywhere in it.
const SCRIM_REACH: f32 = 4000.0;


/// The effort picker as a slider (docs/41 §2.10; the owner chose it,
/// 2026-10-14): the levels as stops on a 4 px track at 12 %, filled to the
/// current stop in the foreground, a tick at every stop (2 × 10, 18 %), a
/// 16 px foreground thumb with a 1 px 40 % black ring; the current level's
/// name (600) and meaning (muted) above, the first, middle and last names
/// (11, faint) under the ends and the middle. Drag or click snaps to a stop
/// and reports it live; **Default** — "leave it to the provider", not a
/// level — stays a row under a separator.
#[derive(IntoElement)]
pub struct EffortSliderMenu {
    id: ElementId,
    levels: Vec<PickerRow>,
    current: Option<usize>,
    default_row: PickerRow,
    open: bool,
    at_rest: bool,
    on_change: Option<PickHandler>,
    on_pick: Option<PickHandler>,
    on_close: Option<CloseHandler>,
    align_end: bool,
}

/// The slider over `levels` (low to high), `current` the level in use or
/// `None` when the session leaves effort to the provider (`default_row`).
pub fn effort_slider_menu(id: impl Into<ElementId>, levels: Vec<PickerRow>, current: Option<usize>, default_row: PickerRow, open: bool) -> EffortSliderMenu {
    EffortSliderMenu { id: id.into(), levels, current, default_row, open, at_rest: false, on_change: None, on_pick: None, on_close: None, align_end: false }
}

impl EffortSliderMenu {
    /// Hang from the chip's right edge (see [`PickerMenu::align_end`]): the
    /// chip's label changes width as the level changes, and a panel pinned
    /// to its left edge slid sideways with every step (owner 2026-10-15).
    pub fn align_end(mut self, end: bool) -> Self {
        self.align_end = end;
        self
    }

    /// Skips the enter (static captures).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// The slider moved to a level (its [`PickerRow::id`]): live, while the
    /// menu stays open.
    pub fn on_change(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// The Default row was picked (its id).
    pub fn on_pick(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(f));
        self
    }

    /// A click outside the menu.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

/// The stop nearest `x` on a track `track` wide, for `count` stops.
pub fn slider_stop(x: f32, track: f32, count: usize) -> usize {
    if count <= 1 || track <= 0.0 {
        return 0;
    }
    let step = track / (count - 1) as f32;
    ((x / step).round().max(0.0) as usize).min(count - 1)
}

/// The slider's measured track, for mapping the pointer to a stop.
#[derive(Default, Clone, Copy)]
struct Track {
    bounds: Option<gpui::Bounds<gpui::Pixels>>,
    dragging: bool,
}

impl RenderOnce for EffortSliderMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let present = if self.at_rest { self.open } else { dropdown_present((id.clone(), "panel"), self.open, window, cx) };
        if !present {
            return div().invisible().into_any_element();
        }
        let count = self.levels.len();
        let track_state = window.use_keyed_state((id.clone(), "track"), cx, |_, _| Track::default());
        let shown = self.current.unwrap_or(0).min(count.saturating_sub(1));
        let fraction = if count > 1 { shown as f32 / (count - 1) as f32 } else { 0.0 };
        let level = self.levels.get(shown);
        let name: SharedString = match (self.current, level) {
            (Some(_), Some(level)) => level.label.clone(),
            _ => self.default_row.label.clone(),
        };
        let meaning: SharedString = match (self.current, level) {
            (Some(_), Some(level)) => level.detail.clone(),
            _ => self.default_row.detail.clone(),
        };

        let line = if p.is_dark() { gpui::white() } else { gpui::black() };
        let mut track = div()
            .id((id.clone(), "slider"))
            .relative()
            .w_full()
            .h(px(SLIDER_HIT))
            .cursor_pointer()
            .role(gpui::Role::Slider)
            .aria_label("Reasoning effort");
        crate::data::record_ax_label("Reasoning effort");
        // The 4 px track, the fill and the ticks, then the thumb.
        let rail = div()
            .absolute()
            .left_0()
            .right_0()
            .top(px((SLIDER_HIT - SLIDER_TRACK) / 2.0))
            .h(px(SLIDER_TRACK))
            .rounded_full()
            .bg(line.alpha(0.12))
            .child(div().absolute().left_0().top_0().h_full().w(gpui::relative(if self.current.is_some() { fraction } else { 0.0 })).rounded_full().bg(p.ink));
        track = track.child(rail);
        for stop in 0..count {
            let at = if count > 1 { stop as f32 / (count - 1) as f32 } else { 0.0 };
            track = track.child(
                div()
                    .absolute()
                    .top(px((SLIDER_HIT - SLIDER_TICK_H) / 2.0))
                    .left(gpui::relative(at))
                    .ml(px(-SLIDER_TICK_W / 2.0))
                    .w(px(SLIDER_TICK_W))
                    .h(px(SLIDER_TICK_H))
                    .bg(line.alpha(0.18)),
            );
        }
        if self.current.is_some() {
            track = track.child(
                div()
                    .absolute()
                    .top(px((SLIDER_HIT - SLIDER_THUMB) / 2.0))
                    .left(gpui::relative(fraction))
                    .ml(px(-SLIDER_THUMB / 2.0))
                    .size(px(SLIDER_THUMB))
                    .rounded_full()
                    .bg(p.ink)
                    .shadow(vec![gpui::BoxShadow { color: gpui::black().alpha(0.4), offset: gpui::point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(1.0), inset: false }]),
            );
        }
        {
            let state = track_state.clone();
            track = track.child(div().absolute().inset_0().on_prepaint(move |bounds, _, cx| {
                state.update(cx, |t, _| t.bounds = Some(bounds));
            }));
        }
        if let Some(change) = self.on_change.clone() {
            let ids: Rc<Vec<SharedString>> = Rc::new(self.levels.iter().map(|l| l.id.clone()).collect());
            let pick = {
                let state = track_state.clone();
                let ids = ids.clone();
                move |x: gpui::Pixels, w: &mut Window, cx: &mut App| {
                    let Some(bounds) = state.read(cx).bounds else { return };
                    let stop = slider_stop(f32::from(x - bounds.left()), f32::from(bounds.size.width), ids.len());
                    if let Some(id) = ids.get(stop) {
                        change(id, w, cx);
                    }
                }
            };
            let (down, moving) = (Rc::new(pick), track_state.clone());
            let on_down = down.clone();
            let start = track_state.clone();
            track = track
                .on_mouse_down(gpui::MouseButton::Left, move |e, w, cx| {
                    cx.stop_propagation();
                    start.update(cx, |t, _| t.dragging = true);
                    on_down(e.position.x, w, cx);
                })
                .on_mouse_move(move |e, w, cx| {
                    if moving.read(cx).dragging && e.pressed_button == Some(gpui::MouseButton::Left) {
                        down(e.position.x, w, cx);
                    }
                })
                .on_mouse_up(gpui::MouseButton::Left, {
                    let state = track_state.clone();
                    move |_, _, cx| state.update(cx, |t, _| t.dragging = false)
                });
        }
        let ends = |i: usize| self.levels.get(i).map(|l| l.label.clone()).unwrap_or_default();
        let under = h_flex()
            .w_full()
            .mt(px(4.0))
            .ui(scale::FS_11)
            .text_color(p.ink_4)
            .child(div().flex_1().child(ends(0)))
            .child(div().flex_none().child(ends(count.saturating_sub(1) / 2)))
            .child(div().flex_1().flex().justify_end().child(ends(count.saturating_sub(1))));
        let slider = v_flex()
            .w_full()
            .px(px(ROW_PAD_X + 2.0))
            .pt(px(6.0))
            .pb(px(8.0))
            .child(
                h_flex()
                    .gap(px(6.0))
                    .items_baseline()
                    .mb(px(8.0))
                    .child(div().flex_none().ui(scale::FS_13).semibold().text_color(p.ink).child(name))
                    .child(div().min_w(px(0.0)).truncate().ui(scale::FS_12).text_color(p.ink_2).child(meaning)),
            )
            .child(track)
            .child(under);

        let mut default = picker_item(&self.default_row, self.current.is_none(), &p);
        if let Some(handler) = self.on_pick.clone() {
            let row_id = self.default_row.id.clone();
            default = default.on_select(move |w, cx| handler(&row_id, w, cx));
        }
        let mut panel = dropdown((id.clone(), "panel"), "Reasoning effort")
            .width(px(MENU_W_MIN))
            .row_padding(px(ROW_PAD_X))
            .side(DropdownSide::Above)
            .open(self.open)
            .entry(DropdownEntry::Label("Reasoning effort".into()))
            .entry(DropdownEntry::Static(slider.into_any_element()))
            .entry(DropdownEntry::Separator)
            .entry(default);
        if self.at_rest {
            panel = panel.at_rest();
        }
        let menu = div().id(id.clone()).absolute().bottom(px(MENU_BOTTOM));
        let menu = if self.align_end { menu.right(px(0.0)) } else { menu.left(px(0.0)) }.child(panel);
        let mut layer = div().absolute().top(px(0.0)).left(px(0.0)).right(px(0.0)).child(menu);
        if let Some(close) = self.on_close.clone() {
            // The catcher's wrapper spans the chip too, or the panel's
            // `right(0)` resolves against a zero-width box (the chip's
            // left edge) instead of the chip's right.
            layer = div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .child(
                    div()
                        .id((id.clone(), "scrim"))
                        .occlude()
                        .absolute()
                        .top(px(-SCRIM_REACH))
                        .left(px(-SCRIM_REACH))
                        .w(px(SCRIM_REACH * 2.0))
                        .h(px(SCRIM_REACH * 2.0))
                        .on_click(move |_, w, cx| close(w, cx)),
                )
                .child(layer);
        }
        popover_layer(layer).into_any_element()
    }
}

/// The slider's geometry (docs/41 §2.10).
const SLIDER_HIT: f32 = 20.0;
const SLIDER_TRACK: f32 = 4.0;
const SLIDER_TICK_W: f32 = 2.0;
const SLIDER_TICK_H: f32 = 10.0;
const SLIDER_THUMB: f32 = 16.0;

#[cfg(test)]
mod slider_tests {
    use super::slider_stop;

    #[test]
    fn the_pointer_snaps_to_the_nearest_stop() {
        assert_eq!(slider_stop(0.0, 280.0, 8), 0);
        assert_eq!(slider_stop(280.0, 280.0, 8), 7);
        assert_eq!(slider_stop(19.0, 280.0, 8), 0);
        assert_eq!(slider_stop(21.0, 280.0, 8), 1);
        assert_eq!(slider_stop(-40.0, 280.0, 8), 0);
        assert_eq!(slider_stop(900.0, 280.0, 8), 7);
        assert_eq!(slider_stop(50.0, 280.0, 1), 0);
    }
}
