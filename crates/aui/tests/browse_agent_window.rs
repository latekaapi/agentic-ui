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
    agent_card, agent_column, agent_line, AgentItem, AgentTranscript, ApprovalData, CardIntent, LineIntent,
    LineState, Step, StepState, transcript_summary_line,
};
use aui::browse::{task_face, TaskFace};
use aui::composer::{composer_state, composer_state_rows, ComposerChip, ComposerChipKind};
use aui::transcript::{arm_ax_probe, take_ax_labels};
use gpui::{point, px, AppContext, Entity, IntoElement, Modifiers, ParentElement, SharedString, Styled, TestAppContext, VisualTestContext};
use gpui_kit::base::input::TextareaState;

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
        allow_always: None,
    })
}

/// The same approval offering the always choice.
fn approval_state_always() -> LineState {
    LineState::Approval(ApprovalData {
        title: "Claude wants to download udise_2024_25_state_indicators.xlsx".into(),
        host: "data.gov.in".into(),
        line: "data.gov.in · 14 MB · to ~/Downloads/pgi-compare".into(),
        kind_pill: "Download".into(),
        hint: "Always asks before downloading".into(),
        deny: "Don't allow".into(),
        allow: "Download".into(),
        allow_always: Some("Always on this site".to_string()),
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
    let state = LineState::from_task(TaskFace::Reading, "Claude is reading UDISE+ · dropout");
    let labels = drawn_labels(cx, state);
    for want in ["Take over", "Expand"] {
        assert!(labels.iter().any(|l| l == want), "the Driving line must label {want:?}, got {labels:?}");
    }
}

/// The Driving line draws the sidebar's own glyph element for its face: the
/// same [`task_face`] the task-folder rows use, which carries no AX label of
/// its own.
#[test]
fn driving_uses_the_sidebar_glyph() {
    for face in [TaskFace::Reading, TaskFace::Typing, TaskFace::Paused, TaskFace::NeedsYou, TaskFace::Done] {
        assert_eq!(task_face(face).face(), face, "the line's glyph must be the sidebar's glyph for {face:?}");
    }
    let state = LineState::from_task(TaskFace::Typing, "typing");
    assert_eq!(state, LineState::Driving { status: SharedString::from("typing"), face: TaskFace::Typing });
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
    AgentTranscript { title: "t".into(), model: "m".into(), chips: Vec::new(), items }
}

/// A small card transcript with the page chip, for the click sweeps.
fn card_transcript() -> AgentTranscript {
    AgentTranscript {
        title: "PGI vs school indicators".into(),
        model: "Claude Sonnet".into(),
        chips: vec![page_chip()],
        items: vec![AgentItem::User("compare PGI grades".into())],
    }
}

/// A transcript whose last item is a question waiting on the person.
fn question_transcript() -> AgentTranscript {
    AgentTranscript {
        title: "t".into(),
        model: "m".into(),
        chips: Vec::new(),
        items: vec![AgentItem::Question {
            text: "Which year for PTR?".into(),
            choices: vec!["2023-24".into(), "2024-25".into()],
        }],
    }
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

/// A transcript whose last step failed summarises as `Failed: <step>`, not
/// `Done`: the failure is what the person must see.
#[test]
fn summary_reports_the_failed_step() {
    let transcript = transcript_with(vec![AgentItem::Steps(vec![
        Step::new("Opened PGI 2.0 state scores (2023-24)", StepState::Done),
        Step::new("Reading dropout rates, secondary", StepState::Failed),
    ])]);
    assert_eq!(
        transcript_summary_line(&transcript),
        "Failed: Reading dropout rates, secondary"
    );
}

/// A running step still wins over an earlier failure.
#[test]
fn summary_prefers_running_over_failed() {
    let transcript = transcript_with(vec![AgentItem::Steps(vec![
        Step::new("Reading dropout rates, secondary", StepState::Failed),
        Step::new("Reading PTR for 5 states", StepState::Running),
    ])]);
    assert_eq!(transcript_summary_line(&transcript), "Reading PTR for 5 states");
}

/// The answer fallback's first sentence breaks on `. `, `! `, `? ` or a
/// newline — never on a bare `.`, so decimals and abbreviations stay whole.
/// Each row goes through [`transcript_summary_line`], the public surface that
/// owns the rule.
#[test]
fn first_sentence_breaks_only_on_sentence_bounds() {
    let cases = [
        ("Gujarat improved 3.5 points.", "Gujarat improved 3.5 points."),
        ("e.g. Kerala", "e.g. Kerala"),
        ("Visit e.g. Kerala soon. It is nice.", "Visit e.g. Kerala soon"),
        ("Karnataka leads on PTR. Tamil Nadu follows.", "Karnataka leads on PTR"),
        ("Gujarat improved most since 2021! The rest is flat.", "Gujarat improved most since 2021"),
        ("Which year for PTR? 2023-24 or 2024-25?", "Which year for PTR"),
        ("Top gainers\nthe rest is flat.", "Top gainers"),
        ("\nthe rest is flat.", "the rest is flat."),
        ("No break here", "No break here"),
    ];
    for (input, want) in cases {
        let transcript = transcript_with(vec![AgentItem::Answer(input.into())]);
        assert_eq!(transcript_summary_line(&transcript), want, "first sentence of {input:?}");
    }
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
    let state = LineState::from_task(TaskFace::Reading, "reading");
    assert_eq!(
        state,
        LineState::Driving { status: SharedString::from("reading"), face: TaskFace::Reading }
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
    assert_eq!(format!("{:?}", LineIntent::AllowAlways), "AllowAlways");
    assert_eq!(format!("{:?}", LineIntent::ChipRemove("this-page".into())), "ChipRemove(\"this-page\")");
    assert_eq!(format!("{:?}", CardIntent::Dock), "Dock");
    assert_eq!(format!("{:?}", CardIntent::Undock), "Undock");
    assert_eq!(format!("{:?}", CardIntent::Choose(1)), "Choose(1)");
}

// ------------------------------------------------------- click and key proof
//
// Headless windows have no rasterizer, so what a test can prove is intent:
// a click sweep over the component can only log through a real clickable
// control, and keystrokes only log through the component's own handlers.

/// What the card or column reported, in order.
type CardLog = Rc<RefCell<Vec<String>>>;

/// The card (or the column) over `transcript`, every intent logged.
struct CardHost {
    transcript: AgentTranscript,
    input: Entity<TextareaState>,
    log: CardLog,
    column: bool,
}

impl CardHost {
    /// Builds the host with a fresh steering input, for `add_window_view`.
    fn build(
        transcript: AgentTranscript,
        log: CardLog,
        column: bool,
    ) -> impl FnOnce(&mut gpui::Window, &mut gpui::Context<CardHost>) -> CardHost {
        move |window, cx| {
            let input = cx.new(|cx| composer_state_rows("Steer, or ⌘↩ to queue", 1, 4, window, cx));
            CardHost { transcript, input, log, column }
        }
    }
}

impl gpui::Render for CardHost {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let log = self.log.clone();
        if self.column {
            // The column is host-sized; pin it to a fixed box so the sweeps
            // know where the header and the bottom-anchored footer are.
            gpui::div()
                .w(px(420.0))
                .h(px(640.0))
                .child(agent_column("test-card", &self.transcript, &self.input).on_intent(move |intent, _, _| {
                    log.borrow_mut().push(format!("{intent:?}"));
                }))
                .into_any_element()
        } else {
            agent_card("test-card", &self.transcript, &self.input)
                .on_intent(move |intent, _, _| {
                    log.borrow_mut().push(format!("{intent:?}"));
                })
                .into_any_element()
        }
    }
}

/// Clicks every point of the grid over `vcx`'s window.
fn sweep(vcx: &mut VisualTestContext, x0: f32, x1: f32, y0: f32, y1: f32, step_x: f32, step_y: f32) {
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 {
            vcx.simulate_click(point(px(x), px(y)), Modifiers::none());
            x += step_x;
        }
        vcx.run_until_parked();
        y += step_y;
    }
}

/// Sweeps one line state and returns what it reported.
fn sweep_line(cx: &mut TestAppContext, state: LineState) -> Vec<String> {
    init(cx);
    let log = log();
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        move |_, _| LineHost { state, log }
    });
    let vcx: &mut VisualTestContext = vcx;
    sweep(vcx, 2.0, 638.0, 2.0, 170.0, 8.0, 6.0);
    let entries = log.borrow().clone();
    entries
}

