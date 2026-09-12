//! Card 23: the three groupings of the same sessions — status, project
//! (with nested children) and date.
//!
//! Every grouping renders the same row (`.sr`, [`crate::nav::compact_session_row`]);
//! only the headers above the rows change. The panel body starts at the caps
//! group row that carries the sliders icon (the view-options menu) and ends
//! with the last row; the primary nav above it belongs to the sidebar, not to
//! the view.

use std::rc::Rc;

use aui_icons::{icon, FileType};
use aui_motion::collapse;
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::tag;
use crate::nav::{compact_session_row, group_header, group_row, RowAction, SessionSummary};

/// `.pj{gap:8px;height:30px;padding:0 10px;margin:4px 8px 0;font-weight:600;font-size:12.5px}`.
/// The height is the shared row metric.
const PJ_GAP: f32 = 8.0;
const PJ_PAD_X: f32 = 10.0;
const PJ_MARGIN_X: f32 = 8.0;
const PJ_MARGIN_TOP: f32 = 4.0;
const PJ_TEXT: f32 = 12.5;
/// `.pj .chev{width:11px;height:11px}` — one pixel smaller than the shared
/// `.chev`, so this row rotates its own glyph.
const PJ_CHEVRON: f32 = 11.0;
/// `.fic{width:14px;height:14px}` — the folder mark on a project row.
const FIC: f32 = 14.0;
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
    /// Muted (`Archived`): ink-3 at weight 500.
    pub muted: bool,
    /// The rows under the project row.
    pub sessions: Vec<SessionSummary>,
}

impl ProjectGroup {
    /// A closed, unmuted project.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, count: impl Into<SharedString>) -> Self {
        Self { id: id.into(), name: name.into(), count: count.into(), open: false, muted: false, sessions: Vec::new() }
    }

    /// Opens the project and gives it its sessions.
    pub fn open(mut self, sessions: Vec<SessionSummary>) -> Self {
        self.open = true;
        self.sessions = sessions;
        self
    }

    /// Mutes the row (`Archived`).
    pub fn muted(mut self) -> Self {
        self.muted = true;
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

impl RenderOnce for SidebarView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let SidebarView { id, grouping, caption, selected, actions, editing, on_select, on_toggle, on_view_options, on_action } = self;
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
                    if let Some(h) = on_toggle.clone() {
                        let group_id = group.id.clone();
                        row = row.on_toggle(move |_, w, cx| h(&group_id, w, cx));
                    }
                    let body = rows(&key, &group.sessions, &selected, &actions, &editing, &on_select, &on_action);
                    let (reveal, _) = collapse((key, "body"), group.open, body, window, cx);
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
    on_toggle: Option<crate::util::ClickHandler>,
}

/// A project row: chevron, folder mark, name, count.
pub fn project_group_row(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    count: impl Into<SharedString>,
    open: bool,
) -> ProjectGroupRow {
    ProjectGroupRow { id: id.into(), name: name.into(), count: count.into(), open, muted: false, on_toggle: None }
}

impl ProjectGroupRow {
    /// `.pj{color:var(--ink-3);font-weight:500}` — the archived project.
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// Toggle click.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for ProjectGroupRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // `.pj .chev{width:11px}` rotates 0° → 90° on the swap spring.
        let chev = super::chevron_sized(id.clone(), self.open, p.ink_3, PJ_CHEVRON, window, cx);
        let folder = if self.open { FileType::FolderOpen } else { FileType::Folder };
        let mut row = h_flex()
            .id(id)
            .w_full()
            .h(cx.aui().metrics.row)
            .flex_none()
            .gap(px(PJ_GAP))
            .px(px(PJ_PAD_X))
            .mx(px(PJ_MARGIN_X))
            .mt(px(PJ_MARGIN_TOP))
            .ui(PJ_TEXT)
            .text_color(if self.muted { p.ink_3 } else { p.ink })
            .cursor_pointer()
            .child(chev)
            .child(icon(folder.icon()).size(px(FIC)).color(p.ink_3))
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .font_weight(if self.muted { gpui::FontWeight::MEDIUM } else { gpui::FontWeight::SEMIBOLD })
                    .child(self.name),
            )
            .child(div().flex_1())
            .child(tag(self.count));
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
