//! Icon morph: swap two glyphs (send ↔ stop, spinner → check, copy → check)
//! on the swap spring. The outgoing glyph fades, moves 3 px and shrinks to
//! .8; the incoming one does the reverse.

use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, Pixels, Window};

use crate::spring::{spring_phase, SpringKind};

/// How far the morph has progressed and the per-glyph styles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MorphSample {
    /// 0 = first glyph, 1 = second glyph.
    pub progress: f32,
    /// Opacity of the first glyph.
    pub a_opacity: f32,
    /// Vertical offset of the first glyph (moves up 3 px as it leaves).
    pub a_offset: Pixels,
    /// Scale of the first glyph (1 → .8).
    pub a_scale: f32,
    /// Opacity of the second glyph.
    pub b_opacity: f32,
    /// Vertical offset of the second glyph (arrives from 3 px below).
    pub b_offset: Pixels,
    /// Scale of the second glyph (.8 → 1).
    pub b_scale: f32,
}

impl MorphSample {
    /// Derives the glyph styles from a 0..1 phase (values outside the range
    /// come from spring overshoot and are clamped per property).
    pub fn from_progress(progress: f32) -> Self {
        let p = progress.clamp(0.0, 1.0);
        Self {
            progress,
            a_opacity: 1.0 - p,
            a_offset: px(-3.0 * p),
            a_scale: 1.0 - 0.2 * p,
            b_opacity: p,
            b_offset: px(3.0 * (1.0 - p)),
            b_scale: 0.8 + 0.2 * p,
        }
    }
}

/// Samples the morph phase for `id`: `show_second` picks the resting glyph.
pub fn icon_morph(id: impl Into<ElementId>, show_second: bool, window: &mut Window, cx: &mut App) -> MorphSample {
    let id: ElementId = id.into();
    MorphSample::from_progress(spring_phase((id, "morph"), show_second, SpringKind::Swap, window, cx))
}

/// A ready-made morph element: both glyphs stacked in a `size` × `size` box,
/// styled from a [`MorphSample`]. Scale is expressed as the glyph box size
/// (gpui has no transform), which is what the design's `scale(.8)` on a 14 px
/// glyph looks like.
#[derive(IntoElement)]
pub struct IconMorph {
    sample: MorphSample,
    size: Pixels,
    first: AnyElement,
    second: AnyElement,
}

impl IconMorph {
    /// Builds the element from a sample and the two glyph elements. Each glyph
    /// should size itself to fill its box (`size_full`) so the scale applies.
    pub fn new(sample: MorphSample, size: Pixels, first: impl IntoElement, second: impl IntoElement) -> Self {
        Self { sample, size, first: first.into_any_element(), second: second.into_any_element() }
    }
}

impl RenderOnce for IconMorph {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let s = self.sample;
        let size = self.size;
        let layer = |opacity: f32, offset: Pixels, scale: f32, child: AnyElement| {
            let box_size = size * scale;
            let inset = (size - box_size) / 2.0;
            div()
                .absolute()
                .top(inset + offset)
                .left(inset)
                .size(box_size)
                .flex()
                .items_center()
                .justify_center()
                .opacity(opacity)
                .child(child)
        };
        div()
            .relative()
            .flex_none()
            .size(size)
            .child(layer(s.a_opacity, s.a_offset, s.a_scale, self.first))
            .child(layer(s.b_opacity, s.b_offset, s.b_scale, self.second))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_show_exactly_one_glyph() {
        let a = MorphSample::from_progress(0.0);
        assert_eq!((a.a_opacity, a.b_opacity), (1.0, 0.0));
        assert_eq!(a.a_scale, 1.0);
        let b = MorphSample::from_progress(1.0);
        assert_eq!((b.a_opacity, b.b_opacity), (0.0, 1.0));
        assert!((b.a_scale - 0.8).abs() < 1e-6);
        assert_eq!(b.b_offset, px(0.0));
        // overshoot is clamped per property
        let o = MorphSample::from_progress(1.08);
        assert_eq!(o.b_opacity, 1.0);
    }
}
