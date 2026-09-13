//! Sidebar: session rows in every state, the sidebar and its collapsed
//! rail, the three groupings (status / project / date) with the view-options
//! menu, and the assistant's role sections (cards 20–23, spec §2).

mod folder_drop;
mod parts;
mod project_mark;
mod rail;
mod roles;
mod session_row;
mod sidebar;
mod types;
mod view_menu;
mod views;

pub use folder_drop::{folder_drop_card, FolderDropCard};
pub use project_mark::{project_mark, ProjectMark};
pub use parts::{
    chevron, chevron_sized, group_header, group_row, nav_item, sidebar_footer, sidebar_search, GroupHeader, GroupRow, NavItem,
    SidebarFooter, SidebarSearch,
};
pub use session_row::{compact_session_row, dense_field, session_row, CompactSessionRow, RowAction, SessionRow, DENSE_FIELD_H};
pub use types::{Activity, ActivityKind, MetaItem, SessionSummary};
pub use rail::{rail, Rail, RailItem, RAIL_WIDTH};
pub use sidebar::{sidebar, Sidebar, SidebarAccount, SidebarGroup, SidebarNav, SidebarNavItem, SIDEBAR_WIDTH};
pub use roles::{knowledge_card, project_row, role_section, role_session_row, KnowledgeCard, Project, ProjectRow, Role, RoleIntent, RoleSection, RoleSession, RoleSessionRow, SessionKind};
pub use view_menu::{view_menu, view_submenu, view_submenu_rows, MenuRow, ViewMenu, ViewSubmenu};
pub use views::{
    date_group_header, project_group_row, sidebar_view, DateGroup, DateGroupHeader, GroupAction, Grouping, ProjectGroup, ProjectGroupRow,
    SidebarView, StatusGroup,
};
