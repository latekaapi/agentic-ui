//! The file preview's code view (D6, docs/43 §2.5; canvas `D6Preview`,
//! `D6StatesFiles` "Preview · code, revealed"). On the sheet, no card: rows
//! 48 / 1fr with a 12 gap and 12 at the right, padding-top 12; the gutter in
//! faint mono 12.1 right-aligned; the code in mono 12.65 / 20, no wrap (the
//! list scrolls sideways); the transcript's highlighter. A reveal (a
//! `path#L…` link) bands its lines at white 7 % (black 5 % in light) full
//! width, numbers in the foreground, and lands with the band three lines
//! from the top.
//!
//! Every file is a virtual list: only the visible rows are built. The host
//! splits the text once when the file loads and passes the lines in an `Arc`,
//! so a render never re-splits or clones the file.

use crate::util::ScrollAxisLock as _;
use std::ops::Range;
use std::sync::Arc;

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{
    div, prelude::*, px, uniform_list, App, ElementId, IntoElement, ListHorizontalSizingBehavior, ScrollStrategy, SharedString, StyledText,
    UniformListScrollHandle, Window,
};
use gpui_kit::base::h_flex;

use crate::transcript::syntax_runs_in;

/// The gutter's width and the gap after it.
const GUTTER_W: f32 = 48.0;
const GUTTER_GAP: f32 = 12.0;
/// Space at the right of the longest line; above the first.
const PAD_RIGHT: f32 = 12.0;
const PAD_TOP: f32 = 12.0;
/// Gutter and code type.
const GUTTER_TEXT: f32 = 12.1;
const CODE_TEXT: f32 = 12.65;
const LINE_H: f32 = 20.0;
/// A reveal lands with its band this many lines below the top.
pub const REVEAL_LEAD_LINES: usize = 3;
/// Frames a reveal keeps asking for before settling anyway.
const SCROLL_ATTEMPTS: usize = 10;

/// The pane's code view. Build with [`code_view`].
#[derive(IntoElement)]
pub struct CodeView {
    id: ElementId,
    lines: Arc<Vec<String>>,
    language: Option<SharedString>,
    band: Option<Range<u32>>,
    reveal_token: u64,
    scroll: Option<UniformListScrollHandle>,
}

/// A code view over `lines` (the file split once, newlines stripped).
pub fn code_view(id: impl Into<ElementId>, lines: Arc<Vec<String>>) -> CodeView {
    CodeView { id: id.into(), lines, language: None, band: None, reveal_token: 0, scroll: None }
}

/// The row a reveal of `band` scrolls to the top: three lines above the
/// band's first line (line numbers start at 1), clamped to the file.
pub fn reveal_top_row(band: &Range<u32>, total: usize) -> usize {
    let first = band.start.saturating_sub(1) as usize;
    first.saturating_sub(REVEAL_LEAD_LINES).min(total.saturating_sub(1))
}

impl CodeView {
    /// The highlighter's language.
    pub fn language(mut self, language: impl Into<SharedString>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Bands lines `start..end` (1-based, end exclusive) and reveals them.
    /// The reveal fires once per `(band, token)`; pass a new token to reveal
    /// the same lines again after the reader scrolled away.
    pub fn band(mut self, band: Range<u32>, token: u64) -> Self {
        self.band = Some(band);
        self.reveal_token = token;
        self
    }

    /// The host's scroll handle, so the position survives the strip's slide
    /// and a push / pop of the view.
    pub fn track_scroll(mut self, handle: &UniformListScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }
}

#[derive(Clone, Default)]
struct RevealState {
    settled: Option<(u32, u64)>,
    waiting: Option<(u32, u64)>,
    attempts: usize,
}

impl RenderOnce for CodeView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let total = self.lines.len();
        let handle = match self.scroll {
            Some(h) => h,
            None => window.use_keyed_state((id.clone(), "scroll"), cx, |_, _| UniformListScrollHandle::default()).read(cx).clone(),
        };
        if let Some(band) = self.band.clone().filter(|b| !b.is_empty() && total > 0) {
            let wanted = (band.start, self.reveal_token);
            let state = window.use_keyed_state((id.clone(), "reveal"), cx, |_, _| RevealState::default());
            let current = state.read(cx).clone();
            if current.settled != Some(wanted) {
                let attempts = if current.waiting == Some(wanted) { current.attempts } else { 0 };
                if attempts >= SCROLL_ATTEMPTS {
                    state.update(cx, |s, _| {
                        s.settled = Some(wanted);
                        s.waiting = None;
                    });
                } else {
                    handle.scroll_to_item_strict(reveal_top_row(&band, total), ScrollStrategy::Top);
                    state.update(cx, |s, _| {
                        s.waiting = Some(wanted);
                        s.attempts = attempts + 1;
                    });
                    // One frame for the deferred scroll to land; bounded above.
                    if attempts + 1 >= 2 {
                        state.update(cx, |s, _| {
                            s.settled = Some(wanted);
                            s.waiting = None;
                        });
                    } else {
                        aui_motion::request_frame(window);
                    }
                }
            }
        }

