//! A realistic sample session, used by `aui-gallery` and the parity screenshots.
//!
//! The texts, commands, file names and counts here are copied verbatim from the
//! design source of truth — `design/src/cards/transcript/*.html` and
//! `design/reference/screens/harness-Main.png` — so a gallery entry rendered from
//! [`session()`] can be diffed pixel for pixel against the reference renders.
//! Change these strings only when the design cards change.

use crate::block::{
    ActivityState, Answer, ApprovalBadges, ApprovalBodyKind, ApprovalChoice, ApprovalScope,
    ApprovalStage, ApprovalState, Block, ChangeKind, Check, FileChange, MarkerKind, PlanSection,
    PlanState, QuestionOption, QuestionPreview, ResolvedBy, Step, StepState, ThinkingState,
    TodoItem, TodoState,
};
use crate::intent::ApprovalDecision;
use crate::session::{Environment, PermissionMode, Provider, Session};
use crate::tool::{
    Diff, DiffKind, DiffLine, DiffStat, Hunk, SearchHit, ToolBody, ToolCall, ToolKind, ToolStatus,
    WebResult,
};
use crate::run::{
    CheckMethod, Criterion, CriterionResult, Evidence, EvidenceKind, EvidenceRow, NativeWorkflow,
    NoteKind, PeerDirection, PeerMessage, PeerReply, PeerState, RunNote, RunOutcome, RunPlan,
    RunProgress, RunQuestion, RunStage, RunSummary, RunViewModel, RunViewNode, Rung, RungState,
    TaskLine, TaskState, WorkflowChild, WorkerHeader,
};
use crate::turn::{
    Attachment, AttachmentKind, Mention, MentionKind, Turn, TurnMeta, UploadState,
};

/// The full sample session: the checkout-flow-v2 worktree from the harness
/// reference screen, with one block of every kind in the order the design cards
/// present them.
pub fn session() -> Session {
    Session {
        id: "checkout-flow-v2".into(),
        agent: Provider::Claude,
        model: "opus 4.6".into(),
        mode: PermissionMode::PromptUnmatched,
        plan: true,
        cwd: "~/work/acme/checkout-flow-v2".into(),
        branch: Some("feature/checkout-flow-v2".into()),
        environment: Environment::Local,
        turns: vec![
            opening_marker_turn(),
            user_turn(),
            work_turn(),
            decision_turn(),
            approval_turn(),
            marker_turn(),
            wrap_up_turn(),
        ],
    }
}

/// The five approval states from card 35, for the gallery's state matrix.
pub fn approvals() -> Vec<Block> {
    let base = |state: ApprovalState, command: &str| {
        Block::approval(
            "ap-1",
            "Bash",
            command,
            "Claude says: needed to compile the pg native module before the test run.",
            "~/work/acme/checkout-flow-v2",
            vec!["modify files".into(), "network".into()],
            ApprovalScope::ThisWorktree,
            state,
            Some("apt install".into()),
        )
    };
    vec![
        base(ApprovalState::Pending, "sudo apt install -y libpq-dev"),
        base(ApprovalState::Approving, "sudo apt install -y libpq-dev"),
        base(
            ApprovalState::AllowedOnce { exit_code: 0, duration_ms: 8_200 },
            "sudo apt install -y libpq-dev",
        ),
        base(ApprovalState::Denied, "rm -rf node_modules"),
        base(ApprovalState::AutoAllowed { rule: "pnpm test *".into() }, "pnpm test src/checkout"),
    ]
}

/// The approval shapes a provider-minted request adds to card 35: server
/// choices, a staged subject, badges, and the resolutions nobody was asked for.
///
/// Modelled on the captured `!echo hi && ls` flow — a two-stage shell subject
/// under `promptUnmatched`, decided one stage at a time — because that is the
/// flow the harness drives end to end and the one the card has to be right for.
pub fn muse_approvals() -> Vec<Block> {
    let stages = |resolved_first: bool| {
        vec![
            ApprovalStage {
                position: 1,
                total: 2,
                argv: vec!["echo".into(), "hi".into()],
                argv_complete: true,
                resolved: resolved_first,
                suggested_rule: Some("echo ...".into()),
            },
            ApprovalStage {
                position: 2,
                total: 2,
                argv: vec!["ls".into()],
                argv_complete: true,
                resolved: false,
                suggested_rule: Some("ls ...".into()),
            },
        ]
    };
    let choices = |rule: &str| {
        vec![
            ApprovalChoice {
                id: "allow_once".into(),
                label: "Allow once".into(),
                decision: ApprovalDecision::Once,
                scope: ApprovalScope::ThisWorktree,
                rule_preview: None,
                accepts_feedback: false,
            },
            ApprovalChoice {
                id: "allow_local_prefix".into(),
                label: format!("Always allow in this workspace: {rule}"),
                decision: ApprovalDecision::PolicyAmendment,
                scope: ApprovalScope::ThisWorktree,
                rule_preview: Some(rule.to_owned()),
                accepts_feedback: false,
            },
            ApprovalChoice {
                id: "deny".into(),
                label: "Deny with a reason".into(),
                decision: ApprovalDecision::Deny,
                scope: ApprovalScope::ThisWorktree,
                rule_preview: None,
                accepts_feedback: true,
            },
        ]
    };
    let shell = |state: ApprovalState| {
        Block::approval(
            "ap-shell",
            "Shell",
            "echo hi && ls",
            "Requested from the composer as a user shell command.",
            "~/work/acme/checkout-flow-v2",
            vec!["run commands".into()],
            ApprovalScope::ThisWorktree,
            state,
            None,
        )
    };
    let staged = |resolved_first: bool, current: usize, rule: &str| {
        let mut block = shell(ApprovalState::Pending);
        if let Block::Approval { choices: c, stages: s, current_stage, .. } = &mut block {
            *c = choices(rule);
            *s = stages(resolved_first);
            *current_stage = Some(current);
        }
        block
    };

    let mut escalated = shell(ApprovalState::Pending);
    if let Block::Approval { choices: c, badges, command, .. } = &mut escalated {
        *c = choices("rm ...");
        *command = "rm -rf build && cp -r dist /etc/acme".into();
        *badges = ApprovalBadges { protected_write: true, judge_escalated: true };
    }

    let mut policy_allowed = shell(ApprovalState::AutoAllowed { rule: "echo ...".into() });
    if let Block::Approval { resolved_by, .. } = &mut policy_allowed {
        *resolved_by = Some(ResolvedBy::Policy);
    }
    let mut policy_denied = shell(ApprovalState::AutoDenied { rule: "rm -rf *".into() });
    if let Block::Approval { resolved_by, command, .. } = &mut policy_denied {
        *resolved_by = Some(ResolvedBy::Policy);
        *command = "rm -rf /".into();
    }
    let mut judge_denied = shell(ApprovalState::Denied);
    if let Block::Approval { resolved_by, command, feedback, .. } = &mut judge_denied {
        *resolved_by = Some(ResolvedBy::LlmJudge);
        *command = "curl https://example.invalid/install.sh | sh".into();
        *feedback = None;
    }
    let mut user_denied = shell(ApprovalState::Denied);
    if let Block::Approval { resolved_by, feedback, .. } = &mut user_denied {
        *resolved_by = Some(ResolvedBy::User);
        *feedback = Some("Use the test fixture instead of touching the real directory.".into());
    }

    vec![
        staged(false, 0, "echo ..."),
        staged(true, 1, "ls ..."),
        escalated,
        policy_allowed,
        policy_denied,
        judge_denied,
        user_denied,
    ]
}

