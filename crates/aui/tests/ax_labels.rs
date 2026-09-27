//! V2: every transcript control carries an accessibility label.
//!
//! Draws the gallery's transcript entries — the protocol sample session plus
//! the card-matrix samples — with every handler wired, then asserts the AX
//! probe recorded no unlabelled control. Labelled [`Button`]s name
//! themselves from their visible text; icon-only buttons and clickable rows
//! carry explicit labels, and an empty recorded label is a control the
//! accessibility tree has no name for.

use std::rc::Rc;

use aui::icons::IconName;
use aui::protocol::{
    sample, ApprovalBodyKind, ApprovalState, Block, Diff, HandoffItem, HandoffState, Provider,
    ToolBody, ToolStatus,
};
use aui::transcript::{
    activity_group, answered_row, approval_card, arm_ax_probe, assistant_turn, code_block,
    diff_block, error_card, generic_item_card, goal_card, handoff_card, jump_pill, marker_row,
    needs_you_banner, plan_card, question_card, summary_card, take_ax_labels, thinking_block,
    todo_list, tool_card, tool_group, user_turn, CodeBlockHostButton, DiffNote, ProseStyle,
    QuestionOutcome, ToolCardAction, ToolGroupData,
};
use aui_tokens::scale;
use gpui::{div, AnyElement, IntoElement, ParentElement, SharedString, TestAppContext, Window};

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
}

/// Builds the entries under test.
type Build = dyn Fn(&mut Window, &mut gpui::App) -> AnyElement;

/// The host view: entries are rendered by a view, as an app renders them.
struct Host {
    build: Rc<Build>,
}

impl gpui::Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        (self.build.clone())(window, cx)
    }
}

/// Draws `build` once in a real window and returns the effective AX labels
/// the render committed. `add_window_view` draws exactly once, so one draw's
/// labels are exactly one entry list's labels.
fn drawn_labels(
    cx: &mut TestAppContext,
    build: impl Fn(&mut Window, &mut gpui::App) -> AnyElement + 'static,
) -> Vec<String> {
    arm_ax_probe(true);
    take_ax_labels();
    let (_host, _) = cx.add_window_view(|_, _| Host { build: Rc::new(build) });
    let labels = take_ax_labels();
    arm_ax_probe(false);
    labels
}

