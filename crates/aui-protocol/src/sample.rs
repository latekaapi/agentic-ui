//! A realistic sample session, used by `aui-gallery` and the parity screenshots.
//!
//! The texts, commands, file names and counts here are copied verbatim from the
//! design source of truth — `design/src/cards/transcript/*.html` and
//! `design/reference/screens/harness-Main.png` — so a gallery entry rendered from
//! [`session()`] can be diffed pixel for pixel against the reference renders.
//! Change these strings only when the design cards change.

use crate::block::{
    ActivityState, Answer, ApprovalScope, ApprovalState, Block, ChangeKind, Check, FileChange,
    MarkerKind, PlanState, QuestionOption, Step, StepState, ThinkingState, TodoItem, TodoState,
};
use crate::session::{Environment, PermissionMode, Provider, Session};
use crate::tool::{
    Diff, DiffKind, DiffLine, Hunk, SearchHit, ToolBody, ToolKind, ToolStatus, WebResult,
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
        mode: PermissionMode::Plan,
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
    let base = |state: ApprovalState, command: &str| Block::Approval {
        id: "ap-1".into(),
        tool: "Bash".into(),
        command: command.into(),
        reason: "Claude says: needed to compile the pg native module before the test run.".into(),
        cwd: "~/work/acme/checkout-flow-v2".into(),
        capabilities: vec!["modify files".into(), "network".into()],
        scope: ApprovalScope::ThisWorktree,
        state,
        rule: Some("apt install".into()),
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
            cost_usd: 0.04,
        },
    }
}

fn decision_turn() -> Turn {
    Turn::Assistant {
        id: "t3".into(),
        blocks: vec![question(), plan(), todo()],
        meta: TurnMeta {
            model: "opus 4.6".into(),
            duration_ms: 1_400,
            tokens_in: 2_400,
            tokens_out: 320,
            cost_usd: 0.02,
        },
    }
}

fn approval_turn() -> Turn {
    Turn::Assistant {
        id: "t4".into(),
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
            cost_usd: 0.01,
        },
    }
}

fn marker_turn() -> Turn {
    Turn::Assistant {
        id: "t5".into(),
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
                kind: MarkerKind::PermissionModeChanged { mode: PermissionMode::Bypass },
                text: "Permission mode changed to Bypass for this session".into(),
            },
        ],
        meta: TurnMeta::default(),
    }
}

fn wrap_up_turn() -> Turn {
    Turn::Assistant {
        id: "t6".into(),
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
        prompt: "Which postcode formats should validate?".into(),
        subtitle: "Claude needs this before editing the validator · pick all that apply".into(),
        options: vec![
            QuestionOption {
                label: "US ZIP and ZIP+4".into(),
                description: "12345 or 12345-6789".into(),
                key: "1".into(),
            },
            QuestionOption {
                label: "Canadian postal".into(),
                description: "A1A 1A1, space optional".into(),
                key: "2".into(),
            },
            QuestionOption {
                label: "UK postcode".into(),
                description: "Outward + inward, e.g. SW1A 1AA".into(),
                key: "3".into(),
            },
        ],
        multi: true,
        allow_other: true,
        answer: Some(Answer { selected: vec![0, 1], other: None }),
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
    }
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
                    text: "Find the canonical CA and GB postcode formats.".into(),
                    attachments: Vec::new(),
                    mentions: Vec::new(),
                },
                Turn::Assistant {
                    id: "sa-2".into(),
                    blocks: vec![Block::text(
                        "CA is A1A 1A1 with an optional space; GB is an outward and an inward \
                         part, e.g. SW1A 1AA.",
                    )],
                    meta: TurnMeta {
                        model: "haiku 4.6".into(),
                        duration_ms: 21_000,
                        tokens_in: 900,
                        tokens_out: 160,
                        cost_usd: 0.002,
                    },
                },
            ],
        },
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
