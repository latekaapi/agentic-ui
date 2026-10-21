//! Tool call cards (D4, docs/41 §2.5): one header for every tool — the
//! status glyph (a success check, a danger ✕, or the spinner), the verb in
//! the foreground, the target in mono muted, and at the right the result and
//! the duration in faint mono and the chevron — and a body per tool: shell
//! output and a read file on the Well, the unified diff on the card's own
//! Raised, search hits, web results, the browser strip with its click ring,
//! the sub-agent's lines and the MCP parameters. No buttons: the header is
//! the toggle, and a long body's "N more lines" row expands it in place.

use aui_motion::{looping, Loop};
use aui_protocol::{Block, DiffKind, DiffStat, ToolBody, ToolStatus, Turn};
use aui_tokens::{scale, ActiveAui, AuiStyled, Easing, Palette};
use gpui::{div, linear_color_stop, linear_gradient, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, StyledText, Window};
use gpui_kit::base::{h_flex, v_flex};
use std::cell::RefCell;

use crate::data::{record_ax_label, spinner};
use crate::icons::{icon, IconName};
use crate::transcript::ansi::{ansi_runs, ansi_spans};
use crate::transcript::caps::{cap_expanded, cut_preview_line, Remainder};
use crate::transcript::selectable::{intersect_range, line_span_event, selectable_text, SelectionKey, SpanEvent};
use crate::transcript::quiet_row::{status_check, status_cross, STATUS_GLYPH};
use crate::transcript::transcript_card;

/// Output on the Well: padding 10 / 12, mono, line 20.
const OUT_PAD_Y: f32 = 10.0;
const OUT_PAD_X: f32 = 12.0;
const OUT_TEXT: f32 = 11.5;
const OUT_LH: f32 = 1.65;
/// Shell output folds after this many lines (the card shows six, then `14 more lines`).
/// The six are always the LAST lines — the tail — whether the command is
/// still running or settled, so a live card follows the fresh output instead
/// of pinning the head.
pub const SHELL_FOLD: usize = 6;
/// Search and MCP bodies cap at the same height as the generic item card's
/// expanded body, so no long result is ever taller than ~40 lines inline.
pub const SEARCH_BODY_CAP: usize = 40;
/// Lines of `result_json` shown before the `N more lines` fold row.
pub const MCP_BODY_CAP: usize = 40;
/// The "N more lines" row: 24 px, caption muted, an 11 px chevron.
const MORE_H: f32 = 24.0;
const MORE_GAP: f32 = 6.0;
const MORE_GLYPH: f32 = 11.0;
/// The unified diff: 6 / 0 padding; rows `36 / 20 / 1fr` with an 8 gap,
/// padded 0 / 12 / 0 / 8.
const DIFF_PAD_Y: f32 = 6.0;
const DIFF_LH: f32 = 1.6;
const DIFF_OLD_W: f32 = 36.0;
const DIFF_NEW_W: f32 = 20.0;
const DIFF_GAP: f32 = 8.0;
const DIFF_PAD_LEFT: f32 = 8.0;
const DIFF_PAD_RIGHT: f32 = 12.0;
/// Search hits and the MCP list.
const HITS_PAD_Y: f32 = 8.0;
const HITS_PAD_X: f32 = 12.0;
const HITS_LH: f32 = 1.7;
/// Web rows: a 14 px favicon tile, radius 3.
const WEB_PAD_Y: f32 = 8.0;
const WEB_PAD_X: f32 = 12.0;
const WEB_GAP: f32 = 6.0;
const FAVICON: f32 = 14.0;
const FAVICON_RADIUS: f32 = 3.0;
/// The browser strip: 70 px, radius 6, margins 8 / 10; the 18 px click ring
/// (2 px foreground, 1 px 40 % black outside); the action in a dark cap.
const SHOT_MARGIN_Y: f32 = 8.0;
const SHOT_MARGIN_X: f32 = 10.0;
const SHOT_H: f32 = 70.0;
const SHOT_RADIUS: f32 = 6.0;
const RING: f32 = 18.0;
const RING_STROKE: f32 = 2.0;
const RING_X: f32 = 0.6;
const RING_Y: f32 = 0.4;
const RING_PERIOD: std::time::Duration = std::time::Duration::from_millis(1600);
const SHOT_LABEL_LEFT: f32 = 10.0;
const SHOT_LABEL_BOTTOM: f32 = 8.0;
const SHOT_LABEL_PAD_Y: f32 = 2.0;
const SHOT_LABEL_PAD_X: f32 = 6.0;
/// Delegated: two columns, who 64 px muted, the text; caption; 10 / 12.
const SUB_PAD_Y: f32 = 10.0;
const SUB_PAD_X: f32 = 12.0;
const SUB_WHO_W: f32 = 64.0;
/// MCP: key column 92 px.
const MCP_KEY_W: f32 = 92.0;
/// The faint mono of header facts (`exit 0`, `0.1 s`, `3 lines`).
const META_TEXT: f32 = 11.0;

/// What the card's header and fold rows ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCardIntent {
    /// Toggle the body (the header).
    Toggle,
    /// Show the whole body in place (the "N more lines" row).
    Unfold,
}

type IntentHandler = std::rc::Rc<dyn Fn(ToolCardIntent, &mut Window, &mut App)>;
type LinkHandler = std::rc::Rc<dyn Fn(String, &mut Window, &mut App)>;

/// A click handler bound to one [`ToolCardIntent`], ready for `on_click`.
type IntentClick = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)>;

