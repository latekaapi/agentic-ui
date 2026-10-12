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

use crate::browse::{task_face, TaskFace};
use crate::composer::{ComposerChip, ComposerChipKind};
use crate::data::{button, chip, icon_button, kbd, pill};
use crate::icons::{icon, IconName};
use crate::keys::{APPROVAL_CONTEXT, ApproveAlways, ApproveOnce, Cancel, ChooseNth, Confirm, Deny, MENU_CONTEXT};
use crate::transcript::{activity_group, user_turn};

/// The `⌘J` row height.
const LINE_H: f32 = 48.0;
/// The `⌘J` row measure, 560–640 px wide.
const LINE_MIN_W: f32 = 560.0;
const LINE_MAX_W: f32 = 640.0;
/// The `⌘E` card measure.
const CARD_W: f32 = 400.0;
const CARD_H: f32 = 520.0;
/// The approval shield: the 26 px tile the approval card leads with.
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
/// The leading kind glyph in a context chip, mirroring the composer's row.
const CHIP_GLYPH: f32 = 11.0;

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
    /// The third quiet choice (`Always on this site`), drawn only when set;
    /// the `a` key reports [`LineIntent::AllowAlways`] only then too.
    pub allow_always: Option<String>,
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
    /// The agent is working: face, status, `Take over`. The face is the
    /// task's own [`TaskFace`], drawn with the sidebar's [`task_face`].
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
    /// The working line for the task's `face` with one `status` line: the
    /// row draws the sidebar's [`task_face`] for it.
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
    /// Allow it and remember the rule (`a`, only when the approval offers
    /// the always choice).
    AllowAlways,
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
    /// Context chips for the footer row (`This page` with `×`).
    pub chips: Vec<ComposerChip>,
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
    /// Undock the column back into the card.
    Undock,
    /// Close the card.
    Close,
    /// Open the model picker.
    Model,
    /// Pick one question choice, by index.
    Choose(usize),
}

/// The one line saying what the agent is doing now: the last running step's
/// text; when no step runs but steps exist, `Failed: <step>` when the last
/// step failed, else `Done`; when there are no steps at all, the last
/// answer's first sentence; empty when there is nothing.
pub fn transcript_summary_line(transcript: &AgentTranscript) -> String {
    let mut last_running: Option<&str> = None;
    let mut saw_steps = false;
    let mut last_step: Option<&Step> = None;
    for item in &transcript.items {
        if let AgentItem::Steps(steps) = item {
            saw_steps = true;
            for step in steps {
                last_step = Some(step);
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
        match last_step {
            Some(step) if step.state == StepState::Failed => return format!("Failed: {}", step.text),
            _ => return "Done".to_string(),
        }
    }
    for item in transcript.items.iter().rev() {
        if let AgentItem::Answer(text) = item {
            return first_sentence(text);
        }
    }
    String::new()
}

/// Up to the first sentence break — `. `, `! `, `? ` or a newline — trimmed.
/// A bare `.` never breaks, so decimals (`3.5`) stay whole; a `. ` that only
/// ends an abbreviation (`e.g.`) is not a break either. Text with no break
/// returns whole, trimmed. A break with nothing before it (text starting
/// with a newline) falls back to the trimmed full text, not `""`.
fn first_sentence(text: &str) -> String {
    fn head_or_full(text: &str, end: usize) -> String {
        let head = text[..end].trim();
        if head.is_empty() { text.trim().to_string() } else { head.to_string() }
    }
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            return head_or_full(text, i);
        }
        if (b == b'.' || b == b'!' || b == b'?')
            && i + 1 < bytes.len()
            && bytes[i + 1] == b' '
            && (b != b'.' || !is_abbreviation_dot(bytes, i))
        {
            return head_or_full(text, i);
        }
        i += 1;
    }
    text.trim().to_string()
}

