//! The handoff card: a session moved from one provider to another as an
//! honest, lossy re-prompt — never pretended continuity.
//!
//! The header names the destination (`Handed off to Claude Code · opus 4.6`)
//! with both provider marks (`from → to`) and the source underneath; the body
//! keeps a compact state line, the two lists that say exactly what crossed
//! over (`Carried`) and what did not (`Not carried`, always shown, never
//! hidden), and the pack size. `Open the new session` appears once the fresh
//! session exists; `Cancel` only while the move can still be stopped. Every
//! button carries an accessibility role and label.
//!
//! [`handoff_confirm`] is the dialog content the host shows before starting:
//! the destination row with the provider mark, the same shared lists the
//! card draws, the pack size, and the honest note that the new session
//! starts fresh — over the existing modal, with Cancel / Hand off.

use std::rc::Rc;

use aui_protocol::{HandoffItem, HandoffState, Provider as WireProvider};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::composer::provider_display_name;
use crate::data::{button, glyph_err, glyph_ok, pill, record_ax_label, spinner, tag, PillVariant};
use crate::icons::{icon, provider_mark, IconName, Provider as IconProvider};
use crate::overlay::{dialog, Dialog};

/// Header rhythm: title 13 semibold over the muted source line, 16 px marks.
const HEAD_PAD_Y: f32 = 10.0;
const HEAD_PAD_X: f32 = 12.0;
const HEAD_GAP: f32 = 8.0;
const TITLE_TEXT: f32 = 13.0;
const SOURCE_TEXT: f32 = 11.5;
const MARK: f32 = 16.0;
const ARROW: f32 = 12.0;
/// Body rhythm: 12 px list text, caps section labels, 8 px section gaps.
const BODY_PAD_X: f32 = 12.0;
const BODY_PAD_BOTTOM: f32 = 10.0;
const BODY_GAP: f32 = 8.0;
const SECTION_GAP_TOP: f32 = 8.0;
const ITEM_TEXT: f32 = 12.5;
const DETAIL_TEXT: f32 = 12.0;
const STATE_TEXT: f32 = 12.0;
/// The action row: hint, spacer, Cancel, Open.
const ACTIONS_PAD_Y: f32 = 10.0;
const ACTIONS_GAP: f32 = 8.0;

/// The honest note every confirm dialog carries, verbatim.
pub const HANDOFF_FRESH_NOTE: &str =
    "The new session starts fresh with a summary. Tool state, pending approvals and the provider's own memory do not carry over.";

/// The confirm dialog's width: wider than the default modal so the shared
/// list rows fit without wrapping their labels.
pub const HANDOFF_CONFIRM_WIDTH: f32 = 520.0;

type IntentHandler = Rc<dyn Fn(HandoffIntent, &mut Window, &mut App)>;

/// What the person asked for on a handoff card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffIntent {
    /// Open the fresh destination session; the id of that session.
    OpenSession(String),
    /// Stop the move while it is still cancellable; the card's own block id
    /// is the one the host's closure captured.
    Cancel,
}

/// Where one step of a handoff stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandoffStepState {
    /// Finished.
    Done,
    /// Running now; drawn with the spinner and the host's elapsed counter.
    Current,
    /// Not started yet.
    #[default]
    Pending,
    /// Stopped by an error; the step's `detail` carries the reason.
    Failed,
    /// Deliberately not run (a redundant summary, a skipped confirmation).
    Skipped,
}

/// One row of the handoff card's progress list: a short human label the host
/// supplies (`Pack the context`), its state, and an optional detail — the
/// live elapsed counter (`6 s`) on the current step, or the reason on a
/// failed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffStep {
    /// The short human label the host supplies.
    pub label: String,
    /// Where the step stands.
    pub state: HandoffStepState,
    /// The elapsed counter on the current step, or the reason on a failed one.
    pub detail: Option<String>,
}

impl HandoffStep {
    /// A step with `label` in `state` and no detail.
    pub fn new(label: impl Into<String>, state: HandoffStepState) -> Self {
        HandoffStep { label: label.into(), state, detail: None }
    }

