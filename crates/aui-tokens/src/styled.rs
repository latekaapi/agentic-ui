//! The type ladder: one mechanism for every text size (docs/83 §2.4).
//!
//! Every size in aui and its hosts resolves through a [`TextRole`]. A role
//! fixes size, whole-pixel line height and weight for the active
//! [`Density`] and [`TextSize`]; the face is the role's own (UI or mono) or
//! the caller's via [`AuiStyled::ui`] / [`AuiStyled::mono`]. There is no
//! px-size entry point and no global multiplier: the window rem follows the
//! active Body size only so gpui-kit's own widgets (inputs, editors) match.

use std::sync::atomic::{AtomicU8, Ordering};

use gpui::{px, App, FontWeight, Pixels, Styled};

use crate::density::Density;
use crate::generated::scale;
use crate::theme::ActiveAui;

/// The text size preference (Settings › Appearance › Text size).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextSize {
    /// The ladder as designed.
    #[default]
    Default,
    /// Every role one step up the ladder ([`TextRole::larger`]).
    Large,
}

impl TextSize {
    /// Human-readable name.
    pub fn label(self) -> &'static str {
        match self {
            TextSize::Default => "Default",
            TextSize::Large => "Large",
        }
    }

    /// The value stored in settings (`"default"` / `"large"`).
    pub fn key(self) -> &'static str {
        match self {
            TextSize::Default => "default",
            TextSize::Large => "large",
        }
    }

    /// Parses [`TextSize::key`].
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "default" => Some(TextSize::Default),
            "large" => Some(TextSize::Large),
            _ => None,
        }
    }
}

/// The active density and text size, mirrored from [`crate::AuiTheme`] so a
/// role resolves in code that holds no `App` (render helpers, measurement).
/// Written only by `AuiTheme::init` / `set_density` / `set_text_size`, on the
/// main thread; bit 0 = compact, bit 1 = large.
static ACTIVE_TYPE: AtomicU8 = AtomicU8::new(0);

pub(crate) fn set_active_type(density: Density, size: TextSize) {
    let bits = u8::from(density == Density::Compact) | (u8::from(size == TextSize::Large) << 1);
    ACTIVE_TYPE.store(bits, Ordering::Relaxed);
}

/// The density the type ladder and [`crate::Metrics::active`] resolve at.
pub fn active_density() -> Density {
    if ACTIVE_TYPE.load(Ordering::Relaxed) & 1 == 1 {
        Density::Compact
    } else {
        Density::Comfortable
    }
}

/// The text size the type ladder resolves at.
pub fn active_text_size() -> TextSize {
    if ACTIVE_TYPE.load(Ordering::Relaxed) & 2 == 2 {
        TextSize::Large
    } else {
        TextSize::Default
    }
}

/// The named text roles of the design (docs/83 §2.4). Each role fixes
/// family, size, whole-pixel line height and weight; colour is applied
/// separately from the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// Display: 500, 28/34.
    Display,
    /// Page and dialog titles: 500, 18/26 (compact 16/22).
    Title,
    /// Region titles (header title, pane title): 500, 14/20.
    Subtitle,
    /// All UI text — rows, menus, buttons, settings: 400, 14/20 (compact 13/20).
    Body,
    /// Bylines, descriptions, group labels, footers: 400, 13/18 (compact 12/16).
    Caption,
    /// Counts, kbd, timestamps: 400, 12/16 (compact 11/14).
    Micro,
    /// Code and terminals: Geist Mono 400, 13/20.
    Mono,
    /// Transcript prose, the user bubble, composer text: 400, 16/26.
    Read,
    /// Code in the transcript: Geist Mono 400, 13.5/22 (compact 13/20).
    ReadMono,
    /// Markdown h1 in reading context: 600, 22/30.
    H1,
    /// Markdown h2: 600, 19/28.
    H2,
    /// Markdown h3: 600, 17/26.
    H3,
}

/// A resolved role: what [`TextRole::spec`] returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeSpec {
    /// Font family.
    pub family: &'static str,
    /// Size in px.
    pub size: f32,
    /// Line height in whole px.
    pub line_height: f32,
    /// Weight.
    pub weight: FontWeight,
}

impl TextRole {
    /// Every role, largest first (for the gallery and the histogram test).
    pub const ALL: [TextRole; 12] = [
        TextRole::Display,
        TextRole::H1,
        TextRole::H2,
        TextRole::Title,
        TextRole::H3,
        TextRole::Read,
        TextRole::Subtitle,
        TextRole::Body,
        TextRole::ReadMono,
        TextRole::Caption,
        TextRole::Mono,
        TextRole::Micro,
    ];

    /// The role whose size and line height this role borrows at
    /// [`TextSize::Large`]: one step up the ladder. Display and H1 are the
    /// top and stay.
    pub fn larger(self) -> TextRole {
        match self {
            TextRole::Micro => TextRole::Caption,
            TextRole::Caption => TextRole::Body,
            TextRole::Body | TextRole::Subtitle => TextRole::Read,
            TextRole::Mono => TextRole::ReadMono,
            TextRole::ReadMono => TextRole::Body,
            TextRole::Read => TextRole::H3,
            TextRole::H3 => TextRole::Title,
            TextRole::Title => TextRole::H2,
            TextRole::H2 => TextRole::H1,
            TextRole::H1 => TextRole::H1,
            TextRole::Display => TextRole::Display,
        }
    }

