//! Overlays: the command palette (card 12), the modal dialog, and later menus
//! and popovers. Overlays render inside the window; the app decides when they
//! are present and positions them.

pub(crate) mod card;
mod command_palette;
mod dialog;
mod settings;

pub use command_palette::*;
pub use dialog::{dialog, Dialog, DialogKind};
pub use settings::{settings_dialog, SettingsDialog, SettingsRow, SettingsSection, ShortcutEdit};

use aui_tokens::scale;
use gpui::{point, prelude::ParentElement, px, size, Anchor, Bounds, IntoElement, Pixels, Size};

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

/// Which side of its parent menu a submenu prefers to open on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmenuSide {
    /// Past the parent's right edge.
    Right,
    /// Past the parent's left edge.
    Left,
}

/// How far a submenu keeps from the window's edges: the parent menus' own
/// gap to their trigger ([`scale::SP_2`]).
pub const SUBMENU_INSET: f32 = scale::SP_2;

/// Where a submenu sits beside its parent menu, in window coordinates: the
/// one fit rule for every submenu, on every side.
///
/// `parent` is the parent panel's bounds and `row_top` the window `y` the
/// submenu's top would take to line its first row up with its parent row;
/// `wanted` is the submenu's own size (its height as last laid out),
/// `viewport` the window's.
///
/// - **Side**: `prefer`, one `gap` clear of the parent; if that side runs
///   past the window (less [`SUBMENU_INSET`]) and the other side fits, the
///   other side; if neither fits, the preferred side slid inside.
/// - **Height**: at most the window's less an inset above and below — a
///   taller submenu is capped there and scrolls.
/// - **Top**: the row-aligned top, shifted up until the bottom edge sits an
///   inset inside the window, and never above the top inset.
///
/// The returned bounds' height is the capped height; hand it to the panel
/// as its maximum.
pub fn submenu_seat(parent: Bounds<Pixels>, row_top: Pixels, wanted: Size<Pixels>, viewport: Size<Pixels>, gap: Pixels, prefer: SubmenuSide) -> Bounds<Pixels> {
    let inset = px(SUBMENU_INSET);
    let width = wanted.width;
    let right = parent.origin.x + parent.size.width + gap;
    let left = parent.origin.x - gap - width;
    let fits_right = right + width <= viewport.width - inset;
    let fits_left = left >= inset;
    let x = match prefer {
        SubmenuSide::Right if fits_right || !fits_left => right,
        SubmenuSide::Right => left,
        SubmenuSide::Left if fits_left || !fits_right => left,
        SubmenuSide::Left => right,
    };
    let x = x.min(viewport.width - inset - width).max(inset);
    let height = wanted.height.min(viewport.height - inset - inset).max(px(0.0));
    let y = row_top.min(viewport.height - inset - height).max(inset);
    Bounds::new(point(x, y), size(width, height))
}

#[cfg(test)]
mod submenu_seat_tests {
    use super::*;

    fn b(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    const WINDOW: (f32, f32) = (1440.0, 900.0);

    fn seat(parent: Bounds<Pixels>, row_top: f32, w: f32, h: f32, prefer: SubmenuSide) -> Bounds<Pixels> {
        submenu_seat(parent, px(row_top), size(px(w), px(h)), size(px(WINDOW.0), px(WINDOW.1)), px(4.0), prefer)
    }

    #[test]
    fn a_submenu_with_room_keeps_its_rows_top_and_side() {
        let s = seat(b(600.0, 100.0, 360.0, 200.0), 120.0, 300.0, 240.0, SubmenuSide::Right);
        assert_eq!(s, b(964.0, 120.0, 300.0, 240.0));
        let s = seat(b(600.0, 100.0, 360.0, 200.0), 120.0, 300.0, 240.0, SubmenuSide::Left);
        assert_eq!(s, b(296.0, 120.0, 300.0, 240.0));
    }

    /// The owner's screenshot: the model menu opens upward from the chip at
    /// the window bottom, the first row's submenu opens to the left at the
    /// row's top and ran down past the window. Its bottom now sits an inset
    /// inside the window.
    #[test]
    fn a_submenu_near_the_bottom_shifts_up_inside_the_window() {
        let parent = b(560.0, 620.0, 380.0, 220.0);
        let s = seat(parent, 650.0, 300.0, 380.0, SubmenuSide::Left);
        assert_eq!(f32::from(s.origin.y + s.size.height), WINDOW.1 - SUBMENU_INSET);
        assert_eq!(s.size.height, px(380.0));
        assert_eq!(s.origin.x, px(256.0));
    }

    #[test]
    fn a_submenu_taller_than_the_window_is_capped_and_scrolls() {
        let s = seat(b(560.0, 620.0, 380.0, 220.0), 650.0, 300.0, 2000.0, SubmenuSide::Right);
        assert_eq!(s.origin.y, px(SUBMENU_INSET));
        assert_eq!(f32::from(s.size.height), WINDOW.1 - 2.0 * SUBMENU_INSET);
    }

    #[test]
    fn a_submenu_never_rises_above_the_top_inset() {
        let s = seat(b(560.0, 0.0, 380.0, 220.0), -30.0, 300.0, 200.0, SubmenuSide::Right);
        assert_eq!(s.origin.y, px(SUBMENU_INSET));
    }

    #[test]
    fn a_submenu_flips_to_the_side_with_room() {
        // Hard against the left edge: Left would start at -244, so Right.
        let s = seat(b(40.0, 100.0, 380.0, 200.0), 120.0, 300.0, 200.0, SubmenuSide::Left);
        assert_eq!(s.origin.x, px(424.0));
        // Hard against the right edge: Right would end at 1754, so Left.
        let s = seat(b(1060.0, 100.0, 380.0, 200.0), 120.0, 300.0, 200.0, SubmenuSide::Right);
        assert_eq!(s.origin.x, px(756.0));
    }

    #[test]
    fn with_no_room_either_side_the_submenu_slides_inside() {
        let narrow = size(px(500.0), px(400.0));
        let s = submenu_seat(b(50.0, 50.0, 400.0, 200.0), px(60.0), size(px(300.0), px(100.0)), narrow, px(4.0), SubmenuSide::Right);
        assert_eq!(f32::from(s.origin.x + s.size.width), 500.0 - SUBMENU_INSET);
    }
}
