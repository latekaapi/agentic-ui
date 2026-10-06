//! The orchestration gallery screen: scenarios A–F as declarative snapshots.
//!
//! Each scenario is a `Vec<Step>`; a step is a full snapshot of what the
//! screen shows, so stepping is trivially correct in both directions. The
//! view renders from the current step and nothing else; clicks mutate the
//! current step's copy (answer a question, expand a reply, retry a child)
//! and usually advance one step.

use aui::nav::SessionSummary;
use aui_icons::Provider as IconProvider;
use aui::protocol::{
    sample, Block, EvidenceKind, EvidenceRow, NativeWorkflow, PeerDirection, PeerMessage,
    PeerReply, PeerState, Provider, Rung, RungState, RunProgress, RunQuestion,
    RunQuestionOption, RunSummary, RunViewModel, TaskLine, TaskState, WorkerHeader, WorkflowChild,
};
use aui_tokens::AgentState;

/// The centre transcript as a list of items.
#[derive(Debug, Clone)]
pub enum CentreItem {
    /// A user bubble.
    User(String),
    /// Lead prose.
    Lead(String),
    /// Muted meta line (timestamps, link-backs rendered by the view).
    Muted(String),
    /// A `← Back to …` link row (worker and peer views).
    BackLink(String),
    /// Any orchestration or tool block from the protocol.
    Block(Block),
    /// A peer message with its reply expanded.
    #[allow(dead_code)]
    PeerExpanded(PeerMessage),
    /// A worker-session header.
    WorkerHead(WorkerHeader),
    /// A native subagent controls row.
    Controls(aui::protocol::SubagentControls),
}

/// The right pane.
#[derive(Debug, Clone)]
pub enum RightPane {
    /// Closed.
    Closed,
    /// The run tree view.
    Run(RunViewModel),
    /// The static browser placeholder (never a real webview).
    Browser,
}

/// One declarative snapshot of the whole screen.
#[derive(Debug, Clone)]
pub struct Step {
    /// The narration line under the chrome.
    pub narration: &'static str,
    /// Sidebar rows.
    pub sidebar: Vec<SessionSummary>,
    /// Selected sidebar row id.
    pub active_row: &'static str,
    /// Centre header title and provider.
    pub centre_title: &'static str,
    pub centre_provider: IconProvider,
    /// Centre transcript items.
    pub items: Vec<CentreItem>,
    /// `true` renders the `Steer this worker…` composer.
    pub steer: bool,
    /// Goal line pinned above the composer.
    pub goal_pin: Option<Block>,
    /// Right pane.
    pub right: RightPane,
    /// Where `Open session ↗` goes, as a step index in this scenario.
    pub peer_goto: Option<usize>,
}

/// One scenario: a picker tab and its steps.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// `A`–`F`.
    pub id: char,
    /// Short picker name.
    pub name: &'static str,
    pub steps: Vec<Step>,
}

// ---- shared builders -----------------------------------------------------

fn lead_row() -> SessionSummary {
    SessionSummary::new("lead", "Login screen run", AgentState::Running, "6m")
        .provider(aui_icons::Provider::Muse)
        .preview("Implement · form and validation")
}

fn plan_worker() -> SessionSummary {
    SessionSummary::new("worker-plan", "Plan", AgentState::Running, "1m")
        .provider(aui_icons::Provider::Claude)
        .preview("reading src/ui/")
}

fn browser_url() -> String {
    "localhost:5174/login".to_string()
}

fn stall_run_view_base() -> RunViewModel {
    sample::run_view_sample()
}

fn block_plan_started() -> Block {
    match sample::run_plan_started() {
        Block::RunPlan(mut plan) => {
            plan.started_at = Some("14:02".into());
            Block::RunPlan(plan)
        }
        _ => unreachable!(),
    }
}

fn plan_stage_progress() -> Block {
    Block::RunProgress(RunProgress {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        stage_index: 0,
        stage_count: 5,
        stage_name: "Plan".into(),
        working: 1,
        verifying: 0,
        needs_you: 0,
        accepted: 0,
        elapsed: "1m".into(),
        tokens: None,
        paused: false,
        waiting_reason: None,
        lines: vec![TaskLine {
            task_id: "t-plan".into(),
            stage: "Plan".into(),
            title: "Plan · Opus".into(),
            provider: Provider::Claude,
            model: Some("opus 4.6".into()),
            state: TaskState::Working,
            activity: "read-only · reading src/ui/ · probing gate on main".into(),
            elapsed: "1m".into(),
            rungs: Vec::new(),
        }],
    })
}

fn question_answered_leave_out() -> Block {
    match sample::run_question_answered() {
        Block::RunQuestion(mut q) => {
            q.answer = Some("Leave it out".into());
            q.answered_at = Some("14:31".into());
            Block::RunQuestion(q)
        }
        _ => unreachable!(),
    }
}

fn review_progress() -> Block {
    Block::RunProgress(RunProgress {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        stage_index: 2,
        stage_count: 5,
        stage_name: "Review".into(),
        working: 1,
        verifying: 0,
        needs_you: 0,
        accepted: 2,
        elapsed: "18m".into(),
        tokens: Some("96k".into()),
        paused: false,
        waiting_reason: None,
        lines: vec![
            TaskLine {
                task_id: "t-review-diff".into(),
                stage: "Review".into(),
                title: "Review · Codex".into(),
                provider: Provider::Codex,
                model: Some("codex-5.2".into()),
                state: TaskState::Working,
                activity: "reading diff · criteria · gate output".into(),
                elapsed: "3m".into(),
                rungs: Vec::new(),
            },
            TaskLine {
                task_id: "t-review-notes".into(),
                stage: "Review".into(),
                title: "Review · Codex".into(),
                provider: Provider::Codex,
                model: Some("codex-5.2".into()),
                state: TaskState::Working,
                activity: "2 findings so far (1 high)".into(),
                elapsed: "3m".into(),
                rungs: Vec::new(),
            },
        ],
    })
}

