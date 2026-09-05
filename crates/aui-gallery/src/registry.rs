//! The list of gallery entries. One entry per design card (plus the motion
//! playground, the screens page and the assistant mock), keyed by the ids
//! used in `docs/03-parity-process.md`.

use aui_tokens::ThemeKind;
use gpui::{AnyElement, App, Window};

/// Which theme a card is designed in (`theme=` in the card's `ds:` header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // all three variants appear once every card is ported
pub enum CardTheme {
    /// Rendered in dark only.
    Dark,
    /// Rendered in light only.
    Light,
    /// Designed for both; follows the gallery's theme switch.
    Both,
}

impl CardTheme {
    /// The theme this card forces when opened, if any.
    pub fn fixed_kind(self) -> Option<ThemeKind> {
        match self {
            CardTheme::Dark => Some(ThemeKind::Dark),
            CardTheme::Light => Some(ThemeKind::Light),
            CardTheme::Both => None,
        }
    }
}

/// A gallery entry.
pub struct Entry {
    /// Stable id, `group/name`, as listed in the parity checklist.
    pub id: &'static str,
    /// Sidebar group ("Foundations", "Shell", …).
    pub group: &'static str,
    /// Card title from the `ds:` header.
    pub title: &'static str,
    /// Card subtitle from the `ds:` header.
    pub subtitle: &'static str,
    /// Card width in logical pixels (the reference PNG size).
    pub width: f32,
    /// Card height in logical pixels.
    pub height: f32,
    /// Theme the card is designed in.
    pub theme: CardTheme,
    /// Builds the card content. The gallery wraps it in the card frame
    /// (card background, 20 px padding) exactly like `body.ds` in the design.
    pub build: fn(&mut Window, &mut App) -> AnyElement,
}

/// All entries, in card order.
pub static ENTRIES: &[Entry] = &[
    Entry {
        id: "foundations/colour",
        group: "Foundations",
        title: "Colour",
        subtitle: "Semantic tokens, agent states, diff and terminal palettes in both themes",
        width: 960.0,
        height: 720.0,
        theme: CardTheme::Dark,
        build: crate::cards::colour::build,
    },
    Entry {
        id: "foundations/type",
        group: "Foundations",
        title: "Typography",
        subtitle: "Geist for UI, Geist Mono for code and terminals; scale 11\u{2013}24",
        width: 760.0,
        height: 560.0,
        theme: CardTheme::Dark,
        build: crate::cards::typography::build,
    },
    Entry {
        id: "foundations/space",
        group: "Foundations",
        title: "Spacing, radius, elevation",
        subtitle: "2/4/8/12/16/24/32 \u{b7} radii 4\u{2013}16 \u{b7} three shadow levels",
        width: 760.0,
        height: 420.0,
        theme: CardTheme::Dark,
        build: crate::cards::space::build,
    },
    Entry {
        id: "foundations/motion",
        group: "Foundations",
        title: "Motion",
        subtitle: "Durations, easings and the four springs; live samples",
        width: 900.0,
        height: 520.0,
        theme: CardTheme::Dark,
        build: crate::cards::motion::build,
    },
    Entry {
        id: "foundations/icons",
        group: "Foundations",
        title: "Icons and marks",
        subtitle: "Lucide-style 14/16 px glyphs, provider marks, status glyphs, file types",
        width: 760.0,
        height: 380.0,
        theme: CardTheme::Dark,
        build: crate::cards::icons::build,
    },
];

/// Looks an entry up by id.
pub fn find(id: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.id == id)
}

/// The distinct groups in order of first appearance.
pub fn groups() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for e in ENTRIES {
        if !out.contains(&e.group) {
            out.push(e.group);
        }
    }
    out
}
