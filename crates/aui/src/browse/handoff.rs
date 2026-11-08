//! The handoff picker (harness R3 board, 3a): "Continue in a session" from the
//! agent card. A search row, recent sessions (provider mark, title, project ·
//! when), then "New session in" one row per project, and a quiet footnote.
//!
//! Stateless like the Field: the host owns the query input, the rows and the
//! selection, and gets [`HandoffIntent`]s back. It draws on the overlay
//! surface and sets its own text colour, so it is safe in a float window with
//! no themed root.

use std::rc::Rc;

use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, relative, App, ElementId, Entity, IntoElement, SharedString, Window};
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::input::Textarea;

use crate::icons::{icon, provider_mark, IconName, Provider};

/// The picker's width.
pub const HANDOFF_W: f32 = 320.0;
const Q_H: f32 = 38.0;
const PAD_X: f32 = 12.0;
const CAPS_H: f32 = 26.0;
const ITEM_H: f32 = 34.0;
const ITEM_INSET: f32 = 4.0;
const FOOT_LH: f32 = 17.0;
const BODY_TEXT: f32 = 13.5;
const META_TEXT: f32 = 12.5;
const FOOT_TEXT: f32 = 12.0;

/// One row of the picker.
#[derive(Debug, Clone, PartialEq)]
pub struct HandoffRow {
    /// Stable identity, handed back on choose.
    pub id: SharedString,
    /// The session title, or the project's name.
    pub title: SharedString,
    /// `harness · 2 h`, or the project's default provider name.
    pub meta: SharedString,
    /// `Some` for a session (its provider's mark); `None` for a project (a folder).
    pub provider: Option<Provider>,
}

/// What the picker asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum HandoffIntent {
    /// Hand off to this row, by id.
    Choose(SharedString),
    /// The pointer is over this row, by index across both sections.
    Hover(usize),
    /// Close without handing off.
    Close,
}

type Handler = Rc<dyn Fn(HandoffIntent, &mut Window, &mut App)>;

/// The picker. Build with [`handoff_picker`].
#[derive(IntoElement)]
pub struct HandoffPicker {
    id: ElementId,
    input: Entity<TextareaState>,
    sessions: Vec<HandoffRow>,
    projects: Vec<HandoffRow>,
    selected: usize,
    note: SharedString,
    on_intent: Option<Handler>,
}

/// The picker over `sessions` then `projects`; `selected` indexes both in order.
pub fn handoff_picker(
    id: impl Into<ElementId>,
    input: &Entity<TextareaState>,
    sessions: Vec<HandoffRow>,
    projects: Vec<HandoffRow>,
    selected: usize,
) -> HandoffPicker {
    HandoffPicker {
        id: id.into(),
        input: input.clone(),
        sessions,
        projects,
        selected,
        note: "Takes a summary of this chat and the pages it used. Its tabs move to that session's task folder. This chat stays here.".into(),
        on_intent: None,
    }
}

/// The picker's height for this many rows, so a host can size its window
/// exactly. The footnote is three lines at [`HANDOFF_W`].
pub fn handoff_height(sessions: usize, projects: usize) -> f32 {
    let section = |n: usize| if n == 0 { 0.0 } else { CAPS_H + n as f32 * ITEM_H };
    let empty = if sessions + projects == 0 { ITEM_H } else { 0.0 };
    Q_H + section(sessions) + section(projects) + empty + 4.0 + 8.0 + FOOT_LH * 3.0 + 10.0
}

impl HandoffPicker {
    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(HandoffIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }

    /// Replaces the footnote.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = note.into();
        self
    }
}

impl RenderOnce for HandoffPicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let query = h_flex()
            .h(px(Q_H))
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .px(px(PAD_X))
            .border_b_1()
            .border_color(p.line)
            .child(icon(IconName::Search).size(px(13.0)).color(p.ink_3))
            .child(
                crate::a11y_text::a11y_text_field((id.clone(), "query"), "Find a session or project", &self.input, false, cx)
                    .flex_1()
                    .min_w(px(0.0))
                    .ui(BODY_TEXT)
                    .text_color(p.ink)
                    .child(Textarea::new(&self.input).appearance(false).bordered(false).text_size(aui_tokens::scaled(BODY_TEXT))),
            );
        let mut body = v_flex().w_full().flex_none();
        let mut index = 0usize;
        for (title, rows) in [("Continue in a session", &self.sessions), ("New session in", &self.projects)] {
            if rows.is_empty() {
                continue;
            }
            body = body.child(
                div()
                    .h(px(CAPS_H))
                    .flex_none()
                    .px(px(PAD_X))
                    .pt(px(8.0))
                    .text_role(TextRole::Caps)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(title),
            );
            for row in rows.iter() {
                let i = index;
                index += 1;
                let lead = match row.provider {
                    Some(provider) => provider_mark(provider).into_any_element(),
                    None => icon(IconName::Folder).size(px(14.0)).color(p.ink_2).into_any_element(),
                };
                let choose = self.on_intent.clone();
                let hover = self.on_intent.clone();
                let row_id = row.id.clone();
                body = body.child(
                    h_flex()
                        .id((id.clone(), SharedString::from(format!("row-{i}"))))
                        .h(px(ITEM_H))
                        .flex_none()
                        .mx(px(ITEM_INSET))
                        .px(px(PAD_X - ITEM_INSET))
                        .gap(px(9.0))
                        .items_center()
                        .rounded(px(scale::R_MD))
                        .when(i == self.selected, |r| r.bg(p.hover_overlay()))
                        .cursor_pointer()
                        .role(gpui::Role::Button)
                        .aria_label(SharedString::from(format!("{} · {}", row.title, row.meta)))
                        .on_hover(move |hovered, w, cx| {
                            if *hovered {
                                if let Some(h) = &hover {
                                    h(HandoffIntent::Hover(i), w, cx);
                                }
                            }
                        })
                        .on_click(move |_, w, cx| {
                            if let Some(h) = &choose {
                                h(HandoffIntent::Choose(row_id.clone()), w, cx);
                            }
                        })
                        .child(lead)
                        .child(div().flex_1().min_w(px(0.0)).truncate().ui(BODY_TEXT).text_color(p.ink).child(row.title.clone()))
                        .child(div().flex_none().ui(META_TEXT).text_color(p.ink_3).child(row.meta.clone())),
                );
            }
        }
        if index == 0 {
            body = body.child(
                div().h(px(ITEM_H)).flex_none().px(px(PAD_X)).flex().items_center().ui(BODY_TEXT).text_color(p.ink_3).child("Nothing matches"),
            );
        }
        let close = self.on_intent.clone();
        v_flex()
            .id(id.clone())
            .w(px(HANDOFF_W))
            .rounded(px(scale::R_LG))
            .bg(p.overlay)
            .text_color(p.ink)
            .shadow(p.floating_shadow())
            .overflow_hidden()
            .on_mouse_down_out(move |_, w, cx| {
                if let Some(h) = &close {
                    h(HandoffIntent::Close, w, cx);
                }
            })
            .child(query)
            .child(body)
            .child(
                div()
                    .mt(px(4.0))
                    .pt(px(8.0))
                    .pb(px(10.0))
                    .px(px(PAD_X))
                    .border_t_1()
                    .border_color(p.line)
                    .ui(FOOT_TEXT)
                    .line_height(px(FOOT_LH))
                    .text_color(p.ink_3)
                    .child(self.note.clone()),
            )
    }
}
