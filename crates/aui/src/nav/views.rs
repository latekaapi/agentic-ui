//! Card 23: the three groupings of the same sessions — status, project
//! (with nested children) and date.
//!
//! Every grouping renders the same row (`.sr`, [`crate::nav::compact_session_row`]);
//! only the headers above the rows change. The panel body starts at the caps
//! group row that carries the sliders icon (the view-options menu) and ends
//! with the last row; the primary nav above it belongs to the sidebar, not to
//! the view.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{collapse, tint_fade, Tween};
use aui_tokens::{scale, scaled, ActiveAui, AuiStyled, TextRole};
use gpui::{
    div, font, prelude::*, px, radians, AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, SharedString, Style, TextRun, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, tag, ButtonSize};
use crate::nav::{compact_session_row, group_header, group_row, project_mark, RowAction, SessionSummary};
use crate::nav::{LEADING_BOX, NAV_GUTTER, NAV_LABEL_X};
use crate::util::{interaction_flags, TrackInteraction};

/// `.pj{height:30px;margin:4px 8px 0}`. The height is the shared row metric;
/// the ground keeps its 8 px inset from the column edge while the content
/// follows the one gutter ([`NAV_GUTTER`], [`LEADING_BOX`], [`NAV_LABEL_X`]).
const PJ_MARGIN_X: f32 = 8.0;
const PJ_MARGIN_TOP: f32 = 4.0;
/// Right padding of a project row; the tray and the count live here.
const PJ_PAD_RIGHT: f32 = 10.0;
/// The label gap after the leading box: `NAV_LABEL_X - NAV_GUTTER -
/// LEADING_BOX`, so a chevron in the box puts the name at `NAV_LABEL_X`.
const PJ_LEAD_GAP: f32 = NAV_LABEL_X - NAV_GUTTER - LEADING_BOX;
/// The plain name's left pad (owner round 5): with no chevron or mark the
/// name starts at the leading centre (`NAV_GUTTER + LEADING_BOX / 2`), the
/// one vertical line the nav icons and the session dots sit on — not at the
/// column margin (where it sat left of the icons) and not at `NAV_LABEL_X`
/// (where the chevron flag puts it, right of them).
const PJ_PLAIN_PAD: f32 = NAV_GUTTER + LEADING_BOX / 2.0 - PJ_MARGIN_X;
/// The "Show N more" row after a folded project's sessions: 28 px, FS_12
/// ink-3, chevron-down/up before the text, the row's own hover ground.
const FOLD_H: f32 = 28.0;
/// `.fold .chev{width:12px}`.
const FOLD_CHEVRON: f32 = 12.0;
/// The plain group name: small muted label, not caps.
const PJ_TEXT: f32 = scale::FS_11;
/// `.pj .chev{width:11px;height:11px}` — one pixel smaller than the shared
/// `.chev`, so this row rotates its own glyph.
const PJ_CHEVRON: f32 = 11.0;
/// The project mark on a group row: 18 px, as the sidebar header draws it.
/// Drawn only when a caller passes one explicitly; the default row is plain.
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
/// The current project's mark: a 2 px accent bar down the row's left edge,
/// inside the row's own margin so it lines up with nothing else moving.
const CURRENT_BAR_W: f32 = 2.0;
/// How far the running state's left-edge bar dims at the pulse's end: full
/// at the ring's birth, this much gone as it fades.
const STATE_BAR_FADE: f32 = 0.55;
/// The bar stops short of the row's rounded corners at top and bottom.
const CURRENT_BAR_INSET: f32 = 4.0;
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

