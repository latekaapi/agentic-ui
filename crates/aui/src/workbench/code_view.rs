//! The file preview's code view (D6, docs/43 §2.5; canvas `D6Preview`,
//! `D6StatesFiles` "Preview · code, revealed"). On the sheet, no card: rows
//! 48 / 1fr with a 12 gap and 12 at the right, padding-top 12; the gutter in
//! faint mono 12.1 right-aligned; the code in mono 12.65 / 20; the
//! transcript's highlighter. A reveal (a `path#L…` link) bands its lines at
//! white 7 % (black 5 % in light) full width, numbers in the foreground, and
//! lands with the band three lines from the top.
//!
//! Long lines soft-wrap by default, as editors do (owner 2026-10-31: "some
//! max width and some wrapping"): at the view's own width, breaking after a
//! space or punctuation where it can (VS Code's break characters), else
//! mid-word. A continuation row sits under its line's own indentation, its
//! gutter is empty (the number is on the first row only), and a band covers
//! every row of a banded line. [`CodeView::soft_wrap`]`(false)` is the old
//! behaviour: one row per line, the list scrolling sideways.
//!
//! Every file is a virtual list: only the visible rows are built. The host
//! splits the text once when the file loads and passes the lines in an `Arc`,
//! so a render never re-splits or clones the file; the wrapped rows are kept
//! per (file, columns) and rebuilt only when either changes.

use crate::util::ScrollAxisLock as _;
use std::ops::Range;
use std::sync::Arc;

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{
    canvas, div, prelude::*, px, uniform_list, App, ElementId, IntoElement, ListHorizontalSizingBehavior, ScrollStrategy, SharedString, StyledText,
    TextRun, UniformListScrollHandle, Window,
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
/// Fewer columns than this and the view does not wrap at all (a pane
/// dragged to a sliver would otherwise stack one character per row).
const MIN_WRAP_COLS: usize = 16;

/// The pane's code view. Build with [`code_view`].
#[derive(IntoElement)]
pub struct CodeView {
    id: ElementId,
    lines: Arc<Vec<String>>,
    language: Option<SharedString>,
    band: Option<Range<u32>>,
    reveal_token: u64,
    scroll: Option<UniformListScrollHandle>,
    wrap: bool,
    width_hint: Option<f32>,
}

/// A code view over `lines` (the file split once, newlines stripped).
pub fn code_view(id: impl Into<ElementId>, lines: Arc<Vec<String>>) -> CodeView {
    CodeView { id: id.into(), lines, language: None, band: None, reveal_token: 0, scroll: None, wrap: true, width_hint: None }
}

/// The row a reveal of `band` scrolls to the top: three lines above the
/// band's first line (line numbers start at 1), clamped to the file.
pub fn reveal_top_row(band: &Range<u32>, total: usize) -> usize {
    let first = band.start.saturating_sub(1) as usize;
    first.saturating_sub(REVEAL_LEAD_LINES).min(total.saturating_sub(1))
}

/// A line as the view draws it: tabs as four spaces.
fn expand_tabs(line: &str) -> String {
    line.replace('\t', "    ")
}

/// One visual row of a wrapped file: `line` (0-based) and the byte range of
/// its tab-expanded text the row shows, with the continuation indent (in
/// columns) a row after the first sits under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrapRow {
    /// The file line, 0-based.
    pub line: usize,
    /// Byte range in the line's tab-expanded text.
    pub start: usize,
    /// End of that range (exclusive).
    pub end: usize,
    /// Columns of indent a continuation row is drawn with (0 on a first row).
    pub indent: usize,
}

impl WrapRow {
    /// Whether this row starts its line (it carries the line number).
    pub fn first(&self) -> bool {
        self.start == 0
    }
}

/// A break may come after one of these (VS Code's
/// `wordWrapBreakAfterCharacters`, its ASCII part)…
fn breaks_after(c: char) -> bool {
    matches!(c, ' ' | '\t' | '}' | ')' | ']' | '?' | '|' | '/' | '&' | '.' | ',' | ';' | ':')
}

/// …or before one of these (`wordWrapBreakBeforeCharacters`).
fn breaks_before(c: char) -> bool {
    matches!(c, '(' | '[' | '{' | '+')
}

