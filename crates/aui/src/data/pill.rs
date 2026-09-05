//! `.pill`: quiet by default; status variants tint the text and barely the
//! ground. Pills are for status only (design rule).

use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, AnyElement, App, Hsla, IntoElement, SharedString, Window};

/// `.pill{height:20px;padding:0 7px;gap:5px}`.
const HEIGHT: f32 = 20.0;
const PAD: f32 = 7.0;
const GAP: f32 = 5.0;

/// `.pill.*` variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PillVariant {
    /// surface-3 ground, ink-2 text.
    #[default]
    Quiet,
    /// accent-soft / accent-ink.
    Accent,
    /// success-soft / success.
    Success,
    /// warning-soft / warning.
    Warning,
    /// danger-soft / danger.
    Danger,
    /// info-soft / info.
    Info,
    /// Transparent with a line border, ink-2.
    Line,
}

impl PillVariant {
    fn colors(self, p: &Palette) -> (Hsla, Hsla, Option<Hsla>) {
        match self {
            PillVariant::Quiet => (p.surface_3, p.ink_2, None),
            PillVariant::Accent => (p.accent_soft, p.accent_ink, None),
            PillVariant::Success => (p.success_soft, p.success, None),
            PillVariant::Warning => (p.warning_soft, p.warning, None),
            PillVariant::Danger => (p.danger_soft, p.danger, None),
            PillVariant::Info => (p.info_soft, p.info, None),
            PillVariant::Line => (gpui::transparent_black(), p.ink_2, Some(p.line)),
        }
    }
}

/// A pill. Build with [`pill`].
#[derive(IntoElement)]
pub struct Pill {
    label: SharedString,
    variant: PillVariant,
    leading: Option<AnyElement>,
    height: Option<f32>,
}

/// A quiet pill.
pub fn pill(label: impl Into<SharedString>) -> Pill {
    Pill { label: label.into(), variant: PillVariant::Quiet, leading: None, height: None }
}

impl Pill {
    /// Sets the status variant.
    pub fn variant(mut self, variant: PillVariant) -> Self {
        self.variant = variant;
        self
    }

    /// A leading element (dot, glyph).
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// Overrides the 20 px height (the assistant footer uses 16).
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

impl RenderOnce for Pill {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let (bg, text, border) = self.variant.colors(&p);
        let mut el = div()
            .flex_none()
            .flex()
            .items_center()
            .h(px(self.height.unwrap_or(HEIGHT)))
            .px(px(PAD))
            .gap(px(GAP))
            .rounded_full()
            .bg(bg)
            .text_color(text)
            .font_family(scale::FONT_UI)
            .text_px(scale::FS_11)
            .line_height(gpui::relative(1.0))
            .medium()
            .whitespace_nowrap();
        if let Some(border) = border {
            el = el.border_1().border_color(border);
        }
        if let Some(leading) = self.leading {
            el = el.child(leading);
        }
        el.child(self.label)
    }
}