pub(crate) type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
pub(crate) type ToggleHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
pub(crate) type PlainHandler = Rc<dyn Fn(&mut Window, &mut App)>;
pub(crate) type RowActionHandler = Rc<dyn Fn(&SharedString, RowAction, &mut Window, &mut App)>;
pub(crate) type GroupActionHandler = Rc<dyn Fn(&SharedString, GroupAction, &mut Window, &mut App)>;
pub(crate) type GroupRowActionHandler = Rc<dyn Fn(GroupAction, &mut Window, &mut App)>;
/// A bounds intent: the library reports laid-out window bounds and stores
/// nothing; the app decides what to do with them. The first argument is the
/// row or group id the bounds belong to.
pub(crate) type BoundsHandler = Rc<dyn Fn(&SharedString, Bounds<Pixels>, &mut Window, &mut App)>;
/// Hover enter/leave with the session id: the hover card's arm. Fires from
/// the row's own hover events, so the caller learns about the pointer even
/// when nothing re-renders.
pub(crate) type RowHoverHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
/// Hover enter/leave with the session id and the row's own window bounds,
/// measured in prepaint: the hover card's arm and seat.
pub(crate) type RowHoverBoundsHandler =
    Rc<dyn Fn(&SharedString, bool, Bounds<Pixels>, &mut Window, &mut App)>;

