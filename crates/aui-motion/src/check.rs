//! Check draw: a success check mark that draws itself in 320 ms (ease-out).
//! gpui cannot animate an SVG stroke offset, so the glyph is revealed left to
//! right through a clip, which reads the same at 12–14 px.

use std::time::Duration;

use aui_tokens::Easing;
use gpui::{div, prelude::*, AnyElement, App, ElementId, IntoElement, Pixels, Window};
use gpui_kit::base::{animate_keyframes, Keyframe, Keyframes, Timing};

/// Draw duration.
pub const DURATION: Duration = Duration::from_millis(320);

/// Progress 0..=1 of the draw for `generation` under `id` (0 = not started).
pub fn check_draw(id: impl Into<ElementId>, generation: u32, window: &mut Window, cx: &mut App) -> f32 {
    if generation == 0 {
        return 0.0;
    }
    let frames = Keyframes::try_new([Keyframe::new(0.0, 0.0f32), Keyframe::new(1.0, 1.0f32)]).expect("valid");
    let id: ElementId = id.into();
    let timing = Timing::new(DURATION).ease(gpui_kit::base::Easing::Custom(std::rc::Rc::new(|t| Easing::OUT.sample(t))));
    animate_keyframes((crate::child_id(id, generation as usize), "check"), &frames, timing, window, cx).value
}

/// Clips `glyph` (a `size` × `size` element) to `progress` of its width.
#[derive(IntoElement)]
pub struct CheckDraw {
    size: Pixels,
    progress: f32,
    glyph: AnyElement,
}

impl CheckDraw {
    /// Wraps a glyph.
    pub fn new(size: Pixels, progress: f32, glyph: impl IntoElement) -> Self {
        Self { size, progress: progress.clamp(0.0, 1.0), glyph: glyph.into_any_element() }
    }
}

impl RenderOnce for CheckDraw {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .relative()
            .flex_none()
            .size(self.size)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h(self.size)
                    .w(self.size * self.progress)
                    .overflow_hidden()
                    .child(div().size(self.size).child(self.glyph)),
            )
    }
}