/// Wraps one tab-expanded line at `cols` columns into `out`.
fn wrap_line(text: &str, cols: usize, line: usize, out: &mut Vec<WrapRow>) {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = chars.len();
    if cols < MIN_WRAP_COLS || n <= cols {
        out.push(WrapRow { line, start: 0, end: text.len(), indent: 0 });
        return;
    }
    // Continuations sit under the line's own indentation (VS Code's
    // `wrappingIndent: same`) while that leaves at least half the width.
    let lead = chars.iter().take_while(|(_, c)| *c == ' ').count();
    let indent = if cols.saturating_sub(lead) >= cols / 2 { lead } else { 0 };
    let byte = |i: usize| if i >= n { text.len() } else { chars[i].0 };
    let mut at = 0;
    loop {
        let width = if at == 0 { cols } else { cols - indent };
        if n - at <= width {
            out.push(WrapRow { line, start: byte(at), end: text.len(), indent: if at == 0 { 0 } else { indent } });
            return;
        }
        let limit = at + width;
        // The last break opportunity in the row's second half, else a hard
        // break at the edge.
        let floor = at + width / 2;
        let mut end = (floor + 1..=limit)
            .rev()
            .find(|&j| breaks_after(chars[j - 1].1) || breaks_before(chars[j].1))
            .unwrap_or(limit);
        // Spaces at a break hang at the row's end (invisible), so the next
        // row starts on text.
        while end < n && chars[end].1 == ' ' {
            end += 1;
        }
        if end >= n {
            out.push(WrapRow { line, start: byte(at), end: text.len(), indent: if at == 0 { 0 } else { indent } });
            return;
        }
        out.push(WrapRow { line, start: byte(at), end: byte(end), indent: if at == 0 { 0 } else { indent } });
        at = end;
    }
}

/// Every visual row of `lines` wrapped at `cols` columns (`cols` under
/// [`MIN_WRAP_COLS`] wraps nothing: one row per line).
pub fn wrap_rows(lines: &[String], cols: usize) -> Vec<WrapRow> {
    let mut out = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        wrap_line(&expand_tabs(line), cols, i, &mut out);
    }
    out
}

/// The highlighter's runs for a whole line, cut to the bytes `start..end`.
fn slice_runs(runs: &[TextRun], start: usize, end: usize) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut at = 0;
    for run in runs {
        let (lo, hi) = (at.max(start), (at + run.len).min(end));
        if lo < hi {
            out.push(TextRun { len: hi - lo, ..run.clone() });
        }
        at += run.len;
        if at >= end {
            break;
        }
    }
    out
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

    /// Soft-wrap long lines at the view's width (the default), or keep one
    /// row per line and scroll sideways (`false`).
    pub fn soft_wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// The width the view expects to get, used to wrap the very first frame
    /// before the view has measured itself (no reflow on open).
    pub fn width_hint(mut self, width: f32) -> Self {
        self.width_hint = Some(width);
        self
    }
}

#[derive(Clone, Default)]
struct RevealState {
    settled: Option<(u32, u64)>,
    waiting: Option<(u32, u64)>,
    attempts: usize,
}

/// The rows of the file last wrapped, keyed by (file, columns).
#[derive(Default)]
struct WrapCache {
    key: Option<(usize, usize, usize)>,
    rows: Arc<Vec<WrapRow>>,
    /// The visual row each file line starts on.
    first_row: Arc<Vec<usize>>,
}

/// The view's own laid-out width, measured each frame.
#[derive(Default)]
struct Measured(Option<f32>);

