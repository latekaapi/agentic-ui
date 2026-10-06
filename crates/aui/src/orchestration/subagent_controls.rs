//! The subagent controls: a compact row of small ghost buttons rendered
//! under the existing SubAgent tool card, one per native control the
//! provider offers. Only the true flags draw a button; a provider with no
//! native controls draws an empty row.

use std::rc::Rc;

use aui_protocol::SubagentControls;
use aui_tokens::{scale, AuiStyled};
use gpui::{prelude::*, px, App, ElementId, IntoElement, Window};
use gpui_kit::base::h_flex;

use crate::data::{button, ButtonVariant};

/// The gap between the control buttons.
const ROW_GAP: f32 = 6.0;

type IntentHandler = Rc<dyn Fn(SubagentIntent, &mut Window, &mut App)>;

/// What the person asked for on the subagent controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentIntent {
    /// Send the subagent a plain message.
    Message,
    /// Follow up on a finished subagent.
    FollowUp,
    /// Interrupt the running subagent.
    Interrupt,
    /// Stop the running subagent.
    Stop,
}

/// The subagent controls. Build with [`subagent_controls`].
#[derive(IntoElement)]
pub struct SubagentControlsRow {
    id: ElementId,
    controls: SubagentControls,
    on_intent: Option<IntentHandler>,
}

/// A compact row of small ghost buttons for `controls`, to be rendered under
/// the existing SubAgent tool card.
///
/// The row copies the flags; it owns no orchestration logic.
pub fn subagent_controls(id: impl Into<ElementId>, controls: &SubagentControls) -> SubagentControlsRow {
    SubagentControlsRow { id: id.into(), controls: *controls, on_intent: None }
}

impl SubagentControlsRow {
    /// A control button was picked.
    pub fn on_intent(mut self, f: impl Fn(SubagentIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SubagentControlsRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let mut row = h_flex().id(id.clone()).w_full().items_center().gap(px(ROW_GAP)).ui(scale::FS_12);
        // Each flag draws its own button; false flags draw nothing.
        let flags = [
            (self.controls.message, "Message", SubagentIntent::Message),
            (self.controls.follow_up, "Follow up", SubagentIntent::FollowUp),
            (self.controls.interrupt, "Interrupt", SubagentIntent::Interrupt),
            (self.controls.stop, "Stop", SubagentIntent::Stop),
        ];
        for (enabled, label, intent) in flags {
            if !enabled {
                continue;
            }
            let mut b = button((id.clone(), label), label).sm().variant(ButtonVariant::Ghost);
            if let Some(on_intent) = self.on_intent.clone() {
                b = b.on_click(move |_, window, cx| on_intent(intent, window, cx));
            }
            row = row.child(b);
        }
        row
    }
}
