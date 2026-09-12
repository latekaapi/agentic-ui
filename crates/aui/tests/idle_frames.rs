//! Settled cards must be silent: a component that leaves an animation mounted
//! while it is at rest asks gpui for a frame on every render, which keeps the
//! whole window redrawing for as long as the card is on screen.
//!
//! `Window::request_animation_frame` registers an `on_next_frame` callback, and
//! `Window::simulate_next_frame` runs the ones registered and returns how many
//! there were — so drawing a card and then asking for the next frame counts
//! exactly the frames that card would have kept requesting. A settled card
//! registers none.
//!
//! Motion settles on the app clock, so the settling draws advance the test
//! executor's clock rather than sleeping: a tween that has reached its target
//! stops asking, and anything still asking after [`SETTLE`] of clock time is
//! genuinely animating at rest.

use std::rc::Rc;
use std::time::Duration;

use aui::composer::{composer, composer_state};
use aui::icons::IconName;
use aui::icons::Provider;
use aui::nav::{sidebar, SessionSummary, SidebarAccount, SidebarGroup, SidebarNav, SidebarNavItem};
use aui::tokens::AgentState;
use aui::protocol::{ToolBody, ToolStatus, TurnMeta};
use aui::screens::{login, LoginState};
use aui::protocol::{ActivityState, QuestionOption, Step, StepState};
use aui::transcript::{activity_group, assistant_turn, code_block, question_card, status_row, tool_card, StatusLead};
use gpui::{AnyElement, IntoElement, TestAppContext, Window};

/// Clock time allowed for a card to settle before its frames are counted:
/// comfortably longer than the slowest tween (280 ms) and spring.
const SETTLE: Duration = Duration::from_millis(1000);
/// Clock time each settling draw advances by (one 60 Hz frame).
const FRAME: Duration = Duration::from_millis(16);
/// Draws taken while counting. Each one is a chance for a mounted animation to
/// register another frame.
const COUNTED_DRAWS: usize = 30;

/// Draws `build` inside a real view until it has settled, then counts the
/// frames it keeps requesting over [`COUNTED_DRAWS`] further draws.
///
/// The card has to be rendered *by a view*: `request_animation_frame` keys its
/// callback on the view being rendered, so an element drawn outside one cannot
/// ask for a frame at all.
fn frames_at_rest(
    cx: &mut TestAppContext,
    label: &str,
    build: impl Fn(&mut Window, &mut gpui::App) -> AnyElement + 'static,
) -> usize {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let (host, cx) = cx.add_window_view(|_, _| Host {
        build: Rc::new(build),
    });

    let redraw = |cx: &mut gpui::VisualTestContext| {
        host.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    };

    let settling_draws = (SETTLE.as_millis() / FRAME.as_millis()) as usize;
    for _ in 0..settling_draws {
        redraw(cx);
        cx.update(|window, app| window.simulate_next_frame(app));
        cx.executor().advance_clock(FRAME);
    }

    let mut frames = 0;
    for _ in 0..COUNTED_DRAWS {
        redraw(cx);
        frames += cx.update(|window, app| window.simulate_next_frame(app));
        cx.executor().advance_clock(FRAME);
    }
    println!("IDLE {label} frames={frames} over {COUNTED_DRAWS} draws");
    frames
}

/// Builds the card under test.
type Build = dyn Fn(&mut Window, &mut gpui::App) -> AnyElement;

/// The host view: the card is rendered by a view, as an app renders it.
struct Host {
    build: Rc<Build>,
}

impl gpui::Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        (self.build.clone())(window, cx)
    }
}

#[gpui::test]
fn a_settled_assistant_turn_requests_no_frames(cx: &mut TestAppContext) {
    let frames = frames_at_rest(cx, "transcript/assistant-turn", |_, _| {
        assistant_turn(
            "turn",
            "## Done\n\nChanged `validateAddress` and re-ran the tests.\n\n- one\n- two",
        )
        .meta(TurnMeta {
            model: "opus".into(),
            duration_ms: 3100,
            tokens_in: 900,
            tokens_out: 400,
            reasoning_tokens: 0,
            cost_usd: 0.04,
        })
        .into_any_element()
    });
    assert_eq!(frames, 0, "a settled assistant turn kept requesting frames");
}

#[gpui::test]
fn a_settled_sidebar_requests_no_frames(cx: &mut TestAppContext) {
    let frames = frames_at_rest(cx, "sidebar", |_, _| {
        sidebar("sidebar", sample_nav()).into_any_element()
    });
    assert_eq!(frames, 0, "a settled sidebar kept requesting frames");
}

#[gpui::test]
fn a_settled_composer_requests_no_frames(cx: &mut TestAppContext) {
    let frames = frames_at_rest(cx, "composer", |window, app| {
        let text = window.use_keyed_state("idle-composer-text", app, |window, cx| {
            composer_state("Reply, or type / for commands", window, cx)
        });
        composer("composer", &text, Provider::Claude, "Opus 4.6").into_any_element()
    });
    assert_eq!(frames, 0, "a settled composer kept requesting frames");
}

#[gpui::test]
fn a_settled_login_screen_requests_no_frames(cx: &mut TestAppContext) {
    let frames = frames_at_rest(cx, "login", |_, _| {
        login("login", LoginState::Choose)
            .product("Muse")
            .provider(Provider::Muse)
            .on_intent(|_, _, _| {})
            .into_any_element()
    });
    assert_eq!(frames, 0, "a settled login screen kept requesting frames");
}

