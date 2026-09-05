//! `.kbd`: a keycap — 18 px, mono 11 / 500, surface-2 with a line-strong border.

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, IntoElement, SharedString, Window};

/// `.kbd{height:18px;padding:0 5px}`.
const HEIGHT: f32 = 18.0;
const PAD: f32 = 5.0;

/// A keycap. Build with [`kbd`].
#[derive(IntoElement)]
pub struct Kbd {
    keys: SharedString,
}

/// A keycap such as `esc` or `⌘K`.
pub fn kbd(keys: impl Into<SharedString>) -> Kbd {
    Kbd { keys: keys.into() }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        div()
            .flex_none()
            .flex()
            .items_center()
            .h(px(HEIGHT))
            .px(px(PAD))
            .rounded(px(scale::R_XS))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.surface_2)
            .text_color(p.ink_2)
            .font_family(scale::FONT_MONO)
            .text_px(scale::FS_11)
            .line_height(gpui::relative(1.0))
            .medium()
            .whitespace_nowrap()
            .child(self.keys)
    }
}
