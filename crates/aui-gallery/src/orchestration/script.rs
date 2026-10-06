//! Timed behaviour for the orchestration mock: auto-advance and retries.
//!
//! Every entry point here is the handler a UI intent calls, so the
//! `AUI_GALLERY_STEPS` harness in [`super::view`] drives the mock through
//! exactly the code path a click does.

use std::time::Duration;

use aui::protocol::{Block, TaskState};
use gpui::{Context, Window};

use super::view::{OrchestrationMock, PLAY_TICK};

/// How long a retried workflow child runs before it lands done.
pub const RETRY_TIME: Duration = Duration::from_millis(1200);

impl OrchestrationMock {
    /// Starts auto-advance: one step every [`PLAY_TICK`] until the last step.
    /// The generation guard stops a stale loop after a manual step change.
    pub fn play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playing = true;
        self.play_run += 1;
        let run = self.play_run;
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            loop {
                cx.background_executor().timer(PLAY_TICK).await;
                let cont = this
                    .update(cx, |this, cx| {
                        if this.play_run != run || !this.playing {
                            return false;
                        }
                        if !this.advance(cx) {
                            this.playing = false;
                            return false;
                        }
                        true
                    })
                    .unwrap_or(false);
                if !cont {
                    return;
                }
            }
        });
        self.tasks.push(task);
        cx.notify();
    }

    /// Pauses auto-advance. The loop sees the cleared flag and stops.
    pub fn pause(&mut self, cx: &mut Context<Self>) {
        self.playing = false;
        self.play_run += 1;
        cx.notify();
    }

    /// Toggles play / pause.
    pub fn toggle_play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.playing {
            self.pause(cx);
        } else {
            self.play(window, cx);
        }
    }

    /// `WorkflowIntent::Retry`: the failed child runs, then lands done.
    pub fn retry_child(&mut self, child_id: String, window: &mut Window, cx: &mut Context<Self>) {
        let step = self.current_mut();
        let mut found = false;
        for item in &mut step.items {
            if let super::model::CentreItem::Block(Block::NativeWorkflow(workflow)) = item {
                for child in &mut workflow.children {
                    if child.child_id == child_id {
                        child.status = TaskState::Working;
                        child.attempt += 1;
                        found = true;
                    }
                }
            }
        }
        if !found {
            return;
        }
        cx.notify();
        self.retry_run += 1;
        let run = self.retry_run;
        let scenario = self.scenario;
        let index = self.index;
        let child_id = child_id.clone();
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            cx.background_executor().timer(RETRY_TIME).await;
            let _ = this.update(cx, |this, cx| {
                if this.retry_run != run || this.scenario != scenario || this.index != index {
                    return;
                }
                let step = this.current_mut();
                for item in &mut step.items {
                    if let super::model::CentreItem::Block(Block::NativeWorkflow(workflow)) = item {
                        for child in &mut workflow.children {
                            if child.child_id == child_id {
                                child.status = TaskState::Accepted;
                            }
                        }
                    }
                }
                cx.notify();
            });
        });
        self.tasks.push(task);
    }
}
