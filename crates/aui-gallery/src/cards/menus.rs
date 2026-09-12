//! Card 41 · Slash commands and mentions. The `/` menu with its Built-in /
//! Skills / Custom sections and key hints beside the `@` picker for files,
//! symbols and worktrees, each above the composer it anchors to. Reproduces
//! `design/src/cards/composer/41-slash-mentions.html` at 800×520.

use aui::composer::{command_menu, mention_picker, CommandItem, CommandSection, MentionIcon, MentionItem, MentionSection};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.grid{grid-template-columns:1fr 1fr;gap:16px;max-width:780px}`.
const GRID_GAP: f32 = 16.0;
const GRID_MAX: f32 = 780.0;
/// `.cp{border-radius:14px;padding:10px 14px;font-size:14px;min-height:60px}`
/// with `box-shadow:0 0 0 3px var(--accent-ring)`.
const CP_RADIUS: f32 = 14.0;
const CP_PAD_X: f32 = 14.0;
const CP_PAD_Y: f32 = 10.0;
const CP_TEXT: f32 = scale::FS_14;
const CP_MIN_H: f32 = 60.0;
const CP_RING: f32 = 3.0;
/// `.ds-note{max-width:80ch;margin-top:12px}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;
const NOTE_TOP: f32 = 12.0;
/// What has been typed after the `/` and the `@`.
const COMMAND_QUERY: &str = "/re";
const MENTION_QUERY: &str = "ators";
/// The row each menu shows highlighted (`.it.on`): the first one.
const SELECTED: usize = 0;
/// The row the long menu shows highlighted: past the visible band, so the
/// capture proves the container scrolled it into view.
const LONG_SELECTED: usize = 25;

/// The three sections of the `/` menu.
fn command_sections() -> Vec<CommandSection> {
    vec![
        CommandSection::new(
            "Built-in",
            vec![
                CommandItem::new("review", "/review", "Review the current diff for bugs").key("↩"),
                CommandItem::new("resume", "/resume", "Resume a paused session"),
                CommandItem::new("rewind", "/rewind", "Restore files to an earlier checkpoint"),
            ],
        ),
        CommandSection::new("Skills", vec![CommandItem::new("report", "/report", "Draft a report from this session").source_tag("skill")]),
        CommandSection::new(
            "Custom",
            vec![CommandItem::new("release-notes", "/release-notes", ".claude/commands/release-notes.md").source_tag("project")],
        ),
    ]
}

/// The three sections of the `@` picker.
fn mention_sections() -> Vec<MentionSection> {
    vec![
        MentionSection::new(
            "Files in acme-web",
            vec![
                MentionItem::new("validators", MentionIcon::Glyph(IconName::File), "validators.ts", "src/checkout")
                    .matching(MENTION_QUERY)
                    .detail_mono()
                    .key("↩"),
                MentionItem::new("validators-test", MentionIcon::Glyph(IconName::File), "validators.test.ts", "src/checkout")
                    .matching(MENTION_QUERY)
                    .detail_mono(),
                MentionItem::new("checkout", MentionIcon::Glyph(IconName::Folder), "src/checkout", "folder · 9 files"),
                // A long row: under the dropdown ceiling the name wraps
                // instead of pushing the detail off the menu.
                MentionItem::new(
                    "messages",
                    MentionIcon::Glyph(IconName::File),
                    "checkout-form-validation-messages.ts",
                    "src/checkout/forms/validation/messages",
                )
                .detail_mono(),
            ],
        ),
        MentionSection::new(
            "Symbols",
            vec![MentionItem::new("validate-address", MentionIcon::Symbol, "validateAddress", "validators.ts:12").detail_mono()],
        ),
        MentionSection::new(
            "Worktrees and more",
            vec![
                MentionItem::new("checkout-flow-v2", MentionIcon::Dot(AgentState::Running), "checkout-flow-v2", "transcript of another worktree"),
                MentionItem::new("localhost", MentionIcon::Glyph(IconName::Globe), "localhost:3000/checkout", "open browser tab"),
            ],
        ),
    ]
}

/// The thirty rows of the scrolling menu: ten built-ins, ten skills, ten
/// custom commands.
fn long_command_sections() -> Vec<CommandSection> {
    let builtin = ["review", "resume", "rewind", "report", "compact", "init", "doctor", "login", "logout", "help"];
    let skills = ["release-notes", "commit-msg", "pr-review", "changelog", "onboard", "migrate", "perf", "a11y", "i18n", "docs"];
    let custom = ["deploy", "staging", "rollback", "seed", "bench", "smoke", "e2e", "lint", "format", "clean"];
    vec![
        CommandSection::new(
            "Built-in",
            builtin
                .iter()
                .map(|name| CommandItem::new(*name, format!("/{}", name), format!("Built-in {} command", name)).key("↩"))
                .collect(),
        ),
        CommandSection::new(
            "Skills",
            skills.iter().map(|name| CommandItem::new(*name, format!("/{}", name), format!("{} skill", name)).source_tag("skill")).collect(),
        ),
        CommandSection::new(
            "Custom",
            custom
                .iter()
                .map(|name| CommandItem::new(*name, format!("/{}", name), format!(".claude/commands/{}.md", name)).source_tag("project"))
                .collect(),
        ),
    ]
}

