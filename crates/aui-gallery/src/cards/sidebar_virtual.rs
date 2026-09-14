//! Card nav/sidebar-virtual · the virtualised sidebar sessions area. Eight
//! project groups of 25 sessions (200 sessions, 3 fold rows, one caption),
//! rendered through the gpui `list` behind [`virtual_sidebar_view`]: only the
//! visible window plus the overdraw runway is built per frame. The note under
//! the panel reports the previous frame's built-row count beside the total,
//! so the screenshots carry the virtualisation proof.

use std::cell::Cell;
use std::rc::Rc;

use aui::nav::{ensure_row_visible, flatten_sidebar, row_index_for_session, sidebar_list_state, virtual_sidebar_view, Grouping, ProjectGroup};
use aui::nav::{ActivityKind, MetaItem, SessionSummary};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.side{width:300px;height:700px}` — the ~700 px viewport the bench and the
/// brief measure against.
const PANEL_W: f32 = 300.0;
const PANEL_H: f32 = 700.0;
/// `.ttl` above the panel.
const TITLE_GAP: f32 = 8.0;
/// The note under the panel.
const NOTE_TOP: f32 = 8.0;

/// Eight groups of 25 sessions with mixed states: one-line rows, two-line
/// meta rows and three-line activity rows, so the list also proves variable
/// heights.
fn project_groups() -> Vec<ProjectGroup> {
    let states = [AgentState::Running, AgentState::Waiting, AgentState::Done, AgentState::Failed, AgentState::Idle];
    let mut groups = Vec::new();
    for g in 0..8 {
        let mut sessions = Vec::new();
        for s in 0..25 {
            let id: SharedString = format!("card-g{g}-s{s}").into();
            let name: SharedString = format!("session-{g}-{s}").into();
            let mut summary = SessionSummary::new(id, name, states[(g + s) % states.len()], "1h").repo("acme-web");
            if (g + s) % 3 == 0 {
                summary = summary.meta(MetaItem::Text("PR #2491 open".into()));
            }
            if (g + s) % 4 == 0 {
                summary = summary.activity(ActivityKind::Working, "running regression tests");
            }
            if states[(g + s) % states.len()] == AgentState::Running {
                summary = summary.pulse();
            }
            sessions.push(summary);
        }
        let mut group = ProjectGroup::new(format!("card-g{g}"), format!("project-{g}"), "25").open(sessions);
        if g % 3 == 0 {
            group = group.folded(10, false);
        }
        groups.push(group);
    }
    groups
}

/// The selected session sits deep in the list (group 5), so the card opens
/// scrolled: the caller-owned state steers to it before the first layout.
const SELECTED: &str = "card-g5-s14";

/// What the card remembers: the caller-owned list state plus the build
/// counter the `on_row_built` hook records into. `last` is the previous
/// frame's count (build runs before layout, so the current frame's count is
/// not known yet when the note renders).
struct VirtualState {
    list: ListState,
    count: Rc<Cell<usize>>,
    last: usize,
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let grouping = Rc::new(Grouping::Project(project_groups()));
    let rows = flatten_sidebar(&grouping, true);
    let total = rows.len();
    let selected_ix = row_index_for_session(&rows, &grouping, &SELECTED.into());

    let state = window.use_keyed_state("sidebar-virtual", cx, |_, _| {
        let list = sidebar_list_state(total);
        // Open scrolled so the selected row is visible on the first frame.
        if let Some(ix) = selected_ix {
            use gpui::ListOffset;
            list.scroll_to(ListOffset { item_ix: ix.saturating_sub(2), offset_in_item: px(0.) });
        }
        VirtualState { list, count: Rc::new(Cell::new(0)), last: 0 }
    });
    // The model is static, so the count cannot drift; still, keep the
    // contract explicit rather than trusting it.
    state.update(cx, |s, _| {
        if s.list.item_count() != total {
            s.list.reset(total);
        }
        s.last = s.count.replace(0);
    });
    let (list, last) = state.update(cx, |s, _| (s.list.clone(), s.last));
    if let Some(ix) = selected_ix {
        ensure_row_visible(&list, ix);
    }
    let hook = state.read(cx).count.clone();

    let panel = v_flex()
        .w(px(PANEL_W))
        .h(px(PANEL_H))
        .flex_none()
        .overflow_hidden()
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .child(
            virtual_sidebar_view("card-sidebar-virtual", grouping, list)
                .caption("Projects")
                .selected(SELECTED)
                .on_row_built(move |_| {
                    hook.set(hook.get() + 1);
                }),
        );
    v_flex()
        .flex_none()
        .w(px(PANEL_W))
        .child(div().mb(px(TITLE_GAP)).ui(scale::FS_12).text_color(p.ink_3).child("Virtualised sidebar · 200 sessions"))
        .child(panel)
        .child(
            div()
                .mt(px(NOTE_TOP))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child(format!("{last} rows built last frame · {total} total")),
        )
        .into_any_element()
}
