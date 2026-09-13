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
