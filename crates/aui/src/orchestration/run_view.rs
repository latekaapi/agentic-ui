//! The run view: a right-pane body with the task tree and the evidence
//! table. Section headers are caps-style muted labels with a hairline under
//! them. Every tree node is clickable and names its own state, so the list
//! reads without a live region. There are no header buttons.

use std::rc::Rc;

use aui_motion::{tint_fade, Tween};
use aui_protocol::{RunViewModel, RunViewNode};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{record_ax_label, status_dot};
use crate::icons::provider_mark;
use crate::util::{interaction_flags, TrackInteraction};

use super::common::{dot_state, mark, state_word};

/// Body rhythm.
const PAD_X: f32 = 12.0;
const SECTION_GAP: f32 = 12.0;
const ROW_GAP: f32 = 6.0;
const LIST_GAP: f32 = 2.0;
const ROW_H: f32 = 30.0;
/// One nesting depth indents 18 px.
const INDENT: f32 = 18.0;
/// Provider marks in the tree.
const MARK: f32 = 14.0;

type IntentHandler = Rc<dyn Fn(RunViewIntent, &mut Window, &mut App)>;

/// What the person asked for on a run view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunViewIntent {
    /// Open one worker session; the task id.
    OpenWorker(String),
}

/// The run tree view. Build with [`run_view`].
#[derive(IntoElement)]
pub struct RunView {
    id: ElementId,
    view: RunViewModel,
    frozen: bool,
    on_intent: Option<IntentHandler>,
}

/// A run's task tree plus its evidence table.
///
/// The view clones the value; it owns no orchestration logic.
pub fn run_view(id: impl Into<ElementId>, view: &RunViewModel) -> RunView {
    RunView { id: id.into(), view: view.clone(), frozen: false, on_intent: None }
}

impl RunView {
    /// A tree node was picked; the task id is echoed back.
    pub fn on_intent(mut self, f: impl Fn(RunViewIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }

    /// Freezes the tree's dots: a paused tree draws static dots and requests
    /// no frames once settled. Defaults to `false`.
    pub fn frozen(mut self, frozen: bool) -> Self {
        self.frozen = frozen;
        self
    }
}

/// `Implement · auth call, working, open worker session` — every node names
/// its own state, so the tree reads without a live region.
fn node_label(node: &RunViewNode) -> String {
    format!("{}, {}, open worker session", node.label, state_word(node.state))
}

/// A caps-style section header with a hairline under it.
fn section_header(title: &str, p: &aui_tokens::Palette) -> AnyElement {
    v_flex()
        .w_full()
        .gap(px(ROW_GAP))
        .child(div().text_role(TextRole::Caps).text_color(p.ink_3).child(title.to_owned()))
        .child(div().w_full().h(px(1.0)).bg(p.line))
        .into_any_element()
}

impl RenderOnce for RunView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        let mut tree = v_flex().w_full().gap(px(LIST_GAP));
        for node in &self.view.tree {
            let label = node_label(node);
            let node_id: ElementId =
                (id.clone(), SharedString::from(format!("node-{}", node.task_id))).into();
            let (agent, pulse) = dot_state(node.state, self.frozen);
            // A node is a button only while a handler is set; otherwise it
            // is plain text with no role and no hover tint.
            let mut row = h_flex()
                .id(node_id.clone())
                .w_full()
                .h(px(ROW_H))
                .items_center()
                .gap(px(ROW_GAP))
                .px(px(PAD_X))
                .pl(px(PAD_X + INDENT * node.depth as f32))
                .overflow_hidden()
                .ui(scale::FS_12);
            if self.on_intent.is_some() {
                let (row_state, row_flags) = interaction_flags(node_id.clone(), window, cx);
                let row_bg =
                    tint_fade((node_id.clone(), "bg"), row_flags.hovered, p.surface_3, Tween::FAST, window, cx);
                record_ax_label(&label);
                row = row
                    .bg(row_bg)
                    .cursor_pointer()
                    .track_interaction(&row_state)
                    .role(gpui::Role::Button)
                    .aria_label(label);
            }
            row = row
                .child(status_dot((node_id.clone(), "dot"), agent).pulse(pulse))
                .child(provider_mark(mark(node.provider)).size(px(MARK)))
                .child(div().min_w(px(0.0)).truncate().text_color(p.ink_2).child(node.label.clone()));
            if let Some(note) = &node.note {
                row = row.child(
                    div().min_w(px(0.0)).truncate().text_color(p.ink_3).child(note.clone()),
                );
            }
            if let Some(on_intent) = self.on_intent.clone() {
                let task_id = node.task_id.clone();
                row = row.on_click(move |_, window, cx| {
                    on_intent(RunViewIntent::OpenWorker(task_id.clone()), window, cx)
                });
            }
            tree = tree.child(row);
        }

        let mut evidence = v_flex().w_full().gap(px(LIST_GAP));
        for line in &self.view.evidence {
            evidence = evidence.child(
                div()
                    .w_full()
                    .px(px(PAD_X))
                    .mono(scale::FS_12)
                    .text_color(p.ink_2)
                    .child(line.text.clone()),
            );
        }

        let group_label = format!("Run view: {}", self.view.title);
        record_ax_label(&group_label);

        v_flex()
            .id(id)
            .w_full()
            .gap(px(SECTION_GAP))
            .role(gpui::Role::Group)
            .aria_label(group_label)
            .child(v_flex().w_full().gap(px(ROW_GAP)).child(section_header("Tasks", &p)).child(tree))
            .child(
                v_flex().w_full().gap(px(ROW_GAP)).child(section_header("Evidence", &p)).child(evidence),
            )
    }
}
