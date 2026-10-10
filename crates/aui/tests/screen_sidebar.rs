//! The v2 sidebar screen: labels, states, menus, hover actions, counts,
//! density, casing and one real click.
//!
//! Headless windows have no rasterizer, so the AX probe (every control
//! records its human label when built) is the observable surface, plus the
//! intent log for the click test.

use std::cell::RefCell;
use std::rc::Rc;

use aui::data::{arm_ax_probe, take_ax_labels};
use aui::nav::{flatten_sidebar, sidebar_list_state, DateGroup, Grouping, SessionSummary};
use aui::screens::sidebar::{
    list_row_height, sample_model, screen_item_count, sidebar_screen, EditingState, Mode, SidebarIntent, SidebarModel,
};
use aui::interact::FluidHandle;
use aui_tokens::{AgentState, AuiTheme, Density, ThemeKind};
use gpui::{div, point, prelude::*, px, IntoElement, Modifiers, TestAppContext, VisualTestContext, Window};

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
}

type Log = Rc<RefCell<Vec<SidebarIntent>>>;

/// A 260 x 900 host around the screen, the shape the gallery gives it.
struct Host {
    model: SidebarModel,
    state: gpui::ListState,
    fluid: FluidHandle,
    log: Log,
}

impl Host {
    fn new(model: SidebarModel, log: Log) -> Self {
        let state = sidebar_list_state(screen_item_count(&model));
        Self { model, state, fluid: FluidHandle::new(), log }
    }
}

impl gpui::Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let log = self.log.clone();
        div().w(px(260.0)).h(px(900.0)).child(
            sidebar_screen("sb", self.model.clone(), self.state.clone(), self.fluid.clone())
                .on_intent(move |intent, _, _| log.borrow_mut().push(intent)),
        )
    }
}

/// Draws `model` once and returns the AX labels the build recorded.
fn labels_of(cx: &mut TestAppContext, model: SidebarModel) -> Vec<String> {
    arm_ax_probe(true);
    take_ax_labels();
    let log: Log = Rc::default();
    cx.add_window_view(|_, _| Host::new(model, log));
    let labels = take_ax_labels();
    arm_ax_probe(false);
    labels
}

fn has(labels: &[String], want: &str) -> bool {
    labels.iter().any(|l| l == want)
}

fn assert_has(labels: &[String], wants: &[&str]) {
    for want in wants {
        assert!(has(labels, want), "missing AX label {want:?} in {labels:?}");
    }
}

#[gpui::test]
fn sessions_mode_labels_every_interactive_element(cx: &mut TestAppContext) {
    init(cx);
    let labels = labels_of(cx, sample_model());
    assert_has(
        &labels,
        &[
            "Hide sidebar",
            "Sessions",
            "Browse",
            "Search",
            "New session",
            "Skills",
            "Automations",
            "Pinned",
            "Open session Design v2 tokens",
            "Project harness",
            "Account Bharani · Max",
        ],
    );
    assert!(labels.iter().all(|l| !l.is_empty()), "unlabelled control: {labels:?}");
}

#[gpui::test]
fn browse_mode_labels_every_interactive_element(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.mode = Mode::Browse;
    let labels = labels_of(cx, model);
    assert_has(
        &labels,
        &[
            "Address",
            "Back",
            "Forward",
            "Reload",
            "New tab",
            "Ask Claude about this page",
            "Page menu",
            "Unified dashboard",
            "Tab State scores 2023-24",
            "Close tab State scores 2023-24",
            "Folder Karnataka review",
            "Space Personal",
            "New space",
        ],
    );
    assert!(labels.iter().all(|l| !l.is_empty()), "unlabelled control: {labels:?}");
}

#[gpui::test]
fn loading_state_shows_no_session_or_project_text(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.loading = true;
    let labels = labels_of(cx, model);
    assert!(!labels.iter().any(|l| l.starts_with("Open session")), "{labels:?}");
    assert!(!labels.iter().any(|l| l.starts_with("Project ")), "{labels:?}");
    assert_has(&labels, &["Search", "New session"]);
}

#[gpui::test]
fn empty_state_says_no_projects_yet(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.grouping = Rc::new(Grouping::Project(vec![]));
    model.pinned.clear();
    let labels = labels_of(cx, model);
    assert_has(&labels, &["No projects yet"]);
    assert!(!labels.iter().any(|l| l.starts_with("Open session")), "{labels:?}");
}

#[gpui::test]
fn collapsed_state_is_only_the_show_sidebar_strip(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.collapsed = true;
    let labels = labels_of(cx, model);
    assert!(has(&labels, "Show sidebar"), "{labels:?}");
    assert!(!has(&labels, "Search"), "{labels:?}");
    // `interactive` may record its label more than once; the strip must
    // carry no other label.
    assert!(labels.iter().all(|l| l == "Show sidebar"), "{labels:?}");
}

#[gpui::test]
fn view_options_menu_opens_with_its_rows(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.view_menu_open = true;
    let labels = labels_of(cx, model);
    assert_has(&labels, &["View options", "Group by"]);
}

