//! Card 23: the three groupings of the same sessions — status, project
//! (with nested children) and date.
//!
//! Every grouping renders the same row (`.sr`, [`crate::nav::compact_session_row`]);
//! only the headers above the rows change. The panel body starts at the caps
//! group row that carries the sliders icon (the view-options menu) and ends
//! with the last row; the primary nav above it belongs to the sidebar, not to
//! the view.

use std::rc::Rc;

use aui_icons::{icon, FileType, IconName};
use aui_motion::{collapse, tint_fade, Tween};
use aui_tokens::{scale, scaled, ActiveAui, AuiStyled, TextRole};
use gpui::{
    div, font, prelude::*, px, radians, AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, SharedString, Style, TextRun, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, status_dot, tag, ButtonSize};
use crate::nav::{compact_session_row, group_header, group_row, project_mark, RowAction, SessionSummary};
use crate::util::{interaction_flags, TrackInteraction};

/// `.pj{gap:8px;height:30px;padding:0 10px;margin:4px 8px 0;font-weight:600;font-size:12.5px}`.
/// The height is the shared row metric.
const PJ_GAP: f32 = 8.0;
const PJ_PAD_X: f32 = 10.0;
const PJ_MARGIN_X: f32 = 8.0;
const PJ_MARGIN_TOP: f32 = 4.0;
/// Sessions under a project group indent one step so the hierarchy reads;
/// status and date groupings keep their rows flush.
const PJ_CHILD_INDENT: f32 = 16.0;
/// The "Show N more" row after a folded project's sessions: 28 px, FS_12
/// ink-3, chevron-down/up before the text, the row's own hover ground.
const FOLD_H: f32 = 28.0;
/// `.fold .chev{width:12px}`.
const FOLD_CHEVRON: f32 = 12.0;
const PJ_TEXT: f32 = 12.5;
/// `.pj .chev{width:11px;height:11px}` — one pixel smaller than the shared
/// `.chev`, so this row rotates its own glyph.
const PJ_CHEVRON: f32 = 11.0;
/// `.fic{width:14px;height:14px}` — the folder mark on a project row.
const FIC: f32 = 14.0;
/// The project mark on a group row: 18 px, as the sidebar header draws it.
const GROUP_MARK: f32 = 18.0;
/// The trailing branch: mono 11 ink-3, truncating.
const TRAIL_TEXT: f32 = scale::FS_11;
const TRAIL_MAX: f32 = 120.0;
/// The project name never shrinks below this while the branch still has room:
/// the branch yields first and collapses toward zero, and only once it is
/// gone does the name truncate.
const NAME_MIN: f32 = 96.0;
/// The branch yields all of its room before the name gives any: this dwarfs
/// the name's default shrink, so narrowing collapses the branch toward zero
/// first and the name only shrinks (down to [`NAME_MIN`], then truncating)
/// once the branch is gone.
const BRANCH_SHRINK_FIRST: f32 = 1000.0;
/// Below this width the branch drops out entirely instead of rendering a
/// sliver: nothing readable fits, so the name keeps the room. Short branches
/// that naturally fit under this still render — only a squeezed branch, one
/// whose natural width no longer fits its room, is dropped.
const BRANCH_DROP_BELOW: f32 = 40.0;
/// The hover tray: the same right/top inset the session row's tray uses, so
/// the two trays sit in the same place.
const TRAY_RIGHT: f32 = 8.0;
const TRAY_TOP: f32 = 6.0;
const TRAY_GAP: f32 = 2.0;
const TRAY_PAD: f32 = 1.0;
const TRAY_GLYPH: f32 = 12.0;
/// `.dg{gap:8px;height:26px;padding:0 12px;margin-top:8px;font-size:11px;font-weight:600;text-transform:uppercase}`.
const DG_GAP: f32 = 8.0;
const DG_H: f32 = 26.0;
const DG_PAD_X: f32 = 12.0;
/// `.dg .rule{flex:1;height:1px;background:var(--line)}`.
const DG_RULE: f32 = 1.0;

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type PlainHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type RowActionHandler = Rc<dyn Fn(&SharedString, RowAction, &mut Window, &mut App)>;
type GroupActionHandler = Rc<dyn Fn(&SharedString, GroupAction, &mut Window, &mut App)>;
type GroupRowActionHandler = Rc<dyn Fn(GroupAction, &mut Window, &mut App)>;

