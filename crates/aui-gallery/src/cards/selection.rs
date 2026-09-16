//! Card · Cross-cell text selection. A multi-block message (paragraph, list,
//! code block) with a scripted span from the paragraph into the fence, plus
//! manual drag support: the card owns the span and its drag session the way
//! an app would.

use aui::transcript::{
    MessageSelection, SelectionEndpoint, SelectionKey, SpanSession, markdown, ProseStyle,
};
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

const SAMPLE: &str = "Tightened `validateAddress` so empty countries no longer pass.\n\n- Empty countries return `{ ok: false }`\n- Canadian codes skip the US branch\n- Coverage now spans CA, GB and US\n\n```rust\nif country.is_empty() {\n    return false;\n}\n```\n";

/// The scripted span: mid-paragraph (`p0`, past "Tightened ") into the fence
/// (`code2`, past the `if` line), so the paragraph tail, the whole list and
/// the head of the code block highlight on the first frame in both themes.
fn scripted_span() -> MessageSelection {
    MessageSelection {
        anchor: SelectionEndpoint {
            cell: SelectionKey::paragraph("", 0),
            offset: 10,
        },
        focus: SelectionEndpoint {
            cell: SelectionKey::code("", 2),
            offset: 24,
        },
    }
}

/// Card-owned span state, like an app would hold: the held span plus the
/// drag session that folds span events into it.
struct SpanState {
    held: Option<MessageSelection>,
    session: SpanSession,
}

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
    // The card owns the span, like an app would: span events fold through
    // the session into the held span, which renders straight back.
    let state = window.use_keyed_state("card-selection-span", cx, |_, _| SpanState {
        held: Some(scripted_span()),
        session: SpanSession::default(),
    });
    let current = state.read(cx).held.clone();
    let setter = state.clone();
    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(
            markdown("card-selection-span", SAMPLE, style)
                .span_selection(current.as_ref())
                .on_span_event(move |event, _, cx| {
                    setter.update(cx, |state, cx| {
                        let next = state.session.apply(state.held.clone(), &event);
                        state.held = next;
                        cx.notify();
                    });
                }),
        )
        .child(
            div()
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Cross-cell selection: one owner-level span (anchor cell + offset, focus cell + offset) with the cells in between fully selected. Drag from the paragraph into the code to move the scripted span (double-click a word, triple-click a paragraph); the highlight uses the selection token in both themes, and scrolling mid-drag never drops the span because the anchor lives in app state. The app copies Markdown::span_selected_text on ⌘C: blocks join with a blank line, wholly selected list items keep their markers, code keeps its exact bytes."),
        )
        .into_any_element()
}
