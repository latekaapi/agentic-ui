//! Browse group: the `⌘J` agent line in its five states over a page strip,
//! and the `⌘E` agent card with the sample PGI transcript.

use aui::browse::agent_window::{
    agent_card, agent_column, agent_line, approval_height, AgentItem, AgentTranscript, ApprovalData, LineState, Step, StepState,
};
use aui::browse::TaskFace;
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
            state: Default::default(),
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
        allow_always: None,
    })
}

/// The page chip the card footer and the ask line share.
fn page_chip() -> ComposerChip {
    ComposerChip {
        id: "this-page".into(),
        kind: ComposerChipKind::File,
        label: "This page".into(),
        removable: true,
        thumbnail: None,
        detail: None,
        state: Default::default(),
    }
}

/// The sample transcript: PGI grades against PTR, dropout and girls'
/// enrolment across five states.
fn sample_transcript() -> AgentTranscript {
    AgentTranscript {
        title: "PGI vs school indicators".into(),
        model: "Claude Sonnet".into(),
        chips: vec![page_chip()],
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

/// The column transcript: the sample plus the answered comparison and the
/// follow-up question waiting on the person.
fn column_transcript() -> AgentTranscript {
    let mut transcript = sample_transcript();
    transcript.items.push(
        AgentItem::Answer(
            "Karnataka and Tamil Nadu lead the five-state table on PTR and girls' enrolment. Gujarat improved 3.5 points since 2021, while dropout is highest in Maharashtra."
                .into(),
        ),
    );
    transcript.items.push(AgentItem::Question {
        text: "Which year for PTR?".into(),
        choices: vec!["2023-24".into(), "2024-25".into()],
    });
    transcript
}

/// Five stacked lines on a muted page strip, 640 wide.
pub fn build_line_states(window: &mut Window, cx: &mut App) -> AnyElement {
    let ask_text = window.use_keyed_state("browse-line-ask", cx, |window, cx| {
        composer_state_rows("Ask about this page", 1, 1, window, cx)
    });
    let states = [
        ask_state(),
        LineState::from_task(TaskFace::Reading, "Claude is reading UDISE+ · dropout"),
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

/// The width of a narrow side pane's line: Baaz's right pane at its
/// narrowest leaves about this much once the line's margins are taken.
const NARROW_W: f32 = 260.0;

/// A page approval with no host (a local file), as the side pane shows it.
fn pay_approval(host: &str) -> ApprovalData {
    ApprovalData {
        title: "Claude Code wants to press \u{201C}Pay \u{20AC}97\u{201D}".into(),
        host: host.to_owned().into(),
        line: host.to_owned().into(),
        kind_pill: "Pay".into(),
        hint: "Pay always asks".into(),
        deny: "Don't allow".into(),
        allow: "Pay".into(),
        allow_always: None,
    }
}

/// The line in a narrow pane: the working row, then three approvals (no
/// host, a host, a long download), each seated in a frame exactly
/// [`approval_height`] tall, the window a host opens for it. A card that
/// overflows its frame is cut, the way the window would cut it.
pub fn build_line_narrow(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let approvals = [
        pay_approval(""),
        pay_approval("shop.example.com"),
        match approval_state() {
            LineState::Approval(data) => data,
            _ => unreachable!(),
        },
    ];
    let mut stack = v_flex().w(px(NARROW_W)).gap(px(STRIP_GAP));
    let input = window.use_keyed_state("browse-narrow-row", cx, |window, cx| {
        composer_state_rows("Ask about this page", 1, 1, window, cx)
    });
    stack = stack.child(
        agent_line("browse-narrow-row", &LineState::from_task(TaskFace::Reading, "Claude Code is reading checkout.html"), &input)
            .compact(true),
    );
    for (i, data) in approvals.into_iter().enumerate() {
        let height = approval_height(&data, true, cx);
        let id = SharedString::from(format!("browse-narrow-{i}"));
        stack = stack.child(
            div()
                .w_full()
                .h(px(height))
                .overflow_hidden()
                .flex()
                .flex_col()
                .justify_end()
                .child(agent_line(id, &LineState::Approval(data), &input).compact(true)),
        );
    }
    // Black text on the strip stands in for a bare host window (Baaz's float
    // over a page has no themed root): anything the line does not colour
    // itself shows up black here, as it did in Baaz on 7 Nov.
    div()
        .w(px(NARROW_W + STRIP_PAD * 2.0))
        .bg(p.surface_2)
        .text_color(gpui::black())
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

/// The column with the answered comparison and the follow-up question, in a
/// 420 × 640 frame standing in for the host's right pane.
pub fn build_column(window: &mut Window, cx: &mut App) -> AnyElement {
    let steer = window.use_keyed_state("browse-column-steer", cx, |window, cx| {
        composer_state_rows("Steer, or ⌘↩ to queue", 1, 4, window, cx)
    });
    let transcript = column_transcript();
    h_flex()
        .w_full()
        .justify_center()
        .child(div().w(px(420.0)).h(px(640.0)).child(agent_column("browse-column", &transcript, &steer)))
        .into_any_element()
}

/// The R3 line states (harness R3 board 1a/1b/2b): the inline answer working
/// and answered, and the landed draft, each in a frame exactly
/// [`line_height`] tall — the window the host opens — at 640 and at the
/// owner's 356 px line. Black ground text stands in for a bare host window.
pub fn build_line_answer(window: &mut Window, cx: &mut App) -> AnyElement {
    use aui::browse::agent_window::{line_height, APPROVAL_COMPACT_BELOW};
    let p = cx.aui().colors;
    let question: SharedString = "What's the refund window?".into();
    let answer: SharedString = "Within 30 days of delivery. Final-sale items, gift cards and opened software can't be returned; the refund reaches your card 5–7 business days after the return arrives.".into();
    let states = [
        LineState::Answer { question: question.clone(), answer: None, source: None, text: "Ask about this page".into(), chips: vec![page_chip()] },
        LineState::Answer {
            question,
            answer: Some(answer),
            source: Some("Returns and refunds · paragraphs 1–2".into()),
            text: "Ask about this page".into(),
            chips: vec![page_chip()],
        },
        LineState::Drafted { label: "Draft ready".into() },
    ];
    let mut columns = h_flex().items_start().gap(px(24.0));
    for (c, width) in [STRIP_W, 356.0].into_iter().enumerate() {
        let compact = width < APPROVAL_COMPACT_BELOW;
        let mut stack = v_flex().w(px(width)).gap(px(STRIP_GAP));
        for (i, state) in states.iter().enumerate() {
            let id = SharedString::from(format!("browse-answer-{c}-{i}"));
            let input = window.use_keyed_state(id.clone(), cx, |window, cx| composer_state_rows("Ask about this page", 1, 1, window, cx));
            let height = line_height(state, compact, width, window, cx);
            stack = stack.child(
                div()
                    .w_full()
                    .h(px(height))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(agent_line(id, state, &input).compact(compact)),
            );
        }
        columns = columns.child(stack);
    }
    div()
        .bg(p.surface_2)
        .text_color(gpui::black())
        .rounded(px(scale::R_LG))
        .p(px(STRIP_PAD))
        .child(columns)
        .into_any_element()
}

/// The handoff (harness R3 board 3a/3b): the picker beside the card that
/// has been handed off.
pub fn build_handoff(window: &mut Window, cx: &mut App) -> AnyElement {
    use aui::browse::handoff::{handoff_height, handoff_picker, HandoffRow};
    use aui::icons::Provider;
    let p = cx.aui().colors;
    let query = window.use_keyed_state("browse-handoff-q", cx, |window, cx| composer_state_rows("Find a session or project", 1, 1, window, cx));
    let sessions = vec![
        HandoffRow { id: "s1".into(), title: "Fix login".into(), meta: "harness · 2 h".into(), provider: Some(Provider::Claude) },
        HandoffRow { id: "s2".into(), title: "Checkout flow tests".into(), meta: "harness · yesterday".into(), provider: Some(Provider::Codex) },
        HandoffRow { id: "s3".into(), title: "Sidebar polish".into(), meta: "agentic-ui · Tue".into(), provider: Some(Provider::Muse) },
    ];
    let projects = vec![
        HandoffRow { id: "p1".into(), title: "harness".into(), meta: "Claude Code".into(), provider: None },
        HandoffRow { id: "p2".into(), title: "agentic-ui".into(), meta: "Claude Code".into(), provider: None },
    ];
    let h = handoff_height(sessions.len(), projects.len());
    let picker = div().flex_none().h(px(h)).overflow_hidden().child(handoff_picker("browse-handoff", &query, sessions, projects, 0));
    let mut transcript = column_transcript();
    transcript.items.truncate(4);
    transcript.items.push(AgentItem::HandedOff {
        title: "Fix login".into(),
        detail: "harness · its 3 tabs are now in that session's task folder".into(),
    });
    let input = window.use_keyed_state("browse-handoff-card", cx, |window, cx| composer_state_rows("Ask something else here", 1, 1, window, cx));
    div()
        .bg(p.surface_2)
        .text_color(gpui::black())
        .rounded(px(scale::R_LG))
        .p(px(STRIP_PAD))
        .child(h_flex().items_start().gap(px(24.0)).child(picker).child(agent_card("browse-handoff-card", &transcript, &input)))
        .into_any_element()
}
