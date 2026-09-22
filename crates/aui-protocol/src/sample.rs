//! A realistic sample session, used by `aui-gallery` and the parity screenshots.
//!
//! The texts, commands, file names and counts here are copied verbatim from the
//! design source of truth — `design/src/cards/transcript/*.html` and
//! `design/reference/screens/harness-Main.png` — so a gallery entry rendered from
//! [`session()`] can be diffed pixel for pixel against the reference renders.
//! Change these strings only when the design cards change.

use crate::block::{
    ActivityState, Answer, ApprovalBadges, ApprovalChoice, ApprovalScope, ApprovalStage,
    ApprovalState, Block, ChangeKind, Check, FileChange, MarkerKind, PlanSection, PlanState,
    QuestionOption, QuestionPreview, ResolvedBy, Step, StepState, ThinkingState, TodoItem,
    TodoState,
};
use crate::intent::ApprovalDecision;
use crate::session::{Environment, PermissionMode, Provider, Session};
use crate::tool::{
    Diff, DiffKind, DiffLine, DiffStat, Hunk, SearchHit, ToolBody, ToolCall, ToolKind, ToolStatus,
    WebResult,
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
        },
        Block::Goal {
            objective: "Ship the checkout fix".into(),
            status: "wrapping up".into(),
            percent_complete: Some(120.0),
            current_work: Some("Re-running the focused tests".into()),
            next_work: None,
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
