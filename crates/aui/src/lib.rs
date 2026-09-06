//! # aui
//!
//! The Agentic UI component library. Two macOS apps share it: an Orca-like
//! agent harness and a day-job assistant with the same three-pane shell.
//!
//! The crate is organised the way `docs/02-component-spec.md` is:
//!
//! | module | cards |
//! |---|---|
//! | [`shell`] | 10 app shell, 11 panel chrome |
//! | [`overlay`] | 12 command palette, menus, popovers, dialogs |
//! | [`feedback`] | 13 toasts and banners, status glyphs, spinners |
//! | [`nav`] | 20–23 sidebar, rows, views, assistant role sections |
//! | [`transcript`] | 30–38, 55 turns, thinking, activity, tool cards, approval, question/plan/todo, code/diff, summary/error/status, citations |
//! | [`composer`] | 40–42 composer, slash/mention menus, attachments |
//! | [`workbench`] | 50–54 terminal, browser, diff review, git, files and documents |
//! | [`data`] | shared data-display primitives: buttons, chips, pills, tags, dots, kbd |
//!
//! Every component takes data in (mostly [`aui_protocol`] types) and emits
//! intents out; there is no I/O inside this crate. Colours, sizes and
//! durations come from [`aui_tokens`]; motion from [`aui_motion`]; glyphs from
//! [`aui_icons`].
//!
//! ```ignore
//! gpui_kit::application().with_assets(aui::assets::AuiAssets).run(|cx| {
//!     aui::init(aui::tokens::ThemeKind::Dark, cx);
//!     // open windows…
//! });
//! ```

#![warn(missing_docs)]

pub mod assets;
pub mod composer;
pub mod data;
pub mod feedback;
pub mod keys;
pub mod nav;
pub mod overlay;
pub mod shell;
pub mod transcript;
pub mod util;
pub mod workbench;

pub use aui_icons as icons;
pub use aui_motion as motion;
pub use aui_protocol as protocol;
pub use aui_tokens as tokens;

use gpui::App;

/// Initialises gpui-kit, installs the design tokens, fonts and theme, and
/// binds the library's keyboard actions ([`keys`]). Call once at application
/// start, before opening any window.
pub fn init(theme: aui_tokens::ThemeKind, cx: &mut App) {
    gpui_kit::init(cx);
    aui_tokens::AuiTheme::init(theme, cx);
    keys::bind(cx);
}
