//! Card 20 · Worktree rows. Every row state in a 300 px list beside the
//! legend. Reproduces `design/src/cards/sidebar/20-worktree-rows.html` at
//! 720×560.

use aui::nav::{session_row, ActivityKind, MetaItem, SessionSummary};
use aui_icons::Provider;
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.two{gap:24px}`.
const COLUMN_GAP: f32 = 24.0;
/// `.list{width:300px;padding:8px}`.
const LIST_WIDTH: f32 = 300.0;
const LIST_PAD: f32 = 8.0;
/// `.legend{gap:10px;padding-top:6px;max-width:34ch}` — 34ch of Geist 12 ≈ 250 px.
const LEGEND_GAP: f32 = 10.0;
const LEGEND_TOP: f32 = 6.0;
const LEGEND_MEASURE: f32 = 262.0;

/// The sample sessions of the card.
pub fn sample_sessions() -> Vec<(SessionSummary, bool)> {
    vec![
        (
            SessionSummary::new("checkout", "checkout-flow-v2", AgentState::Running, "49m")
                .pulse()
                .repo("acme-web")
                .branch("feature/checkout-flow-v2")
                .provider(Provider::Claude)
                .provider(Provider::Codex)
                .activity(ActivityKind::Working, "running checkout regression tests"),
            true,
        ),
        (
            SessionSummary::new("notifier", "infra/notifier", AgentState::Waiting, "3h")
                .pulse()
                .unread()
                .repo("orca")
                .branch("main")
                .provider(Provider::Codex)
                .activity(ActivityKind::Waiting, "awaiting permission · sudo apt install"),
            false,
        ),
        (
            SessionSummary::new("auth", "auth-session-refresh", AgentState::Done, "4h")
                .repo("acme-web")
                .meta(MetaItem::Text("PR #2491 open".into()))
                .meta(MetaItem::Text("3 ahead".into())),
            false,
        ),
        (
            SessionSummary::new("obs", "Observability dashboard tiles", AgentState::Failed, "1d")
                .repo("acme-internal")
                .meta(MetaItem::Danger("2 tests failed".into())),
            false,
        ),
        (
            SessionSummary::new("webhook", "Webhook retry backoff", AgentState::Idle, "2d").repo("acme-internal").branch("fix/webhook-retry-backoff"),
            false,
        ),
        (
            SessionSummary::new("auth-flow", "redesign auth flow", AgentState::Running, "8m")
                .repo("acme-web")
                .meta(MetaItem::Text("2 children · PR 1/2 ready".into()))
                .child(SessionSummary::new("pr1", "PR 1/2 · migrate users.sql", AgentState::Done, "6m"))
                .child(
                    SessionSummary::new("pr2", "PR 2/2 · withSession middleware", AgentState::Running, "now")
                        .activity(ActivityKind::Working, "wiring withSession middleware…"),
                ),
            false,
        ),
    ]
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut list = v_flex()
        .flex_none()
        .w(px(LIST_WIDTH))
        .p(px(LIST_PAD))
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1);
    for (i, (session, selected)) in sample_sessions().into_iter().enumerate() {
        list = list.child(session_row(("card20-row", i), session).selected(selected));
    }
    h_flex()
        .w_full()
        .items_start()
        .gap(px(COLUMN_GAP))
        .child(list)
        .child(legend(p))
        .into_any_element()
}

/// `.legend`: 12 px ink-2 paragraphs led by an ink 600 word.
fn legend(p: Palette) -> impl IntoElement {
    let items = [
        ("Anatomy.", " Status dot, name, elapsed time; second line repo tag, branch, provider marks; optional third line is the live activity sentence, truncated, updated as the agent streams."),
        ("States.", " Running pulses in accent. Needs-you pulses in warning and shows what it is waiting for. Done shows the PR pill. Failed shows the failing count. Idle is grey and silent."),
        ("Hover.", " The time slot yields to four quiet actions: terminal, browser, pin, more. Unread turns get a 3 px accent bar on the left edge."),
        ("Children.", " Fan-out tasks nest under the parent with a hairline rail and a smaller name."),
    ];
    let mut col = v_flex().max_w(px(LEGEND_MEASURE)).pt(px(LEGEND_TOP)).gap(px(LEGEND_GAP)).ui(scale::FS_12).text_color(p.ink_2);
    for (lead, rest) in items {
        col = col.child(lead_paragraph(p, lead, rest));
    }
    col
}

/// A paragraph whose first words are ink / 600.
pub fn lead_paragraph(p: Palette, lead: &'static str, rest: &'static str) -> impl IntoElement {
    let text = format!("{lead}{rest}");
    let highlight = HighlightStyle { color: Some(p.ink), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    div().child(StyledText::new(text).with_highlights([(0..lead.len(), highlight)]))
}
