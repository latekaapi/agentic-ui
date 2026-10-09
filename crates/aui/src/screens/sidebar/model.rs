//! The sidebar view model: plain data in, intents out.
//!
//! The host builds one [`SidebarModel`] per frame from its own state and
//! receives [`SidebarIntent`]s back through the screen's single `on_intent`
//! handler. Everything here is `Clone` with no gpui entities, so the host
//! can cache and compare models freely. Session data stays in the shared
//! [`Grouping`](crate::nav::Grouping) shapes (projects with their sessions,
//! folds and branches); this module adds the chrome around them and the
//! Browse mode's own rows.
//!
//! The shapes mirror the approved mock (`harness/docs/30-sidebar-spec`,
//! the "Baaz sidebar v2" canvas): [`sample_model`] is that mock's data, so
//! a gallery capture and the mock PNG can be diffed one to one.

use std::rc::Rc;

use aui_icons::IconName;
use aui_tokens::AgentState;
use gpui::SharedString;

use crate::nav::{Grouping, ProjectGroup, RowStatusKind, SessionSummary};

/// Which list the sidebar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The session list: search, new session, nav, pinned, projects.
    Sessions,
    /// The browser list: address, page controls, pins, tabs, folders,
    /// spaces, new tab.
    Browse,
}

/// How much every session row says under its title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bylines {
    /// Every row is two lines: the status sentence where the session has
    /// one, otherwise its byline (last ask · last result).
    #[default]
    All,
    /// Only rows with a status sentence (running, waiting, failed) take
    /// the second line; idle rows stay one line.
    StatusOnly,
}

/// One primary nav row under New session.
#[derive(Debug, Clone, PartialEq)]
pub struct NavItem {
    /// Stable id, reported by [`SidebarIntent::Nav`].
    pub id: SharedString,
    /// The visible label.
    pub label: SharedString,
    /// The leading glyph, on the icon axis.
    pub icon: IconName,
    /// The trailing count, if the row carries one.
    pub count: Option<u32>,
    /// The row is coming soon: a quiet "Soon" marker instead of a count.
    pub soon: bool,
}

/// One meter line inside a provider's usage card.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageLine {
    /// "Weekly", "Session · 5h".
    pub label: SharedString,
    /// Fraction used, `0.0..=1.0`; at or past 0.8 the line turns `warning`.
    pub fraction: f32,
    /// "resets in 2d 3h".
    pub resets: SharedString,
}

/// One provider's usage card in the account menu.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderUsage {
    /// "Claude Code".
    pub name: SharedString,
    /// "Pro", "High Usage", "prolite".
    pub plan: SharedString,
    /// The mark's initial.
    pub initial: SharedString,
    /// The mark's label colour index (see `Palette::label`).
    pub tint: u8,
    /// The meters, top to bottom.
    pub lines: Vec<UsageLine>,
}

/// The account footer and its menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    /// The person's name.
    pub name: SharedString,
    /// The email the session really reports.
    pub email: SharedString,
    /// The plan shown beside the name.
    pub plan: SharedString,
    /// The avatar's initial.
    pub initial: SharedString,
    /// The avatar's label colour index (see `Palette::label`).
    pub tint: u8,
    /// Whether the account menu is open above the footer.
    pub menu_open: bool,
    /// Every connected provider's usage, in the order shown.
    pub usage: Vec<ProviderUsage>,
}

/// A rename in flight: which session, and the draft the field shows when
/// the host gives the screen no live input.
#[derive(Debug, Clone, PartialEq)]
pub struct EditingState {
    /// The session being renamed.
    pub session: SharedString,
    /// The draft text.
    pub draft: SharedString,
}

/// A pinned site in Browse mode.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowsePin {
    /// Stable id, reported by [`BrowseIntent::OpenPin`].
    pub id: SharedString,
    /// The tile's initial (no favicon yet).
    pub initial: SharedString,
    /// The human label (the accessible name).
    pub label: SharedString,
}

/// One open tab in Browse mode.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseTab {
    /// Stable id.
    pub id: SharedString,
    /// The page title.
    pub title: SharedString,
    /// The favicon mark's initial.
    pub initial: SharedString,
    /// Whether this is the tab the page shows.
    pub active: bool,
}