impl RenderOnce for CodeView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let total = self.lines.len();
        let handle = match self.scroll {
            Some(h) => h,
            None => window.use_keyed_state((id.clone(), "scroll"), cx, |_, _| UniformListScrollHandle::default()).read(cx).clone(),
        };

        // Soft wrap: the columns that fit, from the measured width (or the
        // host's hint before the first measure) and the mono advance.
        let measured = window.use_keyed_state((id.clone(), "width"), cx, |_, _| Measured::default());
        let wrapped = if self.wrap {
            let width = measured.read(cx).0.or(self.width_hint).unwrap_or(0.0);
            let font_id = window.text_system().resolve_font(&gpui::font(scale::FONT_MONO));
            let advance = window
                .text_system()
                .advance(font_id, px(CODE_TEXT), 'm')
                .map(|size| f32::from(size.width))
                .ok()
                .filter(|w| *w > 0.0)
                .unwrap_or(CODE_TEXT * 0.6);
            let cols = ((width - GUTTER_W - GUTTER_GAP - PAD_RIGHT).max(0.0) / advance).floor() as usize;
            let cache = window.use_keyed_state((id.clone(), "wrap"), cx, |_, _| WrapCache::default());
            let key = (Arc::as_ptr(&self.lines) as usize, total, cols);
            if cache.read(cx).key != Some(key) {
                let rows = wrap_rows(&self.lines, cols);
                let mut first_row = Vec::with_capacity(total);
                for (ix, row) in rows.iter().enumerate() {
                    if row.first() {
                        first_row.push(ix);
                    }
                }
                cache.update(cx, |c, _| {
                    c.key = Some(key);
                    c.rows = Arc::new(rows);
                    c.first_row = Arc::new(first_row);
                });
            }
            let c = cache.read(cx);
            Some((c.rows.clone(), c.first_row.clone()))
        } else {
            None
        };
        let row_count = wrapped.as_ref().map_or(total, |(rows, _)| rows.len());

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
                    let top = reveal_top_row(&band, total);
                    let top = wrapped.as_ref().and_then(|(_, first)| first.get(top).copied()).unwrap_or(top);
                    handle.scroll_to_item_strict(top, ScrollStrategy::Top);
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

        let lines = self.lines.clone();
        let language = self.language.clone();
        let band = self.band.clone();
        let rows = wrapped.as_ref().map(|(rows, _)| rows.clone());
        let list = uniform_list((id.clone(), "lines"), row_count, move |range, window, cx| {
            let p = cx.aui().colors;
            let band_ground = if p.is_dark() { gpui::white().alpha(0.07) } else { gpui::black().alpha(0.05) };
            let advance = rows.as_ref().map(|_| {
                let font_id = window.text_system().resolve_font(&gpui::font(scale::FONT_MONO));
                window.text_system().advance(font_id, px(CODE_TEXT), 'm').map(|s| f32::from(s.width)).unwrap_or(CODE_TEXT * 0.6)
            });
            range
                .map(|ix| {
                    // The row: a whole line, or one wrapped piece of one.
                    let (line_ix, piece, indent) = match &rows {
                        Some(rows) => {
                            let row = &rows[ix];
                            (row.line, Some((row.start, row.end, row.first())), row.indent)
                        }
                        None => (ix, None, 0),
                    };
                    let line_no = line_ix as u32 + 1;
                    let first = piece.is_none_or(|(_, _, first)| first);
                    let banded = band.as_ref().is_some_and(|b| b.contains(&line_no));
                    let line = lines[line_ix].as_str();
                    let full = expand_tabs(line);
                    let (start, end) = piece.map_or((0, full.len()), |(s, e, _)| (s, e));
                    let runs = if full.is_empty() { Vec::new() } else { syntax_runs_in(&full, language.as_deref(), &p, scale::FONT_MONO) };
                    let (text, runs) = if piece.is_some() { (full[start..end].to_string(), slice_runs(&runs, start, end)) } else { (full, runs) };
                    let text = if text.is_empty() { " ".to_string() } else { text };
                    let styled = if runs.is_empty() || runs.iter().map(|r| r.len).sum::<usize>() != text.len() {
                        StyledText::new(text)
                    } else {
                        StyledText::new(text).with_runs(runs)
                    };
                    let indent_px = advance.map_or(0.0, |a| a * indent as f32);
                    h_flex()
                        .h(px(LINE_H))
                        .when(rows.is_some(), |d| d.w_full().overflow_hidden())
                        .when(rows.is_none(), |d| d.min_w_full())
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
                                // The number on a line's first row only.
                                .when(first, |d| d.child(line_no.to_string())),
                        )
                        .child(
                            div()
                                .flex_none()
                                .pl(px(indent_px))
                                .font_family(scale::FONT_MONO)
                                .text_px(CODE_TEXT)
                                .line_height(px(LINE_H))
                                .text_color(p.term_fg)
                                .child(styled),
                        )
                })
                .collect()
        })
        .scroll_axis_locked()
        .track_scroll(&handle)
        .size_full();
        let list = if wrapped.is_some() {
            list
        } else {
            // The widest line sizes the sideways scroll.
            let widest = self.lines.iter().enumerate().max_by_key(|(_, l)| l.chars().count()).map(|(i, _)| i);
            list.with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained).with_width_from_item(widest)
        };
        // Measures the view each frame; a new width re-wraps on the next.
        let probe = canvas(
            move |bounds, _window, cx| {
                let width = f32::from(bounds.size.width);
                if measured.read(cx).0.is_none_or(|w| (w - width).abs() > 0.5) {
                    measured.update(cx, |m, cx| {
                        m.0 = Some(width);
                        cx.notify();
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        div()
            .id(id)
            .relative()
            .size_full()
            .pt(px(PAD_TOP))
            .role(gpui::Role::Document)
            .aria_label("File contents")
            .child(probe)
            .child(list)
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

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|s| s.to_string()).collect()
    }

    /// The pieces each row shows, for reading a wrap at a glance.
    fn pieces(lines: &[String], cols: usize) -> Vec<(usize, String, usize)> {
        wrap_rows(lines, cols).iter().map(|r| (r.line, expand_tabs(&lines[r.line])[r.start..r.end].to_string(), r.indent)).collect()
    }

    #[test]
    fn short_lines_are_one_row_each() {
        let l = lines(&["fn main() {", "", "}"]);
        let rows = wrap_rows(&l, 40);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(WrapRow::first));
    }

    #[test]
    fn a_long_line_breaks_after_a_space_and_keeps_its_text() {
        let l = lines(&["alpha beta gamma delta epsilon zeta eta theta"]);
        let got = pieces(&l, 20);
        assert_eq!(got.iter().map(|(_, s, _)| s.as_str()).collect::<String>(), l[0], "no character lost or doubled");
        assert_eq!(got[0].1, "alpha beta gamma ");
        assert!(got.iter().all(|(_, s, _)| s.trim_end().chars().count() <= 20), "{got:?}");
        assert!(got.len() > 1);
    }

    #[test]
    fn continuation_rows_sit_under_the_line_indentation() {
        let l = lines(&["        call(argument_one, argument_two, argument_three, argument_four)"]);
        let rows = wrap_rows(&l, 32);
        assert!(rows.len() > 1);
        assert_eq!(rows[0].indent, 0);
        assert!(rows[1..].iter().all(|r| r.indent == 8), "{rows:?}");
        // Each continuation fits the width left after its indent.
        let text = &l[0];
        for r in &rows[1..] {
            assert!(text[r.start..r.end].trim_end().chars().count() <= 32 - 8);
        }
    }

    #[test]
    fn a_word_longer_than_the_row_breaks_mid_word() {
        let word = "x".repeat(50);
        let l = lines(&[&word]);
        let got = pieces(&l, 20);
        assert_eq!(got.iter().map(|(_, s, _)| s.len()).collect::<Vec<_>>(), vec![20, 20, 10]);
    }

    #[test]
    fn a_too_narrow_view_does_not_wrap() {
        let l = lines(&["a fairly long line of code that would wrap"]);
        assert_eq!(wrap_rows(&l, MIN_WRAP_COLS - 1).len(), 1);
    }

    #[test]
    fn tabs_wrap_as_four_spaces() {
        let l = lines(&["\tlet value = compute(first_argument, second_argument);"]);
        let rows = wrap_rows(&l, 24);
        assert!(rows.len() > 1);
        assert!(rows[1..].iter().all(|r| r.indent == 4));
    }

    #[test]
    fn runs_slice_to_the_row() {
        let run = |len| TextRun { len, font: gpui::font("Menlo"), color: gpui::black(), background_color: None, underline: None, strikethrough: None };
        let runs = vec![run(5), run(3), run(10)];
        let cut = slice_runs(&runs, 4, 12);
        assert_eq!(cut.iter().map(|r| r.len).collect::<Vec<_>>(), vec![1, 3, 4]);
    }

    #[test]
    fn a_reveal_lands_three_lines_above_its_band() {
        assert_eq!(reveal_top_row(&(2953..2956), 5000), 2949);
        assert_eq!(reveal_top_row(&(2..3), 10), 0);
        assert_eq!(reveal_top_row(&(40..41), 20), 19);
    }
}