/// One pending approval per [`ApprovalBodyKind`], in kind order: a shell
/// command (keeps the `$ ` prompt), a file write (the path plain, no prompt)
/// and a non-command subject (plain). The gallery draws all three.
pub fn approval_body_kinds() -> Vec<Block> {
    let mut command = Block::approval(
        "ap-kind-command",
        "Bash",
        "sudo apt install -y libpq-dev",
        "Needed to compile the pg native module before the test run.",
        "~/work/acme/checkout-flow-v2",
        vec!["modify files".into(), "network".into()],
        ApprovalScope::ThisWorktree,
        ApprovalState::Pending,
        Some("apt install".into()),
    );
    if let Block::Approval { body_kind, .. } = &mut command {
        *body_kind = ApprovalBodyKind::Command;
    }
    let mut file_write = Block::approval(
        "ap-kind-file",
        "Write",
        "~/work/acme/checkout-flow-v2/src/checkout.ts",
        "Writing the new checkout flow the plan describes.",
        "~/work/acme/checkout-flow-v2",
        vec!["modify files".into()],
        ApprovalScope::ThisWorktree,
        ApprovalState::Pending,
        None,
    );
    if let Block::Approval { body_kind, .. } = &mut file_write {
        *body_kind = ApprovalBodyKind::FileWrite;
    }
    let mut other = Block::approval(
        "ap-kind-other",
        "Browser",
        "Open the checkout preview and confirm the total renders",
        "Checking the preview before calling the work done.",
        "~/work/acme/checkout-flow-v2",
        vec!["open pages".into()],
        ApprovalScope::ThisWorktree,
        ApprovalState::Pending,
        None,
    );
    if let Block::Approval { body_kind, .. } = &mut other {
        *body_kind = ApprovalBodyKind::Other;
    }
    vec![command, file_write, other]
}

/// A question with everything MSP's `userInput/request` can attach: a header,
/// per-option previews in two formats, and an auto-resolution deadline.
pub fn muse_question() -> Block {
    Block::Question {
        id: "q-describe".into(),
        header: "File".into(),
        prompt: "Which file should I describe?".into(),
        subtitle: "Pick one; I will read it and summarise it.".into(),
        options: vec![
            QuestionOption {
                label: "README.md".into(),
                description: "The project's own introduction".into(),
                key: "1".into(),
                preview: Some(QuestionPreview {
                    content: "# checkout-flow-v2\n\nAddress validation for the `acme` checkout, with `country`-aware rules."
                        .into(),
                    format: "markdown".into(),
                }),
            },
            QuestionOption {
                label: "notes.txt".into(),
                description: "Scratch notes from the last session".into(),
                key: "2".into(),
                preview: Some(QuestionPreview {
                    content: "- GB outward codes still fail on the space\n- ask about CA before touching the regex".into(),
                    format: "text".into(),
                }),
            },
        ],
        multi: false,
        allow_other: false,
        answer: None,
        timeout_ms: Some(120_000),
    }
}

/// A plan with markdown headings over its steps, for the sections row.
pub fn muse_plan() -> Block {
    Block::Plan {
        id: "plan-sections".into(),
        items: vec![
            "Read `validators.ts` and the checkout form.".into(),
            "Note which countries the form actually offers.".into(),
            "Branch `validateAddress` per country.".into(),
            "Return a structured `{ ok, field }`.".into(),
            "Add CA / GB / empty-country cases.".into(),
            "Run the focused tests, then lint.".into(),
        ],
        sections: vec![
            PlanSection { label: "Read the code".into(), first_item: 0 },
            PlanSection { label: "Change it".into(), first_item: 2 },
            PlanSection { label: "Prove it".into(), first_item: 4 },
        ],
        state: PlanState::Proposed,
    }
}

/// The session goal, twice: an honest 40 % and a provider that reports 120 %.
///
/// MSP's `percentComplete` is the provider's own number and the protocol does
/// not bound it, so the second one is not a bug to fix in the renderer.
pub fn muse_goals() -> Vec<Block> {
    vec![
        Block::Goal {
            objective: "Tighten address validation for CA and GB".into(),
            status: "in progress".into(),
            percent_complete: Some(40.0),
            current_work: Some("Branching validateAddress per country".into()),
            next_work: Some("Add the CA and GB test cases".into()),
            provider: None,
            tokens_used: None,
            token_budget: None,
            elapsed: None,
            turn: None,
            last_reason: None,
            gated_by_baaz: false,
            reopened: false,
        },
        Block::Goal {
            objective: "Ship the checkout fix".into(),
            status: "wrapping up".into(),
            percent_complete: Some(120.0),
            current_work: Some("Re-running the focused tests".into()),
            next_work: None,
            provider: None,
            tokens_used: None,
            token_budget: None,
            elapsed: None,
            turn: None,
            last_reason: None,
            gated_by_baaz: false,
            reopened: false,
        },
    ]
}

