//! App shell: the three-column layout with one 44 px header cell per column,
//! continuous dividers, the docked composer, panel chrome, tab strips with
//! the sliding indicator and drop zones (cards 10–11, spec §1.1–1.2).

mod app_shell;
mod composer_dock;
mod drop_zones;
mod header;
mod panel_header;
mod tab_strip;

pub use app_shell::{app_shell, AppShell, RAIL_WIDTH, RAIL_WIDTH_WITH_LIGHTS, RIGHT_WIDTH, SIDEBAR_WIDTH};
pub use composer_dock::{docked_composer, DockedComposer, DockedComposerIntent};
pub use drop_zones::{drop_zones, tab_ghost, DropZone, DropZones, TabGhost};
pub use header::{centre_header, header_cell, right_header, sidebar_header, CentreHeader, HeaderCell, RightHeader, SidebarHeader};
pub use panel_header::{panel_header, PanelHeader};
pub use tab_strip::{tab_strip, TabItem, TabStrip};