    /// The role at `density`, default text size.
    fn base(self, density: Density) -> TypeSpec {
        let c = density == Density::Compact;
        let pick = |fs: f32, lh: f32, fs_c: f32, lh_c: f32| if c { (fs_c, lh_c) } else { (fs, lh) };
        let (family, (size, line_height), weight) = match self {
            TextRole::Display => (
                scale::FONT_UI,
                pick(scale::T_DISPLAY_FS, scale::T_DISPLAY_LH, scale::T_DISPLAY_FS_C, scale::T_DISPLAY_LH_C),
                FontWeight::MEDIUM,
            ),
            TextRole::Title => (
                scale::FONT_UI,
                pick(scale::T_TITLE_FS, scale::T_TITLE_LH, scale::T_TITLE_FS_C, scale::T_TITLE_LH_C),
                FontWeight::MEDIUM,
            ),
            TextRole::Subtitle => (
                scale::FONT_UI,
                pick(scale::T_SUBTITLE_FS, scale::T_SUBTITLE_LH, scale::T_SUBTITLE_FS_C, scale::T_SUBTITLE_LH_C),
                FontWeight::MEDIUM,
            ),
            TextRole::Body => (
                scale::FONT_UI,
                pick(scale::T_BODY_FS, scale::T_BODY_LH, scale::T_BODY_FS_C, scale::T_BODY_LH_C),
                FontWeight::NORMAL,
            ),
            TextRole::Caption => (
                scale::FONT_UI,
                pick(scale::T_CAPTION_FS, scale::T_CAPTION_LH, scale::T_CAPTION_FS_C, scale::T_CAPTION_LH_C),
                FontWeight::NORMAL,
            ),
            TextRole::Micro => (
                scale::FONT_UI,
                pick(scale::T_MICRO_FS, scale::T_MICRO_LH, scale::T_MICRO_FS_C, scale::T_MICRO_LH_C),
                FontWeight::NORMAL,
            ),
            TextRole::Mono => (
                scale::FONT_MONO,
                pick(scale::T_MONO_FS, scale::T_MONO_LH, scale::T_MONO_FS_C, scale::T_MONO_LH_C),
                FontWeight::NORMAL,
            ),
            TextRole::Read => (
                scale::FONT_UI,
                pick(scale::T_READ_FS, scale::T_READ_LH, scale::T_READ_FS_C, scale::T_READ_LH_C),
                FontWeight::NORMAL,
            ),
            TextRole::ReadMono => (
                scale::FONT_MONO,
                pick(scale::T_READ_MONO_FS, scale::T_READ_MONO_LH, scale::T_READ_MONO_FS_C, scale::T_READ_MONO_LH_C),
                FontWeight::NORMAL,
            ),
            TextRole::H1 => (
                scale::FONT_UI,
                pick(scale::T_H1_FS, scale::T_H1_LH, scale::T_H1_FS_C, scale::T_H1_LH_C),
                FontWeight::SEMIBOLD,
            ),
            TextRole::H2 => (
                scale::FONT_UI,
                pick(scale::T_H2_FS, scale::T_H2_LH, scale::T_H2_FS_C, scale::T_H2_LH_C),
                FontWeight::SEMIBOLD,
            ),
            TextRole::H3 => (
                scale::FONT_UI,
                pick(scale::T_H3_FS, scale::T_H3_LH, scale::T_H3_FS_C, scale::T_H3_LH_C),
                FontWeight::SEMIBOLD,
            ),
        };
        TypeSpec { family, size, line_height, weight }
    }

    /// The role at `density` and `text_size`. Large keeps the role's face
    /// and weight and takes the size of [`TextRole::larger`], with the
    /// taller of the two line heights.
    pub fn spec_in(self, density: Density, text_size: TextSize) -> TypeSpec {
        let own = self.base(density);
        match text_size {
            TextSize::Default => own,
            TextSize::Large => {
                let up = self.larger().base(density);
                TypeSpec { size: up.size, line_height: own.line_height.max(up.line_height), ..own }
            }
        }
    }

    /// The role at the active density and text size.
    pub fn spec(self) -> TypeSpec {
        self.spec_in(active_density(), active_text_size())
    }

    /// The role's size at the active density and text size.
    pub fn size(self) -> Pixels {
        px(self.spec().size)
    }

    /// The role's line height at the active density and text size.
    pub fn line_height(self) -> Pixels {
        px(self.spec().line_height)
    }
}

/// Fluent helpers for applying the type ladder. Every one takes a
/// [`TextRole`]; there is no pixel-size entry point.
pub trait AuiStyled: Styled + Sized {
    /// The role's family, size, line height and weight at the active density.
    fn text_role(self, role: TextRole) -> Self {
        let s = role.spec();
        self.font_family(s.family).text_size(px(s.size)).line_height(px(s.line_height)).font_weight(s.weight)
    }

