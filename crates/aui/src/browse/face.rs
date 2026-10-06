//! The 16 px task status face: a circle outline in `ink-3` (or `warning`
//! for [`TaskFace::NeedsYou`](crate::browse::TaskFace)) with the state's
//! mark inside, drawn from plain elements — no sprite glyph needed.

use aui_icons::{icon, IconName};
use aui_tokens::ActiveAui;
use gpui::{div, prelude::*, px, App, IntoElement, Window};

use crate::browse::TaskFace;

/// The face size: 16 px square, drawn in place of the folder glyph.
pub const FACE_SIZE: f32 = 16.0;
/// The eyes: two 2 px dots, 3 px apart.
const EYE: f32 = 2.0;
const EYE_GAP: f32 = 3.0;
/// Paused bars and the needs-you wave: short 2 px-high lines.
const BAR_H: f32 = 2.0;
const PAUSED_BAR_W: f32 = 6.0;
const PAUSED_GAP: f32 = 2.0;
const WAVE_W: f32 = 5.0;
/// The typing dots sit this far above the circle's bottom edge.
const TYPING_BOTTOM: f32 = 4.0;
/// The wave floats at the circle's upper right.
const WAVE_TOP: f32 = 4.0;
const WAVE_RIGHT: f32 = 2.5;
/// The done check: 10 px in `success`.
const CHECK_SIZE: f32 = 10.0;

/// The 16 px status face for `face`, on its own. Build with [`task_face`].
#[derive(Debug, Clone, Copy, IntoElement)]
pub struct TaskFaceGlyph {
    face: TaskFace,
}

/// The 16 px status face for `face`, on its own.
pub fn task_face(face: TaskFace) -> TaskFaceGlyph {
    TaskFaceGlyph { face }
}

impl TaskFaceGlyph {
    /// Which face this draws.
    pub fn face(&self) -> TaskFace {
        self.face
    }
}

impl RenderOnce for TaskFaceGlyph {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let ring = if self.face == TaskFace::NeedsYou { p.warning } else { p.ink_3 };
        let mut face = div()
            .flex_none()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .size(px(FACE_SIZE))
            .child(div().absolute().inset_0().rounded_full().border_1().border_color(ring));
        let dot = |color: gpui::Hsla| div().flex_none().size(px(EYE)).rounded_full().bg(color);
        match self.face {
            TaskFace::Reading => {
                face = face.child(
                    div().flex().items_center().gap(px(EYE_GAP)).child(dot(p.ink_2)).child(dot(p.ink_2)),
                );
            }
            TaskFace::Typing => {
                face = face.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(TYPING_BOTTOM))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(EYE_GAP))
                        .child(dot(p.ink_2))
                        .child(dot(p.ink_2)),
                );
            }
            TaskFace::Paused => {
                let bar = |color: gpui::Hsla| div().flex_none().w(px(PAUSED_BAR_W)).h(px(BAR_H)).rounded(px(1.0)).bg(color);
                face = face.child(div().flex().flex_col().items_center().gap(px(PAUSED_GAP)).child(bar(p.ink_2)).child(bar(p.ink_2)));
            }
            TaskFace::NeedsYou => {
                face = face.child(
                    div()
                        .absolute()
                        .top(px(WAVE_TOP))
                        .right(px(WAVE_RIGHT))
                        .w(px(WAVE_W))
                        .h(px(BAR_H))
                        .rounded(px(1.0))
                        .bg(p.warning),
                );
            }
            TaskFace::Done => {
                face = face.child(icon(IconName::Check).size(px(CHECK_SIZE)).color(p.success));
            }
        }
        // The face itself is decorative; the row names the state, so it
        // records no AX label of its own.
        face
    }
}
