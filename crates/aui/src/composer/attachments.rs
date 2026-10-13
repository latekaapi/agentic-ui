//! The overlay that covers the sheet while files are dragged over it (D4,
//! docs/41 §2.9). Attachments themselves are the composer's chips
//! ([`crate::composer::ComposerChip`]).

use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_motion::{looping, presence, EnterExit, Loop, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled, Easing};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::v_flex;

const OVERLAY_INSET: f32 = 8.0;
const OVERLAY_BORDER: f32 = 2.0;
/// `@keyframes in{from{opacity:0;transform:scale(.98)}}` on `var(--d-enter)`.
const OVERLAY_FROM_SCALE: f32 = 0.98;
/// `.drop .ov .ic{width:40px;height:40px;border-radius:12px;margin:0 auto 10px}`
/// with a 16 px (`.i.lg`) white glyph.
const TILE: f32 = 40.0;
const TILE_BOTTOM: f32 = 10.0;
const TILE_GLYPH: f32 = 16.0;
/// `@keyframes bob{to{transform:translateY(-4px)}}` at `1.4s var(--e-inout)
/// infinite alternate`; gpui has no transform, so it is a relative `top`.
const BOB_RISE: f32 = 4.0;
const BOB_PERIOD: Duration = Duration::from_millis(1400);
/// `.drop .ov b{font-size:14px}` (a bare `<b>`, so weight 700) and
/// `.drop .ov span{font-size:12px}`.
const OVERLAY_TITLE: f32 = scale::FS_14;
const OVERLAY_SUBTITLE: f32 = scale::FS_12;

/// The drag-over overlay (`.ov`). Build with [`drop_overlay`].
///
/// Render it as an absolutely positioned child of the `relative` pane it
/// covers; it insets itself by 8 px.
#[derive(IntoElement)]
pub struct DropOverlay {
    id: ElementId,
    visible: bool,
    title: SharedString,
    subtitle: SharedString,
    at_rest: bool,
}

/// The overlay over the pane files are being dragged onto.
pub fn drop_overlay(id: impl Into<ElementId>, visible: bool) -> DropOverlay {
    DropOverlay {
        id: id.into(),
        visible,
        title: "Drop to attach to this turn".into(),
        subtitle: "images become screenshots the agent can see".into(),
        at_rest: false,
    }
}

impl DropOverlay {
    /// The 14 px headline.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The 12 px line under it, usually a file count.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// Skips the enter fade and scale (static captures).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }
}

impl RenderOnce for DropOverlay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let style = if self.at_rest {
            PresenceStyle { opacity: 1.0, offset_y: px(0.0), scale: 1.0 }
        } else {
            let sample = presence((id.clone(), "enter"), self.visible, EnterExit::DEFAULT, window, cx);
            PresenceStyle::fade_rise_scale(sample, 0.0, OVERLAY_FROM_SCALE)
        };
        if !self.visible && style.opacity <= 0.001 {
            return div().invisible().into_any_element();
        }
        // gpui has no transform: the enter scale is expressed as a fractional
        // inset inside the 8 px frame, which moves each edge in by (1−s)/2.
        let shrink = relative((1.0 - style.scale) * 0.5);
        let bob = looping((id, "bob"), Loop::eased(BOB_PERIOD, Easing::INOUT).alternate(), window, cx);

        div()
            .absolute()
            .top(px(OVERLAY_INSET))
            .left(px(OVERLAY_INSET))
            .right(px(OVERLAY_INSET))
            .bottom(px(OVERLAY_INSET))
            .child(
                v_flex()
                    .absolute()
                    .top(shrink)
                    .left(shrink)
                    .right(shrink)
                    .bottom(shrink)
                    .items_center()
                    .justify_center()
                    // D4 (docs/41 §2.9): a 2 px dashed ring at white 20 %,
                    // radius 8; the sheet itself does not dim.
                    .rounded(px(8.0))
                    .border(px(OVERLAY_BORDER))
                    .border_dashed()
                    .border_color(if p.is_dark() { gpui::white().alpha(0.2) } else { gpui::black().alpha(0.2) })
                    .opacity(style.opacity)
                    .child(
                        v_flex()
                            .items_center()
                            .child(
                                div()
                                    .relative()
                                    .top(px(-BOB_RISE * bob))
                                    .mb(px(TILE_BOTTOM))
                                    .size(px(TILE))
                                    .rounded(px(8.0))
                                    .bg(p.overlay)
                                    .shadow(p.floating_shadow())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon(IconName::ArrowUp).size(px(TILE_GLYPH)).color(p.ink)),
                            )
                            .child(div().ui(OVERLAY_TITLE).font_weight(gpui::FontWeight::SEMIBOLD).text_color(p.ink).child(self.title))
                            .child(div().ui(OVERLAY_SUBTITLE).text_color(p.ink_2).child(self.subtitle)),
                    ),
            )
            .into_any_element()
    }
}
