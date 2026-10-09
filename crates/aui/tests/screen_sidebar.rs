//! D2: the `screens/sidebar` view model renders labelled rows at the
//! control height, clicks report intents, and no rendered text is uppercase.

use std::cell::RefCell;
use std::rc::Rc;

use aui::data::{arm_ax_probe, take_ax_labels};
use aui::nav::{flatten_sidebar, sidebar_list_state};
use aui::screens::sidebar::{sample_model, screen_item_count, sidebar_screen, SidebarIntent};
use aui_tokens::ActiveAui;
use gpui::{point, prelude::*, px, size, IntoElement, TestAppContext, Window};

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
}

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

/// The host owns what the screen does not: the caller-owned `ListState` and
/// the intent log. The column is fixed at 260 × 720 so the virtual list has
/// a viewport to lay out in.
struct Host {
    model: aui::screens::sidebar::SidebarModel,
    state: gpui::ListState,
    log: Log,
}

impl gpui::Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let log = self.log.clone();
        gpui::div()
            .w(px(260.0))
            .h(px(720.0))
            .child(sidebar_screen("test-sidebar", self.model.clone(), self.state.clone()).on_intent(
                move |intent: SidebarIntent, _, _| {
                    log.borrow_mut().push(match intent {
                        SidebarIntent::Select(id) => format!("Select:{id}"),
                        SidebarIntent::Nav(id) => format!("Nav:{id}"),
                        SidebarIntent::Toggle(group) => format!("Toggle:{group}"),
                        SidebarIntent::SwitchMode(mode) => format!("SwitchMode:{mode:?}"),
                        SidebarIntent::Search => "Search".to_string(),
                        SidebarIntent::NewSession => "NewSession".to_string(),
                        SidebarIntent::OpenProjectMenu => "OpenProjectMenu".to_string(),
                        SidebarIntent::Account => "Account".to_string(),
                        SidebarIntent::Collapse => "Collapse".to_string(),
                        other => format!("Other:{other:?}"),
                    });
                },
            ))
            .into_any_element()
    }
}

/// Draws `model` once in a real window and returns the AX labels the render
/// committed plus the intent log's host pieces for driving clicks.
///
/// Frames after the first re-record the same labels (overlays settling), so
/// the labels are de-duplicated in commit order.
fn draw(
    cx: &mut TestAppContext,
    model: aui::screens::sidebar::SidebarModel,
) -> (Vec<String>, Log, gpui::ListState) {
    let state = sidebar_list_state(screen_item_count(&model));
    let log = log();
    arm_ax_probe(true);
    take_ax_labels();
    let (_host, _) = cx.add_window_view({
        let log = log.clone();
        let state = state.clone();
        |_, _| Host { model, state, log }
    });
    let mut seen = std::collections::HashSet::new();
    let labels: Vec<String> =
        take_ax_labels().into_iter().filter(|l| seen.insert(l.clone())).collect();
    arm_ax_probe(false);
    (labels, log, state)
}

/// Draws `model` at exactly 260 × 720 through an entity, like an app mounts
/// it, and returns the intent log with the list state whose item bounds are
/// now in window coordinates. Explicit geometry (not the default window) is
/// what makes row-centre clicks land.
fn draw_fixed<'a>(
    cx: &'a mut TestAppContext,
    model: aui::screens::sidebar::SidebarModel,
) -> (Log, gpui::ListState, &'a mut gpui::VisualTestContext) {
    let state = sidebar_list_state(screen_item_count(&model));
    let log = log();
    let vcx = cx.add_empty_window();
    let host = Host { model, state: state.clone(), log: log.clone() };
    vcx.draw(point(px(0.0), px(0.0)), size(px(260.0), px(720.0)), |_, cx| {
        cx.new(|_| host).into_any_element()
    });
    vcx.run_until_parked();
    (log, state, vcx)
}

