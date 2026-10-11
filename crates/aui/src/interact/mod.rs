//! Interaction primitives on the v2 tokens: [`Interactive`], [`FluidList`],
//! [`Row`] and [`Surface`].
//!
//! These four elements own every hover, press, highlight, selection and
//! elevation state in the library, so later lanes can migrate the existing
//! components onto them without re-deriving the rules:
//!
//! - hover, selected and pressed are translucent overlays over whatever
//!   surface is beneath, never a surface step of their own;
//! - one highlight per list glides between rows on the moderate spring;
//! - bounded controls inset 1 px on press and never scale;
//! - the focus-visible ring lives on controls only, never on rows.

pub mod dropdown;
pub mod fluid_list;
pub mod interactive;
pub mod row;
pub mod surface;

pub use dropdown::{
    dropdown, dropdown_height, dropdown_label, dropdown_present, dropdown_separator, Dropdown, DropdownEntry, DropdownItem,
    DropdownSide, DropdownTrailing,
};
pub use fluid_list::{fluid_list, nearest, Axis, FluidHandle, FluidList};
pub use interactive::{interactive, interactive_state, press_inset, InteractState, Interactive, INTERACT_FOCUS_TAG};
pub use row::{row, Row};
pub use surface::{ambient_surface, menu_surface, nested_radius, surface, AmbientScope, Surface};

use aui_tokens::scale;
use gpui::px;

/// The corner shape of an interactive control or a surface.
///
/// Radii come from the shape pair (plus the full radius for pills); the
/// concentric rule — an inner radius is the outer radius minus the inset —
/// is [`nested_radius`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shape {
    /// The item radius (`shape-item`, 6 px): rows, controls, menus.
    #[default]
    Item,
    /// The container radius (`shape-container`, 8 px): cards, panels.
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