fn fix_progress() -> Block {
    Block::RunProgress(RunProgress {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        stage_index: 3,
        stage_count: 5,
        stage_name: "Fix".into(),
        working: 0,
        verifying: 1,
        needs_you: 0,
        accepted: 3,
        elapsed: "24m".into(),
        tokens: Some("120k".into()),
        paused: false,
        waiting_reason: None,
        lines: vec![TaskLine {
            task_id: "t-fix-debug-log".into(),
            stage: "Fix".into(),
            title: "Fix · password reaches debug log (high)".into(),
            provider: Provider::Muse,
            model: Some("muse-spark-1.3".into()),
            state: TaskState::Verifying,
            activity: "running the checks".into(),
            elapsed: "2m".into(),
            rungs: vec![
                Rung { name: "tests".into(), state: RungState::Pass, detail: None },
                Rung { name: "mutation".into(), state: RungState::Running, detail: None },
            ],
        }],
    })
}

fn verify_progress() -> Block {
    Block::RunProgress(RunProgress {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        stage_index: 4,
        stage_count: 5,
        stage_name: "Verify".into(),
        working: 1,
        verifying: 0,
        needs_you: 0,
        accepted: 4,
        elapsed: "29m".into(),
        tokens: Some("140k".into()),
        paused: false,
        waiting_reason: None,
        lines: vec![
            TaskLine {
                task_id: "t-verify-browser".into(),
                stage: "Verify".into(),
                title: "Verify · evaluator in the browser".into(),
                provider: Provider::Codex,
                model: None,
                state: TaskState::Working,
                activity: "criterion 2 of 3 · screenshot taken".into(),
                elapsed: "2m".into(),
                rungs: Vec::new(),
            },
            TaskLine {
                task_id: "t-held-out".into(),
                stage: "Verify".into(),
                title: "Held-out checks".into(),
                provider: Provider::Codex,
                model: None,
                state: TaskState::Working,
                activity: "2 of 2 passed".into(),
                elapsed: "2m".into(),
                rungs: Vec::new(),
            },
        ],
    })
}

fn worker_transcript_items() -> Vec<CentreItem> {
    vec![
        CentreItem::BackLink("Back to lead session".into()),
        CentreItem::WorkerHead(sample::worker_header_sample()),
        CentreItem::Lead("Implementing the auth call against the agreed contract.".into()),
        CentreItem::Block(sample_tool_edit()),
        CentreItem::Block(sample_tool_test()),
        CentreItem::Block(sample_subagent_card()),
    ]
}

fn sample_tool_edit() -> Block {
    Block::ToolCall {
        id: "tc-edit-form".into(),
        kind: aui::protocol::ToolKind::Edit,
        verb: "Edited".into(),
        target: "src/login/Form.tsx".into(),
        status: aui::protocol::ToolStatus::Success,
        duration_ms: Some(4_200),
        body: aui::protocol::ToolBody::Edit {
            diff: aui::protocol::Diff {
                path: "src/login/Form.tsx".into(),
                hunks: vec![aui::protocol::Hunk {
                    header: "@@ -12,3 +12,5 @@ export function LoginForm".into(),
                    lines: vec![
                        aui::protocol::DiffLine {
                            kind: aui::protocol::DiffKind::Add,
                            old_no: None,
                            new_no: Some(12),
                            text: "<EmailField keepOnError />".into(),
                        },
                        aui::protocol::DiffLine {
                            kind: aui::protocol::DiffKind::Add,
                            old_no: None,
                            new_no: Some(13),
                            text: "<InlineErrors />".into(),
                        },
                    ],
                }],
                added: 2,
                removed: 0,
            },
        },
        diff_stat: None,
    }
}

fn sample_tool_test() -> Block {
    Block::ToolCall {
        id: "tc-npm-test".into(),
        kind: aui::protocol::ToolKind::Shell,
        verb: "Ran".into(),
        target: "npm test".into(),
        status: aui::protocol::ToolStatus::Success,
        duration_ms: Some(8_100),
        body: aui::protocol::ToolBody::Shell {
            output_lines: vec!["12 passed".into()],
            exit_code: Some(0),
            live: false,
        },
        diff_stat: None,
    }
}

fn sample_subagent_card() -> Block {
    Block::ToolCall {
        id: "tc-subagent-muse".into(),
        kind: aui::protocol::ToolKind::SubAgent,
        verb: "Delegated".into(),
        target: "form-checks".into(),
        status: aui::protocol::ToolStatus::Success,
        duration_ms: Some(21_000),
        body: aui::protocol::ToolBody::SubAgent { turns: Vec::new() },
        diff_stat: None,
    }
}

// ---- scenario A ----------------------------------------------------------

