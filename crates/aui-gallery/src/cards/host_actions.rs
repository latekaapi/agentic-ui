//! `transcript/host-actions` — the L5 host hooks in one place.
//!
//! A [`tool_card`] with a trailing action in its slot, and a fenced code
//! block with its header row and the host action (the "run this" affordance
//! a host would wire). A new module rather than an edit of `tool_cards`, so
//! the existing parity screenshot does not move.

use aui::protocol::{ToolBody, ToolStatus};
use aui::transcript::{code_block, tool_card, CodeBlockHostButton, ToolCardAction};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.cb{margin-bottom:14px}`.
const BLOCK_GAP: f32 = 14.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

/// The runnable script the fenced block carries.
const RUN: &str = "pnpm vitest run src/checkout\n";

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let card = tool_card(
        "host-actions-tool",
        "Ran",
        "pnpm vitest run src/checkout",
        ToolStatus::Success,
        ToolBody::Shell {
            output_lines: vec![
                "\u{1b}[2m$\u{1b}[0m pnpm vitest run src/checkout".into(),
                "\u{1b}[32m✓\u{1b}[0m validators.test.ts \u{1b}[2m(18)\u{1b}[0m".into(),
                "\u{1b}[32m✓\u{1b}[0m AddressForm.test.tsx \u{1b}[2m(9)\u{1b}[0m".into(),
                "".into(),
                "Test Files  \u{1b}[32m2 passed\u{1b}[0m (2)".into(),
                "     Tests  \u{1b}[32m27 passed\u{1b}[0m (27)".into(),
            ],
            exit_code: Some(0),
            live: false,
        },
    )
    .duration_ms(Some(12_400))
    .action(ToolCardAction::new("open-in-terminal", "Open in terminal").icon(IconName::Terminal))
    .on_intent(|intent, _, _| {
        // The gallery has nothing to open; the card exists to show the slot.
        let _ = intent;
    });
    let fence =
        code_block("host-actions-code", "scripts/verify.sh", RUN).language("bash").host_action(0, CodeBlockHostButton::new("Run in terminal", IconName::Play)).on_action(|action, _, _| {
            // The gallery runs nothing; the card exists to show the affordance.
            let _ = action;
        });
    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(card)
        .child(fence)
        .child(
            div()
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("The host hooks L5 added: a tool card carries trailing header actions after the duration, and a fenced block carries one host action after Copy. Pressing either reports an intent with everything the host needs — the tool card its action index, the code block its index, language and code — without the library knowing why."),
        )
        .into_any_element()
}
