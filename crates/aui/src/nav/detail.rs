//! Option B's hover detail: the whole picture at 260 px.
//!
//! The sidebar row truncates, so hovering a row shows a calm key/value card
//! with everything the app knows: the full title, ask, latest reply, status
//! (with its detail — the pending question, the approval command), project,
//! branch, turn count and last change. Caller-supplied and optional per
//! field: only the fields the app sets are drawn.
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

use aui_tokens::{durations, scale, ActiveAui, AuiStyled};
use gpui::{
    div, point, prelude::*, px, Anchor, AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, SharedString,
    Size, Window,
};
use gpui_kit::base::v_flex;

use crate::nav::{RowStatus, RowStatusKind};
use crate::overlay::{anchored_menu, MenuAlign, MenuSide};

/// How long the app waits after hover before showing the detail: the motion
/// token `slow` (280 ms) — long enough that a pointer travelling past rows
/// never flashes the card, short enough that an intentional hover feels
/// immediate.
pub const SESSION_DETAIL_DELAY: Duration = durations::SLOW;

/// Width of the detail card: wide enough for a full ask at the sidebar's
/// narrow end, narrow enough to sit beside a 260 px sidebar.
pub const SESSION_DETAIL_WIDTH: f32 = 300.0;

/// The hover detail's caller-supplied content. Build with
/// [`session_detail`]; every field is optional, and only set fields draw.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionDetailData {
    /// Full title (no truncation).
    pub title: Option<SharedString>,
    /// Full ask.
    pub ask: Option<SharedString>,
    /// Full latest reply.
    pub reply: Option<SharedString>,
    /// Status with its detail (pending question, approval command, elapsed).
    pub status: Option<RowStatus>,
    /// Project name.
    pub project: Option<SharedString>,
    /// Branch name.
    pub branch: Option<SharedString>,
    /// Turn count.
    pub turns: Option<usize>,
    /// When it last changed, in the caller's words (`12m ago`).
    pub updated: Option<SharedString>,
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
}

impl SessionDetail {
    /// The full title, wrapped, never truncated.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The full ask.
    pub fn ask(mut self, ask: impl Into<SharedString>) -> Self {
        self.ask = Some(ask.into());
        self
    }

    /// The full latest reply.
    pub fn reply(mut self, reply: impl Into<SharedString>) -> Self {
        self.reply = Some(reply.into());
        self
    }

    /// The status with its detail; the library owns the colour and weight.
    pub fn status(mut self, kind: RowStatusKind, detail: impl Into<SharedString>) -> Self {
        self.status = Some(RowStatus::new(kind, detail.into()));
        self
    }

    /// The project name.
    pub fn project(mut self, project: impl Into<SharedString>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// The branch name.
    pub fn branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// The turn count, drawn as `1 turn` / `N turns`.
    pub fn turns(mut self, turns: usize) -> Self {
        self.turns = Some(turns);
        self
    }

    /// When it last changed, in the caller's words.
    pub fn updated(mut self, updated: impl Into<SharedString>) -> Self {
        self.updated = Some(updated.into());
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
        }
    }
}

/// Whether `text` is worth a row: present and non-blank.
fn has_text(text: &Option<SharedString>) -> bool {
    text.as_ref().is_some_and(|t| !t.trim().is_empty())
}

impl RenderOnce for SessionDetail {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut body = v_flex().w_full().gap(px(scale::SP_3));
        if let Some(title) = self.title.filter(|t| !t.trim().is_empty()) {
            body = body.child(div().w_full().ui(scale::FS_13).semibold().text_color(p.ink).child(title));
        }
        // Key/value rows, in card order; only set fields draw.
        let mut rows: Vec<AnyElement> = Vec::new();
        if has_text(&self.ask) {
            let value = self.ask.clone().unwrap_or_default();
            rows.push(key_value("Ask", div().w_full().ui(scale::FS_12).text_color(p.ink_2).child(value).into_any_element(), p));
        }
        if has_text(&self.reply) {
            let value = self.reply.clone().unwrap_or_default();
            rows.push(key_value(
                "Reply",
                div().w_full().ui(scale::FS_12).text_color(p.ink_2).child(value).into_any_element(),
                p,
            ));
        }
        if let Some(status) = &self.status {
            rows.push(key_value(
                "Status",
                div().w_full().ui(scale::FS_12).semibold().text_color(status.ink(&p)).child(status.text()).into_any_element(),
                p,
            ));
        }
        if has_text(&self.project) {
            let value = self.project.clone().unwrap_or_default();
            rows.push(key_value(
                "Project",
                div().w_full().ui(scale::FS_12).text_color(p.ink_2).child(value).into_any_element(),
                p,
            ));
        }
        if has_text(&self.branch) {
            let value = self.branch.clone().unwrap_or_default();
            rows.push(key_value(
                "Branch",
                div()
                    .w_full()
                    .font_family(scale::FONT_MONO)
                    .text_px(scale::FS_11)
                    .text_color(p.ink_3)
                    .child(value)
                    .into_any_element(),
                p,
            ));
        }
        if let Some(turns) = self.turns {
            let words: SharedString = if turns == 1 { "1 turn".into() } else { format!("{turns} turns").into() };
            rows.push(key_value("Turns", div().w_full().ui(scale::FS_12).text_color(p.ink_2).child(words).into_any_element(), p));
        }
        if has_text(&self.updated) {
            let value = self.updated.clone().unwrap_or_default();
            rows.push(key_value(
                "Updated",
                div().w_full().ui(scale::FS_12).text_color(p.ink_3).child(value).into_any_element(),
                p,
            ));
        }
        body = body.children(rows);
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

/// One labelled row: the key small caps muted, the value beneath it.
fn key_value(key: &'static str, value: AnyElement, p: aui_tokens::Palette) -> AnyElement {
    v_flex()
        .w_full()
        .gap(px(2.0))
        .child(div().w_full().text_role(aui_tokens::TextRole::Caps).text_color(p.ink_3).child(key))
        .child(value)
        .into_any_element()
}

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

/// Horizontal detail preview used by tests: the keys this card would draw,
/// in order, for `data` — title first when set, then each set field.
pub fn detail_keys(data: &SessionDetailData) -> Vec<&'static str> {
    let mut keys = Vec::new();
    if data.title.as_ref().is_some_and(|t| !t.trim().is_empty()) {
        keys.push("Title");
    }
    if has_text(&data.ask) {
        keys.push("Ask");
    }
    if has_text(&data.reply) {
        keys.push("Reply");
    }
    if data.status.is_some() {
        keys.push("Status");
    }
    if has_text(&data.project) {
        keys.push("Project");
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
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only set fields draw, in card order.
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
            .data();
        assert_eq!(detail_keys(&full), vec!["Title", "Ask", "Reply", "Status", "Project", "Branch", "Turns", "Updated"]);
        let sparse = session_detail("d").title("Blank slate").data();
        assert_eq!(detail_keys(&sparse), vec!["Title"]);
        let empty = session_detail("d").data();
        assert!(detail_keys(&empty).is_empty());
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