fn scenario_a() -> Scenario {
    let user_prompt = "build the login screen for this, plan with opus, implement with muse and then have codex do a code review then finally do all the fixes with muse and give me the preview here in the browser".to_string();
    let base_sidebar = || vec![lead_row()];
    let mk = |narration: &'static str,
              sidebar: Vec<SessionSummary>,
              items: Vec<CentreItem>,
              right: RightPane|
     -> Step {
        Step {
            narration,
            sidebar,
            active_row: "lead",
            centre_title: "Login screen run",
            centre_provider: aui_icons::Provider::Muse,
            items,
            steer: false,
            goal_pin: None,
            right,
            peer_goto: None,
        }
    };
    let running = || sample::run_progress_running();
    let steps = vec![
        mk(
            "The run plan, proposed: five stages with the models you named.",
            base_sidebar(),
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(sample::run_plan_proposed()),
            ],
            RightPane::Closed,
        ),
        mk(
            "You said “go”: the plan started at 14:02 and the Plan worker is reading.",
            vec![lead_row().child(plan_worker())],
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::User("go".into()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(plan_stage_progress()),
            ],
            RightPane::Closed,
        ),
        mk(
            "Implement: two Muse workers, one verifying, one still editing.",
            vec![lead_row()
                .child(plan_worker())
                .child(SessionSummary::new("worker-form", "Implement · form", AgentState::Running, "6m").pulse())
                .child(SessionSummary::new("worker-auth", "Implement · auth call", AgentState::Running, "4m").pulse())],
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(running()),
            ],
            RightPane::Closed,
        ),
        mk(
            "A mid-turn steer goes straight to the Muse worker on form and validation.",
            base_sidebar(),
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(running()),
                CentreItem::User("tell the implementer to use our existing Button component".into()),
                CentreItem::Lead("Sent to the Muse worker on \"form and validation\" (mid-turn).".into()),
            ],
            RightPane::Closed,
        ),
        mk(
            "A contract change needs you: Remember me — add it, or leave it out?",
            vec![lead_row()
                .status(aui::nav::RowStatusKind::Asked, "\"Remember me\"?")
                .child(plan_worker())],
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(sample::run_progress_needs_you()),
                CentreItem::Block(sample::run_question_open()),
            ],
            RightPane::Closed,
        ),
        mk(
            "Answered “Leave it out”: Review reads the diff with Codex.",
            base_sidebar(),
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(running()),
                CentreItem::Block(question_answered_leave_out()),
                CentreItem::Block(review_progress()),
            ],
            RightPane::Closed,
        ),
        mk(
            "Fix: the high finding first — the password must never reach the debug log.",
            base_sidebar(),
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(running()),
                CentreItem::Block(question_answered_leave_out()),
                CentreItem::Block(review_progress()),
                CentreItem::Block(fix_progress()),
            ],
            RightPane::Closed,
        ),
        mk(
            "Verify: the evaluator drives the real page in the browser pane.",
            base_sidebar(),
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(running()),
                CentreItem::Block(question_answered_leave_out()),
                CentreItem::Block(review_progress()),
                CentreItem::Block(fix_progress()),
                CentreItem::Block(verify_progress()),
            ],
            RightPane::Browser,
        ),
        mk(
            "Finished: every criterion carries its evidence, and the gaps are named.",
            base_sidebar(),
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(running()),
                CentreItem::Block(question_answered_leave_out()),
                CentreItem::Block(review_progress()),
                CentreItem::Block(fix_progress()),
                CentreItem::Block(verify_progress()),
                CentreItem::Block(sample::run_summary_finished()),
            ],
            RightPane::Browser,
        ),
        mk(
            "A preference, remembered: Codex always reviews from here on.",
            base_sidebar(),
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(running()),
                CentreItem::Block(question_answered_leave_out()),
                CentreItem::Block(review_progress()),
                CentreItem::Block(fix_progress()),
                CentreItem::Block(verify_progress()),
                CentreItem::Block(sample::run_summary_finished()),
                CentreItem::User("nice. from now on always review with codex".into()),
                CentreItem::Block(sample::run_note_preference()),
            ],
            RightPane::Closed,
        ),
        mk(
            "Merged: a91f04c lands on main.",
            base_sidebar(),
            vec![
                CentreItem::User(user_prompt.clone()),
                CentreItem::Block(block_plan_started()),
                CentreItem::Block(running()),
                CentreItem::Block(question_answered_leave_out()),
                CentreItem::Block(review_progress()),
                CentreItem::Block(fix_progress()),
                CentreItem::Block(verify_progress()),
                CentreItem::User("merge".into()),
                CentreItem::Block(sample::run_summary_merged()),
            ],
            RightPane::Closed,
        ),
        Step {
            narration: "Inside the worker: its own transcript, its own composer.",
            sidebar: vec![lead_row().child(
                SessionSummary::new("worker-auth", "Implement · auth call", AgentState::Running, "4m")
                    .provider(aui_icons::Provider::Muse)
                    .pulse()
                    .preview("editing src/auth/login.ts"),
            )],
            active_row: "worker-auth",
            centre_title: "Implement · auth call",
            centre_provider: aui_icons::Provider::Muse,
            items: worker_transcript_items(),
            steer: true,
            goal_pin: None,
            right: RightPane::Closed,
            peer_goto: None,
        },
    ];
    Scenario { id: 'A', name: "A Pipeline", steps }
}

// ---- scenario B ----------------------------------------------------------

fn explore_card() -> Block {
    Block::ToolCall {
        id: "tc-explore-sessions".into(),
        kind: aui::protocol::ToolKind::SubAgent,
        verb: "Explore".into(),
        target: "where sessions are persisted".into(),
        status: aui::protocol::ToolStatus::Success,
        duration_ms: Some(48_000),
        body: aui::protocol::ToolBody::Search {
            hits: vec![
                aui::protocol::SearchHit {
                    path: "crates/store/src/store.rs".into(),
                    line: 41,
                    snippet: "save_session".into(),
                },
                aui::protocol::SearchHit {
                    path: "crates/store/src/store.rs".into(),
                    line: 88,
                    snippet: "restore_session".into(),
                },
            ],
        },
        diff_stat: None,
    }
}

fn list_files_card() -> Block {
    Block::ToolCall {
        id: "tc-list-repo".into(),
        kind: aui::protocol::ToolKind::SubAgent,
        verb: "Agent".into(),
        target: "list_repo_files".into(),
        status: aui::protocol::ToolStatus::Running,
        duration_ms: None,
        body: aui::protocol::ToolBody::SubAgent { turns: Vec::new() },
        diff_stat: None,
    }
}

fn codex_review_summary() -> Block {
    Block::RunSummary(RunSummary {
        run_id: "run-codex-review".into(),
        title: "Codex review".into(),
        outcome: aui::protocol::RunOutcome::Finished,
        elapsed: "4m".into(),
        workers: 1,
        criteria: vec![
            aui::protocol::CriterionResult {
                text: "restore replays the write-ahead log before opening the store".into(),
                met: true,
                evidence: Vec::new(),
            },
            aui::protocol::CriterionResult {
                text: "snapshot compaction holds the stat lock across rename".into(),
                met: true,
                evidence: Vec::new(),
            },
            aui::protocol::CriterionResult {
                text: "cold restore warms the index in the background".into(),
                met: true,
                evidence: Vec::new(),
            },
        ],
        additions: 0,
        deletions: 0,
        files: 0,
        review_rounds: 0,
        extra: None,
        not_proven: Vec::new(),
        next_hint: "Say \"fix these\" to hand them to a worker.".into(),
        merged: None,
    })
}

fn one_line_review_progress() -> Block {
    Block::RunProgress(RunProgress {
        run_id: "run-codex-review".into(),
        title: "Session restore review".into(),
        stage_index: 0,
        stage_count: 1,
        stage_name: "Review".into(),
        working: 1,
        verifying: 0,
        needs_you: 0,
        accepted: 0,
        elapsed: "2m".into(),
        tokens: None,
        paused: false,
        waiting_reason: None,
        lines: vec![TaskLine {
            task_id: "t-codex-review".into(),
            stage: "Review".into(),
            title: "Review · Codex".into(),
            provider: Provider::Codex,
            model: Some("codex-5.2".into()),
            state: TaskState::Working,
            activity: "read-only · reading crates/store …".into(),
            elapsed: "2m".into(),
            rungs: Vec::new(),
        }],
    })
}