    /// The live detail: the elapsed counter (`6 s`) or the failure reason.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// The default step labels a host can start from — pack, summary, start,
/// confirm — all pending; the host marks them as the move proceeds. The last
/// two name the destination provider.
pub fn default_handoff_steps(to: WireProvider) -> Vec<HandoffStep> {
    let to_name = provider_display_name(mark(to));
    vec![
        HandoffStep::new("Pack the context", HandoffStepState::Pending),
        HandoffStep::new("Write a summary", HandoffStepState::Pending),
        HandoffStep::new(format!("Start {to_name}"), HandoffStepState::Pending),
        HandoffStep::new(format!("{to_name} confirms"), HandoffStepState::Pending),
    ]
}

/// The plain state word a step carries in its accessible label.
pub fn handoff_step_state_word(state: HandoffStepState) -> &'static str {
    match state {
        HandoffStepState::Done => "done",
        HandoffStepState::Current => "in progress",
        HandoffStepState::Pending => "pending",
        HandoffStepState::Failed => "failed",
        HandoffStepState::Skipped => "skipped",
    }
}

/// `Write a summary, in progress, 6 s` — every step names its own state (and
/// its elapsed counter or reason), so the list reads without a live region.
pub fn handoff_step_label(step: &HandoffStep) -> String {
    let mut label = format!("{}, {}", step.label, handoff_step_state_word(step.state));
    if let Some(detail) = &step.detail {
        label.push_str(&format!(", {detail}"));
    }
    label
}

/// The plain words in the title pill: what happened, never the internal
/// state name.
pub fn handoff_pill_text(state: &HandoffState) -> SharedString {
    match state {
        HandoffState::Requested
        | HandoffState::Quiescing
        | HandoffState::Checkpointed
        | HandoffState::Prepared
        | HandoffState::Acknowledged => "Handing off…".into(),
        HandoffState::Activated => "Handed off".into(),
        HandoffState::Refused { .. } | HandoffState::Failed { .. } => "Hand-off failed".into(),
        HandoffState::Cancelled => "Cancelled".into(),
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

/// `8400` → `"8,400"`, for the pack-size line.
fn grouped_tokens(n: u64) -> String {
    let digits: Vec<char> = n.to_string().chars().collect();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*c);
    }
    out
}

/// `Starts a new Claude Code session · opus 4.6` — the confirm's
/// destination line, without the mark.
pub fn handoff_confirm_destination(to: WireProvider, to_model: &str) -> SharedString {
    SharedString::from(format!("Starts a new {} session · {to_model}", provider_display_name(mark(to))))
}

/// The `Carried` list the card and the confirm dialog share: one row per
/// item, the label in ink, the muted detail after it.
fn carried_list(p: &Palette, carried: &[HandoffItem]) -> AnyElement {
    let mut list =
        v_flex().w_full().gap(px(2.0)).child(div().text_role(TextRole::Caps).text_color(p.ink_3).child("Carried"));
    if carried.is_empty() {
        list = list.child(div().ui(ITEM_TEXT).text_color(p.ink_3).child("Nothing carried over."));
    }
    for item in carried {
        list = list.child(
            h_flex()
                .w_full()
                .gap(px(scale::SP_2))
                .ui(ITEM_TEXT)
                .child(div().flex_none().text_color(p.ink).child(item.label.clone()))
                .children(item.detail.clone().map(|d| {
                    div().min_w(px(0.0)).truncate().ui(DETAIL_TEXT).text_color(p.ink_3).child(format!("· {d}"))
                })),
        );
    }
    list.into_any_element()
}

/// The `Not carried` list the card and the confirm dialog share: the same
/// rows in ink-3, never hidden behind a fold.
fn lost_list(p: &Palette, lost: &[HandoffItem]) -> AnyElement {
    let mut list =
        v_flex().w_full().gap(px(2.0)).child(div().text_role(TextRole::Caps).text_color(p.ink_3).child("Not carried"));
    if lost.is_empty() {
        list = list.child(div().ui(ITEM_TEXT).text_color(p.ink_3).child("Nothing left behind."));
    }
    for item in lost {
        list = list.child(
            h_flex()
                .w_full()
                .gap(px(scale::SP_2))
                .ui(ITEM_TEXT)
                .text_color(p.ink_3)
                .child(div().flex_none().child(item.label.clone()))
                .children(item.detail.clone().map(|d| {
                    div().min_w(px(0.0)).truncate().ui(DETAIL_TEXT).child(format!("· {d}"))
                })),
        );
    }
    list.into_any_element()
}

