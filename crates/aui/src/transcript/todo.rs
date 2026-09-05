//! Card 36: the todo list — the agent's task list as a collapsible card with
//! a progress bar and one row per task, whose mark morphs from a pending ring
//! to a pulsing accent dot to a success check.

use std::time::Duration;

use aui_motion::{looping, spring, spring_phase, Loop, SpringKind};
use aui_protocol::{TodoItem, TodoState};
use aui_tokens::{ActiveAui, AuiStyled, Easing, Palette, TextRole};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, StrikethroughStyle, StyledText, TextRun, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::icons::{icon, IconName};
use crate::transcript::transcript_card;
use crate::util::ClickHandler;

/// `.todo .hd{height:32px;font-size:12.5px}` with a 14 px glyph.
const HEADER_H: f32 = 32.0;
const HEADER_GLYPH: f32 = 14.0;
/// `.todo .bar{height:3px}`; the fill's width travels on the layout spring.
const BAR_H: f32 = 3.0;
/// `.ti{height:30px;padding:0 12px;gap:10px;font-size:12.5px}`.
const ROW_H: f32 = 30.0;
const ROW_PAD_X: f32 = 12.0;
const ROW_GAP: f32 = 10.0;
const ROW_TEXT: f32 = 12.5;
/// `.ti .m{width:15px;height:15px;border:1.5px}`.
const MARK: f32 = 15.0;
const MARK_BORDER: f32 = 1.5;
/// `.ti.done .m svg{width:9px;height:9px}`.
const CHECK: f32 = 9.0;
/// `.ti.run .m{box-shadow:0 0 0 3px var(--accent-soft)}` and its 6 px dot,
/// which pulses to 60 % and back over 1.2 s.
const RING: f32 = 3.0;
const RUN_DOT: f32 = 6.0;
const PULSE_TO: f32 = 0.6;
const PULSE_HALF_CYCLE: Duration = Duration::from_millis(600);
/// The strikethrough on a finished label.
const STRIKE_THICKNESS: f32 = 1.0;

/// The todo list. Build with [`todo_list`].
#[derive(IntoElement)]
pub struct TodoList {
    id: ElementId,
    items: Vec<TodoItem>,
    title: SharedString,
    open: bool,
    on_toggle: Option<ClickHandler>,
}

/// The agent's task list over `items`.
pub fn todo_list(id: impl Into<ElementId>, items: Vec<TodoItem>) -> TodoList {
    TodoList { id: id.into(), items, title: "Tasks".into(), open: true, on_toggle: None }
}

impl TodoList {
    /// Overrides the header label (`Tasks`).
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Whether the rows are shown.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Header click: collapse or expand the list.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

/// `m:ss`, the elapsed format the card shows.
fn elapsed_label(ms: u64) -> String {
    let total = ms / 1000;
    format!("{}:{:02}", total / 60, total % 60)
}

/// The 15 px mark: a pending ring that fills with success as the task
/// finishes (swap spring) and carries a pulsing accent dot while running.
fn todo_mark(id: ElementId, state: TodoState, p: &Palette, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let done = spring_phase((id.clone(), "done"), state == TodoState::Done, SpringKind::Swap, window, cx).clamp(0.0, 1.0);
    let running = state == TodoState::Running;
    let pulse = if running {
        looping((id, "pulse"), Loop::eased(PULSE_HALF_CYCLE, Easing::INOUT).alternate().resting(0.0), window, cx)
    } else {
        0.0
    };
    let dot = RUN_DOT * (1.0 - (1.0 - PULSE_TO) * pulse);
    div()
        .relative()
        .flex_none()
        .size(px(MARK))
        .rounded_full()
        .border(px(MARK_BORDER))
        .border_color(if running { p.accent } else { p.line_strong })
        .flex()
        .items_center()
        .justify_center()
        // The 3 px accent-soft ring is a box-shadow spread in the CSS; gpui
        // draws it as a larger circle behind the mark.
        .when(running, |d| {
            d.child(div().absolute().size(px(MARK + 2.0 * RING)).rounded_full().bg(p.accent_soft))
                .child(div().relative().size(px(dot)).rounded_full().bg(p.accent))
        })
        // The success fill crossfades in over the pending ring.
        .when(done > 0.0, |d| {
            d.child(
                div()
                    .absolute()
                    .size(px(MARK))
                    .rounded_full()
                    .border(px(MARK_BORDER))
                    .border_color(p.success)
                    .bg(p.success)
                    .opacity(done)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(IconName::CheckBold).size(px(CHECK)).color(gpui::white())),
            )
        })
}