fn muse_subagent_row(status: aui::protocol::ToolStatus, verb: &'static str, target: &'static str) -> Block {
    Block::ToolCall {
        id: format!("tc-muse-{target}"),
        kind: aui::protocol::ToolKind::SubAgent,
        verb: verb.into(),
        target: target.into(),
        status,
        duration_ms: if status == aui::protocol::ToolStatus::Success { Some(30_000) } else { None },
        body: aui::protocol::ToolBody::SubAgent { turns: Vec::new() },
        diff_stat: None,
    }
}

fn scenario_b() -> Scenario {
    let ask = "why is session restore slow? have codex double-check whatever you find".to_string();
    let sidebar = || {
        vec![SessionSummary::new("restore", "Session restore", AgentState::Running, "3m")
            .provider(aui_icons::Provider::Claude)
            .preview("why is session restore slow?")]
    };
    let mk = |narration: &'static str, items: Vec<CentreItem>| Step {
        narration,
        sidebar: sidebar(),
        active_row: "restore",
        centre_title: "Session restore",
        centre_provider: aui_icons::Provider::Claude,
        items,
        steer: false,
        goal_pin: None,
        right: RightPane::Closed,
        peer_goto: None,
    };
    let steps = vec![
        mk("An ordinary Claude session: the Explore subagent reports back in 48 s.",
            vec![
                CentreItem::User(ask.clone()),
                CentreItem::Block(explore_card()),
                CentreItem::Lead("Restore replays the full snapshot on every open; the index rebuild is the slow half.".into()),
            ],
        ),
        mk(
            "Baaz hands the finding to Codex: a one-task run, then a compact summary.",
            vec![
                CentreItem::User(ask.clone()),
                CentreItem::Block(explore_card()),
                CentreItem::Block(one_line_review_progress()),
                CentreItem::Block(codex_review_summary()),
            ],
        ),
        mk(
            "A Muse session instead: two native subagents, accepted, then running.",
            vec![
                CentreItem::User(ask.clone()),
                CentreItem::Block(muse_subagent_row(aui::protocol::ToolStatus::Pending, "Delegated", "explorer")),
                CentreItem::Block(muse_subagent_row(aui::protocol::ToolStatus::Running, "Delegated", "code-reviewer")),
                CentreItem::Block(Block::RunNote(aui::protocol::RunNote {
                    text: "explorer accepted · code-reviewer running".into(),
                    note_kind: aui::protocol::NoteKind::Info,
                })),
                CentreItem::Controls(aui::protocol::SubagentControls {
                    message: true,
                    follow_up: false,
                    interrupt: true,
                    stop: true,
                }),
            ],
        ),
        mk(
            "Both Muse subagents land: explorer first, then the code review.",
            vec![
                CentreItem::User(ask.clone()),
                CentreItem::Block(muse_subagent_row(aui::protocol::ToolStatus::Success, "Delegated", "explorer")),
                CentreItem::Block(muse_subagent_row(aui::protocol::ToolStatus::Success, "Delegated", "code-reviewer")),
                CentreItem::Controls(aui::protocol::SubagentControls {
                    message: false,
                    follow_up: true,
                    interrupt: false,
                    stop: false,
                }),
            ],
        ),
        mk(
            "A Codex session instead: activity cards named from the agent path.",
            vec![
                CentreItem::User(ask.clone()),
                CentreItem::Block(list_files_card()),
                CentreItem::Muted("Waiting on 1 agent".into()),
            ],
        ),
    ];
    Scenario { id: 'B', name: "B Subagents", steps }
}

// ---- scenario C ----------------------------------------------------------

fn muse_goal(active: bool) -> Block {
    Block::Goal {
        objective: "Implement the login screen".into(),
        status: if active { "active".into() } else { "complete".into() },
        percent_complete: Some(if active { 0.0 } else { 100.0 }),
        current_work: None,
        next_work: None,
        provider: Some(Provider::Muse),
        tokens_used: Some(if active { 0 } else { 41_200 }),
        token_budget: Some(80_000),
        elapsed: Some(if active { "0 s".into() } else { "11m".into() }),
        turn: None,
        last_reason: None,
        gated_by_baaz: false,
        reopened: false,
    }
}

fn codex_gated_complete() -> Block {
    Block::Goal {
        objective: "Make python3 -m unittest -q exit 0 by fixing calc.py".into(),
        status: "complete".into(),
        percent_complete: Some(12.0),
        current_work: None,
        next_work: None,
        provider: Some(Provider::Codex),
        tokens_used: Some(9578),
        token_budget: Some(80_000),
        elapsed: Some("19 s".into()),
        turn: None,
        last_reason: None,
        gated_by_baaz: true,
        reopened: false,
    }
}

fn codex_gated_active() -> Block {
    Block::Goal {
        objective: "Make python3 -m unittest -q exit 0 by fixing calc.py".into(),
        status: "active".into(),
        percent_complete: Some(86.0),
        current_work: None,
        next_work: None,
        provider: Some(Provider::Claude),
        tokens_used: Some(12_400),
        token_budget: Some(80_000),
        elapsed: Some("4m".into()),
        turn: Some(3),
        last_reason: Some("2 tests still failing in test_calc.py".into()),
        gated_by_baaz: true,
        reopened: false,
    }
}

fn muse_workflow(children: Vec<WorkflowChild>, provisional: bool) -> Block {
    Block::NativeWorkflow(NativeWorkflow {
        provider: Provider::Muse,
        name: "Login screen fixes".into(),
        children,
        can_retry: true,
        can_skip: true,
        provisional,
    })
}

fn wf_child(id: &str, label: &str, state: TaskState) -> WorkflowChild {
    WorkflowChild { child_id: id.into(), attempt: 1, label: label.into(), phase: None, status: state }
}