/// A tool call card. Build with [`tool_card`].
#[derive(IntoElement)]
pub struct ToolCard {
    id: ElementId,
    verb: SharedString,
    target: SharedString,
    status: ToolStatus,
    duration: Option<SharedString>,
    body: ToolBody,
    diff_stat: Option<DiffStat>,
    open: bool,
    unfolded: bool,
    flat: bool,
    on_intent: Option<IntentHandler>,
    on_link: Option<LinkHandler>,
    select: Option<OutSelect>,
}

type OutSpanHandler = std::rc::Rc<dyn Fn(SpanEvent, &mut Window, &mut App)>;

/// The output Well as one selectable cell ([`ToolCard::selectable_output`]).
#[derive(Clone)]
struct OutSelect {
    key: SelectionKey,
    range: Option<std::ops::Range<usize>>,
    color: Option<gpui::Hsla>,
    strength: f32,
    on_span: Option<OutSpanHandler>,
}

/// A card for one tool call.
pub fn tool_card(id: impl Into<ElementId>, verb: impl Into<SharedString>, target: impl Into<SharedString>, status: ToolStatus, body: ToolBody) -> ToolCard {
    ToolCard {
        id: id.into(),
        verb: verb.into(),
        target: target.into(),
        status,
        duration: None,
        body,
        diff_stat: None,
        open: true,
        unfolded: false,
        flat: false,
        on_intent: None,
        on_link: None,
        select: None,
    }
}

impl ToolCard {
    /// The duration shown in the header, formatted by [`format_duration`]
    /// once per card rather than once per frame.
    pub fn duration_ms(mut self, ms: Option<u64>) -> Self {
        self.duration = ms.map(format_duration);
        self
    }

    /// Server-authored `+N`/`−N` counts for the header, drawn without a
    /// [`ToolBody::Edit`] diff: the provider's whole-patch summary. `None`
    /// (the default) draws no chips. When both this and an `Edit` diff are
    /// present the summary wins and the body draws no second pair.
    pub fn diff_stat(mut self, stat: Option<DiffStat>) -> Self {
        self.diff_stat = stat;
        self
    }

    /// Whether the body is shown.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Whether a long body shows all of itself (after its "N more lines"
    /// row was pressed): the host keeps the flag, as it keeps `open`.
    pub fn unfolded(mut self, unfolded: bool) -> Self {
        self.unfolded = unfolded;
        self
    }

