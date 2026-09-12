//! Transcript tool group: consecutive tool calls under one summary — a
//! collapsed preview group and an open group with full cards.

use aui::protocol::{sample, Block};
use aui::transcript::{tool_group, ToolGroupData, ToolGroupIntent};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.tg{gap:16px}`.
const BLOCK_GAP: f32 = 16.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let open = window.use_keyed_state("tg-open", cx, |_, _| [false, true]);
    let current = *open.read(cx);
    // Per-call open states for the open group (sized for the larger group).
    let calls_open = window.use_keyed_state("tg-calls", cx, |_, _| [true; 3]);
    let calls_current = *calls_open.read(cx);

    let mut col = v_flex().w_full().gap(px(BLOCK_GAP));
    for (i, block) in sample::tool_groups().into_iter().enumerate() {
        let Block::ToolGroup { calls, summary, state } = block else { continue };
        let data = ToolGroupData { calls, summary: summary.into(), state };
        let group_open = open.clone();
        let group_calls = calls_open.clone();
        let mut group = tool_group(SharedString::from(format!("tg-{i}")), &data, current[i])
            .on_intent(move |intent, _, cx| match intent {
                ToolGroupIntent::Toggle => {
                    group_open.update(cx, |o, cx| {
                        o[i] = !o[i];
                        cx.notify();
                    });
                }
                ToolGroupIntent::Call { index, intent } => {
                    if intent == aui::transcript::ToolCardIntent::Toggle {
                        group_calls.update(cx, |o, cx| {
                            if let Some(slot) = o.get_mut(index) {
                                *slot = !*slot;
                            }
                            cx.notify();
                        });
                    }
                }
            });
        // The open group honours its per-call states.
        if i == 1 {
            for (index, call_open) in calls_current.into_iter().enumerate() {
                group = group.call_open(index, call_open);
            }
        }
        col = col.child(group);
    }
    col.child(
        div()
            .max_w(px(NOTE_MEASURE))
            .ui(scale::FS_12)
            .text_color(p.ink_3)
            .child("Consecutive tool calls fold into one card: the header carries the status glyph, the summary and a muted call count. Collapsed, the first two calls read as single-line verb-plus-target rows with a +k more row; open, every call is the full tool card with its own body and intents."),
    )
    .into_any_element()
}
