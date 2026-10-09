//! # aui-tokens
//!
//! Design tokens for the Agentic UI (`aui`) library. Everything visual that a
//! component needs comes from here: the two colour palettes, the type scale,
//! spacing, radii, control heights, elevation, motion durations, easings and
//! springs, the bundled Geist / Geist Mono fonts, and the gpui-kit theme that
//! makes gpui-kit's own components (Dock, Editor, TextView, Input…) match the
//! design.
//!
//! The source of truth is `design/tokens/tokens.json` and
//! `design/tokens/motion.json`; `build.rs` turns them into typed constants at
//! compile time, so a token that disappears from the design breaks the build
//! instead of silently drifting.
//!
//! ## Usage
//!
//! ```ignore
//! use aui_tokens::{ActiveAui, AuiTheme, ThemeKind};
//!
//! // once, after `gpui_kit::init(cx)`
//! AuiTheme::init(ThemeKind::Dark, cx);
//!
//! // in any render
//! let colors = &cx.aui().colors;
//! div().bg(colors.surface_1).border_color(colors.line)
//! ```
//!
//! Components never hold literal colours, sizes or durations; they read the
//! palette via [`ActiveAui::aui`] and the metrics via [`scale`] and
//! [`Metrics`].

#![warn(missing_docs)]
#![recursion_limit = "256"]

mod generated {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/tokens.rs"));
}

mod density;
mod fonts;
mod kit_theme;
mod styled;
mod theme;

pub use density::{Density, Metrics};
pub use fonts::{load_fonts, FONT_FILES};
pub use generated::{dark, durations, light, scale, springs, Palette, ShadowLayer};
pub use kit_theme::{theme_set_json, theme_set_value, KIT_THEME_DARK, KIT_THEME_LIGHT};
pub use styled::{scaled, AuiStyled, TextRole};
pub use theme::{ActiveAui, AgentState, AuiTheme, Easing, ThemeKind, Tier};

/// Raw JSON of `design/tokens/tokens.json`, embedded for tooling that wants the
/// original values (the gallery's colour card lists them).
pub const TOKENS_JSON: &str = include_str!("../../../design/tokens/tokens.json");
/// Raw JSON of `design/tokens/motion.json`.
pub const MOTION_JSON: &str = include_str!("../../../design/tokens/motion.json");

impl Palette {
    /// The palette for a theme kind.
    pub fn for_kind(kind: ThemeKind) -> Palette {
        match kind {
            ThemeKind::Light => light(),
            ThemeKind::Dark => dark(),
        }
    }

    /// One of the eight project label colours, red through pink in token
    /// order (`label-1` … `label-8`). The index wraps, so `0` and `8` are
    /// both `label-1`: a project record stores its colour as a number and
    /// never checks the range.
    pub fn label(&self, index: u8) -> gpui::Hsla {
        match index % 8 {
            0 => self.label_1,
            1 => self.label_2,
            2 => self.label_3,
            3 => self.label_4,
            4 => self.label_5,
            5 => self.label_6,
            6 => self.label_7,
            _ => self.label_8,
        }
    }

    /// The colour that carries an agent state (running → accent, waiting →
    /// warning, done → success, failed → danger, idle → ink-4).
    pub fn agent_state(&self, state: AgentState) -> gpui::Hsla {
        match state {
            AgentState::Running => self.accent,
            AgentState::Waiting => self.warning,
            AgentState::Done => self.success,
            AgentState::Failed => self.danger,
            AgentState::Idle => self.ink_4,
        }
    }

    /// The 16 ANSI colours in terminal order (black, red, green, yellow, blue,
    /// magenta, cyan, white, then the bright variants).
    pub fn ansi16(&self) -> [gpui::Hsla; 16] {
        [
            self.ansi_black,
            self.ansi_red,
            self.ansi_green,
            self.ansi_yellow,
            self.ansi_blue,
            self.ansi_magenta,
            self.ansi_cyan,
            self.ansi_white,
            self.ansi_bblack,
            self.ansi_bred,
            self.ansi_bgreen,
            self.ansi_byellow,
            self.ansi_bblue,
            self.ansi_bmagenta,
            self.ansi_bcyan,
            self.ansi_bwhite,
        ]
    }

