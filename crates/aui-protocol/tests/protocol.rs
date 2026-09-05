//! Behavioural tests for the session model.

use aui_protocol::{
    ApprovalDecision, ApprovalState, Block, Delta, Session, ToolBody, ToolStatus, Turn, TurnMeta,
};

#[test]
fn sample_session_round_trips_through_json() {
    let session = aui_protocol::sample::session();
    let json = serde_json::to_string_pretty(&session).expect("serialize");
    let back: Session = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(session, back);
}

#[test]
fn sample_session_matches_the_design_cards() {
    let session = aui_protocol::sample::session();
    assert_eq!(session.branch.as_deref(), Some("feature/checkout-flow-v2"));
    assert_eq!(session.cwd, "~/work/acme/checkout-flow-v2");

    // Every tool body shape is represented, so the gallery can render them all.
    let bodies: Vec<&ToolBody> = session
        .turns
        .iter()
        .flat_map(Turn::blocks)
        .filter_map(|b| match b {
            Block::ToolCall { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    assert!(bodies.iter().any(|b| matches!(b, ToolBody::Shell { .. })));
    assert!(bodies.iter().any(|b| matches!(b, ToolBody::Read { .. })));
    assert!(bodies.iter().any(|b| matches!(b, ToolBody::Edit { .. })));
    assert!(bodies.iter().any(|b| matches!(b, ToolBody::Search { .. })));
    assert!(bodies.iter().any(|b| matches!(b, ToolBody::Web { .. })));
    assert!(bodies.iter().any(|b| matches!(b, ToolBody::Browser { .. })));
    assert!(bodies.iter().any(|b| matches!(b, ToolBody::SubAgent { .. })));
    assert!(bodies.iter().any(|b| matches!(b, ToolBody::Mcp { .. })));

    // A pending approval is what the harness reference screen shows.
    let pending = session
        .turns
        .iter()
        .flat_map(Turn::blocks)
        .any(|b| matches!(b, Block::Approval { state: ApprovalState::Pending, .. }));
    assert!(pending, "the sample keeps one approval waiting");
}

#[test]
fn enums_serialize_with_a_kind_tag() {
    let json = serde_json::to_value(Block::text("hi")).unwrap();
    assert_eq!(json["kind"], "text");
    assert_eq!(json["streaming"], false);

    let json = serde_json::to_value(ToolStatus::Success).unwrap();
    assert_eq!(json, "success");
}

#[test]
fn text_delta_appends_to_the_streaming_block() {
    let mut session = Session::new("s1", aui_protocol::Provider::Claude, "opus 4.6", "~/work");

    assert!(session.apply(Delta::TurnStarted {
        turn: Turn::Assistant { id: "t1".into(), blocks: Vec::new(), meta: TurnMeta::default() },
    }));
    assert!(session.apply(Delta::BlockAdded {
        turn_id: "t1".into(),
        block: Block::Text { text: "I'm ".into(), streaming: true },
    }));
    assert!(session.apply(Delta::TextDelta {
        turn_id: "t1".into(),
        block_index: 0,
        text: "checking the form.".into(),
    }));

    assert_eq!(
        session.turn("t1").unwrap().blocks()[0],
        Block::Text { text: "I'm checking the form.".into(), streaming: true }
    );

    // Finishing the turn clears the caret and records the footer meta.
    let meta = TurnMeta {
        model: "opus 4.6".into(),
        duration_ms: 3_100,
        tokens_in: 1_900,
        tokens_out: 500,
        cost_usd: 0.04,
    };
    assert!(session.apply(Delta::TurnFinished { turn_id: "t1".into(), meta: meta.clone() }));
    assert_eq!(
        session.turn("t1").unwrap().blocks()[0],
        Block::Text { text: "I'm checking the form.".into(), streaming: false }
    );
    match session.turn("t1").unwrap() {
        Turn::Assistant { meta: got, .. } => assert_eq!(got, &meta),
        _ => panic!("expected an assistant turn"),
    }
}

#[test]
fn deltas_for_unknown_turns_and_blocks_are_ignored() {
    let mut session = Session::new("s1", aui_protocol::Provider::Claude, "opus 4.6", "~/work");
    assert!(!session.apply(Delta::TextDelta {
        turn_id: "nope".into(),
        block_index: 0,
        text: "x".into(),
    }));
    assert!(session.turns.is_empty());

    session.turns.push(Turn::Assistant {
        id: "t1".into(),
        blocks: vec![Block::text("done")],
        meta: TurnMeta::default(),
    });
    // Out-of-range index, and a text delta aimed at a non-text block.
    assert!(!session.apply(Delta::TextDelta {
        turn_id: "t1".into(),
        block_index: 7,
        text: "x".into(),
    }));
    assert!(session.apply(Delta::BlockUpdated {
        turn_id: "t1".into(),
        block_index: 0,
        block: Block::Todo { items: Vec::new() },
    }));
    assert!(!session.apply(Delta::TextDelta {
        turn_id: "t1".into(),
        block_index: 0,
        text: "x".into(),
    }));
}

#[test]
fn approval_transitions_allow_once() {
    let mut approval = aui_protocol::sample::approvals().remove(0);
    assert!(matches!(approval, Block::Approval { state: ApprovalState::Pending, .. }));

    assert!(approval.decide_approval(ApprovalDecision::Once, None));
    assert!(matches!(approval, Block::Approval { state: ApprovalState::Approving, .. }));

    // A second press changes nothing, so the command cannot run twice.
    assert!(!approval.decide_approval(ApprovalDecision::Once, None));

    assert!(approval.complete_approval(0, 8_200));
    match &approval {
        Block::Approval { state, .. } => {
            assert_eq!(state, &ApprovalState::AllowedOnce { exit_code: 0, duration_ms: 8_200 });
        }
        _ => unreachable!(),
    }
    // And completion is not repeatable either.
    assert!(!approval.complete_approval(0, 1));
}

#[test]
fn approval_transitions_always_and_deny() {
    let mut approval = aui_protocol::sample::approvals().remove(0);
    assert!(approval.decide_approval(ApprovalDecision::Always, Some("apt install *".into())));
    match &approval {
        Block::Approval { state, rule, .. } => {
            assert_eq!(state, &ApprovalState::Approving);
            assert_eq!(rule.as_deref(), Some("apt install *"));
        }
        _ => unreachable!(),
    }

    let mut approval = aui_protocol::sample::approvals().remove(0);
    assert!(approval.decide_approval(ApprovalDecision::Deny, None));
    assert!(matches!(approval, Block::Approval { state: ApprovalState::Denied, .. }));
    assert!(!approval.decide_approval(ApprovalDecision::Once, None));

    // Non-approval blocks are untouched by either method.
    let mut text = Block::text("hello");
    assert!(!text.decide_approval(ApprovalDecision::Once, None));
    assert!(!text.complete_approval(0, 1));
}
