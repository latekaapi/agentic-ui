//! Behavioural tests for the session model.

use aui_protocol::{
    ActivityState, ApprovalDecision, ApprovalState, Block, Delta, Session, ToolBody, ToolCall,
    ToolKind, ToolStatus, Turn, TurnMeta,
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
        turn: Turn::Assistant { id: "t1".into(), blocks: Vec::new(), meta: TurnMeta::default(), timestamp: None },
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
        reasoning_tokens: 120,
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
        timestamp: None,
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

#[test]
fn thinking_delta_appends_to_the_trace() {
    let mut session = Session::new("s1", aui_protocol::Provider::Muse, "muse-spark-1.3", "~/work");
    session.turns.push(Turn::Assistant {
        id: "t1".into(),
        timestamp: None,
        blocks: vec![Block::Thinking {
            text: "Weighing ".into(),
            elapsed_ms: 0,
            summary: None,
            state: aui_protocol::ThinkingState::Thinking,
        }],
        meta: TurnMeta::default(),
    });

    assert!(session.apply(Delta::ThinkingDelta {
        turn_id: "t1".into(),
        block_index: 0,
        text: "the options.".into(),
    }));
    match &session.turn("t1").unwrap().blocks()[0] {
        Block::Thinking { text, .. } => assert_eq!(text, "Weighing the options."),
        other => panic!("expected a thinking block, got {other:?}"),
    }

    // Wrong turn, wrong index, wrong block kind: all ignored.
    assert!(!session.apply(Delta::ThinkingDelta {
        turn_id: "nope".into(),
        block_index: 0,
        text: "x".into(),
    }));
    assert!(!session.apply(Delta::ThinkingDelta {
        turn_id: "t1".into(),
        block_index: 9,
        text: "x".into(),
    }));
    session.turns.push(Turn::Assistant {
        id: "t2".into(),
        blocks: vec![Block::text("prose")],
        meta: TurnMeta::default(),
        timestamp: None,
    });
    assert!(!session.apply(Delta::ThinkingDelta {
        turn_id: "t2".into(),
        block_index: 0,
        text: "x".into(),
    }));
}

fn shell_call(output: Vec<String>) -> Block {
    Block::ToolCall {
        id: "tc1".into(),
        kind: aui_protocol::ToolKind::Shell,
        verb: "Ran".into(),
        target: "pnpm test".into(),
        status: ToolStatus::Running,
        duration_ms: None,
        body: ToolBody::Shell { output_lines: output, exit_code: None, live: true },
    }
}

fn output_lines(session: &Session) -> Vec<String> {
    match &session.turn("t1").unwrap().blocks()[0] {
        Block::ToolCall { body: ToolBody::Shell { output_lines, .. }, .. } => output_lines.clone(),
        other => panic!("expected a shell tool call, got {other:?}"),
    }
}

#[test]
fn tool_output_delta_merges_partial_lines() {
    let mut session = Session::new("s1", aui_protocol::Provider::Muse, "muse-spark-1.3", "~/work");
    session.turns.push(Turn::Assistant {
        id: "t1".into(),
        blocks: vec![shell_call(Vec::new())],
        meta: TurnMeta::default(),
        timestamp: None,
    });
    let mut push = |text: &str| {
        session.apply(Delta::ToolOutputDelta {
            turn_id: "t1".into(),
            block_index: 0,
            text: text.into(),
        })
    };

    assert!(push("PASS src/a"));
    assert!(push(".test.ts\nPASS "));
    assert!(push("src/b.test.ts\n"));
    // An empty chunk changes nothing.
    assert!(!push(""));

    assert_eq!(
        output_lines(&session),
        vec!["PASS src/a.test.ts".to_string(), "PASS src/b.test.ts".to_string(), String::new()]
    );
}

#[test]
fn tool_output_delta_ignores_bodies_with_nowhere_to_put_it() {
    let mut session = Session::new("s1", aui_protocol::Provider::Muse, "muse-spark-1.3", "~/work");
    session.turns.push(Turn::Assistant {
        id: "t1".into(),
        blocks: vec![Block::text("not a tool call")],
        meta: TurnMeta::default(),
        timestamp: None,
    });
    assert!(!session.apply(Delta::ToolOutputDelta {
        turn_id: "t1".into(),
        block_index: 0,
        text: "x".into(),
    }));
    // Missing turn and out-of-range index too.
    assert!(!session.apply(Delta::ToolOutputDelta {
        turn_id: "nope".into(),
        block_index: 0,
        text: "x".into(),
    }));
    assert!(!session.apply(Delta::ToolOutputDelta {
        turn_id: "t1".into(),
        block_index: 4,
        text: "x".into(),
    }));

    // A non-shell tool body has nowhere to put the chunk either.
    session.turns.push(Turn::Assistant {
        id: "t2".into(),
        timestamp: None,
        blocks: vec![Block::ToolCall {
            id: "tc2".into(),
            kind: aui_protocol::ToolKind::Read,
            verb: "Read".into(),
            target: "src/main.rs".into(),
            status: ToolStatus::Success,
            duration_ms: Some(4),
            body: ToolBody::Read { lines: 12 },
        }],
        meta: TurnMeta::default(),
    });
    assert!(!session.apply(Delta::ToolOutputDelta {
        turn_id: "t2".into(),
        block_index: 0,
        text: "x".into(),
    }));
}

#[test]
fn removal_deltas_take_things_out_and_ignore_the_rest() {
    let mut session = Session::new("s1", aui_protocol::Provider::Muse, "muse-spark-1.3", "~/work");
    session.turns.push(Turn::Assistant {
        id: "t1".into(),
        blocks: vec![Block::text("one"), Block::text("two")],
        meta: TurnMeta::default(),
        timestamp: None,
    });
    session.turns.push(Turn::Assistant {
        id: "t2".into(),
        blocks: Vec::new(),
        meta: TurnMeta::default(),
        timestamp: None,
    });

    assert!(!session.apply(Delta::BlockRemoved { turn_id: "t1".into(), block_index: 5 }));
    assert!(!session.apply(Delta::BlockRemoved { turn_id: "nope".into(), block_index: 0 }));
    assert!(session.apply(Delta::BlockRemoved { turn_id: "t1".into(), block_index: 0 }));
    assert_eq!(session.turn("t1").unwrap().blocks(), &[Block::text("two")]);

    assert!(!session.apply(Delta::TurnRemoved { turn_id: "nope".into() }));
    assert!(session.apply(Delta::TurnRemoved { turn_id: "t1".into() }));
    assert_eq!(session.turns.len(), 1);
    assert_eq!(session.last_turn_id(), Some("t2"));
}

#[test]
fn permission_modes_use_the_msp_wire_strings() {
    let pairs = [
        (aui_protocol::PermissionMode::AllowAll, "allowAll", "Full access"),
        (aui_protocol::PermissionMode::OnRequest, "onRequest", "Auto"),
        (aui_protocol::PermissionMode::PromptUnmatched, "promptUnmatched", "Ask"),
        (aui_protocol::PermissionMode::DenyUnmatched, "denyUnmatched", "Read-only"),
    ];
    for (mode, wire, label) in pairs {
        assert_eq!(serde_json::to_value(mode).unwrap(), wire);
        assert_eq!(
            serde_json::from_str::<aui_protocol::PermissionMode>(&format!("\"{wire}\"")).unwrap(),
            mode
        );
        assert_eq!(mode.label(), label);
        assert!(!mode.description().is_empty());
    }
    assert_eq!(aui_protocol::PermissionMode::default(), aui_protocol::PermissionMode::OnRequest);
    // Plan mode is a client-side overlay, never a wire mode.
    assert!(aui_protocol::sample::session().plan);
}

#[test]
fn reasoning_effort_uses_the_msp_wire_strings() {
    use aui_protocol::ReasoningEffort as E;
    let pairs = [
        (E::None, "none"),
        (E::Minimal, "minimal"),
        (E::Low, "low"),
        (E::Medium, "medium"),
        (E::High, "high"),
        (E::Xhigh, "xhigh"),
        (E::Max, "max"),
        (E::Ultra, "ultra"),
    ];
    for (effort, wire) in pairs {
        assert_eq!(serde_json::to_value(effort).unwrap(), wire);
        assert_eq!(serde_json::from_str::<E>(&format!("\"{wire}\"")).unwrap(), effort);
        assert!(!effort.label().is_empty());
    }
    // `max` joined the closed MSP vocabulary in muse 1.1.1, between `xhigh` and `ultra`.
}

#[test]
fn goal_and_generic_blocks_round_trip() {
    let goal = Block::Goal {
        objective: "Ship the checkout rewrite".into(),
        status: "in_progress".into(),
        percent_complete: Some(140.0),
        current_work: Some("Rewriting the validator".into()),
        next_work: None,
    };
    let json = serde_json::to_value(&goal).unwrap();
    assert_eq!(json["kind"], "goal");
    // Verbatim from the provider: over 100 passes straight through.
    assert_eq!(json["percent_complete"], 140.0);
    assert_eq!(serde_json::from_value::<Block>(json).unwrap(), goal);

    let generic = Block::Generic {
        kind: "reminderChild".into(),
        status: "inProgress".into(),
        text: "Reminder agent is running".into(),
    };
    let json = serde_json::to_value(&generic).unwrap();
    assert_eq!(json["kind"], "generic");
    assert_eq!(json["item_kind"], "reminderChild");
    assert_eq!(serde_json::from_value::<Block>(json).unwrap(), generic);
}

#[test]
fn muse_approval_fields_default_when_absent() {
    // A session serialized before the MSP fields existed still loads.
    let json = serde_json::json!({
        "kind": "approval",
        "id": "ap-1",
        "tool": "Bash",
        "command": "ls",
        "reason": "",
        "cwd": "~/work",
        "capabilities": [],
        "scope": "this_worktree",
        "state": { "kind": "pending" },
        "rule": null,
    });
    let block: Block = serde_json::from_value(json).unwrap();
    match &block {
        Block::Approval { choices, stages, current_stage, badges, feedback, resolved_by, .. } => {
            assert!(choices.is_empty());
            assert!(stages.is_empty());
            assert_eq!(*current_stage, None);
            assert_eq!(*badges, aui_protocol::ApprovalBadges::default());
            assert_eq!(*feedback, None);
            assert_eq!(*resolved_by, None);
        }
        other => panic!("expected an approval, got {other:?}"),
    }
}

#[test]
fn the_wider_approval_decisions_settle_the_card() {
    let approving = [ApprovalDecision::ApprovedForSession, ApprovalDecision::PolicyAmendment];
    for decision in approving {
        let mut approval = aui_protocol::sample::approvals().remove(0);
        assert!(approval.decide_approval(decision, Some("ls *".into())));
        assert!(matches!(approval, Block::Approval { state: ApprovalState::Approving, .. }));
    }
    let refusing = [
        ApprovalDecision::DeniedPolicyAmendment,
        ApprovalDecision::TimedOut,
        ApprovalDecision::Abort,
    ];
    for decision in refusing {
        let mut approval = aui_protocol::sample::approvals().remove(0);
        assert!(approval.decide_approval(decision, None));
        assert!(matches!(approval, Block::Approval { state: ApprovalState::Denied, .. }));
    }
}

#[test]
fn tool_group_reuses_the_tool_call_shape() {
    let call = ToolCall {
        id: "tc1".into(),
        kind: ToolKind::Read,
        verb: "Read".into(),
        target: "src/main.rs".into(),
        status: ToolStatus::Success,
        duration_ms: Some(4),
        body: ToolBody::Read { lines: 12 },
        diff_stat: None,
    };
    // The struct and the lone-call variant carry the same data both ways.
    assert_eq!(Block::tool_call(call.clone()).as_tool_call(), Some(call.clone()));
    assert!(Block::text("hi").as_tool_call().is_none());

    let group = Block::ToolGroup {
        calls: vec![call],
        summary: "Checked the flow".into(),
        state: ActivityState::Done,
    };
    let json = serde_json::to_value(&group).expect("serialize");
    assert_eq!(json["kind"], "tool_group");
    assert_eq!(json["calls"][0]["tool_kind"]["kind"], "read");
    let back: Block = serde_json::from_value(json).expect("deserialize");
    assert_eq!(group, back);
}

#[test]
fn sample_tool_groups_fold_sample_calls() {
    for block in aui_protocol::sample::tool_groups() {
        match block {
            Block::ToolGroup { calls, summary, .. } => {
                assert!(!calls.is_empty(), "a group with no calls shows nothing");
                assert!(!summary.is_empty());
            }
            other => panic!("expected a tool group, got {other:?}"),
        }
    }
}

#[test]
fn turn_timestamp_is_additive_and_round_trips() {
    // Old payloads carry no `timestamp`: they decode with `None`.
    let user: Turn = serde_json::from_value(serde_json::json!({
        "kind": "user",
        "id": "t1",
        "text": "hi",
        "attachments": [],
        "mentions": [],
    }))
    .expect("decode");
    assert_eq!(user.timestamp(), None);
    let assistant: Turn = serde_json::from_value(serde_json::json!({
        "kind": "assistant",
        "id": "t2",
        "blocks": [],
        "meta": {
            "model": "",
            "duration_ms": 0,
            "tokens_in": 0,
            "tokens_out": 0,
            "cost_usd": 0.0,
        },
    }))
    .expect("decode");
    assert_eq!(assistant.timestamp(), None);

    // New payloads carry it through JSON and back.
    let stamped = Turn::User {
        id: "t1".into(),
        text: "hi".into(),
        attachments: Vec::new(),
        mentions: Vec::new(),
        timestamp: Some(1_786_320_000_000),
    };
    assert_eq!(stamped.timestamp(), Some(1_786_320_000_000));
    let json = serde_json::to_value(&stamped).expect("serialize");
    assert_eq!(json["timestamp"].as_u64(), Some(1_786_320_000_000));
    let back: Turn = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, stamped);

    // `None` stays off the wire.
    let plain = Turn::Assistant {
        id: "t2".into(),
        blocks: Vec::new(),
        meta: TurnMeta::default(),
        timestamp: None,
    };
    let json = serde_json::to_value(&plain).expect("serialize");
    assert!(json.get("timestamp").is_none(), "absent timestamp was drawn: {json}");
}
