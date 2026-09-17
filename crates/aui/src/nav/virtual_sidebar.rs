//! The virtualised sidebar sessions area: the same grouped sessions
//! [`SidebarView`](crate::nav::SidebarView) renders, flattened into a row
//! model and built through a gpui `list`, so a sidebar frame lays out
//! O(visible) rows instead of every session.
//!
//! # What gpui-pre 0.3.3 `ListState` offers (verified against
//! # `src/elements/list.rs` of the vendored crate)
//!
//! - **Variable heights: yes.** Each item is measured on its own
//!   (`layout_as_root` per item, heights kept in a `SumTree`), so a group
//!   header row, a one-line session row, a two-line meta row, the 28 px fold
//!   row and the date header can all share one list. The one contract (stated
//!   at the top of `list.rs`): elements outside the scrolled area must not
//!   change height without telling the list via [`splice`](gpui::ListState::splice)
//!   or [`reset`](gpui::ListState::reset). A width change already invalidates
//!   every cached height for free (the list's `prepaint` drops all
//!   measurements when the viewport width moves). Callers whose rows are all
//!   one height should look at `UniformList` instead; the sidebar is not that
//!   (meta and activity lines wrap rows to two or three lines).
//! - **Construction:** [`new`](gpui::ListState::new) takes
//!   `(item_count, alignment, overdraw)`. Overdraw is the runway, in pixels,
//!   above and below the viewport whose items are measured though not
//!   painted: the forward pass stops at `viewport + overdraw` and a second
//!   loop measures back into the leading overdraw. The sidebar passes
//!   [`SIDEBAR_OVERDRAW`]; see [`sidebar_list_state`].
//! - **Model sync:** [`reset`](gpui::ListState::reset) replaces the whole
//!   model (drops the scroll offset and ignores scroll events until the next
//!   paint); [`splice`](gpui::ListState::splice) swaps `old_range` for
//!   `count` fresh unmeasured items and repairs the logical scroll top
//!   (an offset inside the swapped range re-anchors to the range start, one
//!   after it shifts by the delta). There is also
//!   `remeasure_items(range)`, which keeps the count and the scroll position
//!   and only drops cached heights — the right call when a row's text grows
//!   without rows being added or removed.
//! - **Scrolling:** [`scroll_by`](gpui::ListState::scroll_by) walks the
//!   height tree by pixels (clamped at zero; a negative delta leaves
//!   tail-follow mode); [`logical_scroll_top`](gpui::ListState::logical_scroll_top)
//!   returns the `(item_ix, offset_in_item)` the tree walk landed on;
//!   [`scroll_to`](gpui::ListState::scroll_to) jumps to an absolute offset
//!   (clamped into the model). Neither the programmatic calls nor `reset`
//!   fire the scroll handler.
//! - **Reveal:** [`scroll_to_reveal_item`](gpui::ListState::scroll_to_reveal_item)
//!   is minimum-move ("nearest"): an item above the viewport scrolls to the
//!   top edge, one below scrolls just enough to bring its bottom edge in,
//!   and a fully visible item leaves the offset untouched. It reads the last
//!   laid-out viewport height, so it only steers correctly once the list has
//!   laid out; [`ensure_row_visible`] wraps it with that caveat documented.
//! - **Notifications:** [`set_scroll_handler`](gpui::ListState::set_scroll_handler)
//!   fires only on wheel scrolling (the list's own bubble-phase wheel
//!   listener), reporting the visible range, the count, and whether the list
//!   is scrolled — programmatic `scroll_by` drains do not call it.
//! - **Measuring modes:** the default builds only the visible window plus
//!   overdraw each frame; `measure_all` instead measures every item on the
//!   first frame (correct scrollbar from frame one, linear upfront cost);
//!   `with_uniform_item_height` seeds every item with a height hint so the
//!   scrollbar is about right until real heights replace it. The sidebar
//!   uses the default: with 200 sessions only the visible ~15–25 rows plus
//!   the overdraw runway are built per frame.
//! - **Probes:** `bounds_for_item(ix)` (window bounds once rendered),
//!   `item_is_above_viewport` / `item_is_below_viewport` (viewport-relative
//!   answers, `None` before layout), and `viewport_bounds`.
//!
//! # Adoption guide for the harness (package C)
//!
//! 1. **Own the state.** Keep one `ListState` beside the sidebar model
//!    (e.g. on the sidebar pane) and hand a clone to
//!    [`virtual_sidebar_view`] every frame — `ListState` is a cheap
//!    reference-counted handle, and the component itself stays stateless.
//!    Build it with [`sidebar_list_state`].
//! 2. **Sync on model change.** Flatten with [`flatten_sidebar`] each frame
//!    (an index walk, no summaries cloned) and keep
//!    `state.item_count() == rows.len()`: after a regroup or filter call
//!    `state.reset(rows.len())`; after a local insert/remove call
//!    `state.splice(range, count)`; after a text-only change that may move
//!    row heights call `remeasure_items(range)`. A mismatch never panics —
//!    the list paints blanks past the model — but the scrollbar and the
//!    reveal math go stale until the counts agree.
//! 3. **Drain the wheel.** Accumulate wheel travel in a capture-phase
//!    handler that stops propagation (so the list's own bubble-phase wheel
//!    listener, registered in its `paint`, never double-applies), then apply
//!    one `state.scroll_by(delta)` per frame before the list lays out — the
//!    same accumulator/drain shape as the transcript. The component installs
//!    no competing handler of its own.
//! 4. **Reveal.** Map the session with
//!    [`row_index_for_session`] and steer with [`ensure_row_visible`]. A
//!    session the caller held back behind a fold is not in the model, so the
//!    mapping returns `None`: expand first (answer the fold row's
//!    `ToggleMore`, pass the full sessions, `reset`), then reveal. Call
//!    `ensure_row_visible` on selection change and for the frames until the
//!    row reports visible — before the first layout the viewport height is
//!    still unknown, and once the row is fully visible the call is a no-op.
//!
//! # Deliberate differences from [`SidebarView`](crate::nav::SidebarView)
//!
//! - Open/close snaps instead of animating: the old path wraps each body in
//!   a spring `collapse` element, which a virtual item cannot host (an
//!   animating height would need a `splice`/`remeasure` every tick anyway).
//!   Closed groups contribute only their header row to the flattened model.
//! - Everything else is shared code: each visible row is built by
//!   `session_row_element`, each project head by `project_head_element`,
//!   and the headers, caption and fold rows are the same builders with the
//!   same element ids, so geometry, tokens, hover, selection, the running
//!   dot, the per-row menu trigger and the round-5 x positions (nav icon
//!   centre / plain label first glyph / dot at 18, titles at 32) are
//!   identical in both paths and both themes.

