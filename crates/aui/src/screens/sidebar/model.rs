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

/// A folder of tabs in Browse mode: one you made, or a session's task
/// folder (its tabs, named after it, below a hairline; docs/27 §4.3).
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseFolder {
    /// Stable id: the folder's own id, or the session id for a task folder.
    /// Reported by every folder intent; never the label (two folders may
    /// share a name).
    pub id: SharedString,
    /// The folder name, or the session's title.
    pub name: SharedString,
    /// Yours, or a session's.
    pub kind: FolderKind,
    /// Whether its tabs are visible (task folders are always open).
    pub open: bool,
    /// The tabs inside.
    pub tabs: Vec<BrowseTab>,
}

/// Whose folder a [`BrowseFolder`] is.
#[derive(Debug, Clone, PartialEq)]
pub enum FolderKind {
    /// A folder you made: chevron on the icon axis, folds on click.
    Yours,
    /// A session's tabs: its status dot on the icon axis and its status word
    /// at the row's end; always open; a click opens its chat.
    Task {
        /// The session's state (the same dot as its Sessions row).
        state: AgentState,
        /// "reading", "typing", "needs you", "done"; empty for none.
        status: SharedString,
        /// The session is waiting on the person: the word takes `warning`.
        needs_you: bool,
    },
}

/// One space in Browse mode: the browser's own set of tabs (docs/27 §4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseSpace {
    /// Stable id, reported by the space intents.
    pub id: SharedString,
    /// The chip's label.
    pub name: SharedString,
}

/// What a Browse name field is naming.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowseNaming {
    /// A folder (new or renamed), by id.
    Folder(SharedString),
    /// A new space (`None`) or an existing one, by id.
    Space(Option<SharedString>),
}

/// A name field in flight in Browse mode: the target and the draft shown
/// when the host gives the screen no live input.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseEditing {
    /// What is being named.
    pub target: BrowseNaming,
    /// The draft text.
    pub draft: SharedString,
}

/// What a Browse right-click menu belongs to.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowseMenuTarget {
    /// A tab: whether it is pinned and whether it sits in one of your folders.
    Tab {
        /// The tab id.
        id: SharedString,
        /// Pinned tabs read "Unpin tab".
        pinned: bool,
        /// A tab in a folder adds "Remove from folder".
        in_folder: bool,
    },
    /// One of your folders, by id.
    Folder(SharedString),
    /// A task folder, by session id; `running` shows Stop.
    TaskFolder {
        /// The session id.
        session: SharedString,
        /// Whether the session runs (Stop appears).
        running: bool,
    },
    /// A space chip, by id; `last` = the only space (cannot be deleted).
    Space {
        /// The space id.
        id: SharedString,
        /// Whether it is the only space.
        last: bool,
    },
}

/// An open right-click menu in Browse mode: what it belongs to and where
/// the pointer was, in window coordinates (the menu's top-left corner).
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseMenu {
    /// The menu's subject.
    pub target: BrowseMenuTarget,
    /// The pointer position at the right-click.
    pub at: gpui::Point<gpui::Pixels>,
}

/// A tab menu row.
#[derive(Debug, Clone, PartialEq)]
pub enum TabAction {
    /// Duplicate.
    Duplicate,
    /// Copy link.
    CopyLink,
    /// Pin tab / Unpin tab.
    TogglePin,
    /// Move to folder › <folder id>.
    MoveToFolder(SharedString),
    /// Move to folder › New folder.
    MoveToNewFolder,
    /// Remove from folder.
    RemoveFromFolder,
    /// Close tab.
    Close,
    /// Close other tabs.
    CloseOthers,
}

/// A folder menu row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderAction {
    /// Rename.
    Rename,
    /// Close its tabs.
    CloseTabs,
    /// Delete folder (its tabs move up into Today).
    Delete,
}

/// A task-folder menu row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFolderAction {
    /// Open chat.
    OpenChat,
    /// Stop (only while the session runs).
    Stop,
    /// Close its tabs.
    CloseTabs,
}

/// A space-chip menu row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceAction {
    /// Rename.
    Rename,
    /// Delete space.
    Delete,
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
    /// Your folders after them.
    pub folders: Vec<BrowseFolder>,
    /// The task folders, below a hairline after the New tab row.
    pub task_folders: Vec<BrowseFolder>,
    /// The spaces, as chips above the footer.
    pub spaces: Vec<BrowseSpace>,
    /// Which space is current (an index into `spaces`).
    pub space: usize,
    /// Whether the Today header shows its actions (hovered).
    pub hovered_today_header: bool,
    /// A folder or space name in flight.
    pub editing: Option<BrowseEditing>,
    /// The open right-click menu.
    pub menu: Option<BrowseMenu>,
}

/// How the session list is grouped (the Group by submenu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupKey {
    /// Projects with their sessions nested.
    #[default]
    Project,
    /// Date buckets over flat rows.
    Date,
}

impl GroupKey {
    /// Every grouping, in menu order.
    pub const ALL: [GroupKey; 2] = [GroupKey::Project, GroupKey::Date];

    /// The menu label.
    pub fn label(self) -> &'static str {
        match self {
            GroupKey::Project => "Project",
            GroupKey::Date => "Date",
        }
    }
}

/// How sessions are ordered inside their group (the Sort by submenu).
/// Pinned sessions stay first whatever the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    /// The latest activity first.
    #[default]
    Recent,
    /// The newest session first.
    Created,
    /// Alphabetical by title.
    Name,
}