    /// Draws the card as a row inside another card (a tool group's call):
    /// no surface of its own, a 7 % rule above it.
    pub fn flat(mut self, flat: bool) -> Self {
        self.flat = flat;
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(ToolCardIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }

    /// Makes a shell or read body's output selectable as one cell, `key`
    /// (D6 follow-up; docs/43 §0: a selection "inside a single code block or
    /// Well output is a code quote"). Offsets are bytes over
    /// [`output_text`] — the ANSI-stripped lines joined by newlines, folded
    /// lines included — so they stay put when the body unfolds. `range` is
    /// the visible selection there; `on_span` gets the line events remapped
    /// to the whole output, as a fence's lines do.
    pub fn selectable_output(
        mut self,
        key: SelectionKey,
        range: Option<std::ops::Range<usize>>,
        color: Option<gpui::Hsla>,
        on_span: impl Fn(SpanEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.select = Some(OutSelect { key, range, color, strength: 1.0, on_span: Some(std::rc::Rc::new(on_span)) });
        self
    }

    /// The output selection's tint at `strength` (0–1), for a host that
    /// flashes a remembered span (a sent quote's source) fading out.
    pub fn output_selection_strength(mut self, strength: f32) -> Self {
        if let Some(select) = &mut self.select {
            select.strength = strength;
        }
        self
    }

    /// A web result's title was pressed: the handler gets its URL. Without
    /// one the titles are plain text.
    pub fn on_link(mut self, f: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(std::rc::Rc::new(f));
        self
    }
}

/// `12.4 s`, `1 m 12 s`, `0.3 s`.
///
/// Rounded to tenths *before* the branch: 59 999 ms reads `1 m 00 s`, not the
/// `60.0 s` a raw comparison would print.
pub fn format_duration(ms: u64) -> SharedString {
    let tenths = (ms + 50) / 100;
    if tenths >= 600 {
        let s = tenths / 10;
        format!("{} m {:02} s", s / 60, s % 60).into()
    } else {
        format!("{}.{} s", tenths / 10, tenths % 10).into()
    }
}

/// `+N` / `−N` chip labels for a server-authored [`DiffStat`], drawn with the
/// same tag component and success/danger colours as the [`ToolBody::Edit`]
/// arm so the two paths are indistinguishable. The minus is U+2212, as there.
fn diff_stat_tags(stat: &DiffStat) -> (String, String) {
    (format!("+{}", stat.added), format!("−{}", stat.removed))
}

/// File-count label for a server-authored [`DiffStat`]: `{n} files`, in the
/// same mono ink-3 style the `Read`/`Search` arms use for `{n} lines` /
/// `{n} hits`. Drawn only when the patch spans more than one file.
fn diff_stat_files_label(stat: &DiffStat) -> Option<String> {
    if stat.files > 1 {
        Some(format!("{} files", stat.files))
    } else {
        None
    }
}

/// Whether the [`ToolBody::Edit`] arm draws its own chips: never when a
/// server-authored stat is present, so the header shows exactly one pair.
fn edit_draws_chips(diff_stat: Option<&DiffStat>) -> bool {
    diff_stat.is_none()
}

// Test probe for the `+N`/`−N` header chips: gpui offers no text query and
// tags paint no quads, so a test arms this, draws a card in a real window,
// and reads back the labels the render committed — the chip analogue of
// `Window::painted_quads`. Arming records; it changes nothing drawn.
thread_local! {
    static CHIP_PROBE_ARMED: RefCell<bool> = const { RefCell::new(false) };
    static DRAWN_CHIPS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Arms (`true`) or disarms the chip probe. Disarmed — the default — nothing
/// is recorded and rendering is byte-for-byte what it was.
pub fn arm_chip_probe(armed: bool) {
    CHIP_PROBE_ARMED.with(|flag| *flag.borrow_mut() = armed);
}

/// Drains the chip labels recorded since the last call, in draw order.
pub fn take_drawn_chips() -> Vec<String> {
    DRAWN_CHIPS.with(|chips| std::mem::take(&mut *chips.borrow_mut()))
}

/// Records one drawn chip label when the probe is armed; a no-op otherwise.
fn record_chip(label: &str) {
    if CHIP_PROBE_ARMED.with(|flag| *flag.borrow()) {
        DRAWN_CHIPS.with(|chips| chips.borrow_mut().push(label.to_owned()));
    }
}

impl RenderOnce for ToolCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let handler = self.on_intent.clone();
        let emit = move |intent: ToolCardIntent| -> IntentClick {
            let handler = handler.clone();
            Box::new(move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                if let Some(h) = &handler {
                    h(intent, w, cx)
                }
            })
        };

        let glyph: gpui::AnyElement = match self.status {
            ToolStatus::Pending | ToolStatus::Running => spinner((id.clone(), "spinner")).size(px(STATUS_GLYPH)).fluid(cx).into_any_element(),
            ToolStatus::Success => status_check(p.success).into_any_element(),
            ToolStatus::Error | ToolStatus::Cancelled => status_cross(p.danger).into_any_element(),
        };
        let mut card = transcript_card(id.clone(), self.open)
            .flat(self.flat)
            .header(glyph)
            .header(div().whitespace_nowrap().child(self.verb.clone()))
            .header(div().min_w(px(0.0)).truncate().mono(scale::FS_12).text_color(p.ink_2).child(self.target.clone()))
            .header(div().flex_1());

        // The right cluster, gap 8: the result, then the duration, in faint
        // mono; `exit N` in danger.
        let mut right = h_flex().flex_none().gap(px(scale::SP_3)).mono(META_TEXT).text_color(p.ink_4);
        let mut show_duration = true;
        let chips = |right: gpui::Div, added: String, removed: String| {
            record_chip(&added);
            record_chip(&removed);
            right.child(h_flex().gap(px(scale::SP_2)).child(div().text_color(p.success).child(added)).child(div().text_color(p.danger).child(removed)))
        };
        // Server-authored counts, drawn without the patch body. They cover
        // every file in the patch while a rendered `Diff` may be one file,
        // so when both are present these win and the `Edit` arm below draws
        // no second pair.
        if let Some(stat) = &self.diff_stat {
            let (added, removed) = diff_stat_tags(stat);
            right = chips(right, added, removed);
            if let Some(files) = diff_stat_files_label(stat) {
                right = right.child(files);
            }
        }
        match &self.body {
            ToolBody::Shell { exit_code, live, .. } => match (live, exit_code) {
                (true, _) => right = right.child("live"),
                (false, Some(0)) => right = right.child("exit 0"),
                (false, Some(code)) => right = right.child(div().text_color(p.danger).child(format!("exit {code}"))),
                (false, None) => {}
            },
            ToolBody::Read { lines, .. } => {
                right = right.child(format!("{lines} lines"));
                show_duration = false;
            }
            ToolBody::Edit { diff } => {
                // Never a second pair: when `diff_stat` is present the chips
                // above already show the whole-patch counts, which can
                // legitimately disagree with this possibly single-file diff.
                if edit_draws_chips(self.diff_stat.as_ref()) {
                    right = chips(right, format!("+{}", diff.added), format!("−{}", diff.removed));
                }
            }
            ToolBody::Search { hits } => right = right.child(format!("{} hits", hits.len())),
            ToolBody::Browser { action, .. } => right = right.child(action.clone()),
            ToolBody::Web { .. } | ToolBody::SubAgent { .. } | ToolBody::Mcp { .. } | ToolBody::None => {}
        }
        if show_duration {
            if let Some(text) = self.duration.clone() {
                right = right.child(text);
            }
        }
        card = card.header(right);

        let unfolded = self.unfolded;
        let body: Option<gpui::AnyElement> = match &self.body {
            ToolBody::Shell { output_lines, .. } => Some(well_body(&p, &id, output_lines, true, unfolded, self.select.as_ref(), emit.clone()).into_any_element()),
            ToolBody::Read { output_lines, .. } if !output_lines.is_empty() => Some(well_body(&p, &id, output_lines, false, unfolded, self.select.as_ref(), emit.clone()).into_any_element()),
            ToolBody::Read { .. } | ToolBody::None => None,
            ToolBody::Edit { diff } => Some(diff_body(&p, diff, unfolded, emit.clone()).into_any_element()),
            ToolBody::Search { hits } => Some(search_body(&p, hits, unfolded, emit.clone()).into_any_element()),
            ToolBody::Web { results, hidden } => Some(web_body(&p, &id, results, *hidden, self.on_link.clone()).into_any_element()),
            ToolBody::Browser { caption, screenshot, .. } => {
                // The click ring pulses only while the call is still in
                // flight; a finished capture is a settled placeholder and
                // must not hold the frame loop open (see `browser_body`).
                let live = matches!(self.status, ToolStatus::Pending | ToolStatus::Running);
                Some(browser_body(&p, &id, screenshot.as_deref(), caption.as_deref(), live, window, cx).into_any_element())
            }
            ToolBody::SubAgent { turns } => Some(sub_agent_body(&p, turns).into_any_element()),
            ToolBody::Mcp { params, result_json } => Some(mcp_body(&p, params, result_json, unfolded, emit.clone()).into_any_element()),
        };
        if let Some(body) = body {
            card = card.body(body);
        }
        let toggle = emit(ToolCardIntent::Toggle);
        card.on_toggle(move |e, w, cx| toggle(e, w, cx))
    }
}

