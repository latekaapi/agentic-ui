//! `.chip`: a bordered, quiet control (model / mode chips in the composer,
//! scope chips in panes, knowledge sources in the assistant sidebar).

use aui_icons::{icon, IconName};
use aui_motion::{tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};

use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// `.chip{height:22px;padding:0 8px 0 7px}`.
const HEIGHT: f32 = 22.0;
const PAD_LEFT: f32 = 7.0;
const PAD_RIGHT: f32 = 8.0;
/// `.comp .bar .chip{height:28px;padding:0 10px 0 8px}`.
const COMPOSER_PAD_LEFT: f32 = 8.0;
const COMPOSER_PAD_RIGHT: f32 = 10.0;
/// `.chip{gap:5px}`.
const GAP: f32 = 5.0;
/// The chevron inside a chip: `width:10px;height:10px`.
const CHEVRON: f32 = 10.0;
/// A leading glyph inside a chip: `width:11px;height:11px`.
const LEADING_ICON: f32 = 11.0;

/// A chip. Build with [`chip`].
#[derive(IntoElement)]
pub struct Chip {
    id: ElementId,
    label: SharedString,
    detail: Option<SharedString>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    chevron: bool,
    composer: bool,
    active: bool,
    accent: bool,
    on_click: Option<ClickHandler>,
}

/// A chip with a label.
pub fn chip(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Chip {
    Chip { id: id.into(), label: label.into(), detail: None, leading: None, trailing: None, chevron: false, composer: false, active: false, accent: false, on_click: None }
}

impl Chip {
    /// A leading 11 px glyph.
    pub fn icon(self, glyph: IconName) -> Self {
        self.leading(icon(glyph).size(px(LEADING_ICON)))
    }

    /// Any leading element (a provider mark, a file-type icon).
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// A trailing element (the remove `x` of a context chip).
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// A muted suffix after the label (a file chip's kind and size).
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A trailing chevron-down, for chips that open a menu.
    pub fn chevron(mut self) -> Self {
        self.chevron = true;
        self
    }

    /// The 28 px composer toolbar size.
    pub fn composer(mut self) -> Self {
        self.composer = true;
        self
    }

    /// Selected: ink text and the line-strong border.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Accent-ink text (the "+ add" chip).
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }

    /// Click handler.
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let lift = flags.hovered || self.active;
        let border = tween((id.clone(), "border"), if lift { p.line_strong } else { p.line }, Tween::FAST, window, cx);
        let rest_text = if self.accent { p.accent_ink } else { p.ink_2 };
        let text = tween((id.clone(), "text"), if lift && !self.accent { p.ink } else { rest_text }, Tween::FAST, window, cx);
        let (height, pl, pr) = if self.composer {
            (cx.aui().metrics.control_md, COMPOSER_PAD_LEFT, COMPOSER_PAD_RIGHT)
        } else {
            (px(HEIGHT), PAD_LEFT, PAD_RIGHT)
        };
        let mut el = div()
            .id(id)
            .flex_none()
            .flex()
            .items_center()
            .h(height)
            .pl(px(pl))
            .pr(px(pr))
            .gap(px(GAP))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(border)
            .bg(p.surface_1)
            .text_color(text)
            .font_family(scale::FONT_UI)
            .text_px(scale::FS_12)
            .line_height(gpui::relative(1.0))
            .medium()
            .whitespace_nowrap()
            .cursor_pointer()
            .track_interaction(&state);
        if let Some(leading) = self.leading {
            el = el.child(leading);
        }
        el = el.child(self.label);
        if let Some(detail) = self.detail {
            el = el.child(div().text_px(scale::FS_11).text_color(p.ink_3).child(detail));
        }
        if self.chevron {
            el = el.child(icon(IconName::ChevronDown).size(px(CHEVRON)).color(text));
        }
        if let Some(trailing) = self.trailing {
            el = el.child(trailing);
        }
        if let Some(on_click) = self.on_click {
            el = el.on_click(move |e, w, cx| on_click(e, w, cx));
        }
        el
    }
}
