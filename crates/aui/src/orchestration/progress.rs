//! The run-progress block: the stage dots and counts in the header, then one
//! row per live task with its state dot, provider mark, activity and elapsed
//! time. Needs-you rows sort first; verifying rows show their rungs inline.
//!
//! The header and every row are links: the header opens the run view, a row
//! opens its worker session. There are no buttons. Dots pulse and the
//! reconciling spinner turns only while the run is moving; a paused run or
//! one waiting on quota settles to zero animation frames.

use std::rc::Rc;

use aui_motion::{tint_fade, Tween};
use aui_protocol::{Provider as WireProvider, RunProgress, RungState, TaskLine, TaskState};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{record_ax_label, spinner, status_dot};
use crate::icons::{provider_mark, Provider as IconProvider};
use crate::util::{interaction_flags, TrackInteraction};

/// The block's outer padding and row rhythm.
const PAD_X: f32 = 12.0;
const HEADER_PAD_Y: f32 = 8.0;
const ROW_H: f32 = 30.0;
const ROW_GAP: f32 = 6.0;
const LIST_GAP: f32 = 2.0;
/// A stage dot is a 14x4 pill; the dots are 3 px apart.
const DOT_W: f32 = 14.0;
const DOT_H: f32 = 4.0;
const DOT_GAP: f32 = 3.0;
/// Provider marks in the rows.
const MARK: f32 = 14.0;

type IntentHandler = Rc<dyn Fn(ProgressIntent, &mut Window, &mut App)>;

/// What the person asked for on a progress block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressIntent {
    /// Open one worker session; the task id.
    OpenWorker(String),
    /// Open the run view.
    OpenRunView,
}

/// The run-progress block. Build with [`progress_block`].
#[derive(IntoElement)]
pub struct ProgressBlock {
    id: ElementId,
    progress: RunProgress,
    on_intent: Option<IntentHandler>,
}

/// A progress block for `progress`.
///
/// The block clones the value; it owns no orchestration logic.
pub fn progress_block(id: impl Into<ElementId>, progress: &RunProgress) -> ProgressBlock {
    ProgressBlock { id: id.into(), progress: progress.clone(), on_intent: None }
}

impl ProgressBlock {
    /// The header opens the run view, a row opens its worker session.
    pub fn on_intent(mut self, f: impl Fn(ProgressIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

/// Maps the wire provider onto its coloured mark.
fn mark(provider: WireProvider) -> IconProvider {
    match provider {
        WireProvider::Claude => IconProvider::Claude,
        WireProvider::Codex => IconProvider::Codex,
        WireProvider::Grok => IconProvider::Grok,
        WireProvider::Gemini => IconProvider::Gemini,
        WireProvider::Pi => IconProvider::Pi,
        WireProvider::Cursor => IconProvider::Cursor,
        WireProvider::Muse => IconProvider::Muse,
    }
}

/// The plain state word a task row carries in its accessible label.
fn state_word(state: TaskState) -> &'static str {
    match state {
        TaskState::Queued => "queued",
        TaskState::Working => "working",
        TaskState::Verifying => "verifying",
        TaskState::Accepted => "accepted",
        TaskState::NeedsYou => "needs you",
        TaskState::Failed => "failed",
        TaskState::Reconciling => "reconciling",
        TaskState::Paused => "paused",
    }
}

/// The dot a task row draws: the agent state and whether it is in motion.
/// A paused run or one waiting on quota never pulses.
fn dot_state(line: &TaskLine, frozen: bool) -> (AgentState, bool) {
    match line.state {
        TaskState::Working | TaskState::Verifying | TaskState::Reconciling => {
            (AgentState::Running, !frozen)
        }
        TaskState::NeedsYou => (AgentState::Waiting, false),
        TaskState::Accepted => (AgentState::Done, false),
        TaskState::Failed => (AgentState::Failed, false),
        TaskState::Queued | TaskState::Paused => (AgentState::Idle, false),
    }
}

/// `Implement · auth call, working, open worker session` — every row names
/// its own state, so the list reads without a live region.
fn row_label(line: &TaskLine) -> String {
    format!("{}, {}, open worker session", line.title, state_word(line.state))
}

/// One verification rung as a small mono chip: `scope ✓`, `tests …`.
fn rung_chip(rung: &aui_protocol::Rung, p: &aui_tokens::Palette) -> AnyElement {
    let (suffix, colour) = match rung.state {
        RungState::Pass => (" ✓", p.success),
        RungState::Running => (" …", p.accent),
        RungState::Fail => (" ✗", p.danger),
        RungState::Pending | RungState::Unverified => ("", p.ink_4),
    };
    div()
        .flex_none()
        .text_role(TextRole::MonoSmall)
        .text_color(colour)
        .child(format!("{}{suffix}", rung.name))
        .into_any_element()
}

impl RenderOnce for ProgressBlock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let frozen = self.progress.paused || self.progress.waiting_reason.is_some();

        let group_label = format!("Run progress: {}", self.progress.title);
        record_ax_label(&group_label);

        // The header line: title plus the counts the protocol carries.
        let header_text: String = if self.progress.paused {
            format!("{} · Paused · {}", self.progress.title, self.progress.elapsed)
        } else if let Some(reason) = &self.progress.waiting_reason {
            format!("{} · {reason} · {}", self.progress.title, self.progress.elapsed)
        } else {
            let mut parts =
                vec![format!("{} {}/{}", self.progress.stage_name, self.progress.stage_index + 1, self.progress.stage_count)];
            if self.progress.working > 0 {
                parts.push(format!("{} working", self.progress.working));
            }
            if self.progress.verifying > 0 {
                parts.push(format!("{} verifying", self.progress.verifying));
            }
            if self.progress.needs_you > 0 {
                parts.push(format!("{} needs you", self.progress.needs_you));
            }
            parts.push(self.progress.elapsed.clone());
            format!("{} · {}", self.progress.title, parts.join(" · "))
        };
        let header_ink = if self.progress.waiting_reason.is_some() {
            p.warning
        } else if self.progress.paused {
            p.ink_3
        } else {
            p.ink_2
        };

        // Stage dots: done in success, the current stage in accent, the rest
        // in line-strong.
        let mut dots = h_flex().flex_none().items_center().gap(px(DOT_GAP));
        for i in 0..self.progress.stage_count {
            let colour = if i < self.progress.stage_index {
                p.success
            } else if i == self.progress.stage_index {
                p.accent
            } else {
                p.line_strong
            };
            dots = dots.child(div().w(px(DOT_W)).h(px(DOT_H)).rounded_full().bg(colour));
        }

        let header_id: ElementId = (id.clone(), "header").into();
        let (header_state, header_flags) = interaction_flags(header_id.clone(), window, cx);
        let header_bg = tint_fade((header_id.clone(), "bg"), header_flags.hovered, p.surface_3, Tween::FAST, window, cx);
        record_ax_label("Open run view");
        let mut header = h_flex()
            .id(header_id.clone())
            .w_full()
            .items_center()
            .gap(px(ROW_GAP))
            .py(px(HEADER_PAD_Y))
            .px(px(PAD_X))
            .bg(header_bg)
            .cursor_pointer()
            .track_interaction(&header_state)
            .role(gpui::Role::Button)
            .aria_label("Open run view")
            .child(dots)
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .ui(scale::FS_12)
                    .text_color(header_ink)
                    .child(SharedString::from(header_text)),
            )
            .child(div().flex_none().ui(scale::FS_12).text_color(p.ink_3).child("›"));
        if let Some(on_intent) = self.on_intent.clone() {
            header = header.on_click(move |_, window, cx| on_intent(ProgressIntent::OpenRunView, window, cx));
        }

