//! App shell: the three-column layout with one 44 px header cell per column,
//! continuous dividers, the docked composer, panel chrome, tab strips with
//! the sliding indicator and drop zones (cards 10–11, spec §1.1–1.2).

mod app_shell;
mod composer_dock;
mod drag_region;
mod drop_zones;
mod header;
mod panel_header;
mod resize_handle;
mod tab_strip;

pub use app_shell::{
    app_shell, clamp_right_width, clamp_sidebar_width, AppShell, RAIL_WIDTH, RAIL_WIDTH_WITH_LIGHTS, RIGHT_MAX_WIDTH, RIGHT_MIN_WIDTH,
    RIGHT_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH,
};
pub use drag_region::{drag_region, DragRegion};
pub use composer_dock::{docked_composer, DockedComposer, DockedComposerIntent};
pub use drop_zones::{drop_zones, tab_ghost, DropZone, DropZones, TabGhost};
pub use header::{
    centre_header, header_cell, right_header, sidebar_header, traffic_light_position, CentreHeader, HeaderCell, RightHeader, SidebarHeader,
    NATIVE_LIGHTS_WIDTH, NATIVE_LIGHTS_X, NATIVE_LIGHT_DIAM, NATIVE_LIGHT_STRIDE,
};
pub use resize_handle::{drag_capture_overlay, resize_handle, DragCaptureOverlay, ResizeHandle, RESIZE_HANDLE_W};
pub use panel_header::{panel_header, PanelHeader};
pub use tab_strip::{tab_strip, TabItem, TabStrip};
