//! Status glyphs: `.glyph.ok` / `.glyph.err` (14 px circle on a soft ground
//! with a 9 px stroke-3 check or x) and `.spinner` (14 px, 1.5 px ring in
//! line-strong with the top quarter in accent, one turn every 0.9 s).

use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_motion::{looping, Loop};
use aui_tokens::ActiveAui;
use gpui::{div, prelude::*, px, radians, App, ElementId, Hsla, IntoElement, Pixels, Window};

/// `.glyph{width:14px;height:14px}` and `.spinner` likewise.
pub const GLYPH_SIZE: f32 = 14.0;
/// The check / x inside a glyph: `width:9px;height:9px`.
const MARK_SIZE: f32 = 9.0;
/// `@keyframes spin … .9s linear infinite`.
pub const SPIN_PERIOD: Duration = Duration::from_millis(900);
/// FF's loading spinner turn (button.tsx: `spinner-move 2s linear infinite`).
pub const FF_SPIN_PERIOD: Duration = Duration::from_secs(2);

/// Which status a glyph shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphKind {
    /// success-soft ground, success check.
    Ok,
    /// danger-soft ground, danger x.
    Err,
}

/// A status glyph. Build with [`glyph_ok`] / [`glyph_err`].
#[derive(IntoElement)]
pub struct Glyph {
    kind: GlyphKind,
    size: Pixels,
}

/// The success glyph.
pub fn glyph_ok() -> Glyph {
    Glyph { kind: GlyphKind::Ok, size: px(GLYPH_SIZE) }
}

/// The error glyph.
pub fn glyph_err() -> Glyph {
    Glyph { kind: GlyphKind::Err, size: px(GLYPH_SIZE) }
}

impl Glyph {
    /// Overrides the diameter; the mark scales with it.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Glyph {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let (ground, ink, name) = match self.kind {
            GlyphKind::Ok => (p.success_soft, p.success, IconName::CheckBold),
            GlyphKind::Err => (p.danger_soft, p.danger, IconName::XBold),
        };
        let mark = self.size * (MARK_SIZE / GLYPH_SIZE);
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(self.size)
            .rounded_full()
            .bg(ground)
            .child(icon(name).size(mark).color(ink))
    }
}

/// The spinner. Build with [`spinner`].
#[derive(IntoElement)]
pub struct Spinner {
    id: ElementId,
    size: Pixels,
    /// (track, arc) in place of line-strong / accent.
    colors: Option<(Hsla, Hsla)>,
    period: Duration,
}

/// A spinning ring; `id` keys its rotation.
pub fn spinner(id: impl Into<ElementId>) -> Spinner {
    Spinner { id: id.into(), size: px(GLYPH_SIZE), colors: None, period: SPIN_PERIOD }
}

impl Spinner {
    /// Overrides the diameter (rows use 10 and 11).
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    /// Explicit track and arc colours (the page's pill stays dark in light).
    pub fn colors(mut self, track: Hsla, arc: Hsla) -> Self {
        self.colors = Some((track, arc));
        self
    }

    /// The pane's 12 px spinner (docs/43 §2.11): a 1.5 px ring at 35 % muted
    /// with a muted arc.
    pub fn muted(mut self, cx: &App) -> Self {
        let p = cx.aui().colors;
        self.colors = Some((p.fg_muted.alpha(0.35), p.fg_muted));
        self
    }

    /// FF's loading spinner (button.tsx): the arc in the foreground on a
    /// border-12 % track, one turn every 2 s. Adaptation: FF also breathes
    /// the arc's length (`spinner-dash 4s`); ours keeps a fixed quarter.
    pub fn fluid(mut self, cx: &App) -> Self {
        let p = cx.aui().colors;
        self.colors = Some((p.line, p.fg));
        self.period = FF_SPIN_PERIOD;
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let (track, arc) = self.colors.unwrap_or((p.line_strong, p.accent));
        let phase = looping((self.id, "spin"), Loop::linear(self.period), window, cx);
        let angle = radians(phase * std::f32::consts::TAU);
        div()
            .relative()
            .flex_none()
            .size(self.size)
            .child(div().absolute().inset_0().child(icon(IconName::SpinnerRing).size(self.size).color(track)))
            .child(div().absolute().inset_0().child(icon(IconName::SpinnerArc).size(self.size).color(arc).rotate(angle)))
    }
}
