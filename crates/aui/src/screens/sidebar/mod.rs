//! The Baaz sidebar on the v2 tokens, built to the approved mock
//! (`harness/docs/30-sidebar-spec`, the "Baaz sidebar v2" canvas).
//!
//! [`SidebarModel`] is plain data in, [`SidebarIntent`]s are intents out:
//! the screen owns no entities and no session state beyond what the caller
//! hands it every frame — the [`gpui::ListState`] it renders the
//! virtualised session list through and the
//! [`FluidHandle`](crate::interact::FluidHandle) that lets the one gliding
//! highlight cover static chrome rows and virtual session rows alike. The
//! session rows come from the shared [`Grouping`](crate::nav::Grouping)
//! shapes, flattened with [`flatten_sidebar`](crate::nav::flatten_sidebar).

mod menus;
mod model;
mod view;

pub use menus::{account_menu, browse_menu, browse_menu_size, view_options_menu};
pub use model::{
    sample_model, Account, AccountAction, BrowseEditing, BrowseFolder, BrowseIntent, BrowseMenu, BrowseMenuTarget, BrowseModel,
    BrowseNaming, BrowsePin, BrowseSpace, BrowseTab, Bylines, EditingState, FolderAction, FolderKind, GroupKey, Mode, NavItem,
    PageAction, ProviderUsage, SidebarIntent, SidebarModel, SortKey, SpaceAction, TabAction, TaskFolderAction, UsageLine, ViewOption,
    ViewOptions,
};
pub use view::{collapsed_strip_width, list_row_height, mode_switch, screen_item_count, sidebar_screen, traffic_light_gap, ModeSwitch, SidebarScreen, SIDEBAR_WIDTH};
