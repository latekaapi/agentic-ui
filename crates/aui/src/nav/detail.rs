//! Option C's hover detail, with option B's question box: the dense card at
//! 320 px.
//!
//! The sidebar row truncates, so hovering a row shows the whole picture in
//! issue order, top to bottom: the full title wrapping with the relative
//! time (`12m`) right-aligned in the header's first line; the owner's last
//! message quoted and muted; the latest reply's first line in the state
//! colour (which carries the status, so there is no separate status row);
//! one inline meta row (branch, turn count, updated) that truncates
//! gracefully; and a mono muted footer merging project and workspace path.
//! Caller-supplied and optional per field: only the fields the app sets are
//! drawn, and a field the app does not have leaves no empty row behind.
//!
//! The one state that stands out is waiting on the owner: a pending question
//! or approval replaces the reply line with the highlighted attention box —
//! warning-tinted border and fill, a small icon, `Asked:` / `Needs approval:`
//! in semibold, then the question/command text wrapping up to three lines.
//!
//! gpui has no multi-line ellipsis, so the two excerpt budgets (ask: two
//! lines, attention: three) are line budgets, not visual clamps: the card
//! keeps the first N newline-separated lines and appends an ellipsis when
//! lines beyond the budget are dropped. Long lines wrap naturally. The reply
//! line is always the first line, truncating with an ellipsis at the card's
//! width.
//!
//! Caller-owned state: the app decides which row is hovered and for how
//! long. Open after [`SESSION_DETAIL_DELAY`] of hover, close on leave, on
//! scroll and on any click:
//!
//! ```ignore
//! // app state: Option<(SharedString, Bounds<Pixels>)> — hovered row + trigger
//! window.on_next_frame(...) // after SESSION_DETAIL_DELAY, if still hovered
//! // on_hover(false) | on_scroll | on_click → hovered = None
//! ```
//!
//! Seat it beside the sidebar with [`anchored_session_detail_at_sidebar`]:
//! the card's left edge at the sidebar pane's right edge plus
//! [`SESSION_DETAIL_GAP`], top-aligned with the hovered row, sliding up near
//! the window bottom — in [`crate::overlay::popover_layer`], so it escapes
//! the sidebar's clipping and never covers the rows. ([`anchored_session_detail`]
//! keeps the older below-start seat.) The card never takes focus (no focus
//! handle, no key context) and never covers the row itself, so the row's own
//! click still lands.

use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_tokens::{durations, scale, ActiveAui, AuiStyled};
use gpui::{
    div, point, prelude::*, px, Anchor, App, Bounds, ElementId, FontWeight, HighlightStyle, IntoElement,
    Pixels, Point, SharedString, Size, StyledText, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::nav::{RowStatus, RowStatusKind};
use crate::overlay::{anchored_menu, MenuAlign, MenuSide};

/// How long the app waits after hover before showing the detail: the motion
/// token `slow` (280 ms) — long enough that a pointer travelling past rows
/// never flashes the card, short enough that an intentional hover feels
/// immediate.
pub const SESSION_DETAIL_DELAY: Duration = durations::SLOW;

/// Width of the detail card: the mockup's 320 px — wide enough for a full
/// ask at the sidebar's narrow end, narrow enough to sit beside it.
pub const SESSION_DETAIL_WIDTH: f32 = 320.0;

/// How many newline-separated lines the quoted ask keeps; further lines are
/// dropped with an ellipsis (gpui has no multi-line ellipsis).
pub const SESSION_DETAIL_ASK_LINES: usize = 2;

/// How many newline-separated lines the attention box keeps; further lines
/// are dropped with an ellipsis.
pub const SESSION_DETAIL_ATTENTION_LINES: usize = 3;

/// The hover detail's caller-supplied content. Build with
/// [`session_detail`]; every field is optional, and only set fields draw.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionDetailData {
    /// Full title, wrapping, never truncated.
    pub title: Option<SharedString>,
    /// The owner's last message, quoted and muted, up to two lines.
    pub ask: Option<SharedString>,
    /// The latest reply: its first line draws in the state colour — unless
    /// the session waits on the owner, when the attention box replaces it.
    pub reply: Option<SharedString>,
    /// Status with its detail; picks the reply line's colour and, when the
    /// session waits on the owner, the attention box's label.
    pub status: Option<RowStatus>,
    /// Project name: merges with [`Self::workspace`] into the footer.
    pub project: Option<SharedString>,
    /// Branch name, on the meta row.
    pub branch: Option<SharedString>,
    /// Turn count, on the meta row as a bare number.
    pub turns: Option<usize>,
    /// When it last changed, in the caller's words (`12m ago`): the meta
    /// row's words, and the header's age with the trailing ` ago` dropped.
    pub updated: Option<SharedString>,
    /// Workspace path: merges with [`Self::project`] into the footer.
    pub workspace: Option<SharedString>,
    /// The pending question's prompt, when the open session holds one: with
    /// an [`RowStatusKind::Asked`] status this replaces the reply line with
    /// the attention box.
    pub pending_question: Option<SharedString>,
    /// The pending approval's exact command, when the open session holds
    /// one: with a [`RowStatusKind::NeedsApproval`] status this replaces the
    /// reply line with the attention box, ahead of [`Self::pending_question`].
    pub pending_approval: Option<SharedString>,
}

