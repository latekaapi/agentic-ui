//! Card 31 · User and assistant turns. Reproduces
//! `design/src/cards/transcript/31-turns.html` at 760×560.

use aui::protocol::{Attachment, AttachmentKind, TurnMeta, UploadState};
use aui::transcript::{assistant_turn, user_turn, TextSelection};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.tr{gap:18px}`.
const TURN_GAP: f32 = 18.0;
/// `.ds-note{max-width:80ch}` ≈ 626 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

const USER_TEXT: &str = "Tighten address validation in `@src/checkout` and add coverage for CA and GB postcodes. Keep the existing copy.";
const ASSISTANT_TEXT: &str = "I'm checking the existing form flow, then I'll patch the validator and run the focused tests.\n\n\
Found the country-specific branch in `validateAddress`. Two things stand out:\n\n\
- An empty country currently returns `true`, which lets a blank address through.\n\
- Canadian postal codes fall into the US ZIP branch.\n\n\
I'll make the ZIP/postal path explicit and keep the checkout copy unchanged, then run the two focused test files";

/// Stores a card-level selection intent: drags, word and paragraph picks
/// replace the stored selection, plain clicks clear it.
fn track_selection(
    setter: Entity<Option<TextSelection>>,
) -> impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static {
    move |next, _, cx| {
        setter.update(cx, |held, cx| {
            *held = next;
            cx.notify();
        });
    }
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let attachments = vec![
        Attachment { name: "validators.ts".into(), kind: AttachmentKind::File, size_bytes: None, meta: Some("180 lines".into()), state: UploadState::Ready },
        Attachment { name: "checkout-form.png".into(), kind: AttachmentKind::Image, size_bytes: None, meta: None, state: UploadState::Ready },
    ];
    // The card owns the selection, like an app would: one
    // `Option<TextSelection>` wired through every turn.
    let selection = window.use_keyed_state("card31-selection", cx, |_, _| None::<TextSelection>);
    let current = selection.read(cx).clone();
    v_flex()
        .w_full()
        .gap(px(TURN_GAP))
        .child(div().w_full().flex().justify_end().child(
            user_turn("card31-user", USER_TEXT)
                .attachments(attachments)
                .selection(current.clone())
                .on_selection_change(track_selection(selection.clone())),
        ))
        .child(
            assistant_turn("card31-assistant", ASSISTANT_TEXT)
                .streaming(true)
                .meta(TurnMeta { model: "opus 4.6".into(), duration_ms: 3100, tokens_in: 1800, tokens_out: 600, reasoning_tokens: 0, cost_usd: 0.04 })
                .selection(current.clone())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(div().w_full().flex().justify_end().child(
            user_turn("card31-user-bottom", USER_TEXT)
                .actions_bottom(true)
                .selection(current.clone())
                .on_selection_change(track_selection(selection.clone())),
        ))
        .child(
            assistant_turn("card31-assistant-bottom", ASSISTANT_TEXT)
                .meta(TurnMeta { model: "opus 4.6".into(), duration_ms: 3100, tokens_in: 1800, tokens_out: 600, reasoning_tokens: 0, cost_usd: 0.04 })
                .actions_bottom(true)
                .selection(current.clone())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(
            div()
                .mt(px(scale::SP_4) - px(TURN_GAP))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("User turns sit right in a soft bubble; assistant turns are full-width text with no bubble, so the transcript reads like a document. Streamed chunks fade and rise 3 px; the caret blinks on the accent. The toolbar appears on hover above the turn and never shifts layout. The bottom-row variant pins the same actions in-flow under the prose, muted until hover. Drag inside any turn to select (double-click a word, triple-click a paragraph): the card holds one Option<TextSelection> wired through each turn's selection/on_selection_change, and copies it out with turn_selected_text."),
        )
        .into_any_element()
}