impl SortKey {
    /// Every order, in menu order.
    pub const ALL: [SortKey; 3] = [SortKey::Recent, SortKey::Created, SortKey::Name];

    /// The menu label.
    pub fn label(self) -> &'static str {
        match self {
            SortKey::Recent => "Recent activity",
            SortKey::Created => "Created",
            SortKey::Name => "Name",
        }
    }
}

/// The view-options menu's state: what the host groups and sorts by and
/// which toggles are on, shown in the menu rows.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewOptions {
    /// The current grouping.
    pub group_by: GroupKey,
    /// The current order.
    pub sort_by: SortKey,
    /// Show projects with no sessions.
    pub show_empty: bool,
    /// Show hidden sessions.
    pub show_hidden: bool,
    /// How many sessions are hidden (the row's trailing count; 0 = none).
    pub hidden_count: usize,
    /// Show archived sessions.
    pub show_archived: bool,
    /// Show PR status on rows.
    pub show_pr_status: bool,
    /// Whether PR status exists at all: false draws the row disabled with
    /// a "Soon" marker (owner 2026-10-11).
    pub pr_status_available: bool,
    /// Whether search reaches every project, not just the current one.
    pub search_all_projects: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            group_by: GroupKey::Project,
            sort_by: SortKey::Recent,
            show_empty: false,
            show_hidden: false,
            hidden_count: 0,
            show_archived: false,
            show_pr_status: false,
            pr_status_available: false,
            search_all_projects: true,
        }
    }
}

/// One row of the view-options menu (or its two submenus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewOption {
    /// A Group by submenu row.
    GroupBy(GroupKey),
    /// A Sort by submenu row.
    SortBy(SortKey),
    /// Show empty projects.
    ToggleEmpty,
    /// Show hidden sessions.
    ToggleHidden,
    /// Show archived.
    ToggleArchived,
    /// Show PR status (only reported while PR status is available).
    TogglePrStatus,
    /// Search all projects.
    ToggleSearchAll,
    /// Clear empty sessions (hide every session with no turns).
    ClearEmpty,
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
    /// The address field was clicked: open the Field on it.
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
    /// One of your folder heads was clicked (by folder id).
    ToggleFolder(SharedString),
    /// A task folder head was clicked: open its session's chat.
    OpenTaskChat(SharedString),
    /// A space chip was clicked; the argument is its index.
    SwitchSpace(usize),
    /// The + after the spaces: name a new space.
    NewSpace,
    /// The Today section's "new folder".
    NewFolder,
    /// The Today section's "close all".
    CloseAll,
    /// The pointer entered or left the Today header.
    HoverTodayHeader(bool),
    /// A right-click: open the menu for `target` at the pointer.
    OpenMenu(BrowseMenu),
    /// Close the open right-click menu (a click on its backdrop or Escape is
    /// the host's to detect; a row click reports its action instead).
    CloseMenu,
    /// A tab menu row.
    Tab(SharedString, TabAction),
    /// A folder menu row.
    Folder(SharedString, FolderAction),
    /// A task-folder menu row.
    TaskFolder(SharedString, TaskFolderAction),
    /// A space-chip menu row.
    Space(SharedString, SpaceAction),
    /// A name field committed: the target and the text.
    Name(BrowseNaming, SharedString),
    /// A name field was abandoned.
    NameCancel,
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
    /// the menu open).
    MenuBackdrop,
    /// A press landed outside the open view-options or account menu: the
    /// host closes it (Escape stays the host's to detect).
    DismissMenus,
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
    /// The pointer-driven highlight moved to a row (or off every row): its
    /// id, and the row's bounds in window coordinates when it was measured —
    /// what a host seats a hover card beside.
    Hover(Option<SharedString>, Option<gpui::Bounds<gpui::Pixels>>),
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
                id: "karnataka".into(),
                name: "Karnataka review".into(),
                kind: FolderKind::Yours,
                open: true,
                tabs: vec![
                    BrowseTab { id: "board".into(), title: "Board results 2025".into(), initial: "B".into(), active: false },
                    BrowseTab { id: "scheme".into(), title: "Scheme review · Karnataka".into(), initial: "S".into(), active: false },
                ],
            }],
            task_folders: vec![
                BrowseFolder {
                    id: "checklist".into(),
                    name: "Checklist run".into(),
                    kind: FolderKind::Task { state: AgentState::Waiting, status: "needs you".into(), needs_you: true },
                    open: true,
                    tabs: vec![BrowseTab { id: "ledger".into(), title: "Ledger".into(), initial: "L".into(), active: false }],
                },
                BrowseFolder {
                    id: "browser-audit".into(),
                    name: "Browser integration audit".into(),
                    kind: FolderKind::Task { state: AgentState::Running, status: "reading".into(), needs_you: false },
                    open: true,
                    tabs: vec![BrowseTab { id: "pr".into(), title: "Pull request #412 · baaz".into(), initial: "G".into(), active: false }],
                },
            ],
            spaces: vec![
                BrowseSpace { id: "personal".into(), name: "Personal".into() },
                BrowseSpace { id: "work".into(), name: "Work".into() },
                BrowseSpace { id: "lakshya".into(), name: "Lakshya".into() },
            ],
            space: 0,
            editing: None,
            menu: None,
            hovered_today_header: false,
        }),
    }
}