/// Whether the `.` at `dot` only ends an abbreviation (`e.g.`, `i.e.`): the
/// letter before it stands alone — at the start or after a non-letter — so
/// the `. ` after it is not a sentence break.
fn is_abbreviation_dot(bytes: &[u8], dot: usize) -> bool {
    if dot == 0 || !bytes[dot - 1].is_ascii_alphabetic() {
        return false;
    }
    dot == 1 || !bytes[dot - 2].is_ascii_alphabetic()
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

/// The keyboard a card keeps for itself: the approval card's, and the current
/// question block's digit keys.
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

/// Reports the input's current text as [`LineIntent::Send`], unless it is
/// blank: trimmed-empty text emits nothing.
fn send_current_text(handler: &Option<LineHandler>, input: &Entity<TextareaState>, w: &mut Window, cx: &mut App) {
    let trimmed = input.read(cx).value().trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    if let Some(h) = handler {
        h(LineIntent::Send(trimmed), w, cx);
    }
}

/// Reports the input's current text as [`LineIntent::Send`] at click time.
fn click_send(handler: Option<LineHandler>, input: Entity<TextareaState>) -> crate::util::ClickHandler {
    Box::new(move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
        send_current_text(&handler, &input, w, cx);
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
                work_row(&id, *face, div().child(status.clone()), vec![take_over.into_any_element()], handler, &p)
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
                work_row(&id, TaskFace::Paused, label, vec![hand_back.into_any_element()], handler, &p)
                    .into_any_element()
            }
            LineState::NeedsYou { text } => {
                let stop = button((id.clone(), "stop"), "Stop").on_click(click(handler.clone(), LineIntent::Stop));
                let done = button((id.clone(), "done"), "Done").primary().on_click(click(handler.clone(), LineIntent::Done));
                let label = div().child(format!("Needs you · {text}"));
                work_row(&id, TaskFace::NeedsYou, label, vec![stop.into_any_element(), done.into_any_element()], handler, &p)
                    .into_any_element()
            }
            LineState::Approval(data) => approval_card(&id, data, handler, window, cx).into_any_element(),
        }
    }
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

/// One 48 px working row: the sidebar's [`task_face`] for `face`, the
/// label, a spacer, action buttons, expand. The warning colour of
/// [`TaskFace::NeedsYou`] comes from the glyph itself.
fn work_row(
    id: &ElementId,
    face: TaskFace,
    label: impl IntoElement,
    actions: Vec<gpui::AnyElement>,
    handler: Option<LineHandler>,
    p: &aui_tokens::Palette,
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
        .bg(p.overlay)
        .shadow(p.floating_shadow())
        .child(task_face(face))
        .child(div().flex_1().min_w(px(0.0)).ui(BODY_TEXT).text_color(p.ink).child(label))
        .children(actions)
        .child(expand_button(id, handler))
}

/// The `Ask` card: the 48 px input row, then the chip row and the site
/// actions below it.
///
/// Enter in the input sends through [`Confirm`] under [`MENU_CONTEXT`] — the
/// input state's own submit is left unwired, so one Enter yields exactly one
/// [`LineIntent::Send`].
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
    let confirm = {
        let handler = handler.clone();
        let input = input.clone();
        move |_: &Confirm, w: &mut Window, cx: &mut App| {
            send_current_text(&handler, &input, w, cx);
        }
    };
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
        .key_context(MENU_CONTEXT)
        .on_action(confirm)
        .w_full()
        .min_w(px(LINE_MIN_W))
        .max_w(px(LINE_MAX_W))
        .rounded(px(scale::R_XL))
        .bg(p.overlay)
        .shadow(p.floating_shadow())
        .pb(px(ROW_PAD_X))
        .child(row)
        .child(chips_row(id, chips, {
            let handler = handler.clone();
            move |chip_id: SharedString| click(handler.clone(), LineIntent::ChipRemove(chip_id))
        }, click(handler.clone(), LineIntent::Plus), cx));
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

