//! Card 23: the view-options menu (250 wide) and its submenu (170 wide).
//!
//! Both render at their intrinsic size and know nothing about where they are
//! anchored; the caller (a popover, or the card) positions them. They enter
//! with a fade and a 6 px rise.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::util::{interaction_flags, TrackInteraction};

/// `.menu{width:250px;border-radius:var(--r-lg);padding:6px;font-size:12.5px}`.
const MENU_W: f32 = 250.0;
const MENU_PAD: f32 = 6.0;
const MENU_TEXT: f32 = 12.5;
/// `.menu .it{gap:8px;height:30px;padding:0 8px;border-radius:var(--r-sm)}`.
const ITEM_GAP: f32 = 8.0;
const ITEM_PAD_X: f32 = 8.0;
/// `.menu .it .v{gap:4px}` — the value and its chevron.
const VALUE_GAP: f32 = 4.0;
/// The submenu chevron on a value row is 10 px; the check is 14 px.
const VALUE_CHEVRON: f32 = 10.0;
const CHECK: f32 = 14.0;
/// A swatch row's leading colour circle.
const SWATCH: f32 = 10.0;
/// `.menu .sep{height:1px;margin:6px 4px}`.
const SEP_H: f32 = 1.0;
const SEP_MARGIN_Y: f32 = 6.0;
const SEP_MARGIN_X: f32 = 4.0;
/// `.sub{width:170px}` with `.sub .it{height:28px}` and a 12 px check.
const SUB_W: f32 = 170.0;
const SUB_ROW_H: f32 = 28.0;
const SUB_CHECK: f32 = 12.0;
/// Menus enter with a fade and a 6 px rise (spec §1.3, §4.2).
const MENU_RISE: f32 = 6.0;

type ActivateHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// One row of the view-options menu.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuRow {
    /// A row that opens a submenu: label, current value, and whether the
    /// submenu is open (the row is held on surface-2).
    Submenu {
        /// The left label.
        label: SharedString,
        /// The current choice, ink-3 at the right.
        value: SharedString,
        /// The submenu is open: surface-2 ground.
        highlighted: bool,
    },
    /// A row that toggles a preference, with an accent-ink check when on.
    Toggle {
        /// The left label.
        label: SharedString,
        /// Draws the check.
        checked: bool,
    },
    /// A toggle row with a leading colour swatch (the project colour
    /// submenu): the check is drawn as [`MenuRow::Toggle`] draws it.
    Swatch {
        /// The left label.
        label: SharedString,
        /// The 10 px circle before the label.
        colour: gpui::Hsla,
        /// Draws the check.
        checked: bool,
    },
    /// A hairline between two blocks of rows.
    Separator,
}

/// The view-options menu. Build with [`view_menu`].
#[derive(IntoElement)]
pub struct ViewMenu {
    id: ElementId,
    rows: Vec<MenuRow>,
    present: bool,
    timing: EnterExit,
    on_activate: Option<ActivateHandler>,
}

/// The 250 px menu reached from the sliders icon on a group row.
pub fn view_menu(id: impl Into<ElementId>, rows: Vec<MenuRow>) -> ViewMenu {
    ViewMenu { id: id.into(), rows, present: true, timing: EnterExit::DEFAULT, on_activate: None }
}

impl ViewMenu {
    /// Whether the menu is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the menu is drawn at rest on its first frame. For a
    /// menu that is part of a static composition (the design card, a restored
    /// panel) rather than one the person just opened.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// A row was clicked; the argument is its index in `rows`.
    pub fn on_activate(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }
}

/// The shared chrome of both menus: overlay ground, 1 px line-strong, radius
/// 12, elevation 3, 6 px padding.
fn menu_surface(p: &aui_tokens::Palette, width: f32) -> gpui::Div {
    v_flex()
        .flex_none()
        .w(px(width))
        .p(px(MENU_PAD))
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line_strong)
        .bg(p.overlay)
        .shadow(p.shadow(3))
        .ui(MENU_TEXT)
        .text_color(p.ink)
}

