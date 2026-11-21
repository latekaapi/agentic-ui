//! The Baaz sidebar on the v2 tokens, built to the approved mock
//! (`harness/docs/30-sidebar-spec`, the "Baaz sidebar v2" canvas).
//!
//! [`SidebarModel`] is plain data in, [`SidebarIntent`]s are intents out:
//! the screen owns no entities and no session state beyond what the caller
//! hands it every frame — the [`gpui::ListState`] it renders the
//! virtualised session list through. Every row, static chrome and virtual
//! session alike, hovers plainly with the one model
//! ([`crate::interact::paint`]). The session rows come from the shared [`Grouping`](crate::nav::Grouping)
//! shapes, flattened with [`flatten_sidebar`](crate::nav::flatten_sidebar).

mod menus;
mod model;
mod view;

pub use menus::{account_menu, browse_menu, browse_menu_size, session_menu, view_options_menu};
pub use model::{
    sample_model, Account, AccountAction, AccountStatus, BrowseEditing, BrowseFolder, BrowseIntent, BrowseMenu, BrowseMenuTarget, BrowseModel, MenuOffers,
    BrowseNaming, BrowsePin, BrowseSpace, BrowseTab, Bylines, EditingState, FolderAction, FolderKind, GroupKey, Mode, NavItem,
    PageAction, ProviderUsage, SidebarIntent, SidebarModel, SortKey, SpaceAction, TabAction, TaskFolderAction, UsageLine, ViewOption,
    ViewOptions,
};
pub use view::{list_row_height, mode_switch, screen_item_count, sidebar_screen, ModeSwitch, SidebarScreen, SIDEBAR_WIDTH};