/// Clicking the approval's buttons reports allow and deny; with no always
/// choice there is no third button to hit.
#[gpui::test]
fn approval_clicks_report_allow_and_deny(cx: &mut TestAppContext) {
    let entries = sweep_line(cx, approval_state());
    assert!(entries.iter().any(|e| e == "Allow"), "an Allow click must report Allow, got {entries:?}");
    assert!(entries.iter().any(|e| e == "Deny"), "a Deny click must report Deny, got {entries:?}");
    assert!(
        !entries.iter().any(|e| e == "AllowAlways"),
        "with no always choice there is no Always button to hit, got {entries:?}"
    );
}

/// The always choice only draws when offered, and clicking it reports
/// [`LineIntent::AllowAlways`].
#[gpui::test]
fn approval_always_click_reports_allow_always(cx: &mut TestAppContext) {
    let entries = sweep_line(cx, approval_state_always());
    assert!(
        entries.iter().any(|e| e == "AllowAlways"),
        "an Always click must report AllowAlways, got {entries:?}"
    );
}

/// `a` reports [`LineIntent::AllowAlways`] when the choice is offered, and
/// nothing when it is not: the key is unclaimed without the choice.
#[gpui::test]
fn approval_a_key_needs_the_always_choice(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |_, _| LineHost { state: approval_state_always(), log }
    });
    cx.simulate_keystrokes("a");
    assert_eq!(&*log.borrow(), &["AllowAlways".to_string()]);

    let log2: Log = Rc::new(RefCell::new(Vec::new()));
    let (_host2, cx2) = cx.add_window_view({
        let log = log2.clone();
        |_, _| LineHost { state: approval_state(), log }
    });
    cx2.simulate_keystrokes("a");
    assert!(log2.borrow().is_empty(), "`a` with no always choice must emit nothing, got {:?}", log2.borrow());
}