/// Builds the inline rename editor fresh for one build of the renaming row.
///
/// A sidebar row can build more than once per frame — the virtualised list
/// re-runs its render pass on an in-prepaint autoscroll, and overdraw
/// measurement builds rows whose elements never paint — while an element
/// builds only once. The editor therefore arrives as a builder the row calls
/// on every build, never as one element the first build claims. Runs inside
/// layout, so it must only build: never notify or mutate UI state (the same
/// contract as the virtual view's `on_row_built` hook).
pub type EditorBuilder = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

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
    /// The project's mark: its initial and label colour, drawn in the
    /// leading box when set. Unset by default: the row is plain.
    pub mark: Option<(SharedString, gpui::Hsla)>,
    /// The trailing mono text before the count (the branch).
    pub trailing: Option<SharedString>,
    /// The rolled-up agent state: an accent bar at the row's left edge, in
    /// the state colour.
    pub state: Option<aui_tokens::AgentState>,
    /// Draw the collapse chevron in the leading box (off: the row is a plain
    /// label whose first glyph starts at the leading centre).
    pub chevron: bool,
    /// Draw the 2 px current bar for [`Self::current`]. `current` alone
    /// changes nothing visible.
    pub current_bar: bool,
    /// The rows under the project row.
    pub sessions: Vec<SessionSummary>,
    /// A folded group: `(held_back, expanded)`. When `held_back > 0` a
    /// "Show N more" / "Show less" row follows the sessions; the library
    /// never decides how many rows to show — the caller passes the rows it
    /// wants visible and the count it held back.
    pub fold: Option<(usize, bool)>,
    /// The project the open session belongs to: the name reads like every
    /// other project, and a 2 px accent bar sits at the row's left edge only
    /// with `current_bar`. One group at a time; the caller decides which.
    pub current: bool,
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
            chevron: false,
            current_bar: false,
            sessions: Vec::new(),
            fold: None,
            current: false,
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

    /// Draws the project's mark in the leading box. Unset by default: the
    /// row is plain.
    pub fn mark(mut self, initial: impl Into<SharedString>, colour: gpui::Hsla) -> Self {
        self.mark = Some((initial.into(), colour));
        self
    }

    /// Sets the trailing mono text before the count (the branch).
    pub fn trailing(mut self, text: impl Into<SharedString>) -> Self {
        self.trailing = Some(text.into());
        self
    }

    /// Sets the rolled-up agent state: an accent bar at the row's left edge
    /// in the state colour, breathing with the shared pulse while a session
    /// runs.
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

    /// Marks this as the project the open session belongs to. The name is
    /// unchanged; pair with [`Self::current_bar`] to mark it.
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    /// Draws the collapse chevron in the leading box; the label follows at
    /// [`NAV_LABEL_X`]. Off by default: the row is a plain label starting
    /// at the leading centre.
    pub fn chevron(mut self, chevron: bool) -> Self {
        self.chevron = chevron;
        self
    }

    /// Draws the 2 px bar with [`Self::current`]. Off by default:
    /// `current` alone changes nothing visible.
    pub fn current_bar(mut self, current_bar: bool) -> Self {
        self.current_bar = current_bar;
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
    editing: Option<(SharedString, EditorBuilder)>,
    pulse_phase: Option<f32>,
    on_select: Option<SelectHandler>,
    on_toggle: Option<ToggleHandler>,
    on_view_options: Option<PlainHandler>,
    on_action: Option<RowActionHandler>,
    on_group_action: Option<GroupActionHandler>,
    on_hover: Option<RowHoverHandler>,
    on_hover_bounds: Option<RowHoverBoundsHandler>,
    on_selected_prepainted: Option<BoundsHandler>,
    on_current_prepainted: Option<BoundsHandler>,
    on_group_menu_prepainted: Option<BoundsHandler>,
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
        pulse_phase: None,
        on_select: None,
        on_toggle: None,
        on_view_options: None,
        on_action: None,
        on_group_action: None,
        on_hover: None,
        on_hover_bounds: None,
        on_selected_prepainted: None,
        on_current_prepainted: None,
        on_group_menu_prepainted: None,
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

    /// One row is being renamed: draw the built editor in place of its name.
    ///
    /// `build` runs on every build of that row in a frame — there can be
    /// several (measure, then paint) — so each one draws the field. The
    /// editor itself is the caller's, and so is everything about it: the
    /// text, the focus, and what Enter and Escape mean.
    pub fn editing(mut self, session_id: impl Into<SharedString>, build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.editing = Some((session_id.into(), Rc::new(build)));
        self
    }

    /// Samples every pulsing dot in the list (session rows, project heads)
    /// at `phase` instead of mounting the looping animation: no frame is
    /// requested per render, so the list only moves when the caller
    /// re-renders it. Sample once per frame and pass the same phase to
    /// every row, so the frame agrees with itself. Unset: dots loop as
    /// before.
    pub fn pulse_phase(mut self, phase: f32) -> Self {
        self.pulse_phase = Some(phase);
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

    /// A session row's hover state changed; the arguments are the session id
    /// and whether the pointer entered. Fires from the row's own hover
    /// events, so the caller learns about the pointer even when nothing
    /// re-renders — what arms the hover detail's delay.
    pub fn on_row_hover(mut self, f: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    /// A session row's hover state changed, with the row's own window bounds
    /// measured in prepaint; the arguments are the session id, whether the
    /// pointer entered, and the row's rect. Supersedes [`Self::on_row_hover`]:
    /// when set, rows report through this alone — what arms the hover
    /// detail's delay and seats its card from the row instead of the pointer.
    pub fn on_row_hover_bounds(
        mut self,
        f: impl Fn(&SharedString, bool, Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hover_bounds = Some(Rc::new(f));
        self
    }

    /// Fires once per frame with the selected session row's bounds. The
    /// library stores nothing; the app decides what to do with the bounds
    /// (for example, seating a trigger menu at the row).
    pub fn on_selected_prepainted(
        mut self,
        f: impl Fn(&SharedString, Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selected_prepainted = Some(Rc::new(f));
        self
    }

    /// Fires once per frame with the `current` project group row's bounds.
    /// See [`Self::on_selected_prepainted`].
    pub fn on_current_prepainted(
        mut self,
        f: impl Fn(&SharedString, Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_current_prepainted = Some(Rc::new(f));
        self
    }

    /// Fires once per frame with every rendered project group row's tray `…`
    /// button bounds, keyed by group id — what a group-row menu seats at.
    /// See [`Self::on_selected_prepainted`].
    pub fn on_group_menu_prepainted(
        mut self,
        f: impl Fn(&SharedString, Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_group_menu_prepainted = Some(Rc::new(f));
        self
    }
}

/// One session row: the compact row with its editor slot, click and action
/// handlers, and the selected row's bounds-intent wrapper.
///
/// Shared by [`SidebarView`] (called once per session by [`rows`]) and the
/// virtualised sidebar (called once per visible session row by the list's
/// `render_item`), so the two paths cannot drift: same element ids, same
/// geometry, same intents.
#[allow(clippy::too_many_arguments)]
pub(crate) fn session_row_element(
    row_id: ElementId,
    session: &SessionSummary,
    selected: &Option<SharedString>,
    actions: &[RowAction],
    editing: &Option<(SharedString, EditorBuilder)>,
    on_select: &Option<SelectHandler>,
    on_action: &Option<RowActionHandler>,
    on_hover: &Option<RowHoverHandler>,
    on_hover_bounds: &Option<RowHoverBoundsHandler>,
    on_selected_prepainted: &Option<BoundsHandler>,
    pulse_phase: Option<f32>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let is_selected = selected.as_ref() == Some(&session.id);
    let mut row =
        compact_session_row(row_id, session.clone()).selected(is_selected).actions(actions.to_vec());
    if let Some(phase) = pulse_phase {
        row = row.pulse_phase(phase);
    }
    // The editor builds fresh on every build of the renaming row: rows can
    // build more than once per frame and an element builds only once, so a
    // shared element would go to whichever build ran first.
    if let Some((editing_id, build)) = editing {
        if editing_id == &session.id {
            row = row.editor(build(window, cx));
        }
    }
    if let Some(h) = on_select.clone() {
        row = row.on_select(move |k, w, cx| h(k, w, cx));
    }
    if let Some(h) = on_action.clone() {
        row = row.on_action(move |k, a, w, cx| h(k, a, w, cx));
    }
    // The bounds-carrying report supersedes the plain one: when set, the
    // row reports its own rect with the hover, which is what seats a hover
    // card from the row instead of the pointer.
    if let Some(h) = on_hover_bounds.clone() {
        row = row.on_hover_bounds(move |k, hovered, bounds, w, cx| h(k, hovered, bounds, w, cx));
    } else if let Some(h) = on_hover.clone() {
        row = row.on_hover(move |k, hovered, w, cx| h(k, hovered, w, cx));
    }
    // Only the selected row gets the wrapper, so the intent fires once
    // per frame with that one row's bounds and other rows lay out
    // exactly as before. The wrapper is a plain full-width column, so
    // the row stretches inside it the way it stretches in the group.
    if is_selected {
        if let Some(h) = on_selected_prepainted.clone() {
            let session_id = session.id.clone();
            return v_flex()
                .w_full()
                .on_children_prepainted(move |bounds, w, cx| {
                    if let Some(first) = bounds.first() {
                        h(&session_id, *first, w, cx);
                    }
                })
                .child(row)
                .into_any_element();
        }
    }
    row.into_any_element()
}

/// The rows of one group, ready to be revealed.
#[allow(clippy::too_many_arguments)]
fn rows<'a>(
    id: &ElementId,
    sessions: impl IntoIterator<Item = &'a SessionSummary>,
    selected: &Option<SharedString>,
    actions: &[RowAction],
    editing: &Option<(SharedString, EditorBuilder)>,
    on_select: &Option<SelectHandler>,
    on_action: &Option<RowActionHandler>,
    on_hover: &Option<RowHoverHandler>,
    on_hover_bounds: &Option<RowHoverBoundsHandler>,
    on_selected_prepainted: &Option<BoundsHandler>,
    pulse_phase: Option<f32>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let mut col = v_flex().w_full();
    for session in sessions {
        let row_id: ElementId = (id.clone(), session.id.clone()).into();
        col = col.child(session_row_element(
            row_id,
            session,
            selected,
            actions,
            editing,
            on_select,
            on_action,
            on_hover,
            on_hover_bounds,
            on_selected_prepainted,
            pulse_phase,
            window,
            cx,
        ));
    }
    col.into_any_element()
}

/// The "Show N more" / "Show less" row after a folded project's sessions:
/// 28 px, FS_12 ink-3, chevron-down (up once expanded) before the text, the
/// row's own hover ground. It carries the session rows' margins and gutter,
/// so its text starts at [`NAV_LABEL_X`] like every other label. Clicking
/// it reports [`GroupAction::ToggleMore`] for `group_id`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fold_row(
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
        .gap(px(PJ_LEAD_GAP))
        .pl(px(NAV_GUTTER))
        .pr(px(PJ_PAD_RIGHT))
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

/// One project group head: the project row with its toggle, tray actions,
/// group-menu bounds intent, and the `current` row's bounds-intent wrapper.
///
/// Shared by [`SidebarView`] and the virtualised sidebar, so the two paths
/// cannot drift: same element ids, same geometry, same intents.
#[allow(clippy::too_many_arguments)]
pub(crate) fn project_head_element(
    key: &ElementId,
    group: &ProjectGroup,
    on_toggle: &Option<ToggleHandler>,
    on_group_action: &Option<GroupActionHandler>,
    on_group_menu_prepainted: &Option<BoundsHandler>,
    on_current_prepainted: &Option<BoundsHandler>,
    pulse_phase: Option<f32>,
) -> AnyElement {
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
    if let Some(phase) = pulse_phase {
        row = row.pulse_phase(phase);
    }
    row = row.chevron(group.chevron).current_bar(group.current_bar);
    if group.current {
        row = row.current();
    }
    if let Some(h) = on_toggle.clone() {
        let group_id = group.id.clone();
        row = row.on_toggle(move |_, w, cx| h(&group_id, w, cx));
    }
    if let Some(h) = on_group_action.clone() {
        let group_id = group.id.clone();
        row = row.on_group_action(move |action, w, cx| h(&group_id, action, w, cx));
    }
    if let Some(h) = on_group_menu_prepainted.clone() {
        let group_id = group.id.clone();
        row = row.on_menu_prepainted(group_id, move |gid, bounds, w, cx| h(gid, bounds, w, cx));
    }
    // Only the current group row gets the wrapper, so the
    // intent fires once per frame with that one row's bounds.
    // The wrapper is a plain full-width column, so the row
    // stretches inside it the way it stretches in the view.
    let head: AnyElement = if group.current {
        if let Some(h) = on_current_prepainted.clone() {
            let group_id = group.id.clone();
            v_flex()
                .w_full()
                .on_children_prepainted(move |bounds, w, cx| {
                    if let Some(first) = bounds.first() {
                        h(&group_id, *first, w, cx);
                    }
                })
                .child(row)
                .into_any_element()
        } else {
            row.into_any_element()
        }
    } else {
        row.into_any_element()
    };
    head
}

impl RenderOnce for SidebarView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let SidebarView {
            id,
            grouping,
            caption,
            selected,
            actions,
            editing,
            pulse_phase,
            on_select,
            on_toggle,
            on_view_options,
            on_action,
            on_group_action,
            on_hover,
            on_hover_bounds,
            on_selected_prepainted,
            on_current_prepainted,
            on_group_menu_prepainted,
        } = self;
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
                    let body = rows(
                        &key,
                        &group.sessions,
                        &selected,
                        &actions,
                        &editing,
                        &on_select,
                        &on_action,
                        &on_hover,
                        &on_hover_bounds,
                        &on_selected_prepainted,
                        pulse_phase,
                        window,
                        cx,
                    );
                    let (reveal, _) = collapse((key, "body"), group.open, body, window, cx);
                    col = col.child(header).child(reveal);
                }
            }
            Grouping::Project(groups) => {
                for group in groups {
                    let key: ElementId = (id.clone(), group.id.clone()).into();
                    let head = project_head_element(
                        &key,
                        group,
                        &on_toggle,
                        &on_group_action,
                        &on_group_menu_prepainted,
                        &on_current_prepainted,
                        pulse_phase,
                    );
                    let body = rows(
                        &key,
                        &group.sessions,
                        &selected,
                        &actions,
                        &editing,
                        &on_select,
                        &on_action,
                        &on_hover,
                        &on_hover_bounds,
                        &on_selected_prepainted,
                        pulse_phase,
                        window,
                        cx,
                    );
                    // Sessions sit flush under their group: the dot lives in
                    // the leading box and the titles start at `NAV_LABEL_X`,
                    // so no indent block separates rows from their header.
                    // The fold row (if any) joins the same column.
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
                    // `w_full` is load-bearing: `collapse` lays its child out
                    // as a *root* (`Reveal::prepaint` → `layout_as_root`), and a
                    // root with `width: auto` is fit-content, not stretch. The
                    // status and date bodies get it for free because `rows`
                    // itself is `w_full`; this wrapper sits between them and the
                    // reveal, so without a width the whole block collapses to
                    // its content — the rows stop short of the project row at
                    // a wide sidebar and spill past its right margin at a
                    // narrow one. With it the block takes the column and the
                    // rows end where the project row ends at any width.
                    let (reveal, _) = collapse((key, "body"), group.open, inner.into_any_element(), window, cx);
                    col = col.child(head).child(reveal);
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
                        .child(rows(
                            &key,
                            pinned.iter().copied(),
                            &selected,
                            &actions,
                            &editing,
                            &on_select,
                            &on_action,
                            &on_hover,
                            &on_hover_bounds,
                            &on_selected_prepainted,
                            pulse_phase,
                            window,
                            cx,
                        ));
                }
                for (i, (label, sessions)) in dated.into_iter().enumerate() {
                    let key: ElementId = (id.clone(), SharedString::from(format!("date-{i}"))).into();
                    col = col
                        .child(date_group_header(label.clone()))
                        .child(rows(
                            &key,
                            sessions.iter().copied(),
                            &selected,
                            &actions,
                            &editing,
                            &on_select,
                            &on_action,
                            &on_hover,
                            &on_hover_bounds,
                            &on_selected_prepainted,
                            pulse_phase,
                            window,
                            cx,
                        ));
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
    pulse_phase: Option<f32>,
    current: bool,
    chevron: bool,
    current_bar: bool,
    menu_bounds: Option<(SharedString, BoundsHandler)>,
    on_toggle: Option<crate::util::ClickHandler>,
    on_group_action: Option<GroupRowActionHandler>,
}

/// A project row: the plain muted name and its count.
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
        pulse_phase: None,
        current: false,
        chevron: false,
        current_bar: false,
        menu_bounds: None,
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

    /// Draws the project's mark in the leading box. Nothing passes one by
    /// default — the row is plain — and muted groups never draw it.
    pub fn mark(mut self, initial: impl Into<SharedString>, colour: gpui::Hsla) -> Self {
        self.mark = Some((initial.into(), colour));
        self
    }

    /// Sets the trailing mono text before the count (the branch).
    pub fn trailing(mut self, text: impl Into<SharedString>) -> Self {
        self.trailing = Some(text.into());
        self
    }

    /// Sets the rolled-up agent state: an accent bar at the row's left edge
    /// in the state colour, breathing with the shared pulse while a session
    /// runs.
    pub fn state(mut self, state: aui_tokens::AgentState) -> Self {
        self.state = Some(state);
        self
    }

    /// Samples the state dot's pulse ring at `phase` instead of mounting
    /// the looping animation: no frame is requested, so the ring only moves
    /// when the caller re-renders (a view on its own timer). Unset: the dot
    /// loops as before.
    pub fn pulse_phase(mut self, phase: f32) -> Self {
        self.pulse_phase = Some(phase);
        self
    }

    /// The project the open session belongs to: the name reads like every
    /// other project. The 2 px accent bar draws only with
    /// [`Self::current_bar`].
    pub fn current(mut self) -> Self {
        self.current = true;
        self
    }

    /// Draws the collapse chevron in the leading box; the label follows at
    /// [`NAV_LABEL_X`]. Off by default: the row is a plain label whose first
    /// glyph starts at the leading centre.
    pub fn chevron(mut self, chevron: bool) -> Self {
        self.chevron = chevron;
        self
    }

    /// Draws the 2 px accent bar at the row's left edge with
    /// [`Self::current`]. Off by default: `current` alone changes nothing
    /// visible.
    pub fn current_bar(mut self, current_bar: bool) -> Self {
        self.current_bar = current_bar;
        self
    }

    /// Reports the tray `…` button's bounds, keyed by `group_id`, once per
    /// frame — what a group-row menu seats at. The library stores nothing.
    pub fn on_menu_prepainted(
        mut self,
        group_id: impl Into<SharedString>,
        f: impl Fn(&SharedString, Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.menu_bounds = Some((group_id.into(), Rc::new(f)));
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

/// What the branch and the count fade to while the project row's hover tray
/// is up. The tray covers this end of the row, so the two are complementary:
/// whatever the tray gains, they give up, on the one tween. Opacity only —
/// both keep their boxes, so nothing under the pointer reflows (D6).
fn under_tray(tray_opacity: f32) -> f32 {
    1.0 - tray_opacity
}

/// The hover tray at a project row's right end: `Plus` (new session here)
/// and `Dots` (this project's menu), over the row's own surface-2 ground so
/// it covers the count the way the session row's tray covers its meta.
#[allow(clippy::too_many_arguments)]
fn group_tray(
    id: &ElementId,
    visible: bool,
    opacity: f32,
    on_group_action: &Option<GroupRowActionHandler>,
    menu_bounds: &Option<(SharedString, BoundsHandler)>,
    _window: &mut Window,
    cx: &mut App,
) -> gpui::Div {
    let p = cx.aui().colors;
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
        // The `…` button seats the group-row menu: wrap it so its bounds are
        // reported once per frame. The wrapper sizes to the button, so the
        // tray's layout is unchanged.
        if action == GroupAction::Menu {
            if let Some((group_id, h)) = menu_bounds.clone() {
                tray = tray.child(
                    div()
                        .on_children_prepainted(move |bounds, w, cx| {
                            if let Some(first) = bounds.first() {
                                h(&group_id, *first, w, cx);
                            }
                        })
                        .child(b),
                );
                continue;
            }
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
        // Plain by default: the name is a small muted label whose first
        // glyph starts at the leading centre. With `chevron` (or an explicit
        // mark, which other consumers may pass), the box is taken and the
        // label follows at `NAV_LABEL_X`. Every project reads the same muted
        // treatment — `current` alone changes nothing (owner round 5); only
        // `current_bar` marks the row, without moving it.
        let leading_box = self.chevron || (self.mark.is_some() && !self.muted);
        let mut row = {
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
                .gap(px(PJ_LEAD_GAP))
                .pl(px(if leading_box { 0.0 } else { PJ_PLAIN_PAD }))
                .pr(px(PJ_PAD_RIGHT))
                .ml(px(PJ_MARGIN_X))
                .mr(px(PJ_MARGIN_X))
                .mt(px(PJ_MARGIN_TOP))
                .ui(PJ_TEXT)
                .text_color(p.ink_3)
                .cursor_pointer()
                .track_interaction(&hover_state);
            if self.chevron {
                // `.pj .chev{width:11px}` rotates 0° → 90° on the swap spring.
                let chev = super::chevron_sized(id.clone(), self.open, p.ink_3, PJ_CHEVRON, window, cx);
                row = row.child(
                    div().flex_none().w(px(LEADING_BOX)).flex().items_center().justify_center().child(chev),
                );
            } else if let Some((initial, colour)) = &self.mark {
                if !self.muted {
                    row = row.child(
                        div()
                            .flex_none()
                            .w(px(LEADING_BOX))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(project_mark(initial.clone(), *colour).size(px(GROUP_MARK))),
                    );
                }
            }
            row.child(
                div()
                    .min_w(px(NAME_MIN))
                    .truncate()
                    .medium()
                    .child(self.name),
            )
        };
        // One left-edge slot, shared by the current-project bar and the
        // rolled-up state: a bar in the state colour when a state is set
        // (running is the more urgent signal on a group that is both
        // current and running), else the accent bar for a current group.
        // Inside the row's margin and absolute, so it marks the row without
        // shifting anything in it — the labels stay at `NAV_LABEL_X`.
        let state_bar = self.state.map(|state| {
            let color = p.agent_state(state);
            let mut bar = div()
                .absolute()
                .left_0()
                .top(px(CURRENT_BAR_INSET))
                .bottom(px(CURRENT_BAR_INSET))
                .w(px(CURRENT_BAR_W))
                .rounded(px(CURRENT_BAR_W))
                .bg(color);
            // The shared pulse, sampled like the session dots: full at the
            // ring's birth, dim as it fades. No phase (reduced motion, or no
            // driver) rests solid, the way dots rest plain.
            if state == aui_tokens::AgentState::Running {
                if let Some(phase) = self.pulse_phase {
                    bar = bar.opacity(1.0 - STATE_BAR_FADE * phase);
                }
            }
            bar
        });
        if let Some(bar) = state_bar {
            row = row.child(bar);
        } else if self.current && self.current_bar {
            // Kept for a current group with no rolled-up state (the state
            // bar above took the slot otherwise): the same geometry in the
            // accent colour.
            row = row.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(CURRENT_BAR_INSET))
                    .bottom(px(CURRENT_BAR_INSET))
                    .w(px(CURRENT_BAR_W))
                    .rounded(px(CURRENT_BAR_W))
                    .bg(p.accent),
            );
        }
        row = row.child(div().flex_1());
        // The tray covers this end of the row, so the branch and the count
        // step aside while it is up: one tween drives both, so they fade out
        // exactly as the tray fades in. Opacity only — they keep their boxes,
        // so nothing reflows under the pointer (D6).
        let tray_opacity = if self.on_group_action.is_some() {
            aui_motion::tween((id.clone(), "tray-opacity"), if flags.hovered { 1.0f32 } else { 0.0 }, aui_motion::Tween::FAST, window, cx)
        } else {
            0.0
        };
        let under_tray = under_tray(tray_opacity);
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
                    .opacity(under_tray)
                    .child(trailing)
                    .into_any_element(),
            ));
        }
        // An empty count draws nothing rather than an empty pill: a caller
        // that has no number to show (a project with no sessions) says so by
        // passing none (audit 2026-09-13).
        if !self.count.is_empty() {
            row = row.child(div().flex_none().opacity(under_tray).child(tag(self.count)));
        }
        if self.on_group_action.is_some() {
            row = row.child(group_tray(&id, flags.hovered, tray_opacity, &self.on_group_action, &self.menu_bounds, window, cx));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// D6: the branch and the count are exactly what the tray is not, so a
    /// half-faded tray leaves them half-visible and the two never both claim
    /// the row's right end.
    #[test]
    fn the_tray_and_what_it_covers_are_complementary() {
        for tray in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            assert!((tray + under_tray(tray) - 1.0).abs() < f32::EPSILON, "tray {tray} leaves {}", under_tray(tray));
        }
        assert_eq!(under_tray(0.0), 1.0, "at rest the branch and the count are fully drawn");
        assert_eq!(under_tray(1.0), 0.0, "a fully shown tray hides both");
    }

    /// D4: `current` survives the builder, and it is independent of `muted` —
    /// the current project can be an archived one.
    #[test]
    fn current_is_its_own_flag() {
        assert!(!ProjectGroup::new("a", "a", "1").current);
        assert!(ProjectGroup::new("a", "a", "1").current(true).current);
        let muted_current = ProjectGroup::new("a", "a", "1").muted().current(true);
        assert!(muted_current.current && muted_current.muted);
    }

    /// The one gutter: the leading centre lands ≈ 18 px from the column
    /// edge and every label at `NAV_LABEL_X`.
    #[test]
    fn gutter_arithmetic_lands_the_leading_centre_and_labels() {
        assert_eq!(NAV_GUTTER, 8.0, "column edge → leading box");
        assert_eq!(LEADING_BOX, 20.0, "the nav-icon / chevron / dot box");
        assert_eq!(NAV_LABEL_X, 32.0, "where every label and title starts");
        assert_eq!(NAV_GUTTER + LEADING_BOX / 2.0, 18.0, "leading centre from the column edge");
        assert_eq!(NAV_GUTTER + LEADING_BOX + PJ_LEAD_GAP, NAV_LABEL_X, "box + gap reaches the labels");
        assert_eq!(PJ_LEAD_GAP, 4.0, "the label gap stays on the 4/8 grid");
        assert_eq!(
            PJ_MARGIN_X + PJ_PLAIN_PAD,
            NAV_GUTTER + LEADING_BOX / 2.0,
            "the plain name starts at the leading centre"
        );
    }

    /// Plain by default: no chevron box, no current bar. `current` alone
    /// changes nothing visible.
    #[test]
    fn group_rows_are_plain_unless_asked() {
        let group = ProjectGroup::new("a", "a", "1");
        assert!(!group.chevron && !group.current_bar, "data defaults to plain");
        let row = project_group_row("a", "a", "1", false);
        assert!(!row.chevron && !row.current_bar, "row defaults to plain");
        let dressed = project_group_row("a", "a", "1", false).chevron(true).current_bar(true);
        assert!(dressed.chevron && dressed.current_bar, "builders opt in");
    }
}