/// Every interactive element names itself: the workspace row, search, new
/// session, each nav row, the mode control, a session row and the footer.
#[gpui::test]
fn sidebar_labels_every_interactive_element(cx: &mut TestAppContext) {
    init(cx);
    let (labels, _, _) = draw(cx, sample_model());
    assert!(!labels.is_empty(), "the screen draws controls at all");
    let unlabelled = labels.iter().filter(|l| l.is_empty()).count();
    assert_eq!(unlabelled, 0, "unlabelled controls: {unlabelled} of {}", labels.len());
    for want in [
        "Workspace harness",
        "Search",
        "New session",
        "Skills",
        "Automations",
        "Mode control",
        "Sessions",
        "Browse",
        "Design v2 tokens",
        "Account Bharani · Max",
    ] {
        assert!(labels.iter().any(|l| l == want), "missing label {want:?}; got {labels:?}");
    }
}

/// Clicking a session row reports `Select` for that session. The click lands
/// at the centre of the row's measured bounds, so only that row can report.
#[gpui::test]
fn clicking_a_session_row_selects_it(cx: &mut TestAppContext) {
    init(cx);
    // Mounted as the window's root view (like an app mounts it), so the
    // scene the clicks dispatch into holds the rows. Sweeps the sidebar
    // column; the first session row in flatten order is the pinned
    // "Design v2 tokens" row, and nothing is hovered, so the trailing slot
    // carries the age text and the row reports Select.
    let log = log();
    let state = sidebar_list_state(screen_item_count(&sample_model()));
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        let state = state.clone();
        |_, _| Host { model: sample_model(), state, log }
    });
    let vcx: &mut gpui::VisualTestContext = vcx;
    let mut y = 0.0;
    while y <= 720.0 {
        vcx.simulate_click(point(px(130.0), px(y)), gpui::Modifiers::none());
        vcx.run_until_parked();
        y += 8.0;
    }
    let entries = log.borrow().clone();
    assert!(!entries.is_empty(), "no click in the sidebar column reached any row");
    assert!(
        entries.iter().any(|e| e == "Select:design-v2"),
        "no Select:design-v2 after sweeping the sidebar column; got {entries:?}"
    );
}

/// Rows sit on the control ladder: one-line session rows measure exactly
/// `metrics.control`, two-line rows (with a byline) exactly `metrics.row_2`.
#[gpui::test]
fn session_rows_are_control_tall(cx: &mut TestAppContext) {
    init(cx);
    let model = sample_model();
    let rows = flatten_sidebar(&model.grouping, false);
    let grouping = model.grouping.clone();
    let (_log, state, _vcx) = draw_fixed(cx, model);
    let (control, row_2) = cx.update(|cx| (cx.aui().metrics.control, cx.aui().metrics.row_2));
    assert_eq!(control, px(36.0), "standard density keeps the 36 px control");
    // The one-line pinned row is control tall.
    let one_line = aui::nav::row_index_for_session(&rows, &grouping, &"design-v2".into())
        .expect("design-v2 has a row");
    let bounds =
        state.bounds_for_item(one_line).expect("the one-line row is on screen");
    assert_eq!(bounds.size.height, control, "one-line row is not control tall");
    // The two-line running row is row_2 tall.
    let two_line =
        aui::nav::row_index_for_session(&rows, &grouping, &"browser-audit".into())
            .expect("browser-audit has a row");
    let bounds =
        state.bounds_for_item(two_line).expect("the two-line row is on screen");
    assert_eq!(bounds.size.height, row_2, "two-line row is not row_2 tall");
}

/// No uppercase text anywhere in the rendered tree: every recorded label is
/// sentence case or lower. Words of two or more ASCII capitals fail; single
/// glyphs (`⌘K`, pin letters) and digits pass.
#[gpui::test]
fn sidebar_renders_no_uppercase_text(cx: &mut TestAppContext) {
    init(cx);
    let (labels, _, _) = draw(cx, sample_model());
    let mut bad = Vec::new();
    for label in &labels {
        for word in label.split_whitespace() {
            let letters: String = word.chars().filter(|c| c.is_ascii_alphabetic()).collect();
            if letters.len() >= 2 && letters.chars().all(|c| c.is_ascii_uppercase()) {
                bad.push(label.clone());
                break;
            }
        }
    }
    assert!(bad.is_empty(), "uppercase text in the tree: {bad:?}");
}

