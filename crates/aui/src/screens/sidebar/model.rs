//! The sidebar view model: plain data in, intents out.
//!
//! The host builds one [`SidebarModel`] per frame from its own state (the
//! session list, the browse tabs, the account) and receives
//! [`SidebarIntent`]s back through the screen's single `on_intent` handler.
//! Everything here is `Clone` with no gpui entities, so the host can cache
//! and compare models freely. Session data stays in the shared
//! [`Grouping`](crate::nav::Grouping) shapes; this module only adds the
//! chrome around them.

use std::rc::Rc;

use aui_icons::IconName;
use aui_tokens::AgentState;
use gpui::SharedString;

use crate::browse::{Address, Pin, TabRowData};
use crate::nav::{DateGroup, Grouping, RowStatusKind, SessionSummary};

/// Which list the sidebar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The session list: nav rows plus the grouped sessions.
    Sessions,
    /// The browse list: pins, tabs and folders from [`BrowseModel`].
    Browse,
}

/// The workspace tile in the header: the project mark, the name and the
/// chevron that opens the project menu, as one row.
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    /// The workspace name.
    pub name: SharedString,
    /// The mark's initial.
    pub initial: SharedString,
    /// The mark's label colour index (see `Palette::label`).
    pub tint: u8,
    /// Whether the project menu is open under the header.
    pub menu_open: bool,
}

/// One primary nav row under the mode control.
#[derive(Debug, Clone, PartialEq)]
pub struct NavItem {
    /// Stable id, reported by [`SidebarIntent::Nav`].
    pub id: SharedString,
    /// The visible label.
    pub label: SharedString,
    /// The leading glyph, on the 16 px icon axis.
    pub icon: IconName,
    /// The trailing count, if the row carries one.
    pub count: Option<u32>,
    /// The row is coming soon: a quiet "Soon" marker instead of a count.
    pub soon: bool,
}

/// The account footer: avatar, name, plan, usage and its menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    /// The person's name.
    pub name: SharedString,
    /// The plan name.
    pub plan: SharedString,
    /// Fraction of the allowance used, `0.0..=1.0`.
    pub usage: f32,
    /// The avatar's initial.
    pub avatar: SharedString,
    /// The avatar's label colour index (see `Palette::label`).
    pub tint: u8,
    /// Whether the account menu is open above the footer.
    pub menu_open: bool,
}

/// A rename in flight: which session, and the draft the field shows.
#[derive(Debug, Clone, PartialEq)]
pub struct EditingState {
    /// The session being renamed.
    pub session: SharedString,
    /// The draft text in the field.
    pub draft: SharedString,
}

/// One browse folder with its tabs.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseFolder {
    /// The folder name.
    pub name: SharedString,
    /// Whether its tabs are visible.
    pub open: bool,
    /// The tabs inside the folder.
    pub tabs: Vec<TabRowData>,
}

/// Browse mode's rows, rendered with the shared `browse/rows.rs` builders.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseModel {
    /// The address line's address.
    pub address: Address,
    /// The pin grid's pins.
    pub pins: Vec<Pin>,
    /// The top-level tabs.
    pub tabs: Vec<TabRowData>,
    /// The folders with their tabs.
    pub folders: Vec<BrowseFolder>,
}

/// The whole sidebar as plain data. The caller owns the matching
/// [`gpui::ListState`] (see [`super::screen_item_count`]) and hands both to
/// [`super::sidebar_screen`] every frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarModel {
    /// The workspace tile.
    pub workspace: Workspace,
    /// Which list is showing.
    pub mode: Mode,
    /// `(sessions, tabs)`: the counts on the segmented control.
    pub counts: (u32, u32),
    /// The search field's text, when the field holds any.
    pub search: Option<SharedString>,
    /// The primary nav rows.
    pub nav: Vec<NavItem>,
    /// The session data, in the shared grouping shapes.
    pub grouping: Rc<Grouping>,
    /// The selected session id.
    pub selected: Option<SharedString>,
    /// The hovered session id: its row swaps the trailing slot for actions.
    pub hovered: Option<SharedString>,
    /// The rename in flight, if any.
    pub editing: Option<EditingState>,
    /// Whether sessions are still loading: three skeleton rows, no text.
    pub loading: bool,
    /// Whether the sidebar is collapsed: only the 12 px strip renders.
    pub collapsed: bool,
    /// The account footer.
    pub account: Account,
    /// Browse mode's rows.
    pub browse: Option<BrowseModel>,
}

