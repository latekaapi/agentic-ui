//! # aui-tokens
//!
//! Design tokens for the Agentic UI (`aui`) library. Everything visual that a
//! component needs comes from here: the two colour palettes, the type scale,
//! spacing, radii, control heights, elevation, motion durations, easings and
//! springs, the bundled Inter / Geist Mono fonts, and the gpui-kit theme that
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
pub use styled::{active_density, active_text_size, AuiStyled, TextRole, TextSize, TypeSpec};
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

    /// The overlay ink — white in dark, black in light — at `alpha`: what
    /// the hover/selected/pressed tints and the scrollbar thumb are made of,
    /// for the few marks that need the same ink at their own strength (a
    /// quote rule, a drop ring, a highlighted code band).
    pub fn overlay_ink(&self, alpha: f32) -> gpui::Hsla {
        if self.is_dark() {
            gpui::white().alpha(alpha)
        } else {
            gpui::black().alpha(alpha)
        }
    }

    /// The hover tint: white at 6 % in dark (lightens), black at 5 % in
    /// light (darkens). Surface-relative: paint it over whatever step it
    /// sits on. The ladder is hover < selected < pressed in both themes
    /// (dark 6 / 12 / 16 %, light 5 / 8 / 10 %; docs/83 §2.1, §9).
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

    /// The surface step `offset` stairs above `step`, capped at 8. A popover
    /// over a step-2 row opens on `elevated(2, 3)` = step 5.
    pub fn elevated(step: u8, offset: u8) -> u8 {
        step.clamp(1, 8).saturating_add(offset).min(8)
    }

    /// The level every floating surface paints — menus, pills, popovers,
    /// toasts, the peek: `surface-5` (= `overlay`), whatever it opens over.
    pub const FLOATING_LEVEL: u8 = 5;

    /// The level of a dialog, the ⌘K palette and anything over a scrim:
    /// `surface-2` (docs/83 §2.1 "dialogs"), told apart from the dimmed
    /// ground by its `line-strong` border and the dialog shadow.
    pub const DIALOG_LEVEL: u8 = 2;

    /// The one shadow of anything that floats over content — dropdowns,
    /// popovers, pills, toasts, hover cards, the peek: one soft drop
    /// (`0 8px 24px -8px`, black 50 % dark / 16 % light; docs/83 §2.3).
    /// It carries no edge: a floating surface draws its own 1 px
    /// `line-strong` border beside this shadow.
    pub fn floating_shadow(&self) -> Vec<gpui::BoxShadow> {
        layers_to_shadows(self.shadow_float)
    }

    /// The dialog's shadow: one deep drop (`0 24px 64px -16px`, black 60 %
    /// dark / 22 % light). Like [`Palette::floating_shadow`] it carries no
    /// edge; the dialog draws a 1 px `line-strong` border.
    pub fn dialog_shadow(&self) -> Vec<gpui::BoxShadow> {
        layers_to_shadows(self.shadow_dialog)
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
    fn two_shadows_each_one_drop_and_no_edge() {
        // docs/83 §2.3: eleven tokens became two. Each is a single outer
        // drop; neither carries an inset ring or a top highlight (the
        // floating surface draws a 1 px `line-strong` border instead).
        let d = dark();
        let l = light();
        for (pal, float_a, dialog_a) in [(d, 0.50, 0.60), (l, 0.16, 0.22)] {
            assert_eq!(pal.shadow_float.len(), 1);
            assert_eq!(pal.shadow_dialog.len(), 1);
            let f = pal.shadow_float[0];
            assert!(!f.inset && (f.x, f.y, f.blur, f.spread) == (0.0, 8.0, 24.0, -8.0) && (f.color.a - float_a).abs() < 1e-6);
            let g = pal.shadow_dialog[0];
            assert!(!g.inset && (g.x, g.y, g.blur, g.spread) == (0.0, 24.0, 64.0, -16.0) && (g.color.a - dialog_a).abs() < 1e-6);
            assert!(pal.floating_shadow().iter().chain(pal.dialog_shadow().iter()).all(|s| !s.inset));
        }
    }

    #[test]
    fn scale_matches_design() {
        assert_eq!(scale::H_MD, 28.0);
        assert_eq!(scale::R_LG, 10.0);
        assert_eq!(scale::D_ENTER.as_millis(), 220);
        assert_eq!(scale::E_OUT, [0.16, 1.0, 0.3, 1.0]);
        assert_eq!(scale::FONT_UI, "Inter");
        assert_eq!(scale::FONT_MONO, "Geist Mono");
        // docs/83 §2.5: the shared row is 32 (compact 28), bars 44.
        assert_eq!(scale::CTL_DEFAULT, 32.0);
        assert_eq!(scale::CTL_COMPACT, 28.0);
        assert_eq!(scale::ROW_2_DEFAULT, 52.0);
        assert_eq!(scale::ROW_2_COMPACT, 44.0);
        assert_eq!(scale::BAR_DEFAULT, 44.0);
        assert_eq!(scale::ICON_DEFAULT, 16.0);
        // docs/83 §2.4 ladder (comfortable) and its `-c` variants (the
        // pre-2.10 sizes, Compact density).
        assert_eq!((scale::T_BODY_FS, scale::T_BODY_LH), (14.0, 20.0));
        assert_eq!((scale::T_BODY_FS_C, scale::T_BODY_LH_C), (13.0, 20.0));
        assert_eq!((scale::T_TITLE_FS, scale::T_TITLE_LH), (18.0, 26.0));
        assert_eq!((scale::T_CAPTION_FS, scale::T_CAPTION_LH), (13.0, 18.0));
        assert_eq!((scale::T_CAPTION_FS_C, scale::T_CAPTION_LH_C), (12.0, 16.0));
        assert_eq!((scale::T_MICRO_FS, scale::T_MICRO_FS_C), (12.0, 11.0));
        assert_eq!(scale::T_DISPLAY_FS, 28.0);
        assert_eq!(scale::T_DISPLAY_LH, 34.0);
        assert_eq!((scale::T_MONO_FS, scale::T_MONO_FS_C), (13.0, 13.0));
        // docs/83 §2.5: items (rows, buttons, inputs, chips) 8, containers 10.
        assert_eq!(scale::SHAPE_CONTAINER, 10.0);
        assert_eq!(scale::SHAPE_ITEM, 8.0);
        assert_eq!(scale::OV_HOVER_DARK, 0.06);
        assert_eq!(scale::OV_PRESSED_LIGHT, 0.10);
        // docs/83 §2.4/§2.5 names.
        assert_eq!((scale::T_READ_FS, scale::T_READ_LH), (16.0, 26.0));
        assert_eq!((scale::T_READ_MONO_FS, scale::T_READ_MONO_LH), (13.5, 22.0));
        assert_eq!((scale::ROW_GAP, scale::LIST_INSET, scale::R_8), (2.0, 8.0, 8.0));
        // docs/83 §2.5 / §5: the transcript column and its card heads (task 2.6).
        assert_eq!((scale::TRANSCRIPT_MAX, scale::TRANSCRIPT_INSET, scale::CARD_HEAD), (760.0, 32.0, 36.0));
        // docs/83 §6: the right pane's sub-header and its two list rows (task 2.7).
        assert_eq!((scale::SUB_HEADER, scale::TREE_ROW, scale::CHANGE_ROW), (36.0, 28.0, 32.0));
        // docs/83 §7: floating surfaces and controls (task 2.8).
        assert_eq!((scale::MENU_ROW, scale::MENU_PAD, scale::MENU_SEP_INSET, scale::R_6), (28.0, 4.0, 8.0, 6.0));
        assert_eq!((scale::PALETTE_W, scale::PALETTE_ROW, scale::PALETTE_TOP, scale::SCRIM), (640.0, 36.0, 0.18, 0.5));
        assert_eq!((scale::BTN_H, scale::BTN_H_DENSE, scale::FIELD_H), (32.0, 28.0, 32.0));
        // docs/83 §2.4: Markdown headings in the reading scale.
        assert_eq!((scale::T_H1_FS, scale::T_H1_LH, scale::T_H2_FS, scale::T_H2_LH, scale::T_H3_FS, scale::T_H3_LH), (22.0, 30.0, 19.0, 28.0, 17.0, 26.0));
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
        // White over dark, black over light.
        assert!(d.hover_overlay().l > 0.99);
        assert!(l.hover_overlay().l < 0.01);
        // docs/83 §2.1: hover < selected < pressed, in both themes.
        assert_eq!((scale::OV_HOVER_DARK, scale::OV_SELECTED_DARK, scale::OV_PRESSED_DARK), (0.06, 0.12, 0.16));
        assert_eq!((scale::OV_HOVER_LIGHT, scale::OV_SELECTED_LIGHT, scale::OV_PRESSED_LIGHT), (0.05, 0.08, 0.10));
        // Lines are solid colours, not alphas (docs/83 §2.1).
        for pal in [d, l] {
            for c in [pal.line, pal.line_region, pal.line_strong] {
                assert!((c.a - 1.0).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn elevated_caps_at_step_8() {
        assert_eq!(Palette::elevated(2, 3), 5);
        assert_eq!(Palette::elevated(7, 4), 8);
        assert_eq!(Palette::elevated(8, 0), 8);
    }

    #[test]
    fn surfaces_are_the_flat_palette() {
        let hex = |c: gpui::Hsla| {
            let r: gpui::Rgba = c.into();
            format!("#{:02X}{:02X}{:02X}", (r.r * 255.0).round() as u8, (r.g * 255.0).round() as u8, (r.b * 255.0).round() as u8)
        };
        // docs/83 §2.1 (dark, measured from the reference).
        let d = dark();
        let want = ["#191919", "#1F1F1F", "#252525", "#2A2A2A", "#303030", "#3A3A3A", "#3A3A3A", "#3A3A3A"];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(hex(d.surface((i + 1) as u8)), *w);
        }
        assert_eq!((hex(d.fg), hex(d.fg_muted), hex(d.ink_4)), ("#EFEFEF".into(), "#9C9C9C".into(), "#6E6E6E".into()));
        assert_eq!((hex(d.well), hex(d.overlay), hex(d.sidebar)), ("#141414".into(), "#303030".into(), "#232323".into()));
        assert_eq!((hex(d.line), hex(d.line_region), hex(d.line_strong)), ("#2A2A2A".into(), "#353535".into(), "#404040".into()));
        assert_eq!(hex(d.accent_ring), "#4C82E0");
        assert_eq!(hex(d.surface(Palette::FLOATING_LEVEL)), "#303030");
        assert_eq!(hex(d.surface(Palette::DIALOG_LEVEL)), "#1F1F1F");
        // docs/83 §2.2 (light, derived).
        let l = light();
        let want = ["#FFFFFF", "#FAFAFA", "#F3F3F3", "#EEEEEE", "#FFFFFF", "#E8E8E8", "#E8E8E8", "#E8E8E8"];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(hex(l.surface((i + 1) as u8)), *w);
        }
        assert_eq!((hex(l.fg), hex(l.fg_muted), hex(l.ink_4)), ("#1A1A1A".into(), "#666666".into(), "#9A9A9A".into()));
        assert_eq!((hex(l.well), hex(l.overlay), hex(l.sidebar)), ("#F7F7F7".into(), "#FFFFFF".into(), "#F6F6F6".into()));
        assert_eq!((hex(l.line), hex(l.line_region), hex(l.line_strong)), ("#EBEBEB".into(), "#E2E2E2".into(), "#D4D4D4".into()));
        assert_eq!(hex(l.accent_ring), "#2F6FDB");
    }
}
