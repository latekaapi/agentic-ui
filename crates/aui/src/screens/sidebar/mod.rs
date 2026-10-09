//! The Baaz sidebar, redesigned on the v2 tokens.
//!
//! [`SidebarModel`] is plain data in, [`SidebarIntent`]s are intents out:
//! the screen owns no entities and no session state beyond the caller-owned
//! [`gpui::ListState`] it renders the virtualised session list through. The
//! session rows come from the shared [`Grouping`](crate::nav::Grouping)
//! shapes, flattened with [`flatten_sidebar`](crate::nav::flatten_sidebar)
//! exactly like [`virtual_sidebar_view`](crate::nav::virtual_sidebar_view).

mod menus;
mod model;
mod view;

pub use menus::{account_menu, project_menu};
pub use model::{
    sample_model, Account, BrowseFolder, BrowseModel, EditingState, Mode, NavItem, SidebarIntent,
    SidebarModel, Workspace,
};
pub use view::{screen_item_count, sidebar_screen, SidebarScreen, SIDEBAR_WIDTH};
