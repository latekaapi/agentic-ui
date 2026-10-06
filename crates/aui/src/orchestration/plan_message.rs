//! The run-plan message: the conductor's prose intro, its stage chips, the
//! acceptance criteria, its assumptions, and the footer line.
//!
//! A proposed plan invites a typed `go`; a started plan carries its start
//! time. There are no buttons: the person answers by typing.

use aui_protocol::{CheckMethod, Provider as WireProvider, RunPlan};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::icons::{icon, provider_mark, IconName, Provider as IconProvider};

/// Horizontal rhythm of the card.
const PAD_X: f32 = 12.0;
/// Vertical gaps between the intro, stages, criteria, assumptions, footer.
const SECTION_GAP: f32 = 8.0;
/// The gap inside a stage chip and between the chip row's items.
const CHIP_GAP: f32 = 4.0;
const CHIP_PAD_X: f32 = 8.0;
const CHIP_PAD_Y: f32 = 4.0;
/// Provider marks and the arrows between the stage chips.
const MARK: f32 = 14.0;
const ARROW: f32 = 12.0;

/// The run-plan message. Build with [`plan_message`].
#[derive(IntoElement)]
pub struct PlanMessageCard {
    id: ElementId,
    plan: RunPlan,
}

/// A run plan as the conductor proposed or started it.
///
/// The card clones the plan; it owns no orchestration logic.
pub fn plan_message(id: impl Into<ElementId>, plan: &RunPlan) -> PlanMessageCard {
    PlanMessageCard { id: id.into(), plan: plan.clone() }
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

/// The plain word a criterion's check method carries.
fn method_word(method: CheckMethod) -> &'static str {
    match method {
        CheckMethod::Test => "test",
        CheckMethod::Browser => "browser",
        CheckMethod::Review => "review",
        CheckMethod::User => "you",
    }
}

impl RenderOnce for PlanMessageCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;

        let mut stages = h_flex().w_full().items_center().gap(px(CHIP_GAP));
        for (i, stage) in self.plan.stages.iter().enumerate() {
            if i > 0 {
                stages = stages.child(icon(IconName::ArrowRight).size(px(ARROW)).color(p.ink_4));
            }
            let mut chip = h_flex()
                .flex_none()
                .items_center()
                .gap(px(CHIP_GAP))
                .px(px(CHIP_PAD_X))
                .py(px(CHIP_PAD_Y))
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(p.line)
                .bg(p.surface_1)
                .ui(scale::FS_12)
                .text_color(p.ink_2);
            if let Some(provider) = stage.provider {
                chip = chip.child(provider_mark(mark(provider)).size(px(MARK)));
            }
            chip = chip.child(div().whitespace_nowrap().child(stage.name.clone()));
            stages = stages.child(chip);
        }

        let mut criteria = v_flex().w_full().gap(px(2.0)).child(
            div().text_role(TextRole::Caps).text_color(p.ink_3).child("Criteria"),
        );
        for criterion in &self.plan.criteria {
            criteria = criteria.child(
                h_flex()
                    .w_full()
                    .gap(px(CHIP_GAP))
                    .ui(scale::FS_12)
                    .child(div().flex_none().text_color(p.ink_4).child("·"))
                    .child(div().min_w(px(0.0)).truncate().text_color(p.ink_2).child(criterion.text.clone()))
                    .child(
                        div()
                            .flex_none()
                            .text_color(p.ink_3)
                            .child(format!("· {}", method_word(criterion.method))),
                    ),
            );
        }

        let assumptions =
            div().w_full().ui(scale::FS_12).text_color(p.ink_3).child(format!(
                "Assumptions: {}",
                self.plan.assumptions.join("; ")
            ));

        let footer: String = match &self.plan.started_at {
            None => "Reply \"go\" to start, or tell me what to change.".into(),
            Some(at) => format!("Started {at}"),
        };

        v_flex()
            .id(self.id)
            .w_full()
            .px(px(PAD_X))
            .gap(px(SECTION_GAP))
            .child(div().w_full().text_role(TextRole::Body).text_color(p.ink).child(self.plan.intro.clone()))
            .child(stages)
            .child(criteria)
            .child(assumptions)
            .child(div().w_full().text_role(TextRole::Meta).text_color(p.ink_3).child(footer))
    }
}
