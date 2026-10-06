//! The agent window over a web page: the `⌘J` line, the `⌘E` card, the column.
//!
//! The host draws its own translucent window; everything here is plain
//! content sized for it: the line is one 48 px row, 560–640 px wide, on the
//! overlay surface at radius `r-xl` with a hairline and `shadow-3`; the card
//! is 400 × 520 at radius 12 on the same surface; the column is the card's
//! body without the frame, sized by the host's right pane.
//!
//! Every state takes data in and reports an intent out; there is no I/O
//! here. The text inputs stay host-owned
//! ([`TextareaState`](gpui_kit::base::input::TextareaState), built with the
//! composer's `composer_state` pattern) and are only borrowed for the frame.

use std::rc::Rc;

use aui_protocol::{ActivityState, Step as ProtocolStep, StepState as ProtocolStepState};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, relative, App, ElementId, Entity, FocusHandle, IntoElement, SharedString, Window};
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::input::Textarea;

use crate::composer::ComposerChip;
use crate::data::{button, chip, icon_button, kbd, pill, record_ax_label};
use crate::icons::{icon, IconName};
use crate::keys::{APPROVAL_CONTEXT, ApproveOnce, Cancel, Deny};
use crate::transcript::{activity_group, user_turn};

/// The `⌘J` row height.
const LINE_H: f32 = 48.0;
/// The `⌘J` row measure, 560–640 px wide.
const LINE_MIN_W: f32 = 560.0;
const LINE_MAX_W: f32 = 640.0;
/// The `⌘E` card measure.
const CARD_W: f32 = 400.0;
const CARD_H: f32 = 520.0;
/// The state face (and the approval shield): the same 26 px tile the
/// approval card leads with.
const FACE: f32 = 26.0;
const FACE_RADIUS: f32 = 7.0;
const FACE_GLYPH: f32 = 14.0;
/// Row padding and gaps.
const ROW_PAD_X: f32 = 12.0;
const ROW_GAP: f32 = 8.0;
/// Body and caption type.
const BODY_TEXT: f32 = 13.5;
const SMALL_TEXT: f32 = 12.0;
const CAPTION_TEXT: f32 = 11.0;
/// The remove `×` in a context chip.
const CHIP_X: f32 = 10.0;

/// Which face the agent line wears while it works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFace {
    /// Running: an accent-tinted tile with a spinner.
    Working,
    /// Parked with the person driving: a quiet tile with a clock.
    Paused,
    /// The person must step in: a warning-tinted tile with a bell.
    Warning,
}

/// One step of an agent transcript body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The step line (`Opened PGI 2.0 state scores (2023-24)`).
    pub text: SharedString,
    /// Whether it is done, running or failed.
    pub state: StepState,
}

impl Step {
    /// A step with `text` in `state`.
    pub fn new(text: impl Into<SharedString>, state: StepState) -> Self {
        Step { text: text.into(), state }
    }

    /// Whether this step is still running.
    pub fn is_running(&self) -> bool {
        self.state == StepState::Running
    }
}

/// Whether one [`Step`] is done, running or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    /// Finished successfully.
    Done,
    /// Currently running.
    Running,
    /// Finished with an error.
    Failed,
}

/// What the approval line asks about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalData {
    /// Bold head line (`Claude wants to download …`).
    pub title: SharedString,
    /// The host the request goes to (`data.gov.in`), drawn in mono.
    pub host: SharedString,
    /// The full second line (`data.gov.in · 14 MB · to ~/Downloads/pgi-compare`);
    /// the host prefix renders in mono, the rest in `ink-2`.
    pub line: SharedString,
    /// The quiet pill at the right (`Download`).
    pub kind_pill: SharedString,
    /// The hint at the left of the action row (`Always asks before downloading`).
    pub hint: SharedString,
    /// The secondary button (`Don't allow`).
    pub deny: SharedString,
    /// The primary button (`Download`).
    pub allow: SharedString,
}