use std::rc::Rc;

use gpui::{div, list, prelude::*, px, AnyElement, App, Bounds, ElementId, IntoElement, ListAlignment, ListState, Pixels, RenderOnce, SharedString, Window};

use super::views::{
    fold_row, project_head_element, session_row_element, BoundsHandler, EditorBuilder, GroupActionHandler, PlainHandler, RowActionHandler,
    RowHoverHandler, SelectHandler, ToggleHandler,
};
use crate::nav::{date_group_header, group_header, group_row, GroupAction, Grouping, RowAction, SessionSummary};

/// The measured-but-unpainted runway above and below the sidebar viewport.
///
/// About four rows a side at the shared 30 px row metric: enough that a fast
/// wheel burst never shows unmeasured rows, small enough that a frame still
/// builds O(visible) rows. Scroll tuning, not a visual size.
pub const SIDEBAR_OVERDRAW: f32 = 120.0;

/// Build the caller-owned [`ListState`] for a sidebar of `item_count` rows:
/// top-aligned, with the [`SIDEBAR_OVERDRAW`] runway.
pub fn sidebar_list_state(item_count: usize) -> ListState {
    ListState::new(item_count, ListAlignment::Top, px(SIDEBAR_OVERDRAW))
}

/// Which sessions a [`SidebarRow::Session`] points at. The key prefix each
/// scope renders under mirrors the non-virtualised view exactly: the group
/// id for status/project rows, `"pinned"` for the lifted pinned rows, and
/// `"date-{n}"` (counting only buckets that still have rows, as the old
/// path's `enumerate` does) for date rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionScope {
    /// A status group's session.
    Status,
    /// A project group's session.
    Project,
    /// A pinned session lifted into the leading group of a date grouping.
    Pinned,
    /// A date bucket's session; `dated` is the bucket's index among the
    /// buckets that still have rows.
    Date {
        /// Index among non-empty buckets, matching the old path's keys.
        dated: usize,
    },
}