/// The "N more lines" row under a long body: 24 px on the body's own
/// ground, caption muted, an 11 px chevron; it expands the body in place.
fn more_row(p: &Palette, label: String, emit: impl Fn(ToolCardIntent) -> IntentClick) -> impl IntoElement {
    let unfold = emit(ToolCardIntent::Unfold);
    let name = format!("Show {label}");
    record_ax_label(&name);
    h_flex()
        .id("more")
        .w_full()
        .h(px(MORE_H))
        .px(px(OUT_PAD_X))
        .gap(px(MORE_GAP))
        .ui(scale::FS_12)
        .text_color(p.ink_2)
        .cursor_pointer()
        .hover(|d| d.text_color(p.ink))
        .role(gpui::Role::Button)
        .aria_label(name)
        .on_click(move |e, w, cx| unfold(e, w, cx))
        .child(icon(IconName::ChevronDown).size(px(MORE_GLYPH)))
        .child(label)
}

/// The [`SHELL_FOLD`] tail of `lines`: the last lines, oldest first, so a
/// live card follows fresh output. Fewer than the fold shows everything.
pub fn shell_visible_lines(lines: &[String]) -> &[String] {
    let skip = lines.len().saturating_sub(SHELL_FOLD);
    &lines[skip..]
}

/// The first [`SEARCH_BODY_CAP`] hits and the overflow count named by the
/// fold row.
pub fn search_visible_hits(hits: &[aui_protocol::SearchHit]) -> (&[aui_protocol::SearchHit], usize) {
    let hidden = hits.len().saturating_sub(SEARCH_BODY_CAP);
    let shown = &hits[..hits.len() - hidden];
    (shown, hidden)
}

/// The visible `result_json`: the first [`MCP_BODY_CAP`] lines and 6,000
/// characters, with the [`Remainder`] naming what the fold row reports as
/// `N more lines` or `M more characters`. A minified single-line payload is
/// cut, never pretty-printed.
pub fn mcp_visible_text(result_json: &str) -> (String, Remainder) {
    cap_expanded(result_json)
}

/// One search hit's snippet, cut like a preview line so a huge match cannot
/// widen the body: at most 160 characters, with an ellipsis marking the cut.
pub fn search_capped_snippet(snippet: &str) -> String {
    cut_preview_line(snippet)
}

/// A line's text and runs as the Well draws it: ANSI stripped into runs
/// for shell output, one mono run for a read. An empty line is a space.
fn out_text(p: &Palette, line: &str, ansi: bool) -> (String, Vec<gpui::TextRun>) {
    let mono = |len: usize, color| vec![gpui::TextRun { len, font: gpui::font(scale::FONT_MONO), color, background_color: None, underline: None, strikethrough: None }];
    if ansi {
        let (t, runs) = ansi_runs(&ansi_spans(line), p, scale::FONT_MONO);
        if t.is_empty() {
            return (" ".to_string(), mono(1, p.ink));
        }
        if runs.is_empty() || runs.iter().map(|r| r.len).sum::<usize>() != t.len() {
            let len = t.len();
            return (t, mono(len, p.ink));
        }
        (t, runs)
    } else {
        let text = if line.is_empty() { " ".to_string() } else { line.to_string() };
        let len = text.len();
        (text, mono(len, p.ink))
    }
}

