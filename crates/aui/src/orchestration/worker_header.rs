//! The worker header: one 30 px line naming the stage and attempt, the
//! provider mark and model, the branch, and a right-aligned link back to the
//! run. The link is a link only while a handler is set; otherwise it is
//! plain text.

use std::rc::Rc;

use aui_protocol::WorkerHeader;
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, Window};
use gpui_kit::base::h_flex;

use crate::data::{pill, record_ax_label};
use crate::icons::provider_mark;

use super::common::mark;

/// The single line's height.
const ROW_H: f32 = 30.0;
const ROW_GAP: f32 = 6.0;
/// Provider marks in the line.
const MARK: f32 = 14.0;

type IntentHandler = Rc<dyn Fn(WorkerHeaderIntent, &mut Window, &mut App)>;

/// What the person asked for on a worker header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerHeaderIntent {
    /// Open the run the worker belongs to.
    OpenRun,
}

/// The header of a worker view. Build with [`worker_header`].
#[derive(IntoElement)]
pub struct WorkerHeaderLine {
    id: ElementId,
    header: WorkerHeader,
    on_intent: Option<IntentHandler>,
}

/// A worker view header for `header`.
///
/// The line clones the value; it owns no orchestration logic.
pub fn worker_header(id: impl Into<ElementId>, header: &WorkerHeader) -> WorkerHeaderLine {
    WorkerHeaderLine { id: id.into(), header: header.clone(), on_intent: None }
}

impl WorkerHeaderLine {
    /// The run link was picked.
    pub fn on_intent(mut self, f: impl Fn(WorkerHeaderIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for WorkerHeaderLine {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        // The trailing run link: a link only while a handler is set.
        let ax = format!("Open the run \"{}\"", self.header.run_title);
        let mut link = div()
            .id((id.clone(), "run"))
            .flex_none()
            .whitespace_nowrap()
            .ui(scale::FS_12)
            .text_color(p.accent_ink)
            .child(format!("for \"{}\" ↗", self.header.run_title));
        if let Some(on_intent) = self.on_intent.clone() {
            record_ax_label(&ax);
            link = link
                .cursor_pointer()
                .role(gpui::Role::Link)
                .aria_label(ax)
                .on_click(move |_, window, cx| on_intent(WorkerHeaderIntent::OpenRun, window, cx));
        }

        h_flex()
            .id(id)
            .w_full()
            .h(px(ROW_H))
            .items_center()
            .gap(px(ROW_GAP))
            .overflow_hidden()
            .ui(scale::FS_12)
            .child(pill("Worker"))
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_color(p.ink)
                    .medium()
                    .child(format!("{} · attempt {}", self.header.stage, self.header.attempt)),
            )
            .child(provider_mark(mark(self.header.provider)).size(px(MARK)))
            .child(div().flex_none().whitespace_nowrap().text_color(p.ink_3).child(self.header.model.clone()))
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .text_role(TextRole::Mono)
                    .text_color(p.ink_3)
                    .child(format!("branch {}", self.header.branch)),
            )
            .child(div().flex_1())
            .child(link)
    }
}
