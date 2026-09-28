//! Skills page. The list with its meter, filter and section rows beside the
//! detail pane; the Add skill menu; the import preview dialog.

use aui::skills::{
    cost_meter, import_preview, menu_row_two_line, scope_section_header, segmented, skill_detail, skill_row,
    switch, ChipTone, CostSegment, DetailAction, ImportRow, ImportStatus, InkLevel, SkillChip, SkillMode,
    SkillRowModel, SkillState,
};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

// ── the page ──────────────────────────────────────────────────────────────

/// The small line over the meter.
const METER_KICKER: &str = "At startup, enabled skills cost";

#[allow(clippy::too_many_arguments)]
fn row(
    id: &str,
    name: &str,
    description: &str,
    chips: Vec<SkillChip>,
    tokens: &str,
    mode: Option<SkillMode>,
    on: bool,
    dimmed: bool,
    selected: bool,
) -> impl IntoElement {
    skill_row(SkillRowModel {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        chips,
        tokens: tokens.into(),
        mode,
        on,
        dimmed,
        selected,
    })
    .on_select(|_, _, _| {})
    .on_toggle(|_, _, _| {})
    .on_set_mode(|_, _, _, _| {})
}

/// Builds the page content: meter, filter, sections and rows with the detail
/// pane at the right.
pub fn build_page(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut list = v_flex().flex_1().min_w(px(0.0)).px(px(8.0)).gap(px(0.0)).child(
        v_flex()
            .flex_none()
            .w_full()
            .gap(px(2.0))
            .px(px(12.0))
            .child(div().flex_none().ui(13.2).text_color(p.ink_3).child(METER_KICKER))
            .child(cost_meter(
                "6.9k",
                "tokens of context · 13 of 17 on",
                vec![
                    CostSegment::new("This project 1.2k", 0.18, InkLevel::Ink),
                    CostSegment::new("Personal 1.1k", 0.16, InkLevel::Ink2),
                    CostSegment::new("Built-in 4.6k", 0.46, InkLevel::Ink3),
                    CostSegment::new("Plugins off", 0.0, InkLevel::Ink4),
                ],
            )),
    );
    list = list.child(
        h_flex()
            .flex_none()
            .w_full()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .child(segmented(
                "skills-filter",
                vec!["All 17".into(), "This project 3".into(), "Personal 1".into(), "Plugins 1".into(), "Built-in 12".into()],
                0,
            ))
            .child(div().flex_1())
            .child(div().flex_none().ui(13.2).text_color(p.ink_2).child("Hide off"))
            .child(switch("hide-off", false, false).accessibility_label("Hide off")),
    );

    list = list.child(scope_section_header("This project", Some(".agents/skills · .claude/skills".into()), "3"));
    list = list.child(row(
        "release-notes",
        "release-notes",
        "Draft release notes from the PRs merged since the last tag, in the team's changelog format.",
        vec![],
        "290",
        Some(SkillMode::Auto),
        true,
        false,
        true,
    ));
    list = list.child(row(
        "git",
        "git",
        "Branch names, commit messages and PR titles the way acme-web does them.",
        vec![SkillChip::new("Overrides built-in git", ChipTone::Muted)],
        "410",
        Some(SkillMode::Auto),
        true,
        false,
        false,
    ));
    list = list.child(row(
        "db-migrations",
        "db-migrations",
        "Write and check Postgres migrations with the rollback rules from docs/db.md.",
        vec![SkillChip::new("1 issue", ChipTone::Warning)],
        "520",
        Some(SkillMode::Auto),
        true,
        false,
        false,
    ));

    list = list.child(scope_section_header("Personal", Some("all projects".into()), "1"));
    list = list.child(row(
        "decoction",
        "decoction",
        "Spec-anchored orchestration for long-running coding projects.",
        vec![],
        "1.1k",
        Some(SkillMode::Only),
        true,
        false,
        false,
    ));

    list = list.child(scope_section_header("Plugins", None, "1"));
    list = list.child(row(
        "threejs",
        "threejs",
        "Build and debug 3D scenes with three.js.",
        vec![SkillChip::new("threejs plugin", ChipTone::Muted)],
        "380",
        None,
        false,
        false,
        false,
    ));

    list = list.child(scope_section_header("Built-in", None, "12"));
    list = list.child(row(
        "plan",
        "plan",
        "Plan before a large change: explore, propose, then wait for a go-ahead.",
        vec![],
        "471",
        Some(SkillMode::Auto),
        true,
        false,
        false,
    ));
    list = list.child(row(
        "git-builtin",
        "git",
        "General git workflow: status, branches, commits and history.",
        vec![SkillChip::new("Overridden by this project's git", ChipTone::Muted)],
        "—",
        None,
        false,
        true,
        false,
    ));
    list = list.child(
        div()
            .flex_none()
            .px(px(12.0))
            .py(px(8.0))
            .ui(13.2)
            .text_color(p.ink_3)
            .child("Show 10 more"),
    );

    let detail = div().flex_none().w(px(460.0)).h_full().border_l_1().border_color(p.line).child(
        skill_detail("release-notes-detail", "release-notes")
            .scope("This project")
            .path("acme-web/.agents/skills/release-notes/SKILL.md")
            .state(SkillState::Automatic)
            .stat("At startup", "290 tokens")
            .stat("When loaded", "2.4k tokens")
            .stat("Last loaded", "Rewrite the router · 2h")
            .stat("Files", "3")
            .section(
                "What the model sees",
                div()
                    .ui(14.3)
                    .line_height(relative(1.6))
                    .text_color(p.ink_2)
                    .child("Draft release notes from the PRs merged since the last tag, in the team's changelog format. Use when the user asks for release notes, a changelog entry or what shipped."),
            )
            .section(
                "SKILL.md",
                v_flex()
                    .gap(px(8.0))
                    .ui(14.3)
                    .line_height(relative(1.65))
                    .text_color(p.ink_2)
                    .child(div().child("1. List merged PRs since the last tag."))
                    .child(div().child("2. Group them under Added, Changed and Fixed; drop chores."))
                    .child(div().child("3. Write one line per PR in CHANGELOG.md.")),
            )
            .section(
                "Files",
                v_flex()
                    .gap(px(6.0))
                    .mono(13.2)
                    .text_color(p.ink_2)
                    .child(div().child("SKILL.md"))
                    .child(div().child("scripts/merged_prs.sh"))
                    .child(div().child("references/changelog-format.md")),
            )
            .hint("Applies to open sessions")
            .overflow("More actions")
            .action(DetailAction::new("reveal", "Reveal in Finder", false))
            .action(DetailAction::new("open", "Open in editor", true))
            .on_intent(|_, _, _| {}),
    );

    h_flex().size_full().child(list).child(detail).into_any_element()
}