    /// Converts one of the three elevation levels into gpui box shadows.
    pub fn shadow(&self, level: u8) -> Vec<gpui::BoxShadow> {
        let layers = match level {
            0 => &[][..],
            1 => self.shadow_1,
            2 => self.shadow_2,
            _ => self.shadow_3,
        };
        layers
            .iter()
            .map(|l| gpui::BoxShadow {
                color: l.color.into(),
                offset: gpui::point(gpui::px(l.x), gpui::px(l.y)),
                // gpui's shader treats `blur_radius` as the Gaussian sigma; CSS's
                // blur radius is twice the sigma, so halve it to match the design.
                blur_radius: gpui::px(l.blur / 2.0),
                spread_radius: gpui::px(l.spread),
                inset: false,
            })
            .collect()
    }

    /// The pending-dialog attention border: the status colour at 70 % alpha,
    /// as the design rules require (no halo, no glow).
    pub fn attention_border(&self, color: gpui::Hsla) -> gpui::Hsla {
        color.alpha(0.7)
    }

    /// Whether this is the dark palette, derived from the background
    /// lightness (dark `bg.l` is below 0.5, light is above 0.9).
    pub fn is_dark(&self) -> bool {
        self.bg.l < 0.5
    }

    /// One of the eight surface steps (`surface-1` … `surface-8`); the step
    /// is clamped to `1..=8`. Steps are relative to the parent: a card on
    /// step 2 nests its sections on step 3, and so on.
    pub fn surface(&self, step: u8) -> gpui::Hsla {
        match step.clamp(1, 8) {
            1 => self.surface_1,
            2 => self.surface_2,
            3 => self.surface_3,
            4 => self.surface_4,
            5 => self.surface_5,
            6 => self.surface_6,
            7 => self.surface_7,
            _ => self.surface_8,
        }
    }

    /// The hover tint: white at the hover alpha in dark (lightens), black at
    /// the hover alpha in light (darkens). Paint over the step beneath it.
    pub fn hover_overlay(&self) -> gpui::Hsla {
        if self.is_dark() {
            gpui::white().alpha(scale::OV_HOVER_DARK)
        } else {
            gpui::black().alpha(scale::OV_HOVER_LIGHT)
        }
    }

    /// The selected tint, same rule as [`Palette::hover_overlay`].
    pub fn selected_overlay(&self) -> gpui::Hsla {
        if self.is_dark() {
            gpui::white().alpha(scale::OV_SELECTED_DARK)
        } else {
            gpui::black().alpha(scale::OV_SELECTED_LIGHT)
        }
    }

    /// The pressed tint, same rule as [`Palette::hover_overlay`].
    pub fn pressed_overlay(&self) -> gpui::Hsla {
        if self.is_dark() {
            gpui::white().alpha(scale::OV_PRESSED_DARK)
        } else {
            gpui::black().alpha(scale::OV_PRESSED_LIGHT)
        }
    }

    /// The elevation ring: white at the ring alpha in dark, `line` in light.
    pub fn ring(&self) -> gpui::Hsla {
        if self.is_dark() {
            gpui::white().alpha(scale::RING_DARK)
        } else {
            self.line
        }
    }

    /// The hovered elevation ring: white at the hover-ring alpha in dark,
    /// `line` in light.
    pub fn ring_hover(&self) -> gpui::Hsla {
        if self.is_dark() {
            gpui::white().alpha(scale::RING_HOVER_DARK)
        } else {
            self.line
        }
    }

    /// The surface step `offset` stairs above `step`, capped at 8. A popover
    /// over a step-2 row opens on `elevated(2, 3)` = step 5.
    pub fn elevated(step: u8, offset: u8) -> u8 {
        step.clamp(1, 8).saturating_add(offset).min(8)
    }