/// The pack size both the card and the confirm dialog draw.
fn pack_line(tokens: u64) -> AnyElement {
    div().w_full().child(tag(format!("~{} tokens of context", grouped_tokens(tokens)))).into_any_element()
}

/// The handoff card. Build with [`handoff_card`].
#[derive(IntoElement)]
pub struct HandoffCard {
    id: ElementId,
    from: WireProvider,
    to: WireProvider,
    from_model: SharedString,
    to_model: SharedString,
    state: HandoffState,
    carried: Vec<HandoffItem>,
    lost: Vec<HandoffItem>,
    pack_tokens: Option<u64>,
    steps: Vec<HandoffStep>,
    destination_session: Option<String>,
    on_intent: Option<IntentHandler>,
}

/// A handoff from `from` to `to`, landing on `to_model`, in `state`.
///
/// The fields mirror [`aui_protocol::Block::Handoff`]; the lists, pack size
/// and destination session are set with the builders below.
pub fn handoff_card(
    id: impl Into<ElementId>,
    from: WireProvider,
    to: WireProvider,
    to_model: impl Into<SharedString>,
    state: HandoffState,
) -> HandoffCard {
    HandoffCard {
        id: id.into(),
        from,
        to,
        from_model: SharedString::default(),
        to_model: to_model.into(),
        state,
        carried: Vec::new(),
        lost: Vec::new(),
        pack_tokens: None,
        steps: Vec::new(),
        destination_session: None,
        on_intent: None,
    }
}

impl HandoffCard {
    /// The model label on the source side, shown under the header.
    pub fn from_model(mut self, model: impl Into<SharedString>) -> Self {
        self.from_model = model.into();
        self
    }

    /// What the context pack carries over.
    pub fn carried(mut self, carried: Vec<HandoffItem>) -> Self {
        self.carried = carried;
        self
    }

    /// What does not carry over; always drawn, never hidden.
    pub fn lost(mut self, lost: Vec<HandoffItem>) -> Self {
        self.lost = lost;
        self
    }

    /// The measured pack size, drawn as `~N tokens of context`.
    pub fn pack_tokens(mut self, tokens: Option<u64>) -> Self {
        self.pack_tokens = tokens;
        self
    }

    /// The progress steps, drawn as the `Hand-off progress` list while the
    /// move is live. The host supplies short human labels (start from
    /// [`default_handoff_steps`]); the current step may carry the elapsed
    /// counter as its detail, a failed step its reason. Once the move
    /// settles (handed off or cancelled) the list hides again, leaving the
    /// settled form.
    pub fn steps(mut self, steps: Vec<HandoffStep>) -> Self {
        self.steps = steps;
        self
    }

    /// The fresh destination session; shows `Open the new session`.
    pub fn destination_session(mut self, session: Option<String>) -> Self {
        self.destination_session = session;
        self
    }