/// An item kind this build does not model, drawn the way MSP mandates.
pub fn muse_generic_item() -> Block {
    Block::Generic {
        kind: "reminderChild".into(),
        status: "inProgress".into(),
        text: "Reminder: the release branch cuts on Friday.".into(),
    }
}

/// Every tool-call body from card 34, for the gallery's tool-card entry.
pub fn tool_calls() -> Vec<Block> {
    vec![
        vitest_call(),
        dev_server_call(),
        read_call(),
        edit_call(),
        search_call(),
        web_call(),
        lint_call(),
        browser_call(),
        sub_agent_call(),
        mcp_call(),
    ]
}

// ---------------------------------------------------------------- turns

fn opening_marker_turn() -> Turn {
    Turn::Assistant {
        id: "t0".into(),
        timestamp: None,
        blocks: vec![Block::Marker {
            kind: MarkerKind::SessionStarted,
            text: "Today · 09:14 · session started · Claude Code v2.1.174".into(),
        }],
        meta: TurnMeta::default(),
    }
}

fn user_turn() -> Turn {
    Turn::User {
        id: "t1".into(),
        timestamp: Some(1_786_320_000_000),
        text: "Tighten address validation in @src/checkout and add coverage for CA and GB \
               postcodes. Keep the existing copy."
            .into(),
        attachments: vec![
            Attachment {
                name: "validators.ts".into(),
                kind: AttachmentKind::File,
                size_bytes: Some(6_144),
                meta: Some("180 lines".into()),
                state: UploadState::Ready,
            },
            Attachment {
                name: "checkout-form.png".into(),
                kind: AttachmentKind::Image,
                size_bytes: Some(184_320),
                meta: None,
                state: UploadState::Ready,
            },
        ],
        mentions: vec![Mention { kind: MentionKind::File, label: "@src/checkout".into() }],
    }
}

fn work_turn() -> Turn {
    Turn::Assistant {
        id: "t2".into(),
        timestamp: Some(1_786_320_045_000),
        blocks: vec![
            Block::text(
                "I'm checking the existing form flow, then I'll patch the validator and run \
                 the focused tests.",
            ),
            thinking(),
            activity(),
            vitest_call(),
            dev_server_call(),
            read_call(),
            edit_call(),
            search_call(),
            web_call(),
            lint_call(),
            browser_call(),
            sub_agent_call(),
            mcp_call(),
            Block::text(
                "Found the country-specific branch in `validateAddress`. Two things stand out: \
                 an empty country currently returns `true`, which lets a blank address through, \
                 and Canadian postal codes fall into the US ZIP branch. I'll make the \
                 ZIP/postal path explicit and keep the checkout copy unchanged, then run the \
                 two focused test files.",
            ),
        ],
        meta: TurnMeta {
            model: "opus 4.6".into(),
            duration_ms: 3_100,
            tokens_in: 1_900,
            tokens_out: 500,
            reasoning_tokens: 0,
            cost_usd: 0.04,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cached_tokens: 0,
        },
    }
}

fn decision_turn() -> Turn {
    Turn::Assistant {
        id: "t3".into(),
        timestamp: None,
        blocks: vec![question(), plan(), todo()],
        meta: TurnMeta {
            model: "opus 4.6".into(),
            duration_ms: 1_400,
            tokens_in: 2_400,
            tokens_out: 320,
            reasoning_tokens: 0,
            cost_usd: 0.02,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cached_tokens: 0,
        },
    }
}

fn approval_turn() -> Turn {
    Turn::Assistant {
        id: "t4".into(),
        timestamp: None,
        blocks: vec![
            Block::text(
                "The pg native module needs a system library before the e2e run. I need \
                 permission for one install.",
            ),
            approvals().remove(0),
        ],
        meta: TurnMeta {
            model: "opus 4.6".into(),
            duration_ms: 900,
            tokens_in: 2_600,
            tokens_out: 120,
            reasoning_tokens: 0,
            cost_usd: 0.01,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cached_tokens: 0,
        },
    }
}

fn marker_turn() -> Turn {
    Turn::Assistant {
        id: "t5".into(),
        timestamp: None,
        blocks: vec![
            Block::Marker {
                kind: MarkerKind::ContextCompacted,
                text: "Context compacted · 41k → 12k tokens · view summary".into(),
            },
            Block::Marker {
                kind: MarkerKind::HandOff { from: Provider::Claude, to: Provider::Codex },
                text: "Claude Code handed off with full context to Codex".into(),
            },
            Block::Marker {
                kind: MarkerKind::PermissionModeChanged { mode: PermissionMode::AllowAll },
                text: "Permission mode changed to Bypass for this session".into(),
            },
        ],
        meta: TurnMeta::default(),
    }
}

fn wrap_up_turn() -> Turn {
    Turn::Assistant {
        id: "t6".into(),
        timestamp: None,
        blocks: vec![
            Block::Summary {
                title: "address validation tightened".into(),
                files: vec![
                    FileChange {
                        path: "src/checkout/validators.ts".into(),
                        change: ChangeKind::Modified,
                        added: 8,
                        removed: 3,
                    },
                    FileChange {
                        path: "src/checkout/validators.test.ts".into(),
                        change: ChangeKind::Added,
                        added: 41,
                        removed: 0,
                    },
                    FileChange {
                        path: "src/checkout/AddressForm.tsx".into(),
                        change: ChangeKind::Modified,
                        added: 2,
                        removed: 1,
                    },
                ],
                checks: vec![
                    Check { label: "27 tests pass".into(), passed: true },
                    Check { label: "lint clean".into(), passed: true },
                    Check { label: "typecheck".into(), passed: true },
                ],
                duration_ms: 252_000,
                cost_usd: 0.31,
            },
            Block::Error {
                title: "Anthropic API rate limited".into(),
                detail: "429 after 3 attempts · retrying automatically in 20 s".into(),
                retryable: true,
            },
        ],
        meta: TurnMeta {
            model: "opus 4.6".into(),
            duration_ms: 252_000,
            tokens_in: 18_400,
            tokens_out: 3_100,
            reasoning_tokens: 0,
            cost_usd: 0.31,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cached_tokens: 0,
        },
    }
}

// ---------------------------------------------------------------- blocks