/// The output as one text: each line's drawn (ANSI-stripped) text, joined
/// by newlines — what a selection's offsets count over, and what a host
/// copies or quotes from ([`ToolCard::selectable_output`]).
pub fn output_text(lines: &[String], ansi: bool) -> String {
    lines
        .iter()
        .map(|line| if ansi { ansi_runs(&ansi_spans(line), &aui_tokens::dark(), scale::FONT_MONO).0 } else { line.clone() })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One line of output on the Well; selectable when the card is. An empty
/// line draws a space but owns no bytes, so `line_len` is its real length.
#[allow(clippy::too_many_arguments)]
fn out_line(p: &Palette, id: &ElementId, index: usize, line: &str, ansi: bool, line_start: usize, line_len: usize, select: Option<&OutSelect>) -> impl IntoElement {
    let (text, runs) = out_text(p, line, ansi);
    let empty = line_len == 0;
    let body: gpui::AnyElement = match select {
        Some(sel) => {
            let local = sel.range.as_ref().and_then(|range| intersect_range(range, line_start, line_len));
            let mut element = selectable_text((id.clone(), SharedString::from(format!("out-{index}"))), sel.key.clone(), text).runs(runs).selection(local);
            let color = sel.color.unwrap_or(p.selection);
            element = element.selection_color(color.alpha(color.a * sel.strength.clamp(0.0, 1.0)));
            if let Some(emit) = sel.on_span.clone() {
                let key = sel.key.clone();
                element = element.on_span_event(move |event, window, cx| emit(line_span_event(event, &key, line_start, empty), window, cx));
            }
            element.into_any_element()
        }
        None => StyledText::new(text).with_runs(runs).into_any_element(),
    };
    div().w_full().overflow_hidden().whitespace_nowrap().child(body)
}

/// Output lines on the Well: the [`SHELL_FOLD`] tail unless `unfolded`,
/// then the "N more lines" row.
fn well_body(p: &Palette, id: &ElementId, lines: &[String], ansi: bool, unfolded: bool, select: Option<&OutSelect>, emit: impl Fn(ToolCardIntent) -> IntentClick) -> impl IntoElement {
    let skip = if unfolded { 0 } else { lines.len() - shell_visible_lines(lines).len() };
    // Each line's start over the whole output (folded lines included), so
    // a selection keeps its offsets when the body unfolds.
    let mut start = 0usize;
    let mut starts = Vec::with_capacity(lines.len());
    for line in lines {
        let drawn = if ansi { ansi_runs(&ansi_spans(line), p, scale::FONT_MONO).0.len() } else { line.len() };
        starts.push((start, drawn));
        start += drawn + 1;
    }
    let mut out = v_flex().w_full().py(px(OUT_PAD_Y)).px(px(OUT_PAD_X)).mono(OUT_TEXT).line_height(relative(OUT_LH)).text_color(p.ink);
    for (index, line) in lines.iter().enumerate().skip(skip) {
        let (line_start, line_len) = starts[index];
        out = out.child(out_line(p, id, index, line, ansi, line_start, line_len, select));
    }
    let mut body = v_flex().w_full().bg(p.well).child(out);
    if !unfolded && lines.len() > SHELL_FOLD {
        body = body.child(more_row(p, format!("{} more lines", lines.len() - SHELL_FOLD), emit));
    }
    body
}

fn diff_body(p: &Palette, diff: &aui_protocol::Diff, unfolded: bool, emit: impl Fn(ToolCardIntent) -> IntentClick) -> impl IntoElement {
    let mut body = v_flex().w_full().py(px(DIFF_PAD_Y)).mono(OUT_TEXT).line_height(relative(DIFF_LH)).text_color(p.ink);
    let hunks = if unfolded { &diff.hunks[..] } else { &diff.hunks[..diff.hunks.len().min(1)] };
    for hunk in hunks {
        for line in &hunk.lines {
            let bg = match line.kind {
                DiffKind::Add => Some(p.diff_add),
                DiffKind::Del => Some(p.diff_del),
                DiffKind::Context => None,
            };
            let number = |n: Option<u32>, w: f32| div().flex_none().w(px(w)).flex().justify_end().text_color(p.ink_4).child(n.map(|n| n.to_string()).unwrap_or_default());
            let mut row = h_flex().w_full().items_start().gap(px(DIFF_GAP)).pl(px(DIFF_PAD_LEFT)).pr(px(DIFF_PAD_RIGHT));
            if let Some(bg) = bg {
                row = row.bg(bg);
            }
            body = body.child(
                row.child(number(line.old_no, DIFF_OLD_W))
                    .child(number(line.new_no, DIFF_NEW_W))
                    .child(div().flex_1().min_w(px(0.0)).whitespace_nowrap().overflow_hidden().child(line.text.clone())),
            );
        }
    }
    let more_hunks = diff.hunks.len().saturating_sub(1);
    let mut col = v_flex().w_full().child(body);
    if !unfolded && more_hunks > 0 {
        col = col.child(more_row(p, if more_hunks == 1 { "1 more hunk".to_owned() } else { format!("{more_hunks} more hunks") }, emit));
    }
    col
}

fn search_body(p: &Palette, hits: &[aui_protocol::SearchHit], unfolded: bool, emit: impl Fn(ToolCardIntent) -> IntentClick) -> impl IntoElement {
    let (shown, hidden) = if unfolded { (hits, 0) } else { search_visible_hits(hits) };
    let mut body = v_flex().w_full().py(px(HITS_PAD_Y)).px(px(HITS_PAD_X)).mono(OUT_TEXT).line_height(relative(HITS_LH)).text_color(p.ink);
    for hit in shown {
        // `path:line snippet` as one line: the path muted, the match in the
        // foreground. The snippet is char-capped first, so a huge match
        // cannot widen the body.
        let head = format!("{}:{} ", hit.path, hit.line);
        let snippet = search_capped_snippet(&hit.snippet);
        let text = format!("{head}{snippet}");
        let runs = vec![
            gpui::TextRun { len: head.len(), font: gpui::font(scale::FONT_MONO), color: p.ink_2, background_color: None, underline: None, strikethrough: None },
            gpui::TextRun { len: snippet.len(), font: gpui::font(scale::FONT_MONO), color: p.ink, background_color: None, underline: None, strikethrough: None },
        ];
        body = body.child(div().w_full().overflow_hidden().whitespace_nowrap().child(StyledText::new(text).with_runs(runs)));
    }
    let mut col = v_flex().w_full().child(body);
    if hidden > 0 {
        col = col.child(more_row(p, format!("{hidden} more hits"), emit));
    }
    col
}

fn web_body(p: &Palette, id: &ElementId, results: &[aui_protocol::WebResult], hidden: usize, on_link: Option<LinkHandler>) -> impl IntoElement {
    let mut body = v_flex().w_full().py(px(WEB_PAD_Y)).px(px(WEB_PAD_X)).gap(px(WEB_GAP)).ui(scale::FS_13).text_color(p.ink);
    for (index, r) in results.iter().enumerate() {
        let mut title = div().id((id.clone(), SharedString::from(format!("web-{index}")))).min_w(px(0.0)).truncate().child(r.title.clone());
        if let Some(open) = on_link.clone() {
            let url = r.url.clone();
            title = title
                .cursor_pointer()
                .hover(|d| d.underline())
                .role(gpui::Role::Link)
                .aria_label(r.title.clone())
                .on_click(move |_, w, cx| {
                    cx.stop_propagation();
                    open(url.clone(), w, cx)
                });
        }
        body = body.child(
            h_flex()
                .w_full()
                .gap(px(scale::SP_3))
                .child(div().flex_none().size(px(FAVICON)).rounded(px(FAVICON_RADIUS)).bg(crate::util::raised_lit(p, p.surface_3)).shadow(p.edge()))
                .child(title)
                .child(div().flex_none().text_color(p.ink_2).child(r.domain.clone())),
        );
    }
    if hidden > 0 {
        body = body.child(div().text_color(p.ink_4).child(format!("+{hidden} more")));
    }
    body
}

/// The phase the click ring rests at once the capture is done, and the one
/// reduced motion holds it at: the ring fully expanded and nearly faded out.
const RING_REST: f32 = 1.0;

fn browser_body(p: &Palette, id: &ElementId, screenshot: Option<&str>, caption: Option<&str>, live: bool, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // The ring pulses on the ease-out curve every 1.6 s — but only while the
    // call is live. `looping` requests a frame on every render for as long as
    // it is mounted, so a settled browser card left pulsing would keep the
    // window redrawing for as long as it is on screen; a finished capture
    // holds the resting phase instead, exactly as reduced motion does.
    let phase = if live {
        looping((id.clone(), "click-ring"), Loop::eased(RING_PERIOD, Easing::OUT).resting(RING_REST), window, cx)
    } else {
        let _ = (window, cx);
        RING_REST
    };
    let ring = if live { RING * (0.6 + 0.4 * phase) } else { RING };
    div().w_full().child(
        div()
            .relative()
            .my(px(SHOT_MARGIN_Y))
            .mx(px(SHOT_MARGIN_X))
            .h(px(SHOT_H))
            .rounded(px(SHOT_RADIUS))
            .overflow_hidden()
            .bg(linear_gradient(135.0, linear_color_stop(p.surface_4, 0.0), linear_color_stop(p.surface_3, 1.0)))
            // The page's picture when the call kept one, cropped to the
            // strip from its top; the gradient is the stand-in without one.
            .children(screenshot.map(std::path::PathBuf::from).filter(|path| path.is_file()).map(|path| {
                div().absolute().inset_0().child(gpui::img(path).object_fit(gpui::ObjectFit::Cover).size_full())
            }))
            .child(div().absolute().inset_0().rounded(px(SHOT_RADIUS)).shadow(p.edge()))
            .child(
                div()
                    .absolute()
                    .left(relative(RING_X))
                    .top(relative(RING_Y))
                    .ml(px(-(ring - RING) / 2.0))
                    .mt(px(-(ring - RING) / 2.0))
                    .size(px(ring))
                    .rounded_full()
                    .border(px(RING_STROKE))
                    .border_color(p.ink)
                    // The 1 px 40 % black ring outside the stroke.
                    .shadow(vec![gpui::BoxShadow {
                        color: gpui::black().alpha(0.4),
                        offset: gpui::point(px(0.0), px(0.0)),
                        blur_radius: px(0.0),
                        spread_radius: px(1.0),
                        inset: false,
                    }]),
            )
            .children(caption.map(|caption| {
                div()
                    .absolute()
                    .left(px(SHOT_LABEL_LEFT))
                    .bottom(px(SHOT_LABEL_BOTTOM))
                    .py(px(SHOT_LABEL_PAD_Y))
                    .px(px(SHOT_LABEL_PAD_X))
                    .rounded(px(scale::R_XS))
                    .bg(gpui::rgba(0x121215d9))
                    .ui(scale::FS_11)
                    .text_color(gpui::rgb(0xF2F2F4))
                    .child(caption.to_string())
            })),
    )
}

/// The sub-agent's own lines, two columns: who (64 px, muted) and what was
/// said, in caption. Nothing nests: no bubbles, no inner cards.
fn sub_agent_body(p: &Palette, turns: &[Turn]) -> impl IntoElement {
    let mut body = v_flex().w_full().py(px(SUB_PAD_Y)).px(px(SUB_PAD_X)).gap(px(scale::SP_2)).ui(scale::FS_12).text_color(p.ink);
    for turn in turns {
        let (who, text) = match turn {
            Turn::User { text, .. } => ("You".to_owned(), text.clone()),
            Turn::Assistant { blocks, meta, .. } => {
                let text = blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::Text { text, .. } => Some(text.trim()),
                        _ => None,
                    })
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let who = if meta.model.is_empty() { "Agent".to_owned() } else { meta.model.clone() };
                (who, text)
            }
        };
        if text.is_empty() {
            continue;
        }
        body = body.child(
            h_flex()
                .w_full()
                .items_start()
                .child(div().flex_none().w(px(SUB_WHO_W)).truncate().text_color(p.ink_2).child(who))
                .child(div().flex_1().min_w(px(0.0)).child(text)),
        );
    }
    body
}