/// One flattened row of a sidebar grouping. Build with [`flatten_sidebar`];
/// the fields stay private so only the flattener (which mirrors the
/// non-virtualised render line for line) can mint rows, and the two paths
/// cannot disagree about order, folds, or keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarRow {
    /// The caps caption row; emitted first when the view has a caption.
    Caption,
    /// A collapsible status header; `group` indexes the status groups.
    StatusHeader {
        /// Index into the status groups.
        group: usize,
    },
    /// A project group head; `group` indexes the project groups.
    ProjectHead {
        /// Index into the project groups.
        group: usize,
    },
    /// The leading `Pinned` header of a date grouping (only when a pinned
    /// session exists).
    PinnedHeader,
    /// A date bucket header; `group` indexes all buckets, `dated` the ones
    /// that still have rows (the key suffix).
    DateHeader {
        /// Index into all date buckets.
        group: usize,
        /// Index among buckets that still have rows.
        dated: usize,
    },
    /// One session; `group`/`session` index the source grouping (`session`
    /// always indexes the bucket's own vec, pinned or not — the row's
    /// position in the flattened vec carries which section it is in).
    Session {
        /// Which section of the grouping this row belongs to.
        scope: SessionScope,
        /// Group or bucket index in the source grouping.
        group: usize,
        /// Session index in the group's own vec.
        session: usize,
    },
    /// A project's "Show N more" / "Show less" row; `group` indexes the
    /// project groups.
    Fold {
        /// Index into the project groups.
        group: usize,
    },
}

/// Flatten a grouping into its rows, in render order: the caption first when
/// `caption` is set, then per group the header/head plus — for open groups
/// only — the sessions and (project only) the fold row when rows are held
/// back. Closed groups contribute only their header/head; the list cannot
/// host the old path's spring `collapse`, so open/close snaps.
///
/// Date groupings lift pinned sessions into a leading `Pinned` section ahead
/// of the buckets, exactly like the non-virtualised view; a session the
/// caller held back behind a fold is not in the model and has no row.
pub fn flatten_sidebar(grouping: &Grouping, caption: bool) -> Vec<SidebarRow> {
    let mut rows = Vec::new();
    if caption {
        rows.push(SidebarRow::Caption);
    }
    match grouping {
        Grouping::Status(groups) => {
            for (g, group) in groups.iter().enumerate() {
                rows.push(SidebarRow::StatusHeader { group: g });
                if group.open {
                    for s in 0..group.sessions.len() {
                        rows.push(SidebarRow::Session { scope: SessionScope::Status, group: g, session: s });
                    }
                }
            }
        }
        Grouping::Project(groups) => {
            for (g, group) in groups.iter().enumerate() {
                rows.push(SidebarRow::ProjectHead { group: g });
                if group.open {
                    for s in 0..group.sessions.len() {
                        rows.push(SidebarRow::Session { scope: SessionScope::Project, group: g, session: s });
                    }
                    if group.fold.map(|(hidden, _)| hidden > 0).unwrap_or(false) {
                        rows.push(SidebarRow::Fold { group: g });
                    }
                }
            }
        }
        Grouping::Date(groups) => {
            let mut pinned: Vec<(usize, usize)> = Vec::new();
            let mut dated: Vec<usize> = Vec::new();
            for (g, group) in groups.iter().enumerate() {
                let mut rest_empty = true;
                for (s, session) in group.sessions.iter().enumerate() {
                    if session.pinned {
                        pinned.push((g, s));
                    } else {
                        rest_empty = false;
                    }
                }
                if !rest_empty {
                    dated.push(g);
                }
            }
            if !pinned.is_empty() {
                rows.push(SidebarRow::PinnedHeader);
                for (g, s) in pinned {
                    rows.push(SidebarRow::Session { scope: SessionScope::Pinned, group: g, session: s });
                }
            }
            for (dated_ix, g) in dated.iter().enumerate() {
                rows.push(SidebarRow::DateHeader { group: *g, dated: dated_ix });
                for (s, session) in groups[*g].sessions.iter().enumerate() {
                    if !session.pinned {
                        rows.push(SidebarRow::Session {
                            scope: SessionScope::Date { dated: dated_ix },
                            group: *g,
                            session: s,
                        });
                    }
                }
            }
        }
    }
    rows
}