/// The regression this suite exists for: `browser_body`'s click ring used to
/// loop for as long as the card was mounted, so a finished capture held the
/// frame loop open. It pulses while the call is live and stops when it is not.
#[gpui::test]
fn a_finished_browser_tool_card_requests_no_frames(cx: &mut TestAppContext) {
    let live = frames_at_rest(cx, "browser tool card (live, = before)", |_, _| {
        browser_card(ToolStatus::Running).into_any_element()
    });
    assert!(live > 0, "a live browser capture should animate its ring");

    let settled = frames_at_rest(cx, "browser tool card (finished, = after)", |_, _| {
        browser_card(ToolStatus::Success).into_any_element()
    });
    assert_eq!(
        settled, 0,
        "a finished browser capture kept requesting frames"
    );
}

/// A small but representative sidebar: nav rows, a group and a live session.
fn sample_nav() -> SidebarNav {
    SidebarNav::new(
        "acme",
        SidebarAccount::new("B", "Someone", Provider::Claude, 0.78),
    )
    .item(SidebarNavItem::new("tasks", "Tasks", IconName::List).count("7"))
    .group(
        SidebarGroup::new("pinned", "Pinned").session(SessionSummary::new(
            "checkout",
            "checkout-flow-v2",
            AgentState::Done,
            "49m",
        )),
    )
}

fn browser_card(status: ToolStatus) -> impl IntoElement {
    tool_card(
        "browser",
        "Browse",
        "example.com",
        status,
        ToolBody::Browser {
            action: "click".into(),
            screenshot: None,
            caption: Some("Signed in".into()),
        },
    )
}

// ---------------------------------------------------------------------------
// Streaming clocks (finding `performance-13`): the ambient loops a transcript
// runs — the caret's blink, the working shimmer and spinner, the question and
// code rows' springs. Each pair asserts both halves of the gate: the active
// card keeps the frames it needs, the settled one asks for none.
// ---------------------------------------------------------------------------

/// The caret's `blink 1s steps(2)` loop: on while the turn streams, gone when
/// the turn is finished prose.
#[gpui::test]
fn the_streaming_caret_stops_when_the_turn_does(cx: &mut TestAppContext) {
    let streaming = frames_at_rest(cx, "transcript/assistant-turn (streaming)", |_, _| {
        assistant_turn("stream-on", "Writing the answer out")
            .streaming(true)
            .into_any_element()
    });
    assert!(
        streaming > 0,
        "a streaming turn should keep blinking its caret"
    );

    let settled = frames_at_rest(cx, "transcript/assistant-turn (not streaming)", |_, _| {
        assistant_turn("stream-off", "Writing the answer out")
            .streaming(false)
            .into_any_element()
    });
    assert_eq!(settled, 0, "a finished turn kept blinking a caret");
}

/// The activity group's working header: spinner plus shimmering label while it
/// works, plain glyph and text once it is done.
#[gpui::test]
fn a_finished_activity_group_requests_no_frames(cx: &mut TestAppContext) {
    let working = frames_at_rest(cx, "transcript/activity (working)", |_, _| {
        activity(ActivityState::Working, StepState::Running).into_any_element()
    });
    assert!(working > 0, "a working activity group should animate");

    let done = frames_at_rest(cx, "transcript/activity (done)", |_, _| {
        activity(ActivityState::Done, StepState::Done).into_any_element()
    });
    assert_eq!(done, 0, "a finished activity group kept requesting frames");
}

/// The status row's two clocks: the braille spinner and the label shimmer.
#[gpui::test]
fn a_settled_status_row_requests_no_frames(cx: &mut TestAppContext) {
    let working = frames_at_rest(cx, "transcript/status-row (working)", |_, _| {
        status_row("status-on", "Working")
            .lead(StatusLead::Braille)
            .shimmer(true)
            .into_any_element()
    });
    assert!(working > 0, "a working status row should animate");

    let settled = frames_at_rest(cx, "transcript/status-row (settled)", |_, _| {
        status_row("status-off", "Done")
            .lead(StatusLead::None)
            .shimmer(false)
            .into_any_element()
    });
    assert_eq!(settled, 0, "a settled status row kept requesting frames");
}

/// The question card's springs and tweens: they run while a selection moves
/// and stop at their target, so an unanswered card at rest is silent.
#[gpui::test]
fn a_settled_question_card_requests_no_frames(cx: &mut TestAppContext) {
    let frames = frames_at_rest(cx, "transcript/question", |_, _| {
        question_card(
            "question",
            "Which postal format?",
            &[
                QuestionOption {
                    label: "Canadian postal".into(),
                    description: "A1A 1A1, space optional".into(),
                    key: "1".into(),
                    preview: None,
                },
                QuestionOption {
                    label: "US ZIP".into(),
                    description: "5 or 9 digits".into(),
                    key: "2".into(),
                    preview: None,
                },
            ],
        )
        .into_any_element()
    });
    assert_eq!(frames, 0, "a settled question card kept requesting frames");
}

/// The code block's hover tweens settle at their resting opacity.
#[gpui::test]
fn a_settled_code_block_requests_no_frames(cx: &mut TestAppContext) {
    let frames = frames_at_rest(cx, "transcript/code-block", |_, _| {
        code_block("code", "src/main.rs", "fn main() {\n    println!(\"hi\");\n}")
            .language("rust")
            .into_any_element()
    });
    assert_eq!(frames, 0, "a settled code block kept requesting frames");
}

/// A working activity group with one running step.
fn activity(state: ActivityState, step: StepState) -> impl IntoElement {
    activity_group(
        "activity",
        vec![
            Step { verb: "Searched".into(), target: "src/checkout".into(), state: StepState::Done, result: Some("2 hits".into()) },
            Step { verb: "Edited".into(), target: "src/checkout/validators.ts".into(), state: step, result: None },
        ],
        "Running tests",
        "12s",
        state,
    )
    .open(true)
}
