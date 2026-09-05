//! Card 50: the agent's TUI rendered line by line, with the docked input box
//! and the hint line — terminal-only mode.

use aui_motion::{looping, Loop};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, StyledText, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::kbd;
use crate::transcript::{ansi_runs, parse_ansi};

/// `.tui{padding:10px 12px;font:12px/1.5 mono}`.
const PAD_Y: f32 = 10.0;
const PAD_X: f32 = 12.0;
const TEXT: f32 = 12.0;
const LH: f32 = 1.5;
/// `.box{border:1px line-strong;radius:6px;padding:6px 10px;margin:6px 0}`.
const BOX_PAD_Y: f32 = 6.0;
const BOX_PAD_X: f32 = 10.0;
const BOX_MARGIN_Y: f32 = 6.0;
/// The cursor: 7 × 14, blinking.
const CURSOR_W: f32 = 7.0;
const CURSOR_H: f32 = 14.0;
const CURSOR_PERIOD: std::time::Duration = std::time::Duration::from_millis(1000);
/// `.hint{right:10px;top:8px;gap:4px}`.
const HINT_RIGHT: f32 = 10.0;
const HINT_TOP: f32 = 8.0;
const HINT_GAP: f32 = 4.0;

/// The TUI pane. Build with [`tui_pane`].
#[derive(IntoElement)]
pub struct TuiPane {
    id: ElementId,
    lines: Vec<String>,
    input: Option<SharedString>,
    footer: Option<SharedString>,
    hint_keys: Vec<SharedString>,
}

/// A pane over the TUI's `lines` (ANSI escapes allowed; bold via SGR 1).
pub fn tui_pane(id: impl Into<ElementId>, lines: Vec<String>) -> TuiPane {
    TuiPane { id: id.into(), lines, input: None, footer: None, hint_keys: Vec::new() }
}

impl TuiPane {
    /// The docked input box with its text (the cursor follows it).
    pub fn input(mut self, text: impl Into<SharedString>) -> Self {
        self.input = Some(text.into());
        self
    }

    /// The dim hint line under the input.
    pub fn footer(mut self, text: impl Into<SharedString>) -> Self {
        self.footer = Some(text.into());
        self
    }

    /// Keycaps pinned top-right (`⌘⇧D`).
    pub fn hint_key(mut self, key: impl Into<SharedString>) -> Self {
        self.hint_keys.push(key.into());
        self
    }
}

fn line(p: &Palette, text: &str) -> impl IntoElement {
    let spans = parse_ansi(text);
    let (t, runs) = ansi_runs(&spans, p, scale::FONT_MONO);
    let t = if t.is_empty() { " ".to_string() } else { t };
    let runs = if runs.iter().map(|r| r.len).sum::<usize>() == t.len() && !runs.is_empty() {
        runs
    } else {
        vec![gpui::TextRun { len: t.len(), font: gpui::font(scale::FONT_MONO), color: p.term_fg, background_color: None, underline: None, strikethrough: None }]
    };
    div().w_full().overflow_hidden().whitespace_nowrap().child(StyledText::new(t).with_runs(runs))
}

impl RenderOnce for TuiPane {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut body = v_flex().w_full().py(px(PAD_Y)).px(px(PAD_X)).mono(TEXT).line_height(relative(LH)).text_color(p.term_fg);
        for l in &self.lines {
            body = body.child(line(&p, l));
        }
        if let Some(input) = self.input {
            let on = looping((id.clone(), "cursor"), Loop::linear(CURSOR_PERIOD).resting(1.0), window, cx) < 0.5;
            body = body.child(
                h_flex()
                    .my(px(BOX_MARGIN_Y))
                    .py(px(BOX_PAD_Y))
                    .px(px(BOX_PAD_X))
                    .gap(px(scale::SP_2))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.line_strong)
                    .child(div().text_color(p.term_dim).child("›"))
                    .child(div().child(input))
                    .child(div().flex_none().w(px(CURSOR_W)).h(px(CURSOR_H)).bg(p.term_cursor).opacity(if on { 1.0 } else { 0.0 })),
            );
        }
        if let Some(footer) = self.footer {
            body = body.child(div().text_color(p.term_dim).child(footer));
        }
        let mut pane = div().id(id).relative().size_full().bg(p.term_bg).child(body);
        if !self.hint_keys.is_empty() {
            let mut hint = h_flex().absolute().right(px(HINT_RIGHT)).top(px(HINT_TOP)).gap(px(HINT_GAP));
            for k in self.hint_keys {
                hint = hint.child(kbd(k));
            }
            pane = pane.child(hint);
        }
        pane
    }
}
