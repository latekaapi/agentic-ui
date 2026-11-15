//! Interaction primitives: the one hover / press / selected model
//! ([`paint`]), [`Interactive`] (the focus ring and tab stop), the one row
//! ([`Row`] / [`list_row`]), [`RowList`], [`Surface`], the one floating
//! panel ([`menu_panel`]) and the menu built on it ([`Dropdown`]).
//!
//! The kinds split between two modules: things you point at in a list or a
//! float live here (`list_row`, `menu_panel`, `tabs` rows' paint); static
//! containers and form controls live in [`crate::data`] (`card`,
//! `section`, `settings_row`, `field`, `banner`, `segmented`, `tabs`,
//! `switch`, `checkbox`).
//!
//! The rules every element here follows (docs/83 §1, §7, §9):
//!
//! - hover, selected and pressed are translucent overlays over whatever is
//!   beneath, painted by gpui's own refinements through [`StatePaint`] —
//!   never a keyed state, a tween or a gliding block;
//! - motion is for appearing, disappearing and collapsing, never hover;
//! - the focus-visible ring lives on controls only, never on rows;
//! - a surface states its step; only floating things cast a shadow.

pub mod dropdown;
pub mod interactive;
pub mod list;
pub mod paint;
pub mod row;
pub mod surface;

pub use dropdown::{
    dropdown, dropdown_height, dropdown_label, dropdown_present, dropdown_separator, Dropdown, DropdownEntry, DropdownItem,
    DropdownSide, DropdownTrailing,
};
pub use interactive::{focus_ring, interactive, InteractState, Interactive, INTERACT_FOCUS_TAG};
pub use list::{row_list, ListItems, RowList};
pub use paint::{over, tinted, PaintHover, PaintStates, PaletteSource, StatePaint, Tint, HOVER_GROUP};
pub use row::{list_row, row, Row};
pub use surface::{menu_panel, nested_radius, surface, Surface, PANEL_PAD};

// ── Flat metrics awaiting tokens ────────────────────────────────────────────
// docs/83 §2.5 names these values; aui-tokens has no token for them yet.
// Task 2.3 (docs/82) moves them into tokens.json; until then the primitives
// read them from here, never as bare literals at call sites.

/// One-line list row height (`ctl-default` flat: 32).
pub const LIST_ROW_H: f32 = 32.0;
/// The gap between rows in a list (`row-gap`: 2).
pub const ROW_GAP: f32 = 2.0;
/// A list's inset from its region's edge (`list-inset`: 8).
pub const LIST_INSET: f32 = 8.0;
/// A row's inner padding on x (flat: 10).
pub const ROW_PAD_X: f32 = 10.0;
/// The item radius (`r-md` / `shape-item` flat: 8): rows, buttons, inputs,
/// tabs, segmented.
pub const R_ITEM: f32 = 8.0;
/// A menu row's height (docs/83 §7: 28).
pub const MENU_ROW_H: f32 = 28.0;
/// A menu row's radius (docs/83 §7: 6).
pub const R_MENU_ROW: f32 = 6.0;
/// A menu separator's inset from the panel's sides (docs/83 §7: 8).
pub const MENU_SEPARATOR_INSET: f32 = 8.0;

use aui_tokens::scale;
use gpui::px;

/// The corner shape of an interactive control or a surface.
///
/// Radii come from the shape pair (plus the full radius for pills); the
/// concentric rule — an inner radius is the outer radius minus the inset —
/// is [`nested_radius`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shape {
    /// The item radius (`shape-item`, 6 px today; 8 in docs/83, task 2.3):
    /// rows, controls.
    #[default]
    Item,
    /// The container radius (`shape-container`, 10 px): cards, panels.
    Container,
    /// Fully round (`r-full`): pills, dots, avatars.
    Pill,
}

impl Shape {
    /// The radius in logical pixels, from the tokens.
    pub fn radius(self) -> f32 {
        match self {
            Shape::Item => scale::SHAPE_ITEM,
            Shape::Container => scale::SHAPE_CONTAINER,
            Shape::Pill => scale::R_FULL,
        }
    }

    /// Applies the radius to an element: the one sanctioned way to round a
    /// corner outside the primitives, so every radius is a shape.
    pub fn rounded<D: gpui::Styled>(self, el: D) -> D {
        el.rounded(px(self.radius()))
    }
}