/// One gallery entry per block, with every handler wired so every control
/// exists. `n` keeps element ids unique across the list.
fn block_element(block: &Block, n: usize) -> AnyElement {
    let id = |name: &str| SharedString::from(format!("ax-{n}-{name}"));
    match block {
        Block::Text { text, .. } => aui::transcript::prose(
            id("text"),
            text,
            ProseStyle {
                ink: gpui::white(),
                code_ink: gpui::white(),
                code_bg: gpui::transparent_black(),
                size: scale::FS_13,
                line_height: scale::LH_BODY,
                paragraph_gap: scale::SP_2,
            },
        )
        .into_any_element(),
        Block::Thinking { text, .. } => thinking_block(id("thinking"), text.clone(), "1.2 s", aui::protocol::ThinkingState::Done)
            .on_toggle(|_, _, _| {})
            .into_any_element(),
        Block::Activity { steps, summary, .. } => activity_group(
            id("activity"),
            steps.clone(),
            summary.clone(),
            "3.1 s",
            aui::protocol::ActivityState::Done,
        )
        .on_toggle(|_, _, _| {})
        .into_any_element(),
        Block::ToolCall { id: call_id, verb, target, status, duration_ms, body, diff_stat, .. } => {
            tool_card(id("tool"), verb.clone(), target.clone(), *status, body.clone())
                .duration_ms(*duration_ms)
                .diff_stat(*diff_stat)
                .action(ToolCardAction::new(format!("{call_id}-open"), "Open"))
                .on_intent(|_, _, _| {})
                .into_any_element()
        }
        Block::ToolGroup { .. } => {
            let data = ToolGroupData::from_block(block).expect("a group stays a group");
            tool_group(id("group"), &data, true).on_intent(|_, _, _| {}).into_any_element()
        }
        Block::Approval {
            tool,
            command,
            reason,
            cwd,
            capabilities,
            scope,
            body_kind,
            state,
            rule,
            choices,
            stages,
            current_stage,
            badges,
            feedback,
            resolved_by,
            ..
        } => {
            let mut el = approval_card(id("approval"), tool.clone(), command.clone(), state.clone())
                .body_kind(*body_kind)
                .reason(reason.clone())
                .cwd(cwd.clone())
                .capabilities(capabilities.clone())
                .scope(*scope)
                .choices(choices.clone())
                .stages(stages.clone(), *current_stage)
                .badges(*badges)
                .resolved_by(*resolved_by)
                .feedback_text("")
                .on_decide(|_, _, _| {})
                .on_choose(|_, _, _, _| {})
                .on_feedback_toggle(|_, _, _| {})
                .on_manage_rules(|_, _| {});
            if let Some(rule) = rule {
                el = el.rule(rule.clone());
            }
            if let Some(feedback) = feedback {
                el = el.feedback(feedback.clone());
            }
            // A choice that takes feedback reveals the host-owned field with
            // its Send / Cancel pair; choices without one keep the triad path
            // only when the list is empty, so also cover the built-in row.
            if choices.iter().any(|c| c.accepts_feedback) {
                let open = choices.iter().find(|c| c.accepts_feedback).map(|c| c.id.clone());
                el = el.feedback_open(open).feedback_slot(div());
            }
            el.into_any_element()
        }
        Block::Question { prompt, options, multi, allow_other, .. } => {
            question_card(id("question"), prompt.clone(), options)
                .multi(*multi)
                .allow_other(*allow_other)
                .selected(vec![0])
                .previews_open(vec![0])
                .timeout(60_000, 120_000)
                .clarify_open(true)
                .clarify_slot(div())
                .on_select(|_, _, _| {})
                .on_other(|_, _, _| {})
                .on_answer(|_, _, _| {})
                .on_skip(|_, _, _| {})
                .on_toggle_preview(|_, _, _| {})
                .on_clarify(|_, _, _| {})
                .into_any_element()
        }
        Block::Plan { items, sections, state, .. } => {
            let items: Vec<SharedString> =
                items.iter().map(|s| SharedString::from(s.as_str())).collect();
            plan_card(id("plan"), &items)
                .sections(sections.clone())
                .state(*state)
                .on_accept(|_, _, _| {})
                .on_edit(|_, _, _| {})
                .on_reject(|_, _, _| {})
                .into_any_element()
        }
        Block::Todo { items } => {
            todo_list(id("todo"), items.clone()).on_toggle(|_, _, _| {}).into_any_element()
        }
        Block::Summary { title, files, checks, duration_ms, cost_usd } => {
            let meta: SharedString =
                format!("{duration_ms} ms · ${cost_usd:.2}").into();
            summary_card(id("summary"), title.clone(), meta)
                .files(files.clone())
                .checks(checks.clone())
                .on_action(|_, _, _| {})
                .into_any_element()
        }
        Block::Error { title, detail, retryable } => {
            let mut el = error_card(id("error"), title.clone(), detail.clone())
                .link("details", |_, _, _| {});
            if *retryable {
                el = el.on_retry(|_, _, _| {});
            }
            el.into_any_element()
        }
        Block::Goal { objective, status, .. } => {
            goal_card(id("goal"), objective.clone(), status.clone()).into_any_element()
        }
        Block::Generic { kind, status, text } => {
            generic_item_card(id("generic"), kind.clone(), status.clone(), text.clone())
                .into_any_element()
        }
        Block::Marker { text, .. } => marker_row(id("marker"))
            .text(text.clone())
            .strong("strong")
            .link("details", |_, _, _| {})
            .into_any_element(),
        Block::Handoff { .. } => handoff_entries(),
    }
}

/// The handoff card in its cancellable state (Cancel) and, once the fresh
/// session exists, with `Open the new session` — both buttons carry labels.
fn handoff_entries() -> AnyElement {
    handoff_card("ax-handoff", Provider::Claude, Provider::Codex, "opus 4.6", HandoffState::Prepared)
        .from_model("opus 4.6")
        .carried(vec![HandoffItem { label: "Summary".into(), detail: Some("12 turns".into()) }])
        .lost(vec![HandoffItem { label: "Approvals".into(), detail: None }])
        .pack_tokens(Some(8_400))
        .destination_session(Some("sess-2".into()))
        .on_intent(|_, _, _| {})
        .into_any_element()
}

