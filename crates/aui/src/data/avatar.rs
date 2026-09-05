//! `.avatar`: a 22 px circle on surface-3 with a 10 px / 600 initial.

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, IntoElement, Pixels, SharedString, Window};

/// `.avatar{width:22px;height:22px;font-size:10px}`.
const SIZE: f32 = 22.0;
const FONT: f32 = 10.0;

/// An avatar. Build with [`avatar`].
#[derive(IntoElement)]
pub struct Avatar {
    initial: SharedString,
    size: Pixels,
}

/// The person's initial in a circle.
pub fn avatar(initial: impl Into<SharedString>) -> Avatar {
    Avatar { initial: initial.into(), size: px(SIZE) }
}

impl Avatar {
    /// Overrides the diameter (the rail uses 24); the initial scales with it.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let font = FONT * f32::from(self.size) / SIZE;
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(self.size)
            .rounded_full()
            .bg(p.surface_3)
            .text_color(p.ink_2)
            .font_family(scale::FONT_UI)
            .text_px(font)
            .line_height(gpui::relative(1.0))
            .semibold()
            .child(self.initial)
    }
}
