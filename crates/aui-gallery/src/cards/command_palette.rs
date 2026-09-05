//! Card 12 · Command palette. The ⌘K palette floating on its scrim, with the
//! worktree / action / file sections, the fuzzy match highlights and the key
//! hints. Reproduces `design/src/cards/shell/12-command-palette.html` at
//! 760×520.

use aui::overlay::{command_palette, palette_scrim, PaletteIcon, PaletteItem, PaletteSection};
use aui_icons::IconName;
use aui_tokens::AgentState;
use gpui::*;

/// The query the card is frozen on: `<input value="wor">`.
const QUERY: &str = "wor";
/// `<input placeholder="Search actions, worktrees, files…">`.
const PLACEHOLDER: &str = "Search actions, worktrees, files…";
/// The row the card shows highlighted (`.it.on`): the first worktree.
const SELECTED: usize = 0;

/// The three sections of the card, in order.
fn sections() -> Vec<PaletteSection> {
    vec![
        PaletteSection::new(
            "Worktrees",
            vec![
                PaletteItem::new("checkout", PaletteIcon::Dot(AgentState::Running), "checkout-flow-v2").context("acme-web").key("↩"),
                PaletteItem::new("notifier", PaletteIcon::Dot(AgentState::Waiting), "infra/notifier").badge("needs you"),
            ],
        ),
        PaletteSection::new(
            "Actions",
            vec![
                PaletteItem::new("new-worktree", PaletteIcon::Glyph(IconName::Plus), "New worktree from prompt").matching(QUERY).key("⌘").key("N"),
                PaletteItem::new("split", PaletteIcon::Glyph(IconName::Split), "Split terminal right").key("⌘").key("D"),
                PaletteItem::new("browser", PaletteIcon::Glyph(IconName::Globe), "Open browser tab").key("⌘").key("⇧").key("B"),
            ],
        ),
        PaletteSection::new(
            "Files",
            vec![PaletteItem::new("queue", PaletteIcon::Glyph(IconName::File), "src/worker/queue.ts").matching(QUERY)],
        ),
    ]
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    // Hovering or clicking a row moves the highlight, so the pointer and the
    // arrow keys would agree on what `↩` opens.
    let selected_state = window.use_keyed_state("card12-selected", cx, |_, _| SELECTED);
    let selected = *selected_state.read(cx);
    let on_hover = selected_state.clone();

    palette_scrim(
        command_palette("card12-palette", QUERY, sections(), selected)
            .placeholder(PLACEHOLDER)
            // The card is a static composition, not a palette the person just
            // opened: draw it at rest so the enter does not blur the capture.
            .at_rest()
            .on_hover(move |index, _, cx| {
                on_hover.update(cx, |current, cx| {
                    if *current != index {
                        *current = index;
                        cx.notify();
                    }
                })
            })
            .on_select(move |_, _, _| {})
            .on_dismiss(move |_, _| {}),
    )
    .into_any_element()
}
