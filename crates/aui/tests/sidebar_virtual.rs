//! The virtualised sidebar builds O(visible) rows per frame, not one per
//! session: with 200 sessions in a ~700 px viewport, rows built per frame
//! stay within visible + overdraw.

use std::cell::Cell;
use std::rc::Rc;

use aui::nav::{ensure_row_visible, flatten_sidebar, row_index_for_session, sidebar_list_state, virtual_sidebar_view, Grouping, ProjectGroup};
use aui::nav::{ActivityKind, SessionSummary};
use aui_tokens::AgentState;
use gpui::{point, prelude::*, px, size, AppContext, Context, IntoElement, Render, TestAppContext, Window};
use gpui_kit::base::v_flex;

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
}

/// Eight open groups of 25 sessions: 200 sessions, 8 heads, 3 fold rows
/// (groups 0, 3, 6 hold rows back), one caption — 212 rows, only ~20 of them
/// visible in a 700 px viewport.
fn grouping() -> Grouping {
    let mut groups = Vec::new();
    for g in 0..8 {
        let mut sessions = Vec::new();
        for s in 0..25 {
            let id = format!("g{g}-s{s}");
            let mut summary = SessionSummary::new(id.clone(), id.clone(), AgentState::Idle, "1h");
            if s % 2 == 0 {
                summary = summary.repo("acme-web");
            }
            if s % 7 == 0 {
                summary = summary.activity(ActivityKind::Working, "running regression tests");
            }
            sessions.push(summary);
        }
        let mut group = ProjectGroup::new(format!("g{g}"), format!("project-{g}"), "25").open(sessions);
        if g % 3 == 0 {
            group = group.folded(10, false);
        }
        groups.push(group);
    }
    Grouping::Project(groups)
}

/// The host owns what the component does not: the caller-owned `ListState`
/// and the build counter the `on_row_built` hook records into.
struct Host {
    grouping: Grouping,
    state: gpui::ListState,
    built: Rc<Cell<usize>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let built = self.built.clone();
        v_flex().w(px(300.)).h(px(700.)).child(
            virtual_sidebar_view("bench", self.grouping.clone(), self.state.clone())
                .caption("Projects")
                .selected("g6-s12")
                .on_row_built(move |_| {
                    built.set(built.get() + 1);
                }),
        )
    }
}

/// One layout+paint pass of a fresh host in a 300x700 panel; returns rows
/// built by the list. Mounting through an entity (rather than drawing the
/// component bare) is what seats the rendered-view stack the list's paint
/// reads for its wheel listener — the same shape an app uses.
fn draw_once(cx: &mut gpui::VisualTestContext, grouping: &Grouping, state: &gpui::ListState, built: &Rc<Cell<usize>>) -> usize {
    built.set(0);
    let host = Host { grouping: grouping.clone(), state: state.clone(), built: built.clone() };
    let built = built.clone();
    cx.draw(point(px(0.), px(0.)), size(px(300.), px(700.)), |_, cx| cx.new(|_| host).into_any_element());
    built.get()
}

#[gpui::test]
fn virtual_sidebar_builds_only_visible_rows_per_frame(cx: &mut TestAppContext) {
    init(cx);
    let cx = cx.add_empty_window();
    let grouping = grouping();
    let rows = flatten_sidebar(&grouping, true);
    let total = rows.len();
    // 1 caption + 8 heads + 200 sessions + 3 fold rows.
    assert_eq!(total, 212, "the bench model must stay large enough to prove virtualisation");

    let selected = row_index_for_session(&rows, &grouping, &"g6-s12".into());
    assert!(selected.is_some_and(|ix| ix > 100), "the selected row sits deep in the list: {selected:?}");

    let state = sidebar_list_state(total);
    let built = Rc::new(Cell::new(0usize));

    // Budget: every row is at least the 30 px row metric, so a 700 px
    // viewport shows at most 24 rows; the 120 px overdraw runway adds at most
    // 4 measured rows a side, plus one boundary row. 48 leaves margin for
    // text-height variance and measure passes — still under a quarter of the
    // 212 rows a non-virtualised frame would build.
    let first = draw_once(cx, &grouping, &state, &built);
    assert!(first <= 48, "cold frame built {first} rows of {total}");
    assert!(first > 10, "cold frame built suspiciously few rows: {first}");

    let warm = draw_once(cx, &grouping, &state, &built);
    assert!(warm <= 48, "warm frame built {warm} rows of {total}");

    state.scroll_by(px(2000.));
    let scrolled = draw_once(cx, &grouping, &state, &built);
    assert!(scrolled <= 48, "scrolled frame built {scrolled} rows of {total}");
    assert!(state.logical_scroll_top().item_ix > 0, "scroll_by moved the list");

    // The reveal helper brings the last row into the viewport.
    let last = row_index_for_session(&rows, &grouping, &"g7-s24".into()).expect("last session has a row");
    ensure_row_visible(&state, last);
    draw_once(cx, &grouping, &state, &built);
    draw_once(cx, &grouping, &state, &built);
    assert!(state.bounds_for_item(last).is_some(), "revealed row has bounds");
    assert_eq!(state.item_is_below_viewport(last), Some(false), "revealed row is on screen");
}
