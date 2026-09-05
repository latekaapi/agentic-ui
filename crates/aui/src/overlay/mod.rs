//! Overlays: the command palette (card 12), and later menus, popovers and
//! dialogs. Overlays render inside the window; the app decides when they are
//! present and positions them.

mod command_palette;

pub use command_palette::*;
