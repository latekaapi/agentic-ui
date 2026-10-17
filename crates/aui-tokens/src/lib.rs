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

/// A scrollbar thumb's state, for [`Palette::scrollbar_thumb`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbState {
    /// At rest.
    Rest,
    /// The track is hovered.
    Hover,
    /// Being dragged.
    Active,
}
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

    /// The shadow recipe for FF level `level` (1–8, clamped; 0 = none).
    /// Light is Fluid Functionalism's ladder: a 6 % hairline plus stacked
    /// drops at 6 %. Dark is v3 (docs/31a §0 "v3 overrides"): each FF
    /// level maps to its v3 equivalent — 1, 2, 4 = the edge (nested
    /// surfaces, the tabs indicator), 3 = float (FF's popup level),
    /// 5–8 = dialog.
    pub fn shadow(&self, level: u8) -> Vec<gpui::BoxShadow> {
        let layers = match level {
            0 => &[][..],
            1 => self.shadow_1,
            2 => self.shadow_2,
            3 => self.shadow_3,
            4 => self.shadow_4,
            5 => self.shadow_5,
            6 => self.shadow_6,
            7 => self.shadow_7,
            _ => self.shadow_8,
        };
        layers_to_shadows(layers)
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

    /// The hover tint (FF `--hover`): white at 6 % in dark (lightens), black
    /// at 4 % in light (darkens). Surface-relative: paint it over whatever
    /// step it sits on. Selected / pressed are FF's `--active` (10 % / 7 %).
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

    /// The scrollbar thumb (FF `rgb(var(--overlay) / a)`, `scroll-area.tsx`):
    /// the overlay colour — white in dark, black in light — at 8 % resting,
    /// 12 % while the track is hovered, 16 % while dragged.
    pub fn scrollbar_thumb(&self, state: ThumbState) -> gpui::Hsla {
        let alpha = match state {
            ThumbState::Rest => 0.08,
            ThumbState::Hover => 0.12,
            ThumbState::Active => 0.16,
        };
        if self.is_dark() {
            gpui::white().alpha(alpha)
        } else {
            gpui::black().alpha(alpha)
        }
    }

    /// The hairline (FF `--border`): foreground at 12 %, both themes.
    pub fn ring(&self) -> gpui::Hsla {
        self.line
    }

    /// The hovered hairline: foreground at 18 %.
    pub fn ring_hover(&self) -> gpui::Hsla {
        self.line_strong
    }

    /// The surface step `offset` stairs above `step`, capped at 8. A popover
    /// over a step-2 row opens on `elevated(2, 3)` = step 5.
    pub fn elevated(step: u8, offset: u8) -> u8 {
        step.clamp(1, 8).saturating_add(offset).min(8)
    }

    /// The level every floating surface paints — menus, pills, popovers,
    /// toasts, the peek: v3's **Floating** (#2C2C32), whatever it opens
    /// over, so a popup reads as one three layers down without stacked
    /// drops. In light, steps 3–8 are all white, so this changes nothing.
    pub const FLOATING_LEVEL: u8 = 5;

    /// The level of a dialog, the ⌘K palette and anything over a scrim:
    /// v3's **Dialog** (#35353C).
    pub const DIALOG_LEVEL: u8 = 6;

    /// The one edge every v3 surface carries (dark): a 7 % white top
    /// highlight and a 6 % white ring, both inset. Light keeps FF's 6 %
    /// hairline until light is designed.
    pub fn edge(&self) -> Vec<gpui::BoxShadow> {
        layers_to_shadows(self.shadow_edge)
    }

    /// The shadow of anything that floats over content — dropdowns,
    /// popovers, pills, toasts, hover cards: the edge plus one soft drop
    /// (dark: `0 8px 24px -10px` at 40 %), constant whatever depth the
    /// popup opens at. Light keeps FF's `shadow-3`.
    pub fn floating_shadow(&self) -> Vec<gpui::BoxShadow> {
        layers_to_shadows(self.shadow_float)
    }

    /// The dialog's shadow: the edge plus one deep drop (dark:
    /// `0 24px 56px -16px` at 50 %). Light keeps FF's `shadow-5`.
    pub fn dialog_shadow(&self) -> Vec<gpui::BoxShadow> {
        layers_to_shadows(self.shadow_dialog)
    }

    /// The shadow of a surface at `step` that sits *in* another (not
    /// floating). Dark (v3): nested surfaces cast nothing — every level
    /// above the frame carries just [`Palette::edge`], the frame (step 1)
    /// nothing. Light: FF's 1:1 pairing, `shadow-N` for `bg-surface-N`.
    pub fn elevation_shadow(&self, step: u8) -> Vec<gpui::BoxShadow> {
        if self.is_dark() {
            if step <= 1 { Vec::new() } else { self.edge() }
        } else {
            self.shadow(step.clamp(1, 8))
        }
    }
}

