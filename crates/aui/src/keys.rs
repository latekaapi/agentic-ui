//! The library's keyboard actions and their default bindings.
//!
//! Components never read raw keystrokes. They declare a [`key_context`] on the
//! element that owns a piece of keyboard behaviour and handle the actions
//! below with `on_action`; the application binds the keys once, through
//! [`bind`], which [`crate::init`] already calls.
//!
//! | keys | action | context |
//! |---|---|---|
//! | `cmd-b` | [`ToggleSidebar`] | anywhere |
//! | `cmd-k` | [`TogglePalette`] | anywhere |
//! | `cmd-\` | [`ToggleRightPane`] | anywhere |
//! | `up` / `down` | [`SelectPrev`] / [`SelectNext`] | [`MENU_CONTEXT`] |
//! | `enter` | [`Confirm`] | [`MENU_CONTEXT`] |
//! | `escape` | [`Cancel`] | [`MENU_CONTEXT`], [`APPROVAL_CONTEXT`], [`ROOT_CONTEXT`] |
//! | `y` / `a` / `n` | [`ApproveOnce`] / [`ApproveAlways`] / [`Deny`] | [`APPROVAL_CONTEXT`] |
//! | `tab` / `shift-tab` | [`FocusNext`] / [`FocusPrev`] | [`ROOT_CONTEXT`] |
//!
//! [`key_context`]: gpui::InteractiveElement::key_context

use gpui::{actions, App, Global, KeyBinding};

actions!(
    aui,
    [
        /// Swap the sidebar for its collapsed rail, and back.
        ToggleSidebar,
        /// Open or close the right pane.
        ToggleRightPane,
        /// Open or close the command palette.
        TogglePalette,
        /// Run the highlighted row of a menu or the palette.
        Confirm,
        /// Close the overlay that has the keyboard.
        Cancel,
        /// Move the highlight down one row.
        SelectNext,
        /// Move the highlight up one row.
        SelectPrev,
        /// Allow the pending request once.
        ApproveOnce,
        /// Allow it and remember the rule.
        ApproveAlways,
        /// Refuse the pending request.
        Deny,
        /// Move the keyboard to the next tab stop.
        FocusNext,
        /// Move the keyboard to the previous tab stop.
        FocusPrev,
    ]
);

/// The context a screen puts on its outermost element: it owns Tab and the
/// escape that closes whatever is open.
pub const ROOT_CONTEXT: &str = "AuiRoot";
/// The context a menu, picker or the command palette puts on itself while it
/// holds the keyboard: it owns the arrows, return and escape.
pub const MENU_CONTEXT: &str = "AuiMenu";
/// The context a pending approval card puts on itself while it is focused: it
/// owns Y, A and N.
pub const APPROVAL_CONTEXT: &str = "AuiApproval";

/// Installs the default bindings. Called by [`crate::init`]; an application
/// that wants a different keymap can rebind the same actions afterwards.
pub fn bind(cx: &mut App) {
    let dismiss = format!("{MENU_CONTEXT} || {APPROVAL_CONTEXT} || {ROOT_CONTEXT}");
    cx.bind_keys([
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-k", TogglePalette, None),
        KeyBinding::new("cmd-\\", ToggleRightPane, None),
        KeyBinding::new("up", SelectPrev, Some(MENU_CONTEXT)),
        KeyBinding::new("down", SelectNext, Some(MENU_CONTEXT)),
        KeyBinding::new("enter", Confirm, Some(MENU_CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(&dismiss)),
        KeyBinding::new("y", ApproveOnce, Some(APPROVAL_CONTEXT)),
        KeyBinding::new("a", ApproveAlways, Some(APPROVAL_CONTEXT)),
        KeyBinding::new("n", Deny, Some(APPROVAL_CONTEXT)),
        KeyBinding::new("tab", FocusNext, Some(ROOT_CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrev, Some(ROOT_CONTEXT)),
    ]);
}

/// The `:focus-visible` approximation. gpui has no notion of it, so the library
/// keeps one window-wide flag: the keyboard arms it (a key on a focusable
/// control, or a [`FocusNext`] / [`FocusPrev`] the application handles), a
/// mouse press on a control disarms it, and [`crate::data::Button`] draws its
/// accent ring only while it is armed. A mouse press on something that is not a
/// control does not disarm it, which is the one place the approximation shows.
#[derive(Default)]
struct KeyboardNav(bool);

impl Global for KeyboardNav {}

/// Whether the keyboard, rather than the mouse, last moved the focus.
pub fn keyboard_nav(cx: &mut App) -> bool {
    cx.default_global::<KeyboardNav>().0
}

/// Arms or disarms the keyboard-focus flag. Call it with `true` from anything
/// that moves focus with the keyboard; the focus ring follows.
pub fn set_keyboard_nav(on: bool, cx: &mut App) {
    if cx.default_global::<KeyboardNav>().0 != on {
        cx.set_global(KeyboardNav(on));
    }
}