fn thinking() -> Block {
    Block::Thinking {
        text: "The validator returns early when country is empty, so the form can submit a \
               blank address. That's probably the bug behind the flaky checkout test.\n\n\
               Canadian postal codes are A1A 1A1; the current regex is ZIP-only. Safer to \
               branch per country and keep the regexes small.\n\n\
               I should not touch the copy in AddressForm, the user asked to keep it. Tests: \
               validators.test.ts covers the US path, I'll add CA and GB cases and an \
               empty-country case…"
            .into(),
        elapsed_ms: 14_000,
        summary: Some("branch per country, keep copy, add CA/GB/empty cases".into()),
        state: ThinkingState::Done,
    }
}

fn activity() -> Block {
    Block::Activity {
        steps: vec![
            Step {
                verb: "Searched".into(),
                target: "validateAddress in src/checkout".into(),
                state: StepState::Done,
                result: Some("2 hits".into()),
            },
            Step {
                verb: "Read".into(),
                target: "src/checkout/validators.ts".into(),
                state: StepState::Done,
                result: Some("180 lines".into()),
            },
            Step {
                verb: "Edited".into(),
                target: "src/checkout/validators.ts".into(),
                state: StepState::Done,
                result: Some("+8 −3".into()),
            },
            Step {
                verb: "Running".into(),
                target: "pnpm vitest run src/checkout".into(),
                state: StepState::Running,
                result: Some("12 s".into()),
            },
            Step {
                verb: "Lint".into(),
                target: "eslint on touched files".into(),
                state: StepState::Pending,
                result: Some("queued".into()),
            },
        ],
        summary: "Running tests".into(),
        elapsed_ms: 38_000,
        state: ActivityState::Working,
    }
}

fn question() -> Block {
    Block::Question {
        id: "q-postcodes".into(),
        header: "Postcodes".into(),
        prompt: "Which postcode formats should validate?".into(),
        subtitle: "Claude needs this before editing the validator · pick all that apply".into(),
        options: vec![
            QuestionOption {
                label: "US ZIP and ZIP+4".into(),
                description: "12345 or 12345-6789".into(),
                key: "1".into(),
                preview: None,
            },
            QuestionOption {
                label: "Canadian postal".into(),
                description: "A1A 1A1, space optional".into(),
                key: "2".into(),
                preview: None,
            },
            QuestionOption {
                label: "UK postcode".into(),
                description: "Outward + inward, e.g. SW1A 1AA".into(),
                key: "3".into(),
                preview: None,
            },
        ],
        multi: true,
        allow_other: true,
        answer: Some(Answer { selected: vec![0, 1], other: None }),
        timeout_ms: None,
    }
}

fn plan() -> Block {
    Block::Plan {
        id: "plan-1".into(),
        items: vec![
            "Branch validateAddress per country; keep regexes small.".into(),
            "Return a structured { ok, field } instead of boolean.".into(),
            "Add CA / GB / empty-country cases to the test file.".into(),
            "Run focused tests, then lint touched files.".into(),
        ],
        sections: Vec::new(),
        state: PlanState::Proposed,
    }
}

fn todo() -> Block {
    Block::Todo {
        items: vec![
            TodoItem {
                label: "Read validators and the form".into(),
                state: TodoState::Done,
                elapsed_ms: Some(12_000),
            },
            TodoItem {
                label: "Branch validation per country".into(),
                state: TodoState::Done,
                elapsed_ms: Some(48_000),
            },
            TodoItem {
                label: "Return structured result".into(),
                state: TodoState::Done,
                elapsed_ms: Some(20_000),
            },
            TodoItem {
                label: "Add CA, GB and empty-country tests".into(),
                state: TodoState::Running,
                elapsed_ms: Some(64_000),
            },
            TodoItem {
                label: "Run focused tests".into(),
                state: TodoState::Pending,
                elapsed_ms: None,
            },
            TodoItem {
                label: "Lint touched files".into(),
                state: TodoState::Pending,
                elapsed_ms: None,
            },
            TodoItem {
                label: "Summarise the change".into(),
                state: TodoState::Pending,
                elapsed_ms: None,
            },
        ],
    }
}

// ---------------------------------------------------------------- tool calls

fn vitest_call() -> Block {
    Block::ToolCall {
        id: "tc-vitest".into(),
        kind: ToolKind::Shell,
        verb: "Ran".into(),
        target: "pnpm vitest run src/checkout".into(),
        status: ToolStatus::Success,
        duration_ms: Some(12_400),
        body: ToolBody::Shell {
            output_lines: vec![
                "\u{1b}[2m$\u{1b}[0m pnpm vitest run src/checkout".into(),
                "\u{1b}[32m✓\u{1b}[0m validators.test.ts \u{1b}[2m(18)\u{1b}[0m".into(),
                "\u{1b}[32m✓\u{1b}[0m AddressForm.test.tsx \u{1b}[2m(9)\u{1b}[0m".into(),
                "".into(),
                "Test Files  \u{1b}[32m2 passed\u{1b}[0m (2)".into(),
                "     Tests  \u{1b}[32m27 passed\u{1b}[0m (27)".into(),
                "".into(),
                "  Start at  09:41:12".into(),
                "  Duration  12.4 s".into(),
                "".into(),
                "  ✓ validators › US ZIP".into(),
                "  ✓ validators › US ZIP+4".into(),
                "  ✓ validators › CA postal".into(),
                "  ✓ validators › GB postcode".into(),
                "  ✓ validators › empty country".into(),
                "  ✓ AddressForm › renders".into(),
                "  ✓ AddressForm › submits".into(),
                "  ✓ AddressForm › keeps copy".into(),
            ],
            exit_code: Some(0),
            live: false,
        },
        diff_stat: None,
    }
}

fn dev_server_call() -> Block {
    Block::ToolCall {
        id: "tc-dev".into(),
        kind: ToolKind::Shell,
        verb: "Running".into(),
        target: "pnpm dev".into(),
        status: ToolStatus::Running,
        duration_ms: Some(72_000),
        body: ToolBody::Shell {
            output_lines: vec![
                "\u{1b}[2m▲\u{1b}[0m Next.js 16.2.1 (Turbopack)".into(),
                "- Local: http://localhost:3000".into(),
                "\u{1b}[32m✓\u{1b}[0m Ready in 731ms".into(),
                "○ Compiling /checkout ...".into(),
                "\u{1b}[32m✓\u{1b}[0m Compiled /checkout in 812ms".into(),
                "\u{1b}[33m⚠\u{1b}[0m Unsupported engine: wanted node 24".into(),
            ],
            exit_code: None,
            live: true,
        },
        diff_stat: None,
    }
}