/// Empty detail content; fill with the builder methods on
/// [`SessionDetail`].
pub fn session_detail(id: impl Into<ElementId>) -> SessionDetail {
    SessionDetail {
        id: id.into(),
        title: None,
        ask: None,
        reply: None,
        status: None,
        project: None,
        branch: None,
        turns: None,
        updated: None,
        workspace: None,
        pending_question: None,
        pending_approval: None,
    }
}

/// The hover detail card. Build with [`session_detail`].
#[derive(IntoElement)]
pub struct SessionDetail {
    id: ElementId,
    title: Option<SharedString>,
    ask: Option<SharedString>,
    reply: Option<SharedString>,
    status: Option<RowStatus>,
    project: Option<SharedString>,
    branch: Option<SharedString>,
    turns: Option<usize>,
    updated: Option<SharedString>,
    workspace: Option<SharedString>,
    pending_question: Option<SharedString>,
    pending_approval: Option<SharedString>,
}

impl SessionDetail {
    /// The full title, wrapping, never truncated.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The owner's last message: quoted, muted, up to two lines.
    pub fn ask(mut self, ask: impl Into<SharedString>) -> Self {
        self.ask = Some(ask.into());
        self
    }

    /// The latest reply: its first line, in the state colour — unless the
    /// session waits on the owner, when the attention box replaces it.
    pub fn reply(mut self, reply: impl Into<SharedString>) -> Self {
        self.reply = Some(reply.into());
        self
    }

    /// The status with its detail; the library owns the colour and weight.
    /// An [`RowStatusKind::Asked`] / [`RowStatusKind::NeedsApproval`] status
    /// with pending words shows the attention box instead of the reply line.
    pub fn status(mut self, kind: RowStatusKind, detail: impl Into<SharedString>) -> Self {
        self.status = Some(RowStatus::new(kind, detail.into()));
        self
    }

    /// The project name: merges with the workspace into the footer.
    pub fn project(mut self, project: impl Into<SharedString>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// The branch name, on the meta row.
    pub fn branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// The turn count, drawn as a bare number on the meta row.
    pub fn turns(mut self, turns: usize) -> Self {
        self.turns = Some(turns);
        self
    }

    /// When it last changed, in the caller's words (`12m ago`).
    pub fn updated(mut self, updated: impl Into<SharedString>) -> Self {
        self.updated = Some(updated.into());
        self
    }

    /// The workspace path: merges with the project into the footer.
    pub fn workspace(mut self, workspace: impl Into<SharedString>) -> Self {
        self.workspace = Some(workspace.into());
        self
    }

    /// The pending question's prompt: with an [`RowStatusKind::Asked`]
    /// status this replaces the reply line with the attention box.
    pub fn pending_question(mut self, question: impl Into<SharedString>) -> Self {
        self.pending_question = Some(question.into());
        self
    }

    /// The pending approval's exact command: with a
    /// [`RowStatusKind::NeedsApproval`] status this replaces the reply line
    /// with the attention box, ahead of the pending question.
    pub fn pending_approval(mut self, command: impl Into<SharedString>) -> Self {
        self.pending_approval = Some(command.into());
        self
    }

    /// The data behind this card, for tests and tooling.
    pub fn data(&self) -> SessionDetailData {
        SessionDetailData {
            title: self.title.clone(),
            ask: self.ask.clone(),
            reply: self.reply.clone(),
            status: self.status.clone(),
            project: self.project.clone(),
            branch: self.branch.clone(),
            turns: self.turns,
            updated: self.updated.clone(),
            workspace: self.workspace.clone(),
            pending_question: self.pending_question.clone(),
            pending_approval: self.pending_approval.clone(),
        }
    }
}

/// Whether `text` is worth a row: present and non-blank.
fn has_text(text: &Option<SharedString>) -> bool {
    text.as_ref().is_some_and(|t| !t.trim().is_empty())
}

/// The first line of `text`, trimmed — the reply line never wraps.
fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_owned()
}