/// The session a flattened row points at.
fn session_summary<'a>(grouping: &'a Grouping, scope: &SessionScope, group: usize, session: usize) -> &'a SessionSummary {
    match (grouping, scope) {
        (Grouping::Status(groups), SessionScope::Status) => &groups[group].sessions[session],
        (Grouping::Project(groups), SessionScope::Project) => &groups[group].sessions[session],
        (Grouping::Date(groups), SessionScope::Pinned) => &groups[group].sessions[session],
        (Grouping::Date(groups), SessionScope::Date { .. }) => &groups[group].sessions[session],
        _ => unreachable!("flatten_sidebar only emits session rows that match their grouping"),
    }
}

/// The flattened index of `session_id`, or `None` when the session has no
/// row: an unknown id, or a session the caller held back behind a fold
/// (expand the fold and re-flatten first). Compare against
/// `state.item_count()` before revealing: the caller must have `reset` the
/// state to the flattened length already.
pub fn row_index_for_session(rows: &[SidebarRow], grouping: &Grouping, session_id: &SharedString) -> Option<usize> {
    rows.iter().position(|row| match row {
        SidebarRow::Session { scope, group, session } => session_summary(grouping, scope, *group, *session).id == *session_id,
        _ => false,
    })
}

/// Steer the list so row `ix` is visible, moving the least distance that
/// shows it whole: a row above the viewport lands on the top edge, one below
/// just clears the bottom edge, and a fully visible row changes nothing
/// (gpui's `scroll_to_reveal_item` "nearest" semantics).
///
/// The reveal reads the last laid-out viewport height, so before the first
/// layout it cannot know what is visible: call on selection change and for
/// the frames after, until the row reports visible. Out-of-range indices
/// clamp in the list instead of panicking.
pub fn ensure_row_visible(state: &ListState, ix: usize) {
    state.scroll_to_reveal_item(ix);
}
/// The body of a virtualised sidebar panel in one grouping. Build with
/// [`virtual_sidebar_view`].
///
/// Like [`SidebarView`](crate::nav::SidebarView) this is stateless: the
/// scroll state lives with the caller (see the adoption guide above), data
/// comes in and intents go out through closures. The flattened rows are
/// derived from the grouping on every render; only rows the list asks for
/// (the visible window plus the [`SIDEBAR_OVERDRAW`] runway) are built.
///
/// Wheel scrolling is caller-driven: accumulate wheel travel in a
/// capture-phase handler that stops propagation, then drain one
/// `state.scroll_by(delta)` per frame. This component installs no wheel
/// handler of its own — the only wheel listener in the subtree is the gpui
/// list's own bubble-phase one, which the caller's capture-phase accumulator
/// suppresses while draining, exactly like the transcript.
#[derive(IntoElement)]
pub struct VirtualSidebarView {
    id: ElementId,
    grouping: Rc<Grouping>,
    state: ListState,
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
    on_selected_prepainted: Option<BoundsHandler>,
    on_current_prepainted: Option<BoundsHandler>,
    on_group_menu_prepainted: Option<BoundsHandler>,
    on_row_built: Option<Rc<dyn Fn(usize)>>,
}