/// The `⌘J` line's state.
#[derive(Debug, Clone, PartialEq)]
pub enum LineState {
    /// The idle prompt: sparkle, input, send; below, the chip row and the
    /// site actions under a quiet caption.
    Ask {
        /// The input's accessible name; the visual hint stays the text
        /// state's own placeholder, which the host owns.
        text: SharedString,
        /// Context chips (`This page` with `×`).
        chips: Vec<ComposerChip>,
        /// `(id, label)` ghost actions under `ON THIS SITE`.
        site_actions: Vec<(SharedString, SharedString)>,
    },
    /// The agent is working: face, status, `Take over`.
    Driving {
        /// One status line (`Claude is reading UDISE+ · dropout`).
        status: SharedString,
        /// Which face the row wears.
        face: TaskFace,
    },
    /// The person drives while the agent waits.
    YouDrive,
    /// The page needs the person (`data.gov.in asks you to sign in`).
    NeedsYou {
        /// What needs the person.
        text: SharedString,
    },
    /// The line grown into a card: shield, title, pill, detail, deny/allow.
    Approval(ApprovalData),
}

impl LineState {
    /// The working line for `face` with one `status` line.
    pub fn from_task(face: TaskFace, status: impl Into<SharedString>) -> Self {
        LineState::Driving { status: status.into(), face }
    }
}

/// What the `⌘J` line asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum LineIntent {
    /// Send the input's current text.
    Send(String),
    /// Open the composer's `+` menu.
    Plus,
    /// Run one site action, by id.
    SiteAction(SharedString),
    /// Interrupt and take the wheel.
    TakeOver,
    /// Hand the wheel back to the agent.
    HandBack,
    /// The person finished what was needed.
    Done,
    /// Stop the agent.
    Stop,
    /// Allow the pending request.
    Allow,
    /// Deny the pending request.
    Deny,
    /// Remove one context chip, by id.
    ChipRemove(SharedString),
    /// Expand the line into the card.
    Expand,
}

/// One item of an agent transcript body.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentItem {
    /// The person's ask, drawn as a user bubble.
    User(SharedString),
    /// Tool steps, drawn as a steps card with ✓/spinner rows.
    Steps(Vec<Step>),
    /// A follow-up waiting behind the turn (`also add girls' GER · queued`).
    Queued(SharedString),
    /// The agent's answer text.
    Answer(SharedString),
    /// A question with choice buttons.
    Question {
        /// The question.
        text: SharedString,
        /// One button per choice, in order.
        choices: Vec<SharedString>,
    },
}

/// The transcript the card and the column share.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentTranscript {
    /// The card title (`PGI vs school indicators`).
    pub title: SharedString,
    /// The model under the title (`Claude Sonnet`).
    pub model: SharedString,
    /// The body, top to bottom.
    pub items: Vec<AgentItem>,
}

/// Which frame the shared body sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSize {
    /// The 400 × 520 card with `Dock to column` in the header.
    Card,
    /// The host-sized right pane: the same body, `Undock` in the header.
    Column,
}

/// What the card (or column) asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum CardIntent {
    /// Send the footer's current text.
    Send(String),
    /// Queue the footer's current text behind the running turn. Reserved for
    /// the host's `⌘↩`-to-queue binding, which reads the same footer input;
    /// the footer's own button reports [`CardIntent::Send`].
    Queue(String),
    /// Open the composer's `+` menu.
    Plus,
    /// Remove the footer page chip.
    ChipRemove(SharedString),
    /// Dock the card into the column.
    Dock,
    /// Close the card.
    Close,
    /// Open the model picker.
    Model,
    /// Pick one question choice, by index.
    Choose(usize),
}

/// The one line saying what the agent is doing now: the last running step's
/// text; when no step runs but steps exist, `Done`; when there are no steps
/// at all, the last answer's first sentence; empty when there is nothing.
pub fn transcript_summary_line(transcript: &AgentTranscript) -> String {
    let mut last_running: Option<&str> = None;
    let mut saw_steps = false;
    for item in &transcript.items {
        if let AgentItem::Steps(steps) = item {
            saw_steps = true;
            for step in steps {
                if step.is_running() {
                    last_running = Some(&step.text);
                }
            }
        }
    }
    if let Some(text) = last_running {
        return text.to_string();
    }
    if saw_steps {
        return "Done".to_string();
    }
    for item in transcript.items.iter().rev() {
        if let AgentItem::Answer(text) = item {
            return first_sentence(text);
        }
    }
    String::new()
}

