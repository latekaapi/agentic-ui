//! Card 23: the view-options menu and its submenu, as rows on the one
//! menu panel ([`Dropdown`], Fluid Functionalism's dropdown).
//!
//! Both render at their intrinsic size and know nothing about where they are
//! anchored; the caller (a popover, or the card) positions them.

use std::rc::Rc;

use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};

use crate::interact::{dropdown, DropdownEntry, DropdownItem, DropdownSide, DropdownTrailing};
use aui_motion::EnterExit;

/// FF's sidebar-anchored menus are 240 wide (`sidebar-menu-grid.ts`).
const MENU_W: f32 = 240.0;
/// FF's submenu (`w-56`).
const SUB_W: f32 = 224.0;
/// A swatch row's leading colour circle.
const SWATCH: f32 = 10.0;

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
    ViewMenu { id: id.into(), rows, present: true, timing: EnterExit::POPUP, on_activate: None }
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

/// The [`Dropdown`] item for one row; `None` for a separator.
fn dropdown_item(row: MenuRow) -> Option<DropdownItem> {
    match row {
        MenuRow::Separator => None,
        MenuRow::Submenu { label, value, .. } => Some(DropdownItem::new(label).trailing(DropdownTrailing::Submenu(value))),
        MenuRow::Toggle { label, checked } => Some(DropdownItem::new(label).checked(checked)),
        MenuRow::Swatch { label, colour, checked } => {
            Some(DropdownItem::new(label).checked(checked).leading(div().size(px(SWATCH)).rounded_full().bg(colour)))
        }
    }
}

/// The entries for `rows`, each item reporting `base + its index` (the
/// index counts separators, as `on_activate` always has).
fn dropdown_entries(rows: Vec<MenuRow>, base: usize, on_activate: &Option<ActivateHandler>) -> Vec<DropdownEntry> {
    rows.into_iter()
        .enumerate()
        .map(|(i, row)| match dropdown_item(row) {
            None => DropdownEntry::Separator,
            Some(mut item) => {
                if let Some(h) = on_activate.clone() {
                    item = item.on_select(move |w, cx| h(base + i, w, cx));
                }
                DropdownEntry::Item(item)
            }
        })
        .collect()
}

impl RenderOnce for ViewMenu {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // The one menu panel (FF's dropdown); a submenu row whose submenu is
        // open is lit by the fluid highlight that sits on it.
        let entries = dropdown_entries(self.rows, 0, &self.on_activate);
        let mut menu = dropdown(self.id, "Menu").width(px(MENU_W)).entries(entries).open(self.present);
        if self.timing.enter.is_zero() {
            menu = menu.at_rest();
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
    ViewSubmenu { id: id.into(), items, rows: Vec::new(), selected, separator_before: None, present: true, timing: EnterExit::POPUP, on_activate: None }
}

/// A submenu of menu rows, each carrying its own check (a `Colour` submenu
/// of [`MenuRow::Swatch`] rows). Indices reported by `on_activate` count
/// across the plain `items` first, then these rows.
pub fn view_submenu_rows(id: impl Into<ElementId>, rows: Vec<MenuRow>) -> ViewSubmenu {
    ViewSubmenu { id: id.into(), items: Vec::new(), rows, selected: None, separator_before: None, present: true, timing: EnterExit::POPUP, on_activate: None }
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
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let plain = self.items.len();
        let mut entries: Vec<DropdownEntry> = Vec::new();
        for (i, label) in self.items.into_iter().enumerate() {
            if self.separator_before == Some(i) {
                entries.push(DropdownEntry::Separator);
            }
            let mut item = DropdownItem::new(label).current(self.selected == Some(i));
            if let Some(h) = self.on_activate.clone() {
                item = item.on_select(move |w, cx| h(i, w, cx));
            }
            entries.push(DropdownEntry::Item(item));
        }
        entries.extend(dropdown_entries(self.rows, plain, &self.on_activate));
        let mut menu = dropdown(self.id, "Submenu").width(px(SUB_W)).entries(entries).open(self.present).side(DropdownSide::Beside);
        if self.timing.enter.is_zero() {
            menu = menu.at_rest();
        }
        menu
    }
}
