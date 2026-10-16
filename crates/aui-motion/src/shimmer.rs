//! Shimmer: the "Thinking…" / "Working…" text sweep (1.8 s linear) and the
//! surface skeleton (1.4 s). The text sweep is gpui-kit's masked-glyph
//! painter (`gpui-component` 0.6.0 `shimmer.rs`, Apache-2.0), ported so its
//! phase comes from [`crate::synced`]: gpui-kit drove it with
//! `with_animation`, which asks for every frame and so re-rendered the whole
//! window at display rate for as long as any turn was running (perf audit
//! 2026-10-16). Same bands, colours and timing; at most 30 redraws a second.
//! The skeleton is a sliding gradient.

use std::time::Duration;

use aui_tokens::ActiveAui;
use gpui::{
    linear_color_stop, linear_gradient, point, px, size, App, Bounds, ContentMask, Div, Element, ElementId,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, LineLayout, ParentElement, Pixels, Point,
    Refineable, RenderOnce, SharedString, StyleRefinement, Styled, StyledText, TextAlign, Window, WrapBoundary,
};

use crate::looping::{looping, Loop};

/// One sweep of the text shimmer.
pub const TEXT_PERIOD: Duration = Duration::from_millis(1800);
/// One sweep of the surface skeleton.
pub const SKELETON_PERIOD: Duration = Duration::from_millis(1400);

/// A shimmering label: ink-3 base with an ink highlight sweeping left to right
/// every 1.8 s (`.shimmer` in base.css). The caller applies size and weight.
pub fn shimmer_text(id: impl Into<ElementId>, text: impl Into<SharedString>, cx: &App) -> ShimmerText {
    let colors = cx.aui().colors;
    // The id is kept for callers' sake; every sweep runs on one shared
    // clock ([`crate::synced`]), so nothing is keyed by it.
    let _ = id.into();
    ShimmerText {
        text: text.into(),
        period: TEXT_PERIOD,
        highlight: colors.ink,
        dark: cx.aui().kind.is_dark(),
        spread: 0.45,
        style: StyleRefinement::default(),
    }
    .text_color(colors.ink_3)
}

/// The label [`shimmer_text`] builds: styled like any text (size, weight,
/// colour), with [`ShimmerText::duration`] for the sweep.
#[derive(IntoElement)]
pub struct ShimmerText {
    text: SharedString,
    period: Duration,
    highlight: Hsla,
    dark: bool,
    spread: f32,
    style: StyleRefinement,
}

impl ShimmerText {
    /// One sweep.
    pub fn duration(mut self, period: Duration) -> Self {
        self.period = period;
        self
    }
}

impl Styled for ShimmerText {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ShimmerText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut container = gpui::div().min_w_0();
        container.style().refine(&self.style);
        if cx.reduce_motion() {
            return container.child(StyledText::new(self.text)).into_any_element();
        }
        let phase = crate::synced(self.period, 0.0, window, cx);
        container
            .child(ShimmerGlyphs {
                text: StyledText::new(self.text),
                highlight: self.highlight,
                dark: self.dark,
                spread: self.spread,
                phase,
            })
            .into_any_element()
    }
}

/// How many nested masks make the soft band.
const SHIMMER_LAYER_COUNT: usize = 12;

/// Paints the highlight over glyphs already laid out by `StyledText`, which
/// stays the layout owner (wrapping, truncation, inherited typography, the
/// glyph cache); nested content masks make one soft continuous band.
struct ShimmerGlyphs {
    text: StyledText,
    highlight: Hsla,
    dark: bool,
    spread: f32,
    phase: f32,
}