        // Needs-you rows sort first, in the warning ink.
        let mut lines = self.progress.lines.clone();
        lines.sort_by_key(|line| line.state != TaskState::NeedsYou);

        let mut rows = v_flex().w_full().gap(px(LIST_GAP));
        for line in &lines {
            let label = row_label(line);
            record_ax_label(&label);
            let row_id: ElementId =
                (id.clone(), SharedString::from(format!("task-{}", line.task_id))).into();
            let (row_state, row_flags) = interaction_flags(row_id.clone(), window, cx);
            let row_bg = tint_fade((row_id.clone(), "bg"), row_flags.hovered, p.surface_3, Tween::FAST, window, cx);
            let (agent, pulse) = dot_state(line, frozen);
            let needs_you = line.state == TaskState::NeedsYou;
            let title_ink = if needs_you { p.warning } else { p.ink_2 };

            let mut row = h_flex()
                .id(row_id.clone())
                .w_full()
                .h(px(ROW_H))
                .items_center()
                .gap(px(ROW_GAP))
                .px(px(PAD_X))
                .bg(row_bg)
                .overflow_hidden()
                .ui(scale::FS_12)
                .cursor_pointer()
                .track_interaction(&row_state)
                .role(gpui::Role::Button)
                .aria_label(label);
            if line.state == TaskState::Reconciling {
                row = row.child(spinner((row_id.clone(), "spin")));
            } else {
                row = row.child(status_dot((row_id.clone(), "dot"), agent).pulse(pulse));
            }
            row = row
                .child(provider_mark(mark(line.provider)).size(px(MARK)))
                .child(
                    div()
                        .min_w(px(0.0))
                        .truncate()
                        .text_color(title_ink)
                        .child(line.title.clone()),
                );
            if line.state == TaskState::Verifying && !line.rungs.is_empty() {
                let mut rungs = h_flex().flex_none().items_center().gap(px(ROW_GAP));
                for rung in &line.rungs {
                    rungs = rungs.child(rung_chip(rung, &p));
                }
                row = row.child(rungs);
            }
            row = row
                .child(div().min_w(px(0.0)).truncate().text_color(p.ink_3).child(line.activity.clone()))
                .child(div().flex_1())
                .child(
                    div()
                        .flex_none()
                        .text_role(TextRole::MonoSmall)
                        .text_color(p.ink_3)
                        .child(line.elapsed.clone()),
                );
            if let Some(on_intent) = self.on_intent.clone() {
                let task_id = line.task_id.clone();
                row = row.on_click(move |_, window, cx| {
                    on_intent(ProgressIntent::OpenWorker(task_id.clone()), window, cx)
                });
            }
            rows = rows.child(row);
        }

        v_flex()
            .id(id)
            .w_full()
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .overflow_hidden()
            .role(gpui::Role::Group)
            .aria_label(group_label)
            .child(header)
            .child(
                div()
                    .w_full()
                    .border_t_1()
                    .border_color(p.line)
                    .child(rows.into_any_element()),
            )
    }
}