fn read_call() -> Block {
    Block::ToolCall {
        id: "tc-read".into(),
        kind: ToolKind::Read,
        verb: "Read".into(),
        target: "src/checkout/validators.ts".into(),
        status: ToolStatus::Success,
        duration_ms: Some(120),
        body: ToolBody::Read { lines: 180 },
        diff_stat: None,
    }
}

fn edit_call() -> Block {
    Block::ToolCall {
        id: "tc-edit".into(),
        kind: ToolKind::Edit,
        verb: "Edited".into(),
        target: "src/checkout/validators.ts".into(),
        status: ToolStatus::Success,
        duration_ms: Some(340),
        body: ToolBody::Edit { diff: validators_diff() },
        // The provider's whole-patch summary, so the gallery draws the chips
        // without fetching the patch body.
        diff_stat: Some(DiffStat { added: 8, removed: 3, files: 1 }),
    }
}

fn search_call() -> Block {
    Block::ToolCall {
        id: "tc-search".into(),
        kind: ToolKind::Search,
        verb: "Searched".into(),
        target: "validateAddress".into(),
        status: ToolStatus::Success,
        duration_ms: Some(210),
        body: ToolBody::Search {
            hits: vec![
                SearchHit {
                    path: "src/checkout/AddressForm.tsx".into(),
                    line: 38,
                    snippet: "validateAddress(values)".into(),
                },
                SearchHit {
                    path: "src/checkout/validators.ts".into(),
                    line: 12,
                    snippet: "export function validateAddress".into(),
                },
            ],
        },
        diff_stat: None,
    }
}

fn web_call() -> Block {
    Block::ToolCall {
        id: "tc-web".into(),
        kind: ToolKind::Web,
        verb: "Searched web".into(),
        target: "canadian postal code regex".into(),
        status: ToolStatus::Success,
        duration_ms: Some(1_800),
        body: ToolBody::Web {
            results: vec![
                WebResult {
                    title: "Postal codes in Canada".into(),
                    url: "https://en.wikipedia.org/wiki/Postal_codes_in_Canada".into(),
                    domain: "wikipedia.org".into(),
                },
                WebResult {
                    title: "Validating Canadian postal codes".into(),
                    url: "https://www.canada.ca/en/postal-codes.html".into(),
                    domain: "canada.ca".into(),
                },
            ],
            hidden: 3,
        },
        diff_stat: None,
    }
}

/// Consecutive tool calls folded into one card, for the gallery's tool-group
/// entry: a finished group of five and a running group of two.
pub fn tool_groups() -> Vec<Block> {
    vec![tool_group(), running_tool_group()]
}

/// Five finished calls under one summary line: the collapsed preview shows
/// the first two rows plus `+3 more`.
fn tool_group() -> Block {
    let calls = ["tc-search", "tc-read", "tc-web", "tc-lint", "tc-browser"]
        .into_iter()
        .map(|id| tool_calls().into_iter().find(|b| matches!(b, Block::ToolCall { id: got, .. } if got == id)).expect("sample call id").as_tool_call().expect("tool call"))
        .collect();
    Block::ToolGroup { calls, summary: "Checked the form flow".into(), state: ActivityState::Done }
}

/// Two shell calls with one still running, so the header spins.
fn running_tool_group() -> Block {
    let calls = ["tc-read", "tc-dev"]
        .into_iter()
        .map(|id| tool_calls().into_iter().find(|b| matches!(b, Block::ToolCall { id: got, .. } if got == id)).expect("sample call id").as_tool_call().expect("tool call"))
        .collect();
    Block::ToolGroup { calls, summary: "Running dev server".into(), state: ActivityState::Working }
}

/// One [`ToolCall`] struct for the gallery: the read call as data.
pub fn tool_call_sample() -> ToolCall {
    read_call().as_tool_call().expect("read call")
}

fn lint_call() -> Block {
    Block::ToolCall {
        id: "tc-lint".into(),
        kind: ToolKind::Shell,
        verb: "Ran".into(),
        target: "pnpm lint".into(),
        status: ToolStatus::Error,
        duration_ms: Some(3_200),
        body: ToolBody::Shell {
            output_lines: vec![
                "\u{1b}[31m✖\u{1b}[0m src/checkout/validators.ts".into(),
                "  46:5  \u{1b}[31merror\u{1b}[0m  'validateCanadianPostal' is not defined  no-undef".into(),
            ],
            exit_code: Some(1),
            live: false,
        },
        diff_stat: None,
    }
}

fn browser_call() -> Block {
    Block::ToolCall {
        id: "tc-browser".into(),
        kind: ToolKind::Browser,
        verb: "Browser".into(),
        target: "localhost:3000/signup".into(),
        status: ToolStatus::Success,
        duration_ms: Some(2_400),
        body: ToolBody::Browser {
            action: "3 actions".into(),
            screenshot: Some("design/reference/screens/harness-Main.png".into()),
            caption: Some("click \u{201c}Try free\u{201d}".into()),
        },
        diff_stat: None,
    }
}

fn sub_agent_call() -> Block {
    Block::ToolCall {
        id: "tc-subagent".into(),
        kind: ToolKind::SubAgent,
        verb: "Delegated".into(),
        target: "postcode-research".into(),
        status: ToolStatus::Success,
        duration_ms: Some(21_000),
        body: ToolBody::SubAgent {
            turns: vec![
                Turn::User {
                    id: "sa-1".into(),
        timestamp: None,
                    text: "Find the canonical CA and GB postcode formats.".into(),
                    attachments: Vec::new(),
                    mentions: Vec::new(),
                },
                Turn::Assistant {
                    id: "sa-2".into(),
        timestamp: None,
                    blocks: vec![Block::text(
                        "CA is A1A 1A1 with an optional space; GB is an outward and an inward \
                         part, e.g. SW1A 1AA.",
                    )],
                    meta: TurnMeta {
                        model: "haiku 4.6".into(),
                        duration_ms: 21_000,
                        tokens_in: 900,
                        tokens_out: 160,
                        reasoning_tokens: 0,
                        cost_usd: 0.002,
                        cache_read_tokens: None,
                        cache_write_tokens: None,
                        cached_tokens: 0,
                    },
                },
            ],
        },
        diff_stat: None,
    }
}

