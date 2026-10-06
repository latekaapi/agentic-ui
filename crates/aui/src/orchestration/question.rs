//! The run question: a warning-bordered card with the source label, the
//! question, and an action row of one-tap answers. The recommended option is
//! the primary button. Once answered the card collapses to one muted line.

use std::rc::Rc;

use aui_protocol::RunQuestion;
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::button;

type IntentHandler = Rc<dyn Fn(RunQuestionIntent, &mut Window, &mut App)>;

/// What the person asked for on a run question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunQuestionIntent {
    /// One offered answer was picked; the option id.
    Answer(String),
}

/// The run question. Build with [`run_question`].
#[derive(IntoElement)]
pub struct RunQuestionCard {
    id: ElementId,
    question: RunQuestion,
    on_intent: Option<IntentHandler>,
}

/// A run question: open while unanswered, one muted line once answered.
///
/// The card clones the value; the host keeps the answer.
pub fn run_question(id: impl Into<ElementId>, question: &RunQuestion) -> RunQuestionCard {
    RunQuestionCard { id: id.into(), question: question.clone(), on_intent: None }
}

impl RunQuestionCard {
    /// An option button was picked; the option id is echoed back.
    pub fn on_intent(mut self, f: impl Fn(RunQuestionIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for RunQuestionCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        // Answered: one muted line, no controls left.
        if let Some(answer) = &self.question.answer {
            let mut line = format!("Question from the run · You: {answer}");
            if let Some(at) = &self.question.answered_at {
                line.push_str(&format!(" · {at}"));
            }
            return div()
                .id(id)
                .w_full()
                .text_role(TextRole::Meta)
                .text_color(p.ink_3)
                .child(line)
                .into_any_element();
        }

        let mut actions = h_flex()
            .w_full()
            .items_center()
            .gap(px(scale::SP_2))
            .py(px(10.0))
            .px(px(12.0))
            .border_t_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .text_role(TextRole::Meta)
                    .text_color(p.ink_3)
                    .child(self.question.hint.clone()),
            )
            .child(div().flex_1());
        for option in &self.question.options {
            let mut b = button((id.clone(), option.id.as_str()), option.label.clone()).sm();
            if option.recommended {
                b = b.primary();
            }
            b = b.accessibility_label(option.label.clone());
            if let Some(on_intent) = self.on_intent.clone() {
                let option_id = option.id.clone();
                b = b.on_click(move |_, window, cx| {
                    on_intent(RunQuestionIntent::Answer(option_id.clone()), window, cx)
                });
            }
            actions = actions.child(b);
        }

        v_flex()
            .id(id)
            .w_full()
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.attention_border(p.warning))
            .bg(p.surface_1)
            .overflow_hidden()
            .child(
                v_flex()
                    .w_full()
                    .gap(px(scale::SP_2))
                    .py(px(10.0))
                    .px(px(12.0))
                    .child(
                        div()
                            .w_full()
                            .text_role(TextRole::Caps)
                            .text_color(p.ink_3)
                            .child(self.question.source_label.clone()),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_role(TextRole::Body)
                            .text_color(p.ink)
                            .child(self.question.text.clone()),
                    ),
            )
            .child(actions)
            .into_any_element()
    }
}