/// Up to the first `.`, `!`, `?` or newline, trimmed.
fn first_sentence(text: &str) -> String {
    let head = text.find(['.', '!', '?', '\n']).map(|i| &text[..i]).unwrap_or(text);
    let head = head.trim();
    if head.is_empty() { text.trim().to_string() } else { head.to_string() }
}

type LineHandler = Rc<dyn Fn(LineIntent, &mut Window, &mut App)>;
type CardHandler = Rc<dyn Fn(CardIntent, &mut Window, &mut App)>;

/// The `⌘J` line over `state`. Build with [`agent_line`].
#[derive(IntoElement)]
pub struct AgentLine {
    id: ElementId,
    state: LineState,
    input: Entity<TextareaState>,
    on_intent: Option<LineHandler>,
}

/// The `⌘J` line: one 48 px row (a card for `Ask` and `Approval`).
pub fn agent_line(id: impl Into<ElementId>, state: &LineState, input: &Entity<TextareaState>) -> AgentLine {
    AgentLine { id: id.into(), state: state.clone(), input: input.clone(), on_intent: None }
}

impl AgentLine {
    /// Intent handler; every [`LineIntent`] variant reports here.
    pub fn on_intent(mut self, f: impl Fn(LineIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

/// The keyboard the approval card keeps for itself.
struct ApprovalFocus {
    focus: FocusHandle,
    claimed: bool,
}

/// Wraps one intent in a click handler.
fn click(handler: Option<LineHandler>, intent: LineIntent) -> crate::util::ClickHandler {
    Box::new(move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
        if let Some(h) = &handler {
            h(intent.clone(), w, cx);
        }
    })
}

/// Reports the input's current text as [`LineIntent::Send`] at click time.
fn click_send(handler: Option<LineHandler>, input: Entity<TextareaState>) -> crate::util::ClickHandler {
    Box::new(move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
        if let Some(h) = &handler {
            h(LineIntent::Send(input.read(cx).value().to_string()), w, cx);
        }
    })
}

impl RenderOnce for AgentLine {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let handler = self.on_intent.clone();
        match &self.state {
            LineState::Ask { text, chips, site_actions } => {
                ask_card(&id, text, chips, site_actions, &self.input, handler, cx).into_any_element()
            }
            LineState::Driving { status, face } => {
                let take_over = button((id.clone(), "take-over"), "Take over")
                    .trailing(kbd("⌘."))
                    .on_click(click(handler.clone(), LineIntent::TakeOver));
                work_row(&id, *face, div().child(status.clone()), vec![take_over.into_any_element()], handler, &p, cx)
                    .into_any_element()
            }
            LineState::YouDrive => {
                let hand_back = button((id.clone(), "hand-back"), "Hand back")
                    .trailing(kbd("⌘↩"))
                    .on_click(click(handler.clone(), LineIntent::HandBack));
                let label = v_flex()
                    .gap(px(0.0))
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("You're driving"))
                    .child(div().ui(SMALL_TEXT).text_color(p.ink_2).child("The agent is waiting"));
                work_row(&id, TaskFace::Paused, label, vec![hand_back.into_any_element()], handler, &p, cx)
                    .into_any_element()
            }
            LineState::NeedsYou { text } => {
                let stop = button((id.clone(), "stop"), "Stop").on_click(click(handler.clone(), LineIntent::Stop));
                let done = button((id.clone(), "done"), "Done").primary().on_click(click(handler.clone(), LineIntent::Done));
                let label = div().child(format!("Needs you · {text}"));
                work_row(&id, TaskFace::Warning, label, vec![stop.into_any_element(), done.into_any_element()], handler, &p, cx)
                    .into_any_element()
            }
            LineState::Approval(data) => approval_card(&id, data, handler, window, cx).into_any_element(),
        }
    }
}

