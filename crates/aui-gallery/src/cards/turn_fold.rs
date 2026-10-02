//! Quieter transcript primitives: the settled-turn fold, the live activity
//! row, the collapsed generic card, the shell tail, and the capped
//! Search / MCP bodies — plus the group header without its doubled count.

use aui::protocol::{sample, ActivityState, DiffStat, SearchHit, ToolBody, ToolCall, ToolKind, ToolStatus};
use aui::transcript::{
    generic_item_card, live_activity_row, tool_card, tool_group, turn_fold, GenericItemIntent,
    ToolGroupData, ToolGroupIntent, TurnFoldIntent,
};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.qf{gap:16px}` between sections.
const BLOCK_GAP: f32 = 16.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

fn long_text(lines: usize, prefix: &str) -> String {
    (1..=lines).map(|n| format!("{prefix} line {n}")).collect::<Vec<_>>().join("\n")
}

fn read_call(id: &str, target: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        kind: ToolKind::Read,
        verb: "Read".into(),
        target: target.into(),
        status: ToolStatus::Success,
        duration_ms: Some(120),
        body: ToolBody::Read { lines: 180 },
        diff_stat: None,
    }
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    // Collapsed fold, open fold, live row, collapsed generic, open generic.
    let open = window.use_keyed_state("qf-open", cx, |_, _| [false, true, false, false, true]);
    let current = *open.read(cx);
    // Header toggles report through intents; each section flips its own slot.
    let toggle = |index: usize| {
        let open = open.clone();
        move |cx: &mut App| {
            open.update(cx, |o, cx| {
                o[index] = !o[index];
                cx.notify();
            })
        }
    };
    let toggle_fold = toggle(0);
    let toggle_fold_open = toggle(1);
    let toggle_live = toggle(2);
    let toggle_generic = toggle(3);
    let toggle_generic_open = toggle(4);

    // A live shell with twenty lines: the body follows the tail.
    let live_shell = tool_card(
        "qf-shell",
        "Running",
        "cargo test",
        ToolStatus::Running,
        ToolBody::Shell {
            output_lines: (1..=20).map(|n| format!("test binary {n} … ok")).collect(),
            exit_code: None,
            live: true,
        },
    )
    .duration_ms(Some(14_000))
    .open(true);

    // Forty-five hits cap at forty with a fold row.
    let capped_search = tool_card(
        "qf-search",
        "Searched",
        "validateAddress",
        ToolStatus::Success,
        ToolBody::Search {
            hits: (1..=45)
                .map(|n| SearchHit {
                    path: format!("src/checkout/file{n}.rs"),
                    line: n,
                    snippet: format!("validateAddress call {n}"),
                })
                .collect(),
        },
    )
    .duration_ms(Some(210))
    .open(true);

    // Sixty JSON lines cap at forty with a fold row.
    let capped_mcp = tool_card(
        "qf-mcp",
        "Called",
        "linear · get_issue",
        ToolStatus::Success,
        ToolBody::Mcp {
            params: vec![("id".into(), "ACME-2491".into())],
            result_json: (1..=60).map(|n| format!("{{\"field{n}\": \"value {n}\"}}")).collect::<Vec<_>>().join("\n"),
        },
    )
    .duration_ms(Some(640))
    .open(true);

    // The group whose summary already counts drops its own `N calls`.
    let mut counted = sample::tool_calls();
    counted.truncate(2);
    let counted_calls: Vec<ToolCall> = counted.into_iter().filter_map(|b| b.as_tool_call()).collect();
    let counted_group = ToolGroupData { calls: counted_calls, summary: "Ran 2 commands".into(), state: ActivityState::Done };

    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(
            turn_fold("qf-fold", 134_000, 12, 3, 5)
                .diff_stat(Some(DiffStat { added: 48, removed: 12, files: 3 }))
                .open(current[0])
                .child(tool_card("qf-fold-call", "Read", "src/checkout/validators.ts", ToolStatus::Success, ToolBody::Read { lines: 180 }).open(false))
                .on_intent(move |intent, _, cx| {
                    if intent == TurnFoldIntent::Toggle {
                        toggle_fold(cx);
                    }
                }),
        )
        .child(
            turn_fold("qf-fold-open", 74_000, 6, 1, 2)
                .open(current[1])
                .child(div().ui(scale::FS_12).text_color(p.ink_2).child("Checking the existing form flow, then patching the validator."))
                .child(tool_card("qf-fold-call2", "Read", "src/checkout/validators.ts", ToolStatus::Success, ToolBody::Read { lines: 180 }).open(false))
                .on_intent(move |intent, _, cx| {
                    if intent == TurnFoldIntent::Toggle {
                        toggle_fold_open(cx);
                    }
                }),
        )
        .child(
            live_activity_row("qf-live", "Reading", "crates/baaz/src/app.rs", 4, 14_000)
                .open(current[2])
                .child(tool_card("qf-live-call", "Read", "crates/baaz/src/app.rs", ToolStatus::Running, ToolBody::Read { lines: 40 }).open(false))
                .on_toggle(move |_, _, cx| toggle_live(cx)),
        )
        .child(
            generic_item_card("qf-generic", "Artifact", "completed", long_text(50, "quickstart"))
                .open(current[3])
                .on_intent(move |intent, _, cx| {
                    if intent == GenericItemIntent::Toggle {
                        toggle_generic(cx);
                    }
                }),
        )
        .child(
            generic_item_card("qf-generic-open", "Artifact", "completed", long_text(50, "quickstart"))
                .open(current[4])
                .on_open_full(|_, _| {})
                .on_intent(move |intent, _, cx| {
                    if intent == GenericItemIntent::Toggle {
                        toggle_generic_open(cx);
                    }
                }),
        )
        .child(live_shell)
        .child(capped_search)
        .child(capped_mcp)
        .child(tool_group("qf-group", &counted_group, false).on_intent(|intent, _, _| {
            let _ = intent == ToolGroupIntent::Toggle;
        }))
        .child(
            tool_group(
                "qf-group-plain",
                &ToolGroupData {
                    calls: vec![read_call("qf-r1", "src/checkout/validators.ts"), read_call("qf-r2", "src/checkout/AddressForm.tsx")],
                    summary: "Checked the form flow".into(),
                    state: ActivityState::Done,
                },
                false,
            )
            .on_intent(|intent, _, _| {
                let _ = intent == ToolGroupIntent::Toggle;
            }),
        )
        .child(
            div()
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Settled turns fold to one row with counts and a diff chip; the live run is one row naming the current call with its earlier count. Generic results collapse to three preview lines and cap at forty with an Open control. Shell bodies follow the six-line tail; Search and MCP bodies cap at forty with fold rows. Group headers drop their own count when the summary already states it."),
        )
        .into_any_element()
}
