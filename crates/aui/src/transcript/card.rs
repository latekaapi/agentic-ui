//! `.card` / `.card-h` / `.card-b`: the collapsible card frame shared by the
//! tool cards, the tool group, the tasks list and the item cards — D4
//! (docs/41 §2.5): Raised, radius 8, the one edge; a 34 px header padded
//! 0 / 8 / 0 / 12 that tints on hover and carries the chevron at the right;
//! a body behind a 1 px 7 % rule that opens on `spring.moderate` and closes
//! on a 0.12 s exit, its content fading in 80 ms after the height starts.
//! A card that needs the person (a pending approval or question, a failure
//! with a retry) swaps the edge's white ring for its status colour at 70 %
//! ([`TranscriptCard::attention`]); nothing else is ever marked.

use aui_motion::{collapse_fluid, collapse_fluid_opacity, tint_fade, Tween};
use aui_tokens::{ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, radians, AnyElement, App, ElementId, IntoElement, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::record_ax_label;
use crate::icons::{icon, IconName};
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// D4 header: gap 8, padding 0 / 8 / 0 / 12.
const HEADER_GAP: f32 = 8.0;
const HEADER_PAD_LEFT: f32 = 12.0;
const HEADER_PAD_RIGHT: f32 = 8.0;
/// D4 header height (docs/41 §2.5), in both densities.
pub const CARD_HEADER_H: f32 = 34.0;
/// The chevron at the header's right: 12 px, muted, 4 px in from the cluster.
const CHEVRON: f32 = 12.0;
/// Header text is 12.5 px in every transcript card.
const HEADER_TEXT: f32 = 12.5;
/// Cards and blocks: radius 8, the sheet's own (docs/41 §2.1).
pub const CARD_RADIUS: f32 = 8.0;
/// The status ring's alpha on a card that needs the person.
pub const ATTENTION_RING_ALPHA: f32 = 0.7;

/// A collapsible transcript card. Build with [`transcript_card`].
#[derive(IntoElement)]
pub struct TranscriptCard {
    id: ElementId,
    open: bool,
    header: Vec<AnyElement>,
    body: Option<AnyElement>,
    chevron: bool,
    hover_tint: bool,
    header_height: Option<gpui::Pixels>,
    body_border: bool,
    attention: Option<gpui::Hsla>,
    flat: bool,
    on_toggle: Option<ClickHandler>,
}

/// An empty card; add header parts with [`TranscriptCard::header`] and a body
/// with [`TranscriptCard::body`].
pub fn transcript_card(id: impl Into<ElementId>, open: bool) -> TranscriptCard {
    TranscriptCard { id: id.into(), open, header: Vec::new(), body: None, chevron: true, hover_tint: true, header_height: None, body_border: true, attention: None, flat: false, on_toggle: None }
}

impl TranscriptCard {
    /// Appends a header part (glyph, label, spacer, meta).
    pub fn header(mut self, el: impl IntoElement) -> Self {
        self.header.push(el.into_any_element());
        self
    }

    /// An optional header part (a status glyph only some states draw).
    pub fn children_header(mut self, el: Option<impl IntoElement>) -> Self {
        if let Some(el) = el {
            self.header.push(el.into_any_element());
        }
        self
    }

    /// The body, drawn behind a 1 px top border and collapsed when closed.
    pub fn body(mut self, el: impl IntoElement) -> Self {
        self.body = Some(el.into_any_element());
        self
    }

    /// Overrides the 34 px header (the thinking block and todo list use 32).
    pub fn header_height(mut self, height: impl Into<gpui::Pixels>) -> Self {
        self.header_height = Some(height.into());
        self
    }

    /// Drops the 1 px line between header and body (the thinking block).
    pub fn body_border(mut self, border: bool) -> Self {
        self.body_border = border;
        self
    }

    /// Marks a card that needs the person: the edge's white ring becomes
    /// `status` at 70 % (warning for an approval or question, danger for a
    /// failure with a retry); the top highlight stays. `None` is the plain
    /// edge. The ring leaves the moment the card resolves.
    pub fn attention(mut self, status: Option<gpui::Hsla>) -> Self {
        self.attention = status;
        self
    }

    /// Draws the card as a row inside another card (a tool group's calls,
    /// docs/41 §2.5 "nothing nests"): no surface, no radius, a 7 % rule
    /// above the header.
    pub fn flat(mut self, flat: bool) -> Self {
        self.flat = flat;
        self
    }

    /// Hides the chevron (cards that never collapse).
    pub fn chevron(mut self, show: bool) -> Self {
        self.chevron = show;
        self
    }

    /// Disables the header hover tint (non-interactive headers).
    pub fn hover_tint(mut self, tint: bool) -> Self {
        self.hover_tint = tint;
        self
    }

    /// Header click.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for TranscriptCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let header_id: ElementId = (id.clone(), "header").into();
        let (state, flags) = interaction_flags(header_id.clone(), window, cx);
        let tint = self.hover_tint && flags.hovered;
        let bg = tint_fade((header_id.clone(), "bg"), tint, crate::util::raised_lit(&p, p.surface_2), Tween::FAST, window, cx);

        // The body's motion runs whether or not there is a body, so the
        // chevron turns on the same curve the height follows.
        let fade = collapse_fluid_opacity((id.clone(), "body"), self.open, window, cx);
        let (reveal, progress) = match self.body {
            Some(body) => {
                let body = div().w_full().opacity(fade).when(self.body_border, |d| d.border_t_1().border_color(p.line)).child(body).into_any_element();
                let (reveal, progress) = collapse_fluid((id.clone(), "body"), self.open, body, window, cx);
                (Some(reveal), progress)
            }
            None => (None, if self.open { 1.0 } else { 0.0 }),
        };

        let mut header = h_flex()
            .id(header_id.clone())
            .w_full()
            .h(self.header_height.unwrap_or(px(CARD_HEADER_H)))
            .flex_none()
            .gap(px(HEADER_GAP))
            .pl(px(HEADER_PAD_LEFT))
            .pr(px(HEADER_PAD_RIGHT))
            .bg(bg)
            .ui(HEADER_TEXT)
            .text_color(p.ink)
            .cursor_pointer()
            .track_interaction(&state)
            .children(self.header);
        if self.chevron {
            header = header.child(
                div()
                    .flex_none()
                    .ml(px(4.0))
                    .child(icon(IconName::Chev).size(px(CHEVRON)).color(p.ink_3).rotate(radians(progress * std::f32::consts::FRAC_PI_2))),
            );
        }
        if let Some(on_toggle) = self.on_toggle {
            // The header is the expand/collapse control: it names itself so
            // the toggle is reachable by label ("Expand tool output").
            let label = if self.open { "Collapse section" } else { "Expand section" };
            record_ax_label(label);
            header = header.role(gpui::Role::Button).aria_label(label).on_click(move |e, w, cx| on_toggle(e, w, cx));
        }

        let mut card = if self.flat {
            v_flex().id(id.clone()).w_full().border_t_1().border_color(p.line)
        } else {
            let mut card = crate::util::raised_block(v_flex().id(id.clone()).w_full().rounded(px(CARD_RADIUS)), &p, p.surface_1);
            if let Some(status) = self.attention {
                card = card.shadow(attention_edge(status));
            }
            card.overflow_hidden()
        };
        card = card.child(header);
        if let Some(reveal) = reveal {
            card = card.child(reveal);
        }
        card
    }
}

/// The edge of a card that needs the person: the top highlight kept, the
/// 1 px ring in `status` at 70 % (docs/41 §2.6).
pub fn attention_edge(status: gpui::Hsla) -> Vec<gpui::BoxShadow> {
    let inset = |offset_y: f32, spread: f32, color: gpui::Hsla| gpui::BoxShadow {
        color,
        offset: gpui::point(px(0.0), px(offset_y)),
        blur_radius: px(0.0),
        spread_radius: px(spread),
        inset: true,
    };
    vec![inset(1.0, 0.0, gpui::white().alpha(0.07)), inset(0.0, 1.0, status.alpha(ATTENTION_RING_ALPHA))]
}