    /// The shadow for a surface step: in dark one ring layer (spread 1 at
    /// the ring alpha) plus `shadow-2` for steps 5 and up; in light
    /// `shadow-1` for steps 3–4, `shadow-2` for 5–6, `shadow-3` for 7–8
    /// (steps 1–2 sit flat).
    pub fn elevation_shadow(&self, step: u8) -> Vec<gpui::BoxShadow> {
        let step = step.clamp(1, 8);
        if self.is_dark() {
            let mut out = vec![gpui::BoxShadow {
                color: self.ring(),
                offset: gpui::point(gpui::px(0.0), gpui::px(0.0)),
                blur_radius: gpui::px(0.0),
                spread_radius: gpui::px(1.0),
                inset: false,
            }];
            if step >= 5 {
                out.extend(self.shadow(2));
            }
            out
        } else {
            match step {
                1 | 2 => Vec::new(),
                3 | 4 => self.shadow(1),
                5 | 6 => self.shadow(2),
                _ => self.shadow(3),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_have_every_colour_in_both_themes() {
        let l = light();
        let d = dark();
        for name in Palette::COLOR_NAMES {
            assert!(l.color(name).is_some(), "light missing {name}");
            assert!(d.color(name).is_some(), "dark missing {name}");
        }
        assert!(Palette::COLOR_NAMES.contains(&"accent"));
        assert!(Palette::COLOR_NAMES.contains(&"ansi-bwhite"));
    }

    #[test]
    fn label_ramp_has_eight_colours_in_each_theme() {
        for theme in [ThemeKind::Light, ThemeKind::Dark] {
            let palette = Palette::for_kind(theme);
            let labels: Vec<&&str> = Palette::COLOR_NAMES.iter().filter(|n| n.starts_with("label-")).collect();
            assert_eq!(labels.len(), 8, "{theme:?} has {} label names", labels.len());
            for i in 1..=8 {
                let name = format!("label-{i}");
                assert!(Palette::COLOR_NAMES.contains(&name.as_str()), "{theme:?} missing {name}");
                assert!(palette.color(&name).is_some(), "{theme:?} cannot look up {name}");
            }
        }
        let light = light();
        assert_eq!(light.label(0), light.label_1);
        assert_eq!(light.label(7), light.label_8);
        assert_eq!(light.label(8), light.label_1);
        assert_eq!(dark().label(15), dark().label_8);
    }

    #[test]
    fn dark_is_dark_and_light_is_light() {
        assert!(light().bg.l > 0.9);
        assert!(dark().bg.l < 0.1);
        assert!(light().ink.l < dark().ink.l);
    }

    #[test]
    fn shadows_parse_all_layers() {
        assert_eq!(light().shadow_1.len(), 1);
        assert_eq!(light().shadow_2.len(), 2);
        assert_eq!(dark().shadow_3.len(), 2);
        assert_eq!(light().shadow_2[0].blur, 14.0);
    }

    #[test]
    fn scale_matches_design() {
        assert_eq!(scale::FS_13, 13.0);
        assert_eq!(scale::H_MD, 28.0);
        assert_eq!(scale::R_LG, 12.0);
        assert_eq!(scale::D_ENTER.as_millis(), 220);
        assert_eq!(scale::E_OUT, [0.16, 1.0, 0.3, 1.0]);
        assert_eq!(scale::FONT_UI, "Geist");
        assert_eq!(scale::FONT_MONO, "Geist Mono");
        assert_eq!(scale::TRACK_CAPS_EM, 0.08);
        // v2 removed the global text multiplier; the shim stays neutral.
        assert_eq!(scale::TEXT_SCALE, 1.0);
        // v2 size ladder.
        assert_eq!(scale::CTL_DEFAULT, 36.0);
        assert_eq!(scale::CTL_COMPACT, 28.0);
        assert_eq!(scale::ROW_2_DEFAULT, 52.0);
        assert_eq!(scale::ROW_2_COMPACT, 44.0);
        assert_eq!(scale::BAR_DEFAULT, 44.0);
        assert_eq!(scale::BAR_COMPACT, 36.0);
        assert_eq!(scale::ICON_DEFAULT, 16.0);
        assert_eq!(scale::ICON_COMPACT, 14.0);
        // v2 whole-pixel type ladder.
        assert_eq!(scale::T_BODY_FS, 14.0);
        assert_eq!(scale::T_BODY_LH, 20.0);
        assert_eq!(scale::T_BODY_FS_C, 13.0);
        assert_eq!(scale::T_DISPLAY_FS, 28.0);
        assert_eq!(scale::T_DISPLAY_LH, 34.0);
        assert_eq!(scale::T_MONO_FS, 13.0);
        assert_eq!(scale::T_MONO_FS_C, 12.0);
        // v2 shape and overlays.
        assert_eq!(scale::SHAPE_CONTAINER, 8.0);
        assert_eq!(scale::SHAPE_ITEM, 6.0);
        assert_eq!(scale::OV_HOVER_DARK, 0.06);
        assert_eq!(scale::OV_PRESSED_LIGHT, 0.09);
        assert_eq!(scale::RING_DARK, 0.08);
        assert_eq!(scale::RING_HOVER_DARK, 0.13);
    }

    #[test]
    fn springs_match_motion_json() {
        assert_eq!(springs::PRESS.stiffness, 500.0);
        assert_eq!(springs::SWAP.mass, 0.55);
        assert_eq!(springs::LAYOUT.damping, 32.0);
        assert_eq!(springs::GENTLE.stiffness, 200.0);
        assert_eq!(durations::SLOW.as_millis(), 280);
        assert_eq!(springs::FAST.stiffness, 6168.0);
        assert_eq!(springs::FAST.damping, 157.1);
        assert_eq!(springs::MODERATE.stiffness, 1542.0);
        assert_eq!(springs::SLOW.damping, 46.1);
        assert_eq!(durations::FAST_EXIT.as_millis(), 60);
        assert_eq!(durations::MODERATE_EXIT.as_millis(), 120);
        assert_eq!(durations::SLOW_EXIT.as_millis(), 160);
        assert_eq!(durations::MODERATE.as_millis(), 160);
    }

    #[test]
    fn surface_ladder_has_eight_steps_in_both_themes() {
        for theme in [ThemeKind::Light, ThemeKind::Dark] {
            let p = Palette::for_kind(theme);
            for step in 1..=8 {
                let name = format!("surface-{step}");
                assert!(p.color(&name).is_some(), "{theme:?} cannot look up {name}");
            }
            assert_eq!(p.surface(1), p.surface_1);
            assert_eq!(p.surface(8), p.surface_8);
        }
        // Clamped, not panicking.
        assert_eq!(light().surface(0), light().surface_1);
        assert_eq!(dark().surface(255), dark().surface_8);
        // Dark knows it is dark, light knows it is light.
        assert!(dark().is_dark());
        assert!(!light().is_dark());
        // The two-colour text rule has names.
        assert_eq!(light().fg, light().ink);
        assert_eq!(light().fg_muted, light().ink_3);
        assert_eq!(dark().fg, dark().ink);
    }

    #[test]
    fn overlays_lighten_dark_and_darken_light() {
        let d = dark();
        let l = light();
        assert!((d.hover_overlay().a - scale::OV_HOVER_DARK).abs() < 1e-6);
        assert!((d.selected_overlay().a - scale::OV_SELECTED_DARK).abs() < 1e-6);
        assert!((d.pressed_overlay().a - scale::OV_PRESSED_DARK).abs() < 1e-6);
        assert!((l.hover_overlay().a - scale::OV_HOVER_LIGHT).abs() < 1e-6);
        assert!((l.pressed_overlay().a - scale::OV_PRESSED_LIGHT).abs() < 1e-6);
        // Dark lightens (white tint), light darkens (black tint).
        assert!(d.hover_overlay().l > 0.9);
        assert!(l.hover_overlay().l < 0.1);
        // Rings: white at alpha in dark, line in light.
        assert_eq!(l.ring(), l.line);
        assert_eq!(l.ring_hover(), l.line);
        assert!((d.ring().a - scale::RING_DARK).abs() < 1e-6);
        assert!((d.ring_hover().a - scale::RING_HOVER_DARK).abs() < 1e-6);
    }

    #[test]
    fn elevated_caps_at_step_8() {
        assert_eq!(Palette::elevated(2, 3), 5);
        assert_eq!(Palette::elevated(7, 4), 8);
        assert_eq!(Palette::elevated(8, 0), 8);
    }

    #[test]
    fn elevation_shadow_follows_the_step_table() {
        let d = dark();
        // Dark: ring only below step 5, ring + shadow-2 at and above.
        assert_eq!(d.elevation_shadow(2).len(), 1);
        assert_eq!(d.elevation_shadow(5).len(), 1 + d.shadow_2.len());
        assert_eq!(d.elevation_shadow(2)[0].spread_radius, gpui::px(1.0));
        // Light: flat, shadow-1, shadow-2, shadow-3.
        assert!(light().elevation_shadow(1).is_empty());
        assert_eq!(light().elevation_shadow(3).len(), light().shadow_1.len());
        assert_eq!(light().elevation_shadow(6).len(), light().shadow_2.len());
        assert_eq!(light().elevation_shadow(8).len(), light().shadow_3.len());
    }
}