fn muse_workflow_failed() -> Block {
    muse_workflow(
        vec![
            wf_child("w-discover", "discover", TaskState::Accepted),
            wf_child("w-port-1", "port · form", TaskState::Failed),
            wf_child("w-port-2", "port · auth call", TaskState::Working),
            wf_child("w-port-3", "port · styles", TaskState::Queued),
            wf_child("w-verify", "verify", TaskState::Queued),
        ],
        true,
    )
}

fn muse_workflow_retrying() -> Block {
    muse_workflow(
        vec![
            wf_child("w-discover", "discover", TaskState::Accepted),
            WorkflowChild {
                child_id: "w-port-1".into(),
                attempt: 2,
                label: "port · form".into(),
                phase: None,
                status: TaskState::Working,
            },
            wf_child("w-port-2", "port · auth call", TaskState::Working),
            wf_child("w-port-3", "port · styles", TaskState::Queued),
            wf_child("w-verify", "verify", TaskState::Queued),
        ],
        true,
    )
}

fn scenario_c() -> Scenario {
    let sidebar = || {
        vec![SessionSummary::new("calc", "Calc goals", AgentState::Running, "4m")
            .provider(aui_icons::Provider::Codex)
            .preview("Make unittest exit 0")]
    };
    let mk = |narration: &'static str, items: Vec<CentreItem>| Step {
        narration,
        sidebar: sidebar(),
        active_row: "calc",
        centre_title: "Calc goals",
        centre_provider: aui_icons::Provider::Codex,
        items,
        steer: false,
        goal_pin: None,
        right: RightPane::Closed,
        peer_goto: None,
    };
    let steps = vec![
        mk("Codex starts the calc goal: 6.7k of 80k tokens, 7 s in.", vec![
            CentreItem::User("make unittest pass".into()),
            CentreItem::Block(sample::goal_codex_active()),
        ]),
        mk("…and lands it: complete at 9.6k of 80k, 19 s.", vec![
            CentreItem::User("make unittest pass".into()),
            CentreItem::Block(sample::goal_codex_active()),
            CentreItem::Block(sample::goal_codex_complete()),
        ]),
        mk(
            "Claude's goal on turn 3 (provisional: verdict frames not yet probed).",
            vec![
                CentreItem::User("make unittest pass".into()),
                CentreItem::Block(sample::goal_claude_active()),
                CentreItem::Muted("(provisional: verdict frames not yet probed)".into()),
            ],
        ),
        mk("Muse starts at 0%.", vec![
            CentreItem::User("build the login screen".into()),
            CentreItem::Block(muse_goal(true)),
        ]),
        mk("…and finishes at 100% (only start and end frames exist).", vec![
            CentreItem::User("build the login screen".into()),
            CentreItem::Block(muse_goal(true)),
            CentreItem::Block(muse_goal(false)),
        ]),
        mk("Baaz-gated: Codex says complete, and the gate believes it — for now.", vec![
            CentreItem::User("make unittest pass".into()),
            CentreItem::Block(codex_gated_complete()),
        ]),
        mk("The gate disagrees: Reopened — provider said done; tests still fail: 2 of 14.", vec![
            CentreItem::User("make unittest pass".into()),
            CentreItem::Block(codex_gated_complete()),
            CentreItem::Block(sample::goal_reopened()),
        ]),
        mk("…and the goal moves again.", vec![
            CentreItem::User("make unittest pass".into()),
            CentreItem::Block(codex_gated_complete()),
            CentreItem::Block(sample::goal_reopened()),
            CentreItem::Block(codex_gated_active()),
        ]),
        mk(
            "A Muse workflow (provisional on 1.4.3): one port child failed — Retry or Skip it.",
            vec![
                CentreItem::User("port the login screen".into()),
                CentreItem::Block(muse_workflow_failed()),
                CentreItem::Muted("(provisional: unavailable to MSP clients on 1.4.3)".into()),
            ],
        ),
        mk("Retried: the failed child runs again.", vec![
            CentreItem::User("port the login screen".into()),
            CentreItem::Block(muse_workflow_retrying()),
        ]),
        mk("A Claude workflow instead: Review → Verify, agents per phase (provisional).", vec![
            CentreItem::User("validate the checkout".into()),
            CentreItem::Block(sample::workflow_claude_phases()),
            CentreItem::Muted("(provisional)".into()),
        ]),
    ];
    Scenario { id: 'C', name: "C Goals & workflows", steps }
}

// ---- scenario D ----------------------------------------------------------

fn approval_question() -> Block {
    Block::RunQuestion(RunQuestion {
        question_id: "q-rm-node-modules".into(),
        run_id: "run-auth-worker".into(),
        source_label: "QUESTION FROM THE RUN · IMPLEMENT · AUTH WORKER".into(),
        text: "Muse wants to run `rm -rf node_modules && npm ci` in the auth worker. Allow?".into(),
        hint: "the worker is paused".into(),
        options: vec![
            RunQuestionOption { id: "allow".into(), label: "Allow once".into(), recommended: false },
            RunQuestionOption { id: "deny".into(), label: "Deny".into(), recommended: true },
        ],
        answer: None,
        answered_at: None,
    })
}

fn quota_progress() -> Block {
    match sample::run_progress_quota_wait() {
        Block::RunProgress(mut p) => {
            p.waiting_reason = Some("Waiting for Muse · resets 16:00".into());
            Block::RunProgress(p)
        }
        _ => unreachable!(),
    }
}

fn quota_question() -> Block {
    Block::RunQuestion(RunQuestion {
        question_id: "q-quota".into(),
        run_id: "run-login-screen".into(),
        source_label: "QUESTION FROM THE RUN · IMPLEMENT".into(),
        text: "Muse's window resets at 16:00. Wait, or move the 2 implement tasks to Codex?".into(),
        hint: "the run is waiting".into(),
        options: vec![
            RunQuestionOption { id: "wait".into(), label: "Wait".into(), recommended: true },
            RunQuestionOption { id: "move".into(), label: "Move to Codex".into(), recommended: false },
        ],
        answer: None,
        answered_at: None,
    })
}

