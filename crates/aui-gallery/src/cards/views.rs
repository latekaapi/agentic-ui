//! Card 23 · Sidebar views. The same sessions grouped three ways — by status,
//! by project with nested children, by date — beside the view-options menu.
//! Reproduces `design/src/cards/sidebar/23-sidebar-views.html` at 1180×680.

use aui::keys::{Cancel, Confirm, SelectNext, SelectPrev, MENU_CONTEXT};
use aui::nav::{
    nav_item, sidebar_search, sidebar_view, view_menu, view_submenu, view_submenu_rows, DateGroup, Grouping, MenuRow, ProjectGroup, RowAction,
    SessionSummary, StatusGroup,
};
use aui::nav::{ActivityKind, MetaItem};
use aui::overlay::popover_layer;
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.wrap{grid-template-columns:262px 262px 262px 1fr;gap:18px;align-items:start}`.
const COLUMN: f32 = 262.0;
const COLUMN_GAP: f32 = 18.0;
/// `.side{height:600px;border-radius:var(--r-lg)}`.
const PANEL_H: f32 = 600.0;
/// The two width states under the main row: the sidebar's narrow end and a
/// width past the shell's own maximum. Short panels — the rows' right edge is
/// what they are for, not the whole list.
const WIDTH_NARROW: f32 = 240.0;
const WIDTH_WIDE: f32 = 520.0;
const WIDTH_STATE_H: f32 = 300.0;
/// The gap between the card's main row and the width states.
const ROW_GAP: f32 = 26.0;
/// `.nav{padding:10px 8px 6px}`.
const NAV_PAD_TOP: f32 = 10.0;
const NAV_PAD_X: f32 = 8.0;
const NAV_PAD_BOTTOM: f32 = 6.0;
/// `.ttl{font-size:12px;color:var(--ink-3);margin:0 0 8px}`.
const TITLE_GAP: f32 = 8.0;
/// `.sub{margin-top:-4px;margin-left:40px}` — the card positions the submenu.
const SUB_TOP: f32 = -4.0;
const SUB_LEFT: f32 = 40.0;
/// `.ds-note{margin-top:14px}`; its 80ch measure is wider than the column, so
/// the note wraps at the column's own 300 px.
const NOTE_TOP: f32 = 14.0;
/// `.grp{height:28px}` — the caps group row that carries the sliders icon; the
/// live menu hangs under it.
const GROUP_ROW_H: f32 = 28.0;
/// The open menu is inset from the panel's right edge, so its 250 px sit
/// inside the column's 260 px of inner width under the sliders button.
const MENU_INSET: f32 = scale::SP_2;
/// The gap between the group row and the menu it opens.
const MENU_GAP: f32 = scale::SP_2;

/// The row shared by every grouping: `checkout-flow-v2`, selected.
fn checkout(activity: bool) -> SessionSummary {
    let row = SessionSummary::new("checkout", "checkout-flow-v2", AgentState::Running, "49m").pulse().repo("acme-web");
    if activity {
        row.activity(ActivityKind::Working, "running regression tests")
    } else {
        row
    }
}

fn notifier() -> SessionSummary {
    SessionSummary::new("notifier", "infra/notifier", AgentState::Waiting, "3h")
        .pulse()
        .meta(MetaItem::Warning("awaiting permission".into()))
}

fn auth_refresh() -> SessionSummary {
    SessionSummary::new("auth", "auth-session-refresh", AgentState::Done, "4h").meta(MetaItem::Text("PR #2491 open".into()))
}

/// The status grouping of column one.
fn status_groups() -> Vec<StatusGroup> {
    vec![
        StatusGroup::new("needs-you", "Needs you", "1", vec![notifier()]),
        StatusGroup::new(
            "running",
            "Running",
            "3",
            vec![
                checkout(false).meta(MetaItem::Text("running regression tests".into())),
                SessionSummary::new("auth-flow", "redesign auth flow", AgentState::Running, "8m")
                    .repo("acme-web")
                    .meta(MetaItem::Text("2 children".into())),
                SessionSummary::new("cart", "cart-recovery-email", AgentState::Running, "12m").repo("acme-web"),
            ],
        ),
        StatusGroup::new(
            "done",
            "Done",
            "2",
            vec![
                auth_refresh(),
                SessionSummary::new("obs", "Observability tiles", AgentState::Failed, "1d").meta(MetaItem::Danger("2 tests failed".into())),
            ],
        ),
    ]
}

/// The project grouping of column two: marks, a trailing branch, one running
/// group with its dot, and the closed muted "Other workspaces" group.
fn project_groups(p: &Palette) -> Vec<ProjectGroup> {
    vec![
        ProjectGroup::new("acme-web", "acme-web", "5")
            .mark("A", p.label(5))
            .trailing("main")
            .state(AgentState::Running)
            // The project the open session belongs to: the accent bar at the
            // row's left edge.
            .current(true)
            // Twelve rows held back: the column shows the visible three and
            // the "Show 12 more" row after them.
            .folded(12, false)
            .open(vec![
                SessionSummary::new("checkout", "checkout-flow-v2", AgentState::Running, "49m")
                    .pulse()
                    .activity(ActivityKind::Working, "running regression tests"),
                SessionSummary::new("auth-flow", "redesign auth flow", AgentState::Running, "8m")
                    .meta(MetaItem::Text("2 children · PR 1/2 ready".into()))
                    .child(SessionSummary::new("pr1", "PR 1/2 · migrate users.sql", AgentState::Done, "6m"))
                    .child(SessionSummary::new("pr2", "PR 2/2 · withSession", AgentState::Running, "now")),
                auth_refresh(),
            ]),
        // A branch far longer than the column fits: it must give way (truncate)
        // while the project name keeps its readable minimum.
        ProjectGroup::new("orca", "orca", "2")
            .mark("O", p.label(3))
            .trailing("feature/projects-2026-09-13-long")
            .open(vec![notifier()]),
        ProjectGroup::new("acme-internal", "acme-internal", "4").mark("I", p.label(1)),
        ProjectGroup::new("other", "Other workspaces", "3").muted(),
    ]
}

/// The date grouping of column three: rows carry only a repo tag. The
/// first Today row is pinned, so the column opens with the leading `Pinned`
/// group (the row leaves its date bucket).
fn date_groups() -> Vec<DateGroup> {
    vec![
        DateGroup::new(
            "Today",
            vec![
                checkout(false).pinned(),
                SessionSummary::new("auth-flow", "redesign auth flow", AgentState::Running, "8m").repo("acme-web"),
                SessionSummary::new("notifier", "infra/notifier", AgentState::Waiting, "3h").pulse().repo("orca"),
                SessionSummary::new("auth", "auth-session-refresh", AgentState::Done, "4h").repo("acme-web"),
            ],
        ),
        DateGroup::new(
            "Yesterday",
            vec![SessionSummary::new("obs", "Observability tiles", AgentState::Failed, "1d").repo("acme-internal")],
        ),
        DateGroup::new(
            "This week",
            vec![SessionSummary::new("webhook", "Webhook retry backoff", AgentState::Idle, "2d").repo("acme-internal")],
        ),
    ]
}

/// The rows of the view-options menu.
fn menu_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::Submenu { label: "Status".into(), value: "Active".into(), highlighted: false },
        MenuRow::Submenu { label: "Environment".into(), value: "All".into(), highlighted: false },
        MenuRow::Separator,
        MenuRow::Submenu { label: "Group by".into(), value: "Project".into(), highlighted: true },
        MenuRow::Submenu { label: "Sort by".into(), value: "Last activity".into(), highlighted: false },
        MenuRow::Separator,
        MenuRow::Toggle { label: "Show empty groups".into(), checked: false },
        MenuRow::Toggle { label: "Show PR status".into(), checked: true },
        MenuRow::Toggle { label: "Show archived".into(), checked: false },
    ]
}

/// `.nav` — the two primary rows every column repeats.
fn nav(id: &'static str) -> impl IntoElement {
    let id = ElementId::from(id);
    v_flex()
        .w_full()
        .flex_none()
        .pt(px(NAV_PAD_TOP))
        .px(px(NAV_PAD_X))
        .pb(px(NAV_PAD_BOTTOM))
        .child(nav_item((id.clone(), "tasks"), IconName::List, "Tasks").count("7"))
        .child(nav_item((id, "inbox"), IconName::Inbox, "Inbox").count("2").count_warning())
}

/// `.ttl` above a `.side` panel. `overlay` is the column's open view menu,
/// already lifted onto [`popover_layer`]; it is absolutely positioned inside
/// the panel and so leaves the panel's own layout untouched.
fn column(p: &Palette, title: &'static str, body: AnyElement, overlay: Option<AnyElement>) -> Div {
    sized_column(p, title, body, overlay, COLUMN, PANEL_H)
}

/// [`column()`] at a chosen panel width and height — what the width states
/// below the card's main row are built from.
fn sized_column(p: &Palette, title: &'static str, body: AnyElement, overlay: Option<AnyElement>, width: f32, height: f32) -> Div {
    let mut panel = v_flex()
        .relative()
        .w_full()
        .h(px(height))
        .flex_none()
        .overflow_hidden()
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .child(body);
    if let Some(overlay) = overlay {
        panel = panel.child(overlay);
    }
    v_flex()
        .flex_none()
        .w(px(width))
        .child(div().mb(px(TITLE_GAP)).ui(scale::FS_12).text_color(p.ink_3).child(title))
        .child(panel)
}

/// The project grouping at the sidebar's narrow and wide ends. Every row —
/// the project row, its sessions, a nested child, the fold row — ends at the
/// same right edge at both widths, and so does the selected row's ground:
/// `.pj` and `.sr` both carry an 8 px gutter, so that edge is the panel's
/// inner edge less 8 whatever the width. The wide state is past the app's own
/// `SIDEBAR_MAX_WIDTH` on purpose: the component must not assume a bound the
/// shell happens to impose.
fn width_state(p: &Palette, title: &'static str, id: &'static str, width: f32) -> Div {
    let view = sidebar_view(id, Grouping::Project(project_groups(p))).caption("Projects").selected("checkout");
    let body = v_flex().w_full().child(view).into_any_element();
    sized_column(p, title, body, None, width, WIDTH_STATE_H)
}

/// How one column groups its sessions. The card starts with one of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupBy {
    Status,
    Project,
    Date,
}

impl GroupBy {
    /// The label the `Group by` row shows, and the submenu item that is checked.
    fn label(self) -> &'static str {
        match self {
            GroupBy::Status => "Status",
            GroupBy::Project => "Project",
            GroupBy::Date => "Date",
        }
    }

    /// The submenu index of this choice.
    fn index(self) -> usize {
        match self {
            GroupBy::Status => 0,
            GroupBy::Project => 1,
            GroupBy::Date => 2,
        }
    }

    /// The grouping (and its sessions) this choice renders.
    fn grouping(self, p: &Palette) -> Grouping {
        match self {
            GroupBy::Status => Grouping::Status(status_groups()),
            GroupBy::Project => Grouping::Project(project_groups(p)),
            GroupBy::Date => Grouping::Date(date_groups()),
        }
    }
}

/// The three live columns.
const COLUMNS: usize = 3;
/// The index of the `Group by` row in [`live_menu_rows`]; the rows at 2 and 5
/// are separators, which the keyboard skips.
const GROUP_BY_ROW: usize = 3;
/// The number of rows the live menu has, separators included.
const MENU_ROWS: usize = 9;
/// The first of the three toggle rows.
const FIRST_TOGGLE: usize = 6;
/// The number of toggle rows.
const TOGGLES: usize = 3;
/// The grouping choices of the `Group by` submenu; only the first three
/// switch the column.
const GROUP_CHOICES: usize = 3;

/// Is `row` one of the two separators?
fn is_separator(row: usize) -> bool {
    row == 2 || row == 5
}

/// What the card remembers: how each column groups, which column's menu is
/// open, and the keyboard's place in it.
struct ViewsState {
    /// The grouping each column renders.
    group_by: [GroupBy; COLUMNS],
    /// The three preference toggles of each column's menu.
    toggles: [[bool; TOGGLES]; COLUMNS],
    /// The column whose view menu is open.
    open: Option<usize>,
    /// The `Group by` submenu is open under the open menu.
    submenu: bool,
    /// The highlighted menu row, once the keyboard has moved.
    highlight: Option<usize>,
    /// The open menu's own focus.
    focus: FocusHandle,
    /// What had the keyboard when the menu opened.
    restore: Option<FocusHandle>,
}

/// The rows of an open column's menu: the `Group by` value follows the column,
/// and the highlight follows the keyboard (a row is also held while its
/// submenu is open).
fn live_menu_rows(group_by: GroupBy, toggles: [bool; TOGGLES], highlight: Option<usize>, submenu: bool) -> Vec<MenuRow> {
    let lit = |row: usize| highlight == Some(row) || (submenu && row == GROUP_BY_ROW);
    vec![
        MenuRow::Submenu { label: "Status".into(), value: "Active".into(), highlighted: lit(0) },
        MenuRow::Submenu { label: "Environment".into(), value: "All".into(), highlighted: lit(1) },
        MenuRow::Separator,
        MenuRow::Submenu { label: "Group by".into(), value: group_by.label().into(), highlighted: lit(GROUP_BY_ROW) },
        MenuRow::Submenu { label: "Sort by".into(), value: "Last activity".into(), highlighted: lit(4) },
        MenuRow::Separator,
        MenuRow::Toggle { label: "Show empty groups".into(), checked: toggles[0] },
        MenuRow::Toggle { label: "Show PR status".into(), checked: toggles[1] },
        MenuRow::Toggle { label: "Show archived".into(), checked: toggles[2] },
    ]
}

/// Opens `column`'s view menu, taking the keyboard and remembering what had it.
fn open_menu(state: &Entity<ViewsState>, column: usize, window: &mut Window, cx: &mut App) {
    let focus = state.read(cx).focus.clone();
    let restore = window.focused(cx);
    state.update(cx, |s, cx| {
        s.open = Some(column);
        s.submenu = false;
        s.highlight = None;
        s.restore = restore;
        cx.notify();
    });
    window.focus(&focus, cx);
}

/// Closes both menus and gives the keyboard back.
fn close_menu(state: &Entity<ViewsState>, window: &mut Window, cx: &mut App) {
    let restore = state.update(cx, |s, cx| {
        s.open = None;
        s.submenu = false;
        s.highlight = None;
        cx.notify();
        s.restore.take()
    });
    if let Some(handle) = restore {
        window.focus(&handle, cx);
    }
}

/// Runs menu row `row`: `Group by` opens its submenu, a toggle row flips, and
/// the other value rows only take the highlight.
fn activate_row(state: &Entity<ViewsState>, row: usize, _window: &mut Window, cx: &mut App) {
    if is_separator(row) {
        return;
    }
    state.update(cx, |s, cx| {
        let Some(column) = s.open else { return };
        s.highlight = Some(row);
        if row == GROUP_BY_ROW {
            s.submenu = !s.submenu;
        } else if row >= FIRST_TOGGLE {
            let toggle = row - FIRST_TOGGLE;
            s.toggles[column][toggle] = !s.toggles[column][toggle];
            s.submenu = false;
        } else {
            s.submenu = false;
        }
        cx.notify();
    });
}

/// Runs submenu item `item`: the first three regroup the open column and close
/// the menus; `Custom groups` and `None` are not modelled by this card.
fn activate_group(state: &Entity<ViewsState>, item: usize, window: &mut Window, cx: &mut App) {
    if item >= GROUP_CHOICES {
        return;
    }
    let choice = [GroupBy::Status, GroupBy::Project, GroupBy::Date][item];
    state.update(cx, |s, cx| {
        if let Some(column) = s.open {
            s.group_by[column] = choice;
        }
        cx.notify();
    });
    close_menu(state, window, cx);
}

/// Moves the highlight by `delta` rows, skipping separators and wrapping.
fn move_highlight(state: &Entity<ViewsState>, delta: isize, cx: &mut App) {
    state.update(cx, |s, cx| {
        let mut row = match s.highlight {
            Some(row) => row as isize,
            None if delta > 0 => -1,
            None => MENU_ROWS as isize,
        };
        for _ in 0..MENU_ROWS {
            row = (row + delta).rem_euclid(MENU_ROWS as isize);
            if !is_separator(row as usize) {
                break;
            }
        }
        s.highlight = Some(row as usize);
        cx.notify();
    });
}

/// The environment variable the screenshot hook reads. It is a comma-separated
/// list of steps applied once, after the first frame, through the same
/// handlers the mouse uses: `menu` opens column one's view menu, `groupby`
/// runs its `Group by` row, and `status` / `project` / `date` run that item of
/// the submenu — so `AUI_GALLERY_VIEWS_STEPS=menu,groupby,project` captures
/// the card mid-regroup. The gallery cannot be typed at from the command line;
/// nothing else in the card reads it.
const STEPS_ENV: &str = "AUI_GALLERY_VIEWS_STEPS";

/// Applies one step of [`STEPS_ENV`].
fn apply_step(state: &Entity<ViewsState>, step: &str, window: &mut Window, cx: &mut App) {
    match step {
        "menu" => open_menu(state, 0, window, cx),
        "groupby" => activate_row(state, GROUP_BY_ROW, window, cx),
        "status" => activate_group(state, 0, window, cx),
        "project" => activate_group(state, 1, window, cx),
        "date" => activate_group(state, 2, window, cx),
        _ => {}
    }
}

/// The five choices of the `Group by` submenu.
fn group_items() -> Vec<SharedString> {
    vec!["Status".into(), "Project".into(), "Date".into(), "Custom groups".into(), "None".into()]
}

/// The eight rows of the `Colour` submenu: the label ramp, with the current
/// project's colour checked.
fn colour_rows(p: &Palette) -> Vec<MenuRow> {
    ["Red", "Orange", "Yellow", "Green", "Teal", "Blue", "Violet", "Pink"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| MenuRow::Swatch { label: name.into(), colour: p.label(i as u8), checked: i == 5 })
        .collect()
}

/// The `.ttl` above each column.
const TITLES: [&str; COLUMNS] = ["Group by status", "Group by project · sessions and children", "By date · flat"];
/// The caps group row of each column — the column's own name, not its grouping.
const CAPTIONS: [&str; COLUMNS] = ["Workspaces", "Projects", "Recent"];
/// A stand-in for the field a caller would put in the search slot: the row
/// owns the frame, the caller owns the text and the caret.
fn search_field(p: &Palette) -> impl IntoElement {
    div().ui(scale::FS_12).text_color(p.ink_2).child("checkout")
}

/// The same, for a row being renamed in place: the dense 22 px wrapper at
/// the row-title size, so the editing row keeps the 30 px row height (the
/// static mock of `aui::nav::dense_field` plus its focus border).
fn rename_field(p: &Palette) -> impl IntoElement {
    h_flex()
        .w_full()
        .h(px(22.0))
        .px(px(6.0))
        .items_center()
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(p.accent)
        .bg(p.surface_1)
        .ui(12.5)
        .text_color(p.ink)
        .child(div().flex_1().min_w(px(0.0)).truncate().child("Checkout flow"))
}

/// The [`sidebar_view`] id of each column.
const VIEW_IDS: [&str; COLUMNS] = ["view-status", "view-project", "view-date"];
/// The `.nav` id of each column.
const NAV_IDS: [&str; COLUMNS] = ["status", "project", "date"];

/// `column`'s open view menu, hung under the sliders button and lifted onto
/// [`popover_layer`] so it paints over the panel's border and its neighbours.
fn menu_overlay(state: &Entity<ViewsState>, column: usize, cx: &mut App) -> AnyElement {
    let (group_by, toggles, submenu, highlight, focus) = {
        let s = state.read(cx);
        (s.group_by[column], s.toggles[column], s.submenu, s.highlight, s.focus.clone())
    };
    // The caption row sits under `.nav` (its padding and two rows) and carries
    // the 8 px top margin every group row has.
    let top = px(NAV_PAD_TOP + NAV_PAD_BOTTOM + scale::SP_3 + GROUP_ROW_H + MENU_GAP) + cx.aui().metrics.row * 2.0;

    let mut wrapper = v_flex()
        .absolute()
        .top(top)
        .right(px(MENU_INSET))
        .key_context(MENU_CONTEXT)
        .track_focus(&focus)
        .on_action::<SelectNext>({
            let state = state.clone();
            move |_, _, cx| move_highlight(&state, 1, cx)
        })
        .on_action::<SelectPrev>({
            let state = state.clone();
            move |_, _, cx| move_highlight(&state, -1, cx)
        })
        .on_action::<Confirm>({
            let state = state.clone();
            move |_, window, cx| {
                if let Some(row) = state.read(cx).highlight {
                    activate_row(&state, row, window, cx);
                }
            }
        })
        .on_action::<Cancel>({
            let state = state.clone();
            move |_, window, cx| {
                if state.read(cx).submenu {
                    state.update(cx, |s, cx| {
                        s.submenu = false;
                        cx.notify();
                    });
                } else {
                    close_menu(&state, window, cx);
                }
            }
        })
        .child(
            view_menu(("view-menu-live", column), live_menu_rows(group_by, toggles, highlight, submenu))
                .present(true)
                .on_activate({
                    let state = state.clone();
                    move |row, window, cx| activate_row(&state, row, window, cx)
                }),
        );

    if submenu {
        wrapper = wrapper.child(
            div().ml(px(SUB_LEFT)).mt(px(SUB_TOP)).child(
                view_submenu(("view-submenu-live", column), group_items(), Some(group_by.index()))
                    .separator_before(4)
                    .present(true)
                    .on_activate({
                        let state = state.clone();
                        move |item, window, cx| activate_group(&state, item, window, cx)
                    }),
            ),
        );
    }
    popover_layer(wrapper).into_any_element()
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let state = window.use_keyed_state("card23-views", cx, |_, cx| ViewsState {
        group_by: [GroupBy::Status, GroupBy::Project, GroupBy::Date],
        toggles: [[false, true, false]; COLUMNS],
        open: None,
        submenu: false,
        highlight: None,
        focus: cx.focus_handle(),
        restore: None,
    });
    let (group_by, open) = {
        let s = state.read(cx);
        (s.group_by, s.open)
    };

    // Screenshot hook: the gallery cannot be typed at from the command line,
    // so `AUI_GALLERY_VIEWS_STEPS` replays a few steps through the very same
    // handlers the mouse calls, once, after the first frame.
    if let Ok(steps) = std::env::var(STEPS_ENV) {
        let applied = window.use_keyed_state("card23-steps", cx, |_, _| false);
        if !*applied.read(cx) {
            applied.update(cx, |done, _| *done = true);
            let state = state.clone();
            window.on_next_frame(move |window, cx| {
                for step in steps.split(',') {
                    apply_step(&state, step.trim(), window, cx);
                }
            });
        }
    }

    let mut columns = Vec::with_capacity(COLUMNS);
    for i in 0..COLUMNS {
        let on_view_options = {
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                if state.read(cx).open == Some(i) {
                    close_menu(&state, window, cx);
                } else {
                    open_menu(&state, i, window, cx);
                }
            }
        };
        let mut view = sidebar_view(VIEW_IDS[i], group_by[i].grouping(&p))
            .caption(CAPTIONS[i])
            .selected("checkout")
            .on_view_options(on_view_options);
        // The third column carries the two affordances the harness's sessions
        // list needs: a search row above the groups, and a row that is being
        // renamed in place through the editor slot.
        if i == COLUMNS - 1 {
            view = view
                .row_actions(vec![RowAction::Rename, RowAction::Hide])
                .editing("checkout", rename_field(&p));
        }
        let mut body = v_flex().w_full().child(nav(NAV_IDS[i]));
        if i == COLUMNS - 1 {
            body = body.child(sidebar_search("views-search", search_field(&p)).clearable(true).on_clear(|_, _, _| {}));
        }
        let body = body.child(view).into_any_element();
        let overlay = if open == Some(i) { Some(menu_overlay(&state, i, cx)) } else { None };
        columns.push(column(&p, TITLES[i], body, overlay));
    }

    let menus = v_flex()
        .flex_1()
        .min_w(px(0.0))
        .child(div().mb(px(TITLE_GAP)).ui(scale::FS_12).text_color(p.ink_3).child("View options · from the sliders icon"))
        .child(view_menu("view-menu", menu_rows()).at_rest())
        .child(
            div().ml(px(SUB_LEFT)).mt(px(SUB_TOP)).child(
                view_submenu(
                    "view-submenu",
                    vec!["Status".into(), "Project".into(), "Date".into(), "Custom groups".into(), "None".into()],
                    Some(1),
                )
                .separator_before(4)
                .at_rest(),
            ),
        )
        .child(div().mt(px(NOTE_TOP)).ui(scale::FS_12).text_color(p.ink_3).child("Colour · the project submenu"))
        .child(view_submenu_rows("view-colour", colour_rows(&p)).at_rest())
        .child(
            div().mt(px(NOTE_TOP)).ui(scale::FS_12).text_color(p.ink_3).child(
                "One row anatomy serves every view: status dot, name, elapsed time, and an optional \
                 meta line. Grouping changes the headers, not the rows. Children nest under a hairline \
                 rail. The menu is reached from the sliders icon on the group header and remembers its \
                 choice per workspace.",
            ),
        );

    let mut root = h_flex().relative().w_full().items_start().gap(px(COLUMN_GAP));
    // The click-catcher goes in first, so its deferred draw is painted under
    // the menu's (deferred draws of equal priority keep their order): a click
    // anywhere else in the card closes the menu.
    if open.is_some() {
        let state = state.clone();
        root = root.child(popover_layer(
            div()
                .id("card23-outside")
                .absolute()
                .inset_0()
                .occlude()
                .on_click(move |_, window, cx| close_menu(&state, window, cx)),
        ));
    }
    for c in columns {
        root = root.child(c);
    }
    let root = root.child(menus);

    // The same grouping at both ends of the sidebar's width range: the rows
    // must end where the project row ends in each.
    let widths = h_flex()
        .w_full()
        .items_start()
        .gap(px(COLUMN_GAP))
        .child(width_state(&p, "Narrow · 240 px", "view-narrow", WIDTH_NARROW))
        .child(width_state(&p, "Wide · 520 px", "view-wide", WIDTH_WIDE));

    v_flex().w_full().child(root).child(div().mt(px(ROW_GAP)).w_full().child(widths)).into_any_element()
}