/// Clicking the driving row's button reports taking over.
#[gpui::test]
fn driving_click_reports_take_over(cx: &mut TestAppContext) {
    let state = LineState::from_task(TaskFace::Reading, "Claude is reading UDISE+ · dropout");
    let entries = sweep_line(cx, state);
    assert!(entries.iter().any(|e| e == "TakeOver"), "a Take over click must report TakeOver, got {entries:?}");
}

/// Clicking the blocked row's buttons reports stopping and finishing.
#[gpui::test]
fn needs_you_clicks_report_stop_and_done(cx: &mut TestAppContext) {
    let state = LineState::NeedsYou { text: "data.gov.in asks you to sign in".into() };
    let entries = sweep_line(cx, state);
    assert!(entries.iter().any(|e| e == "Stop"), "a Stop click must report Stop, got {entries:?}");
    assert!(entries.iter().any(|e| e == "Done"), "a Done click must report Done, got {entries:?}");
}

/// Typing into the Ask input and pressing Enter sends exactly once: the
/// input's own submit stays unwired and [`Confirm`] is the one path.
#[gpui::test]
fn one_enter_sends_once(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        |_, _| LineHost { state: ask_state(), log }
    });
    // The input's left half, away from every button.
    vcx.simulate_click(point(px(200.0), px(24.0)), Modifiers::none());
    assert!(log.borrow().is_empty(), "focusing the input must emit nothing, got {:?}", log.borrow());
    vcx.simulate_input("hello");
    vcx.simulate_keystrokes("enter");
    assert_eq!(&*log.borrow(), &["Send(\"hello\")".to_string()], "one Enter must yield exactly one Send");
}