fn scenario_d() -> Scenario {
    let sidebar = || {
        vec![SessionSummary::new("auth", "Auth worker run", AgentState::Waiting, "9m")
            .provider(aui_icons::Provider::Muse)
            .preview("Implement · auth call")]
    };
    let mk = |narration: &'static str, items: Vec<CentreItem>| Step {
        narration,
        sidebar: sidebar(),
        active_row: "auth",
        centre_title: "Auth worker run",
        centre_provider: aui_icons::Provider::Muse,
        items,
        steer: false,
        goal_pin: None,
        right: RightPane::Closed,
        peer_goto: None,
    };
    let steps = vec![
        mk(
            "A provider approval, surfaced in the lead chat as a run question.",
            vec![
                CentreItem::Block(sample::run_progress_needs_you()),
                CentreItem::Block(approval_question()),
            ],
        ),
        mk(
            "A blocker the lead settles itself: contract unchanged, back to work.",
            vec![
                CentreItem::Block(sample::run_progress_needs_you()),
                CentreItem::Block(approval_question()),
                CentreItem::Lead("Ruled on a worker question: keep the existing API shape (contract unchanged).".into()),
                CentreItem::Block(sample::run_progress_running()),
            ],
        ),
        mk("Quota: Muse's window resets at 16:00 — wait it out, or move to Codex?", vec![
            CentreItem::Block(quota_progress()),
            CentreItem::Block(quota_question()),
        ]),
        mk("“what's happening?” — the lead answers from the progress state.", vec![
            CentreItem::Block(quota_progress()),
            CentreItem::Block(quota_question()),
            CentreItem::User("what's happening?".into()),
            CentreItem::Lead("Waiting on Muse quota until 16:00: one task verifying, one editing src/auth/login.ts. Say “move to Codex” and I'll reassign both.".into()),
        ]),
        mk("“pause” — the header reads Paused and the lines settle.", vec![
            CentreItem::User("pause".into()),
            CentreItem::Block(sample::run_progress_paused()),
        ]),
    ];
    Scenario { id: 'D', name: "D Questions", steps }
}

// ---- scenario E ----------------------------------------------------------

fn reconciling_progress() -> Block {
    Block::RunProgress(RunProgress {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        stage_index: 1,
        stage_count: 5,
        stage_name: "Implement".into(),
        working: 0,
        verifying: 0,
        needs_you: 0,
        accepted: 0,
        elapsed: "—".into(),
        tokens: None,
        paused: false,
        waiting_reason: None,
        lines: vec![
            TaskLine {
                task_id: "t-impl-form".into(),
                stage: "Implement".into(),
                title: "Implement · form and validation".into(),
                provider: Provider::Muse,
                model: Some("muse-spark-1.3".into()),
                state: TaskState::Reconciling,
                activity: "reconciling…".into(),
                elapsed: "—".into(),
                rungs: Vec::new(),
            },
            TaskLine {
                task_id: "t-auth-call".into(),
                stage: "Implement".into(),
                title: "Implement · auth call".into(),
                provider: Provider::Muse,
                model: Some("muse-spark-1.3".into()),
                state: TaskState::Reconciling,
                activity: "reconciling…".into(),
                elapsed: "—".into(),
                rungs: Vec::new(),
            },
        ],
    })
}

fn resumed_progress() -> Block {
    Block::RunProgress(RunProgress {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        stage_index: 1,
        stage_count: 5,
        stage_name: "Implement".into(),
        working: 1,
        verifying: 1,
        needs_you: 0,
        accepted: 0,
        elapsed: "6m".into(),
        tokens: Some("48k".into()),
        paused: false,
        waiting_reason: None,
        lines: vec![
            TaskLine {
                task_id: "t-impl-form".into(),
                stage: "Implement".into(),
                title: "Implement · form and validation".into(),
                provider: Provider::Muse,
                model: Some("muse-spark-1.3".into()),
                state: TaskState::Verifying,
                activity: "resumed in the same session".into(),
                elapsed: "6m".into(),
                rungs: vec![Rung { name: "scope".into(), state: RungState::Pass, detail: None }],
            },
            TaskLine {
                task_id: "t-auth-call".into(),
                stage: "Implement".into(),
                title: "Implement · auth call".into(),
                provider: Provider::Muse,
                model: Some("muse-spark-1.3".into()),
                state: TaskState::Working,
                activity: "re-dispatched from saved work".into(),
                elapsed: "4m".into(),
                rungs: Vec::new(),
            },
        ],
    })
}

fn stall_run_view() -> RunViewModel {
    let mut view = stall_run_view_base();
    view.evidence = vec![
        EvidenceRow {
            task_id: "t-auth-call".into(),
            text: "nudged after 10 min idle".into(),
            evidence_kind: EvidenceKind::User,
        },
        EvidenceRow {
            task_id: "t-auth-call".into(),
            text: "interrupted and re-sent".into(),
            evidence_kind: EvidenceKind::User,
        },
    ];
    view
}

fn exhausted_progress() -> Block {
    Block::RunProgress(RunProgress {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        stage_index: 1,
        stage_count: 5,
        stage_name: "Implement".into(),
        working: 0,
        verifying: 1,
        needs_you: 1,
        accepted: 0,
        elapsed: "22m".into(),
        tokens: Some("90k".into()),
        paused: false,
        waiting_reason: None,
        lines: vec![
            TaskLine {
                task_id: "t-auth-call".into(),
                stage: "Implement".into(),
                title: "Implement · auth call".into(),
                provider: Provider::Muse,
                model: Some("muse-spark-1.3".into()),
                state: TaskState::NeedsYou,
                activity: "stopped after 2 recoveries".into(),
                elapsed: "22m".into(),
                rungs: Vec::new(),
            },
            TaskLine {
                task_id: "t-impl-form".into(),
                stage: "Implement".into(),
                title: "Implement · form and validation".into(),
                provider: Provider::Muse,
                model: Some("muse-spark-1.3".into()),
                state: TaskState::Verifying,
                activity: "running the checks".into(),
                elapsed: "6m".into(),
                rungs: vec![Rung { name: "tests".into(), state: RungState::Running, detail: None }],
            },
        ],
    })
}

