//! Browse group: the `⌘J` agent line in its five states over a page strip,
//! and the `⌘E` agent card with the sample PGI transcript.

use aui::browse::agent_window::{
    agent_card, agent_line, AgentItem, AgentTranscript, ApprovalData, LineState, Step, StepState, TaskFace,
};
use aui::composer::{composer_state_rows, ComposerChip, ComposerChipKind};
use aui_tokens::{scale, ActiveAui};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// The page strip the lines float over: muted ground, 640 wide.
const STRIP_W: f32 = 640.0;
const STRIP_PAD: f32 = 16.0;
const STRIP_GAP: f32 = 12.0;

/// The sample ask line: page chip plus the two site actions.
fn ask_state() -> LineState {
    LineState::Ask {
        text: "Ask about this page".into(),
        chips: vec![ComposerChip {
            id: "this-page".into(),
            kind: ComposerChipKind::File,
            label: "This page".into(),
            removable: true,
            thumbnail: None,
            detail: None,
        }],
        site_actions: vec![
            ("sum".into(), "Sum it up".into()),
            ("extract-table".into(), "Extract the table".into()),
        ],
    }
}

/// The sample approval: a download asking once.
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

/// The sample transcript: PGI grades against PTR, dropout and girls'
/// enrolment across five states.
fn sample_transcript() -> AgentTranscript {
    AgentTranscript {
        title: "PGI vs school indicators".into(),
        model: "Claude Sonnet".into(),
        items: vec![
            AgentItem::User(
                "compare PGI 2.0 grades with PTR, dropout and girls' enrolment for Karnataka, Tamil Nadu, Kerala, Maharashtra and Gujarat — one table, with sources"
                    .into(),
            ),
            AgentItem::Steps(vec![
                Step::new("Opened PGI 2.0 state scores (2023-24)", StepState::Done),
                Step::new("Read UDISE+ 2024-25 PTR for 5 states", StepState::Done),
                Step::new("Reading dropout rates, secondary", StepState::Running),
            ]),
            AgentItem::Queued("also add girls' GER".into()),
        ],
    }
}

/// Five stacked lines on a muted page strip, 640 wide.
pub fn build_line_states(window: &mut Window, cx: &mut App) -> AnyElement {
    let ask_text = window.use_keyed_state("browse-line-ask", cx, |window, cx| {
        composer_state_rows("Ask about this page", 1, 1, window, cx)
    });
    let states = [
        ask_state(),
        LineState::from_task(TaskFace::Working, "Claude is reading UDISE+ · dropout"),
        LineState::YouDrive,
        LineState::NeedsYou { text: "data.gov.in asks you to sign in".into() },
        approval_state(),
    ];
    let mut stack = v_flex().w(px(STRIP_W)).gap(px(STRIP_GAP));
    for (i, state) in states.iter().enumerate() {
        let input = window.use_keyed_state(
            SharedString::from(format!("browse-line-{i}")),
            cx,
            |window, cx| composer_state_rows("Ask about this page", 1, 1, window, cx),
        );
        let input = if i == 0 { ask_text.clone() } else { input };
        stack = stack.child(agent_line(SharedString::from(format!("browse-line-{i}")), state, &input));
    }
    let p = cx.aui().colors;
    div()
        .w(px(STRIP_W + STRIP_PAD * 2.0))
        .bg(p.surface_2)
        .rounded(px(scale::R_LG))
        .p(px(STRIP_PAD))
        .child(stack)
        .into_any_element()
}

/// The card with the sample transcript.
pub fn build_card(window: &mut Window, cx: &mut App) -> AnyElement {
    let steer = window.use_keyed_state("browse-card-steer", cx, |window, cx| {
        composer_state_rows("Steer, or ⌘↩ to queue", 1, 4, window, cx)
    });
    let transcript = sample_transcript();
    h_flex()
        .w_full()
        .justify_center()
        .child(agent_card("browse-card", &transcript, &steer))
        .into_any_element()
}