    /// The person pressed `Open the new session` or `Cancel`.
    pub fn on_intent(mut self, f: impl Fn(HandoffIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

/// The pill beside the title: quiet while moving, success once handed off,
/// danger on refusal or failure. The text is plain words ([`handoff_pill_text`]),
/// never the internal state name.
fn state_pill(state: &HandoffState) -> (SharedString, PillVariant) {
    let variant = match state {
        HandoffState::Activated => PillVariant::Success,
        HandoffState::Refused { .. } | HandoffState::Failed { .. } => PillVariant::Danger,
        _ => PillVariant::Quiet,
    };
    (handoff_pill_text(state), variant)
}

/// Whether the move has settled: handed off or cancelled. Settled cards hide
/// the step list again, collapsing to the state line and the shared lists.
fn settled(state: &HandoffState) -> bool {
    matches!(state, HandoffState::Activated | HandoffState::Cancelled)
}

/// The progress list: one row per step with its state mark, labelled
/// `Hand-off progress` with every row naming its own state. The current step
/// spins and shows the host's elapsed counter; a failed step shows its
/// reason in the danger tone.
fn step_list(id: &ElementId, p: &Palette, steps: &[HandoffStep]) -> AnyElement {
    let mut list = v_flex()
        .id((id.clone(), "steps"))
        .w_full()
        .gap(px(2.0))
        .role(gpui::Role::List)
        .aria_label("Hand-off progress");
    for (i, step) in steps.iter().enumerate() {
        let label = handoff_step_label(step);
        record_ax_label(&label);
        let row_id: ElementId = (id.clone(), SharedString::from(format!("step-{i}"))).into();
        let mark: AnyElement = match step.state {
            HandoffStepState::Done => glyph_ok().into_any_element(),
            HandoffStepState::Current => spinner((row_id.clone(), "spin")).into_any_element(),
            HandoffStepState::Failed => glyph_err().into_any_element(),
            HandoffStepState::Pending | HandoffStepState::Skipped => div()
                .flex_none()
                .size(px(14.0))
                .rounded_full()
                .border_1()
                .border_color(p.line_strong)
                .into_any_element(),
        };
        let failed = step.state == HandoffStepState::Failed;
        let dimmed = matches!(step.state, HandoffStepState::Pending | HandoffStepState::Skipped);
        let mut row = h_flex()
            .id(row_id.clone())
            .w_full()
            .items_center()
            .gap(px(scale::SP_2))
            .ui(ITEM_TEXT)
            .text_color(if failed { p.danger } else if dimmed { p.ink_3 } else { p.ink })
            .role(gpui::Role::ListItem)
            .aria_label(label)
            .child(mark)
            .child(div().flex_none().child(step.label.clone()));
        if let Some(detail) = &step.detail {
            row = row.child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .ui(DETAIL_TEXT)
                    .text_color(if failed { p.danger } else { p.ink_3 })
                    .child(detail.clone()),
            );
        }
        list = list.child(row);
    }
    list.into_any_element()
}

/// Whether the move can still be stopped: Requested through Prepared.
fn cancellable(state: &HandoffState) -> bool {
    matches!(
        state,
        HandoffState::Requested | HandoffState::Quiescing | HandoffState::Checkpointed | HandoffState::Prepared
    )
}

impl RenderOnce for HandoffCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let to_name = provider_display_name(mark(self.to));
        let from_name = provider_display_name(mark(self.from));

        let title: SharedString =
            SharedString::from(format!("Handed off to {to_name} · {}", self.to_model));
        let source: SharedString =
            SharedString::from(format!("From {from_name} · {}", self.from_model));
        let (pill_label, pill_variant) = state_pill(&self.state);

        let head = h_flex()
            .w_full()
            .items_center()
            .gap(px(HEAD_GAP))
            .py(px(HEAD_PAD_Y))
            .px(px(HEAD_PAD_X))
            .child(provider_mark(mark(self.from)).size(px(MARK)))
            .child(icon(IconName::ArrowRight).size(px(ARROW)).color(p.ink_3))
            .child(provider_mark(mark(self.to)).size(px(MARK)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().min_w(px(0.0)).truncate().ui(TITLE_TEXT).semibold().text_color(p.ink).child(title))
                    .child(div().min_w(px(0.0)).truncate().ui(SOURCE_TEXT).text_color(p.ink_3).child(source)),
            )
            .child(div().flex_none().child(pill(pill_label).variant(pill_variant)));

        // The compact state line: a quiet progress label while moving, the
        // reason in the danger tone on refusal or failure, and the fresh
        // session's name once active.
        let mut body = v_flex().w_full().px(px(BODY_PAD_X)).pt(px(BODY_GAP)).pb(px(BODY_PAD_BOTTOM)).gap(px(BODY_GAP));
        let state_line: SharedString = match &self.state {
            HandoffState::Requested => "Preparing the context pack…".into(),
            HandoffState::Quiescing => "Quiescing the old session…".into(),
            HandoffState::Checkpointed => "Preparing the handoff…".into(),
            HandoffState::Prepared => SharedString::from(format!("Pack ready — handing over to {to_name}…")),
            HandoffState::Acknowledged => SharedString::from(format!("{to_name} acknowledged — starting the new session…")),
            HandoffState::Activated => SharedString::from(format!("Continued in a new {to_name} session")),
            HandoffState::Refused { reason } | HandoffState::Failed { reason } => SharedString::from(reason.clone()),
            HandoffState::Cancelled => "Handoff cancelled.".into(),
        };
        let failed = matches!(&self.state, HandoffState::Refused { .. } | HandoffState::Failed { .. });
        body = body.child(
            div().w_full().ui(STATE_TEXT).text_color(if failed { p.danger } else { p.ink_2 }).child(state_line),
        );

        // The progress steps while the move is live; settled cards collapse
        // back to the state line and the shared lists.
        if !settled(&self.state) && !self.steps.is_empty() {
            body = body.child(step_list(&id, &p, &self.steps));
        }

        // Carried and Not carried share their rendering with the confirm
        // dialog: the same rows, the lost ones in ink-3, never hidden
        // behind a fold.
        body = body
            .child(div().w_full().mt(px(SECTION_GAP_TOP)).child(carried_list(&p, &self.carried)))
            .child(div().w_full().child(lost_list(&p, &self.lost)));

        if let Some(tokens) = self.pack_tokens {
            body = body.child(pack_line(tokens));
        }

        let mut card = v_flex()
            .id(id.clone())
            .w_full()
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .child(head)
            .child(div().w_full().border_t_1().border_color(p.line).child(body.into_any_element()));

        // The one action row: hint on the left, spacer, Cancel, then Open the
        // new session on the far right.
        let show_open = self.destination_session.is_some();
        let show_cancel = cancellable(&self.state);
        if show_open || show_cancel {
            let mut row = h_flex()
                .w_full()
                .items_center()
                .gap(px(ACTIONS_GAP))
                .py(px(ACTIONS_PAD_Y))
                .px(px(HEAD_PAD_X))
                .border_t_1()
                .border_color(p.line)
                .bg(p.surface_2)
                .child(div().flex_1().min_w(px(0.0)));
            if show_cancel {
                let mut cancel = button((id.clone(), "cancel"), "Cancel").sm();
                cancel = cancel.accessibility_label("Cancel the handoff");
                if let Some(on_intent) = self.on_intent.clone() {
                    cancel = cancel.on_click(move |_, window, cx| on_intent(HandoffIntent::Cancel, window, cx));
                }
                row = row.child(cancel);
            }
            if let Some(session) = self.destination_session.clone() {
                let mut open = button((id.clone(), "open"), "Open the new session").sm().primary();
                open = open.accessibility_label(format!("Open the new session on {to_name}"));
                if let Some(on_intent) = self.on_intent.clone() {
                    open = open.on_click(move |_, window, cx| {
                        on_intent(HandoffIntent::OpenSession(session.clone()), window, cx)
                    });
                }
                row = row.child(open);
            }
            card = card.child(row);
        }
        card
    }
}

