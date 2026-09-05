//! Shimmer: the "Thinking…" / "Working…" text sweep (1.8 s linear) and the
//! surface skeleton (1.4 s). Text shimmer reuses gpui-kit's masked-glyph
//! implementation with the design's timing; the skeleton is a sliding
//! gradient.

use std::time::Duration;

use aui_tokens::ActiveAui;
use gpui::{linear_color_stop, linear_gradient, px, App, Div, ElementId, Pixels, SharedString, Styled, Window};
use gpui_kit::component::shimmer::ShimmerText;

use crate::looping::{looping, Loop};

/// One sweep of the text shimmer.
pub const TEXT_PERIOD: Duration = Duration::from_millis(1800);
/// One sweep of the surface skeleton.
pub const SKELETON_PERIOD: Duration = Duration::from_millis(1400);

/// A shimmering label: ink-3 base with an ink highlight sweeping left to right
/// every 1.8 s (`.shimmer` in base.css). The caller applies size and weight.
pub fn shimmer_text(id: impl Into<ElementId>, text: impl Into<SharedString>, cx: &App) -> ShimmerText {
    let colors = cx.aui().colors;
    ShimmerText::new(text)
        .id(id)
        .duration(TEXT_PERIOD)
        .highlight_color(colors.ink)
        .spread(0.45)
        .text_color(colors.ink_3)
}

/// A skeleton block: surface-3 with a line-strong band sweeping across every
/// 1.4 s (`.sk` in base.css). Returns a `div` sized by the caller.
pub fn skeleton(id: impl Into<ElementId>, width: Pixels, height: Pixels, window: &mut Window, cx: &mut App) -> Div {
    let colors = cx.aui().colors;
    let id: ElementId = id.into();
    let phase = looping((id, "skeleton"), Loop::linear(SKELETON_PERIOD), window, cx);
    // Move the band from fully left of the box to fully right.
    let band = (phase * 2.0 - 0.5).clamp(-0.5, 1.5);
    let stop = |at: f32| (band + at).clamp(0.0, 1.0);
    gpui::div()
        .w(width)
        .h(height)
        .rounded(px(aui_tokens::scale::R_XS))
        .bg(linear_gradient(
            90.0,
            linear_color_stop(colors.surface_3, stop(-0.25)),
            linear_color_stop(colors.line_strong, stop(0.0)),
        ))
}