/// Enter (and the send button) with blank text emits no [`LineIntent::Send`].
#[gpui::test]
fn send_is_not_emitted_for_empty_text(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        |_, _| LineHost { state: ask_state(), log }
    });
    vcx.simulate_click(point(px(200.0), px(24.0)), Modifiers::none());
    vcx.simulate_keystrokes("enter");
    assert!(log.borrow().is_empty(), "Enter on empty text must emit nothing, got {:?}", log.borrow());
    vcx.simulate_keystrokes("space space space");
    vcx.simulate_keystrokes("enter");
    assert!(log.borrow().is_empty(), "Enter on blank text must emit nothing, got {:?}", log.borrow());
    // The sweep below hits the send button too: still no Send.
    let vcx: &mut VisualTestContext = vcx;
    sweep(vcx, 2.0, 638.0, 2.0, 170.0, 8.0, 6.0);
    assert!(
        !log.borrow().iter().any(|e| e.starts_with("Send")),
        "no sweep click may send blank text, got {:?}",
        log.borrow()
    );
}

/// The card draws its labels: the chip's remove name, the model name as
/// `Model, <model>`, and the dock control. Every label — the chip `×`
/// included — comes from a rendered element, never a bare record.
#[gpui::test]
fn card_renders_chip_model_and_dock_labels(cx: &mut TestAppContext) {
    let labels = drawn_card_labels(cx, card_transcript(), false);
    assert!(!labels.is_empty(), "the card draws controls at all");
    assert_eq!(
        labels.iter().filter(|l| l.is_empty()).count(),
        0,
        "every card control names itself from its rendered element, got {labels:?}"
    );
    for want in ["Remove This page", "Model, Claude Sonnet", "Dock to column", "Close"] {
        assert!(labels.iter().any(|l| l == want), "the card must label {want:?}, got {labels:?}");
    }
}

/// The column names undocking instead.
#[gpui::test]
fn column_renders_undock_label(cx: &mut TestAppContext) {
    let labels = drawn_card_labels(cx, card_transcript(), true);
    assert!(labels.iter().any(|l| l == "Undock"), "the column must label Undock, got {labels:?}");
    assert!(labels.iter().any(|l| l == "Remove This page"), "the column footer keeps the chip, got {labels:?}");
}

/// Draws one card (or column) once in a real window and returns the
/// effective AX labels the render committed.
fn drawn_card_labels(cx: &mut TestAppContext, transcript: AgentTranscript, column: bool) -> Vec<String> {
    init(cx);
    arm_ax_probe(true);
    take_ax_labels();
    let (_host, _) = cx.add_window_view(CardHost::build(transcript, CardLog::default(), column));
    let labels = take_ax_labels();
    arm_ax_probe(false);
    labels
}

/// Clicking the card's chip `×` reports [`CardIntent::ChipRemove`], and
/// docking reports [`CardIntent::Dock`]. The steering input is empty, so no
/// sweep click may send.
#[gpui::test]
fn card_clicks_report_chip_remove_and_dock(cx: &mut TestAppContext) {
    init(cx);
    let log: CardLog = Rc::new(RefCell::new(Vec::new()));
    let (_host, vcx) = cx.add_window_view(CardHost::build(card_transcript(), log.clone(), false));
    let vcx: &mut VisualTestContext = vcx;
    // The 400 × 520 card sits at the window's top-left.
    sweep(vcx, 2.0, 398.0, 2.0, 518.0, 6.0, 5.0);
    let entries = log.borrow().clone();
    assert!(
        entries.iter().any(|e| e == "ChipRemove(\"this-page\")"),
        "the card's × must report ChipRemove, got {entries:?}"
    );
    assert!(entries.iter().any(|e| e == "Dock"), "a Dock click must report Dock, got {entries:?}");
    assert!(
        !entries.iter().any(|e| e.starts_with("Send")),
        "empty steering text must never send, got {entries:?}"
    );
}

/// The column's button undocks instead of docking.
#[gpui::test]
fn column_click_reports_undock(cx: &mut TestAppContext) {
    init(cx);
    let log: CardLog = Rc::new(RefCell::new(Vec::new()));
    let (_host, vcx) = cx.add_window_view(CardHost::build(card_transcript(), log.clone(), true));
    let vcx: &mut VisualTestContext = vcx;
    // The column sits in a 420 × 640 box: sweep its header and its
    // bottom-anchored footer, where the dock button and the chips live.
    sweep(&mut *vcx, 2.0, 418.0, 2.0, 70.0, 6.0, 4.0);
    sweep(&mut *vcx, 2.0, 418.0, 530.0, 638.0, 5.0, 3.0);
    let entries = log.borrow().clone();
    assert!(entries.iter().any(|e| e == "Undock"), "an Undock click must report Undock, got {entries:?}");
    assert!(
        !entries.iter().any(|e| e == "Dock"),
        "the column must never report Dock, got {entries:?}"
    );
}

