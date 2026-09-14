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

/// The rename probe: an editor-shaped element (a fixed-size box like the dense
/// rename field) carrying a `canvas` that records every paint. Only an editor
/// that reaches the prepainted tree paints — one claimed by a discarded
/// first pass never does.
///
/// The canvas also forces the multi-build frame: its prepaint requests an
/// autoscroll just above the row, the way a field keeping its cursor visible
/// would, and gpui-pre 0.3.3's `list` answers an in-prepaint autoscroll by
/// laying out every visible row a second time in the same frame (`list.rs`
/// `prepaint` retries `prepaint_items` with `autoscroll=false`). A take-once
/// editor slot loses to that second pass.
fn probe_editor(paints: &Rc<Cell<usize>>) -> impl IntoElement {
    let paints = paints.clone();
    gpui::div().w(gpui::px(120.)).h(gpui::px(20.)).child(gpui::canvas(
        |bounds, window, _| {
            // Wholly above the viewport: the list answers with its
            // autoscroll retry, laying every visible row out a second time
            // in this same frame.
            window.request_autoscroll(gpui::Bounds::from_corners(
                point(bounds.left(), bounds.top() - px(1000.)),
                point(bounds.right(), bounds.top() - px(900.)),
            ));
        },
        move |_, _, _, _| {
            paints.set(paints.get() + 1);
        },
    ))
}

/// A host mid-rename: the renaming row's editor is a fresh probe element per
/// host render (one slot per frame, as an app builds it), and `on_row_built`
/// records every build of the renaming row's flattened index.
struct RenameHost {
    grouping: Grouping,
    state: gpui::ListState,
    target_ix: usize,
    paints: Rc<Cell<usize>>,
    builds: Rc<Cell<usize>>,
}

impl Render for RenameHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let paints = self.paints.clone();
        let builds = self.builds.clone();
        let target_ix = self.target_ix;
        v_flex().w(px(300.)).h(px(700.)).child(
            virtual_sidebar_view("rename", self.grouping.clone(), self.state.clone())
                .editing("r1", move |_, _| probe_editor(&paints).into_any_element())
                .on_row_built(move |ix| {
                    if ix == target_ix {
                        builds.set(builds.get() + 1);
                    }
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

/// The rename editor must reach the painted row even though the list can
/// build a visible row twice in one frame (first prepaint pass, then the
/// autoscroll retry's second pass): with a take-once slot the first build
/// claims the editor and the painted build draws the plain row, so the field
/// vanishes while the sidebar scrolls.
#[gpui::test]
fn renaming_editor_reaches_the_painted_row_across_remeasure(cx: &mut TestAppContext) {
    init(cx);
    let cx = cx.add_empty_window();
    // One open group of three sessions: the renaming row sits at the top of
    // a 300x700 panel, on screen with no scrolling needed.
    let grouping = Grouping::Project(vec![
        ProjectGroup::new("p", "project", "3").open(vec![
            SessionSummary::new("r1", "first", AgentState::Idle, "1h"),
            SessionSummary::new("r2", "second", AgentState::Idle, "1h"),
            SessionSummary::new("r3", "third", AgentState::Idle, "1h"),
        ]),
    ]);
    let rows = flatten_sidebar(&grouping, false);
    let target_ix = row_index_for_session(&rows, &grouping, &"r1".into()).expect("renaming row has an index");
    let state = sidebar_list_state(rows.len());
    let paints = Rc::new(Cell::new(0usize));
    let builds = Rc::new(Cell::new(0usize));

    // One frame: the autoscroll the probe requests mid-prepaint makes the
    // list lay the visible rows out twice in this same frame, with the
    // renaming row on screen throughout (the overshoot only steers the top
    // edge).
    builds.set(0);
    paints.set(0);
    let host = RenameHost {
        grouping: grouping.clone(),
        state: state.clone(),
        target_ix,
        paints: paints.clone(),
        builds: builds.clone(),
    };
    cx.draw(point(px(0.), px(0.)), size(px(300.), px(700.)), |_, cx| cx.new(|_| host).into_any_element());

    assert!(
        builds.get() >= 2,
        "the renaming row must build more than once in this frame (first pass, then the autoscroll retry): {}",
        builds.get()
    );
    println!("RENAME builds={} paints={}", builds.get(), paints.get());
    assert!(
        paints.get() > 0,
        "the renaming row's editor painted {} times: the measure build stole it",
        paints.get()
    );
}