/// A status group: `Needs you 1`, `Running 3`, `Done 2`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusGroup {
    /// Stable id, reported by the toggle handler.
    pub id: SharedString,
    /// The 12 px / 600 label.
    pub label: SharedString,
    /// The mono count after the label.
    pub count: SharedString,
    /// Open: the chevron points down and the rows are visible.
    pub open: bool,
    /// The rows under the header.
    pub sessions: Vec<SessionSummary>,
}

impl StatusGroup {
    /// An open group with `sessions` under it.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        count: impl Into<SharedString>,
        sessions: Vec<SessionSummary>,
    ) -> Self {
        Self { id: id.into(), label: label.into(), count: count.into(), open: true, sessions }
    }

    /// Closes the group.
    pub fn closed(mut self) -> Self {
        self.open = false;
        self
    }
}

/// What a project group row's hover tray asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupAction {
    /// The `Plus` button: start a session in this project.
    New,
    /// The `Dots` button: open this project's menu.
    Menu,
    /// The "Show N more" / "Show less" row: show the rows the caller held
    /// back, or hide them again.
    ToggleMore,
}

/// A project group: the `.pj` row and the sessions (and their children) below.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectGroup {
    /// Stable id, reported by the toggle handler.
    pub id: SharedString,
    /// The project name.
    pub name: SharedString,
    /// The mono count at the right.
    pub count: SharedString,
    /// Open: folder-open icon, chevron rotated, rows visible.
    pub open: bool,
    /// Muted (`Archived`, `Other workspaces`): ink-3 at weight 500, and the
    /// folder glyph stays even when a mark is set.
    pub muted: bool,
    /// The project's mark: its initial and label colour, drawn in place of
    /// the folder glyph. A group with no mark (muted ones) keeps the folder.
    pub mark: Option<(SharedString, gpui::Hsla)>,
    /// The trailing mono text before the count (the branch).
    pub trailing: Option<SharedString>,
    /// The rolled-up agent state: a status dot after the name.
    pub state: Option<aui_tokens::AgentState>,
    /// The rows under the project row.
    pub sessions: Vec<SessionSummary>,
    /// A folded group: `(held_back, expanded)`. When `held_back > 0` a
    /// "Show N more" / "Show less" row follows the sessions; the library
    /// never decides how many rows to show — the caller passes the rows it
    /// wants visible and the count it held back.
    pub fold: Option<(usize, bool)>,
}

impl ProjectGroup {
    /// A closed, unmuted project.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, count: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            count: count.into(),
            open: false,
            muted: false,
            mark: None,
            trailing: None,
            state: None,
            sessions: Vec::new(),
            fold: None,
        }
    }

    /// Opens the project and gives it its sessions.
    pub fn open(mut self, sessions: Vec<SessionSummary>) -> Self {
        self.open = true;
        self.sessions = sessions;
        self
    }

    /// Mutes the row (`Archived`, `Other workspaces`).
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// Draws the project's mark in place of the folder glyph.
    pub fn mark(mut self, initial: impl Into<SharedString>, colour: gpui::Hsla) -> Self {
        self.mark = Some((initial.into(), colour));
        self
    }

    /// Sets the trailing mono text before the count (the branch).
    pub fn trailing(mut self, text: impl Into<SharedString>) -> Self {
        self.trailing = Some(text.into());
        self
    }

    /// Sets the rolled-up agent state: a status dot after the name, pulsing
    /// while a session runs.
    pub fn state(mut self, state: aui_tokens::AgentState) -> Self {
        self.state = Some(state);
        self
    }

    /// Folds a long group: `hidden` rows are held back and `expanded`
    /// picks the row's label ("Show {hidden} more" / "Show less").
    /// The caller passes the rows it wants visible in [`Self::open`] and
    /// the count it held back here; clicking the row reports
    /// [`GroupAction::ToggleMore`].
    pub fn folded(mut self, hidden: usize, expanded: bool) -> Self {
        self.fold = Some((hidden, expanded));
        self
    }
}

