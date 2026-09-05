//! Sidebar: session rows in every state, the sidebar and its collapsed
//! rail, the three groupings (status / project / date) with the view-options
//! menu, and the assistant's role sections (cards 20–23, spec §2).

mod parts;
mod rail;
mod roles;
mod session_row;
mod sidebar;
mod types;
mod view_menu;
mod views;

pub use parts::{chevron, group_header, group_row, nav_item, sidebar_footer, GroupHeader, GroupRow, NavItem, SidebarFooter};
pub use session_row::{compact_session_row, session_row, CompactSessionRow, RowAction, SessionRow};
pub use types::{Activity, ActivityKind, MetaItem, SessionSummary};
pub use rail::{rail, Rail, RailItem, RAIL_WIDTH};
pub use sidebar::{sidebar, Sidebar, SidebarAccount, SidebarGroup, SidebarNav, SidebarNavItem, SIDEBAR_WIDTH};
pub use roles::{knowledge_card, project_row, role_section, role_session_row, KnowledgeCard, Project, ProjectRow, Role, RoleIntent, RoleSection, RoleSession, RoleSessionRow, SessionKind};
pub use view_menu::{view_menu, view_submenu, MenuRow, ViewMenu, ViewSubmenu};
pub use views::{
    date_group_header, project_group_row, sidebar_view, DateGroup, DateGroupHeader, Grouping, ProjectGroup, ProjectGroupRow, SidebarView,
    StatusGroup,
};
