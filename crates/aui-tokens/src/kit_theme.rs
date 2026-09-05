//! Projects the design tokens onto gpui-kit's theme so its components (Dock,
//! Editor, TextView, Input, Menu, Tabs…) look like the design without
//! per-component overrides.
//!
//! The output is a gpui-kit `ThemeSet` JSON document with two themes,
//! [`KIT_THEME_LIGHT`] and [`KIT_THEME_DARK`]. `cargo run -p aui-tokens
//! --example dump-theme` writes it to `crates/aui-tokens/themes/agentic-ui.json`
//! so other gpui-kit apps can drop it into their `themes/` directory.

use gpui::{Hsla, Rgba};
use serde_json::{json, Map, Value};

use crate::{
    generated::{dark, light, scale, Palette},
    theme::ThemeKind,
};

/// Name of the generated light theme inside gpui-kit's registry.
pub const KIT_THEME_LIGHT: &str = "Agentic UI Light";
/// Name of the generated dark theme inside gpui-kit's registry.
pub const KIT_THEME_DARK: &str = "Agentic UI Dark";

fn hex(color: Hsla) -> String {
    let c: Rgba = color.into();
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if c.a >= 0.999 {
        format!("#{:02x}{:02x}{:02x}", ch(c.r), ch(c.g), ch(c.b))
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", ch(c.r), ch(c.g), ch(c.b), ch(c.a))
    }
}

fn white() -> Hsla {
    gpui::white()
}

fn transparent() -> Hsla {
    gpui::transparent_black()
}

