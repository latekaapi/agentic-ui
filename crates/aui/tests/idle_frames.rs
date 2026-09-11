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
use aui::transcript::{assistant_turn, tool_card};
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
