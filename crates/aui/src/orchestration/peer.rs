//! The peer message: a compact activity row for outgoing messages — sent,
//! delivered, working, queued or unreachable — that becomes the peer's reply
//! once it arrives, and a left-aligned bubble for incoming ones.
//!
//! The outgoing row is not a card. The reply fold is host-owned through
//! [`.expanded()`](PeerMessageCard::expanded): the fold control reports
//! [`PeerIntent::ToggleReply`] and the host flips the flag and re-renders,
//! while `Show more` / `Show less` names the fold for the accessibility
//! tree. An incoming message is data from another session, so it is never
//! styled like the person's own bubble and carries no approval affordance.

use std::rc::Rc;

use aui_protocol::{PeerDirection, PeerMessage, PeerState, Provider as WireProvider};
use aui_tokens::{scale, scaled, ActiveAui, AgentState, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, record_ax_label, status_dot, ButtonVariant};
use crate::icons::{provider_mark, Provider as IconProvider};

/// Row rhythm shared by the outgoing rows.
const ROW_GAP: f32 = 6.0;
/// Provider marks in every peer row and label.
const MARK: f32 = 14.0;
/// The reply sits indented under the `Reply from …` row.
const REPLY_INDENT: f32 = 22.0;
/// The collapsed reply shows two lines of 12 px body text.
const REPLY_TEXT: f32 = scale::FS_12;
/// Replies longer than this get the fold control; shorter ones render in
/// full, since two lines already fit them.
const REPLY_FOLD_CHARS: usize = 140;
/// The incoming bubble's padding.
const BUBBLE_PAD_Y: f32 = 10.0;
const BUBBLE_PAD_X: f32 = 12.0;

type IntentHandler = Rc<dyn Fn(PeerIntent, &mut Window, &mut App)>;

/// What the person asked for on a peer message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerIntent {
    /// Open the peer session; the message id, which the host maps back to
    /// the session.
    OpenSession(String),
    /// The reply fold control was picked; the message id, which the host
    /// maps back to its transcript-level expanded flag.
    ToggleReply(String),
}

/// The peer message. Build with [`peer_message`].
#[derive(IntoElement)]
pub struct PeerMessageCard {
    id: ElementId,
    message: PeerMessage,
    expanded: bool,
    on_intent: Option<IntentHandler>,
}

/// A message to or from a peer session.
///
/// The card clones the value; it owns no orchestration logic.
pub fn peer_message(id: impl Into<ElementId>, message: &PeerMessage) -> PeerMessageCard {
    PeerMessageCard { id: id.into(), message: message.clone(), expanded: false, on_intent: None }
}