/// gpui-kit colour roles for one palette.
fn colors(p: &Palette, kind: ThemeKind) -> Map<String, Value> {
    let scrim = if kind.is_dark() {
        gpui::black().alpha(0.45)
    } else {
        gpui::black().alpha(0.25)
    };
    let pairs: Vec<(&str, Hsla)> = vec![
        ("background", p.bg),
        ("foreground", p.ink),
        ("border", p.line),
        ("input.border", p.line_strong),
        ("ring", p.accent_ring),
        ("caret", p.accent),
        ("selection.background", p.selection),
        // Primary = accent, secondary = surface step, muted = tertiary text.
        ("primary.background", p.accent),
        ("primary.hover.background", p.accent_strong),
        ("primary.active.background", p.accent_ink),
        ("primary.foreground", white()),
        ("secondary.background", p.surface_2),
        ("secondary.hover.background", p.surface_3),
        ("secondary.active.background", p.surface_3),
        ("secondary.foreground", p.ink),
        ("muted.background", p.surface_2),
        ("muted.foreground", p.ink_3),
        // Hover accents on menu / list items are a surface step, never an accent wash.
        ("accent.background", p.surface_3),
        ("accent.foreground", p.ink),
        ("accordion.background", p.surface_1),
        // Buttons: secondary is filled and bordered.
        ("button.background", p.surface_2),
        ("button.hover.background", p.surface_3),
        ("button.active.background", p.surface_3),
        ("button.foreground", p.ink),
        ("button.primary.background", p.accent),
        ("button.primary.hover.background", p.accent_strong),
        ("button.primary.active.background", p.accent_ink),
        ("button.primary.foreground", white()),
        ("button.secondary.background", p.surface_2),
        ("button.secondary.hover.background", p.surface_3),
        ("button.secondary.active.background", p.surface_3),
        ("button.secondary.foreground", p.ink),
        ("button.danger.background", transparent()),
        ("button.danger.hover.background", p.danger_soft),
        ("button.danger.active.background", p.danger_soft),
        ("button.danger.foreground", p.danger),
        ("button.success.background", p.success_soft),
        ("button.success.foreground", p.success),
        ("button.warning.background", p.warning_soft),
        ("button.warning.foreground", p.warning),
        ("button.info.background", p.info_soft),
        ("button.info.foreground", p.info),
        // Status.
        ("danger.background", p.danger),
        ("danger.foreground", white()),
        ("success.background", p.success),
        ("success.foreground", white()),
        ("warning.background", p.warning),
        ("warning.foreground", white()),
        ("info.background", p.info),
        ("info.foreground", white()),
        // Popovers and overlays.
        ("popover.background", p.overlay),
        ("popover.foreground", p.ink),
        ("overlay", scrim),
        ("window.border", p.line),
        // Sidebar.
        ("sidebar.background", p.surface_1),
        ("sidebar.foreground", p.ink),
        ("sidebar.border", p.line),
        ("sidebar.accent.background", p.surface_3),
        ("sidebar.accent.foreground", p.ink),
        ("sidebar.primary.background", p.accent),
        ("sidebar.primary.foreground", white()),
        // Lists and tables: selected rows are a surface step.
        ("list.background", p.surface_1),
        ("list.hover.background", p.surface_2),
        ("list.active.background", p.surface_3),
        ("list.active.border", p.line_strong),
        ("list.even.background", p.surface_1),
        ("list.head.background", p.surface_2),
        ("table.background", p.surface_1),
        ("table.hover.background", p.surface_2),
        ("table.active.background", p.surface_3),
        ("table.active.border", p.line_strong),
        ("table.even.background", p.surface_1),
        ("table.head.background", p.surface_2),
        ("table.head.foreground", p.ink_3),
        ("table.foot.background", p.surface_2),
        ("table.foot.foreground", p.ink_3),
        ("table.row.border", p.line),
        // Tabs: indicator is ink, inactive ink-3.
        ("tab.background", transparent()),
        ("tab.active.background", p.surface_1),
        ("tab.active.foreground", p.ink),
        ("tab.foreground", p.ink_3),
        ("tab_bar.background", p.surface_1),
        ("tab_bar.segmented.background", p.surface_2),
        // Chrome.
        ("title_bar.background", p.surface_1),
        ("title_bar.border", p.line),
        ("status_bar.background", p.surface_1),
        ("status_bar.border", p.line),
        ("tiles.background", p.bg),
        ("group_box.background", p.surface_2),
        ("group_box.foreground", p.ink),
        ("description_list.label.background", p.surface_2),
        ("description_list.label.foreground", p.ink_3),
        // Controls.
        ("link", p.accent_ink),
        ("link.hover", p.accent_strong),
        ("link.active", p.accent_ink),
        ("progress.bar.background", p.ink_3),
        ("slider.background", p.ink),
        ("slider.thumb.background", p.surface_1),
        ("switch.background", p.surface_3),
        ("switch.thumb.background", white()),
        ("skeleton.background", p.surface_3),
        ("scrollbar.background", transparent()),
        ("scrollbar.thumb.background", p.ink_4.alpha(0.6)),
        ("scrollbar.thumb.hover.background", p.ink_4),
        ("drag.border", p.accent),
        ("drop_target.background", p.accent_soft),
        // Charts: accent ramp.
        ("chart.1", p.accent_soft),
        ("chart.2", p.accent_ring),
        ("chart.3", p.accent),
        ("chart.4", p.accent_strong),
        ("chart.5", p.accent_ink),
        ("chart_bullish", p.success),
        ("chart_bearish", p.danger),
        // Base colours feed gpui-kit's fallbacks and the terminal-ish widgets.
        ("base.red", p.ansi_red),
        ("base.red.light", p.ansi_bred),
        ("base.green", p.ansi_green),
        ("base.green.light", p.ansi_bgreen),
        ("base.blue", p.ansi_blue),
        ("base.blue.light", p.ansi_bblue),
        ("base.yellow", p.ansi_yellow),
        ("base.yellow.light", p.ansi_byellow),
        ("base.magenta", p.ansi_magenta),
        ("base.magenta.light", p.ansi_bmagenta),
        ("base.cyan", p.ansi_cyan),
        ("base.cyan.light", p.ansi_bcyan),
    ];
    pairs.into_iter().map(|(k, v)| (k.to_string(), Value::String(hex(v)))).collect()
}

