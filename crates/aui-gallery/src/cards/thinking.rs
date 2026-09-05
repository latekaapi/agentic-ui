//! Card 32 · Thinking block: thinking, done (collapsed to the summary) and
//! re-expanded. Reproduces `design/src/cards/transcript/32-thinking.html` at 760×520.

use aui::protocol::ThinkingState;
use aui::transcript::thinking_block;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.th{margin-bottom:16px}`.
const BLOCK_GAP: f32 = 16.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

const TRACE: &str = "The validator returns early when country is empty, so the form can submit a blank address. That's probably the bug behind the flaky checkout test.\n\n\
Canadian postal codes are A1A 1A1; the current regex is ZIP-only. Safer to branch per country and keep the regexes small.\n\n\
I should not touch the copy in AddressForm, the user asked to keep it. Tests: validators.test.ts covers the US path, I'll add CA and GB cases and an empty-country case…";
const TRACE_SHORT: &str = "The validator returns early when country is empty, so the form can submit a blank address. That's probably the bug behind the flaky checkout test.\n\n\
Canadian postal codes are A1A 1A1; the current regex is ZIP-only. Safer to branch per country and keep the regexes small.\n\n\
I should not touch the copy in AddressForm, the user asked to keep it.";
const SUMMARY: &str = "branch per country, keep copy, add CA/GB/empty cases";

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    // Clicking the finished blocks toggles their expansion.
    let expanded = window.use_keyed_state("card32-expanded", cx, |_, _| [false, true]);
    let current = *expanded.read(cx);
    let toggle = |index: usize| {
        let expanded = expanded.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            expanded.update(cx, |e, cx| {
                e[index] = !e[index];
                cx.notify();
            })
        }
    };
    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(thinking_block("card32-thinking", TRACE, "14 s", ThinkingState::Thinking))
        .child(thinking_block("card32-done", TRACE_SHORT, "14 s", ThinkingState::Done).summary(SUMMARY).expanded(current[0]).on_toggle(toggle(0)))
        .child(thinking_block("card32-open", TRACE_SHORT, "14 s", ThinkingState::Done).summary(SUMMARY).expanded(current[1]).on_toggle(toggle(1)))
        .child(
            div()
                .mt(px(scale::SP_4) - px(BLOCK_GAP))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("While thinking, the label shimmers and the viewport is capped at four lines with a top fade; new text pushes up from the bottom. On completion the block collapses to one line with a model-written summary. Click re-expands the full trace."),
        )
        .into_any_element()
}
