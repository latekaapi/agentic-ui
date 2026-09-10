//! Attachments and drop (card 42, spec §4.3): the 44 px attachment rows that
//! sit above the composer input — ready, uploading, failed and the dashed
//! paste hint — and the overlay that covers the transcript pane while files
//! are dragged over it.

use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_motion::{looping, presence, shimmer_text, EnterExit, Loop, PresenceStyle};
use aui_protocol::{AttachmentKind, UploadState};
use aui_tokens::{scale, ActiveAui, AuiStyled, Easing};
use gpui::{div, linear_color_stop, linear_gradient, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, ButtonSize};

/// `.a{height:44px;padding:0 10px 0 6px;gap:10px;font-size:12.5px}`.
const ROW_H: f32 = 44.0;
const ROW_PAD_LEFT: f32 = 6.0;
const ROW_PAD_RIGHT: f32 = 10.0;
const ROW_GAP: f32 = 10.0;
const ROW_TEXT: f32 = 12.5;
/// `.att{gap:6px}` — the gap between rows in the column.
pub const ROW_STACK_GAP: f32 = 6.0;
/// `.a .th{width:32px;height:32px;border-radius:6px}` with a 135° gradient.
const THUMB: f32 = 32.0;
const THUMB_GRADIENT_ANGLE: f32 = 135.0;
/// `.a.up .th{opacity:.5}`.
const THUMB_UPLOADING_OPACITY: f32 = 0.5;
/// `.a span.s{font-size:11px}`.
const META_TEXT: f32 = 11.0;
/// `.a .x` is a plain 14 px glyph, not a button.
const REMOVE_GLYPH: f32 = 14.0;
/// `.a .bar{height:2px}`; the fill runs `width:68%` → `80%` on
/// `pg 1.6s var(--e-out) infinite alternate`.
const BAR_H: f32 = 2.0;
const BAR_SWEEP: f32 = 0.12;
const BAR_PERIOD: Duration = Duration::from_millis(1600);

/// `.drop .ov{inset:8px;border-radius:var(--r-lg);border:2px dashed var(--accent)}`.
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

/// Where one attachment row is in its life.
#[derive(Debug, Clone, PartialEq)]
pub enum AttachmentRowState {
    /// Uploaded and usable: the name in ink and an `x` to drop it.
    Ready,
    /// Transferring: dimmed thumb, shimmering name, `Cancel`, and the accent
    /// hairline across the bottom. The fraction is 0.0 to 1.0.
    Uploading(f32),
    /// The transfer failed: danger border, danger thumb, the reason as the
    /// meta line, and `Retry`.
    Failed(SharedString),
    /// The dashed "paste an image or drop files" affordance, which carries no
    /// action of its own.
    Hint,
}

impl AttachmentRowState {
    /// Maps the protocol's [`UploadState`] onto a row state.
    pub fn from_upload(state: &UploadState) -> Self {
        match state {
            UploadState::Ready => Self::Ready,
            UploadState::Uploading { progress } => Self::Uploading(*progress),
            UploadState::Failed { reason } => Self::Failed(reason.clone().into()),
        }
    }
}

type Handler = std::rc::Rc<dyn Fn(&mut Window, &mut App)>;

/// One attachment row (`.a`). Build with [`attachment_row`].
#[derive(IntoElement)]
pub struct AttachmentRow {
    id: ElementId,
    name: SharedString,
    meta: SharedString,
    kind: AttachmentKind,
    glyph: Option<IconName>,
    thumbnail: Option<std::sync::Arc<gpui::RenderImage>>,
    state: AttachmentRowState,
    on_remove: Option<Handler>,
    on_cancel: Option<Handler>,
    on_retry: Option<Handler>,
}