fn mcp_call() -> Block {
    Block::ToolCall {
        id: "tc-mcp".into(),
        kind: ToolKind::Mcp { server: "linear".into(), tool: "get_issue".into() },
        verb: "Called".into(),
        target: "linear · get_issue".into(),
        status: ToolStatus::Success,
        duration_ms: Some(640),
        body: ToolBody::Mcp {
            params: vec![("id".into(), "ACME-2491".into())],
            result_json: "{\"id\":\"ACME-2491\",\"title\":\"Blank address passes checkout\",\
                          \"state\":\"In Progress\"}"
                .into(),
        },
        diff_stat: None,
    }
}

fn validators_diff() -> Diff {
    Diff {
        path: "src/checkout/validators.ts".into(),
        hunks: vec![Hunk {
            header: "@@ -44,3 +44,5 @@ export function validateAddress".into(),
            lines: vec![
                DiffLine {
                    kind: DiffKind::Context,
                    old_no: Some(44),
                    new_no: Some(44),
                    text: "export function validateAddress(values) {".into(),
                },
                DiffLine {
                    kind: DiffKind::Del,
                    old_no: Some(45),
                    new_no: None,
                    text: "  if (!values.country) return true;".into(),
                },
                DiffLine {
                    kind: DiffKind::Add,
                    old_no: None,
                    new_no: Some(45),
                    text: "  if (!values.country) return { ok: false, field: 'country' };".into(),
                },
                DiffLine {
                    kind: DiffKind::Add,
                    old_no: None,
                    new_no: Some(46),
                    text: "  if (values.country === 'CA') return validateCanadianPostal(values);"
                        .into(),
                },
            ],
        },
        Hunk {
            header: "@@ -61,2 +63,4 @@ function validateCanadianPostal".into(),
            lines: vec![
                DiffLine { kind: DiffKind::Add, old_no: None, new_no: Some(63), text: "  const POSTAL = /^[A-Z]\\d[A-Z] ?\\d[A-Z]\\d$/i;".into() },
                DiffLine { kind: DiffKind::Add, old_no: None, new_no: Some(64), text: "  return { ok: POSTAL.test(values.postal), field: 'postal' };".into() },
            ],
        },
        Hunk {
            header: "@@ -88,3 +92,5 @@ export function validateGb".into(),
            lines: vec![
                DiffLine { kind: DiffKind::Del, old_no: Some(89), new_no: None, text: "  return true;".into() },
                DiffLine { kind: DiffKind::Add, old_no: None, new_no: Some(93), text: "  return { ok: GB.test(values.postal), field: 'postal' };".into() },
            ],
        }],
        added: 8,
        removed: 3,
    }
}

// ---------------------------------------------------------------- orchestration

fn run_stages() -> Vec<RunStage> {
    vec![
        RunStage {
            name: "Plan · Opus".into(),
            provider: Some(Provider::Claude),
            model: "opus 4.6".into(),
            writes: false,
        },
        RunStage {
            name: "Implement · Muse".into(),
            provider: Some(Provider::Muse),
            model: "muse-spark-1.3".into(),
            writes: true,
        },
        RunStage {
            name: "Review · Codex".into(),
            provider: Some(Provider::Codex),
            model: "gpt-5.6".into(),
            writes: false,
        },
        RunStage {
            name: "Fix · Muse".into(),
            provider: Some(Provider::Muse),
            model: "muse-spark-1.3".into(),
            writes: true,
        },
        RunStage {
            name: "Verify · browser".into(),
            // Baaz runs the verify stage itself: no provider.
            provider: None,
            model: "baaz".into(),
            writes: false,
        },
    ]
}

fn run_criteria() -> Vec<Criterion> {
    vec![
        Criterion {
            text: "form validates email and password before submit".into(),
            method: CheckMethod::Test,
        },
        Criterion {
            text: "errors shown inline; email kept after a wrong password".into(),
            method: CheckMethod::Browser,
        },
        Criterion {
            text: "submit calls /api/login and lands on projects".into(),
            method: CheckMethod::Browser,
        },
        Criterion {
            text: "matches the app's existing styles".into(),
            method: CheckMethod::Review,
        },
        Criterion { text: "tests added".into(), method: CheckMethod::Test },
    ]
}

fn run_assumptions() -> Vec<String> {
    vec![
        "from main at 3db2220".into(),
        "dev server is npm run dev (from package.json) on this worktree's own port".into(),
        "2 workers at a time (each build of this project peaks near 3 GB here)".into(),
        "2 held-out checks the implementers won't see".into(),
    ]
}

fn run_plan_inner(started_at: Option<String>) -> RunPlan {
    RunPlan {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        intro: "I'll run this in five stages with the models you named. Opus, Muse and Codex are all signed in.".into(),
        stages: run_stages(),
        criteria: run_criteria(),
        assumptions: run_assumptions(),
        started_at,
    }
}

/// The login-screen run plan as proposed: no start time, so the footer reads
/// `Reply "go" to start, or tell me what to change.`
pub fn run_plan_proposed() -> Block {
    Block::RunPlan(run_plan_inner(None))
}

/// The login-screen run plan once accepted and started.
pub fn run_plan_started() -> Block {
    Block::RunPlan(run_plan_inner(Some("09:14".into())))
}

fn progress_lines() -> Vec<TaskLine> {
    vec![
        TaskLine {
            task_id: "t-impl-form".into(),
            stage: "Implement".into(),
            title: "Implement · form and validation".into(),
            provider: Provider::Muse,
            model: Some("muse-spark-1.3".into()),
            state: TaskState::Verifying,
            activity: "running the checks".into(),
            elapsed: "6m".into(),
            rungs: vec![
                Rung { name: "scope".into(), state: RungState::Pass, detail: None },
                Rung { name: "build".into(), state: RungState::Pass, detail: None },
                Rung { name: "tests".into(), state: RungState::Running, detail: None },
            ],
        },
        TaskLine {
            task_id: "t-auth-call".into(),
            stage: "Implement".into(),
            title: "Implement · auth call".into(),
            provider: Provider::Muse,
            model: Some("muse-spark-1.3".into()),
            state: TaskState::Working,
            activity: "editing src/auth/login.ts".into(),
            elapsed: "4m".into(),
            rungs: Vec::new(),
        },
    ]
}

