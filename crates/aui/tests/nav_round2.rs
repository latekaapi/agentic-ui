//! Owner round 2: the folded project row.
//!
//! `painted_quads` only sees solid fills, the pointer is inert under
//! `#[gpui::test]` and headless windows have no rasterizer — so text, the
//! pin glyph and hover grounds are all invisible here, and the gallery
//! screenshots cover them instead. What a test *can* prove is intent:
//! the test host wires no handler but `on_group_action`, so a click sweep
//! down the column can only log through the fold row. The sweep band
//! (40–120 px) covers the session row and the 28 px fold row beneath it
//! with room for the row metrics to drift; clicks anywhere else log
//! nothing, which is also what the no-fold cases assert.

use std::cell::RefCell;
use std::rc::Rc;

use aui::nav::{sidebar_view, GroupAction, Grouping, ProjectGroup, SessionSummary};
use aui::tokens::AgentState;
use gpui::{point, px, IntoElement, Modifiers, SharedString, TestAppContext, VisualTestContext};

/// Sweep band: below the 34 px project row, across the session row and the
/// fold row under it.
const SWEEP_TOP: f32 = 40.0;
const SWEEP_BOTTOM: f32 = 120.0;
const SWEEP_STEP: f32 = 4.0;
/// Clicks land mid-column; rows span the full test window width.
const SWEEP_X: f32 = 200.0;

/// What the view reported, in order.
type Log = Rc<RefCell<Vec<String>>>;

/// One open project group holding one session, optionally folded.
fn grouping(fold: Option<(usize, bool)>) -> Grouping {
    let sessions = vec![SessionSummary::new("one", "first-session", AgentState::Done, "4h")];
    let mut group = ProjectGroup::new("proj", "acme-web", "1").open(sessions);
    group.fold = fold;
    Grouping::Project(vec![group])
}

struct FoldHost {
    grouping: Grouping,
    log: Log,
}

impl gpui::Render for FoldHost {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let log = self.log.clone();
        sidebar_view("round2-fold", self.grouping.clone()).on_group_action(move |id: &SharedString, action: GroupAction, _, _| {
            log.borrow_mut().push(format!("{id}:{action:?}"));
        })
    }
}

/// Clicks down the column of a fresh `fold` window; returns what the view
/// reported. Only the fold row has a handler, so only it can log.
fn sweep_clicks(cx: &mut TestAppContext, fold: Option<(usize, bool)>) -> Vec<String> {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        |_, _| FoldHost { grouping: grouping(fold), log }
    });
    let vcx: &mut VisualTestContext = vcx;
    let mut y = SWEEP_TOP;
    while y <= SWEEP_BOTTOM {
        vcx.simulate_click(point(px(SWEEP_X), px(y)), Modifiers::none());
        vcx.run_until_parked();
        y += SWEEP_STEP;
    }
    let out = log.borrow().clone();
    out
}

/// Clicking the fold row reports `ToggleMore` for its group — and nothing
/// else in the band reports at all.
#[gpui::test]
fn fold_row_click_reports_toggle_more(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let log = sweep_clicks(cx, Some((12, false)));
    assert!(!log.is_empty(), "no click in the band reached the fold row");
    assert!(log.iter().all(|entry| entry == "proj:ToggleMore"), "unexpected report: {log:?}");
}

/// The expanded row ("Show less") reports through the same path.
#[gpui::test]
fn expanded_fold_row_click_reports_toggle_more(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let log = sweep_clicks(cx, Some((3, true)));
    assert!(!log.is_empty(), "no click in the band reached the expanded row");
    assert!(log.iter().all(|entry| entry == "proj:ToggleMore"), "unexpected report: {log:?}");
}

/// Nothing held back, no row: the sweep clicks nothing with a handler in
/// the plain group or in `folded(0, _)`.
#[gpui::test]
fn no_fold_row_when_nothing_is_hidden(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    for fold in [None, Some((0, false))] {
        assert!(sweep_clicks(cx, fold).is_empty(), "fold={fold:?} reported without a fold row");
    }
}