/// A ready file row showing `name` over one line of `meta`.
///
/// The state decides what the meta line actually says: [`AttachmentRowState::Uploading`]
/// replaces it with `Uploading · N%` and [`AttachmentRowState::Failed`] with the
/// reason, in danger.
pub fn attachment_row(id: impl Into<ElementId>, name: impl Into<SharedString>, meta: impl Into<SharedString>) -> AttachmentRow {
    AttachmentRow {
        id: id.into(),
        name: name.into(),
        meta: meta.into(),
        kind: AttachmentKind::File,
        glyph: None,
        thumbnail: None,
        state: AttachmentRowState::Ready,
        on_remove: None,
        on_cancel: None,
        on_retry: None,
    }
}

impl AttachmentRow {
    /// What the thumb tile shows: an image and a file sit on the gradient
    /// placeholder, a promoted text excerpt on the info tint.
    pub fn kind(mut self, kind: AttachmentKind) -> Self {
        self.kind = kind;
        self
    }

    /// Overrides the tile glyph the kind would pick (a PDF is a file).
    pub fn glyph(mut self, glyph: IconName) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// A decoded preview for [`AttachmentKind::Image`]: drawn as the tile in
    /// place of the glyph, like the composer image chip.
    pub fn thumbnail(mut self, thumbnail: std::sync::Arc<gpui::RenderImage>) -> Self {
        self.thumbnail = Some(thumbnail);
        self
    }

    /// Sets the row state.
    pub fn state(mut self, state: AttachmentRowState) -> Self {
        self.state = state;
        self
    }

    /// Drop the attachment (the `x` on a ready row).
    pub fn on_remove(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(std::rc::Rc::new(f));
        self
    }

    /// Abandon the upload (`Cancel`).
    pub fn on_cancel(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(std::rc::Rc::new(f));
        self
    }

    /// Try the failed upload again (`Retry`).
    pub fn on_retry(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for AttachmentRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let uploading = matches!(self.state, AttachmentRowState::Uploading(_));
        let failed = matches!(self.state, AttachmentRowState::Failed(_));
        let hint = matches!(self.state, AttachmentRowState::Hint);

        // The tile: gradient placeholder, info tint for a promoted excerpt,
        // danger tint once the upload failed.
        let glyph = self.glyph.unwrap_or(match (&self.state, self.kind) {
            (AttachmentRowState::Hint, _) => IconName::Paperclip,
            (AttachmentRowState::Failed(_), _) => IconName::X,
            (_, AttachmentKind::Image) => IconName::Image,
            (_, AttachmentKind::File | AttachmentKind::Text) => IconName::File,
        });
        let thumb: gpui::AnyElement = match (&self.kind, &self.thumbnail) {
            // A decoded preview replaces the glyph tile, like the composer
            // image chip.
            (AttachmentKind::Image, Some(image)) => {
                let mut preview = div()
                    .flex_none()
                    .size(px(THUMB))
                    .rounded(px(scale::R_SM))
                    .overflow_hidden()
                    .child(gpui::img(image.clone()).object_fit(gpui::ObjectFit::Cover).w_full().h_full());
                if uploading {
                    preview = preview.opacity(THUMB_UPLOADING_OPACITY);
                }
                preview.into_any_element()
            }
            _ => {
                let mut tile = div()
                    .flex_none()
                    .size(px(THUMB))
                    .rounded(px(scale::R_SM))
                    .flex()
                    .items_center()
                    .justify_center();
                tile = if failed {
                    tile.bg(p.danger_soft).text_color(p.danger)
                } else if matches!(self.kind, AttachmentKind::Text) && !hint {
                    tile.bg(p.info_soft).text_color(p.info)
                } else {
                    tile
                        .bg(linear_gradient(
                            THUMB_GRADIENT_ANGLE,
                            linear_color_stop(p.surface_3, 0.0),
                            linear_color_stop(p.line_strong, 1.0),
                        ))
                        .text_color(p.ink_3)
                };
                if uploading {
                    tile = tile.opacity(THUMB_UPLOADING_OPACITY);
                }
                tile.child(icon(glyph)).into_any_element()
            }
        };

        // The meta line follows the state.
        let (meta_text, meta_color) = match &self.state {
            AttachmentRowState::Uploading(progress) => (SharedString::from(format!("Uploading · {}%", (progress * 100.0).round() as i32)), p.ink_3),
            AttachmentRowState::Failed(reason) => (reason.clone(), p.danger),
            _ => (self.meta.clone(), p.ink_3),
        };

        // The name: ink, ink-2 + shimmer while uploading, ink-3 on the hint.
        let name = div().w_full().min_w(px(0.0)).truncate().medium();
        let name = if uploading {
            name.child(shimmer_text((id.clone(), "name"), self.name.clone(), cx).text_size(aui_tokens::scaled(ROW_TEXT)).font_weight(gpui::FontWeight::MEDIUM))
        } else if hint {
            name.text_color(p.ink_3).child(self.name.clone())
        } else {
            name.child(self.name.clone())
        };

        let mut row = h_flex()
            .id(id.clone())
            .relative()
            .w_full()
            .h(px(ROW_H))
            .gap(px(ROW_GAP))
            .pl(px(ROW_PAD_LEFT))
            .pr(px(ROW_PAD_RIGHT))
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(if failed { p.danger.alpha(FAILED_BORDER_ALPHA) } else { p.line })
            .when(hint, |d| d.border_dashed())
            .bg(p.surface_1)
            .ui(ROW_TEXT)
            .overflow_hidden()
            .child(thumb)
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(name)
                    .child(div().w_full().min_w(px(0.0)).truncate().ui(META_TEXT).text_color(meta_color).child(meta_text)),
            );