/// Converts token shadow layers into gpui box shadows.
fn layers_to_shadows(layers: &[ShadowLayer]) -> Vec<gpui::BoxShadow> {
    layers
        .iter()
        .map(|l| gpui::BoxShadow {
            color: l.color.into(),
            offset: gpui::point(gpui::px(l.x), gpui::px(l.y)),
            // gpui's shader treats `blur_radius` as the Gaussian sigma; CSS's
            // blur radius is twice the sigma, so halve it to match the design.
            blur_radius: gpui::px(l.blur / 2.0),
            spread_radius: gpui::px(l.spread),
            inset: l.inset,
        })
        .collect()
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
    fn shadows_follow_the_ff_ladder() {
        // Light: a 6 % hairline plus one stacked drop per level above 1.
        for n in 1..=8u8 {
            assert_eq!(light().shadow(n).len(), n as usize);
        }
        assert_eq!(light().shadow_3[2].y, 3.0);
        assert_eq!(light().shadow_3[2].spread, -1.5);
        // Dark is v3 (canvas "Baaz v3", docs/31a §0): one edge — a 7 % top
        // highlight and a 6 % ring, both inset — and two drops.
        let d = dark();
        let hi = d.shadow_edge[0];
        assert!(hi.inset && hi.y == 1.0 && hi.blur == 0.0 && (hi.color.a - 0.07).abs() < 1e-6);
        let ring = d.shadow_edge[1];
        assert!(ring.inset && ring.y == 0.0 && ring.spread == 1.0 && (ring.color.a - 0.06).abs() < 1e-6);
        let float = d.shadow_float[2];
        assert!(!float.inset && (float.y, float.blur, float.spread) == (8.0, 24.0, -10.0) && (float.color.a - 0.4).abs() < 1e-6);
        let dialog = d.shadow_dialog[2];
        assert!(!dialog.inset && (dialog.y, dialog.blur, dialog.spread) == (24.0, 56.0, -16.0) && (dialog.color.a - 0.5).abs() < 1e-6);
        // FF's levels map onto them: 1, 2, 4 = edge; 3 = float; 5–8 = dialog.
        for (n, recipe) in [(1, d.shadow_edge), (2, d.shadow_edge), (3, d.shadow_float), (4, d.shadow_edge), (5, d.shadow_dialog), (8, d.shadow_dialog)] {
            assert_eq!(d.shadow(n), layers_to_shadows(recipe), "dark shadow-{n}");
        }
        assert_eq!(d.floating_shadow(), layers_to_shadows(d.shadow_float));
        assert_eq!(d.dialog_shadow(), layers_to_shadows(d.shadow_dialog));
        // Nested surfaces cast nothing but the edge; the frame not even that.
        assert!(d.elevation_shadow(1).is_empty());
        for n in 2..=8 {
            assert_eq!(d.elevation_shadow(n), d.edge());
        }
    }

    #[test]
    fn scale_matches_design() {
        assert_eq!(scale::FS_13, 13.0);
        assert_eq!(scale::H_MD, 28.0);
        assert_eq!(scale::R_LG, 10.0);
        assert_eq!(scale::D_ENTER.as_millis(), 220);
        assert_eq!(scale::E_OUT, [0.16, 1.0, 0.3, 1.0]);
        assert_eq!(scale::FONT_UI, "Inter");
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
        // FF's six-role type ladder (body 13/20, compact 12/18).
        assert_eq!(scale::T_BODY_FS, 13.0);
        assert_eq!(scale::T_BODY_LH, 20.0);
        assert_eq!(scale::T_BODY_FS_C, 12.0);
        assert_eq!(scale::T_BODY_LH_C, 18.0);
        assert_eq!((scale::T_TITLE_FS, scale::T_TITLE_LH), (16.0, 22.0));
        assert_eq!((scale::T_CAPTION_FS_C, scale::T_CAPTION_LH_C), (11.0, 14.0));
        assert_eq!(scale::T_DISPLAY_FS, 28.0);
        assert_eq!(scale::T_DISPLAY_LH, 34.0);
        assert_eq!(scale::T_MONO_FS, 13.0);
        assert_eq!(scale::T_MONO_FS_C, 12.0);
        // Shapes: one step tighter than FF Rounded (owner 2026-10-10), still
        // concentric for the 4 px popup inset (10 = 6 + 4).
        assert_eq!(scale::SHAPE_CONTAINER, 10.0);
        assert_eq!(scale::SHAPE_ITEM, 6.0);
        assert_eq!(scale::OV_HOVER_DARK, 0.05);
        assert_eq!(scale::OV_PRESSED_LIGHT, 0.07);
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
        // White over dark (v3: 5 / 9 / 12 %), black over light (FF: 4 % / 7 %).
        assert!(d.hover_overlay().l > 0.99);
        assert!(l.hover_overlay().l < 0.01);
        assert_eq!((scale::OV_HOVER_DARK, scale::OV_SELECTED_DARK, scale::OV_PRESSED_DARK), (0.05, 0.09, 0.12));
        assert_eq!((scale::OV_HOVER_LIGHT, scale::OV_SELECTED_LIGHT), (0.04, 0.07));
        // The line: v3 draws none on a surface edge, only inside one, at
        // white 7 %; light draws its lines at black 7 % (docs/43 §2.8).
        assert_eq!(d.ring(), d.line);
        assert!((d.line.a - 0.07).abs() < 1e-6 && (l.line.a - 0.07).abs() < 1e-6);
    }

    #[test]
    fn elevated_caps_at_step_8() {
        assert_eq!(Palette::elevated(2, 3), 5);
        assert_eq!(Palette::elevated(7, 4), 8);
        assert_eq!(Palette::elevated(8, 0), 8);
    }

    #[test]
    fn light_elevation_shadow_pairs_each_level_with_its_recipe() {
        for p in [light()] {
            for n in 1..=8u8 {
                assert_eq!(p.elevation_shadow(n), p.shadow(n));
            }
        }
    }

    #[test]
    fn surfaces_are_v3_dark_and_d6_light() {
        let hex = |c: gpui::Hsla| {
            let r: gpui::Rgba = c.into();
            format!("#{:02X}{:02X}{:02X}", (r.r * 255.0).round() as u8, (r.g * 255.0).round() as u8, (r.b * 255.0).round() as u8)
        };
        let d = dark();
        // v3: frame, sheet, raised, raised-lit, floating, dialog (7 and 8 clamp to dialog).
        let want = ["#121215", "#1B1B1F", "#232328", "#2A2A30", "#2C2C32", "#35353C", "#35353C", "#35353C"];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(hex(d.surface((i + 1) as u8)), *w);
        }
        assert_eq!((hex(d.fg), hex(d.fg_muted), hex(d.ink_4)), ("#F2F2F4".into(), "#9D9DA6".into(), "#6E6E78".into()));
        assert_eq!((hex(d.well), hex(d.overlay)), ("#151518".into(), "#2C2C32".into()));
        assert_eq!(hex(d.surface(Palette::FLOATING_LEVEL)), "#2C2C32");
        assert_eq!(hex(d.surface(Palette::DIALOG_LEVEL)), "#35353C");
        let l = light();
        // Light, first drawn for D6 (docs/43 §2.8): frame, sheet, raised, raised-lit, floating.
        let want = ["#ECECEF", "#FAFAFB", "#FFFFFF", "#F3F3F5", "#FFFFFF"];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(hex(l.surface((i + 1) as u8)), *w);
        }
        assert_eq!((hex(l.fg), hex(l.fg_muted), hex(l.ink_4)), ("#18181B".into(), "#63636D".into(), "#9A9AA4".into()));
        assert_eq!(hex(l.well), "#F2F2F4");
    }

    #[test]
    fn floating_shadow_is_the_constant_shadow_3() {
        // Dark: v3 maps FF's popup level (3) to the float recipe. Light keeps
        // FF's ladder; its float is the D6 recipe (docs/43 §2.8): ring + one drop.
        let d = dark();
        assert_eq!(d.floating_shadow(), d.shadow(3));
        assert_eq!(light().floating_shadow().len(), 2);
    }
}