/// A date group: the `.dg` caps header with its hairline rule, and the flat
/// rows under it.
#[derive(Debug, Clone, PartialEq)]
pub struct DateGroup {
    /// `Today`, `Yesterday`, `This week`.
    pub label: SharedString,
    /// The rows under the header.
    pub sessions: Vec<SessionSummary>,
}

impl DateGroup {
    /// A date group.
    pub fn new(label: impl Into<SharedString>, sessions: Vec<SessionSummary>) -> Self {
        Self { label: label.into(), sessions }
    }
}

/// How a [`SidebarView`] groups its sessions. The variant carries the groups,
/// because each grouping has its own header shape.
#[derive(Debug, Clone, PartialEq)]
pub enum Grouping {
    /// Collapsible status groups with a chevron header.
    Status(Vec<StatusGroup>),
    /// Project rows with nested sessions and child sessions.
    Project(Vec<ProjectGroup>),
    /// Flat rows under caps date headers.
    Date(Vec<DateGroup>),
}

/// The body of a sidebar panel in one grouping. Build with [`sidebar_view`].
#[derive(IntoElement)]
pub struct SidebarView {
    id: ElementId,
    grouping: Rc<Grouping>,
    caption: Option<SharedString>,
    selected: Option<SharedString>,
    actions: Vec<RowAction>,
    editing: Option<(SharedString, std::cell::RefCell<Option<AnyElement>>)>,
    on_select: Option<SelectHandler>,
    on_toggle: Option<ToggleHandler>,
    on_view_options: Option<PlainHandler>,
    on_action: Option<RowActionHandler>,
    on_group_action: Option<GroupActionHandler>,
}

/// The sessions of a sidebar, grouped by `grouping`.
///
/// The grouping is held behind an [`Rc`] because a sidebar is rebuilt on every
/// frame while its sessions change only when the list does: an app that caches
/// the built [`Grouping`] hands the same `Rc` over and over and pays nothing
/// per frame. A bare `Grouping` still works — `impl Into<Rc<Grouping>>` moves
/// it into a fresh `Rc` — so a caller that builds one per frame is unchanged.
pub fn sidebar_view(id: impl Into<ElementId>, grouping: impl Into<Rc<Grouping>>) -> SidebarView {
    SidebarView {
        id: id.into(),
        grouping: grouping.into(),
        caption: None,
        selected: None,
        actions: Vec::new(),
        editing: None,
        on_select: None,
        on_toggle: None,
        on_view_options: None,
        on_action: None,
        on_group_action: None,
    }
}

