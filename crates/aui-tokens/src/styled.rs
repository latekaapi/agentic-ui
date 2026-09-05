//! Style helpers that apply the type scale to gpui elements.

use gpui::{px, relative, FontWeight, Styled};

use crate::generated::scale;

/// The named text roles of the design. Each role fixes family, size, line
/// height and weight; colour is applied separately from the palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextRole {
    /// Default UI text: Geist 13 / 1.5, 400.
    Ui,
    /// Emphasised UI text (controls, names): Geist 13 / 1.5, 500.
    UiMedium,
    /// Secondary UI text: Geist 12 / 1.5, 400.
    UiSmall,
    /// Assistant body copy: Geist 13.5 / 1.65, 400.
    Body,
    /// Eyebrow / caps label: Geist 11 / 1, 600, uppercase (tracking is applied
    /// by the caller because gpui has no letter-spacing style yet).
    Caps,
    /// Meta line: Geist 11 / 1.5, 400.
    Meta,
    /// Code and terminals: Geist Mono 12 / 1.6, 400.
    Mono,
    /// Small mono meta (elapsed, counts): Geist Mono 11 / 1, 400.
    MonoSmall,
    /// Tags (repo, branch): Geist Mono 10.5 / 1, 500.
    Tag,
    /// Card titles: Geist 13 / 1.3, 600.
    Title,
}

impl TextRole {
    /// `(family, size px, line-height ratio, weight)`.
    pub fn spec(self) -> (&'static str, f32, f32, FontWeight) {
        match self {
            TextRole::Ui => (scale::FONT_UI, scale::FS_13, scale::LH_UI, FontWeight::NORMAL),
            TextRole::UiMedium => (scale::FONT_UI, scale::FS_13, scale::LH_UI, FontWeight::MEDIUM),
            TextRole::UiSmall => (scale::FONT_UI, scale::FS_12, scale::LH_UI, FontWeight::NORMAL),
            TextRole::Body => (scale::FONT_UI, 13.5, scale::LH_BODY, FontWeight::NORMAL),
            TextRole::Caps => (scale::FONT_UI, scale::FS_11, 1.0, FontWeight::SEMIBOLD),
            TextRole::Meta => (scale::FONT_UI, scale::FS_11, scale::LH_UI, FontWeight::NORMAL),
            TextRole::Mono => (scale::FONT_MONO, scale::FS_12, scale::LH_MONO, FontWeight::NORMAL),
            TextRole::MonoSmall => (scale::FONT_MONO, scale::FS_11, 1.0, FontWeight::NORMAL),
            TextRole::Tag => (scale::FONT_MONO, 10.5, 1.0, FontWeight::MEDIUM),
            TextRole::Title => (scale::FONT_UI, scale::FS_13, scale::LH_TIGHT, FontWeight::SEMIBOLD),
        }
    }
}

/// Fluent helpers for applying the type scale.
pub trait AuiStyled: Styled + Sized {
    /// Applies a [`TextRole`] (family, size, line height, weight).
    fn text_role(self, role: TextRole) -> Self {
        let (family, size, lh, weight) = role.spec();
        self.font_family(family)
            .text_size(px(size))
            .line_height(relative(lh))
            .font_weight(weight)
    }

    /// UI face at a size from the scale with the UI line height (1.5).
    fn ui(self, size: f32) -> Self {
        self.font_family(scale::FONT_UI)
            .text_size(px(size))
            .line_height(relative(scale::LH_UI))
    }

    /// Mono face at a size from the scale with the mono line height (1.6).
    fn mono(self, size: f32) -> Self {
        self.font_family(scale::FONT_MONO)
            .text_size(px(size))
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