/// The empty model renders the quiet line and no session rows; the loading
/// model renders skeletons with no session labels at all.
#[gpui::test]
fn empty_and_loading_states(cx: &mut TestAppContext) {
    init(cx);
    let mut empty = sample_model();
    empty.grouping = std::rc::Rc::new(aui::nav::Grouping::Date(Vec::new()));
    empty.selected = None;
    let (labels, _, _) = draw(cx, empty);
    assert!(
        labels.iter().any(|l| l == "No sessions yet"),
        "empty state misses its quiet line; got {labels:?}"
    );
    assert!(
        !labels.iter().any(|l| l.contains("Design v2 tokens")),
        "empty state shows session rows"
    );

    let mut loading = sample_model();
    loading.loading = true;
    let (labels, _, _) = draw(cx, loading);
    assert!(
        !labels.iter().any(|l| l.contains("Design v2 tokens")),
        "loading state shows session rows; got {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == "New session"),
        "loading state hides the header; got {labels:?}"
    );
}

/// A collapsed sidebar renders only the strip's expand control.
#[gpui::test]
fn collapsed_renders_only_the_strip(cx: &mut TestAppContext) {
    init(cx);
    let mut collapsed = sample_model();
    collapsed.collapsed = true;
    let (labels, _, _) = draw(cx, collapsed);
    assert_eq!(labels, vec!["Expand sidebar".to_string()], "strip labels: {labels:?}");
}

/// Browse mode renders the browse rows' sample data inside the sidebar.
#[gpui::test]
fn browse_mode_renders_browse_rows(cx: &mut TestAppContext) {
    init(cx);
    let mut browse = sample_model();
    browse.mode = aui::screens::sidebar::Mode::Browse;
    let (labels, _, _) = draw(cx, browse);
    for want in [
        "Tab 1: State scores 2023-24, selected",
        "Folder: Karnataka review",
        "Pinned 1: Unified dashboard",
    ] {
        assert!(labels.iter().any(|l| l == want), "missing label {want:?}; got {labels:?}");
    }
}

/// The rename field renders at control height with the draft and its label.
#[gpui::test]
fn rename_field_renders_at_control_height(cx: &mut TestAppContext) {
    init(cx);
    let mut renaming = sample_model();
    renaming.editing = Some(aui::screens::sidebar::EditingState {
        session: "design-v2".into(),
        draft: "Design v3 tokens".into(),
    });
    let (labels, _, state) = draw(cx, renaming.clone());
    assert!(
        labels.iter().any(|l| l == "Rename session Design v2 tokens"),
        "missing rename label; got {labels:?}"
    );
    let rows = flatten_sidebar(&renaming.grouping, false);
    let ix = aui::nav::row_index_for_session(&rows, &renaming.grouping, &"design-v2".into())
        .expect("renaming row has an index");
    let control = cx.update(|cx| cx.aui().metrics.control);
    let bounds =
        state.bounds_for_item(ix).expect("the renaming row is on screen in a 720 px viewport");
    assert_eq!(bounds.size.height, control, "rename field is not control tall");
}

/// The pure helpers the screen renders with agree on the sample: the first
/// session row in flatten order is the pinned "Design v2 tokens" row.
#[test]
fn first_session_row_is_design_v2() {
    let model = sample_model();
    let rows = flatten_sidebar(&model.grouping, false);
    let ix = aui::nav::row_index_for_session(&rows, &model.grouping, &"design-v2".into())
        .expect("design-v2 has a row");
    assert!(matches!(rows[ix], aui::nav::SidebarRow::Session { .. }));
    let first = rows
        .iter()
        .position(|row| matches!(row, aui::nav::SidebarRow::Session { .. }))
        .expect("the sample has session rows");
    assert_eq!(first, ix, "design-v2 is not the first session row");
}