impl SidebarView {
    /// The caps group row above the groups (`Workspaces`, `Projects`,
    /// `Recent`). It carries the sliders icon when [`Self::on_view_options`]
    /// is set.
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// The id of the selected session.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// A row was clicked; the argument is the session id.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// A group header or project row was clicked; the argument is the group id.
    pub fn on_toggle(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    /// The sliders icon on the caption row was clicked: open the view menu.
    pub fn on_view_options(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_view_options = Some(Rc::new(f));
        self
    }

    /// The hover actions every row carries; none by default.
    pub fn row_actions(mut self, actions: Vec<RowAction>) -> Self {
        self.actions = actions;
        self
    }

    /// One row is being renamed: draw `editor` in place of its name.
    ///
    /// The element is the caller's, and so is everything about it — the text,
    /// the focus, and what Enter and Escape mean.
    pub fn editing(mut self, session_id: impl Into<SharedString>, editor: impl IntoElement) -> Self {
        self.editing = Some((session_id.into(), std::cell::RefCell::new(Some(editor.into_any_element()))));
        self
    }

    /// A row's hover action was clicked.
    pub fn on_action(mut self, f: impl Fn(&SharedString, RowAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    /// A project group row's hover action was clicked; the arguments are the
    /// group id and what the tray button asked for.
    pub fn on_group_action(mut self, f: impl Fn(&SharedString, GroupAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_group_action = Some(Rc::new(f));
        self
    }
}

/// The rows of one group, ready to be revealed.
#[allow(clippy::too_many_arguments)]
fn rows<'a>(
    id: &ElementId,
    sessions: impl IntoIterator<Item = &'a SessionSummary>,
    selected: &Option<SharedString>,
    actions: &[RowAction],
    editing: &Option<(SharedString, std::cell::RefCell<Option<AnyElement>>)>,
    on_select: &Option<SelectHandler>,
    on_action: &Option<RowActionHandler>,
) -> AnyElement {
    let mut col = v_flex().w_full();
    for session in sessions {
        let row_id: ElementId = (id.clone(), session.id.clone()).into();
        let mut row = compact_session_row(row_id, session.clone())
            .selected(selected.as_ref() == Some(&session.id))
            .actions(actions.to_vec());
        // The editor is one element and elements are not `Clone`, so it goes to
        // whichever row claims it and the rest see none.
        if let Some((editing_id, slot)) = editing {
            if editing_id == &session.id {
                if let Some(editor) = slot.borrow_mut().take() {
                    row = row.editor(editor);
                }
            }
        }
        if let Some(h) = on_select.clone() {
            row = row.on_select(move |k, w, cx| h(k, w, cx));
        }
        if let Some(h) = on_action.clone() {
            row = row.on_action(move |k, a, w, cx| h(k, a, w, cx));
        }
        col = col.child(row);
    }
    col.into_any_element()
}

/// The "Show N more" / "Show less" row after a folded project's sessions:
/// 28 px, FS_12 ink-3, chevron-down (up once expanded) before the text, the
/// row's own hover ground. It carries the session rows' margins, so inside
/// the indented block it keeps their indent and their right edge. Clicking
/// it reports [`GroupAction::ToggleMore`] for `group_id`.
#[allow(clippy::too_many_arguments)]
fn fold_row(
    id: impl Into<ElementId>,
    group_id: &SharedString,
    hidden: usize,
    expanded: bool,
    on_group_action: &Option<GroupActionHandler>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let (state, flags) = interaction_flags(id.clone(), window, cx);
    let bg = tint_fade((id.clone(), "bg"), flags.hovered, p.surface_2, Tween::FAST, window, cx);
    let label: SharedString = if expanded { "Show less".into() } else { format!("Show {hidden} more").into() };
    let mut glyph = icon(IconName::ChevronDown).size(px(FOLD_CHEVRON)).color(p.ink_3);
    if expanded {
        glyph = glyph.rotate(radians(std::f32::consts::PI));
    }
    let mut row = h_flex()
        .id(id.clone())
        .relative()
        .flex_none()
        .min_w(px(0.0))
        .h(px(FOLD_H))
        .items_center()
        .gap(px(PJ_GAP))
        .px(px(PJ_PAD_X))
        .ml(px(PJ_MARGIN_X))
        .mr(px(PJ_MARGIN_X))
        .rounded(px(scale::R_SM))
        .bg(bg)
        .ui(scale::FS_12)
        .text_color(p.ink_3)
        .cursor_pointer()
        .track_interaction(&state)
        .child(glyph)
        .child(div().flex_1().min_w(px(0.0)).truncate().child(label));
    if let Some(h) = on_group_action.clone() {
        let group_id = group_id.clone();
        row = row.on_click(move |_, w, cx| h(&group_id, GroupAction::ToggleMore, w, cx));
    }
    row.into_any_element()
}

impl RenderOnce for SidebarView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let SidebarView { id, grouping, caption, selected, actions, editing, on_select, on_toggle, on_view_options, on_action, on_group_action } =
            self;
        let mut col = v_flex().w_full();

        if let Some(caption) = caption {
            let mut row = group_row((id.clone(), "caption"), caption);
            if let Some(h) = on_view_options {
                row = row.on_view_options(move |_, w, cx| h(w, cx));
            }
            col = col.child(row);
        }

        // The grouping is only read here, so every group, session and label is
        // borrowed out of the caller's `Rc`: the only clone a frame pays is the
        // per-row one [`compact_session_row`] takes by value.
        match &*grouping {
            Grouping::Status(groups) => {
                for group in groups {
                    let key: ElementId = (id.clone(), group.id.clone()).into();
                    let mut header = group_header((key.clone(), "header"), group.label.clone(), group.open).count(group.count.clone());
                    if let Some(h) = on_toggle.clone() {
                        let group_id = group.id.clone();
                        header = header.on_toggle(move |_, w, cx| h(&group_id, w, cx));
                    }
                    let body = rows(&key, &group.sessions, &selected, &actions, &editing, &on_select, &on_action);
                    let (reveal, _) = collapse((key, "body"), group.open, body, window, cx);
                    col = col.child(header).child(reveal);
                }
            }
            Grouping::Project(groups) => {
                for group in groups {
                    let key: ElementId = (id.clone(), group.id.clone()).into();
                    let mut row = project_group_row((key.clone(), "project"), group.name.clone(), group.count.clone(), group.open);
                    if group.muted {
                        row = row.muted();
                    }
                    if let Some((initial, colour)) = group.mark.clone() {
                        row = row.mark(initial, colour);
                    }
                    if let Some(trailing) = group.trailing.clone() {
                        row = row.trailing(trailing);
                    }
                    if let Some(state) = group.state {
                        row = row.state(state);
                    }
                    if let Some(h) = on_toggle.clone() {
                        let group_id = group.id.clone();
                        row = row.on_toggle(move |_, w, cx| h(&group_id, w, cx));
                    }
                    if let Some(h) = on_group_action.clone() {
                        let group_id = group.id.clone();
                        row = row.on_group_action(move |action, w, cx| h(&group_id, action, w, cx));
                    }
                    let body = rows(&key, &group.sessions, &selected, &actions, &editing, &on_select, &on_action);
                    // Project children indent one step; the fold row (if any)
                    // sits inside the same block so it keeps their indent and
                    // their right edge. Status and date bodies stay flush.
                    let mut inner = v_flex().w_full().child(body);
                    if let Some((hidden, expanded)) = group.fold {
                        if hidden > 0 {
                            inner = inner.child(fold_row(
                                (key.clone(), "more"),
                                &group.id,
                                hidden,
                                expanded,
                                &on_group_action,
                                window,
                                cx,
                            ));
                        }
                    }
                    let indented = v_flex().pl(px(PJ_CHILD_INDENT)).child(inner).into_any_element();
                    let (reveal, _) = collapse((key, "body"), group.open, indented, window, cx);
                    col = col.child(row).child(reveal);
                }
            }
            Grouping::Date(groups) => {
                // Pinned sessions lift out of the date buckets into a leading
                // group styled like the sidebar's `Pinned 3` (card 21).
                // The partition is by reference: only the pointers move into
                // the leading group, never the summaries themselves.
                let mut pinned: Vec<&SessionSummary> = Vec::new();
                let mut dated: Vec<(&SharedString, Vec<&SessionSummary>)> = Vec::with_capacity(groups.len());
                for group in groups {
                    let (is_pinned, rest): (Vec<_>, Vec<_>) = group.sessions.iter().partition(|s| s.pinned);
                    pinned.extend(is_pinned);
                    if !rest.is_empty() {
                        dated.push((&group.label, rest));
                    }
                }
                if !pinned.is_empty() {
                    let key: ElementId = (id.clone(), "pinned").into();
                    let count = SharedString::from(pinned.len().to_string());
                    col = col
                        .child(group_header((key.clone(), "header"), "Pinned", true).count(count))
                        .child(rows(&key, pinned.iter().copied(), &selected, &actions, &editing, &on_select, &on_action));
                }
                for (i, (label, sessions)) in dated.into_iter().enumerate() {
                    let key: ElementId = (id.clone(), SharedString::from(format!("date-{i}"))).into();
                    col = col
                        .child(date_group_header(label.clone()))
                        .child(rows(&key, sessions.iter().copied(), &selected, &actions, &editing, &on_select, &on_action));
                }
            }
        }
        col
    }
}

/// A project row (`.pj`). Build with [`project_group_row`].
#[derive(IntoElement)]
pub struct ProjectGroupRow {
    id: ElementId,
    name: SharedString,
    count: SharedString,
    open: bool,
    muted: bool,
    mark: Option<(SharedString, gpui::Hsla)>,
    trailing: Option<SharedString>,
    state: Option<aui_tokens::AgentState>,
    on_toggle: Option<crate::util::ClickHandler>,
    on_group_action: Option<GroupRowActionHandler>,
}

/// A project row: chevron, folder mark, name, count.
pub fn project_group_row(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    count: impl Into<SharedString>,
    open: bool,
) -> ProjectGroupRow {
    ProjectGroupRow {
        id: id.into(),
        name: name.into(),
        count: count.into(),
        open,
        muted: false,
        mark: None,
        trailing: None,
        state: None,
        on_toggle: None,
        on_group_action: None,
    }
}

impl ProjectGroupRow {
    /// `.pj{color:var(--ink-3);font-weight:500}` — the archived project.
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// Draws the project's mark in place of the folder glyph. Muted groups
    /// keep the folder glyph.
    pub fn mark(mut self, initial: impl Into<SharedString>, colour: gpui::Hsla) -> Self {
        self.mark = Some((initial.into(), colour));
        self
    }