/// `.cp`: the composer the popover anchors to, drawn as a focused stand-in so
/// the card can show the caret's text without the whole composer.
fn composer_stand_in(children: Vec<AnyElement>, cx: &App) -> impl IntoElement {
    let p = cx.aui().colors;
    h_flex()
        // `.cp` is a block, not a centred box: its text sits under the 10 px
        // top padding however tall the 60 px minimum leaves it.
        .items_start()
        .w_full()
        .min_h(px(CP_MIN_H))
        .px(px(CP_PAD_X))
        .py(px(CP_PAD_Y))
        .rounded(px(CP_RADIUS))
        .border_1()
        .border_color(p.accent)
        .bg(p.surface_1)
        .shadow(vec![BoxShadow { color: p.accent_ring, offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(CP_RING), inset: false }])
        .ui(CP_TEXT)
        .text_color(p.ink)
        .children(children)
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    // Hovering a row moves the highlight, so the pointer and the arrow keys
    // agree on what `↩` would insert.
    let command_selected = window.use_keyed_state("card41-command-selected", cx, |_, _| SELECTED);
    let mention_selected = window.use_keyed_state("card41-mention-selected", cx, |_, _| SELECTED);
    let long_selected = window.use_keyed_state("card41-long-selected", cx, |_, _| LONG_SELECTED);
    let command_index = *command_selected.read(cx);
    let mention_index = *mention_selected.read(cx);
    let long_index = *long_selected.read(cx);
    let command_hover = command_selected.clone();
    let mention_hover = mention_selected.clone();
    let long_hover = long_selected.clone();

    v_flex()
        .w_full()
        .max_w(px(GRID_MAX))
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap(px(GRID_GAP))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(
                            command_menu("card41-commands", COMMAND_QUERY, command_sections(), command_index)
                                // A static composition, not a menu the person
                                // just opened: draw it at rest.
                                .at_rest()
                                .on_hover(move |index, _, cx| {
                                    command_hover.update(cx, |current, cx| {
                                        if *current != index {
                                            *current = index;
                                            cx.notify();
                                        }
                                    })
                                })
                                .on_select(|_, _, _| {}),
                        )
                        .child(composer_stand_in(
                            vec![
                                div().flex_none().mono(CP_TEXT).text_color(p.accent_ink).child("/re").into_any_element(),
                                div().flex_none().text_color(p.ink_3).child("view").into_any_element(),
                            ],
                            cx,
                        )),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(
                            mention_picker("card41-mentions", MENTION_QUERY, mention_sections(), mention_index)
                                .at_rest()
                                .on_hover(move |index, _, cx| {
                                    mention_hover.update(cx, |current, cx| {
                                        if *current != index {
                                            *current = index;
                                            cx.notify();
                                        }
                                    })
                                })
                                .on_select(|_, _, _| {}),
                        )
                        .child(composer_stand_in(
                            vec![
                                div().flex_none().child("Tighten validation in ").into_any_element(),
                                div().flex_none().mono(CP_TEXT).text_color(p.accent_ink).child("@validators").into_any_element(),
                            ],
                            cx,
                        )),
                ),
        )
        .child(
            div()
                .mt(px(NOTE_TOP))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Both menus anchor to the caret, open upward from the composer and are fully keyboard-driven. Typed characters highlight in accent inside matches. Selecting a mention inserts a chip; selecting a command either runs it or expands into its arguments inline."),
        )
        .child(
            v_flex()
                .w_full()
                .mt(px(NOTE_TOP))
                .child(div().text_role(TextRole::Caps).text_color(p.ink_3).child("Long menu · scrolls inside its own frame · selection 25 of 30".to_uppercase()))
                .child(
                    command_menu("card41-commands-long", "", long_command_sections(), long_index)
                        .at_rest()
                        .on_hover(move |index, _, cx| {
                            long_hover.update(cx, |current, cx| {
                                if *current != index {
                                    *current = index;
                                    cx.notify();
                                }
                            })
                        })
                        .on_select(|_, _, _| {}),
                )
                .child(composer_stand_in(
                    vec![div().flex_none().mono(CP_TEXT).text_color(p.accent_ink).child("/").into_any_element()],
                    cx,
                ))
                .child(
                    div()
                        .mt(px(NOTE_TOP))
                        .max_w(px(NOTE_MEASURE))
                        .ui(scale::FS_12)
                        .text_color(p.ink_3)
                        .child("Thirty rows in a 560 px frame: the wheel scrolls the menu, never the page behind, and the arrow keys keep the selection visible — row 25 sits in view on this capture."),
                ),
        )
        .into_any_element()
}
