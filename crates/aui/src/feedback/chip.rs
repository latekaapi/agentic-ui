//! The corner stack's two pills (D4, docs/41 §2.11): the toast chip and the
//! floating pill the feedback slot sits in. Both are Floating, the edge and
//! the float drop; the host stacks them 12 px from the sheet's right, newest
//! toast above the feedback slot, 8 apart (Sessions: 8 above the composer;
//! Browse: 12 above the sheet's bottom, in an overlay window over the page).

use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;

use crate::data::record_ax_label;
use crate::util::ClickHandler;

/// A pill is 36 tall, radius 10.
pub const PILL_H: f32 = 36.0;
const PILL_RADIUS: f32 = 10.0;
/// The toast chip's widest.
pub const TOAST_MAX_W: f32 = 480.0;
/// A toast rises 6 px as it fades in; it leaves in 0.12 s.
const TOAST_RISE: f32 = 6.0;
const TOAST_TIMING: EnterExit = EnterExit { enter: Duration::from_millis(160), exit: Duration::from_millis(120), delay: Duration::ZERO };

/// The floating pill: 36 px, radius 10, Floating, the edge and the drop.
/// The caller sets its padding and children.
pub fn floating_pill(id: impl Into<ElementId>, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    h_flex().id(id.into()).flex_none().h(px(PILL_H)).items_center().rounded(px(PILL_RADIUS)).bg(p.overlay).shadow(p.floating_shadow())
}

/// The status a toast's dot says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastTone {
    /// Something finished (success).
    Success,
    /// Something needs a look (warning).
    Warning,
    /// Neither (muted).
    #[default]
    Neutral,
}

/// One toast chip. Build with [`toast_chip`].
#[derive(IntoElement)]
pub struct ToastChip {
    id: ElementId,
    tone: ToastTone,
    title: SharedString,
    body: SharedString,
    queued: usize,
    action: Option<(SharedString, ClickHandler)>,
    on_close: Option<ClickHandler>,
    present: bool,
    at_rest: bool,
}

/// A toast: the dot (8, in its tone), the title 600, the body muted
/// (truncating), `+n` when more wait, an optional text action and ✕.
pub fn toast_chip(id: impl Into<ElementId>, tone: ToastTone, title: impl Into<SharedString>, body: impl Into<SharedString>) -> ToastChip {
    ToastChip { id: id.into(), tone, title: title.into(), body: body.into(), queued: 0, action: None, on_close: None, present: true, at_rest: false }
}

impl ToastChip {
    /// How many more toasts wait behind this one.
    pub fn queued(mut self, queued: usize) -> Self {
        self.queued = queued;
        self
    }

    /// One text action (`Undo`, `Show in Finder`).
    pub fn action(mut self, label: impl Into<SharedString>, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.action = Some((label.into(), Box::new(f)));
        self
    }

    /// ✕ was pressed.
    pub fn on_close(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }

    /// Whether it is on screen; `false` plays the 0.12 s leave.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Drawn settled, without the enter (deterministic captures).
    pub fn at_rest(mut self, at_rest: bool) -> Self {
        self.at_rest = at_rest;
        self
    }
}

/// A 24 px text control inside a pill.
fn pill_text(id: impl Into<ElementId>, label: SharedString, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    record_ax_label(label.as_ref());
    h_flex()
        .id(id.into())
        .flex_none()
        .h(px(24.0))
        .px(px(8.0))
        .rounded(px(6.0))
        .ui(scale::FS_12)
        .text_color(p.ink_2)
        .cursor_pointer()
        .hover(|d| d.text_color(p.ink).bg(p.hover_overlay()))
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .child(label)
}