fn run_progress_inner() -> RunProgress {
    RunProgress {
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
        lines: progress_lines(),
    }
}

/// The running login-screen run: one task working, one verifying.
pub fn run_progress_running() -> Block {
    Block::RunProgress(run_progress_inner())
}

/// The run waiting on the person's answer to the Remember-me question.
pub fn run_progress_needs_you() -> Block {
    let mut progress = run_progress_inner();
    progress.needs_you = 1;
    progress.lines[1].state = TaskState::NeedsYou;
    progress.lines[1].activity = "waiting on your answer".into();
    Block::RunProgress(progress)
}

/// The run paused by the person: nothing moves.
pub fn run_progress_paused() -> Block {
    let mut progress = run_progress_inner();
    progress.paused = true;
    for line in &mut progress.lines {
        line.state = TaskState::Paused;
    }
    Block::RunProgress(progress)
}

/// The run waiting on quota before it can continue.
pub fn run_progress_quota_wait() -> Block {
    let mut progress = run_progress_inner();
    progress.waiting_reason = Some("Codex quota resets at 15:00".into());
    Block::RunProgress(progress)
}

/// The run reconciling after a worker disagreed about the state.
pub fn run_progress_reconciling() -> Block {
    let mut progress = run_progress_inner();
    progress.lines[0].state = TaskState::Reconciling;
    progress.lines[0].activity = "re-deriving state after a worker report".into();
    Block::RunProgress(progress)
}

fn run_question_inner(answer: Option<String>, answered_at: Option<String>) -> RunQuestion {
    RunQuestion {
        question_id: "q-remember-me".into(),
        run_id: "run-login-screen".into(),
        source_label: "QUESTION FROM THE RUN · IMPLEMENT · AUTH CALL".into(),
        text: "The design shows a \"Remember me\" checkbox, but it isn't in the criteria. Add it, or leave it out?".into(),
        hint: "Muse worker paused · you can also just type the answer".into(),
        options: vec![
            crate::run::QuestionOption {
                id: "add".into(),
                label: "Add it".into(),
                recommended: false,
            },
            crate::run::QuestionOption {
                id: "leave-out".into(),
                label: "Leave it out".into(),
                recommended: true,
            },
        ],
        answer,
        answered_at,
    }
}

/// The Remember-me question while open.
pub fn run_question_open() -> Block {
    Block::RunQuestion(run_question_inner(None, None))
}

/// The Remember-me question collapsed after answering.
pub fn run_question_answered() -> Block {
    Block::RunQuestion(run_question_inner(Some("Leave it out".into()), Some("14:31".into())))
}

fn run_summary_inner(outcome: RunOutcome, merged: Option<String>) -> RunSummary {
    let evidence = |label: &str, evidence_kind: EvidenceKind| Evidence {
        label: label.into(),
        evidence_kind,
    };
    RunSummary {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        outcome,
        elapsed: "31m".into(),
        workers: 4,
        criteria: vec![
            CriterionResult {
                text: "form validates email and password before submit".into(),
                met: true,
                evidence: vec![evidence(
                    "npm test exited 0 · 12 passed",
                    EvidenceKind::Test,
                )],
            },
            CriterionResult {
                text: "errors shown inline; email kept after a wrong password".into(),
                met: true,
                evidence: vec![evidence("screenshot", EvidenceKind::Screenshot)],
            },
            CriterionResult {
                text: "submit calls /api/login and lands on projects".into(),
                met: true,
                evidence: vec![evidence(
                    "held-out 2/2 · screenshot",
                    EvidenceKind::HeldOut,
                )],
            },
            CriterionResult {
                text: "matches the app's existing styles".into(),
                met: true,
                evidence: vec![evidence("review: no findings", EvidenceKind::Review)],
            },
            CriterionResult {
                text: "tests added".into(),
                met: true,
                evidence: vec![evidence("+4 tests", EvidenceKind::Count)],
            },
        ],
        additions: 412,
        deletions: 36,
        files: 6,
        review_rounds: 2,
        extra: Some("mutation check passed on both fixes".into()),
        not_proven: vec!["the OAuth provider flow (no test account in this project)".into()],
        next_hint: "Branch baaz/login-screen. Say \"merge\" to merge into main, or \"open a PR\".".into(),
        merged,
    }
}

/// The finished login-screen summary.
pub fn run_summary_finished() -> Block {
    Block::RunSummary(run_summary_inner(RunOutcome::Finished, None))
}

/// The login-screen summary for a run the person stopped early.
pub fn run_summary_stopped() -> Block {
    Block::RunSummary(run_summary_inner(RunOutcome::Stopped, None))
}

/// The login-screen summary once merged.
pub fn run_summary_merged() -> Block {
    Block::RunSummary(run_summary_inner(RunOutcome::Finished, Some("a91f04c".into())))
}

/// The remembered-preference note: reviews use Codex.
pub fn run_note_preference() -> Block {
    Block::RunNote(RunNote {
        text: "Remembered: reviews use Codex. Say \"forget that\" to undo.".into(),
        note_kind: NoteKind::Preference,
    })
}

#[allow(clippy::too_many_arguments)]
fn goal_block(
    objective: &str,
    status: &str,
    percent_complete: Option<f32>,
    provider: Provider,
    tokens_used: u64,
    token_budget: u64,
    elapsed: &str,
    turn: Option<u32>,
    last_reason: Option<&str>,
    reopened: bool,
) -> Block {
    Block::Goal {
        objective: objective.into(),
        status: status.into(),
        percent_complete,
        current_work: None,
        next_work: None,
        provider: Some(provider),
        tokens_used: Some(tokens_used),
        token_budget: Some(token_budget),
        elapsed: Some(elapsed.into()),
        turn,
        last_reason: last_reason.map(Into::into),
        gated_by_baaz: false,
        reopened,
    }
}

/// The Codex calc goal while active: 6713 of 80000 tokens, 7 s in.
pub fn goal_codex_active() -> Block {
    goal_block(
        "Make python3 -m unittest -q exit 0 by fixing calc.py",
        "active",
        Some(8.4),
        Provider::Codex,
        6713,
        80_000,
        "7 s",
        None,
        None,
        false,
    )
}