// ── the narrow row ────────────────────────────────────────────────────────

/// A narrow skill list (≈360 px of row width) with a long name and two long
/// chips beside the `Auto` chip and `1.1k` tokens: the name truncates and the
/// chips wrap inside the left block instead of painting over the right block.
pub fn build_narrow_row(_window: &mut Window, _cx: &mut App) -> AnyElement {
    v_flex()
        .w_full()
        .gap(px(8.0))
        .child(row(
            "decoction-long-narrow",
            "decoction-with-a-very-long-name-that-must-truncate",
            "Spec-anchored orchestration for long-running coding projects.",
            vec![
                SkillChip::new("Overrides personal decoction", ChipTone::Muted),
                SkillChip::new("threejs plugin", ChipTone::Muted),
            ],
            "1.1k",
            Some(SkillMode::Auto),
            true,
            false,
            false,
        ))
        .into_any_element()
}

// ── the add menu ──────────────────────────────────────────────────────────

/// The menu shell both library menus share, sized to the Add skill menu.
fn menu_shell() -> gpui::Div {
    v_flex().flex_none().w(px(340.0)).p(px(6.0)).rounded(px(scale::R_LG)).border_1()
}

/// A hairline between menu blocks.
fn menu_separator(cx: &mut App) -> impl IntoElement {
    div().flex_none().h(px(1.0)).my(px(6.0)).mx(px(4.0)).bg(cx.aui().colors.line)
}