/// The state face: a 26 px tile tinted by [`TaskFace`].
fn face_tile(id: &ElementId, face: TaskFace, cx: &mut App) -> gpui::AnyElement {
    let p = cx.aui().colors;
    let (bg, glyph): (gpui::Hsla, gpui::AnyElement) = match face {
        TaskFace::Working => (p.accent_soft, crate::data::spinner((id.clone(), "face")).into_any_element()),
        TaskFace::Paused => (p.surface_3, icon(IconName::Clock).size(px(FACE_GLYPH)).color(p.ink_3).into_any_element()),
        TaskFace::Warning => (p.warning_soft, icon(IconName::Bell).size(px(FACE_GLYPH)).color(p.warning).into_any_element()),
    };
    div()
        .flex_none()
        .size(px(FACE))
        .rounded(px(FACE_RADIUS))
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .child(glyph)
        .into_any_element()
}

/// The trailing window chrome shared by the 48 px rows: expand the line into
/// the card. The approval is already a card, so it carries none.
fn expand_button(id: &ElementId, handler: Option<LineHandler>) -> impl IntoElement {
    icon_button((id.clone(), "expand"), IconName::Split)
        .ghost()
        .sm()
        .accessibility_label("Expand")
        .on_click(click(handler, LineIntent::Expand))
}

/// One 48 px working row: face, label, spacer, action buttons, expand.
fn work_row(
    id: &ElementId,
    face: TaskFace,
    label: impl IntoElement,
    actions: Vec<gpui::AnyElement>,
    handler: Option<LineHandler>,
    p: &aui_tokens::Palette,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    h_flex()
        .id(id.clone())
        .w_full()
        .min_w(px(LINE_MIN_W))
        .max_w(px(LINE_MAX_W))
        .h(px(LINE_H))
        .items_center()
        .gap(px(ROW_GAP))
        .px(px(ROW_PAD_X))
        .rounded(px(scale::R_XL))
        .border_1()
        .border_color(p.line)
        .bg(p.overlay)
        .shadow(p.shadow(3))
        .child(face_tile(id, face, cx))
        .child(div().flex_1().min_w(px(0.0)).ui(BODY_TEXT).text_color(p.ink).child(label))
        .children(actions)
        .child(expand_button(id, handler))
}

/// The `Ask` card: the 48 px input row, then the chip row and the site
/// actions below it.
#[allow(clippy::too_many_arguments)]
fn ask_card(
    id: &ElementId,
    text: &SharedString,
    chips: &[ComposerChip],
    site_actions: &[(SharedString, SharedString)],
    input: &Entity<TextareaState>,
    handler: Option<LineHandler>,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let send = icon_button((id.clone(), "send"), IconName::ArrowUp)
        .primary()
        .accessibility_label("Send")
        .on_click(click_send(handler.clone(), input.clone()));
    let row = h_flex()
        .w_full()
        .h(px(LINE_H))
        .items_center()
        .gap(px(ROW_GAP))
        .px(px(ROW_PAD_X))
        .child(icon(IconName::Sparkle).size(px(FACE_GLYPH)).color(p.accent_ink))
        .child(ask_input((id.clone(), "input"), text, input, cx))
        .child(expand_button(id, handler.clone()))
        .child(send);
    let mut card = v_flex()
        .id(id.clone())
        .w_full()
        .min_w(px(LINE_MIN_W))
        .max_w(px(LINE_MAX_W))
        .rounded(px(scale::R_XL))
        .border_1()
        .border_color(p.line)
        .bg(p.overlay)
        .shadow(p.shadow(3))
        .pb(px(ROW_PAD_X))
        .child(row)
        .child(chips_row(id, chips, handler.clone(), cx));
    if !site_actions.is_empty() {
        let mut actions = h_flex().gap(px(ROW_GAP)).flex_wrap();
        for (action_id, label) in site_actions {
            actions = actions.child(
                button((id.clone(), SharedString::from(format!("site-{action_id}"))), label.clone())
                    .ghost()
                    .sm()
                    .on_click(click(handler.clone(), LineIntent::SiteAction(action_id.clone()))),
            );
        }
        card = card.child(
            v_flex()
                .w_full()
                .px(px(ROW_PAD_X))
                .pt(px(ROW_GAP))
                .gap(px(4.0))
                .child(div().ui(CAPTION_TEXT).medium().text_color(p.ink_3).child("ON THIS SITE"))
                .child(actions),
        );
    }
    card
}