/// Digits pick the current question's choices; an out-of-range digit is
/// ignored.
#[gpui::test]
fn question_digits_choose(cx: &mut TestAppContext) {
    init(cx);
    let log: CardLog = Rc::new(RefCell::new(Vec::new()));
    let (_host, vcx) = cx.add_window_view(CardHost::build(question_transcript(), log.clone(), false));
    vcx.simulate_keystrokes("1");
    vcx.simulate_keystrokes("2");
    vcx.simulate_keystrokes("9");
    assert_eq!(
        &*log.borrow(),
        &["Choose(0)".to_string(), "Choose(1)".to_string()],
        "digits must pick the current question's choices and ignore the rest"
    );
}

/// With no question current, digits stay text: nothing claims them.
#[gpui::test]
fn digits_do_nothing_without_a_current_question(cx: &mut TestAppContext) {
    init(cx);
    let log: CardLog = Rc::new(RefCell::new(Vec::new()));
    let (host, vcx) = cx.add_window_view(CardHost::build(card_transcript(), log.clone(), false));
    // Focus the steering input; the card is 400 × 520 at the top-left.
    vcx.simulate_click(point(px(100.0), px(460.0)), Modifiers::none());
    vcx.simulate_keystrokes("1");
    assert!(log.borrow().is_empty(), "a digit with no current question must emit nothing, got {:?}", log.borrow());
    let value = vcx.update(|_, cx| host.read(cx).input.read(cx).value().to_string());
    assert_eq!(value, "1", "the digit must land in the steering input, proving focus was inside the card");
}

/// With a question current, a digit typed into the focused steering input
/// stays text: the question's digit context lives on the question block,
/// which is not an ancestor of the footer input.
#[gpui::test]
fn question_digit_stays_text_in_the_focused_input(cx: &mut TestAppContext) {
    init(cx);
    let log: CardLog = Rc::new(RefCell::new(Vec::new()));
    let (host, vcx) = cx.add_window_view(CardHost::build(question_transcript(), log.clone(), false));
    // Focus the steering input; the card is 400 × 520 at the top-left.
    vcx.simulate_click(point(px(100.0), px(460.0)), Modifiers::none());
    vcx.simulate_keystrokes("2");
    assert!(
        log.borrow().is_empty(),
        "a digit typed into the steering input must not choose, got {:?}",
        log.borrow()
    );
    let value = vcx.update(|_, cx| host.read(cx).input.read(cx).value().to_string());
    assert_eq!(value, "2", "the digit must land in the steering input, not in a choice");
}

/// A question arriving while the person steers keeps the caret: the
/// question block's one-time keyboard claim never steals the steering
/// input's focus.
#[gpui::test]
fn question_arriving_mid_steer_keeps_input_focus(cx: &mut TestAppContext) {
    init(cx);
    let log: CardLog = Rc::new(RefCell::new(Vec::new()));
    let (host, vcx) = cx.add_window_view(CardHost::build(card_transcript(), log.clone(), false));
    // Focus the steering input and start typing a year.
    vcx.simulate_click(point(px(100.0), px(460.0)), Modifiers::none());
    vcx.simulate_input("20");
    // The follow-up question lands while the person types.
    vcx.update(|_, cx| host.update(cx, |h, _| h.transcript = question_transcript()));
    vcx.run_until_parked();
    vcx.simulate_keystrokes("2");
    assert!(
        log.borrow().is_empty(),
        "a digit typed after the question lands must not choose, got {:?}",
        log.borrow()
    );
    let value = vcx.update(|_, cx| host.read(cx).input.read(cx).value().to_string());
    assert_eq!(value, "202", "the digit must extend the steered text, proving the input kept focus");
}
