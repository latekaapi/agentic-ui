//! Overlay · Settings dialog. The settings card over its scrim: the header
//! with its title, `esc` keycap and close glyph; the section rail; the open
//! section's switch rows with a detail line, a heading and a note. The card
//! shows what the component paints at rest; the intents are no-ops, like the
//! dialog card's.

use aui::overlay::{settings_dialog, SettingsRow, SettingsSection};
use gpui::*;

/// The sections of the card, in order: "Sidebar" first, as the app opens it.
fn sections() -> Vec<SettingsSection> {
    vec![
        SettingsSection {
            id: "sidebar".into(),
            label: "Sidebar".into(),
            rows: vec![
                SettingsRow::Switch { id: "collapse-chevron".into(), label: "Collapse chevron".into(), detail: None, on: true },
                SettingsRow::Switch {
                    id: "current-bar".into(),
                    label: "Current-project bar".into(),
                    detail: Some("Marks the project the open session belongs to.".into()),
                    on: true,
                },
                SettingsRow::Switch { id: "branch-name".into(), label: "Branch name".into(), detail: None, on: false },
            ],
        },
        SettingsSection {
            id: "appearance".into(),
            label: "Appearance".into(),
            rows: vec![
                SettingsRow::Heading { text: "Density".into() },
                SettingsRow::Switch { id: "compact-rows".into(), label: "Compact rows".into(), detail: None, on: false },
                SettingsRow::Note {
                    text: "Compact rows tighten the sidebar and the transcript without changing type sizes.".into(),
                },
            ],
        },
    ]
}

/// Builds the card content.
pub fn build(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div()
        .relative()
        .size_full()
        .child(
            settings_dialog("card-settings", sections(), 0)
                // A static capture: no enter motion to blur it.
                .at_rest()
                .on_select_section(|_, _, _| {})
                .on_switch(|_, _, _, _| {})
                .on_dismiss(|_, _| {}),
        )
        .into_any_element()
}

/// A long page: 40+ shortcut rows under headings, to prove the page scrolls
/// inside the bounded card while the header and the section rail stay fixed.
fn long_sections() -> Vec<SettingsSection> {
    fn shortcuts(group: &str, n: usize) -> Vec<SettingsRow> {
        (0..n)
            .map(|i| SettingsRow::Shortcut {
                id: format!("{group}-{i}").into(),
                label: format!("Shortcut action {i}").into(),
                detail: Some(format!("What {group} shortcut {i} does.").into()),
                keystroke: Some(format!("cmd-{i}").into()),
                recording: false,
                editable: true,
            })
            .collect()
    }

    let mut rows = vec![SettingsRow::Heading { text: "General".into() }];
    rows.extend(shortcuts("general", 15));
    rows.push(SettingsRow::Heading { text: "Terminal".into() });
    rows.extend(shortcuts("terminal", 15));
    rows.push(SettingsRow::Heading { text: "Sessions".into() });
    rows.extend(shortcuts("session", 12));
    vec![
        SettingsSection { id: "sidebar".into(), label: "Sidebar".into(), rows: sections()[0].rows.clone() },
        SettingsSection { id: "shortcuts".into(), label: "Shortcuts".into(), rows },
    ]
}

/// Builds the long-page content: the Shortcuts section open.
pub fn build_long(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div()
        .relative()
        .size_full()
        .child(
            settings_dialog("card-settings-long", long_sections(), 1)
                // A static capture: no enter motion to blur it.
                .at_rest()
                .on_select_section(|_, _, _| {})
                .on_switch(|_, _, _, _| {})
                .on_shortcut(|_, _, _, _| {})
                .on_dismiss(|_, _| {}),
        )
        .into_any_element()
}