        // The trailing action.
        match &self.state {
            AttachmentRowState::Uploading(progress) => {
                let cancel = self.on_cancel.clone();
                row = row.child(
                    button((id.clone(), "cancel"), "Cancel")
                        .ghost()
                        .size(ButtonSize::Xs)
                        .on_click(move |_, w, cx| {
                            if let Some(h) = &cancel {
                                h(w, cx)
                            }
                        }),
                );
                let phase = looping((id.clone(), "progress"), Loop::eased(BAR_PERIOD, Easing::OUT).alternate(), window, cx);
                let fill = (progress + BAR_SWEEP * phase).clamp(0.0, 1.0);
                row = row.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(BAR_H))
                        .bg(p.surface_3)
                        .child(div().h_full().w(relative(fill)).bg(p.accent)),
                );
            }
            AttachmentRowState::Failed(_) => {
                let retry = self.on_retry.clone();
                row = row.child(
                    button((id.clone(), "retry"), "Retry")
                        .ghost()
                        .size(ButtonSize::Xs)
                        .on_click(move |_, w, cx| {
                            if let Some(h) = &retry {
                                h(w, cx)
                            }
                        }),
                );
            }
            AttachmentRowState::Ready => {
                let remove = self.on_remove.clone();
                let mut x = div().id((id, "remove")).flex_none().flex().text_color(p.ink_4).child(icon(IconName::X).size(px(REMOVE_GLYPH)));
                if let Some(h) = remove {
                    x = x.cursor_pointer().on_click(move |_, w, cx| h(w, cx));
                }
                row = row.child(x);
            }
            AttachmentRowState::Hint => {}
        }
        row
    }
}

/// `.a.fail{border-color:rgba(241,106,98,.45)}` — the only literal in the card,
/// which is the danger hue at 45 %.
const FAILED_BORDER_ALPHA: f32 = 0.45;

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
                    .rounded(px(scale::R_LG))
                    .border(px(OVERLAY_BORDER))
                    .border_dashed()
                    .border_color(p.accent)
                    .bg(p.accent_soft)
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
                                    .rounded(px(scale::R_LG))
                                    .bg(p.accent)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon(IconName::ArrowUp).size(px(TILE_GLYPH)).color(gpui::white())),
                            )
                            .child(div().ui(OVERLAY_TITLE).font_weight(gpui::FontWeight::BOLD).text_color(p.ink).child(self.title))
                            .child(div().ui(OVERLAY_SUBTITLE).text_color(p.ink_2).child(self.subtitle)),
                    ),
            )
            .into_any_element()
    }
}
