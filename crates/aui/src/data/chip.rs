//! `.chip`: a bordered, quiet control (model / mode chips in the composer,
//! scope chips in panes, knowledge sources in the assistant sidebar).

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};

use crate::interact::paint::{PaintStates, StatePaint};
use crate::util::ClickHandler;

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
/// The quiet-row text control: 8 px sides, 6 px icon-to-label, a 12 px chevron.
const TEXT_CONTROL_PAD: f32 = 8.0;
const TEXT_CONTROL_GAP: f32 = 6.0;
const TEXT_CONTROL_CHEVRON: f32 = 12.0;
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
    text_control: bool,
    active: bool,
    accent: bool,
    accessibility_label: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

/// A chip with a label.
pub fn chip(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Chip {
    Chip { id: id.into(), label: label.into(), detail: None, leading: None, trailing: None, chevron: false, composer: false, text_control: false, active: false, accent: false, accessibility_label: None, on_click: None }
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

    /// The composer's quiet-row control (docs/83 §5): text with an
    /// optional leading mark and a chevron — no border, no fill; 28 tall,
    /// 8 px sides, `t-caption` at 400, muted at rest and lit (ink on the
    /// `ov-hover` pill, radius 8) on hover or when [`Chip::active`].
    pub fn text_control(mut self) -> Self {
        self.text_control = true;
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

    /// The accessible name for this chip. Setting it also gives the chip the
    /// button role, so a screen reader announces name, role and value
    /// together (a chip without one keeps its previous silent shape).
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Click handler.
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        if self.text_control {
            // Hover is the one model; `active` holds the lit ink.
            let rest_ink = if self.active { p.ink } else { p.fg_muted };
            let mut el = div()
                .id(id)
                .flex_none()
                .flex()
                .items_center()
                .h(cx.aui().metrics.control_md)
                .px(px(TEXT_CONTROL_PAD))
                .gap(px(TEXT_CONTROL_GAP))
                .rounded(px(scale::R_8))
                .paint_states(StatePaint::new().ink(rest_ink, p.ink).pressable(false), &p)
                .text_role(TextRole::Caption)
                .line_height(gpui::relative(1.0))
                .whitespace_nowrap()
                .cursor_pointer()
                .children(self.leading)
                .child(self.label);
            if self.chevron {
                el = el.child(icon(IconName::ChevronDown).size(px(TEXT_CONTROL_CHEVRON)));
            }
            el = el.children(self.trailing);
            if let Some(label) = self.accessibility_label {
                el = el.role(gpui::Role::Button).aria_label(label);
            }
            if let Some(on_click) = self.on_click {
                el = el.on_click(move |e, w, cx| on_click(e, w, cx));
            }
            return el;
        }
        // The bordered chip: its edge strengthens and its ink lifts under
        // the pointer (or while `active`), through the one model.
        let rest_text = if self.accent { p.accent_ink } else if self.active { p.ink } else { p.fg_muted };
        let lit_text = if self.accent { p.accent_ink } else { p.ink };
        let rest_edge = if self.active { p.line_strong } else { p.line };
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
            .paint_states(StatePaint::new().on(p.surface_1).tint(false).ink(rest_text, lit_text).edge(rest_edge, p.line_strong), &p)
            .font_family(scale::FONT_UI)
            .role_size(TextRole::Caption)
            .line_height(gpui::relative(1.0))
            .medium()
            .whitespace_nowrap()
            .cursor_pointer();
        if let Some(leading) = self.leading {
            el = el.child(leading);
        }
        el = el.child(self.label);
        if let Some(detail) = self.detail {
            el = el.child(div().role_size(TextRole::Micro).text_color(p.fg_muted).child(detail));
        }
        if self.chevron {
            el = el.child(icon(IconName::ChevronDown).size(px(CHEVRON)));
        }
        if let Some(trailing) = self.trailing {
            el = el.child(trailing);
        }
        if let Some(label) = self.accessibility_label {
            el = el.role(gpui::Role::Button).aria_label(label);
        }
        if let Some(on_click) = self.on_click {
            el = el.on_click(move |e, w, cx| on_click(e, w, cx));
        }
        el
    }
}
