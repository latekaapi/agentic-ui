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
pub use theme::{ActiveAui, AgentState, AuiTheme, Easing, ThemeKind};

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
        assert_eq!(scale::TEXT_SCALE, 1.1);
    }

    #[test]
    fn springs_match_motion_json() {
        assert_eq!(springs::PRESS.stiffness, 500.0);
        assert_eq!(springs::SWAP.mass, 0.55);
        assert_eq!(springs::LAYOUT.damping, 32.0);
        assert_eq!(springs::GENTLE.stiffness, 200.0);
        assert_eq!(durations::SLOW.as_millis(), 280);
    }
}