/// What the person asked the app to do. The screen never searches, opens
/// menus or renames anything itself: it reports, the host acts.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarIntent {
    /// The workspace row was clicked: open the project menu.
    OpenProjectMenu,
    /// The search row was clicked: focus the search field.
    Search,
    /// The new-session row was clicked: start a session.
    NewSession,
    /// A nav row was clicked; the argument is the nav id.
    Nav(SharedString),
    /// The segmented control switched lists.
    SwitchMode(Mode),
    /// A session row was clicked; the argument is the session id.
    Select(SharedString),
    /// A group header was clicked; the argument is the group label.
    Toggle(SharedString),
    /// A section's view-options (`…`) button was clicked.
    ViewOptions,
    /// A section's action button was clicked.
    GroupAction(SharedString, crate::nav::GroupAction),
    /// A session row's hover action was clicked.
    RowAction(SharedString, crate::nav::RowAction),
    /// A rename was committed; the arguments are the session and the text.
    Rename(SharedString, SharedString),
    /// The footer was clicked: open the account menu.
    Account,
    /// The collapsed strip (or its control) was clicked.
    Collapse,
    /// The pointer entered or left a session row.
    Hover(Option<SharedString>),
}

/// The gallery and test sample: workspace "harness", Skills (23) and
/// Automations (soon), Pinned (2) / Today (4) / Yesterday (3) with one
/// running, one waiting and one failed session, "Bharani · Max" at 78 %,
/// and the browse rows' sample data.
pub fn sample_model() -> SidebarModel {
    let pinned = DateGroup::new(
        "Pinned",
        vec![
            SessionSummary::new("design-v2", "Design v2 tokens", AgentState::Idle, "12m").pinned(),
            SessionSummary::new("sidebar-states", "Sidebar states card", AgentState::Idle, "26m")
                .pinned(),
        ],
    );
    let today = DateGroup::new(
        "Today",
        vec![
            SessionSummary::new("browser-audit", "Browser integration audit", AgentState::Running, "now")
                .pulse()
                .preview("running the visual probe")
                .status(RowStatusKind::Working, "2m"),
            SessionSummary::new("checklist", "Checklist run", AgentState::Waiting, "3h")
                .pulse()
                .preview("approve `cargo build`")
                .status(RowStatusKind::NeedsApproval, ""),
            SessionSummary::new("token-ladder", "Token ladder audit", AgentState::Idle, "2h"),
            SessionSummary::new("relay-capture", "Relay capture pass", AgentState::Idle, "3h"),
        ],
    );
    let yesterday = DateGroup::new(
        "Yesterday",
        vec![
            SessionSummary::new("fluid-glide", "Fluid list glide", AgentState::Idle, "1d"),
            SessionSummary::new("rename-fit", "Rename field fit", AgentState::Failed, "1d")
                .preview("typecheck failed in sidebar view")
                .status(RowStatusKind::Failed, "1d"),
            SessionSummary::new("skeleton-rows", "Skeleton rows", AgentState::Idle, "1d"),
        ],
    );
    SidebarModel {
        workspace: Workspace { name: "harness".into(), initial: "h".into(), tint: 2, menu_open: false },
        mode: Mode::Sessions,
        counts: (9, 5),
        search: None,
        nav: vec![
            NavItem { id: "skills".into(), label: "Skills".into(), icon: IconName::Book, count: Some(23), soon: false },
            NavItem { id: "automations".into(), label: "Automations".into(), icon: IconName::Zap, count: None, soon: true },
        ],
        grouping: Rc::new(Grouping::Date(vec![pinned, today, yesterday])),
        selected: Some("design-v2".into()),
        hovered: None,
        editing: None,
        loading: false,
        collapsed: false,
        account: Account {
            name: "Bharani".into(),
            plan: "Max".into(),
            usage: 0.78,
            avatar: "B".into(),
            tint: 5,
            menu_open: false,
        },
        browse: Some(BrowseModel {
            address: Address::new(
                "pgi.udiseplus.gov.in",
                "/#/home",
                false,
                None,
                true,
                Some("https".to_owned()),
            ),
            pins: vec![
                Pin::new("u", "U", "Unified dashboard").position(1),
                Pin::new("p", "P", "Performance grades").position(2),
                Pin::new("d", "D", "Open data portal").position(3),
                Pin::new("n", "N", "Survey reports").position(4),
            ],
            tabs: vec![
                TabRowData::new("State scores 2023-24", "S").active().position(1),
                TabRowData::new("Dashboard 2024-25", "D").position(2),
                TabRowData::new("State report cards", "S").position(3),
            ],
            folders: vec![BrowseFolder {
                name: "Karnataka review".into(),
                open: true,
                tabs: vec![
                    TabRowData::new("Board results 2025", "B").depth(1).position(4),
                    TabRowData::new("Scheme review · Karnataka", "S").depth(1).position(5),
                ],
            }],
        }),
    }
}