/// The context-chip row the Ask line and the card footer share: one chip per
/// entry, then the composer's bordered `+`.
///
/// Each chip draws the way the composer's own row draws it (its chips row is
/// inline in `composer.rs`, so this mirrors it): the kind's leading glyph —
/// `@`, `$`, file, image — the file's detail suffix, and a real remove `×`
/// (button role, `Remove <label>`, [`IconName::X`]) reporting through
/// `on_remove`. The `+` reports through `on_plus`.
fn chips_row(
    id: &ElementId,
    chips: &[ComposerChip],
    on_remove: impl Fn(SharedString) -> crate::util::ClickHandler,
    on_plus: crate::util::ClickHandler,
    cx: &mut App,
) -> gpui::AnyElement {
    let p = cx.aui().colors;
    let mut row = h_flex().w_full().flex_wrap().gap(px(6.0)).px(px(ROW_PAD_X)).pt(px(6.0));
    for c in chips {
        let leading: gpui::AnyElement = match c.kind {
            ComposerChipKind::Mention => div()
                .mono(scale::FS_12)
                .line_height(relative(1.0))
                .text_color(p.accent_ink)
                .child("@")
                .into_any_element(),
            ComposerChipKind::Skill => div()
                .mono(scale::FS_12)
                .line_height(relative(1.0))
                .text_color(p.accent_ink)
                .child("$")
                .into_any_element(),
            ComposerChipKind::Image => icon(IconName::Image).size(px(CHIP_GLYPH)).into_any_element(),
            ComposerChipKind::File => icon(IconName::File).size(px(CHIP_GLYPH)).into_any_element(),
        };
        let mut el = chip((id.clone(), SharedString::from(format!("chip-{}", c.id))), c.label.clone()).leading(leading);
        if c.kind == ComposerChipKind::File {
            if let Some(detail) = &c.detail {
                el = el.detail(detail.clone());
            }
        }
        if c.removable {
            el = el.trailing(
                icon_button((id.clone(), SharedString::from(format!("chip-x-{}", c.id))), IconName::X)
                    .ghost()
                    .xs()
                    .icon_size(px(CHIP_X))
                    .accessibility_label(SharedString::from(format!("Remove {}", c.label)))
                    .on_click(on_remove(c.id.clone())),
            );
        }
        row = row.child(el);
    }
    row = row.child(
        icon_button((id.clone(), "plus"), IconName::Plus)
            .accessibility_label("Attach or add")
            .on_click(on_plus),
    );
    row.into_any_element()
}

/// The approval: the line grown into a card — shield, bold title, quiet
/// pill, the host in mono on the second line, then the one action-row
/// pattern: hint left, spacer, secondary deny, primary allow.
///
/// The card takes the keyboard once when it opens (the settings dialog's
/// pattern): `y` allows, `n` denies through [`ApproveOnce`] and [`Deny`] in
/// [`APPROVAL_CONTEXT`], and escape denies too. When the approval offers the
/// always choice, `a` reports [`LineIntent::AllowAlways`] through
/// [`ApproveAlways`]; otherwise nothing claims that key. The host binds `⌘↩`
/// to allow.
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
    let always: Option<gpui::AnyElement> = data.allow_always.as_ref().map(|label| {
        button((id.clone(), "allow-always"), SharedString::from(label.clone()))
            .ghost()
            .on_click(click(handler.clone(), LineIntent::AllowAlways))
            .into_any_element()
    });
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
    let mut card = v_flex()
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
        });
    if data.allow_always.is_some() {
        card = card.on_action({
            let handler = handler.clone();
            move |_: &ApproveAlways, w: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(LineIntent::AllowAlways, w, cx);
                }
            }
        });
    }
    card.w_full()
        .min_w(px(LINE_MIN_W))
        .max_w(px(LINE_MAX_W))
        .rounded(px(scale::R_XL))
        .border_1()
        .border_color(p.attention_border(p.warning))
        .bg(p.overlay)
        .shadow(p.floating_shadow())
        .p(px(ROW_PAD_X))
        .gap(px(8.0))
        .child(head)
        .child(approval_detail(data, cx))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap(px(ROW_GAP))
                .child(div().min_w(px(0.0)).truncate().ui(SMALL_TEXT).text_color(p.ink_3).child(data.hint.clone()))
                .child(div().flex_1())
                .children(always)
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