        // The widest line sizes the sideways scroll.
        let widest = self.lines.iter().enumerate().max_by_key(|(_, l)| l.chars().count()).map(|(i, _)| i);
        let lines = self.lines.clone();
        let language = self.language.clone();
        let band = self.band.clone();
        let list = uniform_list((id.clone(), "lines"), total, move |range, _window, cx| {
            let p = cx.aui().colors;
            let band_ground = if p.is_dark() { gpui::white().alpha(0.07) } else { gpui::black().alpha(0.05) };
            range
                .map(|i| {
                    let line_no = i as u32 + 1;
                    let banded = band.as_ref().is_some_and(|b| b.contains(&line_no));
                    let line = lines[i].as_str();
                    let text = if line.is_empty() { " ".to_string() } else { line.replace('\t', "    ") };
                    let runs = if line.is_empty() { Vec::new() } else { syntax_runs_in(&text, language.as_deref(), &p, scale::FONT_MONO) };
                    let styled = if runs.is_empty() { StyledText::new(text) } else { StyledText::new(text).with_runs(runs) };
                    h_flex()
                        .h(px(LINE_H))
                        .min_w_full()
                        .pr(px(PAD_RIGHT))
                        .gap(px(GUTTER_GAP))
                        .items_center()
                        .whitespace_nowrap()
                        .when(banded, |d| d.bg(band_ground))
                        .child(
                            div()
                                .w(px(GUTTER_W))
                                .flex_none()
                                .text_right()
                                .font_family(scale::FONT_MONO)
                                .text_px(GUTTER_TEXT)
                                .text_color(if banded { p.fg } else { p.ink_4 })
                                .child(line_no.to_string()),
                        )
                        .child(div().flex_none().font_family(scale::FONT_MONO).text_px(CODE_TEXT).line_height(px(LINE_H)).text_color(p.term_fg).child(styled))
                })
                .collect()
        })
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
        .scroll_axis_locked()
        .with_width_from_item(widest)
        .track_scroll(&handle)
        .size_full();
        div().id(id).size_full().pt(px(PAD_TOP)).role(gpui::Role::Document).aria_label("File contents").child(list)
    }
}


/// The file preview's Markdown (docs/43 §2.5): it reads like an answer —
/// the transcript's renderer at the answer's scale (body 16 / 26, h1 24 / 32,
/// h2 20 / 28 with 28 above and 10 below, code blocks Raised), inline code on
/// the 9 % overlay, padded 16 / 20 / 24 on the sheet. Not the white page.
pub fn markdown_view(id: impl Into<ElementId>, source: impl Into<SharedString>, cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    let style = crate::transcript::ProseStyle {
        ink: p.ink,
        code_ink: p.ink,
        code_bg: p.selected_overlay(),
        // The answer's body (turns.rs BODY_TEXT / BODY_LH / PARAGRAPH_GAP).
        size: 16.0 / 1.1,
        line_height: 26.0 / 16.0,
        paragraph_gap: 16.0,
    };
    div()
        .w_full()
        .pt(px(16.0))
        .px(px(20.0))
        .pb(px(24.0))
        .child(crate::transcript::markdown(id, source, style).settled(true))
}

/// The preview's loading body (docs/43 §2.5): the gutter and line bars at the
/// selected overlay, on the code view's grid; no spinner.
pub fn code_skeleton(widths: &[f32], cx: &App) -> gpui::Div {
    let sel = cx.aui().colors.selected_overlay();
    gpui_kit::base::v_flex().w_full().pt(px(PAD_TOP)).children(widths.iter().map(|&w| {
        h_flex()
            .h(px(LINE_H))
            .gap(px(GUTTER_GAP))
            .items_center()
            .child(h_flex().w(px(GUTTER_W)).flex_none().justify_end().child(div().w(px(18.0)).h(px(10.0)).rounded(px(5.0)).bg(sel)))
            .child(div().w(px(w)).h(px(10.0)).rounded(px(5.0)).bg(sel))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reveal_lands_three_lines_above_its_band() {
        assert_eq!(reveal_top_row(&(2953..2956), 5000), 2949);
        assert_eq!(reveal_top_row(&(2..3), 10), 0);
        assert_eq!(reveal_top_row(&(40..41), 20), 19);
    }
}