/// The single-line input: the composer's borderless text area inside the
/// accessibility frame, so assistive technology sees its name and value.
fn ask_input(id: impl Into<ElementId>, label: &SharedString, input: &Entity<TextareaState>, cx: &mut App) -> gpui::AnyElement {
    let p = cx.aui().colors;
    crate::a11y_text::a11y_text_field(id, label.clone(), input, false, cx)
        .flex_1()
        .min_w(px(0.0))
        .ui(BODY_TEXT)
        .line_height(relative(1.4))
        .text_color(p.ink)
        .child(
            Textarea::new(input)
                .appearance(false)
                .bordered(false)
                .text_size(aui_tokens::scaled(BODY_TEXT))
                .line_height(relative(1.4)),
        )
        .into_any_element()
}

/// The chip row: the given chips with `×`, then the composer's bordered `+`.
fn chips_row(id: &ElementId, chips: &[ComposerChip], handler: Option<LineHandler>, cx: &mut App) -> gpui::AnyElement {
    let p = cx.aui().colors;
    let mut row = h_flex().w_full().flex_wrap().gap(px(6.0)).px(px(ROW_PAD_X)).pt(px(6.0));
    for c in chips {
        let mut el = chip((id.clone(), SharedString::from(format!("chip-{}", c.id))), c.label.clone());
        if c.removable {
            record_ax_label(&format!("Remove {}", c.label));
            el = el.trailing(
                div()
                    .id((id.clone(), SharedString::from(format!("chip-x-{}", c.id))))
                    .cursor_pointer()
                    .role(gpui::Role::Button)
                    .aria_label(SharedString::from(format!("Remove {}", c.label)))
                    .on_click(click(handler.clone(), LineIntent::ChipRemove(c.id.clone())))
                    .child(icon(IconName::X).size(px(CHIP_X)).color(p.ink_4)),
            );
        }
        row = row.child(el);
    }
    row = row.child(
        icon_button((id.clone(), "plus"), IconName::Plus)
            .accessibility_label("Attach or add")
            .on_click(click(handler, LineIntent::Plus)),
    );
    row.into_any_element()
}

