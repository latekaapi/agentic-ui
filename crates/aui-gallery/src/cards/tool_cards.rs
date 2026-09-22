//! Card 34 · Tool call cards: the eight variants from the protocol sample in
//! a two-column grid. Reproduces `design/src/cards/transcript/34-tool-cards.html` at 800×760.

use aui::protocol::{sample, Block};
use aui::transcript::{tool_card, ToolCardIntent};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.grid{grid-template-columns:1fr 1fr;gap:12px}`.
const GRID_GAP: f32 = 12.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;
/// The card shows the first eight sample calls (sub-agent and MCP come later).
const SHOWN: usize = 8;

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let open = window.use_keyed_state("card34-open", cx, |_, _| [true; SHOWN]);
    let current = *open.read(cx);
    let calls: Vec<Block> = sample::tool_calls().into_iter().take(SHOWN).collect();
    let mut cards: Vec<AnyElement> = Vec::new();
    for (i, block) in calls.into_iter().enumerate() {
        let Block::ToolCall { id, verb, target, status, duration_ms, body, diff_stat, .. } = block else { continue };
        let open = open.clone();
        cards.push(
            tool_card(SharedString::from(format!("card34-{id}")), verb, target, status, body)
                .duration_ms(duration_ms)
                // The block's own summary: a producer-set summary draws with
                // no further opt-in.
                .diff_stat(diff_stat)
                .open(current[i])
                .on_intent(move |intent, _, cx| {
                    if intent == ToolCardIntent::Toggle {
                        open.update(cx, |o, cx| {
                            o[i] = !o[i];
                            cx.notify();
                        })
                    }
                })
                .into_any_element(),
        );
    }
    // Rows of two, like the CSS grid: cards in a row share the row's height.
    let mut rows = v_flex().w_full().gap(px(GRID_GAP));
    let mut iter = cards.into_iter();
    while let Some(left) = iter.next() {
        let mut row = h_flex().w_full().items_stretch().gap(px(GRID_GAP)).child(div().flex_1().min_w(px(0.0)).flex().flex_col().child(left));
        row = row.child(div().flex_1().min_w(px(0.0)).flex().flex_col().children(iter.next()));
        rows = rows.child(row);
    }
    v_flex()
        .w_full()
        .child(rows)
        .child(
            div()
                .mt(px(scale::SP_4))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("One header for every tool: status glyph, verb, mono target, right-aligned result and duration, chevron. Bodies differ: terminal output on the terminal ground with ANSI colours and a fold after eight lines, diffs with gutters, search hits, web results with favicons, browser actions with a screenshot and the click ring."),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    // NOTE: `use super::build`, not a glob: the parent's `use gpui::*`
    // imports gpui's `test` macro, and a glob would pull it into this
    // scope where the `#[gpui::test]` expansion's inner `#[test]`
    // re-resolves to the macro instead of the builtin, recursing forever.
    use super::build;
    use aui::transcript::{arm_chip_probe, take_drawn_chips};
    use gpui::{IntoElement, TestAppContext, Window};

    struct Host;

    impl gpui::Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            build(window, cx)
        }
    }

    /// The sample edit call carries the provider's summary, so the ordinary
    /// lone-block path draws its chips with no caller opting in.
    #[gpui::test]
    fn sample_blocks_draw_their_server_chips(cx: &mut TestAppContext) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        arm_chip_probe(true);
        take_drawn_chips();
        let (_host, _) = cx.add_window_view(|_, _| Host);
        let chips = take_drawn_chips();
        arm_chip_probe(false);
        assert!(chips.contains(&"+8".to_string()), "the sample edit block draws +8, got {chips:?}");
        assert!(chips.contains(&"−3".to_string()), "the sample edit block draws −3, got {chips:?}");
    }
}