/// A folder of tabs in Browse mode.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseFolder {
    /// The folder name (also its id).
    pub name: SharedString,
    /// Whether its tabs are visible.
    pub open: bool,
    /// The tabs inside.
    pub tabs: Vec<BrowseTab>,
}

/// Browse mode's rows.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseModel {
    /// The page's host, drawn in `fg`.
    pub host: SharedString,
    /// The path after it, drawn muted.
    pub path: SharedString,
    /// Whether Back is enabled.
    pub can_go_back: bool,
    /// Whether Forward is enabled.
    pub can_go_forward: bool,
    /// The pinned sites, as tiles.
    pub pins: Vec<BrowsePin>,
    /// Today's tabs.
    pub tabs: Vec<BrowseTab>,
    /// The folders after them.
    pub folders: Vec<BrowseFolder>,
    /// The spaces, as chips above the footer.
    pub spaces: Vec<SharedString>,
    /// Which space is current.
    pub space: usize,
    /// Whether the Today header shows its actions (hovered).
    pub hovered_today_header: bool,
}

/// The view-options menu's state: what the host groups and sorts by and
/// which toggles are on, shown in the menu rows.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewOptions {
    /// "Project", "Date", "Status".
    pub group_by: SharedString,
    /// "Last activity", "Name".
    pub sort_by: SharedString,
    /// Show projects with no sessions.
    pub show_empty: bool,
    /// Show PR status on rows.
    pub show_pr_status: bool,
    /// Show archived sessions.
    pub show_archived: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self { group_by: "Project".into(), sort_by: "Last activity".into(), show_empty: false, show_pr_status: true, show_archived: false }
    }
}

/// One row of the view-options menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewOption {
    /// Group by ›
    GroupBy,
    /// Sort by ›
    SortBy,
    /// Show empty projects.
    ToggleEmpty,
    /// Show PR status.
    TogglePrStatus,
    /// Show archived.
    ToggleArchived,
}

/// The whole sidebar as plain data. The host owns the matching
/// [`gpui::ListState`] (see [`super::screen_item_count`]) and the
/// [`FluidHandle`](crate::interact::FluidHandle), and hands all three to
/// [`super::sidebar_screen`] every frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarModel {
    /// Which list is showing.
    pub mode: Mode,
    /// The search field's text, when it holds any.
    pub search: Option<SharedString>,
    /// The primary nav rows.
    pub nav: Vec<NavItem>,
    /// The pinned sessions, lifted above the projects.
    pub pinned: Vec<SessionSummary>,
    /// Whether the Pinned section is open.
    pub pinned_open: bool,
    /// The projects with their sessions, folds and branches.
    pub grouping: Rc<Grouping>,
    /// The selected session id.
    pub selected: Option<SharedString>,
    /// The row the pointer is over, as reported by [`SidebarIntent::Hover`]:
    /// a session id (its trailing slot swaps to actions), a Browse tab id
    /// (its close appears), or one of the reserved chrome keys `"search"`
    /// and `"new-session"` (their shortcut chip appears). Session ids must
    /// not use the reserved keys.
    pub hovered: Option<SharedString>,
    /// The project head the pointer is over: it shows its actions.
    pub hovered_group: Option<SharedString>,
    /// Whether the Projects header shows its actions (hovered).
    pub hovered_projects_header: bool,
    /// The keyboard-highlighted session (no ring: the list highlight).
    pub highlighted: Option<SharedString>,
    /// The rename in flight, if any.
    pub editing: Option<EditingState>,
    /// Whether sessions are still loading: skeleton rows, no text.
    pub loading: bool,
    /// Whether the sidebar is collapsed: only the edge strip renders.
    pub collapsed: bool,
    /// Whether the view-options menu is open under the Projects header.
    pub view_menu_open: bool,
    /// The view-options menu's current values.
    pub view_options: ViewOptions,
    /// Second lines on every row, or only where a status demands one.
    pub bylines: Bylines,
    /// The account footer and menu.
    pub account: Account,
    /// Browse mode's rows.
    pub browse: Option<BrowseModel>,
}

/// What the person did in the account menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountAction {
    /// Settings…
    Settings,
    /// Providers…
    Providers,
    /// Sign out.
    SignOut,
}