/// The approval: the line grown into a card — shield, bold title, quiet
/// pill, the host in mono on the second line, then the one action-row
/// pattern: hint left, spacer, secondary deny, primary allow.
///
/// The card takes the keyboard once when it opens (the settings dialog's
/// pattern): `y` allows, `n` denies through [`ApproveOnce`] and [`Deny`] in
/// [`APPROVAL_CONTEXT`], and escape denies too. The host binds `⌘↩` to allow.
fn approval_card(
    id: &ElementId,
    data: &ApprovalData,
    handler: Option<LineHandler>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let keyboard = window.use_keyed_state((id.clone(), "approval-focus"), cx, |_, cx| ApprovalFocus {
        focus: cx.focus_handle(),
        claimed: false,
    });
    if !keyboard.read(cx).claimed {
        let focus = keyboard.read(cx).focus.clone();
        window.focus(&focus, cx);
        keyboard.update(cx, |s, _| s.claimed = true);
    }
    let focus = keyboard.read(cx).focus.clone();
    let deny = button((id.clone(), "deny"), data.deny.clone()).on_click(click(handler.clone(), LineIntent::Deny));
    let allow =
        button((id.clone(), "allow"), data.allow.clone()).primary().on_click(click(handler.clone(), LineIntent::Allow));
    let head = h_flex()
        .w_full()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .flex_none()
                .size(px(FACE))
                .rounded(px(FACE_RADIUS))
                .bg(p.warning_soft)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(IconName::Shield).size(px(FACE_GLYPH)).color(p.warning)),
        )
        .child(div().flex_1().min_w(px(0.0)).font_weight(gpui::FontWeight::SEMIBOLD).ui(BODY_TEXT).child(data.title.clone()))
        .child(pill(data.kind_pill.clone()));
    v_flex()
        .id(id.clone())
        .key_context(APPROVAL_CONTEXT)
        .track_focus(&focus)
        .on_action({
            let handler = handler.clone();
            move |_: &ApproveOnce, w: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(LineIntent::Allow, w, cx);
                }
            }
        })
        .on_action({
            let handler = handler.clone();
            move |_: &Deny, w: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(LineIntent::Deny, w, cx);
                }
            }
        })
        .on_action({
            let handler = handler.clone();
            move |_: &Cancel, w: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(LineIntent::Deny, w, cx);
                }
            }
        })
        .w_full()
        .min_w(px(LINE_MIN_W))
        .max_w(px(LINE_MAX_W))
        .rounded(px(scale::R_XL))
        .border_1()
        .border_color(p.attention_border(p.warning))
        .bg(p.overlay)
        .shadow(p.shadow(3))
        .p(px(ROW_PAD_X))
        .gap(px(8.0))
        .child(head)
        .child(approval_detail(data, cx))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap(px(ROW_GAP))
                .child(div().ui(SMALL_TEXT).text_color(p.ink_3).child(data.hint.clone()))
                .child(div().flex_1())
                .child(deny)
                .child(allow),
        )
}

/// The approval's second line in `ink-2`, with the host in mono.
fn approval_detail(data: &ApprovalData, cx: &mut App) -> gpui::AnyElement {
    let p = cx.aui().colors;
    let host = data.host.to_string();
    match data.line.strip_prefix(host.as_str()) {
        Some(tail) => h_flex()
            .gap(px(4.0))
            .child(div().mono(SMALL_TEXT).text_color(p.ink_2).child(data.host.clone()))
            .child(div().ui(SMALL_TEXT).text_color(p.ink_2).child(tail.to_string()))
            .into_any_element(),
        None => div().ui(SMALL_TEXT).text_color(p.ink_2).child(data.line.clone()).into_any_element(),
    }
}

/// The `⌘E` card (or the column). Build with [`agent_card`] or [`agent_column`].
#[derive(IntoElement)]
pub struct AgentCard {
    id: ElementId,
    transcript: AgentTranscript,
    input: Entity<TextareaState>,
    size: AgentSize,
    on_intent: Option<CardHandler>,
}

/// The `⌘E` card: 400 × 520, header with `Dock to column`, the shared body,
/// the steering footer.
pub fn agent_card(id: impl Into<ElementId>, transcript: &AgentTranscript, input: &Entity<TextareaState>) -> AgentCard {
    AgentCard { id: id.into(), transcript: transcript.clone(), input: input.clone(), size: AgentSize::Card, on_intent: None }
}

/// The column: the card's body without the frame, sized by the host's right
/// pane, with `Undock` in place of `Dock`.
pub fn agent_column(id: impl Into<ElementId>, transcript: &AgentTranscript, input: &Entity<TextareaState>) -> AgentCard {
    AgentCard { id: id.into(), transcript: transcript.clone(), input: input.clone(), size: AgentSize::Column, on_intent: None }
}

