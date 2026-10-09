//! Style helpers that apply the type ladder to gpui elements.

use gpui::{px, relative, rems, App, FontWeight, Styled};

use crate::density::Density;
use crate::generated::scale;
use crate::theme::ActiveAui;

/// The named text roles of the design. Each role fixes family, size,
/// whole-pixel line height and weight; colour is applied separately from the
/// palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextRole {
    /// Display: Geist 700, 28/34 (compact 24/30).
    Display,
    /// Titles: Geist 600, 18/24 (compact 16/22).
    Title,
    /// Subtitles: Geist 600, 15/20 (compact 14/20).
    Subtitle,
    /// Body copy and default UI text: Geist 400, 14/20 (compact 13/20).
    Body,
    /// Secondary text: Geist 400, 12/16 in both densities.
    Caption,
    /// Tiny labels: Geist 500, 11/14 in both densities.
    Micro,
    /// Code and terminals: Geist Mono 400, 13/20 (compact 12/20).
    Mono,
    /// Deprecated v1 default UI text. Resolves to [`TextRole::Body`].
    Ui,
    /// Deprecated v1 emphasised UI text. Resolves to [`TextRole::Body`] at
    /// weight 500.
    UiMedium,
    /// Deprecated v1 secondary UI text. Resolves to [`TextRole::Caption`].
    UiSmall,
    /// Deprecated v1 eyebrow label. Resolves to [`TextRole::Caption`] with
    /// no uppercase and no tracking: callers must stop uppercasing.
    Caps,
    /// Deprecated v1 meta line. Resolves to [`TextRole::Caption`].
    Meta,
    /// Deprecated v1 small mono meta. Resolves to [`TextRole::Mono`] at its
    /// compact size.
    MonoSmall,
    /// Deprecated v1 tag. Resolves to [`TextRole::Micro`] in the mono family.
    Tag,
}

impl TextRole {
    /// `(family, size px, line-height px, weight)` for a density. Line
    /// heights are absolute pixels, so every rendered line lands on a whole
    /// pixel.
    pub fn spec(self, density: Density) -> (&'static str, f32, f32, FontWeight) {
        let compact = density == Density::Compact;
        match self {
            TextRole::Display => {
                if compact {
                    (scale::FONT_UI, scale::T_DISPLAY_FS_C, scale::T_DISPLAY_LH_C, FontWeight::BOLD)
                } else {
                    (scale::FONT_UI, scale::T_DISPLAY_FS, scale::T_DISPLAY_LH, FontWeight::BOLD)
                }
            }
            TextRole::Title => {
                if compact {
                    (scale::FONT_UI, scale::T_TITLE_FS_C, scale::T_TITLE_LH_C, FontWeight::SEMIBOLD)
                } else {
                    (scale::FONT_UI, scale::T_TITLE_FS, scale::T_TITLE_LH, FontWeight::SEMIBOLD)
                }
            }
            TextRole::Subtitle => {
                if compact {
                    (scale::FONT_UI, scale::T_SUBTITLE_FS_C, scale::T_SUBTITLE_LH_C, FontWeight::SEMIBOLD)
                } else {
                    (scale::FONT_UI, scale::T_SUBTITLE_FS, scale::T_SUBTITLE_LH, FontWeight::SEMIBOLD)
                }
            }
            TextRole::Body => {
                if compact {
                    (scale::FONT_UI, scale::T_BODY_FS_C, scale::T_BODY_LH_C, FontWeight::NORMAL)
                } else {
                    (scale::FONT_UI, scale::T_BODY_FS, scale::T_BODY_LH, FontWeight::NORMAL)
                }
            }
            TextRole::Caption => {
                if compact {
                    (scale::FONT_UI, scale::T_CAPTION_FS_C, scale::T_CAPTION_LH_C, FontWeight::NORMAL)
                } else {
                    (scale::FONT_UI, scale::T_CAPTION_FS, scale::T_CAPTION_LH, FontWeight::NORMAL)
                }
            }
            TextRole::Micro => {
                if compact {
                    (scale::FONT_UI, scale::T_MICRO_FS_C, scale::T_MICRO_LH_C, FontWeight::MEDIUM)
                } else {
                    (scale::FONT_UI, scale::T_MICRO_FS, scale::T_MICRO_LH, FontWeight::MEDIUM)
                }
            }
            TextRole::Mono => {
                if compact {
                    (scale::FONT_MONO, scale::T_MONO_FS_C, scale::T_MONO_LH_C, FontWeight::NORMAL)
                } else {
                    (scale::FONT_MONO, scale::T_MONO_FS, scale::T_MONO_LH, FontWeight::NORMAL)
                }
            }
            TextRole::Ui => TextRole::Body.spec(density),
            TextRole::UiMedium => {
                let (family, size, lh, _) = TextRole::Body.spec(density);
                (family, size, lh, FontWeight::MEDIUM)
            }
            TextRole::UiSmall | TextRole::Caps | TextRole::Meta => TextRole::Caption.spec(density),
            TextRole::MonoSmall => TextRole::Mono.spec(Density::Compact),
            TextRole::Tag => {
                let (_, size, lh, _) = TextRole::Micro.spec(density);
                (scale::FONT_MONO, size, lh, FontWeight::MEDIUM)
            }
        }
    }
}

/// Converts a design pixel size into rems against the 14 px body base.
pub fn scaled(size_px: f32) -> gpui::Rems {
    rems(size_px / scale::T_BODY_FS)
}

/// Fluent helpers for applying the type ladder. Sizes are expressed in rems
/// against the 14 px body base.
pub trait AuiStyled: Styled + Sized {
    /// Deprecated v1 pixel text size. Prefer a [`TextRole`] via
    /// [`AuiStyled::text_role`].
    fn text_px(self, size: f32) -> Self {
        self.text_size(scaled(size))
    }

    /// Applies a [`TextRole`] at the standard density (family, size, line
    /// height, weight). Line heights are absolute pixels.
    fn text_role(self, role: TextRole) -> Self {
        self.text_role_in(role, Density::Standard)
    }

    /// Applies a [`TextRole`] at `density` (family, size, line height,
    /// weight). Line heights are absolute pixels.
    fn text_role_in(self, role: TextRole, density: Density) -> Self {
        let (family, size, lh, weight) = role.spec(density);
        self.font_family(family).text_size(scaled(size)).line_height(px(lh)).font_weight(weight)
    }

    /// Applies a [`TextRole`] at the density of the active [`crate::AuiTheme`].
    /// Named `role_in` (rather than `role`) because gpui's interactive
    /// elements already have an accessibility `role` method and this trait is
    /// implemented for all of them.
    fn role_in(self, role: TextRole, cx: &App) -> Self {
        self.text_role_in(role, cx.aui().density)
    }

    /// Deprecated v1 UI face at an arbitrary size. Prefer a [`TextRole`].
    fn ui(self, size: f32) -> Self {
        self.font_family(scale::FONT_UI)
            .text_size(scaled(size))
            .line_height(relative(scale::LH_UI))
    }

    /// Deprecated v1 mono face at an arbitrary size. Prefer [`TextRole::Mono`].
    fn mono(self, size: f32) -> Self {
        self.font_family(scale::FONT_MONO)
            .text_size(scaled(size))
            .line_height(relative(scale::LH_MONO))
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