impl ShimmerGlyphs {
    fn paint_highlight(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let masks = std::array::from_fn::<_, SHIMMER_LAYER_COUNT, _>(|layer| {
            shimmer_band_bounds(bounds, self.phase, self.spread, layer).map(|bounds| ContentMask { bounds })
        });
        if masks.iter().all(Option::is_none) {
            return;
        }
        let peak_opacity: f32 = if self.dark { 0.6 } else { 0.75 };
        let layer_opacity = 1. - (1. - peak_opacity).powf(1. / SHIMMER_LAYER_COUNT as f32);
        let color = Hsla { a: self.highlight.a * layer_opacity.clamp(0.0, 1.0), ..self.highlight };
        let layout = self.text.layout();
        let line_height = layout.line_height();
        let text_align = window.text_style().text_align;
        let mut line_origin = bounds.origin;
        window.paint_layer(bounds, |window| {
            for wrapped_line in layout.line_layouts() {
                let line = &wrapped_line.unwrapped_layout;
                let baseline_offset =
                    point(px(0.), (line_height - line.ascent - line.descent) / 2. + line.ascent);
                let mut wraps = wrapped_line.wrap_boundaries.iter().peekable();
                let mut glyph_origin = point(
                    aligned_origin_x(line_origin, bounds.size.width, px(0.), text_align, line, wraps.peek().copied()),
                    line_origin.y,
                );
                let mut previous_glyph_position = Point::default();
                for (run_index, run) in line.runs.iter().enumerate() {
                    let glyph_size = cx.text_system().bounding_box(run.font_id, line.font_size).size;
                    for (glyph_index, glyph) in run.glyphs.iter().enumerate() {
                        glyph_origin.x += glyph.position.x - previous_glyph_position.x;
                        if wraps.peek().is_some_and(|wrap| wrap.run_ix == run_index && wrap.glyph_ix == glyph_index) {
                            wraps.next();
                            glyph_origin.x = aligned_origin_x(
                                line_origin,
                                bounds.size.width,
                                glyph.position.x,
                                text_align,
                                line,
                                wraps.peek().copied(),
                            );
                            glyph_origin.y += line_height;
                        }
                        previous_glyph_position = glyph.position;
                        if glyph.is_emoji {
                            continue;
                        }
                        let glyph_bounds = Bounds::new(glyph_origin, glyph_size);
                        let paint_origin = glyph_origin + baseline_offset + point(px(0.), glyph.position.y);
                        for mask in masks.iter().flatten() {
                            if !glyph_bounds.intersects(&mask.bounds) {
                                continue;
                            }
                            window.with_content_mask(Some(*mask), |window| {
                                let _ = window.paint_glyph(paint_origin, run.font_id, glyph.id, line.font_size, color);
                            });
                        }
                    }
                }
                line_origin.y += wrapped_line.size(line_height).height;
            }
        });
    }
}

impl IntoElement for ShimmerGlyphs {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ShimmerGlyphs {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(global_id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.text.prepaint(global_id, inspector_id, bounds, layout, window, cx);
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.text.paint(global_id, inspector_id, bounds, layout, prepaint, window, cx);
        self.paint_highlight(bounds, window, cx);
    }
}

/// The `layer`th of the nested band masks at `phase`, or `None` when it is
/// empty or off the text.
fn shimmer_band_bounds(bounds: Bounds<Pixels>, phase: f32, spread: f32, layer: usize) -> Option<Bounds<Pixels>> {
    let width = f32::from(bounds.size.width);
    if width <= 0. || bounds.size.height <= px(0.) || layer >= SHIMMER_LAYER_COUNT {
        return None;
    }
    let half_width = width * spread;
    let padding = half_width / width + 0.05;
    let center = phase.mul_add(1. + padding * 2., -padding) * width;
    let radius = half_width * (1. - layer as f32 / SHIMMER_LAYER_COUNT as f32);
    let left = (center - radius).max(0.);
    let right = (center + radius).min(width);
    (right > left).then(|| {
        Bounds::new(point(bounds.origin.x + px(left), bounds.origin.y), size(px(right - left), bounds.size.height))
    })
}

/// Where a line's first glyph sits under `align`.
fn aligned_origin_x(
    origin: Point<Pixels>,
    align_width: Pixels,
    previous_glyph_x: Pixels,
    align: TextAlign,
    layout: &LineLayout,
    next_wrap: Option<&WrapBoundary>,
) -> Pixels {
    let line_end = next_wrap.map(|wrap| layout.runs[wrap.run_ix].glyphs[wrap.glyph_ix].position.x).unwrap_or(layout.width);
    let line_width = line_end - previous_glyph_x;
    match align {
        TextAlign::Left => origin.x,
        TextAlign::Center => (origin.x * 2. + align_width - line_width) / 2.,
        TextAlign::Right => origin.x + align_width - line_width,
    }
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
