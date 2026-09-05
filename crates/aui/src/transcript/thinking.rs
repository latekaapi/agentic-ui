//! Card 32: the thinking block — shimmering label with elapsed time, a
//! capped streaming viewport with a top fade, collapsing to a one-line
//! summary when done; click re-expands the full trace.

use aui_motion::shimmer_text;
use aui_protocol::ThinkingState;
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, linear_color_stop, linear_gradient, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::v_flex;

use crate::icons::{icon, IconName};
use crate::transcript::transcript_card;
use crate::util::ClickHandler;

/// `.th .hd{height:32px;font-size:12.5px}`.
const HEADER_H: f32 = 32.0;
/// `.th .vp{max-height:96px;padding:0 12px 10px 30px;font-size:12.5px;line-height:1.55}`.
const VIEWPORT_MAX: f32 = 96.0;
const VP_PAD_RIGHT: f32 = 12.0;
const VP_PAD_BOTTOM: f32 = 10.0;
const VP_PAD_LEFT: f32 = 30.0;
const TRACE_TEXT: f32 = 12.5;
const TRACE_LH: f32 = 1.55;
/// `.th .vp::before{height:34px}` — the fade at the top of the capped viewport.
const FADE_H: f32 = 34.0;
/// `.th .vp p{margin:0 0 6px}`.
const PARAGRAPH_GAP: f32 = 6.0;

/// The thinking block. Build with [`thinking_block`].
#[derive(IntoElement)]
pub struct ThinkingBlock {
    id: ElementId,
    text: SharedString,
    elapsed: SharedString,
    summary: Option<SharedString>,
    state: ThinkingState,
    expanded: bool,
    on_toggle: Option<ClickHandler>,
}

/// A block over the trace `text`, with `elapsed` as shown (`14 s`).
pub fn thinking_block(id: impl Into<ElementId>, text: impl Into<SharedString>, elapsed: impl Into<SharedString>, state: ThinkingState) -> ThinkingBlock {
    ThinkingBlock { id: id.into(), text: text.into(), elapsed: elapsed.into(), summary: None, state, expanded: false, on_toggle: None }
}

impl ThinkingBlock {
    /// The one-line summary shown once done.
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Whether a finished trace is expanded to its full text.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Header click.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for ThinkingBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let thinking = self.state == ThinkingState::Thinking;
        let open = thinking || self.expanded;

        let mut card = transcript_card(id.clone(), open).header_height(px(HEADER_H));
        card = card.header(icon(IconName::Brain).color(if thinking { p.accent_ink } else { p.ink_3 }));
        if thinking {
            card = card.header(shimmer_text((id.clone(), "label"), "Thinking", cx).text_size(aui_tokens::scaled(TRACE_TEXT)).font_weight(gpui::FontWeight::MEDIUM));
        } else {
            card = card.header(div().child(format!("Thought for {}", self.elapsed)));
            if let Some(summary) = &self.summary {
                if !self.expanded {
                    card = card.header(div().min_w(px(0.0)).truncate().text_color(p.ink_2).child(format!("· {summary}")));
                }
            }
        }
        let right: SharedString = if thinking {
            self.elapsed.clone()
        } else if self.expanded {
            "expanded".into()
        } else {
            "".into()
        };
        card = card.header(div().flex_1()).header(div().text_role(TextRole::MonoSmall).font_weight(gpui::FontWeight::MEDIUM).text_color(p.ink_3).child(right));

        // The trace: paragraphs 12.5 / 1.55 in ink-2.
        let mut paragraphs = v_flex().w_full().ui(TRACE_TEXT).line_height(relative(TRACE_LH)).text_color(p.ink_2);
        let parts: Vec<&str> = self.text.split("\n\n").filter(|s| !s.trim().is_empty()).collect();
        let count = parts.len();
        for (i, para) in parts.into_iter().enumerate() {
            paragraphs = paragraphs.child(div().w_full().when(i + 1 < count, |d| d.mb(px(PARAGRAPH_GAP))).child(para.trim().to_string()));
        }
        let mut viewport = div().relative().w_full().pr(px(VP_PAD_RIGHT)).pb(px(VP_PAD_BOTTOM)).pl(px(VP_PAD_LEFT));
        if thinking {
            // Capped at four lines, content bottom-aligned so new text pushes
            // up, with a surface-1 fade over the top edge.
            viewport = viewport
                .max_h(px(VIEWPORT_MAX))
                .overflow_hidden()
                .child(v_flex().w_full().min_h(px(VIEWPORT_MAX)).justify_end().child(paragraphs))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(FADE_H))
                        .bg(linear_gradient(180.0, linear_color_stop(p.surface_1, 0.0), linear_color_stop(p.surface_1.alpha(0.0), 1.0))),
                );
        } else {
            viewport = viewport.child(paragraphs);
        }
        // `.th .vp` sits directly under the header, without a separator line.
        card = card.body_border(false).body(viewport);
        if let Some(on_toggle) = self.on_toggle {
            card = card.on_toggle(move |e, w, cx| on_toggle(e, w, cx));
        }
        card
    }
}