fn exhausted_question() -> Block {
    Block::RunQuestion(RunQuestion {
        question_id: "q-auth-stopped".into(),
        run_id: "run-login-screen".into(),
        source_label: "QUESTION FROM THE RUN · IMPLEMENT · AUTH CALL".into(),
        text: "The auth-call worker stopped after 2 recoveries. How should the run continue?".into(),
        hint: "the worker is stopped".into(),
        options: vec![
            RunQuestionOption { id: "retry".into(), label: "Retry".into(), recommended: false },
            RunQuestionOption { id: "stop".into(), label: "Stop".into(), recommended: false },
            RunQuestionOption { id: "open".into(), label: "Open worker".into(), recommended: true },
        ],
        answer: None,
        answered_at: None,
    })
}

fn scenario_e() -> Scenario {
    let sidebar = || {
        vec![SessionSummary::new("login", "Login screen run", AgentState::Running, "6m")
            .provider(aui_icons::Provider::Muse)
            .preview("Implement · auth call")]
    };
    let mk = |narration: &'static str, items: Vec<CentreItem>, right: RightPane| Step {
        narration,
        sidebar: sidebar(),
        active_row: "login",
        centre_title: "Login screen run",
        centre_provider: aui_icons::Provider::Muse,
        items,
        steer: false,
        goal_pin: None,
        right,
        peer_goto: None,
    };
    let steps = vec![
        mk("Reopened after quit: every line reconciles before it trusts anything.", vec![
            CentreItem::Block(reconciling_progress()),
        ], RightPane::Closed),
        mk("Resolved: one worker resumed in place, the other re-dispatched.", vec![
            CentreItem::Block(reconciling_progress()),
            CentreItem::Block(resumed_progress()),
        ], RightPane::Closed),
        mk("A stall: nudged after 10 min idle, then interrupted and re-sent.", vec![
            CentreItem::Block(resumed_progress()),
            CentreItem::Lead("Nudged the auth-call worker after 10 min idle — no answer, so interrupted and re-sent the turn.".into()),
        ], RightPane::Run(stall_run_view())),
        mk("…and the line returns to working.", vec![
            CentreItem::Block(resumed_progress()),
            CentreItem::Lead("Nudged the auth-call worker after 10 min idle — no answer, so interrupted and re-sent the turn.".into()),
            CentreItem::Block(sample::run_progress_running()),
        ], RightPane::Closed),
        mk("Recoveries exhausted: the line stops and asks you what to do.", vec![
            CentreItem::Block(exhausted_progress()),
            CentreItem::Block(exhausted_question()),
        ], RightPane::Closed),
    ];
    Scenario { id: 'E', name: "E Recovery", steps }
}

// ---- scenario F ----------------------------------------------------------

fn sender_row_waiting() -> SessionSummary {
    SessionSummary::new("orch", "Orchestration in Baaz", AgentState::Idle, "2m")
        .provider(aui_icons::Provider::Claude)
        .preview("waiting for reply")
}

fn receiver_row(pulsing: bool, unread: bool) -> SessionSummary {
    let mut row = SessionSummary::new("audit", "Browser audit", AgentState::Running, "2m")
        .provider(aui_icons::Provider::Codex)
        .preview("checking registry.rs");
    if pulsing {
        row = row.pulse();
    }
    if unread {
        row = row.unread();
    }
    row
}

fn outgoing_sent() -> PeerMessage {
    PeerMessage {
        message_id: "pm-orch-1".into(),
        direction: PeerDirection::Outgoing,
        peer_title: "Browser audit".into(),
        peer_provider: Provider::Codex,
        peer_project: Some("agentic-ui".into()),
        text: "Do you touch registry.rs or cards/mod.rs? I'm about to start orchestration mocks.".into(),
        state: PeerState::Sent,
        reason: None,
        at: "14:20".into(),
        reply: None,
    }
}

fn outgoing_delivered() -> PeerMessage {
    let mut m = outgoing_sent();
    m.state = PeerState::Delivered;
    m
}

fn outgoing_replied() -> PeerMessage {
    let mut m = outgoing_delivered();
    m.state = PeerState::Replied;
    m.reply = Some(PeerReply {
        text: "No clash: I only append a Browse group at the end of registry.rs and add browse_* modules to cards/mod.rs. Keep yours at the end too.".into(),
        at: "14:26".into(),
    });
    m
}

fn outgoing_queued() -> PeerMessage {
    let mut m = outgoing_sent();
    m.state = PeerState::Queued;
    m
}

fn outgoing_unreachable() -> PeerMessage {
    let mut m = outgoing_sent();
    m.state = PeerState::Unreachable;
    m.reason = Some("the session is closed".into());
    m
}

fn incoming() -> PeerMessage {
    PeerMessage {
        message_id: "pm-audit-1".into(),
        direction: PeerDirection::Incoming,
        peer_title: "Orchestration in Baaz".into(),
        peer_provider: Provider::Claude,
        peer_project: Some("agentic-ui".into()),
        text: "Do you touch registry.rs or cards/mod.rs? I'm about to start orchestration mocks.".into(),
        state: PeerState::Delivered,
        reason: None,
        at: "14:20".into(),
        reply: None,
    }
}

fn receiver_work_card() -> Block {
    Block::ToolCall {
        id: "tc-audit-read".into(),
        kind: aui::protocol::ToolKind::Read,
        verb: "Read".into(),
        target: "registry.rs, cards/mod.rs · git status".into(),
        status: aui::protocol::ToolStatus::Success,
        duration_ms: Some(3_000),
        body: aui::protocol::ToolBody::Read { lines: 42 },
        diff_stat: None,
    }
}

