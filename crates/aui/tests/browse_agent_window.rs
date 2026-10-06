//! P3: the agent window — the line's five states, the summary line, and the
//! approval keys.
//!
//! Each [`LineState`] must render its action labels (the gate's proof the
//! states draw), [`transcript_summary_line`] follows its table, and the
//! approval's `y` / `n` / escape keystrokes must report [`LineIntent::Allow`]
//! / [`LineIntent::Deny`] through the component's own intent handler.

use std::cell::RefCell;
use std::rc::Rc;

use aui::browse::agent_window::{
    agent_line, AgentItem, AgentTranscript, ApprovalData, LineIntent, LineState, Step, StepState, TaskFace,
    transcript_summary_line,
};
use aui::composer::{composer_state, ComposerChip, ComposerChipKind};
use aui::transcript::{arm_ax_probe, take_ax_labels};
use gpui::{IntoElement, SharedString, TestAppContext};

/// What the line reported, in order.
type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
}

/// The host: one line over `state`, every intent logged.
struct LineHost {
    state: LineState,
    log: Log,
}

impl gpui::Render for LineHost {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let text = window.use_keyed_state("browse-test-text", cx, |window, cx| {
            composer_state("Ask about this page", window, cx)
        });
        let log = self.log.clone();
        agent_line("test-line", &self.state, &text).on_intent(move |intent, _, _| {
            log.borrow_mut().push(format!("{intent:?}"));
        })
    }
}

/// Draws one [`LineState`] once in a real window and returns the effective AX
/// labels the render committed.
fn drawn_labels(cx: &mut TestAppContext, state: LineState) -> Vec<String> {
    init(cx);
    arm_ax_probe(true);
    take_ax_labels();
    let (_host, _) = cx.add_window_view({
        let log = log();
        move |_, _| LineHost { state, log }
    });
    let labels = take_ax_labels();
    arm_ax_probe(false);
    labels
}

fn page_chip() -> ComposerChip {
    ComposerChip {
        id: "this-page".into(),
        kind: ComposerChipKind::File,
        label: "This page".into(),
        removable: true,
        thumbnail: None,
        detail: None,
    }
}

fn ask_state() -> LineState {
    LineState::Ask {
        text: "Ask about this page".into(),
        chips: vec![page_chip()],
        site_actions: vec![("sum".into(), "Sum it up".into()), ("extract-table".into(), "Extract the table".into())],
    }
}

fn approval_state() -> LineState {
    LineState::Approval(ApprovalData {
        title: "Claude wants to download udise_2024_25_state_indicators.xlsx".into(),
        host: "data.gov.in".into(),
        line: "data.gov.in · 14 MB · to ~/Downloads/pgi-compare".into(),
        kind_pill: "Download".into(),
        hint: "Always asks before downloading".into(),
        deny: "Don't allow".into(),
        allow: "Download".into(),
    })
}

/// The idle line offers send, the `+` menu, chip removal and site actions.
#[gpui::test]
fn ask_renders_its_labels(cx: &mut TestAppContext) {
    let labels = drawn_labels(cx, ask_state());
    for want in ["Send", "Attach or add", "Remove This page", "Sum it up", "Extract the table", "Expand"] {
        assert!(labels.iter().any(|l| l == want), "the Ask line must label {want:?}, got {labels:?}");
    }
}

/// The driving line offers taking over.
#[gpui::test]
fn driving_renders_its_labels(cx: &mut TestAppContext) {
    let state = LineState::from_task(TaskFace::Working, "Claude is reading UDISE+ · dropout");
    let labels = drawn_labels(cx, state);
    for want in ["Take over", "Expand"] {
        assert!(labels.iter().any(|l| l == want), "the Driving line must label {want:?}, got {labels:?}");
    }
}

/// The parked line offers handing back.
#[gpui::test]
fn you_drive_renders_its_labels(cx: &mut TestAppContext) {
    let labels = drawn_labels(cx, LineState::YouDrive);
    for want in ["Hand back", "Expand"] {
        assert!(labels.iter().any(|l| l == want), "the YouDrive line must label {want:?}, got {labels:?}");
    }
}

/// The blocked line offers stopping and finishing.
#[gpui::test]
fn needs_you_renders_its_labels(cx: &mut TestAppContext) {
    let labels = drawn_labels(cx, LineState::NeedsYou { text: "data.gov.in asks you to sign in".into() });
    for want in ["Stop", "Done", "Expand"] {
        assert!(labels.iter().any(|l| l == want), "the NeedsYou line must label {want:?}, got {labels:?}");
    }
}