impl RenderOnce for TodoList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let total = self.items.len();
        let done = self.items.iter().filter(|i| i.state == TodoState::Done).count();
        let fraction = if total == 0 { 0.0 } else { done as f32 / total as f32 };
        let width = spring((id.clone(), "bar"), fraction, SpringKind::Layout, window, cx).clamp(0.0, 1.0);

        let mut card = transcript_card(id.clone(), self.open)
            .header_height(px(HEADER_H))
            .header(icon(IconName::List).size(px(HEADER_GLYPH)).color(p.ink_3))
            .header(div().medium().child(self.title.clone()))
            .header(div().text_role(TextRole::MonoSmall).text_color(p.ink_3).child(format!("{done} / {total}")))
            .header(div().flex_1());

        let mut body = v_flex()
            .w_full()
            .child(div().w_full().h(px(BAR_H)).bg(p.surface_3).child(div().h_full().w(relative(width)).bg(p.ink_3)));
        for (index, item) in self.items.iter().enumerate() {
            let label: SharedString = item.label.clone().into();
            let text = match item.state {
                TodoState::Done => StyledText::new(label.clone())
                    .with_runs(vec![TextRun {
                        len: label.len(),
                        font: gpui::font(aui_tokens::scale::FONT_UI),
                        color: p.ink_3,
                        background_color: None,
                        underline: None,
                        strikethrough: Some(StrikethroughStyle { thickness: px(STRIKE_THICKNESS), color: Some(p.ink_4) }),
                    }])
                    .into_any_element(),
                _ => label.clone().into_any_element(),
            };
            body = body.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .h(px(ROW_H))
                    .flex_none()
                    .gap(px(ROW_GAP))
                    .px(px(ROW_PAD_X))
                    .border_t_1()
                    .border_color(p.line)
                    .ui(ROW_TEXT)
                    .child(todo_mark((id.clone(), SharedString::from(format!("mark-{index}"))).into(), item.state, &p, window, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .truncate()
                            .text_color(p.ink)
                            .when(item.state == TodoState::Running, |d| d.medium())
                            .child(text),
                    )
                    .children(item.elapsed_ms.map(|ms| {
                        // `.ti.done span` and `.ti.run span` also catch `.t`,
                        // so a finished row's elapsed is struck through too and
                        // a running one takes the brighter ink.
                        let elapsed: SharedString = elapsed_label(ms).into();
                        let mut font = gpui::font(aui_tokens::scale::FONT_MONO);
                        font.weight = gpui::FontWeight::MEDIUM;
                        let run = TextRun {
                            len: elapsed.len(),
                            font,
                            color: if item.state == TodoState::Running { p.ink } else { p.ink_3 },
                            background_color: None,
                            underline: None,
                            strikethrough: (item.state == TodoState::Done).then_some(StrikethroughStyle { thickness: px(STRIKE_THICKNESS), color: Some(p.ink_4) }),
                        };
                        div().flex_none().text_role(TextRole::MonoSmall).child(StyledText::new(elapsed).with_runs(vec![run]))
                    })),
            );
        }
        // `.todo .bar` sits flush under the header; each row draws its own
        // hairline, so the card frame must not add one.
        card = card.body_border(false).body(body);
        if let Some(on_toggle) = self.on_toggle {
            card = card.on_toggle(move |e, w, cx| on_toggle(e, w, cx));
        }
        card
    }
}

#[cfg(test)]
mod tests {
    use super::elapsed_label;

    #[test]
    fn formats_elapsed_as_minutes_and_seconds() {
        assert_eq!(elapsed_label(12_000), "0:12");
        assert_eq!(elapsed_label(64_000), "1:04");
    }
}