/// What the person did in Browse mode.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowseIntent {
    /// The address field was clicked: focus it.
    Address,
    /// Back.
    Back,
    /// Forward.
    Forward,
    /// Reload.
    Reload,
    /// New tab (the + beside reload, or the New tab row).
    NewTab,
    /// Ask Claude about this page.
    Ask,
    /// The page menu.
    PageMenu,
    /// A pinned site was clicked.
    OpenPin(SharedString),
    /// A tab was clicked.
    SelectTab(SharedString),
    /// A tab's close was clicked.
    CloseTab(SharedString),
    /// A folder head was clicked.
    ToggleFolder(SharedString),
    /// A space chip was clicked; the argument is its index.
    SwitchSpace(usize),
    /// The + after the spaces.
    NewSpace,
    /// The Today section's "new folder".
    NewFolder,
    /// The Today section's "close all".
    CloseAll,
    /// The pointer entered or left the Today header.
    HoverTodayHeader(bool),
}

/// What the person asked the app to do. The screen never searches, opens
/// menus or renames anything itself: it reports, the host acts.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarIntent {
    /// The search row was clicked: focus the search field.
    Search,
    /// The new-session row was clicked.
    NewSession,
    /// A nav row was clicked; the argument is the nav id.
    Nav(SharedString),
    /// The title-bar switch changed lists.
    SwitchMode(Mode),
    /// The title-bar toggle: collapse the sidebar (or, from the collapsed
    /// strip, expand it).
    Collapse,
    /// A session row was activated; the argument is the session id.
    Select(SharedString),
    /// A project head was clicked: open or close it.
    Toggle(SharedString),
    /// The Pinned header was clicked: open or close it.
    TogglePinned,
    /// The Projects header's view-options button: open or close the menu.
    ViewOptions,
    /// A row of the view-options menu.
    ViewOption(ViewOption),
    /// A click landed on a menu's own surface but no row (the host keeps
    /// the menu open); outside clicks and Escape are the host's to detect
    /// and answer with its own close.
    MenuBackdrop,
    /// The Projects header's add button.
    AddProject,
    /// A project head's action (new session in it, or its menu).
    GroupAction(SharedString, crate::nav::GroupAction),
    /// A project's fold row ("Show N more" / "Show less").
    ToggleMore(SharedString),
    /// A session row's hover action.
    RowAction(SharedString, crate::nav::RowAction),
    /// A rename was committed; the arguments are the session and the text.
    Rename(SharedString, SharedString),
    /// A rename was abandoned.
    RenameCancel,
    /// The footer was clicked: open or close the account menu.
    Account,
    /// A row of the account menu.
    AccountAction(AccountAction),
    /// The pointer-driven highlight moved to a session (or off every row).
    Hover(Option<SharedString>),
    /// The pointer entered or left a project head (its actions appear).
    HoverGroup(Option<SharedString>),
    /// The pointer entered or left the Projects header.
    HoverProjectsHeader(bool),
    /// Something in Browse mode.
    Browse(BrowseIntent),
}