impl RenderOnce for ToastChip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let style = if self.at_rest {
            PresenceStyle { opacity: 1.0, offset_y: px(0.0), scale: 1.0 }
        } else {
            PresenceStyle::fade_rise(presence((id.clone(), "enter"), self.present, TOAST_TIMING, window, cx), TOAST_RISE)
        };
        let dot = match self.tone {
            ToastTone::Success => p.success,
            ToastTone::Warning => p.warning,
            ToastTone::Neutral => p.ink_2,
        };
        let tip = format!("{}: {}", self.title, self.body);
        let mut chip = floating_pill(id.clone(), cx)
            .relative()
            .top(style.offset_y)
            .opacity(style.opacity)
            .max_w(px(TOAST_MAX_W))
            .pl(px(12.0))
            .pr(px(6.0))
            .gap(px(8.0))
            .overflow_hidden()
            .role(gpui::Role::Status)
            .aria_label(tip)
            .child(div().flex_none().size(px(8.0)).rounded_full().bg(dot))
            // Title and body truncate so the action and ✕ always stay inside.
            .child(div().flex_none().max_w(px(200.0)).truncate().ui(scale::FS_13).semibold().text_color(p.ink).child(self.title))
            .child(div().flex_shrink(1.0).min_w(px(0.0)).truncate().ui(scale::FS_13).text_color(p.ink_2).child(self.body));
        if self.queued > 0 {
            chip = chip.child(div().flex_none().ui(scale::FS_12).text_color(p.ink_4).child(format!("+{}", self.queued)));
        }
        if let Some((label, on_click)) = self.action {
            chip = chip.child(pill_text((id.clone(), "action"), label, cx).on_click(move |e, w, cx| on_click(e, w, cx)));
        }
        if let Some(on_close) = self.on_close {
            record_ax_label("Dismiss notification");
            chip = chip.child(
                div()
                    .id((id, "close"))
                    .flex_none()
                    .size(px(24.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(p.ink_2)
                    .hover(|d| d.text_color(p.ink).bg(p.hover_overlay()))
                    .role(gpui::Role::Button)
                    .aria_label("Dismiss notification")
                    .on_click(move |e, w, cx| on_close(e, w, cx))
                    .child(icon(IconName::X).size(px(11.0))),
            );
        }
        chip
    }
}

/// The feedback slot's controls in their pill (docs/41 §2.11): the toggle
/// (28, the message-plus glyph); while active the toggle lit (9 % and the
/// foreground), the note count in faint mono, **Done** as the primary (24)
/// and **Discard** as text.
#[derive(IntoElement)]
pub struct FeedbackPill {
    id: ElementId,
    active: bool,
    notes: Option<SharedString>,
    on_toggle: Option<ClickHandler>,
    on_done: Option<ClickHandler>,
    on_discard: Option<ClickHandler>,
}

/// The feedback slot; `active` while feedback mode is capturing.
pub fn feedback_pill(id: impl Into<ElementId>, active: bool) -> FeedbackPill {
    FeedbackPill { id: id.into(), active, notes: None, on_toggle: None, on_done: None, on_discard: None }
}

impl FeedbackPill {
    /// `3 notes`: shown with Done and Discard.
    pub fn notes(mut self, notes: Option<SharedString>) -> Self {
        self.notes = notes;
        self
    }

    /// The toggle.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    /// Done.
    pub fn on_done(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_done = Some(Box::new(f));
        self
    }

    /// Discard.
    pub fn on_discard(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_discard = Some(Box::new(f));
        self
    }
}

impl RenderOnce for FeedbackPill {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let label = if self.active { "Stop giving feedback" } else { "Give feedback" };
        record_ax_label(label);
        let mut toggle = div()
            .id((id.clone(), "toggle"))
            .flex_none()
            .size(px(28.0))
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(if self.active { p.ink } else { p.ink_2 })
            .when(self.active, |d| d.bg(p.selected_overlay()))
            .when(!self.active, |d| d.hover(|d| d.bg(p.hover_overlay()).text_color(p.ink)))
            .role(gpui::Role::Button)
            .aria_label(label)
            .child(icon(IconName::MessagePlus).size(px(14.0)).lit(self.active));
        if let Some(on_toggle) = self.on_toggle {
            toggle = toggle.on_click(move |e, w, cx| on_toggle(e, w, cx));
        }
        let mut pill = floating_pill(id.clone(), cx).px(px(4.0)).gap(px(6.0)).child(toggle);
        if let Some(notes) = self.notes {
            pill = pill.child(div().flex_none().pl(px(2.0)).mono(scale::FS_11).text_color(p.ink_4).whitespace_nowrap().child(notes));
            record_ax_label("Done");
            let mut done = h_flex()
                .id((id.clone(), "done"))
                .flex_none()
                .h(px(24.0))
                .px(px(8.0))
                .rounded(px(6.0))
                .bg(p.ink)
                .text_color(p.bg)
                .ui(scale::FS_12)
                .semibold()
                .cursor_pointer()
                .role(gpui::Role::Button)
                .aria_label("Done")
                .child("Done");
            if let Some(on_done) = self.on_done {
                done = done.on_click(move |e, w, cx| on_done(e, w, cx));
            }
            let mut discard = pill_text((id, "discard"), "Discard".into(), cx);
            if let Some(on_discard) = self.on_discard {
                discard = discard.on_click(move |e, w, cx| on_discard(e, w, cx));
            }
            pill = pill.child(done).child(discard).pr(px(6.0));
        }
        pill
    }
}