impl PeerMessageCard {
    /// Whether the reply is shown in full. Unset: the reply collapses to two
    /// lines behind `Show more`.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// `Open session` was picked; the message id is echoed back.
    pub fn on_intent(mut self, f: impl Fn(PeerIntent, &mut Window, &mut App) + 'static) -> Self {
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

/// The dot an outgoing message draws: in motion while the reply is pending,
/// danger when it can never arrive, settled once replied.
fn dot_state(message: &PeerMessage) -> (AgentState, bool) {
    match message.state {
        PeerState::Sent | PeerState::Delivered | PeerState::Working => (AgentState::Running, true),
        PeerState::Queued => (AgentState::Idle, false),
        PeerState::Replied => (AgentState::Done, false),
        PeerState::Unreachable => (AgentState::Failed, false),
    }
}

/// The status words of an outgoing message that has no reply yet.
fn pending_status(message: &PeerMessage) -> String {
    let peer = message.peer_title.as_str();
    match message.state {
        PeerState::Sent => format!("Messaged \"{peer}\" · waiting for reply"),
        PeerState::Delivered => format!("Messaged \"{peer}\" · delivered · waiting for reply"),
        PeerState::Working => format!("\"{peer}\" is working on it"),
        PeerState::Queued => format!("Queued · \"{peer}\" is busy, delivered when its turn ends"),
        PeerState::Unreachable => {
            let reason = message.reason.as_deref().unwrap_or("the session is gone");
            format!("Couldn't reach \"{peer}\" · {reason}")
        }
        PeerState::Replied => format!("Reply from \"{peer}\""),
    }
}

impl RenderOnce for PeerMessageCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        // Incoming: a left-aligned bubble on surface-2 with a line border —
        // deliberately not the person's own bubble.
        if self.message.direction == PeerDirection::Incoming {
            return v_flex()
                .id(id)
                .w_full()
                .items_start()
                .child(
                    v_flex()
                        .max_w(gpui::relative(0.85))
                        .gap(px(ROW_GAP))
                        .py(px(BUBBLE_PAD_Y))
                        .px(px(BUBBLE_PAD_X))
                        .rounded(px(scale::R_LG))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.surface_2)
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .gap(px(ROW_GAP))
                                .child(provider_mark(mark(self.message.peer_provider)).size(px(MARK)))
                                .child(status_dot((self.id.clone(), "dot"), AgentState::Done))
                                .child(
                                    div()
                                        .min_w(px(0.0))
                                        .truncate()
                                        .text_role(TextRole::Meta)
                                        .text_color(p.ink_3)
                                        .child(format!(
                                            "From \"{}\" · another session, not you",
                                            self.message.peer_title
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .ui(scale::FS_12)
                                .text_color(p.ink)
                                .child(self.message.text.clone()),
                        ),
                )
                .into_any_element();
        }

        // The trailing session link every outgoing row carries. It is a link
        // only while a handler is set; otherwise it is plain text.
        let link_label = format!("Open the \"{}\" session", self.message.peer_title);
        let message_id = self.message.message_id.clone();
        let mut link = div()
            .id((id.clone(), "session"))
            .flex_none()
            .whitespace_nowrap()
            .ui(scale::FS_12)
            .text_color(p.accent_ink)
            .child("Open session ↗");
        if let Some(on_intent) = self.on_intent.clone() {
            record_ax_label(&link_label);
            let link_id = message_id.clone();
            link = link
                .cursor_pointer()
                .role(gpui::Role::Link)
                .aria_label(link_label.clone())
                .on_click(move |_, window, cx| {
                    on_intent(PeerIntent::OpenSession(link_id.clone()), window, cx)
                });
        }

        // Outgoing with a reply: the row becomes the reply header and the
        // reply text sits indented below it.
        if let Some(reply) = &self.message.reply {
            let needs_fold = reply.text.chars().count() > REPLY_FOLD_CHARS;
            let mut reply_text =
                div().w_full().ui(REPLY_TEXT).text_color(p.ink_2).child(reply.text.clone());
            if needs_fold && !self.expanded {
                reply_text = reply_text
                    .line_clamp(2)
                    .max_h(scaled(2.0 * REPLY_TEXT * scale::LH_UI))
                    .overflow_hidden();
            }
            let mut body =
                v_flex().w_full().pl(px(REPLY_INDENT)).gap(px(ROW_GAP)).child(reply_text);
            if needs_fold {
                let fold_label = if self.expanded { "Show less" } else { "Show more" };
                let fold_ax = if self.expanded {
                    format!("Collapse the reply from \"{}\"", self.message.peer_title)
                } else {
                    format!("Show the whole reply from \"{}\"", self.message.peer_title)
                };
                let fold_id = message_id.clone();
                let mut fold = button((id.clone(), "fold"), fold_label)
                    .variant(ButtonVariant::Ghost)
                    .muted()
                    .sm()
                    .accessibility_label(fold_ax);
                if let Some(on_intent) = self.on_intent.clone() {
                    fold = fold.on_click(move |_, window, cx| {
                        on_intent(PeerIntent::ToggleReply(fold_id.clone()), window, cx)
                    });
                }
                body = body.child(div().w_full().flex().justify_start().child(fold));
            }
            return v_flex()
                .id(id.clone())
                .w_full()
                .gap(px(ROW_GAP))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap(px(ROW_GAP))
                        .ui(scale::FS_12)
                        .child(status_dot((id.clone(), "dot"), AgentState::Done))
                        .child(provider_mark(mark(self.message.peer_provider)).size(px(MARK)))
                        .child(
                            div()
                                .min_w(px(0.0))
                                .truncate()
                                .medium()
                                .text_color(p.ink)
                                .child(format!("Reply from \"{}\"", self.message.peer_title)),
                        )
                        .child(div().flex_1())
                        .child(link),
                )
                .child(body)
                .into_any_element();
        }

        // Outgoing and still pending: one compact activity row plus the link.
        let (agent, pulse) = dot_state(&self.message);
        let unreachable = self.message.state == PeerState::Unreachable;
        h_flex()
            .id(id)
            .w_full()
            .items_center()
            .gap(px(ROW_GAP))
            .ui(scale::FS_12)
            .child(status_dot((self.id.clone(), "dot"), agent).pulse(pulse))
            .child(provider_mark(mark(self.message.peer_provider)).size(px(MARK)))
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .text_color(if unreachable { p.danger } else { p.ink_2 })
                    .child(pending_status(&self.message)),
            )
            .child(div().flex_1())
            .child(link)
            .into_any_element()
    }
}