    /// [`AuiStyled::text_role`], reading the density from `cx`'s theme.
    /// Named `role_in` (rather than `role`) because gpui's interactive
    /// elements already have an accessibility `role` method and this trait is
    /// implemented for all of them.
    fn role_in(self, role: TextRole, cx: &App) -> Self {
        let theme = cx.aui();
        let s = role.spec_in(theme.density, theme.text_size);
        self.font_family(s.family).text_size(px(s.size)).line_height(px(s.line_height)).font_weight(s.weight)
    }

    /// The UI face at the role's size and line height; the weight is the
    /// caller's.
    fn ui(self, role: TextRole) -> Self {
        let s = role.spec();
        self.font_family(scale::FONT_UI).text_size(px(s.size)).line_height(px(s.line_height))
    }

    /// The mono face at the role's size and line height; the weight is the
    /// caller's.
    fn mono(self, role: TextRole) -> Self {
        let s = role.spec();
        self.font_family(scale::FONT_MONO).text_size(px(s.size)).line_height(px(s.line_height))
    }

    /// The role's size alone (face and line height inherited).
    fn role_size(self, role: TextRole) -> Self {
        self.text_size(role.size())
    }

    /// Weight 500.
    fn medium(self) -> Self {
        self.font_weight(FontWeight::MEDIUM)
    }

    /// Weight 600.
    fn semibold(self) -> Self {
        self.font_weight(FontWeight::SEMIBOLD)
    }
}

impl<T: Styled> AuiStyled for T {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizes(density: Density, text_size: TextSize) -> Vec<f32> {
        let mut v: Vec<f32> = TextRole::ALL.iter().map(|r| r.spec_in(density, text_size).size).collect();
        v.sort_by(f32::total_cmp);
        v.dedup();
        v
    }

    #[test]
    fn ladder_matches_docs_83() {
        let c = Density::Comfortable;
        let d = TextSize::Default;
        let at = |r: TextRole| {
            let s = r.spec_in(c, d);
            (s.size, s.line_height)
        };
        assert_eq!(at(TextRole::Display), (28.0, 34.0));
        assert_eq!(at(TextRole::Title), (18.0, 26.0));
        assert_eq!(at(TextRole::Subtitle), (14.0, 20.0));
        assert_eq!(at(TextRole::Body), (14.0, 20.0));
        assert_eq!(at(TextRole::Caption), (13.0, 18.0));
        assert_eq!(at(TextRole::Micro), (12.0, 16.0));
        assert_eq!(at(TextRole::Mono), (13.0, 20.0));
        assert_eq!(at(TextRole::Read), (16.0, 26.0));
        assert_eq!(at(TextRole::ReadMono), (13.5, 22.0));
        assert_eq!((at(TextRole::H1), at(TextRole::H2), at(TextRole::H3)), ((22.0, 30.0), (19.0, 28.0), (17.0, 26.0)));
        // Rule 6: no semibold in chrome; headings in reading context only.
        assert_eq!(TextRole::Title.spec_in(c, d).weight, FontWeight::MEDIUM);
        assert_eq!(TextRole::Micro.spec_in(c, d).weight, FontWeight::NORMAL);
    }

    #[test]
    fn compact_is_the_type_before_task_2_10() {
        let at = |r: TextRole| r.spec_in(Density::Compact, TextSize::Default).size;
        assert_eq!((at(TextRole::Title), at(TextRole::Body), at(TextRole::Caption), at(TextRole::Micro)), (16.0, 13.0, 12.0, 11.0));
        assert_eq!((at(TextRole::Mono), at(TextRole::Read)), (13.0, 16.0));
    }

    #[test]
    fn at_most_nine_sizes_per_density() {
        // Display is the gallery's; no production site resolves to it, so
        // the app's ladder is the other eleven roles.
        for density in [Density::Comfortable, Density::Compact] {
            let all = sizes(density, TextSize::Default);
            let app: Vec<f32> = all.iter().copied().filter(|s| *s != scale::T_DISPLAY_FS).collect();
            assert!(app.len() <= 9, "{density:?}: {app:?}");
        }
    }

    #[test]
    fn large_is_one_step_up() {
        let l = |r: TextRole| r.spec_in(Density::Comfortable, TextSize::Large);
        assert_eq!(l(TextRole::Body).size, 16.0);
        assert_eq!(l(TextRole::Caption).size, 14.0);
        assert_eq!(l(TextRole::Micro).size, 13.0);
        assert_eq!(l(TextRole::Read).size, 17.0);
        assert_eq!(l(TextRole::Mono).family, scale::FONT_MONO);
        // Every role grows or is already the top.
        for r in TextRole::ALL {
            let d = r.spec_in(Density::Comfortable, TextSize::Default);
            assert!(l(r).size >= d.size && l(r).line_height >= d.line_height, "{r:?}");
        }
        assert_eq!(TextSize::parse(TextSize::Large.key()), Some(TextSize::Large));
    }
}