/// The virtualised sessions of a sidebar, grouped by `grouping`.
///
/// `state` is the caller-owned [`ListState`] built by [`sidebar_list_state`]
/// and kept in sync with [`flatten_sidebar`] (see the adoption guide above);
/// the grouping itself is held behind an [`Rc`] like the non-virtualised
/// view, so a caller that caches the built grouping pays nothing per frame.
pub fn virtual_sidebar_view(id: impl Into<ElementId>, grouping: impl Into<Rc<Grouping>>, state: ListState) -> VirtualSidebarView {
    VirtualSidebarView {
        id: id.into(),
        grouping: grouping.into(),
        state,
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
        on_selected_prepainted: None,
        on_current_prepainted: None,
        on_group_menu_prepainted: None,
        on_row_built: None,
    }
}

impl VirtualSidebarView {
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
    /// several (measure, then paint) — so each one draws the field, and any
    /// number of sidebar paths may render in a frame while renaming. The
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

    /// A session row's hover state changed; the arguments are the session id
    /// and whether the pointer entered. Fires from the row's own hover
    /// events, so the caller learns about the pointer even when nothing
    /// re-renders — what arms the hover detail's delay.
    pub fn on_row_hover(mut self, f: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    /// A project group row's hover action was clicked; the arguments are the
    /// group id and what the tray button asked for.
    pub fn on_group_action(mut self, f: impl Fn(&SharedString, GroupAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_group_action = Some(Rc::new(f));
        self
    }

    /// Fires once per frame with the selected session row's bounds. The
    /// library stores nothing; the app decides what to do with the bounds
    /// (for example, seating a trigger menu at the row). Only fires while
    /// the selected row is built — a scrolled-out selection reports nothing
    /// until it is revealed.
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

    /// Fires once per frame with every built project group row's tray `…`
    /// button bounds, keyed by group id — what a group-row menu seats at.
    /// See [`Self::on_selected_prepainted`].
    pub fn on_group_menu_prepainted(
        mut self,
        f: impl Fn(&SharedString, Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_group_menu_prepainted = Some(Rc::new(f));
        self
    }

    /// Debug/testing hook: fires with the flattened index each time the list
    /// builds a row (visible rows plus the overdraw runway, plus any measure
    /// pass), so a test or the gallery can prove only visible rows are
    /// built. Never fires for rows the list skips. Not for production use:
    /// it runs inside layout, so it must only record (a counter put) and
    /// never notify or mutate UI state.
    pub fn on_row_built(mut self, f: impl Fn(usize) + 'static) -> Self {
        self.on_row_built = Some(Rc::new(f));
        self
    }
}

/// One flattened row, built with the same builders and element ids as the
/// non-virtualised view.
///
/// The row sits inside a marginless full-width box first: gpui's `list`
/// lays each item out at the full list width, so a row root's own margins
/// never inset it the way they do as a flex-column child on the old path —
/// the session rows would come out 8 px wider on each side and the date
/// headers would lose their 8 px top margin (one missing gap per header,
/// accumulating down the list). Inside the box the row is an ordinary block
/// child again, so its margins apply exactly as they do in the flex column
/// and the list measures the box at the row's true outer geometry. Same
/// element ids, same geometry, same intents.
#[allow(clippy::too_many_arguments)]
fn render_flat_row(
    view_id: &ElementId,
    grouping: &Grouping,
    row: &SidebarRow,
    caption: &Option<SharedString>,
    selected: &Option<SharedString>,
    actions: &[RowAction],
    editing: &Option<(SharedString, EditorBuilder)>,
    pulse_phase: Option<f32>,
    on_select: &Option<SelectHandler>,
    on_toggle: &Option<ToggleHandler>,
    on_view_options: &Option<PlainHandler>,
    on_action: &Option<RowActionHandler>,
    on_group_action: &Option<GroupActionHandler>,
    on_hover: &Option<RowHoverHandler>,
    on_selected_prepainted: &Option<BoundsHandler>,
    on_current_prepainted: &Option<BoundsHandler>,
    on_group_menu_prepainted: &Option<BoundsHandler>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    div().w_full().child(match row {
        SidebarRow::Caption => {
            // `flatten_sidebar` only emits this when a caption is set.
            let text = caption.clone().unwrap_or_default();
            let mut caption_row = group_row((view_id.clone(), "caption"), text);
            if let Some(h) = on_view_options.clone() {
                caption_row = caption_row.on_view_options(move |_, w, cx| h(w, cx));
            }
            caption_row.into_any_element()
        }
        SidebarRow::StatusHeader { group } => {
            let status = match grouping {
                Grouping::Status(groups) => &groups[*group],
                _ => unreachable!("flatten_sidebar only emits status headers for a status grouping"),
            };
            let key: ElementId = (view_id.clone(), status.id.clone()).into();
            let mut header =
                group_header((key.clone(), "header"), status.label.clone(), status.open).count(status.count.clone());
            if let Some(h) = on_toggle.clone() {
                let group_id = status.id.clone();
                header = header.on_toggle(move |_, w, cx| h(&group_id, w, cx));
            }
            header.into_any_element()
        }
        SidebarRow::ProjectHead { group } => {
            let project = match grouping {
                Grouping::Project(groups) => &groups[*group],
                _ => unreachable!("flatten_sidebar only emits project heads for a project grouping"),
            };
            let key: ElementId = (view_id.clone(), project.id.clone()).into();
            project_head_element(
                &key,
                project,
                on_toggle,
                on_group_action,
                on_group_menu_prepainted,
                on_current_prepainted,
                pulse_phase,
            )
        }
        SidebarRow::PinnedHeader => {
            let mut count = 0;
            if let Grouping::Date(groups) = grouping {
                for group in groups {
                    count += group.sessions.iter().filter(|s| s.pinned).count();
                }
            }
            let key: ElementId = (view_id.clone(), "pinned").into();
            // The pinned header is not collapsible: no toggle, like the old path.
            group_header((key.clone(), "header"), "Pinned", true).count(count.to_string()).into_any_element()
        }
        SidebarRow::DateHeader { group, .. } => {
            let bucket = match grouping {
                Grouping::Date(groups) => &groups[*group],
                _ => unreachable!("flatten_sidebar only emits date headers for a date grouping"),
            };
            date_group_header(bucket.label.clone()).into_any_element()
        }
        SidebarRow::Session { scope, group, session } => {
            let summary = session_summary(grouping, scope, *group, *session);
            let key: ElementId = match scope {
                SessionScope::Status | SessionScope::Project => match grouping {
                    Grouping::Status(groups) => (view_id.clone(), groups[*group].id.clone()).into(),
                    Grouping::Project(groups) => (view_id.clone(), groups[*group].id.clone()).into(),
                    _ => unreachable!("flatten_sidebar only emits session rows that match their grouping"),
                },
                SessionScope::Pinned => (view_id.clone(), "pinned").into(),
                SessionScope::Date { dated } => (view_id.clone(), SharedString::from(format!("date-{dated}"))).into(),
            };
            let row_id: ElementId = (key.clone(), summary.id.clone()).into();
            session_row_element(
                row_id,
                summary,
                selected,
                actions,
                editing,
                on_select,
                on_action,
                on_hover,
                on_selected_prepainted,
                pulse_phase,
                window,
                cx,
            )
        }
        SidebarRow::Fold { group } => {
            let project = match grouping {
                Grouping::Project(groups) => &groups[*group],
                _ => unreachable!("flatten_sidebar only emits fold rows for a project grouping"),
            };
            let key: ElementId = (view_id.clone(), project.id.clone()).into();
            let (hidden, expanded) = project.fold.unwrap_or((0, false));
            fold_row((key.clone(), "more"), &project.id, hidden, expanded, on_group_action, window, cx)
        }
    })
    .into_any_element()
}

impl RenderOnce for VirtualSidebarView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let VirtualSidebarView {
            id,
            grouping,
            state,
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
            on_selected_prepainted,
            on_current_prepainted,
            on_group_menu_prepainted,
            on_row_built,
        } = self;
        let rows = flatten_sidebar(&grouping, caption.is_some());
        let render = move |ix: usize, window: &mut Window, cx: &mut App| -> AnyElement {
            if let Some(hook) = on_row_built.clone() {
                hook(ix);
            }
            // A stale state (the caller grew the model without `reset`) asks
            // for rows past the model: paint a blank instead of panicking.
            // The scrollbar and reveal math are stale until the counts agree
            // again — see the adoption guide.
            match rows.get(ix) {
                Some(row) => render_flat_row(
                    &id,
                    &grouping,
                    row,
                    &caption,
                    &selected,
                    &actions,
                    &editing,
                    pulse_phase,
                    &on_select,
                    &on_toggle,
                    &on_view_options,
                    &on_action,
                    &on_group_action,
                    &on_hover,
                    &on_selected_prepainted,
                    &on_current_prepainted,
                    &on_group_menu_prepainted,
                    window,
                    cx,
                ),
                None => div().into_any_element(),
            }
        };
        // The column gives this wrapper its height (flex); the list fills it.
        div().w_full().flex_1().min_h(px(0.0)).child(list(state, render).w_full().h_full())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::{DateGroup, ProjectGroup, StatusGroup};
    use aui_tokens::AgentState;

    fn session(id: &str) -> SessionSummary {
        SessionSummary::new(id, id, AgentState::Idle, "1h")
    }

    fn status_grouping() -> Grouping {
        Grouping::Status(vec![
            StatusGroup::new("needs-you", "Needs you", "1", vec![session("a")]),
            StatusGroup::new("running", "Running", "2", vec![session("b"), session("c")]).closed(),
            StatusGroup::new("done", "Done", "0", Vec::new()),
        ])
    }

    /// Headers survive in order and a closed group contributes only its
    /// header: the list cannot host the old path's spring collapse.
    #[test]
    fn status_order_is_stable_and_closed_groups_contribute_only_their_header() {
        let rows = flatten_sidebar(&status_grouping(), false);
        assert_eq!(
            rows,
            vec![
                SidebarRow::StatusHeader { group: 0 },
                SidebarRow::Session { scope: SessionScope::Status, group: 0, session: 0 },
                SidebarRow::StatusHeader { group: 1 },
                SidebarRow::StatusHeader { group: 2 },
            ]
        );
    }

    /// A fold row follows the visible sessions only while rows are held back;
    /// a closed group shows its head alone.
    #[test]
    fn project_folds_follow_the_visible_sessions() {
        let grouping = Grouping::Project(vec![
            ProjectGroup::new("web", "web", "8").folded(5, false).open(vec![session("a"), session("b"), session("c")]),
            ProjectGroup::new("api", "api", "3").open(vec![session("d")]),
            ProjectGroup::new("old", "old", "2"),
        ]);
        let rows = flatten_sidebar(&grouping, false);
        assert_eq!(
            rows,
            vec![
                SidebarRow::ProjectHead { group: 0 },
                SidebarRow::Session { scope: SessionScope::Project, group: 0, session: 0 },
                SidebarRow::Session { scope: SessionScope::Project, group: 0, session: 1 },
                SidebarRow::Session { scope: SessionScope::Project, group: 0, session: 2 },
                SidebarRow::Fold { group: 0 },
                SidebarRow::ProjectHead { group: 1 },
                SidebarRow::Session { scope: SessionScope::Project, group: 1, session: 0 },
                SidebarRow::ProjectHead { group: 2 },
            ]
        );
    }

    /// Pinned sessions lift ahead of the buckets in bucket order; the
    /// `dated` key suffix counts only buckets that still have rows, so an
    /// emptied bucket leaves no gap in the keys.
    #[test]
    fn date_buckets_lift_pinned_first_and_number_only_nonempty_buckets() {
        let grouping = Grouping::Date(vec![
            DateGroup::new("Today", vec![session("a").pinned(), session("b")]),
            DateGroup::new("Yesterday", vec![session("c").pinned()]),
            DateGroup::new("This week", vec![session("d")]),
        ]);
        let rows = flatten_sidebar(&grouping, false);
        assert_eq!(
            rows,
            vec![
                SidebarRow::PinnedHeader,
                SidebarRow::Session { scope: SessionScope::Pinned, group: 0, session: 0 },
                SidebarRow::Session { scope: SessionScope::Pinned, group: 1, session: 0 },
                SidebarRow::DateHeader { group: 0, dated: 0 },
                SidebarRow::Session { scope: SessionScope::Date { dated: 0 }, group: 0, session: 1 },
                SidebarRow::DateHeader { group: 2, dated: 1 },
                SidebarRow::Session { scope: SessionScope::Date { dated: 1 }, group: 2, session: 0 },
            ]
        );
    }

    /// The caption is one leading row; an empty grouping flattens to nothing
    /// (or to the caption alone).
    #[test]
    fn caption_is_one_leading_row_and_empty_groupings_flatten_to_nothing() {
        let empty = Grouping::Project(Vec::new());
        assert!(flatten_sidebar(&empty, false).is_empty());
        assert_eq!(flatten_sidebar(&empty, true), vec![SidebarRow::Caption]);
        let rows = flatten_sidebar(&status_grouping(), true);
        assert_eq!(rows[0], SidebarRow::Caption);
        assert_eq!(rows.len(), 5);
    }

    /// The selected session maps to its flattened index; a session held back
    /// behind a fold has no row until the caller expands and re-flattens.
    #[test]
    fn session_index_finds_the_selected_row_and_misses_rows_behind_a_fold() {
        let grouping = Grouping::Project(vec![
            ProjectGroup::new("web", "web", "8").folded(1, false).open(vec![session("a"), session("b")]),
        ]);
        let rows = flatten_sidebar(&grouping, true);
        assert_eq!(row_index_for_session(&rows, &grouping, &"b".into()), Some(3));
        // Held back behind the fold: no row yet.
        assert_eq!(row_index_for_session(&rows, &grouping, &"held-back".into()), None);
        // Expanded, the caller passes every session and the row appears.
        let expanded = Grouping::Project(vec![
            ProjectGroup::new("web", "web", "8").folded(1, true).open(vec![session("a"), session("b"), session("held-back")]),
        ]);
        let expanded_rows = flatten_sidebar(&expanded, true);
        assert_eq!(row_index_for_session(&expanded_rows, &expanded, &"held-back".into()), Some(4));
        // Unknown ids never match.
        assert_eq!(row_index_for_session(&expanded_rows, &expanded, &"nope".into()), None);
    }

    /// The state helper builds a top-aligned list with the sidebar overdraw.
    #[test]
    fn list_state_helper_counts_and_starts_at_the_top() {
        let state = sidebar_list_state(203);
        assert_eq!(state.item_count(), 203);
        let top = state.logical_scroll_top();
        assert_eq!(top.item_ix, 0);
    }
}
