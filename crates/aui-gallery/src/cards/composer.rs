//! Card 40 · Composer: queued message, meta strip, the focused floating card
//! with its `+` menu open, suggestion chips, and the docked variant.
//! Reproduces `design/src/cards/composer/40-composer.html` at 760×620.

use aui::composer::{composer, composer_state, composer_state_rows, plus_menu, queue_row, suggestion_chips, ComposerChip, ComposerChipKind, ComposerIntent, ComposerMeta, PlusMenuItem};
use aui_icons::{IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.wrap{max-width:640px;gap:18px}`.
const WRAP_MAX: f32 = 640.0;
const WRAP_GAP: f32 = 18.0;
/// The docked variant spans the card: `margin:0 -20px`.
const DOCKED_BLEED: f32 = -20.0;
/// The caps label above it: `margin-top:6px`.
const CAPS_TOP: f32 = 6.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;
const DRAFT: &str = "Add GB postcode validation too, then run the focused tests and lint.";

/// A 16 px procedural preview (diagonal gray gradient) standing in for a
/// decoded upload, so the thumbnail variants render without image files.
pub fn sample_thumbnail() -> std::sync::Arc<gpui::RenderImage> {
    const SIDE: u32 = 16;
    let buffer = image::ImageBuffer::from_fn(SIDE, SIDE, |x, y| {
        let v = ((x + y) * 255 / (2 * (SIDE - 1))) as u8;
        image::Rgba([v, v, v, 255])
    });
    std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buffer)]))
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let text = window.use_keyed_state("card40-text", cx, |window, cx| {
        let mut state = composer_state("Reply, or type / for commands and @ to mention files", window, cx);
        state.set_value(DRAFT, window, cx);
        state
    });
    let docked_text = window.use_keyed_state("card40-docked-text", cx, |window, cx| composer_state_rows("Reply, or type / for commands and @ to mention files", 1, 8, window, cx));
    let ui = window.use_keyed_state("card40-ui", cx, |_, _| (true, true)); // (plus open, streaming)
    let (plus_open, streaming) = *ui.read(cx);
    let ui_for_intent = ui.clone();
    let chips = vec![
        ComposerChip { id: "mention".into(), kind: ComposerChipKind::Mention, label: "src/checkout".into(), removable: true, thumbnail: None, detail: None },
        ComposerChip {
            id: "image".into(),
            kind: ComposerChipKind::Image,
            label: "form.png".into(),
            removable: true,
            thumbnail: Some(sample_thumbnail()),
            detail: None,
        },
        ComposerChip {
            id: "file".into(),
            kind: ComposerChipKind::File,
            label: "design-spec.pdf".into(),
            removable: true,
            thumbnail: None,
            detail: Some("PDF · 2.1 MB".into()),
        },
        ComposerChip { id: "skill".into(), kind: ComposerChipKind::Skill, label: "test-writer".into(), removable: false, thumbnail: None, detail: None },
    ];
    let menu = plus_menu(
        "card40-plus-menu",
        vec![
            PlusMenuItem::new("attach", IconName::Paperclip, "Attach file").key("⌘U"),
            PlusMenuItem::new("screenshot", IconName::Camera, "Screenshot browser"),
            PlusMenuItem::new("mention", IconName::At, "Mention file or symbol"),
            PlusMenuItem::new("skill", IconName::Zap, "Use a skill").key("$"),
            PlusMenuItem::new("commands", IconName::Slash, "Commands").key("/"),
        ],
        plus_open,
    )
    .at_rest();

    v_flex()
        .w_full()
        .max_w(px(WRAP_MAX))
        .gap(px(WRAP_GAP))
        .child(queue_row("card40-queue", "Also run the e2e suite for checkout after unit tests pass"))
        .child(
            composer("card40-composer", &text, Provider::Claude, "Opus 4.6")
                .chips(chips)
                .mode("Plan")
                .effort("High")
                .streaming(streaming)
                .focused(true)
                .plus_menu(plus_open, Some(menu))
                .meta(ComposerMeta { branch: "feature/checkout-flow-v2".into(), context: 0.34, cost: "$0.31 this session".into(), hint: "⌘↩ to queue".into() })
                .on_intent(move |intent, _, cx| {
                    ui_for_intent.update(cx, |(open, streaming), cx| {
                        match intent {
                            ComposerIntent::TogglePlus => *open = !*open,
                            ComposerIntent::Send | ComposerIntent::Stop => *streaming = !*streaming,
                            _ => {}
                        }
                        cx.notify();
                    })
                }),
        )
        .child(suggestion_chips("card40-suggestions", vec!["Run the e2e suite".into(), "Open the diff for review".into(), "Draft a commit message".into()]).at_rest())
        .child(div().mt(px(CAPS_TOP)).text_role(TextRole::Caps).line_height(relative(scale::LH_UI)).text_color(p.ink_3).child("DOCKED · SPANS THE PANE AT THE BOTTOM"))
        .child(
            div().mx(px(DOCKED_BLEED)).child(composer("card40-docked", &docked_text, Provider::Codex, "gpt-5.5 medium").mode("Auto").docked(true).can_send(false)),
        )
        .child(
            div()
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("In the shell the composer is docked: full pane width, a top hairline, no card. The floating card above is the focused state with the accent ring. Context chips sit above the text; the toolbar holds the plus menu (rotates 45° and morphs the popover from its corner), model, mode and effort chips, then the send button which morphs to stop while a turn runs (click it). Queued messages stack above with a dashed border. Suggestions rise in with a 60 ms stagger."),
        )
        .into_any_element()
}
