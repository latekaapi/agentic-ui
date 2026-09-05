//! `.card` / `.card-h` / `.card-b`: the collapsible card frame shared by the
//! thinking block, activity group, tool cards and todo list — radius 8, 1 px
//! line, surface-1; a 34 px header that tints on hover and carries the chevron
//! (swap spring); a body behind a 1 px top border that collapses on the
//! layout spring.

use aui_motion::{collapse, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::nav::chevron;
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// `.card-h{gap:8px;padding:0 10px}`.
const HEADER_GAP: f32 = 8.0;
const HEADER_PAD: f32 = 10.0;
/// Header text is 12.5 px in every transcript card.
const HEADER_TEXT: f32 = 12.5;

/// A collapsible transcript card. Build with [`transcript_card`].
#[derive(IntoElement)]
pub struct TranscriptCard {
    id: ElementId,
    open: bool,
    header: Vec<AnyElement>,
    body: Option<AnyElement>,
    chevron: bool,
    hover_tint: bool,
    on_toggle: Option<ClickHandler>,
}

/// An empty card; add header parts with [`TranscriptCard::header`] and a body
/// with [`TranscriptCard::body`].
pub fn transcript_card(id: impl Into<ElementId>, open: bool) -> TranscriptCard {
    TranscriptCard { id: id.into(), open, header: Vec::new(), body: None, chevron: true, hover_tint: true, on_toggle: None }
}

impl TranscriptCard {
    /// Appends a header part (glyph, label, spacer, meta).
    pub fn header(mut self, el: impl IntoElement) -> Self {
        self.header.push(el.into_any_element());
        self
    }

    /// The body, drawn behind a 1 px top border and collapsed when closed.
    pub fn body(mut self, el: impl IntoElement) -> Self {
        self.body = Some(el.into_any_element());
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
        let bg = tween((header_id.clone(), "bg"), if tint { p.surface_2 } else { gpui::transparent_black() }, Tween::FAST, window, cx);

        let mut header = h_flex()
            .id(header_id.clone())
            .w_full()
            .h(cx.aui().metrics.card_header)
            .flex_none()
            .gap(px(HEADER_GAP))
            .px(px(HEADER_PAD))
            .bg(bg)
            .ui(HEADER_TEXT)
            .text_color(p.ink)
            .cursor_pointer()
            .track_interaction(&state)
            .children(self.header);
        if self.chevron {
            header = header.child(chevron((id.clone(), "chevron"), self.open, p.ink_3, window, cx));
        }
        if let Some(on_toggle) = self.on_toggle {
            header = header.on_click(move |e, w, cx| on_toggle(e, w, cx));
        }

        let mut card = v_flex()
            .id(id.clone())
            .w_full()
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .child(header);
        if let Some(body) = self.body {
            let body = div().w_full().border_t_1().border_color(p.line).child(body).into_any_element();
            let (reveal, _) = collapse((id, "body"), self.open, body, window, cx);
            card = card.child(reveal);
        }
        card
    }
}