/// Reports the footer's current text as [`CardIntent::Send`] at click time,
/// unless it is blank: trimmed-empty text emits nothing.
fn card_click_send(handler: Option<CardHandler>, input: Entity<TextareaState>) -> crate::util::ClickHandler {
    Box::new(move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
        let trimmed = input.read(cx).value().trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        if let Some(h) = &handler {
            h(CardIntent::Send(trimmed), w, cx);
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
        let footer = card_footer(&id, &self.transcript, &self.input, handler.clone(), window, cx);
        let shell = match self.size {
            AgentSize::Card => v_flex()
                .id(id.clone())
                .w(px(CARD_W))
                .h(px(CARD_H))
                .rounded(px(scale::R_LG))
                .bg(p.overlay)
                .shadow(p.floating_shadow())
                .overflow_hidden()
                .child(header)
                .child(body)
                .child(footer),
            AgentSize::Column => v_flex()
                .id(id.clone())
                .w_full()
                .h_full()
                .bg(p.overlay)
                .child(header)
                .child(body)
                .child(footer),
        };
        shell.into_any_element()
    }
}

/// The header: bold title, the model button left-aligned under it, dock (or
/// undock) and close at the right. The model button reads `<model> ▾` while
/// its accessible name stays `Model, <model>`.
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
    let (dock_label, dock_icon, dock_intent) = match size {
        AgentSize::Card => ("Dock to column", IconName::PanelRight, CardIntent::Dock),
        AgentSize::Column => ("Undock", IconName::Layout, CardIntent::Undock),
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
                .items_start()
                .gap(px(2.0))
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).ui(BODY_TEXT).text_color(p.ink).child(transcript.title.clone()))
                .child(
                    button((id.clone(), "model"), transcript.model.clone())
                        .ghost()
                        .sm()
                        .trailing(icon(IconName::ChevronDown).size(px(10.0)).color(p.ink_3))
                        .accessibility_label(SharedString::from(format!("Model, {}", transcript.model)))
                        .on_click(card_click(handler.clone(), CardIntent::Model)),
                ),
        )
        .child(
            icon_button((id.clone(), "dock"), dock_icon)
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
/// queued chips, answers, questions. The body scrolls while the steering
/// footer stays pinned.
///
/// Digits pick the current question's choices through [`ChooseNth`] in
/// [`APPROVAL_CONTEXT`], the approval card's pattern: the context lives on
/// the question block itself — never on an ancestor of the steering input —
/// so digits typed into the focused input stay text, while digits with the
/// input unfocused pick the choice.
fn agent_body(
    id: &ElementId,
    transcript: &AgentTranscript,
    handler: Option<CardHandler>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let p = cx.aui().colors;
    let last = transcript.items.len().saturating_sub(1);
    let mut body = v_flex()
        .id((id.clone(), "body"))
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .w_full()
        .gap(px(10.0))
        .px(px(ROW_PAD_X))
        .py(px(ROW_GAP));
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
                let mut question = v_flex()
                    .id(item_id)
                    .w_full()
                    .gap(px(6.0))
                    .child(div().ui(BODY_TEXT).text_color(p.ink).child(text.clone()));
                for (n, choice) in choices.iter().enumerate() {
                    question = question.child(
                        button((id.clone(), SharedString::from(format!("item-{i}-choice-{n}"))), choice.clone())
                            .sm()
                            .on_click(card_click(handler.clone(), CardIntent::Choose(n))),
                    );
                }
                // Only the current question (the last item) owns the digit
                // keys, and only while it has choices to pick from.
                if i == last && !choices.is_empty() {
                    let count = choices.len();
                    let keyboard = window.use_keyed_state((id.clone(), "question-focus"), cx, |_, cx| ApprovalFocus {
                        focus: cx.focus_handle(),
                        claimed: false,
                    });
                    if !keyboard.read(cx).claimed {
                        // A question arriving mid-steer must not pull the
                        // caret out of the input: claim the keyboard only
                        // while nothing else holds it.
                        let free = window.focused(cx).is_none();
                        let focus = keyboard.read(cx).focus.clone();
                        if free {
                            window.focus(&focus, cx);
                        }
                        keyboard.update(cx, |s, _| s.claimed = true);
                    }
                    let focus = keyboard.read(cx).focus.clone();
                    question = question
                        .key_context(APPROVAL_CONTEXT)
                        .track_focus(&focus)
                        .on_action({
                            let handler = handler.clone();
                            move |action: &ChooseNth, w: &mut Window, cx: &mut App| {
                                if action.index < count {
                                    if let Some(h) = &handler {
                                        h(CardIntent::Choose(action.index), w, cx);
                                    }
                                }
                            }
                        });
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
/// the input state's own placeholder), the transcript's chip row, send. The
/// chips are the same shared row the Ask line draws.
fn card_footer(
    id: &ElementId,
    transcript: &AgentTranscript,
    input: &Entity<TextareaState>,
    handler: Option<CardHandler>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .gap(px(6.0))
        .pt(px(ROW_GAP))
        .pb(px(ROW_PAD_X))
        .border_t_1()
        .border_color(p.line)
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap(px(ROW_GAP))
                .px(px(ROW_PAD_X))
                .child(multiline_input((id.clone(), "steer"), "Steer the agent", input, window, cx))
                .child(
                    icon_button((id.clone(), "send"), IconName::ArrowUp)
                        .primary()
                        .accessibility_label("Send")
                        .on_click(card_click_send(handler.clone(), input.clone())),
                ),
        )
        .child(chips_row(id, &transcript.chips, {
            let handler = handler.clone();
            move |chip_id: SharedString| card_click(handler.clone(), CardIntent::ChipRemove(chip_id))
        }, card_click(handler, CardIntent::Plus), cx))
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
