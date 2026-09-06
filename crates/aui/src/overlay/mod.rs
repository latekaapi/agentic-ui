//! Overlays: the command palette (card 12), and later menus, popovers and
//! dialogs. Overlays render inside the window; the app decides when they are
//! present and positions them.

mod command_palette;

pub use command_palette::*;

/// The priority every popover in the library paints at. Deferred draws are
/// painted in priority order, so a menu opened from inside another popover can
/// ask for [`POPOVER_LAYER`] + 1 and land on top of it.
pub const POPOVER_LAYER: usize = 1;

/// Lifts an anchored overlay — a menu, a popover, a hover card — out of the
/// paint order of the surface it hangs off.
///
/// gpui paints in tree order, so an element rendered inline inside a card is
/// covered by the card's later siblings and by the borders, rings and shadows
/// of the surfaces around it; the composer's `+` menu came out with the card's
/// focus ring drawn straight across it. `deferred` moves the child to the end
/// of the frame while leaving its *layout* where it was, so the overlay stays
/// anchored to whatever positioned it (an `absolute` offset from a `relative`
/// holder) and keeps its enter and exit motion, but paints above everything
/// else in the window.
///
/// Wrap the overlay at the point where it is anchored, not where it is built:
/// the stateless menus ([`crate::composer::CommandMenu`],
/// [`crate::composer::MentionPicker`], [`crate::nav::ViewMenu`]) do not know
/// how they are placed, so the caller that positions them over other content
/// is the one that has to call this.
pub fn popover_layer(child: impl gpui::IntoElement) -> gpui::Deferred {
    gpui::deferred(child).with_priority(POPOVER_LAYER)
}
