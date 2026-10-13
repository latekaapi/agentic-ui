//! The thinking row (D4, docs/41 §2.4): a quiet row — "Thinking" shimmering
//! with its elapsed while it streams, "Thought for 4.2 s · summary" once
//! done — whose open content is the trace in caption muted, 32 in from the
//! measure, capped with a 34 px top fade while it streams.

use aui_motion::shimmer_text;
use aui_protocol::ThinkingState;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, linear_color_stop, linear_gradient, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::v_flex;

use crate::icons::{icon, IconName};
use crate::transcript::quiet_row;
use crate::util::ClickHandler;

/// The thinking glyph after the chevron.
const GLYPH: f32 = 14.0;
/// The live label's shimmer sweep (docs/41 §3).
const SHIMMER: std::time::Duration = std::time::Duration::from_millis(1600);
/// The streaming viewport's cap.
const VIEWPORT_MAX: f32 = 96.0;
/// The trace sits 32 in from the measure, under the row's text.
const TRACE_INDENT: f32 = 32.0;
const TRACE_INDENT_RIGHT: f32 = 12.0;
const VP_PAD_BOTTOM: f32 = 8.0;
/// Caption.
const TRACE_TEXT: f32 = 12.0;
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

        // D4 (docs/41 §2.4): a quiet row — chevron, the thinking glyph 14
        // muted, "Thought for 4.2 s · summary" — whose open content is the
        // trace in caption muted, 32 in from the measure.
        let mut row = quiet_row(id.clone(), open).part(div().flex_none().child(icon(IconName::Brain).size(px(GLYPH)).color(p.ink_3)));
        if thinking {
            row = row
                .part(div().flex_none().child(shimmer_text((id.clone(), "label"), "Thinking", cx).duration(SHIMMER)))
                .part(div().flex_none().mono(scale::FS_12).text_color(p.ink_4).child(self.elapsed.clone()));
        } else {
            row = row.part(div().flex_none().child(format!("Thought for {}", self.elapsed)));
            if let Some(summary) = &self.summary {
                row = row.part(div().min_w(px(0.0)).truncate().child(format!("· {summary}")));
            }
        }

        let mut paragraphs = v_flex().w_full().ui(TRACE_TEXT).line_height(relative(TRACE_LH)).text_color(p.ink_2);
        let parts: Vec<&str> = self.text.split("\n\n").filter(|s| !s.trim().is_empty()).collect();
        let count = parts.len();
        for (i, para) in parts.into_iter().enumerate() {
            paragraphs = paragraphs.child(div().w_full().when(i + 1 < count, |d| d.mb(px(PARAGRAPH_GAP))).child(para.trim().to_string()));
        }
        let mut viewport = div().relative().w_full().pl(px(TRACE_INDENT)).pr(px(TRACE_INDENT_RIGHT)).pt(px(2.0)).pb(px(VP_PAD_BOTTOM));
        if thinking {
            // Capped while it streams, content bottom-aligned so new text
            // pushes up, with a 34 px fade to the sheet over the top edge.
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
                        .bg(linear_gradient(180.0, linear_color_stop(p.surface_2, 0.0), linear_color_stop(p.surface_2.alpha(0.0), 1.0))),
                );
        } else {
            viewport = viewport.child(paragraphs);
        }
        row = row.body(viewport);
        if let Some(on_toggle) = self.on_toggle {
            row = row.on_toggle(move |e, w, cx| on_toggle(e, w, cx));
        }
        row
    }
}