    /// Sets the trailing mono text before the count (the branch).
    pub fn trailing(mut self, text: impl Into<SharedString>) -> Self {
        self.trailing = Some(text.into());
        self
    }

    /// Sets the rolled-up agent state: a status dot after the name, pulsing
    /// while a session runs.
    pub fn state(mut self, state: aui_tokens::AgentState) -> Self {
        self.state = Some(state);
        self
    }

    /// Toggle click.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    /// A hover-tray button was clicked; the argument is what it asked for.
    pub fn on_group_action(mut self, f: impl Fn(GroupAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_group_action = Some(Rc::new(f));
        self
    }
}

/// The hover tray at a project row's right end: `Plus` (new session here)
/// and `Dots` (this project's menu), over the row's own surface-2 ground so
/// it covers the count the way the session row's tray covers its meta.
fn group_tray(
    id: &ElementId,
    visible: bool,
    on_group_action: &Option<GroupRowActionHandler>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Div {
    let p = cx.aui().colors;
    let opacity = aui_motion::tween((id.clone(), "tray-opacity"), if visible { 1.0f32 } else { 0.0 }, aui_motion::Tween::FAST, window, cx);
    let mut tray = h_flex()
        .absolute()
        .right(px(TRAY_RIGHT))
        .top(px(TRAY_TOP))
        .gap(px(TRAY_GAP))
        .p(px(TRAY_PAD))
        .rounded(px(scale::R_SM))
        .bg(p.surface_2)
        .opacity(opacity);
    for (glyph, action, name) in [(IconName::Plus, GroupAction::New, "new"), (IconName::Dots, GroupAction::Menu, "menu")] {
        let mut b = icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(TRAY_GLYPH));
        if let Some(handler) = on_group_action.clone() {
            // The tray sits inside the row, and gpui fires every `on_click`
            // up the tree in the bubble phase: without this the tray buttons
            // also toggled the group. The action is the whole click.
            b = b.on_click(move |_, w, cx| {
                cx.stop_propagation();
                handler(action, w, cx)
            });
        }
        tray = tray.child(b);
    }
    if !visible && opacity <= 0.001 {
        tray = tray.invisible();
    }
    tray
}

/// A trailing branch that drops out entirely when squeezed below
/// [`BRANCH_DROP_BELOW`]. The wrapper carries the flex (yield first, down to
/// zero); each frame re-derives the decision from live layout, so widening
/// brings the branch back with no state and no extra frames.
struct BranchDrop {
    text: SharedString,
    child: AnyElement,
}

/// A trailing branch that drops out entirely when squeezed below
/// [`BRANCH_DROP_BELOW`].
fn branch_drop(text: SharedString, child: AnyElement) -> BranchDrop {
    BranchDrop { text, child }
}

/// The branch's natural width in real pixels: the same face and size its div
/// renders, measured without a width cap.
fn branch_natural_width(text: &SharedString, window: &mut Window, cx: &mut App) -> Pixels {
    let run = TextRun {
        len: text.len(),
        font: font(scale::FONT_MONO),
        color: cx.aui().colors.ink_3,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window.text_system().layout_line(text, scaled(TRAIL_TEXT).to_pixels(window.rem_size()), &[run], None).width
}

impl IntoElement for BranchDrop {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for BranchDrop {
    type RequestLayoutState = LayoutId;
    type PrepaintState = bool;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let child_id = self.child.request_layout(window, cx);
        let mut style = Style { flex_shrink: BRANCH_SHRINK_FIRST, ..Style::default() };
        style.min_size.width = px(0.0).into();
        style.max_size.width = px(TRAIL_MAX).into();
        (window.request_layout(style, [child_id], cx), child_id)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let room = bounds.size.width;
        // Squeezed below the minimum while wanting more room: paint nothing.
        // A branch that naturally fits its room always paints.
        let show = room >= px(BRANCH_DROP_BELOW) || branch_natural_width(&self.text, window, cx) <= room;
        if show {
            self.child.prepaint(window, cx);
        }
        show
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        show: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if *show {
            self.child.paint(window, cx);
        }
    }
}

impl RenderOnce for ProjectGroupRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (hover_state, flags) = interaction_flags(id.clone(), window, cx);
        // `.pj .chev{width:11px}` rotates 0° → 90° on the swap spring.
        let chev = super::chevron_sized(id.clone(), self.open, p.ink_3, PJ_CHEVRON, window, cx);
        let folder = if self.open { FileType::FolderOpen } else { FileType::Folder };
        // Muted groups keep the folder glyph; the rest draw their mark.
        let leading: gpui::AnyElement = match &self.mark {
            Some((initial, colour)) if !self.muted => project_mark(initial.clone(), *colour).size(px(GROUP_MARK)).into_any_element(),
            _ => icon(folder.icon()).size(px(FIC)).color(p.ink_3).into_any_element(),
        };
        // No `w_full`: at full width the 8 px margins overflow the column and
        // the row ends 16 px past the session rows under it. Without a
        // width the row stretches to the column minus its margins (the
        // column's default align), so both gutters stay 8. It must not be
        // `flex_1` either: in the view's column that would grow it
        // vertically into all of the column's free space.
        let mut row = h_flex()
            .id(id.clone())
            .relative()
            .flex_none()
            .min_w(px(0.0))
            .h(cx.aui().metrics.row)
            .gap(px(PJ_GAP))
            .px(px(PJ_PAD_X))
            .ml(px(PJ_MARGIN_X))
            .mr(px(PJ_MARGIN_X))
            .mt(px(PJ_MARGIN_TOP))
            .ui(PJ_TEXT)
            .text_color(if self.muted { p.ink_3 } else { p.ink })
            .cursor_pointer()
            .track_interaction(&hover_state)
            .child(chev)
            .child(leading)
            .child(
                div()
                    .min_w(px(NAME_MIN))
                    .truncate()
                    .font_weight(if self.muted { gpui::FontWeight::MEDIUM } else { gpui::FontWeight::SEMIBOLD })
                    .child(self.name),
            );
        if let Some(state) = self.state {
            let pulse = state == aui_tokens::AgentState::Running;
            row = row.child(status_dot((id.clone(), "state"), state).pulse(pulse));
        }
        row = row.child(div().flex_1());
        if let Some(trailing) = self.trailing {
            // The branch yields room before the name does: it shrinks while
            // the name holds `NAME_MIN`, truncates inside whatever room is
            // left, and drops out entirely once squeezed below
            // `BRANCH_DROP_BELOW`.
            row = row.child(branch_drop(
                trailing.clone(),
                div()
                    .min_w(px(0.0))
                    .mono(TRAIL_TEXT)
                    .text_color(p.ink_3)
                    .max_w(px(TRAIL_MAX))
                    .truncate()
                    .child(trailing)
                    .into_any_element(),
            ));
        }
        row = row.child(tag(self.count));
        if self.on_group_action.is_some() {
            let tray_visible = flags.hovered;
            row = row.child(group_tray(&id, tray_visible, &self.on_group_action, window, cx));
        }
        if let Some(on_toggle) = self.on_toggle {
            row = row.on_click(move |e, w, cx| on_toggle(e, w, cx));
        }
        row
    }
}

/// A date header (`.dg`). Build with [`date_group_header`].
#[derive(IntoElement)]
pub struct DateGroupHeader {
    label: SharedString,
}

/// `TODAY`, `YESTERDAY`, `THIS WEEK` with the hairline rule after them.
pub fn date_group_header(label: impl Into<SharedString>) -> DateGroupHeader {
    DateGroupHeader { label: label.into() }
}

impl RenderOnce for DateGroupHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        h_flex()
            .w_full()
            .h(px(DG_H))
            .flex_none()
            .mt(px(scale::SP_3))
            .gap(px(DG_GAP))
            .px(px(DG_PAD_X))
            .text_role(TextRole::Caps)
            .text_color(p.ink_3)
            .child(div().flex_none().child(self.label.to_uppercase()))
            .child(div().flex_1().h(px(DG_RULE)).bg(p.line))
    }
}