impl AgentCard {
    /// Intent handler; every [`CardIntent`] variant reports here.
    pub fn on_intent(mut self, f: impl Fn(CardIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

/// Wraps one card intent in a click handler.
fn card_click(handler: Option<CardHandler>, intent: CardIntent) -> crate::util::ClickHandler {
    Box::new(move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
        if let Some(h) = &handler {
            h(intent.clone(), w, cx);
        }
    })
}

/// Reports the footer's current text as [`CardIntent::Send`] at click time.
fn card_click_send(handler: Option<CardHandler>, input: Entity<TextareaState>) -> crate::util::ClickHandler {
    Box::new(move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
        if let Some(h) = &handler {
            h(CardIntent::Send(input.read(cx).value().to_string()), w, cx);
        }
    })
}

impl RenderOnce for AgentCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let handler = self.on_intent.clone();
        let header = card_header(&id, &self.transcript, self.size, handler.clone(), window, cx);
        let body = agent_body(&id, &self.transcript, handler.clone(), window, cx);
        let footer = card_footer(&id, &self.input, handler, window, cx);
        match self.size {
            AgentSize::Card => v_flex()
                .id(id)
                .w(px(CARD_W))
                .h(px(CARD_H))
                .rounded(px(scale::R_LG))
                .border_1()
                .border_color(p.line_strong)
                .bg(p.overlay)
                .shadow(p.shadow(3))
                .overflow_hidden()
                .child(header)
                .child(body)
                .child(footer)
                .into_any_element(),
            AgentSize::Column => v_flex()
                .id(id)
                .w_full()
                .h_full()
                .bg(p.overlay)
                .child(header)
                .child(body)
                .child(footer)
                .into_any_element(),
        }
    }
}

/// The header: bold title, the `Model, <name>` button under it, dock (or
/// undock) and close at the right.
fn card_header(
    id: &ElementId,
    transcript: &AgentTranscript,
    size: AgentSize,
    handler: Option<CardHandler>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let p = cx.aui().colors;
    let _ = window;
    let (dock_label, dock_intent) = match size {
        AgentSize::Card => ("Dock to column", CardIntent::Dock),
        AgentSize::Column => ("Undock", CardIntent::Dock),
    };
    h_flex()
        .w_full()
        .items_start()
        .gap(px(ROW_GAP))
        .px(px(ROW_PAD_X))
        .pt(px(ROW_PAD_X))
        .pb(px(ROW_GAP))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(2.0))
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).ui(BODY_TEXT).text_color(p.ink).child(transcript.title.clone()))
                .child(
                    button((id.clone(), "model"), SharedString::from(format!("Model, {}", transcript.model)))
                        .ghost()
                        .sm()
                        .trailing(icon(IconName::ChevronDown).size(px(10.0)).color(p.ink_3))
                        .on_click(card_click(handler.clone(), CardIntent::Model)),
                ),
        )
        .child(
            icon_button((id.clone(), "dock"), IconName::PanelRight)
                .ghost()
                .xs()
                .accessibility_label(dock_label)
                .on_click(card_click(handler.clone(), dock_intent)),
        )
        .child(
            icon_button((id.clone(), "close"), IconName::X)
                .ghost()
                .xs()
                .accessibility_label("Close")
                .on_click(card_click(handler, CardIntent::Close)),
        )
        .into_any_element()
}

/// The body the card and the column share: user bubbles, steps cards,
/// queued chips, answers, questions.
fn agent_body(
    id: &ElementId,
    transcript: &AgentTranscript,
    handler: Option<CardHandler>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let p = cx.aui().colors;
    let _ = window;
    let mut body = v_flex().flex_1().min_h(px(0.0)).w_full().gap(px(10.0)).px(px(ROW_PAD_X)).py(px(ROW_GAP));
    for (i, item) in transcript.items.iter().enumerate() {
        let item_id = (id.clone(), SharedString::from(format!("item-{i}")));
        match item {
            AgentItem::User(text) => {
                body = body.child(user_turn(item_id, text.clone()));
            }
            AgentItem::Steps(steps) => {
                body = body.child(steps_card(item_id, steps));
            }
            AgentItem::Queued(text) => {
                body = body.child(
                    h_flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon(IconName::Clock).size(px(12.0)).color(p.ink_3))
                        .child(div().ui(SMALL_TEXT).text_color(p.ink_2).child(format!("{text} · queued"))),
                );
            }
            AgentItem::Answer(text) => {
                body = body.child(
                    div().w_full().ui(BODY_TEXT).line_height(relative(1.65)).text_color(p.ink).child(text.clone()),
                );
            }
            AgentItem::Question { text, choices } => {
                let mut question =
                    v_flex().w_full().gap(px(6.0)).child(div().ui(BODY_TEXT).text_color(p.ink).child(text.clone()));
                for (n, choice) in choices.iter().enumerate() {
                    question = question.child(
                        button((id.clone(), SharedString::from(format!("item-{i}-choice-{n}"))), choice.clone())
                            .sm()
                            .on_click(card_click(handler.clone(), CardIntent::Choose(n))),
                    );
                }
                body = body.child(question);
            }
        }
    }
    body.into_any_element()
}

