//! Card 22 · Assistant sidebar. Roles → projects → sessions with a knowledge
//! card per role. Reproduces `design/src/cards/sidebar/22-assistant-sidebar.html`
//! at 760×640, light theme.

use aui::data::{icon_button, pill};
use aui::nav::{role_section, sidebar_footer, Project, Role, RoleSession, SessionKind};
use aui_icons::{IconName, RoleIcon};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.wrap{gap:20px}`.
const COLUMN_GAP: f32 = 20.0;
/// `.side{width:268px;height:600px}`.
const SIDE_W: f32 = 268.0;
const SIDE_H: f32 = 600.0;
/// `.hd{height:44px;padding:0 10px 0 12px;gap:8px}`.
const HEAD_PAD_LEFT: f32 = 12.0;
const HEAD_PAD_RIGHT: f32 = 10.0;
const HEAD_GAP: f32 = 8.0;
/// The legend column: `max-width:32ch;font-size:12.5px;gap:10px;padding-top:4px`
/// — 32ch of Geist 12.5 measures 268 px once gpui's Geist metrics are applied.
const LEGEND_MEASURE: f32 = 268.0;
const LEGEND_GAP: f32 = 10.0;
const LEGEND_TOP: f32 = 4.0;
const LEGEND_TEXT: f32 = 12.5;
/// `.ft{padding:8px 12px}` and `.pill{height:16px}` in the footer.
const FOOTER_PAD_Y: f32 = 8.0;
const FOOTER_PILL_H: f32 = 16.0;

/// The three roles of the card: Education open, Law and Operations closed.
pub fn sample_roles() -> Vec<Role> {
    vec![
        Role::new("education", "Director, Education", RoleIcon::GradCap, 3)
            .open()
            .project(
                Project::new("rfp", "Teacher recruitment RFP", 6)
                    .session(RoleSession::new("rfp-draft", "RFP draft v3", SessionKind::Document, "now"))
                    .session(RoleSession::new("eligibility", "Eligibility criteria review", SessionKind::Chat, "2h"))
                    .session(RoleSession::new("scoring", "Vendor scoring sheet", SessionKind::Sheet, "1d")),
            )
            .project(Project::new("annual", "Annual report 2026", 3))
            .project(Project::new("letters", "Letters & notes", 12))
            .knowledge("Education Code")
            .knowledge("Procurement rules")
            .knowledge("GO 2024-18"),
        Role::new("law", "Director, Law", RoleIcon::Scale, 2),
        Role::new("ops", "Operations", RoleIcon::Gear, 4),
    ]
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut side = v_flex()
        .flex_none()
        .w(px(SIDE_W))
        .h(px(SIDE_H))
        .overflow_hidden()
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .child(header(cx));
    for (i, role) in sample_roles().into_iter().enumerate() {
        side = side.child(role_section(("card22-role", i), &role).active_session(Some("rfp-draft")));
    }
    side = side.child(div().flex_1()).child(
        sidebar_footer("card22-footer", "B", "Bharani").pad_y(FOOTER_PAD_Y).trailing(pill("local-first").height(FOOTER_PILL_H)),
    );

    h_flex().w_full().items_start().gap(px(COLUMN_GAP)).child(side).child(legend(p)).into_any_element()
}

/// `.hd`: the desk name and the search / new actions.
fn header(cx: &App) -> impl IntoElement {
    let p = cx.aui().colors;
    h_flex()
        .w_full()
        .h(cx.aui().metrics.header)
        .flex_none()
        .gap(px(HEAD_GAP))
        .pl(px(HEAD_PAD_LEFT))
        .pr(px(HEAD_PAD_RIGHT))
        .border_b_1()
        .border_color(p.line)
        .child(div().flex_1().min_w(px(0.0)).truncate().ui(scale::FS_13).semibold().text_color(p.ink).child("Desk"))
        .child(icon_button("card22-search", IconName::Search).ghost().sm())
        .child(icon_button("card22-new", IconName::Plus).ghost().sm())
}

/// The legend: 12.5 px ink-2 paragraphs led by an ink 600 word.
fn legend(p: Palette) -> impl IntoElement {
    let items = [
        ("Sections.", " Each role is a bordered section: a 44 px header with chevron, muted icon and name, no background. The open role shows Projects and Knowledge groups with a hairline and a count; closed roles are one row. Hierarchy by weight and ink: roles 600, projects 500 in ink-2, sessions 400 in ink-3. A role has its own knowledge sources; projects count their sessions; sessions show the kind of work by icon: chat, document, sheet."),
        ("Knowledge card.", " The orders, rules and regulations attached to the active role, so it is always visible what the assistant is grounded in. Projects and sessions can add more below it."),
        ("Light theme", " is the default here: the assistant is a writing tool and documents read best on paper-white."),
    ];
    let mut col = v_flex().max_w(px(LEGEND_MEASURE)).pt(px(LEGEND_TOP)).gap(px(LEGEND_GAP)).ui(LEGEND_TEXT).text_color(p.ink_2);
    for (lead, rest) in items {
        col = col.child(crate::cards::rows::lead_paragraph(p, lead, rest));
    }
    col
}