/// The first `max` newline-separated lines of `text`, trimmed at the ends;
/// an ellipsis marks lines dropped past the budget.
fn first_lines(text: &str, max: usize) -> String {
    let mut lines = text.lines().map(str::trim_end).filter(|line| !line.is_empty());
    let kept: Vec<&str> = lines.by_ref().take(max).collect();
    let mut out = kept.join("\n").trim().to_owned();
    if lines.next().is_some() && !out.is_empty() {
        out.push('…');
    }
    out
}

/// The header's age from the caller's `updated` words: `12m ago` reads `12m`
/// beside the title, `now` stays `now`.
fn age_text(updated: &str) -> &str {
    updated.strip_suffix(" ago").unwrap_or(updated)
}

/// Which waiting state the card is in, if any: an [`RowStatusKind::Asked`] /
/// [`RowStatusKind::NeedsApproval`] status with pending words. The explicit
/// pending fields win over the status detail; approval wins over the
/// question. Returns the box's label and its words.
fn attention_for(data: &SessionDetailData) -> Option<(&'static str, String)> {
    let status = data.status.as_ref()?;
    let explicit = match status.kind {
        RowStatusKind::NeedsApproval => data.pending_approval.as_deref().or(data.pending_question.as_deref()),
        RowStatusKind::Asked => data.pending_question.as_deref().or(data.pending_approval.as_deref()),
        _ => None,
    };
    let words = explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| match status.kind {
            RowStatusKind::Asked | RowStatusKind::NeedsApproval => {
                let detail = status.detail.trim();
                (!detail.is_empty()).then(|| detail.to_owned())
            }
            _ => None,
        });
    let words = words?;
    let label = match status.kind {
        RowStatusKind::NeedsApproval => "Needs approval:",
        _ => "Asked:",
    };
    Some((label, words))
}

/// Ink for the reply line, from the status: settled green, working accent,
/// failed danger, waiting warning, everything else muted.
fn reply_ink(status: Option<&RowStatus>, p: aui_tokens::Palette) -> gpui::Hsla {
    match status.map(|s| s.kind) {
        Some(RowStatusKind::Settled) => p.success,
        Some(RowStatusKind::Working) => p.accent_ink,
        Some(RowStatusKind::Failed) => p.danger,
        Some(RowStatusKind::Asked) | Some(RowStatusKind::NeedsApproval) => p.warning,
        Some(RowStatusKind::NoReply) | None => p.ink_3,
    }
}