fn scenario_f() -> Scenario {
    let sender_sidebar_waiting =
        || vec![sender_row_waiting(), receiver_row(true, false)];
    let sender_sidebar_replied =
        || vec![sender_row_waiting(), receiver_row(false, true)];
    let receiver_sidebar = || {
        vec![
            SessionSummary::new("audit", "Browser audit", AgentState::Running, "2m")
                .provider(aui_icons::Provider::Codex)
                .pulse()
                .preview("checking registry.rs"),
            SessionSummary::new("orch", "Orchestration in Baaz", AgentState::Idle, "2m")
                .provider(aui_icons::Provider::Claude)
                .preview("waiting for reply"),
        ]
    };
    let sender = |narration: &'static str,
                  sidebar: Vec<SessionSummary>,
                  items: Vec<CentreItem>,
                  peer_goto: Option<usize>|
     -> Step {
        Step {
            narration,
            sidebar,
            active_row: "orch",
            centre_title: "Orchestration in Baaz",
            centre_provider: aui_icons::Provider::Claude,
            items,
            steer: false,
            goal_pin: None,
            right: RightPane::Closed,
            peer_goto,
        }
    };
    let steps = vec![
        sender(
            "Two peers, not parent and child. You ask the browser session first.",
            vec![sender_row_waiting(), receiver_row(false, false)],
            vec![CentreItem::User(
                "ask the browser session whether it touches registry.rs or cards/mod.rs before we start".into(),
            )],
            None,
        ),
        sender(
            "Asked: the message is on its way, and the peer row pulses.",
            sender_sidebar_waiting(),
            vec![
                CentreItem::User(
                    "ask the browser session whether it touches registry.rs or cards/mod.rs before we start".into(),
                ),
                CentreItem::Lead("Asking the \"Browser audit\" session.".into()),
                CentreItem::Block(Block::PeerMessage(outgoing_delivered())),
            ],
            Some(2),
        ),
        Step {
            narration: "The receiver: a bubble from another session, its own work, its reply.",
            sidebar: receiver_sidebar(),
            active_row: "audit",
            centre_title: "Browser audit",
            centre_provider: aui_icons::Provider::Codex,
            items: vec![
                CentreItem::Block(Block::PeerMessage(incoming())),
                CentreItem::Block(receiver_work_card()),
                CentreItem::Lead("No clash on my side — checking my diff now.".into()),
                CentreItem::Lead("Replied to \"Orchestration in Baaz\".".into()),
            ],
            steer: false,
            goal_pin: None,
            right: RightPane::Closed,
            peer_goto: Some(3),
        },
        sender(
            "Answered: the reply lands quoted, two lines until you expand it.",
            sender_sidebar_replied(),
            vec![
                CentreItem::User(
                    "ask the browser session whether it touches registry.rs or cards/mod.rs before we start".into(),
                ),
                CentreItem::Lead("Asking the \"Browser audit\" session.".into()),
                CentreItem::Block(Block::PeerMessage(outgoing_replied())),
                CentreItem::Lead("Good — no clash. Starting the orchestration mocks at the end of the registry.".into()),
            ],
            Some(2),
        ),
        sender(
            "Variant: the receiver was mid-turn, so the message queues.",
            vec![sender_row_waiting(), receiver_row(false, false)],
            vec![
                CentreItem::User(
                    "ask the browser session whether it touches registry.rs or cards/mod.rs before we start".into(),
                ),
                CentreItem::Lead("Asking the \"Browser audit\" session.".into()),
                CentreItem::Block(Block::PeerMessage(outgoing_queued())),
            ],
            Some(2),
        ),
        sender(
            "Variant: the session is closed, so the message can never arrive.",
            vec![sender_row_waiting(), receiver_row(false, false)],
            vec![
                CentreItem::User(
                    "ask the browser session whether it touches registry.rs or cards/mod.rs before we start".into(),
                ),
                CentreItem::Lead("Asking the \"Browser audit\" session.".into()),
                CentreItem::Block(Block::PeerMessage(outgoing_unreachable())),
            ],
            Some(2),
        ),
    ];
    Scenario { id: 'F', name: "F Sessions", steps }
}

// ---- all -----------------------------------------------------------------

/// All six scenarios, A–F.
pub fn scenarios() -> Vec<Scenario> {
    vec![scenario_a(), scenario_b(), scenario_c(), scenario_d(), scenario_e(), scenario_f()]
}

/// The browser placeholder URL, in one place.
pub fn browser_url_string() -> String {
    browser_url()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scenario skeleton: six scenarios, A–F, each non-empty.
    #[test]
    fn six_scenarios_in_order() {
        let all = scenarios();
        assert_eq!(all.iter().map(|s| s.id).collect::<Vec<_>>(), vec!['A', 'B', 'C', 'D', 'E', 'F']);
        for s in &all {
            assert!(!s.steps.is_empty(), "scenario {} has no steps", s.id);
            for step in &s.steps {
                assert!(!step.sidebar.is_empty(), "scenario {} has a step with no sidebar", s.id);
                assert!(!step.items.is_empty(), "scenario {} has a step with no items", s.id);
            }
        }
    }

    /// Step counts the screenshot harness relies on (`goto:<n>` is 1-based).
    #[test]
    fn step_counts() {
        let all = scenarios();
        let counts: Vec<usize> = all.iter().map(|s| s.steps.len()).collect();
        assert_eq!(counts, vec![12, 5, 11, 5, 5, 6]);
    }

    /// Scenario A carries the exact copy the spec demands.
    #[test]
    fn scenario_a_copy() {
        let all = scenarios();
        let a = &all[0];
        let first_user = a.steps[0].items.iter().find_map(|item| match item {
            CentreItem::User(text) => Some(text.clone()),
            _ => None,
        });
        assert_eq!(
            first_user.as_deref(),
            Some("build the login screen for this, plan with opus, implement with muse and then have codex do a code review then finally do all the fixes with muse and give me the preview here in the browser")
        );
        // The question step asks about Remember-me with the recommended leave-out.
        let mut found_question = false;
        for item in &a.steps[4].items {
            if let CentreItem::Block(Block::RunQuestion(q)) = item {
                assert!(q.text.contains("Remember me"));
                assert!(q.options.iter().any(|o| o.label == "Leave it out" && o.recommended));
                found_question = true;
            }
        }
        assert!(found_question, "A5 must hold the open Remember-me question");
        // The merged summary names the commit.
        let mut found_merge = false;
        for item in &a.steps[10].items {
            if let CentreItem::Block(Block::RunSummary(s)) = item {
                assert_eq!(s.merged.as_deref(), Some("a91f04c"));
                found_merge = true;
            }
        }
        assert!(found_merge, "A11 must hold the merged summary");
    }

    /// Scenario F's replied step quotes the peer's answer.
    #[test]
    fn scenario_f_reply() {
        let all = scenarios();
        let f = &all[5];
        let mut found = false;
        for item in &f.steps[3].items {
            if let CentreItem::Block(Block::PeerMessage(m)) = item {
                if let Some(reply) = &m.reply {
                    assert!(reply.text.contains("No clash"));
                    found = true;
                }
            }
        }
        assert!(found, "F4 must quote the Browser audit reply");
    }
}
