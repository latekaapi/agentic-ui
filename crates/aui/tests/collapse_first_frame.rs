//! F1: an open collapse paints its rows on the very first frame.
//!
//! Regression test for the harness finding behind [`aui_motion::collapse`]:
//! the old reveal registered no layout child before its first measure, so the
//! node's first frame was height 0 and the rows only appeared on a later
//! repaint (in a quiet window, that repaint never came). `add_window_view`
//! draws exactly once; reading `painted_quads` before any further
//! notify/simulate call inspects that first frame, so an open group must
//! already paint its rows' quads there.

use aui::nav::{sidebar_view, Grouping, ProjectGroup, SessionSummary};
use aui::tokens::AgentState;
use gpui::{IntoElement, TestAppContext};

/// One project group holding three sessions. Closed groups carry no sessions
/// through the public API (callers mount bodies from `group.sessions`), so
/// the closed control is the realistic empty group; under the bug both paint
/// zero row quads on frame one.
fn grouping(open: bool) -> Grouping {
    let sessions = vec![
        SessionSummary::new("one", "first-session", AgentState::Done, "4h"),
        SessionSummary::new("two", "second-session", AgentState::Idle, "2d"),
        SessionSummary::new("three", "third-session", AgentState::Done, "1d"),
    ];
    let group = ProjectGroup::new("proj", "acme-web", "3");
    Grouping::Project(vec![if open { group.open(sessions) } else { group }])
}

struct Host {
    grouping: Grouping,
}

impl gpui::Render for Host {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        sidebar_view("collapse-first-frame", self.grouping.clone())
    }
}

/// Draws `grouping` in a fresh window and returns its first frame's quads.
fn first_frame_quads(cx: &mut TestAppContext, grouping: Grouping) -> Vec<gpui::Quad> {
    let (_host, cx) = cx.add_window_view(|_, _| Host { grouping });
    cx.update(|window, _| window.painted_quads())
}

#[gpui::test]
fn an_open_project_group_paints_its_rows_on_the_first_frame(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let open = first_frame_quads(cx, grouping(true));
    let closed = first_frame_quads(cx, grouping(false));
    assert!(
        open.len() >= 3,
        "an open group of three sessions painted {} quads on its first frame, expected at least one per row",
        open.len()
    );
    assert!(
        open.len() > closed.len(),
        "first frame painted {} quads open vs {} closed: the rows were culled to height 0",
        open.len(),
        closed.len()
    );
}