/// Builds the Add skill menu: two-line rows with trailing muted text.
pub fn build_add_menu(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    div()
        .size_full()
        .flex()
        .items_start()
        .justify_center()
        .pt(px(40.0))
        .child(
            menu_shell()
                .bg(p.overlay)
                .border_color(p.line_strong)
                .shadow(p.shadow(3))
                .text_color(p.ink)
                .child(
                    menu_row_two_line(
                        "add-new",
                        IconName::Plus,
                        "New skill…",
                        "A SKILL.md from a template, in this project or Personal",
                    )
                    .key("new")
                    .accessibility_label("New skill. A SKILL.md from a template, in this project or Personal.")
                    .on_activate(|_, _, _| {}),
                )
                .child(
                    menu_row_two_line(
                        "add-install",
                        IconName::Folder,
                        "Install from folder…",
                        "Any folder with a SKILL.md, checked before it's copied",
                    )
                    .key("install")
                    .on_activate(|_, _, _| {}),
                )
                .child(menu_separator(cx))
                .child(
                    menu_row_two_line(
                        "add-import-claude",
                        IconName::Inbox,
                        "Import from Claude Code",
                        "~/.claude/skills",
                    )
                    .key("import-claude")
                    .subtitle_mono()
                    .trailing("3 new")
                    .on_activate(|_, _, _| {}),
                )
                .child(
                    menu_row_two_line(
                        "add-import-codex",
                        IconName::Inbox,
                        "Import from Codex",
                        "~/.codex/skills",
                    )
                    .key("import-codex")
                    .subtitle_mono()
                    .trailing("none")
                    .on_activate(|_, _, _| {}),
                )
                .child(menu_separator(cx))
                .child(
                    menu_row_two_line(
                        "add-ask",
                        IconName::Sparkle,
                        "Ask Muse to write one",
                        "Opens a session with /create-skill",
                    )
                    .key("ask")
                    .trailing("uses a turn")
                    .on_activate(|_, _, _| {}),
                ),
        )
        .into_any_element()
}

// ── the import preview ────────────────────────────────────────────────────

/// Builds the import preview dialog over its scrim.
pub fn build_import_preview(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div().relative().size_full().child(
        import_preview("card-import", "Import from Claude Code")
            .subtitle("Found 4 skills in ~/.claude/skills. Muse copies the ones you pick into Personal, so every project sees them.")
            .row(ImportRow::new(
                "pdf-tools",
                "pdf-tools",
                "Fill, merge and extract text from PDF files.",
                ImportStatus::New,
                "640",
                640,
            ))
            .row(ImportRow::new(
                "code-review",
                "code-review",
                "Review a diff for correctness bugs before a PR.",
                ImportStatus::New,
                "820",
                820,
            ))
            .row(ImportRow::new(
                "changelog",
                "changelog",
                "Keep CHANGELOG.md in Keep a Changelog format.",
                ImportStatus::Replaces,
                "300",
                300,
            ))
            .row(
                ImportRow::new(
                    "decoction",
                    "decoction",
                    "Already in Personal, unchanged",
                    ImportStatus::Installed,
                    "1.1k",
                    1100,
                )
                .unchecked(),
            )
            .summary("Adds 1.8k tokens to every session's startup context.")
            .hint("Nothing is copied until you import")
            // A static capture: no enter motion to blur it.
            .at_rest()
            .on_intent(|_, _, _| {}),
    )
    .into_any_element()
}
