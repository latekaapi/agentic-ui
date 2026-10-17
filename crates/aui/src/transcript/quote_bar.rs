//! The quote flow's selection bar (docs/43 §0; canvas v49 `D4QuoteSelect`):
//! on mouse-up after a selection inside one turn, one bar — Floating, r 8,
//! the edge + float drop, 32 tall, padding 3 — with one text control,
//! **Quote** (26, r 6, padding 0 / 10 / 0 / 8, the quote glyph 14 muted, the
//! label 14.3, hover 5 %). Role toolbar "Selection". It enters in 80 ms and
//! leaves in 60 ms (`EnterExit::POPUP`). The host places it: 6 px above the
//! selection's first line at the selection's start, or 6 under the last line
//! when that would sit within 38 px of the sheet's top.

use aui_icons::{icon, IconName};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{prelude::*, px, App, ClickEvent, ElementId, IntoElement, Window};
use gpui_kit::base::h_flex;

/// The bar's height.
pub const QUOTE_BAR_H: f32 = 32.0;
/// The gap between the bar and the selection's line.
pub const QUOTE_BAR_GAP: f32 = 6.0;
/// Under this distance from the sheet's top the bar hangs below instead.
pub const QUOTE_BAR_TOP_CLEARANCE: f32 = 38.0;

/// Where the bar's top goes for a selection whose first line starts at
/// `first_top` and whose last line ends at `last_bottom` (both in sheet
/// coordinates): above with a 6 px gap, or under the last line when above
/// would come within 38 px of the sheet's top.
pub fn quote_bar_top(first_top: f32, last_bottom: f32) -> f32 {
    let above = first_top - QUOTE_BAR_GAP - QUOTE_BAR_H;
    if above < QUOTE_BAR_TOP_CLEARANCE {
        last_bottom + QUOTE_BAR_GAP
    } else {
        above
    }
}

/// The bar; `None` when it has fully left. `present` drives the 80 / 60 ms
/// fade.
pub fn quote_bar(id: impl Into<ElementId>, present: bool, on_quote: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) -> Option<gpui::AnyElement> {
    let id: ElementId = id.into();
    let sample = presence((id.clone(), "presence"), present, EnterExit::POPUP, window, cx);
    let style = PresenceStyle::fade(sample);
    if !present && style.opacity <= 0.001 {
        return None;
    }
    let p = cx.aui().colors;
    let hover = p.hover_overlay();
    Some(
        h_flex()
            .id(id.clone())
            .role(gpui::Role::Toolbar)
            .aria_label("Selection")
            .opacity(style.opacity)
            .h(px(QUOTE_BAR_H))
            .p(px(3.0))
            .rounded(px(8.0))
            .bg(p.overlay)
            .shadow(p.floating_shadow())
            .occlude()
            .child(
                h_flex()
                    .id((id, "quote"))
                    .role(gpui::Role::Button)
                    .aria_label("Quote the selection in the composer")
                    .cursor_pointer()
                    .h(px(26.0))
                    .pl(px(8.0))
                    .pr(px(10.0))
                    .gap(px(6.0))
                    .items_center()
                    .rounded(px(6.0))
                    .font_family(scale::FONT_UI)
                    .text_px(14.3)
                    .text_color(p.fg)
                    .hover(move |s| s.bg(hover))
                    .child(icon(IconName::Quote).size(px(14.0)).color(p.fg_muted))
                    .child("Quote")
                    .on_click(on_quote),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_hangs_above_unless_the_title_row_is_in_the_way() {
        assert_eq!(quote_bar_top(200.0, 240.0), 162.0);
        // 60 − 38 = 22 < 38: under the last line instead.
        assert_eq!(quote_bar_top(60.0, 104.0), 110.0);
    }
}
