//! The tooltip: Fluid Functionalism's (`registry/base/tooltip.tsx`).
//!
//! An inverted chip — the foreground as its fill, the background as its
//! text — at the caption role (12/16), 8 px across and 4 px down, on the
//! item radius, with no shadow. It fades in over the fast tier (80 ms)
//! after FF's 200 ms hover delay.
//!
//! Adaptations: gpui seats a tooltip beside the pointer, not 8 px off the
//! trigger's side, and has no slide-in offset; the fade is kept. gpui flips
//! a tooltip only at the window's edge, so a tooltip next to a native page
//! (which draws above everything gpui paints) was cut by the page; a tip
//! built with [`tip_within`] opens to the pointer's left instead when it
//! would cross its right limit.

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
    /// The x it must not cross (window coordinates), when bounded.
    right: Option<gpui::Pixels>,
}

/// A caption-role chip's width for `text`, estimated: Inter at 12 px runs
/// about 6.6 px a character, plus the chip's 8 px each side.
fn estimated_width(text: &str) -> f32 {
    text.chars().count() as f32 * 6.6 + scale::SP_3 * 2.0
}

impl gpui::Render for Tip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let elapsed = self.born.elapsed();
        let opacity = (elapsed.as_secs_f32() / TIP_FADE.as_secs_f32()).min(1.0);
        if opacity < 1.0 {
            aui_motion::request_frame(window);
        }
        let chip = Shape::Item.rounded(
            div()
                .px(px(scale::SP_3))
                .py(px(scale::SP_2))
                .bg(p.fg)
                .text_color(p.bg)
                .role_in(TextRole::Caption, cx)
                .whitespace_nowrap()
                .opacity(opacity)
                .child(self.text.clone()),
        );
        let crosses = self.right.is_some_and(|right| window.mouse_position().x + px(1.0) + px(estimated_width(&self.text)) > right);
        if crosses {
            // A zero-size anchor at the pointer with the chip hung off its
            // left: gpui places the anchor, the chip opens inward.
            return div().relative().size(px(0.0)).child(chip.absolute().right(px(scale::SP_1)).top(px(0.0))).into_any_element();
        }
        chip.into_any_element()
    }
}

/// A tooltip builder for gpui's `.tooltip(..)`: pair it with
/// `.tooltip_show_delay(TIP_DELAY)`.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip { text: text.clone(), born: Instant::now(), right: None }).into()
}

/// [`tip`] that opens to the pointer's left when it would cross `right`
/// (window x): for controls beside a native page, such as the sidebar's.
pub fn tip_within(text: impl Into<SharedString>, right: gpui::Pixels) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip { text: text.clone(), born: Instant::now(), right: Some(right) }).into()
}
