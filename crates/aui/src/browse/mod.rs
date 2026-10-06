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

/// What the address line shows: the host in `ink`, the path in `ink-3`, and
/// — for local pages — the port tag and the `>_` glyph instead of the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// The host (`pgi.udiseplus.gov.in`, `localhost`).
    pub host: SharedString,
    /// The path including any query or hash (`/#/home`, `/overview`, `/`).
    pub path: SharedString,
    /// True for `localhost`, `127.0.0.1`, `*.localhost` and `*.test`.
    pub local: bool,
    /// The port when the URL names one (`5173`).
    pub port: Option<u16>,
    /// True for `https:` (and for bare hosts, which are assumed `https:`).
    pub secure: bool,
}

impl Address {
    /// An address from its parts; prefer [`split_address`] for URLs.
    pub fn new(
        host: impl Into<SharedString>,
        path: impl Into<SharedString>,
        local: bool,
        port: Option<u16>,
        secure: bool,
    ) -> Self {
        Self { host: host.into(), path: path.into(), local, port, secure }
    }
}

/// One pin in the 4-up grid: a 34 px tile showing `letter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// Stable id, reported by `on_select`.
    pub id: SharedString,
    /// The single letter (or short mark) drawn on the tile.
    pub letter: SharedString,
    /// The page title; the tile's accessible name is `Pinned: <title>`.
    pub title: SharedString,
}

impl Pin {
    /// A pin with an id, a tile letter and a title.
    pub fn new(id: impl Into<SharedString>, letter: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self { id: id.into(), letter: letter.into(), title: title.into() }
    }
}

/// One tab row: a 16 px letter tile, the title, an optional port tag and —
/// on hover or when active — the close `×`.
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
}

impl TabRowData {
    /// An inactive top-level tab with no port tag.
    pub fn new(title: impl Into<SharedString>, letter: impl Into<SharedString>) -> Self {
        Self { title: title.into(), letter: letter.into(), active: false, port: None, depth: 0 }
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
