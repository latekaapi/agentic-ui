//! Memory page (harness docs/85 12.3b): the notes every agent in a project
//! shares, per project, as a plain list — title, type and date — each
//! opening in the preview, each deletable from its row. No editor. Built
//! from the Settings page's own parts (`section`, `settings_row`), so the
//! app builds it without new library pieces.

use aui::data::{icon_button, section, settings_row};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, Window};
use gpui_kit::base::v_flex;

/// One note row: its title opens it; the bin deletes it.
fn note(id: &str, title: &str, kind: &str, date: &str) -> aui::data::SettingsRow {
    settings_row(title.to_owned())
        .description(format!("{kind} \u{b7} {date}"))
        .control(
            icon_button(format!("memory-delete-{id}"), IconName::Trash)
                .ghost()
                .muted()
                .accessibility_label(format!("Delete the note {title}")),
        )
}

/// `memory/page`: two projects with notes, the personal notes, and a
/// project with none.
pub fn build_page(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .px(px(scale::SP_7))
        .pt(px(scale::SP_6))
        .gap(px(scale::SP_7))
        .child(
            v_flex()
                .gap(px(scale::SP_1))
                .child(div().role_in(TextRole::Title, cx).text_color(p.ink).child("Memory"))
                .child(
                    div()
                        .role_in(TextRole::Body, cx)
                        .text_color(p.fg_muted)
                        .child("What your agents remember, per project. Every provider reads the same notes."),
                ),
        )
        .child(
            section("shop")
                .row(note("gb", "GB postcode rule", "project", "25 Nov 2026"))
                .row(note("release", "Release day", "project", "22 Nov 2026"))
                .row(note("pay", "Payments sandbox keys live in 1Password", "reference", "18 Nov 2026"))
                .note("~/.claude/projects/-Users-ada-shop/memory"),
        )
        .child(
            section("harness")
                .row(note("fmt", "Never run cargo fmt here", "feedback", "24 Nov 2026"))
                .note("~/.claude/projects/-Users-ada-harness/memory"),
        )
        .child(section("Personal").row(note("ada", "Ada prefers short answers", "user", "20 Nov 2026")).note("Notes about you, in every project."))
        .child(section("acme-web").note("No notes yet. Agents save notes here as they learn."))
        .into_any_element()
}
