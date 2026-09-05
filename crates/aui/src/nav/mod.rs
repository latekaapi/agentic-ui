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
// card 21 exports
// card 22 exports
// card 23 exports