/// One clickable row of either menu.
fn menu_row(id: ElementId, height: gpui::Pixels, ground: gpui::Hsla, window: &mut Window, cx: &mut App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let (state, flags) = interaction_flags(id.clone(), window, cx);
    // A row without its own ground borrows surface-2 while the pointer is on
    // it; either way the tint fades its own alpha rather than tweening toward
    // transparent black, which would dip the row through a darker colour than
    // both the tint and the menu ground.
    let lit = flags.hovered || ground.a > 0.0;
    let tint = if ground.a > 0.0 { ground } else { p.surface_2 };
    let bg = aui_motion::tint_fade((id.clone(), "bg"), lit, tint, aui_motion::Tween::FAST, window, cx);
    h_flex()
        .id(id)
        .w_full()
        .h(height)
        .flex_none()
        .gap(px(ITEM_GAP))
        .px(px(ITEM_PAD_X))
        .rounded(px(scale::R_SM))
        .bg(bg)
        .cursor_pointer()
        .track_interaction(&state)
}

/// A toggle row's content in either menu: the optional leading colour
/// swatch, the truncating label, and the accent-ink check when on.
fn toggle_row(
    mut el: gpui::Stateful<gpui::Div>,
    label: SharedString,
    checked: bool,
    swatch: Option<gpui::Hsla>,
    check: f32,
    p: &aui_tokens::Palette,
) -> gpui::Stateful<gpui::Div> {
    if let Some(colour) = swatch {
        el = el.child(div().flex_none().size(px(SWATCH)).rounded_full().bg(colour));
    }
    el = el.child(div().min_w(px(0.0)).truncate().child(label)).child(div().flex_1());
    if checked {
        el = el.child(icon(IconName::Check).size(px(check)).color(p.accent_ink));
    }
    el
}

/// `.menu .sep` / `.sub .sep`.
fn separator(p: &aui_tokens::Palette) -> gpui::Div {
    div().flex_none().h(px(SEP_H)).my(px(SEP_MARGIN_Y)).mx(px(SEP_MARGIN_X)).bg(p.line)
}

impl RenderOnce for ViewMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let style = PresenceStyle::fade_rise(sample, MENU_RISE);
        let row_h = cx.aui().metrics.row;

        let mut menu = menu_surface(&p, MENU_W).relative().top(style.offset_y).opacity(style.opacity);
        for (i, row) in self.rows.into_iter().enumerate() {
            let key: ElementId = (id.clone(), SharedString::from(format!("row-{i}"))).into();
            match row {
                MenuRow::Separator => menu = menu.child(separator(&p)),
                MenuRow::Submenu { label, value, highlighted } => {
                    let ground = if highlighted { p.surface_2 } else { gpui::transparent_black() };
                    let mut el = menu_row(key, row_h, ground, window, cx)
                        .child(div().min_w(px(0.0)).truncate().child(label))
                        .child(div().flex_1())
                        .child(
                            h_flex()
                                .flex_none()
                                .items_center()
                                .gap(px(VALUE_GAP))
                                .text_color(p.ink_3)
                                .child(value)
                                .child(icon(IconName::Chev).size(px(VALUE_CHEVRON)).color(p.ink_3)),
                        );
                    if let Some(h) = self.on_activate.clone() {
                        el = el.on_click(move |_, w, cx| h(i, w, cx));
                    }
                    menu = menu.child(el);
                }
                MenuRow::Toggle { label, checked } => {
                    let mut el = toggle_row(menu_row(key, row_h, gpui::transparent_black(), window, cx), label, checked, None, CHECK, &p);
                    if let Some(h) = self.on_activate.clone() {
                        el = el.on_click(move |_, w, cx| h(i, w, cx));
                    }
                    menu = menu.child(el);
                }
                MenuRow::Swatch { label, colour, checked } => {
                    let mut el =
                        toggle_row(menu_row(key, row_h, gpui::transparent_black(), window, cx), label, checked, Some(colour), CHECK, &p);
                    if let Some(h) = self.on_activate.clone() {
                        el = el.on_click(move |_, w, cx| h(i, w, cx));
                    }
                    menu = menu.child(el);
                }
            }
        }
        menu
    }
}

/// The 170 px submenu of the `Group by` row. Build with [`view_submenu`], or
/// with [`view_submenu_rows`] for rows that carry their own checks (the
/// project colour swatches).
#[derive(IntoElement)]
pub struct ViewSubmenu {
    id: ElementId,
    items: Vec<SharedString>,
    rows: Vec<MenuRow>,
    selected: Option<usize>,
    separator_before: Option<usize>,
    present: bool,
    timing: EnterExit,
    on_activate: Option<ActivateHandler>,
}