/// The mock's data, exactly: the "Baaz sidebar v2" canvas populated state.
pub fn sample_model() -> SidebarModel {
    let pinned = vec![
        SessionSummary::new("design-v2", "Design v2 tokens", AgentState::Idle, "12m")
            .pinned()
            .preview("Apply D1 review · four fixes landed"),
        SessionSummary::new("browser-audit", "Browser integration audit", AgentState::Running, "2m")
            .pinned()
            .pulse()
            .status(RowStatusKind::Working, "Running the visual probe"),
    ];
    let mut harness = ProjectGroup::new("harness", "harness", "4")
        .open(vec![
            SessionSummary::new("checklist", "Checklist run", AgentState::Waiting, "")
                .pulse()
                .status(RowStatusKind::NeedsApproval, "approve cargo build"),
            SessionSummary::new("token-ladder", "Token ladder audit", AgentState::Idle, "2h")
                .preview("Audit the ladder · 11 metrics → 7"),
            SessionSummary::new("relay-capture", "Relay capture pass", AgentState::Idle, "3h")
                .preview("Capture the right pane · 5 entries"),
        ])
        .trailing("feedback-mode");
    harness.fold = Some((1, false));
    let aui = ProjectGroup::new("agentic-ui", "agentic-ui", "3")
        .open(vec![
            SessionSummary::new("fluid-glide", "Fluid list glide", AgentState::Done, "1d")
                .preview("Glide between rows · merged"),
            SessionSummary::new("rename-fit", "Rename field fit", AgentState::Failed, "1d")
                .status(RowStatusKind::Failed, "Typecheck failed in sidebar view"),
            SessionSummary::new("skeleton-rows", "Skeleton rows", AgentState::Idle, "1d")
                .preview("Three skeleton rows · looked at"),
        ])
        .trailing("design-v2");
    let lakshya = ProjectGroup::new("lakshya", "lakshya", "2");
    SidebarModel {
        mode: Mode::Sessions,
        search: None,
        nav: vec![
            NavItem { id: "skills".into(), label: "Skills".into(), icon: IconName::Book, count: Some(23), soon: false },
            NavItem { id: "automations".into(), label: "Automations".into(), icon: IconName::Zap, count: None, soon: true },
        ],
        pinned,
        pinned_open: true,
        grouping: Rc::new(Grouping::Project(vec![harness, aui, lakshya])),
        selected: Some("design-v2".into()),
        hovered: None,
        hovered_group: None,
        hovered_projects_header: false,
        highlighted: None,
        editing: None,
        loading: false,
        collapsed: false,
        view_menu_open: false,
        view_options: ViewOptions::default(),
        bylines: Bylines::All,
        account: Account {
            name: "Bharani".into(),
            email: "bharani91@gmail.com".into(),
            plan: "Max".into(),
            initial: "B".into(),
            tint: 5,
            menu_open: false,
            usage: vec![
                ProviderUsage {
                    name: "Muse".into(),
                    plan: "High Usage".into(),
                    initial: "M".into(),
                    tint: 6,
                    lines: vec![UsageLine { label: "Weekly".into(), fraction: 0.01, resets: "resets in 47h 59m".into() }],
                },
                ProviderUsage {
                    name: "Claude Code".into(),
                    plan: "Pro".into(),
                    initial: "C".into(),
                    tint: 2,
                    lines: vec![
                        UsageLine { label: "Session · 5h".into(), fraction: 0.42, resets: "resets in 40m".into() },
                        UsageLine { label: "Weekly".into(), fraction: 0.83, resets: "resets in 2d 3h".into() },
                    ],
                },
                ProviderUsage {
                    name: "Codex".into(),
                    plan: "prolite".into(),
                    initial: "O".into(),
                    tint: 4,
                    lines: vec![UsageLine { label: "Weekly".into(), fraction: 0.19, resets: "resets in 6d 23h".into() }],
                },
            ],
        },
        browse: Some(BrowseModel {
            host: "pgi.udiseplus.gov.in".into(),
            path: "/#/home".into(),
            can_go_back: true,
            can_go_forward: false,
            pins: vec![
                BrowsePin { id: "u".into(), initial: "U".into(), label: "Unified dashboard".into() },
                BrowsePin { id: "p".into(), initial: "P".into(), label: "Performance grades".into() },
                BrowsePin { id: "d".into(), initial: "D".into(), label: "Open data portal".into() },
                BrowsePin { id: "n".into(), initial: "N".into(), label: "Survey reports".into() },
            ],
            tabs: vec![
                BrowseTab { id: "scores".into(), title: "State scores 2023-24".into(), initial: "S".into(), active: true },
                BrowseTab { id: "dashboard".into(), title: "Dashboard 2024-25".into(), initial: "D".into(), active: false },
                BrowseTab { id: "cards".into(), title: "State report cards".into(), initial: "S".into(), active: false },
            ],
            folders: vec![BrowseFolder {
                name: "Karnataka review".into(),
                open: true,
                tabs: vec![
                    BrowseTab { id: "board".into(), title: "Board results 2025".into(), initial: "B".into(), active: false },
                    BrowseTab { id: "scheme".into(), title: "Scheme review · Karnataka".into(), initial: "S".into(), active: false },
                ],
            }],
            spaces: vec!["Personal".into(), "Work".into(), "Lakshya".into()],
            space: 0,
            hovered_today_header: false,
        }),
    }
}
