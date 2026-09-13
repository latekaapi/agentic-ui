//! `nav/gutter` — one gutter for every sidebar row kind.
//!
//! A nav item, the caption row, a plain group row, a group row with chevron
//! and current bar, a compact session row and the fold row on one card, at
//! the sidebar's narrow (240 px) and wide (420 px) ends. A 1 px ruler runs
//! through the leading centre ([`NAV_GUTTER`] + [`LEADING_BOX`] / 2 = 18 px)
//! and one through [`NAV_LABEL_X`] (32 px): the nav icon, the chevron and
//! the session dot share one x, and every label starts at one x.

use aui::nav::{nav_item, sidebar_view, ActivityKind, Grouping, ProjectGroup, SessionSummary, LEADING_BOX, NAV_GUTTER, NAV_LABEL_X};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// The two panel widths on the card.
const WIDTHS: [f32; 2] = [240.0, 420.0];
/// The gap between the panels.
const PANEL_GAP: f32 = 16.0;
/// `.ttl{font-size:12px;color:var(--ink-3);margin:0 0 8px}`.
const TITLE_GAP: f32 = 8.0;
/// The note under the panels.
const NOTE_TOP: f32 = 12.0;
/// A 1 px measurement ruler.
const RULER_W: f32 = 1.0;

/// The rows one panel shows: nav item, caption, plain group, chevron + bar
/// group with sessions and the fold row.
fn panel(id: &'static str, width: f32, cx: &mut App) -> impl IntoElement {
    let p = cx.aui().colors;
    let prefix = ElementId::from(id);
    let first = SessionSummary::new("s1", "checkout-flow-v2", AgentState::Running, "49m")
        .pulse()
        .repo("acme-web")
        .activity(ActivityKind::Working, "running regression tests");
    let second = SessionSummary::new("s2", "redesign auth flow", AgentState::Running, "8m").repo("acme-web");
    let grouping = Grouping::Project(vec![
        ProjectGroup::new("web", "acme-web", "3").open(vec![first]),
        ProjectGroup::new("orca", "orca", "2").chevron(true).current(true).current_bar(true).folded(4, false).open(vec![second]),
    ]);
    let view = sidebar_view((prefix.clone(), "view"), grouping).caption("Projects").selected("s1");
    v_flex()
        .flex_none()
        .w(px(width))
        .child(div().mb(px(TITLE_GAP)).ui(scale::FS_12).text_color(p.ink_3).child(format!("{width:.0} px")))
        .child(
            v_flex()
                .relative()
                .w_full()
                .rounded(px(scale::R_LG))
                .border_1()
                .border_color(p.line)
                .bg(p.surface_1)
                // The rows, edge to edge: the rulers measure from this edge.
                .child(nav_item((prefix.clone(), "new"), IconName::Plus, "New session"))
                .child(view)
                // The rulers paint last, over the rows but under nothing
                // interactive: the card is a static capture.
                .child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(NAV_GUTTER + LEADING_BOX / 2.0))
                        .w(px(RULER_W))
                        .bg(p.accent),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(NAV_LABEL_X))
                        .w(px(RULER_W))
                        .bg(p.accent),
                ),
        )
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                .gap(px(PANEL_GAP))
                .child(panel("gutter-narrow", WIDTHS[0], cx))
                .child(panel("gutter-wide", WIDTHS[1], cx)),
        )
        .child(
            div()
                .mt(px(NOTE_TOP))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Rulers at the leading centre (18 px) and NAV_LABEL_X (32 px): the nav icon, the chevron and the dot share one x; every label starts at one x."),
        )
        .into_any_element()
}