/// gpui-kit highlight (editor + syntax) section: term colours and the spec's
/// syntax mapping (keyword magenta, function blue, string green, number
/// yellow, comment dim italic).
fn highlight(p: &Palette) -> Value {
    let c = |h: Hsla| json!({ "color": hex(h) });
    json!({
        "editor.background": hex(p.term_bg),
        "editor.foreground": hex(p.term_fg),
        "editor.active_line.background": hex(p.surface_2),
        "editor.line_number": hex(p.term_dim),
        "editor.active_line_number": hex(p.ink),
        "editor.invisible": hex(p.ink_4.alpha(0.4)),
        "editor.gutter.background": hex(p.term_bg),
        "conflict": hex(p.danger),
        "created": hex(p.success),
        "modified": hex(p.warning),
        "hidden": hex(p.ink_4),
        "hint": hex(p.accent_ink),
        "predictive": hex(p.ink_4),
        "warning": hex(p.warning),
        "syntax": {
            "attribute": c(p.ansi_cyan),
            "boolean": c(p.ansi_yellow),
            "comment": { "color": hex(p.term_dim), "font_style": "italic" },
            "comment.doc": { "color": hex(p.term_dim), "font_style": "italic" },
            "constant": c(p.ansi_yellow),
            "constructor": c(p.ansi_blue),
            "embedded": c(p.term_fg),
            "emphasis": { "font_style": "italic" },
            "emphasis.strong": { "font_weight": 600 },
            "function": c(p.ansi_blue),
            "keyword": c(p.ansi_magenta),
            "link_text": c(p.accent_ink),
            "link_uri": c(p.accent_ink),
            "number": c(p.ansi_yellow),
            "property": c(p.term_fg),
            "string": c(p.ansi_green),
            "string.escape": c(p.ansi_cyan),
            "string.regex": c(p.ansi_cyan),
            "string.special": c(p.ansi_green),
            "string.special.symbol": c(p.ansi_green),
            "tag": c(p.ansi_blue),
            "text.code.span": c(p.ansi_green),
            "text.literal": c(p.ansi_green),
            "title": { "color": hex(p.ink), "font_weight": 600 },
            "type": c(p.ansi_cyan),
            "variable": c(p.term_fg),
            "variable.special": c(p.ansi_magenta)
        }
    })
}

fn theme(kind: ThemeKind) -> Value {
    let p = match kind {
        ThemeKind::Light => light(),
        ThemeKind::Dark => dark(),
    };
    json!({
        "is_default": true,
        "name": match kind { ThemeKind::Light => KIT_THEME_LIGHT, ThemeKind::Dark => KIT_THEME_DARK },
        "mode": match kind { ThemeKind::Light => "light", ThemeKind::Dark => "dark" },
        "font.family": scale::FONT_UI,
        "font.size": scale::FS_13,
        "mono_font.family": scale::FONT_MONO,
        "mono_font.size": scale::FS_12,
        "radius": scale::R_SM as u32,
        "radius.lg": scale::R_LG as u32,
        "shadow": true,
        "colors": Value::Object(colors(&p, kind)),
        "highlight": highlight(&p),
    })
}

/// The generated gpui-kit `ThemeSet` as a JSON value.
pub fn theme_set_value() -> Value {
    json!({
        "$schema": "https://github.com/longbridge/gpui-kit/raw/refs/heads/main/.theme-schema.json",
        "name": "Agentic UI",
        "author": "aui-tokens (generated from design/tokens/tokens.json)",
        "themes": [theme(ThemeKind::Light), theme(ThemeKind::Dark)]
    })
}

/// The generated gpui-kit `ThemeSet` as pretty JSON.
pub fn theme_set_json() -> String {
    serde_json::to_string_pretty(&theme_set_value()).expect("theme set serialises")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::ThemeSet;

    #[test]
    fn hex_formats_alpha_only_when_needed() {
        assert_eq!(hex(gpui::rgb(0xF5F6F9).into()), "#f5f6f9");
        assert_eq!(hex(gpui::rgba(0x565CC44D).into()), "#565cc44d");
    }

    #[test]
    fn theme_set_parses_with_gpui_kit() {
        let json = theme_set_json();
        let set: ThemeSet = serde_json::from_str(&json).expect("gpui-kit parses the theme set");
        assert_eq!(set.themes.len(), 2);
        assert_eq!(set.themes[0].name.as_ref(), KIT_THEME_LIGHT);
        assert_eq!(set.themes[1].name.as_ref(), KIT_THEME_DARK);
        assert!(set.themes[1].mode.is_dark());
        assert_eq!(set.themes[0].font_family.as_deref(), Some("Geist"));
        assert_eq!(set.themes[0].colors.primary.as_deref(), Some("#565cc4"));
        assert!(set.themes[0].highlight.is_some());
    }
}