impl RenderOnce for SessionDetail {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let data = self.data();
        let p = cx.aui().colors;
        let mut body = v_flex().w_full().gap(px(scale::SP_3));
        // Header: the full title wrapping, the relative time right-aligned
        // in its first line.
        if let Some(title) = data.title.clone().filter(|t| !t.trim().is_empty()) {
            let mut header = h_flex().w_full().items_start().gap(px(scale::SP_3));
            header = header.child(
                div().flex_1().min_w(px(0.0)).ui(scale::FS_13).semibold().text_color(p.ink).child(title),
            );
            if let Some(updated) = data.updated.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                header = header.child(
                    div()
                        .flex_none()
                        .font_family(scale::FONT_MONO)
                        .text_px(scale::FS_11)
                        .text_color(p.ink_3)
                        .child(SharedString::from(age_text(updated).to_owned())),
                );
            }
            body = body.child(header);
        }
        // The waiting state replaces the reply line with the attention box.
        let attention = attention_for(&data);
        // Ask and reply share the quote bar; the box stands beside it.
        let ask = data
            .ask
            .as_deref()
            .map(|ask| first_lines(ask, SESSION_DETAIL_ASK_LINES))
            .filter(|s| !s.is_empty());
        let reply = match &attention {
            Some(_) => None,
            None => {
                let own = data.reply.as_deref().map(first_line).filter(|s| !s.is_empty());
                own.or_else(|| data.status.as_ref().map(|status| status.text().to_string()))
            }
        };
        if ask.is_some() || reply.is_some() {
            let mut quote = v_flex()
                .w_full()
                .gap(px(scale::SP_1))
                .border_l(px(QUOTE_RAIL))
                .border_color(p.line_strong)
                .pl(px(scale::SP_3));
            if let Some(ask) = ask {
                quote = quote.child(
                    div().w_full().ui(scale::FS_12).text_color(p.ink_2).child(SharedString::from(format!("“{ask}”"))),
                );
            }
            if let Some(reply) = reply {
                quote = quote.child(
                    div()
                        .w_full()
                        .min_w(px(0.0))
                        .truncate()
                        .ui(scale::FS_12)
                        .semibold()
                        .text_color(reply_ink(data.status.as_ref(), p))
                        .child(SharedString::from(reply)),
                );
            }
            body = body.child(quote);
        }
        if let Some((label, words)) = attention {
            let shown = first_lines(&words, SESSION_DETAIL_ATTENTION_LINES);
            // A question reads quoted; a command reads verbatim.
            let quoted = label == "Asked:";
            let text = if quoted { format!("{label} “{shown}”") } else { format!("{label} {shown}") };
            let highlight = HighlightStyle {
                color: Some(p.ink),
                font_weight: Some(FontWeight::SEMIBOLD),
                ..Default::default()
            };
            let glyph = if quoted { IconName::Question } else { IconName::Shield };
            body = body.child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap(px(scale::SP_2))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.warning)
                    .bg(p.warning_soft)
                    .py(px(scale::SP_2))
                    .px(px(scale::SP_3))
                    .child(div().flex_none().child(icon(glyph).color(p.warning)))
                    .child(
                        div().flex_1().min_w(px(0.0)).ui(scale::FS_12).text_color(p.ink).child(
                            StyledText::new(text).with_highlights([(0..label.len(), highlight)]),
                        ),
                    ),
            );
        }
        // One inline meta row; the branch shrinks with an ellipsis so the
        // count and the age always fit.
        let mut meta = h_flex().w_full().items_center().gap(px(scale::SP_4));
        let mut meta_any = false;
        if let Some(branch) = data.branch.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            meta_any = true;
            meta = meta.child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .items_center()
                    .gap(px(scale::SP_2))
                    .text_role(aui_tokens::TextRole::Meta)
                    .text_color(p.ink_3)
                    .child(icon(IconName::Git))
                    .child(
                        div().flex_1().min_w(px(0.0)).truncate().child(SharedString::from(branch.to_owned())),
                    ),
            );
        }
        if let Some(turns) = data.turns {
            meta_any = true;
            meta = meta.child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap(px(scale::SP_2))
                    .text_role(aui_tokens::TextRole::Meta)
                    .text_color(p.ink_3)
                    .child(icon(IconName::List))
                    .child(SharedString::from(turns.to_string())),
            );
        }
        if let Some(updated) = data.updated.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            meta_any = true;
            meta = meta.child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap(px(scale::SP_2))
                    .text_role(aui_tokens::TextRole::Meta)
                    .text_color(p.ink_3)
                    .child(icon(IconName::Clock))
                    .child(SharedString::from(updated.to_owned())),
            );
        }
        if meta_any {
            body = body.child(meta);
        }
        // Footer: `project · workspace`, mono and muted — whichever half the
        // app has, or no row at all.
        let mut halves = Vec::new();
        if let Some(project) = data.project.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            halves.push(project.to_owned());
        }
        if let Some(workspace) = data.workspace.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            halves.push(workspace.to_owned());
        }
        if !halves.is_empty() {
            body = body.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(scale::SP_2))
                    .font_family(scale::FONT_MONO)
                    .text_px(scale::FS_11)
                    .text_color(p.ink_3)
                    .child(icon(IconName::Folder))
                    .child(
                        div().flex_1().min_w(px(0.0)).truncate().child(SharedString::from(halves.join(" · "))),
                    ),
            );
        }
        div()
            .id(self.id)
            .flex_none()
            .w(px(SESSION_DETAIL_WIDTH))
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.overlay)
            .shadow(p.shadow(2))
            .p(px(scale::SP_4))
            .child(body)
    }
}

