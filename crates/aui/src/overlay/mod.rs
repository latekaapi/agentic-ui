//! Overlays: the command palette (card 12), the modal dialog, and later menus
//! and popovers. Overlays render inside the window; the app decides when they
//! are present and positions them.

mod card;
mod command_palette;
mod dialog;
mod settings;

pub use command_palette::*;
pub use dialog::{dialog, Dialog, DialogKind};
pub use settings::{settings_dialog, SettingsDialog, SettingsRow, SettingsSection, ShortcutEdit};

use aui_tokens::scale;
use gpui::{point, prelude::ParentElement, px, Anchor, Bounds, IntoElement, Pixels};

/// How far an overlay dims the window behind it.
///
/// `.scrim{background:linear-gradient(180deg,rgba(0,0,0,.25),rgba(0,0,0,.45))}`
/// — [`palette_scrim`] runs the gradient between the two stops; the modal
/// [`Dialog`] covers the whole window, where the heavier bottom stop turns a
/// light theme into a grey slab, so it uses the flat top stop. Both live here
/// rather than as a literal in each file.
pub(crate) const SCRIM_TINT_TOP: f32 = 0.25;
/// The bottom stop of the scrim gradient, and the modal scrim's flat tint.
pub(crate) const SCRIM_TINT_BOTTOM: f32 = 0.45;

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

/// Which side of the trigger an [`anchored_menu`] hangs off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSide {
    /// Below the trigger: the menu's top edge sits one gap under the
    /// trigger's bottom edge.
    Below,
    /// Above the trigger: the menu's bottom edge sits one gap over the
    /// trigger's top edge.
    Above,
}

/// Which vertical edge of the trigger an [`anchored_menu`] aligns to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAlign {
    /// The menu's leading edge meets the trigger's leading edge.
    Start,
    /// The menu's trailing edge meets the trigger's trailing edge.
    End,
}

/// Seats a trigger menu at its trigger, inside the window.
///
/// The one seat rule for every trigger menu: open below-start of the trigger,
/// flip above near the window bottom, slide to stay inside; never from the
/// mouse, a row rect, or a fixed corner.
///
/// `trigger` is the trigger's bounds in window coordinates, measured in
/// prepaint — the caller records them with
/// [`gpui::Div::on_children_prepainted`] on the trigger (or on its tray
/// button) and hands them back here. The seat is the trigger's bottom-left
/// (`Below`/`Start`), bottom-right (`Below`/`End`), top-left (`Above`/`Start`)
/// or top-right (`Above`/`End`) corner plus one [`scale::SP_2`] gap, with the
/// matching menu corner ([`Anchor::TopLeft`], `TopRight`, `BottomLeft`,
/// `BottomRight`) put on it. Position mode is window coordinates.
///
/// The seat keeps gpui's default `SwitchAnchor` fit mode, which flips the
/// side when the menu would overflow and then slides it inside the window.
/// gpui's fit modes are exclusive — calling
/// `snap_to_window_with_margin` would trade the flip for a margined slide —
/// so the flip (the behaviour the window bottom needs) wins and the slide
/// clamps to the window edge. The whole seat rides in [`popover_layer`], so
/// it paints above the surface that opened it; the `Anchored`-inside-
/// `deferred` fit is covered by `anchored_menu_keeps_its_window_fit` in
/// `crates/aui/tests/anchored_menu.rs`.
pub fn anchored_menu(trigger: Bounds<Pixels>, side: MenuSide, align: MenuAlign, menu: impl IntoElement) -> impl IntoElement {
    let gap = px(scale::SP_2);
    let y = match side {
        MenuSide::Below => trigger.origin.y + trigger.size.height + gap,
        MenuSide::Above => trigger.origin.y - gap,
    };
    let x = match align {
        MenuAlign::Start => trigger.origin.x,
        MenuAlign::End => trigger.origin.x + trigger.size.width,
    };
    let anchor = match (side, align) {
        (MenuSide::Below, MenuAlign::Start) => Anchor::TopLeft,
        (MenuSide::Below, MenuAlign::End) => Anchor::TopRight,
        (MenuSide::Above, MenuAlign::Start) => Anchor::BottomLeft,
        (MenuSide::Above, MenuAlign::End) => Anchor::BottomRight,
    };
    popover_layer(gpui::anchored().anchor(anchor).position(point(x, y)).child(menu))
}