#[gpui::test]
fn account_menu_opens_with_its_rows(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.account.menu_open = true;
    let labels = labels_of(cx, model);
    assert_has(&labels, &["Account menu for Bharani", "Settings…", "Providers…", "Sign out of Muse"]);
}

#[gpui::test]
fn rename_replaces_the_open_label_with_a_rename_label(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.editing = Some(EditingState { session: "token-ladder".into(), draft: "Token ladder audit".into() });
    let labels = labels_of(cx, model);
    assert_has(&labels, &["Rename session Token ladder audit"]);
    assert!(!has(&labels, "Open session Token ladder audit"), "{labels:?}");
}

#[gpui::test]
fn hovered_session_shows_pin_rename_archive(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.hovered = Some("checklist".into());
    let labels = labels_of(cx, model);
    assert_has(&labels, &["Pin session Checklist run", "Rename session Checklist run", "Archive session Checklist run"]);
}

#[gpui::test]
fn hovered_projects_header_shows_add_and_view_options(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.hovered_projects_header = true;
    let labels = labels_of(cx, model);
    assert_has(&labels, &["Add project", "View options"]);
}

#[gpui::test]
fn hovered_project_head_shows_new_session_and_options(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.hovered_group = Some("harness".into());
    let labels = labels_of(cx, model);
    assert_has(&labels, &["New session in harness", "harness options"]);
}

#[test]
fn screen_item_count_follows_mode_and_loading() {
    let mut model = sample_model();
    let flat = flatten_sidebar(&model.grouping, false).len();
    assert!(flat > 0);
    assert_eq!(screen_item_count(&model), flat);
    model.loading = true;
    assert_eq!(screen_item_count(&model), 4);
    model.loading = false;
    model.mode = Mode::Browse;
    assert_eq!(screen_item_count(&model), 0);
}

#[gpui::test]
fn list_row_height_is_32_standard_28_compact(cx: &mut TestAppContext) {
    init(cx);
    assert_eq!(cx.update(|cx| list_row_height(cx)), px(32.0));
    cx.update(|cx| AuiTheme::set_density(Density::Compact, None, cx));
    assert_eq!(cx.update(|cx| list_row_height(cx)), px(28.0));
}

/// All-caps text longer than one character, other than known acronyms.
fn shouty(text: &str) -> bool {
    const ACRONYMS: [&str; 3] = ["PGI", "URL", "UDISE"];
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) && !ACRONYMS.contains(&text.trim())
}

#[gpui::test]
fn no_visible_or_labelled_text_is_all_caps(cx: &mut TestAppContext) {
    init(cx);
    assert!(shouty("NEW SESSION") && !shouty("New session") && !shouty("PGI") && !shouty("A"), "the detector itself");
    let model = sample_model();
    let mut texts: Vec<String> = ["Pinned", "Projects", "Today", "Search", "New session", "New tab"].map(String::from).to_vec();
    for item in &model.nav {
        texts.push(item.label.to_string());
    }
    let mut sessions: Vec<&aui::nav::SessionSummary> = model.pinned.iter().collect();
    if let Grouping::Project(groups) = model.grouping.as_ref() {
        for group in groups {
            texts.push(group.name.to_string());
            sessions.extend(group.sessions.iter());
        }
    }
    for session in sessions {
        texts.push(session.name.to_string());
        if let Some(status) = session.status.as_ref() {
            texts.push(status.text().to_string());
        }
    }
    let mut browse = sample_model();
    browse.mode = Mode::Browse;
    for model in [sample_model(), browse] {
        texts.extend(labels_of(cx, model));
    }
    let shouting: Vec<&String> = texts.iter().filter(|t| shouty(t)).collect();
    assert!(shouting.is_empty(), "all-caps text: {shouting:?}");
}

/// One real click through the window: move onto the New session row
/// (y 84-116 under the 52 px title bar and the 32 px search row), click, and
/// read the intent the screen reported.
#[gpui::test]
fn clicking_new_session_reports_new_session(cx: &mut TestAppContext) {
    init(cx);
    let log: Log = Rc::default();
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        |_, _| Host::new(sample_model(), log)
    });
    let vcx: &mut VisualTestContext = vcx;
    let at = point(px(100.0), px(100.0));
    vcx.simulate_mouse_move(at, None, Modifiers::none());
    vcx.simulate_click(at, Modifiers::none());
    vcx.run_until_parked();
    let got = log.borrow().clone();
    assert!(got.contains(&SidebarIntent::NewSession), "intents: {got:?}");
}

/// A host with no plan to show (Baaz offline) labels the footer by name
/// alone — no dangling "·".
#[gpui::test]
fn account_footer_drops_an_empty_plan(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.account.plan = "".into();
    let labels = labels_of(cx, model);
    assert_has(&labels, &["Account Bharani"]);
    assert!(!labels.iter().any(|l| l.starts_with("Account Bharani ·")), "{labels:?}");
}

fn date_model(groups: Vec<DateGroup>) -> SidebarModel {
    let mut model = sample_model();
    model.grouping = Rc::new(Grouping::Date(groups));
    model
}

