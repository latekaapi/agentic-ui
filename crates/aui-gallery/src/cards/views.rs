//! Card 23 · Sidebar views. The same sessions grouped three ways — by status,
//! by project with nested children, by date — beside the view-options menu.
//! Reproduces `design/src/cards/sidebar/23-sidebar-views.html` at 1180×680.

use aui::nav::{
    nav_item, sidebar_view, view_menu, view_submenu, DateGroup, Grouping, MenuRow, ProjectGroup, SessionSummary, StatusGroup,
};
use aui::nav::{ActivityKind, MetaItem};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.wrap{grid-template-columns:262px 262px 262px 1fr;gap:18px;align-items:start}`.
const COLUMN: f32 = 262.0;
const COLUMN_GAP: f32 = 18.0;
/// `.side{height:600px;border-radius:var(--r-lg)}`.
const PANEL_H: f32 = 600.0;
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

/// The project grouping of column two.
fn project_groups() -> Vec<ProjectGroup> {
    vec![
        ProjectGroup::new("acme-web", "acme-web", "5").open(vec![
            SessionSummary::new("checkout", "checkout-flow-v2", AgentState::Running, "49m")
                .pulse()
                .activity(ActivityKind::Working, "running regression tests"),
            SessionSummary::new("auth-flow", "redesign auth flow", AgentState::Running, "8m")
                .meta(MetaItem::Text("2 children · PR 1/2 ready".into()))
                .child(SessionSummary::new("pr1", "PR 1/2 · migrate users.sql", AgentState::Done, "6m"))
                .child(SessionSummary::new("pr2", "PR 2/2 · withSession", AgentState::Running, "now")),
            auth_refresh(),
        ]),
        ProjectGroup::new("orca", "orca", "2").open(vec![notifier()]),
        ProjectGroup::new("acme-internal", "acme-internal", "4"),
        ProjectGroup::new("archived", "Archived", "37").muted(),
    ]
}

/// The date grouping of column three: rows carry only a repo tag.
fn date_groups() -> Vec<DateGroup> {
    vec![
        DateGroup::new(
            "Today",
            vec![
                checkout(false),
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

/// `.ttl` above a `.side` panel.
fn column(p: &Palette, title: &'static str, body: AnyElement) -> Div {
    v_flex()
        .flex_none()
        .w(px(COLUMN))
        .child(div().mb(px(TITLE_GAP)).ui(scale::FS_12).text_color(p.ink_3).child(title))
        .child(
            v_flex()
                .w_full()
                .h(px(PANEL_H))
                .flex_none()
                .overflow_hidden()
                .rounded(px(scale::R_LG))
                .border_1()
                .border_color(p.line)
                .bg(p.surface_1)
                .child(body),
        )
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;

    let status = column(
        &p,
        "Group by status",
        v_flex()
            .w_full()
            .child(nav("status"))
            .child(
                sidebar_view("view-status", Grouping::Status(status_groups()))
                    .caption("Workspaces")
                    .selected("checkout")
                    .on_view_options(|_, _| {}),
            )
            .into_any_element(),
    );

    let projects = column(
        &p,
        "Group by project · sessions and children",
        v_flex()
            .w_full()
            .child(nav("project"))
            .child(
                sidebar_view("view-project", Grouping::Project(project_groups()))
                    .caption("Projects")
                    .selected("checkout")
                    .on_view_options(|_, _| {}),
            )
            .into_any_element(),
    );

    let dates = column(
        &p,
        "By date · flat",
        v_flex()
            .w_full()
            .child(nav("date"))
            .child(
                sidebar_view("view-date", Grouping::Date(date_groups()))
                    .caption("Recent")
                    .selected("checkout")
                    .on_view_options(|_, _| {}),
            )
            .into_any_element(),
    );

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
        .child(
            div().mt(px(NOTE_TOP)).ui(scale::FS_12).text_color(p.ink_3).child(
                "One row anatomy serves every view: status dot, name, elapsed time, and an optional \
                 meta line. Grouping changes the headers, not the rows. Children nest under a hairline \
                 rail. The menu is reached from the sliders icon on the group header and remembers its \
                 choice per workspace.",
            ),
        );

    h_flex()
        .w_full()
        .items_start()
        .gap(px(COLUMN_GAP))
        .child(status)
        .child(projects)
        .child(dates)
        .child(menus)
        .into_any_element()
}