/// The confirm dialog the host shows before starting a handoff.
///
/// Built on the existing modal ([`dialog`]): the title names the destination
/// and the structured body carries the destination row with the provider
/// mark, the [`carried_list`] / [`lost_list`] rows the card draws, the pack
/// size, and the honest note as its own paragraph in ink-3. Actions are
/// Cancel (secondary) and Hand off (primary); the host owns what they do
/// through the dialog's own handlers.
pub fn handoff_confirm(
    id: impl Into<ElementId>,
    to: WireProvider,
    to_model: impl Into<SharedString>,
    carried: &[HandoffItem],
    lost: &[HandoffItem],
    pack_tokens: Option<u64>,
) -> Dialog {
    let to_name = provider_display_name(mark(to));
    let to_model = to_model.into();
    let carried = carried.to_vec();
    let lost = lost.to_vec();
    dialog(id, SharedString::from(format!("Hand off to {to_name}?")))
        .width(HANDOFF_CONFIRM_WIDTH)
        .rich_body(move |p| {
            v_flex()
                .w_full()
                .gap(px(BODY_GAP))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap(px(scale::SP_2))
                        .child(provider_mark(mark(to)).size(px(MARK)))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .ui(ITEM_TEXT)
                                .semibold()
                                .text_color(p.ink)
                                .child(handoff_confirm_destination(to, &to_model)),
                        ),
                )
                .child(carried_list(p, &carried))
                .child(lost_list(p, &lost))
                .children(pack_tokens.map(pack_line))
                .child(
                    div()
                        .w_full()
                        .ui(DETAIL_TEXT)
                        .text_color(p.ink_3)
                        .child(HANDOFF_FRESH_NOTE),
                )
                .into_any_element()
        })
        .secondary("Cancel")
        .primary("Hand off")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed_state() -> HandoffState {
        HandoffState::Failed { reason: "destination unreachable".into() }
    }

    /// The pill says what happened in plain words — the internal state names
    /// never reach the person.
    #[test]
    fn pill_uses_plain_words_for_every_state() {
        for state in [
            HandoffState::Requested,
            HandoffState::Quiescing,
            HandoffState::Checkpointed,
            HandoffState::Prepared,
            HandoffState::Acknowledged,
        ] {
            assert_eq!(handoff_pill_text(&state).to_string(), "Handing off…");
        }
        assert_eq!(handoff_pill_text(&HandoffState::Activated).to_string(), "Handed off");
        assert_eq!(
            handoff_pill_text(&HandoffState::Refused { reason: "no".into() }).to_string(),
            "Hand-off failed"
        );
        assert_eq!(handoff_pill_text(&failed_state()).to_string(), "Hand-off failed");
        assert_eq!(handoff_pill_text(&HandoffState::Cancelled).to_string(), "Cancelled");
        for state in [
            HandoffState::Requested,
            HandoffState::Quiescing,
            HandoffState::Checkpointed,
            HandoffState::Prepared,
            HandoffState::Acknowledged,
            HandoffState::Activated,
            HandoffState::Refused { reason: "no".into() },
            failed_state(),
            HandoffState::Cancelled,
        ] {
            let text = handoff_pill_text(&state).to_string();
            for internal in ["Requested", "Quiescing", "Checkpointed", "Prepared", "Acknowledged", "Refused", "Failed"] {
                assert!(!text.contains(internal), "pill leaks {internal:?} in {text:?}");
            }
        }
    }

    /// Every step state has a distinct plain word for the accessible label.
    #[test]
    fn every_step_state_has_a_plain_word() {
        assert_eq!(handoff_step_state_word(HandoffStepState::Done), "done");
        assert_eq!(handoff_step_state_word(HandoffStepState::Current), "in progress");
        assert_eq!(handoff_step_state_word(HandoffStepState::Pending), "pending");
        assert_eq!(handoff_step_state_word(HandoffStepState::Failed), "failed");
        assert_eq!(handoff_step_state_word(HandoffStepState::Skipped), "skipped");
    }

    /// Each step's label names its own state, plus the elapsed counter or
    /// the failure reason when the host supplies one.
    #[test]
    fn step_label_carries_state_and_detail() {
        let current = HandoffStep::new("Write a summary", HandoffStepState::Current).detail("6 s");
        assert_eq!(handoff_step_label(&current), "Write a summary, in progress, 6 s");
        let failed =
            HandoffStep::new("Start Codex", HandoffStepState::Failed).detail("destination unreachable");
        assert_eq!(handoff_step_label(&failed), "Start Codex, failed, destination unreachable");
        let pending = HandoffStep::new("Pack the context", HandoffStepState::Pending);
        assert_eq!(handoff_step_label(&pending), "Pack the context, pending");
        let skipped = HandoffStep::new("Write a summary", HandoffStepState::Skipped);
        assert_eq!(handoff_step_label(&skipped), "Write a summary, skipped");
    }

    /// The default set is pack, summary, start, confirm — the last two naming
    /// the destination — all pending for the host to mark.
    #[test]
    fn default_steps_name_pack_summary_start_and_confirm() {
        let steps = default_handoff_steps(WireProvider::Codex);
        let labels: Vec<&str> = steps.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Pack the context", "Write a summary", "Start Codex", "Codex confirms"]);
        assert!(steps.iter().all(|s| s.state == HandoffStepState::Pending));
        assert!(steps.iter().all(|s| s.detail.is_none()));
    }

    /// Only handed-off and cancelled cards settle; failures keep their steps
    /// so the failed step's reason stays visible.
    #[test]
    fn only_handed_off_and_cancelled_settle() {
        assert!(settled(&HandoffState::Activated));
        assert!(settled(&HandoffState::Cancelled));
        for state in [
            HandoffState::Requested,
            HandoffState::Quiescing,
            HandoffState::Checkpointed,
            HandoffState::Prepared,
            HandoffState::Acknowledged,
            HandoffState::Refused { reason: "no".into() },
            failed_state(),
        ] {
            assert!(!settled(&state), "{state:?} must keep its steps");
        }
    }
}