fn two_buckets() -> Vec<DateGroup> {
    vec![
        DateGroup::new("Today", vec![SessionSummary::new("a", "Alpha task", AgentState::Idle, "1h")]),
        DateGroup::new(
            "Yesterday",
            vec![
                SessionSummary::new("b", "Beta task", AgentState::Done, "1d"),
                SessionSummary::new("c", "Gamma task", AgentState::Idle, "1d"),
            ],
        ),
    ]
}

#[gpui::test]
fn date_grouping_shows_bucket_labels_session_rows_and_sessions_header(cx: &mut TestAppContext) {
    init(cx);
    let labels = labels_of(cx, date_model(two_buckets()));
    // Date bucket headers are plain section text with no AX label, so "Today" /
    // "Yesterday" are not observable here; the rows under them are.
    assert_has(&labels, &["Open session Alpha task", "Open session Beta task", "Open session Gamma task", "Sessions"]);
}

#[gpui::test]
fn empty_date_grouping_says_no_sessions_yet(cx: &mut TestAppContext) {
    init(cx);
    let mut model = date_model(vec![]);
    model.loading = false;
    let labels = labels_of(cx, model);
    let text_ok = has(&labels, "No sessions yet");
    assert!(text_ok, "missing 'No sessions yet' in {labels:?}");
    assert!(!has(&labels, "No projects yet"), "{labels:?}");
}

#[test]
fn screen_item_count_counts_a_date_groupings_flattened_rows() {
    let model = date_model(two_buckets());
    let flat = flatten_sidebar(&model.grouping, false).len();
    assert!(flat >= 3, "headers + rows: {flat}");
    assert_eq!(screen_item_count(&model), flat);
}

/// The email equal to the name is hidden in the account menu, but the menu
/// records no AX label for the email line, so it is not observable here.
/// Instead pin what is: the footer label never carries the email.
#[gpui::test]
fn account_footer_label_with_email_equal_to_name_is_unchanged(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.account.email = model.account.name.clone();
    let labels = labels_of(cx, model);
    assert_has(&labels, &["Account Bharani · Max"]);
    assert!(!labels.iter().any(|l| l.contains("Bharani Bharani")), "{labels:?}");
}

/// Moving the pointer over a session row reports `Hover(Some(id), Some(bounds))`
/// with real (non-zero) bounds. Scans down the list since row y positions
/// depend on the chrome above.
#[gpui::test]
fn hovering_a_session_row_reports_id_and_bounds(cx: &mut TestAppContext) {
    init(cx);
    let log: Log = Rc::default();
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        let model = date_model(two_buckets());
        |_, _| Host::new(model, log)
    });
    let vcx: &mut VisualTestContext = vcx;
    let mut y = 100.0;
    while y < 500.0 {
        vcx.simulate_mouse_move(point(px(100.0), px(y)), None, Modifiers::none());
        vcx.run_until_parked();
        y += 6.0;
    }
    let got = log.borrow().clone();
    let hit = got.iter().find_map(|i| match i {
        SidebarIntent::Hover(Some(id), Some(b)) if id.as_ref() == "a" => Some(*b),
        _ => None,
    });
    let b = hit.unwrap_or_else(|| panic!("no Hover(Some(a), Some(bounds)) in {got:?}"));
    assert!(b.size.width > px(0.0) && b.size.height > px(0.0), "{b:?}");
}

#[gpui::test]
fn browse_rows_carry_their_accessible_names(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.mode = Mode::Browse;
    let labels = labels_of(cx, model);
    assert_has(
        &labels,
        &[
            "Address",
            "New tab",
            "Spaces",
            "Tab State scores 2023-24",
            "Folder Karnataka review",
            "Task folder Checklist run, open its chat",
            "Space Personal",
        ],
    );
    assert!(labels.iter().all(|l| !l.is_empty()), "unlabelled control: {labels:?}");
}

#[gpui::test]
fn sessions_mode_still_labels_search_and_new_session(cx: &mut TestAppContext) {
    init(cx);
    let labels = labels_of(cx, sample_model());
    assert_has(&labels, &["Search", "New session"]);
}

#[gpui::test]
fn view_options_menu_wording(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.view_menu_open = true;
    model.view_options = aui::screens::sidebar::ViewOptions {
        hidden_count: 3,
        show_hidden: false,
        pr_status_available: false,
        ..Default::default()
    };
    let labels = labels_of(cx, model);
    assert_has(
        &labels,
        &["Show hidden", "Show PR status", "Search all projects", "Clear empty sessions", "Group by", "Sort by"],
    );
}

#[gpui::test]
fn account_menu_wording(cx: &mut TestAppContext) {
    init(cx);
    let mut model = sample_model();
    model.account.menu_open = true;
    let name = model.account.name.to_string();
    let labels = labels_of(cx, model);
    let header = format!("Account menu for {name}");
    assert_has(&labels, &["Settings…", "Providers…", "Sign out of Muse", header.as_str()]);
}
