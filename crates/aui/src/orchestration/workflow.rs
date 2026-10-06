//! The provider-native workflow: the provider's mark, the workflow name, a
//! `native` pill, a `provisional` pill while the provider may still reshape
//! the tree, the done count, and one row per child. Failed children carry
//! Retry and Skip buttons when the provider allows them.

use std::rc::Rc;

use aui_protocol::{NativeWorkflow, TaskState};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, pill, status_dot, PillVariant};

use super::common::{dot_state, mark, state_word};

/// Card rhythm.
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 8.0;
const ROW_GAP: f32 = 6.0;
const LIST_GAP: f32 = 2.0;
const ROW_H: f32 = 30.0;
/// Provider marks in the header.
const MARK: f32 = 14.0;

type IntentHandler = Rc<dyn Fn(WorkflowIntent, &mut Window, &mut App)>;

/// What the person asked for on a native workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowIntent {
    /// Retry a failed child; its id and attempt.
    Retry {
        /// The child to retry.
        child_id: String,
        /// Which attempt the retry leaves from.
        attempt: u32,
    },
    /// Skip a failed child; its id and attempt.
    Skip {
        /// The child to skip.
        child_id: String,
        /// Which attempt the skip leaves from.
        attempt: u32,
    },
}

/// The provider-native workflow. Build with [`native_workflow`].
#[derive(IntoElement)]
pub struct NativeWorkflowCard {
    id: ElementId,
    workflow: NativeWorkflow,
    on_intent: Option<IntentHandler>,
}

/// A provider-native subagent tree the conductor watches but does not own.
///
/// The card clones the value; it owns no orchestration logic.
pub fn native_workflow(id: impl Into<ElementId>, workflow: &NativeWorkflow) -> NativeWorkflowCard {
    NativeWorkflowCard { id: id.into(), workflow: workflow.clone(), on_intent: None }
}

impl NativeWorkflowCard {
    /// Retry or Skip was picked on a failed child.
    pub fn on_intent(mut self, f: impl Fn(WorkflowIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

/// The status word's ink: failed reads in danger, needs-you in warning, the
/// rest are muted.
fn status_ink(state: TaskState, p: &Palette) -> gpui::Hsla {
    match state {
        TaskState::Failed => p.danger,
        TaskState::NeedsYou => p.warning,
        _ => p.ink_3,
    }
}

impl RenderOnce for NativeWorkflowCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        let done = self.workflow.children.iter().filter(|c| c.status == TaskState::Accepted).count();
        let total = self.workflow.children.len();
        let mut header = h_flex()
            .w_full()
            .items_center()
            .gap(px(ROW_GAP))
            .py(px(PAD_Y))
            .px(px(PAD_X))
            .child(crate::icons::provider_mark(mark(self.workflow.provider)).size(px(MARK)))
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .ui(scale::FS_13)
                    .medium()
                    .text_color(p.ink)
                    .child(format!("Workflow · {}", self.workflow.name)),
            )
            .child(pill("native"))
            .child(div().flex_1())
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_role(TextRole::MonoSmall)
                    .text_color(p.ink_3)
                    .child(format!("{done} of {total} done")),
            );
        if self.workflow.provisional {
            header = header.child(pill("provisional").variant(PillVariant::Warning));
        }

        let mut rows = v_flex().w_full().gap(px(LIST_GAP));
        for child in &self.workflow.children {
            let row_id: ElementId =
                (id.clone(), SharedString::from(format!("child-{}", child.child_id))).into();
            let (agent, pulse) = dot_state(child.status);
            let mut row = h_flex()
                .id(row_id)
                .w_full()
                .h(px(ROW_H))
                .items_center()
                .gap(px(ROW_GAP))
                .px(px(PAD_X))
                .overflow_hidden()
                .ui(scale::FS_12)
                .child(status_dot((id.clone(), SharedString::from(format!("dot-{}", child.child_id))), agent).pulse(pulse))
                .child(
                    div()
                        .min_w(px(0.0))
                        .truncate()
                        .text_color(p.ink_2)
                        .child(child.label.clone()),
                );
            if let Some(phase) = &child.phase {
                row = row.child(
                    div().flex_none().whitespace_nowrap().text_color(p.ink_3).child(phase.clone()),
                );
            }
            row = row
                .child(div().flex_none().whitespace_nowrap().text_color(status_ink(child.status, &p)).child(state_word(child.status)))
                .child(div().flex_1());
            // Retry and Skip appear only on failed children, and only when
            // the provider allows them.
            if child.status == TaskState::Failed {
                if self.workflow.can_retry {
                    let mut retry = button(
                        (id.clone(), SharedString::from(format!("retry-{}", child.child_id))),
                        "Retry",
                    )
                    .sm()
                    .accessibility_label(format!("Retry {}", child.label));
                    if let Some(on_intent) = self.on_intent.clone() {
                        let child_id = child.child_id.clone();
                        let attempt = child.attempt;
                        retry = retry.on_click(move |_, window, cx| {
                            on_intent(WorkflowIntent::Retry { child_id: child_id.clone(), attempt }, window, cx)
                        });
                    }
                    row = row.child(retry);
                }
                if self.workflow.can_skip {
                    let mut skip = button(
                        (id.clone(), SharedString::from(format!("skip-{}", child.child_id))),
                        "Skip",
                    )
                    .sm()
                    .accessibility_label(format!("Skip {}", child.label));
                    if let Some(on_intent) = self.on_intent.clone() {
                        let child_id = child.child_id.clone();
                        let attempt = child.attempt;
                        skip = skip.on_click(move |_, window, cx| {
                            on_intent(WorkflowIntent::Skip { child_id: child_id.clone(), attempt }, window, cx)
                        });
                    }
                    row = row.child(skip);
                }
            }
            rows = rows.child(row);
        }

        v_flex()
            .id(id)
            .w_full()
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .child(header)
            .child(div().w_full().border_t_1().border_color(p.line).child(rows.into_any_element()))
    }
}
