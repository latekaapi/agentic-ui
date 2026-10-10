//! The tooltip: Fluid Functionalism's (`registry/base/tooltip.tsx`).
//!
//! An inverted chip — the foreground as its fill, the background as its
//! text — at the caption role (12/16), 8 px across and 4 px down, on the
//! item radius, with no shadow. It fades in over the fast tier (80 ms)
//! after FF's 200 ms hover delay.
//!
//! Adaptations: gpui seats a tooltip beside the pointer, not 8 px off the
//! trigger's side, and has no slide-in offset; the fade is kept.

use std::time::{Duration, Instant};

use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyView, App, Context, IntoElement, SharedString, Window};

use crate::interact::Shape;

/// FF's hover delay before a tooltip shows (`tooltip.tsx`: `delay 200`).
pub const TIP_DELAY: Duration = Duration::from_millis(200);

/// FF's fast tier: the tooltip's fade-in.
const TIP_FADE: Duration = Duration::from_millis(80);

/// One tooltip's view: its text and when it appeared (for the fade).
pub struct Tip {
    text: SharedString,
    born: Instant,
}

impl gpui::Render for Tip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let elapsed = self.born.elapsed();
        let opacity = (elapsed.as_secs_f32() / TIP_FADE.as_secs_f32()).min(1.0);
        if opacity < 1.0 {
            window.request_animation_frame();
        }
        Shape::Item.rounded(
            div()
                .px(px(scale::SP_3))
                .py(px(scale::SP_2))
                .bg(p.fg)
                .text_color(p.bg)
                .role_in(TextRole::Caption, cx)
                .whitespace_nowrap()
                .opacity(opacity)
                .child(self.text.clone()),
        )
    }
}

/// A tooltip builder for gpui's `.tooltip(..)`: pair it with
/// `.tooltip_show_delay(TIP_DELAY)`.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip { text: text.clone(), born: Instant::now() }).into()
}
