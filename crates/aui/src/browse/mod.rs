//! The Browse sidebar: the only chrome of the Browse surface (the page
//! beside it is edge to edge). Top to bottom: the traffic-light row with the
//! back/forward/reload cluster ([`browse_nav_cluster`]), the address line
//! ([`address_line`]), the pin grid ([`pin_grid`]), tab and folder rows
//! ([`tab_row`], [`folder_row`]), task folders with status faces
//! ([`task_folder_row`], [`task_face`]) and the footer with the space chip
//! ([`browse_footer`]).
//!
//! Row sizing and hover-tint conventions follow `crate::nav` (30 px rows,
//! `surface-2` hover via [`aui_motion::tint_fade`](aui_motion), `surface-3`
//! for the active row); colours come from [`aui_tokens`] only.

mod address;
mod face;
/// The browser's Field: one input for addresses, tabs, history, commands, Ask and Task.
pub mod field;
mod footer;
mod rows;

pub use address::{address_line, split_address, AddressLine};
pub use face::{task_face, TaskFaceGlyph};
pub use footer::{browse_footer, BrowseFooter};
pub use rows::{
    browse_nav_cluster, folder_row, pin_grid, tab_row, task_folder_row, BrowseNavCluster, FolderRow, PinGrid, TabRow,
    TaskFolderRow,
};

use gpui::SharedString;

/// The Browse sidebar's fixed width.
pub const BROWSE_SIDEBAR_WIDTH: f32 = 248.0;

/// What the address line shows: the host in `ink`, the path in `ink-3`, the
/// port tag for non-default ports, and the glyph: `✦` for internal pages,
/// `>_` for local ones, the lock when secure, the warning lock otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// The host (`pgi.udiseplus.gov.in`, `localhost`, `settings`).
    pub host: SharedString,
    /// The path including any query or hash (`/#/home`, `/overview`, `/`).
    pub path: SharedString,
    /// True for loopbacks (`localhost` with an optional trailing dot,
    /// `*.localhost`, `*.test`, `127.0.0.0/8`, `0.0.0.0`, `[::1]`).
    pub local: bool,
    /// The port when the URL names a non-default one (`5173`); default
    /// ports (80 for `http`, 443 for `https`) are hidden (`None`).
    pub port: Option<u16>,
    /// True for `https:` (and for bare hosts, which are assumed `https:`),
    /// and always true for internal schemes.
    pub secure: bool,
    /// The lowercased URL scheme (`https`, `chrome`, …); `None` for bare
    /// hosts, which are assumed `https:`.
    pub scheme: Option<String>,
}

impl Address {
    /// An address from its parts; prefer [`split_address`] for URLs.
    pub fn new(
        host: impl Into<SharedString>,
        path: impl Into<SharedString>,
        local: bool,
        port: Option<u16>,
        secure: bool,
        scheme: Option<String>,
    ) -> Self {
        Self { host: host.into(), path: path.into(), local, port, secure, scheme }
    }

    /// True for the internal schemes (`about:`, `chrome:`, `file:`,
    /// `data:`): those render the `✦` glyph and never the insecure state.
    pub fn is_internal(&self) -> bool {
        matches!(self.scheme.as_deref(), Some("about" | "chrome" | "file" | "data"))
    }
}

/// One pin in the 4-up grid: a 34 px tile showing `letter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// Stable id, reported by `on_select`.
    pub id: SharedString,
    /// The single letter (or short mark) drawn on the tile.
    pub letter: SharedString,
    /// The page title; the tile's accessible name is `Pinned <n>: <title>`.
    pub title: SharedString,
    /// The pin's position in the grid, used in the accessible name.
    pub position: usize,
}

impl Pin {
    /// A pin with an id, a tile letter and a title.
    pub fn new(id: impl Into<SharedString>, letter: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self { id: id.into(), letter: letter.into(), title: title.into(), position: 0 }
    }

    /// Sets the pin's grid position, used in the accessible name.
    pub fn position(mut self, position: usize) -> Self {
        self.position = position;
        self
    }
}

/// One tab row: a 16 px letter tile, the title, an optional port tag and the
/// close `×` (always in the tree; opacity 0 until hovered or active).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabRowData {
    /// The tab title (truncated, reported by `on_select` / `on_close`).
    pub title: SharedString,
    /// The letter drawn on the 16 px tile.
    pub letter: SharedString,
    /// Active tabs sit on `surface-3` and always show the close `×`.
    pub active: bool,
    /// An optional mono port tag after the title.
    pub port: Option<u16>,
    /// Nesting depth; each level indents 22 px.
    pub depth: u8,
    /// The row's position, passed in by the caller and used in the
    /// accessible names (`Tab <n>: <title>`, `Close tab <n>: <title>`).
    pub position: usize,
}

impl TabRowData {
    /// An inactive top-level tab with no port tag.
    pub fn new(title: impl Into<SharedString>, letter: impl Into<SharedString>) -> Self {
        Self { title: title.into(), letter: letter.into(), active: false, port: None, depth: 0, position: 0 }
    }

    /// Marks the tab active (tinted `surface-3`, close always visible).
    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    /// Adds the mono port tag after the title.
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Nests the row one level deeper (22 px per level).
    pub fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Sets the row's position, used in the accessible names.
    pub fn position(mut self, position: usize) -> Self {
        self.position = position;
        self
    }
}

/// What a task folder is doing, drawn as the 16 px status face in place of
/// the folder glyph. Colour appears only where something needs attention:
/// `NeedsYou` draws in `warning`, `Done` in `success`, the rest in `ink`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskFace {
    /// Reading: two dots mid-face.
    Reading,
    /// Typing: two dots low.
    Typing,
    /// Needs you: a small raised line at the right (a wave), warning circle.
    NeedsYou,
    /// Paused: two short horizontal lines.
    Paused,
    /// Done: a `✓` in `success` inside the circle.
    Done,
}

impl TaskFace {
    /// The quiet status word the row's end carries (`done` on its own; the
    /// caller composes `done · N tabs` with the tab count).
    pub fn status_word(self) -> &'static str {
        match self {
            TaskFace::Reading => "reading",
            TaskFace::Typing => "typing",
            TaskFace::NeedsYou => "needs you",
            TaskFace::Paused => "paused",
            TaskFace::Done => "done",
        }
    }
}