/// The approval offers denying and allowing, with the spec's exact labels.
#[gpui::test]
fn approval_renders_its_labels(cx: &mut TestAppContext) {
    let labels = drawn_labels(cx, approval_state());
    for want in ["Don't allow", "Download"] {
        assert!(labels.iter().any(|l| l == want), "the approval must label {want:?}, got {labels:?}");
    }
}

fn transcript_with(items: Vec<AgentItem>) -> AgentTranscript {
    AgentTranscript { title: "t".into(), model: "m".into(), items }
}

/// The last running step wins, in order.
#[test]
fn summary_prefers_the_last_running_step() {
    let transcript = transcript_with(vec![AgentItem::Steps(vec![
        Step::new("Opened PGI 2.0 state scores (2023-24)", StepState::Done),
        Step::new("Reading dropout rates, secondary", StepState::Running),
        Step::new("Reading PTR for 5 states", StepState::Running),
    ])]);
    assert_eq!(transcript_summary_line(&transcript), "Reading PTR for 5 states");
}

/// Settled steps with nothing running read `Done`, even past an answer.
#[test]
fn summary_reads_done_when_steps_settle() {
    let transcript = transcript_with(vec![
        AgentItem::Answer("Karnataka leads on PTR.".into()),
        AgentItem::Steps(vec![Step::new("Opened PGI 2.0 state scores (2023-24)", StepState::Done)]),
    ]);
    assert_eq!(transcript_summary_line(&transcript), "Done");
}

/// Failed steps are settled too: still `Done`, never the failure text.
#[test]
fn summary_reads_done_when_steps_fail() {
    let transcript = transcript_with(vec![AgentItem::Steps(vec![
        Step::new("Reading dropout rates, secondary", StepState::Failed),
    ])]);
    assert_eq!(transcript_summary_line(&transcript), "Done");
}

/// With no steps at all, the last answer's first sentence speaks.
#[test]
fn summary_falls_back_to_the_last_answer() {
    let transcript = transcript_with(vec![
        AgentItem::Answer("Karnataka leads on PTR. Tamil Nadu follows.".into()),
        AgentItem::Queued("also add girls' GER".into()),
        AgentItem::Answer("Gujarat improved most since 2021! The rest is flat.".into()),
    ]);
    assert_eq!(transcript_summary_line(&transcript), "Gujarat improved most since 2021");
}

/// Nothing to summarise is the empty string, not a stale line.
#[test]
fn summary_is_empty_for_an_empty_transcript() {
    let transcript = transcript_with(vec![]);
    assert_eq!(transcript_summary_line(&transcript), "");
}

/// A question never poses as the summary.
#[test]
fn summary_ignores_questions() {
    let transcript = transcript_with(vec![AgentItem::Question {
        text: "Which states?".into(),
        choices: vec!["Five southern".into()],
    }]);
    assert_eq!(transcript_summary_line(&transcript), "");
}

/// `from_task` builds the driving line; only `Running` runs.
#[test]
fn task_helpers() {
    let state = LineState::from_task(TaskFace::Working, "reading");
    assert_eq!(
        state,
        LineState::Driving { status: SharedString::from("reading"), face: TaskFace::Working }
    );
    assert!(Step::new("x", StepState::Running).is_running());
    assert!(!Step::new("x", StepState::Done).is_running());
    assert!(!Step::new("x", StepState::Failed).is_running());
}

/// `y` allows and `n` denies through the component's own intent handler: the
/// approval card owns `APPROVAL_CONTEXT` with [`ApproveOnce`]-to-allow and
/// [`Deny`]-to-deny, taking the keyboard once when it opens.
#[gpui::test]
fn approval_keys_emit_allow_and_deny(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |_, _| LineHost { state: approval_state(), log }
    });
    cx.simulate_keystrokes("y");
    cx.simulate_keystrokes("n");
    assert_eq!(&*log.borrow(), &["Allow".to_string(), "Deny".to_string()]);
}

/// Escape denies too: the card maps the context's dismiss keystroke to deny.
#[gpui::test]
fn approval_escape_denies(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |_, _| LineHost { state: approval_state(), log }
    });
    cx.simulate_keystrokes("escape");
    assert_eq!(&*log.borrow(), &["Deny".to_string()]);
}

/// Clicking allow reports [`LineIntent::Allow`] with its debug name stable.
#[test]
fn line_intent_debug_names_are_stable() {
    assert_eq!(format!("{:?}", LineIntent::TakeOver), "TakeOver");
    assert_eq!(format!("{:?}", LineIntent::HandBack), "HandBack");
    assert_eq!(format!("{:?}", LineIntent::ChipRemove("this-page".into())), "ChipRemove(\"this-page\")");
}
