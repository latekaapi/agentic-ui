//! A2fix: the `+N`/`−N` chip must reach the block the producer builds.
//!
//! These tests draw real windows — none of them reads a builder field back.
//! The chip labels a draw commits are observed through the [`take_drawn_chips`]
//! probe (the chip analogue of [`Window::painted_quads`]: gpui offers no text
//! query and tags paint no quads), so mutating the precedence guard in
//! `ToolCard::render` changes what the tests see.

use std::rc::Rc;

use aui::protocol::{ActivityState, Block, Diff, DiffStat, ToolBody, ToolCall, ToolKind, ToolStatus};
use aui::transcript::{
    arm_chip_probe, take_drawn_chips, tool_card, tool_group, ToolGroupData,
};
use gpui::{AnyElement, IntoElement, SharedString, TestAppContext, Window};

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
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

/// Draws `build` once in a real window and returns the chip labels the render
/// committed. `add_window_view` draws exactly once, so one draw's chips are
/// exactly one header's chips — no settling loop, which would record the same
/// chips once per extra draw.
fn drawn_chips(
    cx: &mut TestAppContext,
    build: impl Fn(&mut Window, &mut gpui::App) -> AnyElement + 'static,
) -> Vec<String> {
    arm_chip_probe(true);
    take_drawn_chips();
    let (_host, _) = cx.add_window_view(|_, _| Host { build: Rc::new(build) });
    let chips = take_drawn_chips();
    arm_chip_probe(false);
    chips
}

fn edit_call(stat: Option<DiffStat>, body: ToolBody) -> ToolCall {
    ToolCall {
        id: "tc-edit".into(),
        kind: ToolKind::Edit,
        verb: "Edited".into(),
        target: "src/main.rs".into(),
        status: ToolStatus::Success,
        duration_ms: Some(340),
        body,
        diff_stat: stat,
    }
}

/// A `Block::ToolCall` carrying a summary, built into a card the way the
/// gallery's lone-block path builds it, draws the server's chips — the value
/// an outside producer sets on the block ends up drawn, with no opt-in.
#[gpui::test]
fn a_lone_block_carrying_a_summary_draws_the_chips(cx: &mut TestAppContext) {
    init(cx);
    let block = Block::tool_call(edit_call(
        Some(DiffStat { added: 8, removed: 3, files: 1 }),
        ToolBody::None,
    ));
    let chips = drawn_chips(cx, move |_, _| {
        let Block::ToolCall { id, verb, target, status, duration_ms, body, diff_stat, .. } =
            block.clone()
        else {
            unreachable!("a tool call stays a tool call");
        };
        tool_card(SharedString::from(format!("chip-{id}")), verb, target, status, body)
            .duration_ms(duration_ms)
            .diff_stat(diff_stat)
            .into_any_element()
    });
    assert_eq!(chips, vec!["+8".to_string(), "−3".to_string()]);
}

/// A group entry with both a summary and an `Edit` body draws exactly one
/// pair: the server's whole-patch counts win and the `Edit` arm stands down.
/// With the precedence guard reverted to `if true` this sees four chips.
#[gpui::test]
fn a_group_call_with_a_summary_and_an_edit_body_draws_exactly_one_pair(
    cx: &mut TestAppContext,
) {
    init(cx);
    let data = ToolGroupData {
        calls: vec![edit_call(
            Some(DiffStat { added: 20, removed: 7, files: 4 }),
            ToolBody::Edit {
                diff: Diff { path: "src/main.rs".into(), hunks: Vec::new(), added: 2, removed: 1 },
            },
        )],
        summary: "Edited".into(),
        state: ActivityState::Done,
    };
    let chips = drawn_chips(cx, move |_, _| {
        tool_group("chip-group", &data, true).into_any_element()
    });
    assert_eq!(
        chips,
        vec!["+20".to_string(), "−7".to_string()],
        "one pair only: the server summary wins over the Edit diff"
    );
}

/// A group entry with a summary and no diff body draws the server's chips —
/// the group renderer forwards `diff_stat` to each inner card.
#[gpui::test]
fn a_group_call_with_a_summary_and_no_diff_body_draws_the_chips(cx: &mut TestAppContext) {
    init(cx);
    let data = ToolGroupData {
        calls: vec![edit_call(
            Some(DiffStat { added: 8, removed: 3, files: 1 }),
            ToolBody::None,
        )],
        summary: "Edited".into(),
        state: ActivityState::Done,
    };
    let chips = drawn_chips(cx, move |_, _| {
        tool_group("chip-group", &data, true).into_any_element()
    });
    assert_eq!(chips, vec!["+8".to_string(), "−3".to_string()]);
}