/// One steps card, reusing the transcript activity group: ✓/spinner rows
/// under the running step's line (or `Done`).
fn steps_card(id: impl Into<ElementId>, steps: &[Step]) -> impl IntoElement {
    let id = id.into();
    let protocol: Vec<ProtocolStep> = steps
        .iter()
        .map(|s| ProtocolStep {
            verb: s.text.to_string(),
            target: String::new(),
            state: match s.state {
                StepState::Done => ProtocolStepState::Done,
                StepState::Running => ProtocolStepState::Running,
                StepState::Failed => ProtocolStepState::Failed,
            },
            result: None,
        })
        .collect();
    let state = if protocol.iter().any(|s| s.state == ProtocolStepState::Running) {
        ActivityState::Working
    } else if protocol.iter().any(|s| s.state == ProtocolStepState::Failed) {
        ActivityState::Failed
    } else {
        ActivityState::Done
    };
    let summary: SharedString = steps
        .iter()
        .rev()
        .find(|s| s.is_running())
        .map(|s| s.text.clone())
        .unwrap_or_else(|| "Done".into());
    activity_group(id, protocol, summary, "", state).open(true)
}

/// The steering footer: the composer row (`Steer, or ⌘↩ to queue` comes from
/// the input state's own placeholder), the page chip row, send.
fn card_footer(
    id: &ElementId,
    input: &Entity<TextareaState>,
    handler: Option<CardHandler>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let p = cx.aui().colors;
    record_ax_label("Remove This page");
    let page = chip((id.clone(), "page"), "This page").trailing(
        div()
            .id((id.clone(), "page-x"))
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label("Remove This page")
            .on_click({
                let handler = handler.clone();
                move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                    if let Some(h) = &handler {
                        h(CardIntent::ChipRemove("this-page".into()), w, cx);
                    }
                }
            }),
    );
    v_flex()
        .w_full()
        .gap(px(6.0))
        .px(px(ROW_PAD_X))
        .pt(px(ROW_GAP))
        .pb(px(ROW_PAD_X))
        .border_t_1()
        .border_color(p.line)
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap(px(ROW_GAP))
                .child(multiline_input((id.clone(), "steer"), "Steer the agent", input, window, cx))
                .child(
                    icon_button((id.clone(), "send"), IconName::ArrowUp)
                        .primary()
                        .accessibility_label("Send")
                        .on_click(card_click_send(handler.clone(), input.clone())),
                ),
        )
        .child(
            h_flex()
                .gap(px(6.0))
                .child(page)
                .child(
                    icon_button((id.clone(), "plus"), IconName::Plus)
                        .accessibility_label("Attach or add")
                        .on_click(card_click(handler, CardIntent::Plus)),
                ),
        )
        .into_any_element()
}

/// The multi-line steering input with its accessible name.
fn multiline_input(
    id: impl Into<ElementId>,
    label: &str,
    input: &Entity<TextareaState>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let p = cx.aui().colors;
    let _ = window;
    crate::a11y_text::a11y_text_field(id, label, input, true, cx)
        .flex_1()
        .min_w(px(0.0))
        .ui(BODY_TEXT)
        .line_height(relative(1.55))
        .text_color(p.ink)
        .child(
            Textarea::new(input)
                .appearance(false)
                .bordered(false)
                .text_size(aui_tokens::scaled(BODY_TEXT))
                .line_height(relative(1.55)),
        )
        .into_any_element()
}