/// A submenu listing `items`, with `selected` marked by an accent-ink check.
pub fn view_submenu(id: impl Into<ElementId>, items: Vec<SharedString>, selected: Option<usize>) -> ViewSubmenu {
    ViewSubmenu { id: id.into(), items, rows: Vec::new(), selected, separator_before: None, present: true, timing: EnterExit::DEFAULT, on_activate: None }
}

/// A submenu of menu rows, each carrying its own check (a `Colour` submenu
/// of [`MenuRow::Swatch`] rows). Indices reported by `on_activate` count
/// across the plain `items` first, then these rows.
pub fn view_submenu_rows(id: impl Into<ElementId>, rows: Vec<MenuRow>) -> ViewSubmenu {
    ViewSubmenu { id: id.into(), items: Vec::new(), rows, selected: None, separator_before: None, present: true, timing: EnterExit::DEFAULT, on_activate: None }
}

impl ViewSubmenu {
    /// Draws a hairline above the item at `index` (`None` sits below the
    /// grouping choices).
    pub fn separator_before(mut self, index: usize) -> Self {
        self.separator_before = Some(index);
        self
    }

    /// Whether the submenu is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the submenu is drawn at rest on its first frame.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// An item was clicked; the argument is its index.
    pub fn on_activate(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ViewSubmenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let style = PresenceStyle::fade_rise(sample, MENU_RISE);

        let mut menu = menu_surface(&p, SUB_W).relative().top(style.offset_y).opacity(style.opacity);
        let plain = self.items.len();
        for (i, label) in self.items.into_iter().enumerate() {
            if self.separator_before == Some(i) {
                menu = menu.child(separator(&p));
            }
            let key: ElementId = (id.clone(), SharedString::from(format!("item-{i}"))).into();
            let on = self.selected == Some(i);
            let ground = if on { p.surface_3 } else { gpui::transparent_black() };
            let mut el = menu_row(key, px(SUB_ROW_H), ground, window, cx).child(div().min_w(px(0.0)).truncate().child(label));
            if on {
                el = el.child(div().flex_1()).child(icon(IconName::Check).size(px(SUB_CHECK)).color(p.accent_ink));
            }
            if let Some(h) = self.on_activate.clone() {
                el = el.on_click(move |_, w, cx| h(i, w, cx));
            }
            menu = menu.child(el);
        }
        for (j, row) in self.rows.into_iter().enumerate() {
            let i = plain + j;
            let key: ElementId = (id.clone(), SharedString::from(format!("row-{j}"))).into();
            match row {
                MenuRow::Separator => menu = menu.child(separator(&p)),
                MenuRow::Toggle { label, checked } => {
                    let mut el = toggle_row(menu_row(key, px(SUB_ROW_H), gpui::transparent_black(), window, cx), label, checked, None, SUB_CHECK, &p);
                    if let Some(h) = self.on_activate.clone() {
                        el = el.on_click(move |_, w, cx| h(i, w, cx));
                    }
                    menu = menu.child(el);
                }
                MenuRow::Swatch { label, colour, checked } => {
                    let mut el =
                        toggle_row(menu_row(key, px(SUB_ROW_H), gpui::transparent_black(), window, cx), label, checked, Some(colour), SUB_CHECK, &p);
                    if let Some(h) = self.on_activate.clone() {
                        el = el.on_click(move |_, w, cx| h(i, w, cx));
                    }
                    menu = menu.child(el);
                }
                MenuRow::Submenu { label, value, .. } => {
                    let mut el = menu_row(key, px(SUB_ROW_H), gpui::transparent_black(), window, cx)
                        .child(div().min_w(px(0.0)).truncate().child(label))
                        .child(div().flex_1())
                        .child(div().flex_none().text_color(p.ink_3).child(value));
                    if let Some(h) = self.on_activate.clone() {
                        el = el.on_click(move |_, w, cx| h(i, w, cx));
                    }
                    menu = menu.child(el);
                }
            }
        }
        menu
    }
}