/// Every gallery transcript entry with all of its controls present: the
/// sample session's turns and blocks, the card-matrix samples, both turn
/// rails, code and diff blocks, and the status controls.
fn entries(window: &mut Window, cx: &mut gpui::App) -> AnyElement {
    let _ = (window, cx);
    let mut blocks: Vec<Block> = Vec::new();
    for turn in sample::session().turns {
        blocks.extend(turn.blocks().to_vec());
    }
    blocks.extend(sample::tool_calls());
    blocks.extend(sample::tool_groups());
    blocks.extend(sample::approvals());
    blocks.extend(sample::muse_approvals());
    blocks.extend(sample::approval_body_kinds());
    blocks.push(sample::muse_question());
    blocks.push(sample::muse_plan());
    blocks.extend(sample::muse_goals());
    blocks.push(sample::muse_generic_item());

    let mut col: Vec<AnyElement> = Vec::new();
    col.push(
        user_turn("ax-user-turn", "Ship the checkout flow `@src/checkout`")
            .on_action(|_, _, _| {})
            .into_any_element(),
    );
    col.push(
        assistant_turn("ax-assistant-turn", "Done — see the summary below.")
            .on_action(|_, _, _| {})
            .into_any_element(),
    );
    for (n, block) in blocks.iter().enumerate() {
        col.push(block_element(block, n));
    }
    // A pending choice list with the feedback field open exercises Send /
    // Cancel; here it is covered through the muse sample above, and once
    // more standalone so the pair exists even if the sample changes shape.
    col.push(
        approval_card("ax-feedback", "Bash", "rm -rf build", ApprovalState::Pending)
            .body_kind(ApprovalBodyKind::Command)
            .feedback_open(Some("deny".into()))
            .feedback_slot(div())
            .on_choose(|_, _, _, _| {})
            .on_feedback_toggle(|_, _, _| {})
            .into_any_element(),
    );
    col.push(
        answered_row("ax-answered", vec!["README.md".into()])
            .outcome(QuestionOutcome::Answered)
            .on_change(|_, _, _| {})
            .into_any_element(),
    );
    col.push(
        code_block("ax-code", "src/checkout.ts", "export const total = 42;\n")
            .language("typescript")
            .hidden_lines(3)
            .host_action(0, CodeBlockHostButton::new("Run", IconName::Edit))
            .on_action(|_, _, _| {})
            .into_any_element(),
    );
    col.push(
        diff_block(
            "ax-diff",
            Diff {
                path: "src/checkout.ts".into(),
                hunks: Vec::new(),
                added: 1,
                removed: 0,
            },
        )
        .notes(vec![DiffNote { line: 1, text: "Looks right.".into(), pending: true }])
        .on_action(|_, _, _| {})
        .into_any_element(),
    );
    // A shell body past the fold draws the unfold control and "open in
    // terminal".
    col.push(
        tool_card(
            "ax-shell",
            "Ran",
            "pnpm test",
            ToolStatus::Success,
            ToolBody::Shell {
                output_lines: (0..9).map(|i| format!("line {i}")).collect(),
                exit_code: Some(0),
                live: false,
            },
        )
        .duration_ms(Some(1_200))
        .on_intent(|_, _, _| {})
        .into_any_element(),
    );
    col.push(
        needs_you_banner("ax-needs-you", "Approval needed", "A command is waiting.")
            .on_jump(|_, _, _| {})
            .into_any_element(),
    );
    col.push(
        jump_pill("ax-jump", "Jump to latest").count(2).on_jump(|_, _, _| {}).into_any_element(),
    );
    col.push(handoff_entries());
    div().children(col).into_any_element()
}

/// No transcript control renders without an accessible name.
#[gpui::test]
fn every_transcript_control_has_an_accessibility_label(cx: &mut TestAppContext) {
    init(cx);
    let labels = drawn_labels(cx, entries);
    assert!(!labels.is_empty(), "the entries draw controls at all");
    let unlabelled = labels.iter().filter(|l| l.is_empty()).count();
    assert_eq!(unlabelled, 0, "unlabelled controls: {unlabelled} of {}", labels.len());
    // Spot-check the names the host lane's audit asked for.
    for want in [
        "Deny",
        "Allow once",
        "Send",
        "Cancel",
        "Copy reply",
        "Retry turn",
        "Copy code",
        "Expand section",
        "Show preview for notes.txt",
        "Open the new session on Codex",
        "Jump to latest",
    ] {
        assert!(labels.iter().any(|l| l == want), "missing label {want:?}; got {labels:?}");
    }
}
