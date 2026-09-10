//! Card · Markdown blocks. Renders `aui::transcript::markdown` over a sample
//! covering every block: headings, paragraph spans, both list kinds, a fenced
//! block, a table, a quote, a rule and an image placeholder. The sample is
//! selectable: the card holds one `Option<TextSelection>` in its own window
//! state and reads it back through `Markdown::selection`.

use aui::transcript::{markdown, ProseStyle, TextSelection};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.tr{gap:18px}` — matches the turns card rhythm.
const BLOCK_GAP: f32 = 18.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;
/// `.a{font-size:13.5px;line-height:1.65}` with a 10 px paragraph gap.
const BODY_TEXT: f32 = 13.5;
const PARAGRAPH_GAP: f32 = 10.0;

const SAMPLE: &str = "## Validator patch\n\nTightened `validateAddress` in `crates/aui/src/transcript/turns.rs:90` — see the [turns card](transcript/turns), crates/aui/src/transcript/turns.rs and https://example.com/spec for context. The old branch is ~gone~.\n\n- Empty countries no longer pass\n- Canadian codes skip the US branch\n\n1. Patch the ZIP/postal path\n2. Run the two focused test files\n\n```rust\nif country.is_empty() {\n    return false;\n}\n```\n\n| File | Lines | State |\n| :-- | :-: | --: |\n| turns.rs | 12 | done |\n| prose.rs | 8 | open |\n\n> Keep the checkout copy unchanged.\n\n---\n\n![checkout form](checkout-form.png)";

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let style = ProseStyle {
        ink: p.ink,
        code_ink: p.ink,
        code_bg: p.surface_2,
        size: BODY_TEXT,
        line_height: scale::LH_BODY,
        paragraph_gap: PARAGRAPH_GAP,
    };
    // The card owns the selection, like an app would: drags, word and
    // paragraph picks replace it, plain clicks clear it.
    let selection = window.use_keyed_state("card-markdown-selection", cx, |_, _| None::<TextSelection>);
    let current = selection.read(cx).clone();
    let setter = selection.clone();
    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(
            markdown("card-markdown", SAMPLE, style)
                .selection(current.as_ref())
                .on_selection_change(move |next, _, cx| {
                    setter.update(cx, |held, cx| {
                        *held = next;
                        cx.notify();
                    });
                }),
        )
        .child(
            div()
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Headings step down the token ramp; links underline in accent and click through on_link; fences reuse the code block; tables scroll inside their own frame; images are placeholder tiles and never fetch. Drag inside any paragraph, heading, list item, quote, table cell or fence to select (double-click a word, triple-click a paragraph); a press-release on a link still follows it, and the app copies Markdown::selected_text on ⌘C. Selections never span cells."),
        )
        .into_any_element()
}