fn mcp_body(p: &Palette, params: &[(String, String)], result_json: &str, unfolded: bool, emit: impl Fn(ToolCardIntent) -> IntentClick) -> impl IntoElement {
    let mut body = v_flex().w_full().py(px(HITS_PAD_Y)).px(px(HITS_PAD_X)).ui(scale::FS_12).line_height(relative(HITS_LH)).text_color(p.ink);
    for (k, v) in params {
        body = body.child(
            h_flex()
                .w_full()
                .items_start()
                .child(div().flex_none().w(px(MCP_KEY_W)).truncate().text_color(p.ink_2).child(k.clone()))
                .child(div().flex_1().min_w(px(0.0)).mono(OUT_TEXT).child(v.clone())),
        );
    }
    let (shown, remainder) = if unfolded { (result_json.to_owned(), Remainder::default()) } else { mcp_visible_text(result_json) };
    if !shown.trim().is_empty() {
        body = body.child(div().w_full().pt(px(if params.is_empty() { 0.0 } else { scale::SP_2 })).mono(OUT_TEXT).text_color(p.ink_2).child(shown));
    }
    let mut col = v_flex().w_full().child(body);
    if let Some(cut) = remainder.label() {
        col = col.child(more_row(p, cut, emit));
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;

    mod selectable_output {
        use super::super::{output_text, tool_card};
        use crate::transcript::{SelectionKey, SpanEvent};
        use aui_protocol::{ToolBody, ToolStatus};
        use gpui::{div, point, prelude::*, px, Context, Modifiers, MouseButton, Window};
        use std::cell::RefCell;
        use std::rc::Rc;

        fn lines() -> Vec<String> {
            (0..10).map(|i| if i == 6 { "\u{1b}[32mok\u{1b}[0m line 6".to_string() } else { format!("line {i}") }).collect()
        }

        struct Host {
            got: Rc<RefCell<Vec<SpanEvent>>>,
        }

        impl Render for Host {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let got = self.got.clone();
                let body = ToolBody::Shell { output_lines: lines(), exit_code: Some(0), live: false };
                div().w(px(500.0)).child(
                    tool_card("card", "Ran", "echo", ToolStatus::Success, body)
                        .selectable_output(SelectionKey::new("code0"), None, None, move |event, _, _| got.borrow_mut().push(event)),
                )
            }
        }

        /// D6 follow-up (docs/43 §0): a tool's output could not be selected,
        /// so no code quote came from it. Its lines are one selectable cell
        /// now; a press on the first shown line of a folded body reports an
        /// offset past the four hidden lines, over the ANSI-stripped text.
        #[gpui::test]
        fn a_press_in_the_output_reports_its_offset_over_the_whole_output(cx: &mut gpui::TestAppContext) {
            cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
            let got = Rc::new(RefCell::new(Vec::new()));
            let host = got.clone();
            let (_view, vc) = cx.add_window_view(move |_, _| Host { got: host });
            vc.run_until_parked();
            let text = output_text(&lines(), true);
            assert!(text.contains("ok line 6") && !text.contains('\u{1b}'), "the drawn text, ANSI stripped");
            let first_shown = text.find("line 4").expect("line 4");
            // Find a row of output under the pointer: a press there is a span press.
            let mut offset = None;
            for y in (20..260).step_by(4) {
                got.borrow_mut().clear();
                vc.simulate_mouse_down(point(px(40.0), px(y as f32)), MouseButton::Left, Modifiers::default());
                vc.simulate_mouse_up(point(px(40.0), px(y as f32)), MouseButton::Left, Modifiers::default());
                vc.run_until_parked();
                if let Some(SpanEvent::Press { cell, offset: at }) = got.borrow().first().cloned() {
                    assert_eq!(cell, SelectionKey::new("code0"));
                    offset = Some(at);
                    break;
                }
            }
            let offset = offset.expect("a press on the output reports a span event");
            assert!(offset >= first_shown, "offset {offset} counts the hidden lines (line 4 starts at {first_shown})");
            assert!(offset < text.len());
        }
    }

    #[test]
    fn durations_under_a_minute_print_tenths_of_a_second() {
        assert_eq!(format_duration(0), "0.0 s");
        assert_eq!(format_duration(300), "0.3 s");
        assert_eq!(format_duration(999), "1.0 s");
        assert_eq!(format_duration(1_000), "1.0 s");
        assert_eq!(format_duration(12_400), "12.4 s");
        assert_eq!(format_duration(59_000), "59.0 s");
    }

    #[test]
    fn a_minute_and_over_prints_minutes_and_padded_seconds() {
        // Rounding decides the branch, so 59 999 ms is already a minute.
        assert_eq!(format_duration(59_999), "1 m 00 s");
        assert_eq!(format_duration(60_000), "1 m 00 s");
        assert_eq!(format_duration(61_000), "1 m 01 s");
        assert_eq!(format_duration(72_000), "1 m 12 s");
        assert_eq!(format_duration(3_600_000), "60 m 00 s");
    }

    #[test]
    fn the_seconds_branch_never_prints_sixty_seconds() {
        for ms in 59_900..=60_100 {
            let out = format_duration(ms);
            assert!(!out.starts_with("60.") && !out.starts_with("60 s"), "{ms} ms printed `{out}`");
        }
    }

    #[test]
    fn diff_stat_tags_use_ascii_plus_and_u2212_minus() {
        let (added, removed) = diff_stat_tags(&DiffStat { added: 8, removed: 3, files: 1 });
        assert_eq!(added, "+8");
        assert_eq!(removed, "−3");
        assert!(removed.starts_with('−'), "the minus must stay U+2212, as the Edit arm uses");
    }

    #[test]
    fn the_file_count_shows_only_for_multi_file_patches() {
        assert_eq!(diff_stat_files_label(&DiffStat { added: 8, removed: 3, files: 0 }), None);
        assert_eq!(diff_stat_files_label(&DiffStat { added: 8, removed: 3, files: 1 }), None);
        assert_eq!(diff_stat_files_label(&DiffStat { added: 20, removed: 7, files: 4 }).as_deref(), Some("4 files"));
    }

    #[test]
    fn a_stat_card_with_no_body_shows_the_server_counts() {
        let card = tool_card("t", "Edited", "a.rs", ToolStatus::Success, ToolBody::None).diff_stat(Some(DiffStat { added: 8, removed: 3, files: 1 }));
        // The builder stores the stat the header chips read...
        let stat = card.diff_stat.as_ref().expect("diff_stat builder stores the stat");
        let (added, removed) = diff_stat_tags(stat);
        assert_eq!((added.as_str(), removed.as_str()), ("+8", "−3"));
        assert_eq!(diff_stat_files_label(stat), None);
        // ...and with no `Edit` body there is no second source of chips.
        assert!(!matches!(card.body, ToolBody::Edit { .. }));
    }

    #[test]
    fn a_stat_card_with_an_edit_body_draws_exactly_one_pair() {
        let diff = aui_protocol::Diff { path: "a.rs".into(), hunks: Vec::new(), added: 2, removed: 1 };
        let card = tool_card("t", "Edited", "a.rs", ToolStatus::Success, ToolBody::Edit { diff })
            .diff_stat(Some(DiffStat { added: 20, removed: 7, files: 4 }));
        // The stat block draws its whole-patch pair...
        let stat = card.diff_stat.as_ref().expect("diff_stat builder stores the stat");
        let (added, removed) = diff_stat_tags(stat);
        assert_eq!((added.as_str(), removed.as_str()), ("+20", "−7"));
        assert_eq!(diff_stat_files_label(stat).as_deref(), Some("4 files"));
        // ...and the `Edit` arm stands down, so the header holds one pair.
        assert!(matches!(card.body, ToolBody::Edit { .. }));
        assert!(!edit_draws_chips(card.diff_stat.as_ref()));
    }

    #[test]
    fn an_edit_card_without_a_stat_still_draws_its_own_chips() {
        let diff = aui_protocol::Diff { path: "a.rs".into(), hunks: Vec::new(), added: 2, removed: 1 };
        let card = tool_card("t", "Edited", "a.rs", ToolStatus::Success, ToolBody::Edit { diff });
        assert_eq!(card.diff_stat, None);
        assert!(edit_draws_chips(card.diff_stat.as_ref()));
    }

    fn numbered_lines(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("line {i}")).collect()
    }

    fn search_hits(n: usize) -> Vec<aui_protocol::SearchHit> {
        (1..=n)
            .map(|i| aui_protocol::SearchHit {
                path: format!("src/file{i}.rs"),
                line: i as u32,
                snippet: format!("hit {i}"),
            })
            .collect()
    }

    #[test]
    fn shell_shows_the_last_six_lines() {
        // Twenty lines in, the head is gone and the tail stays in order.
        let lines = numbered_lines(20);
        assert_eq!(
            shell_visible_lines(&lines),
            &["line 15", "line 16", "line 17", "line 18", "line 19", "line 20"]
        );
    }

    #[test]
    fn shell_shorter_than_the_fold_shows_everything() {
        let lines = numbered_lines(4);
        assert_eq!(shell_visible_lines(&lines), &["line 1", "line 2", "line 3", "line 4"]);
        assert!(shell_visible_lines(&[]).is_empty());
        // Exactly the fold: nothing to fold away.
        assert_eq!(shell_visible_lines(&numbered_lines(SHELL_FOLD)).len(), SHELL_FOLD);
    }

    #[test]
    fn search_caps_at_forty_hits() {
        let hits = search_hits(100);
        let (shown, hidden) = search_visible_hits(&hits);
        assert_eq!(shown.len(), SEARCH_BODY_CAP);
        assert_eq!(shown[0].snippet, "hit 1");
        assert_eq!(hidden, 60);
    }

    #[test]
    fn search_shorter_than_the_cap_hides_nothing() {
        let hits = search_hits(2);
        let (shown, hidden) = search_visible_hits(&hits);
        assert_eq!(shown.len(), 2);
        assert_eq!(hidden, 0);
    }

    #[test]
    fn mcp_result_caps_at_forty_lines() {
        let json = (1..=100).map(|i| format!("{{\"i\":{i}}}")).collect::<Vec<_>>().join("\n");
        let (shown, remainder) = mcp_visible_text(&json);
        assert_eq!(shown.lines().count(), MCP_BODY_CAP);
        assert!(shown.starts_with("{\"i\":1}"));
        assert_eq!(remainder.lines, 60);
        assert_eq!(remainder.chars, 0);
    }

    #[test]
    fn mcp_minified_single_line_result_is_cut_not_reformatted() {
        // A 2,000-char minified line: no newlines, so only the char cap binds.
        let json = format!("{{\"data\":\"{}\"}}", "d".repeat(2_000));
        let (shown, remainder) = mcp_visible_text(&json);
        assert!(
            shown.chars().count() <= 6_000,
            "minified JSON shows {}, over the 6,000 cap",
            shown.chars().count()
        );
        assert!(!shown.contains('\n'), "the JSON must be cut, never pretty-printed");
        assert!(shown.starts_with("{\"data\":\"ddd"));
        assert_eq!(remainder.lines, 0);
        assert!(remainder.chars > 0);
    }

    #[test]
    fn mcp_short_result_hides_nothing() {
        let (shown, remainder) = mcp_visible_text("{\"id\":\"ACME-2491\"}");
        assert_eq!(shown, "{\"id\":\"ACME-2491\"}");
        assert!(remainder.is_empty());
    }

    #[test]
    fn search_snippet_with_a_huge_match_is_bounded() {
        assert_eq!(search_capped_snippet("hit 1"), "hit 1");
        let huge = "s".repeat(5_000);
        let capped = search_capped_snippet(&huge);
        assert!(capped.ends_with('…'), "the cut snippet must mark its cut");
        assert!(
            capped.chars().count() <= 161,
            "huge snippet shows {}, over the 160-char line cap",
            capped.chars().count()
        );
    }
}
