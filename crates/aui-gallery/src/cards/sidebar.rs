//! Card 21 · Sidebar. The 256 px sidebar, the 48 px collapsed rail and the
//! legend. Reproduces `design/src/cards/sidebar/21-sidebar.html` at 760×640.

use aui::nav::{
    rail, sidebar, sidebar_footer, MetaItem, RailItem, SessionSummary, SidebarAccount, SidebarGroup, SidebarNav, SidebarNavItem,
};
use aui_icons::{IconName, Provider};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.wrap{gap:20px}`.
const WRAP_GAP: f32 = 20.0;
/// The legend: `max-width:30ch;gap:10px;padding-top:4px` — 30ch of Geist 12 ≈ 244 px.
const LEGEND_MEASURE: f32 = 244.0;
const LEGEND_GAP: f32 = 10.0;
const LEGEND_TOP: f32 = 4.0;
/// The second footer state renders at this width: the plan has no room, so
/// the name holds intact and the plan drops.
const NARROW_FOOTER_W: f32 = 200.0;
/// `.ft{padding:8px 12px}` — the sidebar cards use 8, the shell 10.
const FOOTER_PAD_Y: f32 = 8.0;

/// The card's sample sidebar.
fn sample_nav() -> SidebarNav {
    SidebarNav::new(
        "acme",
        SidebarAccount::new("A", "Alex Rivera · Max", Provider::Claude, 0.78).plan("Power Usage", false),
    )
    .item(SidebarNavItem::new("tasks", "Tasks", IconName::List).count("7"))
    .item(SidebarNavItem::new("automations", "Automations", IconName::Zap))
    .item(SidebarNavItem::new("inbox", "Inbox", IconName::Inbox).count("2").warning())
    .group(
        SidebarGroup::new("pinned", "Pinned")
            .count("3")
            .session(
                SessionSummary::new("checkout", "checkout-flow-v2", AgentState::Running, "49m")
                    .pulse()
                    .repo("acme-web")
                    .branch("feature/checkout-flow-v2"),
            )
            .session(
                SessionSummary::new("notifier", "infra/notifier", AgentState::Waiting, "3h")
                    .pulse()
                    .meta(MetaItem::Warning("awaiting permission".into())),
            )
            .session(
                SessionSummary::new("auth", "auth-session-refresh", AgentState::Done, "4h")
                    .meta(MetaItem::Text("PR #2491 open".into())),
            ),
    )
    .group(
        SidebarGroup::new("acme-web", "acme-web")
            .count("4")
            .trailing("main")
            .session(SessionSummary::new("cart", "cart-recovery-email", AgentState::Running, "12m"))
            .session(SessionSummary::new("baseline", "checkout-baseline", AgentState::Idle, "3d")),
    )
    .group(SidebarGroup::new("acme-internal", "acme-internal").count("4").closed())
    .group(SidebarGroup::new("done", "Done").count("37").closed())
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let items = vec![
        RailItem::nav("tasks", IconName::List),
        RailItem::nav("automations", IconName::Zap),
        RailItem::nav("inbox", IconName::Inbox).badge(),
        RailItem::separator(),
        RailItem::session("checkout", AgentState::Running).pulse().selected(true).label("checkout-flow-v2").tint(p.label(5)),
        RailItem::session("notifier", AgentState::Waiting).pulse().label("infra/notifier").tint(p.label(3)),
        RailItem::session("auth", AgentState::Done),
    ];
    h_flex()
        .size_full()
        .items_start()
        .gap(px(WRAP_GAP))
        .child(sidebar("card21-side", sample_nav()))
        .child(rail("card21-rail", items).avatar("B"))
        .child(legend(p))
        .into_any_element()
}

/// The legend: 12 px ink-2 paragraphs led by an ink 600 word.
fn legend(p: Palette) -> impl IntoElement {
    let items = [
        ("Header", " holds the workspace switcher and three quiet actions. The sliders icon on the Workspaces row opens the view options (see Sidebar views)."),
        ("Groups", " collapse with a chevron on the swap spring; counts stay visible when closed so the sidebar is scannable at a glance."),
        ("Rail", " is the collapsed form (\u{2318}B): nav icons, then one dot per active worktree in its state colour, tooltips on hover."),
        ("Footer", " keeps the name: row one is the name, the plan and the chevron; row two is the meter beside the identity, or alone. The plan truncates first and drops out entirely under 40 px of room, so at 200 px only the name and the meter are left."),
    ];
    let mut col = v_flex()
        .max_w(px(LEGEND_MEASURE))
        .pt(px(LEGEND_TOP))
        .gap(px(LEGEND_GAP))
        .ui(scale::FS_12)
        .text_color(p.ink_2);
    for (lead, rest) in items {
        col = col.child(crate::cards::rows::lead_paragraph(p, lead, rest));
    }
    col.child(narrow_footer(p))
}

/// A second footer state at [`NARROW_FOOTER_W`]: the plan has no room, so
/// the name holds intact, the plan drops, and the meter sits on its own
/// second row.
fn narrow_footer(p: Palette) -> impl IntoElement {
    div()
        .w(px(NARROW_FOOTER_W))
        .flex_none()
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .overflow_hidden()
        .child(
            sidebar_footer("card21-narrow-footer", "A", "Alex Rivera · Max")
                .plan("Power Usage", false)
                .meter(Provider::Claude, 0.78)
                .pad_y(FOOTER_PAD_Y),
        )
}