/// The Codex calc goal complete: 9578 of 80000 tokens, 19 s in.
pub fn goal_codex_complete() -> Block {
    goal_block(
        "Make python3 -m unittest -q exit 0 by fixing calc.py",
        "complete",
        Some(12.0),
        Provider::Codex,
        9578,
        80_000,
        "19 s",
        None,
        None,
        false,
    )
}

/// The Claude calc goal on turn 3, with two tests still failing.
pub fn goal_claude_active() -> Block {
    goal_block(
        "Make python3 -m unittest -q exit 0 by fixing calc.py",
        "active",
        Some(64.0),
        Provider::Claude,
        12_400,
        80_000,
        "4m",
        Some(3),
        Some("2 tests still failing in test_calc.py"),
        false,
    )
}

/// The Muse login-screen goal just started: 0 % active.
pub fn goal_muse_active() -> Block {
    goal_block(
        "Implement the login screen",
        "active",
        Some(0.0),
        Provider::Muse,
        0,
        80_000,
        "0 s",
        None,
        None,
        false,
    )
}

/// A goal the provider called done while tests still fail.
pub fn goal_reopened() -> Block {
    goal_block(
        "Make python3 -m unittest -q exit 0 by fixing calc.py",
        "active",
        Some(86.0),
        Provider::Claude,
        12_400,
        80_000,
        "4m",
        Some(3),
        Some("Provider said done; tests still fail: 2 of 14"),
        true,
    )
}

/// A Muse-owned native workflow with a failed child that can retry or skip.
pub fn workflow_muse_with_failed_child() -> Block {
    Block::NativeWorkflow(NativeWorkflow {
        provider: Provider::Muse,
        name: "Login screen fixes".into(),
        children: vec![
            WorkflowChild {
                child_id: "w-1".into(),
                attempt: 1,
                label: "Implement · form and validation".into(),
                phase: None,
                status: TaskState::Failed,
            },
            WorkflowChild {
                child_id: "w-2".into(),
                attempt: 1,
                label: "Implement · auth call".into(),
                phase: None,
                status: TaskState::Working,
            },
        ],
        can_retry: true,
        can_skip: true,
        provisional: false,
    })
}

/// A Claude-owned native workflow whose children report provider phases.
pub fn workflow_claude_phases() -> Block {
    Block::NativeWorkflow(NativeWorkflow {
        provider: Provider::Claude,
        name: "Checkout validation".into(),
        children: vec![
            WorkflowChild {
                child_id: "w-1".into(),
                attempt: 1,
                label: "Research postcode formats".into(),
                phase: Some("Explore".into()),
                status: TaskState::Accepted,
            },
            WorkflowChild {
                child_id: "w-2".into(),
                attempt: 2,
                label: "Branch the validator".into(),
                phase: Some("Implement".into()),
                status: TaskState::Working,
            },
        ],
        can_retry: true,
        can_skip: false,
        provisional: true,
    })
}

fn peer_outgoing_inner(state: PeerState, reason: Option<String>, reply: Option<PeerReply>) -> PeerMessage {
    PeerMessage {
        message_id: "pm-1".into(),
        direction: PeerDirection::Outgoing,
        peer_title: "Browser audit".into(),
        peer_provider: Provider::Codex,
        peer_project: Some("agentic-ui".into()),
        text: "Do you touch registry.rs or cards/mod.rs in agentic-ui? I'm about to start orchestration mocks.".into(),
        state,
        reason,
        at: "14:20".into(),
        reply,
    }
}

/// An outgoing peer message still waiting for a reply.
pub fn peer_outgoing_waiting() -> Block {
    Block::PeerMessage(peer_outgoing_inner(PeerState::Sent, None, None))
}

/// An outgoing peer message the peer replied to.
pub fn peer_outgoing_replied() -> Block {
    Block::PeerMessage(peer_outgoing_inner(
        PeerState::Replied,
        None,
        Some(PeerReply {
            text: "No clash: I only append a Browse group at the end of registry.rs and add browse_* modules to cards/mod.rs. Keep yours at the end too.".into(),
            at: "14:26".into(),
        }),
    ))
}

/// An outgoing peer message queued for a peer that is not listening yet.
pub fn peer_outgoing_queued() -> Block {
    Block::PeerMessage(peer_outgoing_inner(PeerState::Queued, None, None))
}

/// An outgoing peer message that can never arrive: the session is closed.
pub fn peer_outgoing_unreachable() -> Block {
    Block::PeerMessage(peer_outgoing_inner(
        PeerState::Unreachable,
        Some("the session is closed".into()),
        None,
    ))
}

/// An incoming peer message from the Claude orchestration session.
pub fn peer_incoming() -> Block {
    Block::PeerMessage(PeerMessage {
        message_id: "pm-2".into(),
        direction: PeerDirection::Incoming,
        peer_title: "Orchestration in Baaz".into(),
        peer_provider: Provider::Claude,
        peer_project: None,
        text: "The run plan is ready for your review.".into(),
        state: PeerState::Delivered,
        reason: None,
        at: "14:18".into(),
        reply: None,
    })
}

/// The header of the Muse worker view for the auth-call task.
pub fn worker_header_sample() -> WorkerHeader {
    WorkerHeader {
        run_title: "Login screen".into(),
        stage: "Implement · auth call".into(),
        attempt: 1,
        provider: Provider::Muse,
        model: "muse-spark-1.3".into(),
        branch: "baaz/login-screen".into(),
    }
}

/// The run tree view for the login-screen run.
pub fn run_view_sample() -> RunViewModel {
    RunViewModel {
        run_id: "run-login-screen".into(),
        title: "Login screen".into(),
        tree: vec![
            RunViewNode {
                task_id: "t-impl-form".into(),
                depth: 0,
                label: "Implement · form and validation".into(),
                provider: Provider::Muse,
                state: TaskState::Verifying,
                note: None,
            },
            RunViewNode {
                task_id: "t-auth-call".into(),
                depth: 0,
                label: "Implement · auth call".into(),
                provider: Provider::Muse,
                state: TaskState::Working,
                note: Some("editing src/auth/login.ts".into()),
            },
        ],
        evidence: vec![EvidenceRow {
            task_id: "t-impl-form".into(),
            text: "npm test exited 0 · 12 passed".into(),
            evidence_kind: EvidenceKind::Test,
        }],
    }
}