/// The quote bar's rail: the `line-strong` 2 px the mockup quotes excerpts
/// behind.
const QUOTE_RAIL: f32 = 2.0;

/// Seats a [`SessionDetail`] at its row: below-start of `trigger` in
/// [`crate::overlay::popover_layer`], flipping above near the window bottom
/// and sliding inside — the one seat rule, so the card escapes the sidebar's
/// clipping and stays in the window at the top and bottom edges.
///
/// `trigger` is the hovered row's bounds in window coordinates (measured in
/// prepaint with `on_children_prepainted`); the card hangs below the row, so
/// it never covers the row's own click target.
pub fn anchored_session_detail(trigger: Bounds<Pixels>, detail: SessionDetail) -> impl IntoElement {
    anchored_menu(trigger, MenuSide::Below, MenuAlign::Start, detail)
}

/// The gap between the sidebar's right edge and the side-seated detail card,
/// and the margin it keeps to the window on every side: the `SP_2` spacing
/// token, the same gap [`crate::overlay::anchored_menu`] hangs its menus off.
pub const SESSION_DETAIL_GAP: f32 = scale::SP_2;

/// Where a side-seated detail card's top-left goes: the sidebar pane's right
/// edge plus [`SESSION_DETAIL_GAP`], top-aligned with the hovered row —
/// regardless of where in the row the pointer is, so the card never overlaps
/// the sidebar. A row near the window bottom shifts up just enough to keep
/// the whole card inside with the same margin, and never above the window's
/// top; a card taller than the window rests at the top margin.
///
/// `row` is the hovered row's bounds in window coordinates (what the row's
/// hover report carries), `sidebar_right` the laid-out sidebar pane's right
/// edge, `window` the window bounds, `card` the card's measured size. Pure,
/// so the seat is unit-testable without a window; the element below trusts
/// gpui's fit to apply the same slide at layout.
pub fn session_detail_side_origin(
    row: Bounds<Pixels>,
    sidebar_right: Pixels,
    window: Bounds<Pixels>,
    card: Size<Pixels>,
) -> Point<Pixels> {
    let gap = px(SESSION_DETAIL_GAP);
    let x = sidebar_right + gap;
    let top = f32::from(row.origin.y);
    let margin = f32::from(gap);
    let window_top = f32::from(window.origin.y) + margin;
    let window_bottom = f32::from(window.origin.y) + f32::from(window.size.height) - margin;
    let y = top.max(window_top).min((window_bottom - f32::from(card.height)).max(window_top));
    point(x, px(y))
}

/// Seats a [`SessionDetail`] beside the sidebar at its row: the card's left
/// edge sits at the sidebar pane's right edge plus [`SESSION_DETAIL_GAP`]
/// with its top at the hovered row's top, in [`crate::overlay::popover_layer`]
/// so it escapes the sidebar's clipping and paints above everything. The
/// slide-only fit (`snap_to_window_with_margin`) shifts a bottom row's card
/// up just enough to stay inside with the same margin — never flipping over
/// the sidebar the way [`anchored_menu`]'s anchor switch would.
///
/// `row` is the hovered row's bounds and `sidebar_right` the laid-out pane's
/// right edge, both in window coordinates; the card's contents are unchanged.
pub fn anchored_session_detail_at_sidebar(
    row: Bounds<Pixels>,
    sidebar_right: Pixels,
    detail: SessionDetail,
) -> impl IntoElement {
    let gap = px(SESSION_DETAIL_GAP);
    let at = point(sidebar_right + gap, row.origin.y);
    crate::overlay::popover_layer(
        gpui::anchored().anchor(Anchor::TopLeft).position(at).snap_to_window_with_margin(gap).child(detail),
    )
}

