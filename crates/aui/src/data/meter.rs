//! `.meter`: the provider usage meter in the sidebar footer — an 11 px
//! provider mark, a 40 × 3 bar in ink-3 on surface-3, and the percentage in
//! mono 11.

use aui_icons::{provider_mark, Provider};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, IntoElement, Window};

/// `.meter i{width:40px;height:3px;border-radius:2px}`.
const BAR_WIDTH: f32 = 40.0;
const BAR_HEIGHT: f32 = 3.0;
const BAR_RADIUS: f32 = 2.0;
/// `.meter{gap:6px}`.
const GAP: f32 = 6.0;
/// The mark inside the meter: `width:11px;height:11px`.
const MARK: f32 = 11.0;

/// The usage meter. Build with [`usage_meter`].
#[derive(IntoElement)]
pub struct UsageMeter {
    provider: Provider,
    fraction: f32,
}

/// `fraction` in `0..=1`.
pub fn usage_meter(provider: Provider, fraction: f32) -> UsageMeter {
    UsageMeter { provider, fraction: fraction.clamp(0.0, 1.0) }
}

impl RenderOnce for UsageMeter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(GAP))
            .text_role(TextRole::MonoSmall)
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(p.ink_3)
            .child(provider_mark(self.provider).size(px(MARK)))
            .child(
                div()
                    .w(px(BAR_WIDTH))
                    .h(px(BAR_HEIGHT))
                    .rounded(px(BAR_RADIUS))
                    .bg(p.surface_3)
                    .overflow_hidden()
                    .child(div().h_full().w(px(BAR_WIDTH * self.fraction)).rounded(px(BAR_RADIUS)).bg(p.ink_3)),
            )
            .child(format!("{}%", (self.fraction * 100.0).round()))
    }
}