/// Horizontal detail preview used by tests: the blocks this card would draw,
/// in order, for `data` — title first when set, then the ask, then either
/// the attention box (waiting with pending words) or the reply line, then
/// each set meta field, then the path footer when the app names a project
/// or workspace.
pub fn detail_keys(data: &SessionDetailData) -> Vec<&'static str> {
    let mut keys = Vec::new();
    if data.title.as_ref().is_some_and(|t| !t.trim().is_empty()) {
        keys.push("Title");
    }
    if has_text(&data.ask) {
        keys.push("Ask");
    }
    if attention_for(data).is_some() {
        keys.push("Attention");
    } else if has_text(&data.reply) || data.status.is_some() {
        keys.push("Reply");
    }
    if has_text(&data.branch) {
        keys.push("Branch");
    }
    if data.turns.is_some() {
        keys.push("Turns");
    }
    if has_text(&data.updated) {
        keys.push("Updated");
    }
    if has_text(&data.project) || has_text(&data.workspace) {
        keys.push("Path");
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only set fields draw, in card order; blank fields leave no row.
    #[test]
    fn detail_keys_follow_set_fields_in_order() {
        let full = session_detail("d")
            .title("checkout-flow-v2")
            .ask("Tighten validation")
            .reply("Patched the validator")
            .status(RowStatusKind::Working, "14m")
            .project("acme-web")
            .branch("feature/checkout")
            .turns(5)
            .updated("12m ago")
            .workspace("~/Projects/acme-web")
            .data();
        assert_eq!(
            detail_keys(&full),
            vec!["Title", "Ask", "Reply", "Branch", "Turns", "Updated", "Path"]
        );
        let sparse = session_detail("d").title("Blank slate").data();
        assert_eq!(detail_keys(&sparse), vec!["Title"]);
        let empty = session_detail("d").data();
        assert!(detail_keys(&empty).is_empty());
        // Whitespace-only fields draw nothing; one half of the footer still
        // draws the path row.
        let blanks = session_detail("d").ask("   ").branch("  ").project("acme-web").data();
        assert_eq!(detail_keys(&blanks), vec!["Path"]);
    }

    /// The question box replaces the reply line: a waiting session with
    /// pending words draws Attention, never Reply — even when a reply is set.
    #[test]
    fn question_box_replaces_the_reply_line() {
        let asked = session_detail("d")
            .title("auth-session-refresh")
            .ask("Refresh the session tokens")
            .reply("Loaded the auth client")
            .status(RowStatusKind::Asked, "")
            .pending_question("Which bucket for staging?")
            .data();
        assert_eq!(detail_keys(&asked), vec!["Title", "Ask", "Attention"]);
        let (label, words) = attention_for(&asked).expect("pending words draw the box");
        assert_eq!(label, "Asked:");
        assert_eq!(words, "Which bucket for staging?");
        // No pending words anywhere: the reply line stands.
        let bare = session_detail("d").reply("Loaded the auth client").status(RowStatusKind::Asked, "").data();
        assert!(attention_for(&bare).is_none());
        assert_eq!(detail_keys(&bare), vec!["Reply"]);
    }

    /// Approval wins over the question, and the status detail is the
    /// fallback when neither pending field is set.
    #[test]
    fn approval_wins_and_status_detail_falls_back() {
        let approval = session_detail("d")
            .status(RowStatusKind::NeedsApproval, "")
            .pending_question("Which bucket for staging?")
            .pending_approval("cargo install notifierd")
            .data();
        assert_eq!(detail_keys(&approval), vec!["Attention"]);
        let (label, words) = attention_for(&approval).expect("approval words draw the box");
        assert_eq!(label, "Needs approval:");
        assert_eq!(words, "cargo install notifierd");
        // A closed session's flag-only wait: the status detail carries the
        // words when the open session holds none.
        let flagged = session_detail("d").status(RowStatusKind::Asked, "Which bucket for staging?").data();
        let (label, words) = attention_for(&flagged).expect("status detail falls back");
        assert_eq!(label, "Asked:");
        assert_eq!(words, "Which bucket for staging?");
        assert_eq!(detail_keys(&flagged), vec!["Attention"]);
        // A non-waiting status never draws the box, whatever is set.
        let settled = session_detail("d")
            .status(RowStatusKind::Settled, "12m · 5 turns")
            .pending_question("Which bucket for staging?")
            .data();
        assert!(attention_for(&settled).is_none());
        assert_eq!(detail_keys(&settled), vec!["Reply"]);
    }

    /// The header age drops the trailing ` ago`; the reply line keeps the
    /// first line only and the excerpts keep their line budgets.
    #[test]
    fn excerpt_budgets_hold() {
        assert_eq!(age_text("12m ago"), "12m");
        assert_eq!(age_text("now"), "now");
        assert_eq!(first_line("Patched the validator\nsecond line"), "Patched the validator");
        assert_eq!(first_lines("one\n\ntwo\nthree", SESSION_DETAIL_ASK_LINES), "one\ntwo…");
        assert_eq!(first_lines("one\ntwo", SESSION_DETAIL_ASK_LINES), "one\ntwo");
        assert_eq!(first_lines("   ", SESSION_DETAIL_ASK_LINES), "");
    }

    /// The hover delay is the slow motion token.
    #[test]
    fn hover_delay_is_the_slow_token() {
        assert_eq!(SESSION_DETAIL_DELAY, durations::SLOW);
    }

    /// The side gap is the `SP_2` spacing token, not a literal.
    #[test]
    fn side_gap_is_the_spacing_token() {
        assert_eq!(SESSION_DETAIL_GAP, scale::SP_2);
    }

    fn window_800x600() -> Bounds<Pixels> {
        Bounds::new(gpui::point(px(0.0), px(0.0)), gpui::size(px(800.0), px(600.0)))
    }

    fn row_at(y: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(gpui::point(px(8.0), px(y)), gpui::size(px(236.0), px(h)))
    }

    /// The seat ignores the pointer: the row reports the same bounds
    /// wherever in it the pointer sits, so rows at the same height seat
    /// identically — the card's left at the sidebar edge plus the gap, its
    /// top at the row's top.
    #[test]
    fn side_seat_ignores_pointer_x_and_aligns_to_the_row() {
        let window = window_800x600();
        let card = gpui::size(px(SESSION_DETAIL_WIDTH), px(200.0));
        let row = row_at(140.0, 62.0);
        let at = session_detail_side_origin(row, px(252.0), window, card);
        assert_eq!(at, gpui::point(px(256.0), px(140.0)));
        // Same height, different x-extent: still the same seat.
        let shifted = Bounds::new(gpui::point(px(100.0), px(140.0)), gpui::size(px(60.0), px(62.0)));
        assert_eq!(session_detail_side_origin(shifted, px(252.0), window, card), at);
    }


    /// A row near the bottom shifts up just enough to stay inside the window
    /// with the same margin.
    #[test]
    fn side_seat_near_the_bottom_shifts_up_and_stays_inside() {
        let window = window_800x600();
        let card = gpui::size(px(SESSION_DETAIL_WIDTH), px(200.0));
        // Row top 500 with a 200 px card would end at 700 past the 596 margin.
        let at = session_detail_side_origin(row_at(500.0, 62.0), px(252.0), window, card);
        assert_eq!(at, gpui::point(px(256.0), px(396.0)));
        assert!(f32::from(at.y) + 200.0 <= 600.0 - SESSION_DETAIL_GAP);
    }

    /// The seat never climbs above the window's top, even for a card taller
    /// than the window.
    #[test]
    fn side_seat_never_passes_the_window_top() {
        let window = window_800x600();
        let tall = gpui::size(px(SESSION_DETAIL_WIDTH), px(900.0));
        let at = session_detail_side_origin(row_at(500.0, 62.0), px(252.0), window, tall);
        assert_eq!(at, gpui::point(px(256.0), px(SESSION_DETAIL_GAP)));
    }

    /// A narrow and a wide sidebar seat the card at their own right edge.
    #[test]
    fn side_seat_follows_narrow_and_wide_sidebars() {
        let window = window_800x600();
        let card = gpui::size(px(SESSION_DETAIL_WIDTH), px(200.0));
        let narrow = session_detail_side_origin(row_at(140.0, 62.0), px(180.0), window, card);
        assert_eq!(narrow, gpui::point(px(184.0), px(140.0)));
        let wide = session_detail_side_origin(row_at(140.0, 62.0), px(400.0), window, card);
        assert_eq!(wide, gpui::point(px(404.0), px(140.0)));
    }
}
