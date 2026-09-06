//! Card 55 · Sources and citations: the inline citation marker that sits in
//! the assistant's answer, the sources card grouped by retrieval tier (role,
//! project, session) and the source hover card with the matched passage.
//!
//! gpui text hosts no inline elements, so an answer that carries markers is
//! not one `StyledText`: [`cited_answer`] splits the paragraph at its spaces
//! into unbreakable units (a word plus any markers and punctuation glued to
//! it) and lays those out in a wrapping flex row, so the only line-break
//! opportunities are the ones the browser has. The space between units is the
//! font's own space advance, measured through the text system, so the wrap
//! points land where the CSS puts them.

use std::ops::Range;
use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, spring_phase, tint_fade, tween, EnterExit, PresenceStyle, SpringKind, Tween};
use aui_tokens::{scale, scaled, ActiveAui, AuiStyled, Palette};
use gpui::{
    div, font, prelude::*, px, relative, AnyElement, App, ElementId, IntoElement, SharedString, StyledText, TextRun,
    Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, ButtonSize};
use crate::transcript::ProseStyle;
use crate::util::{interaction_flags, TrackInteraction};

/// `.cite{min-width:16px;height:16px;padding:0 4px;border-radius:4px;font:600 10px/1 mono;margin:0 1px}`.
const CITE_MIN_W: f32 = 16.0;
const CITE_H: f32 = 16.0;
const CITE_PAD_X: f32 = 4.0;
const CITE_TEXT: f32 = 10.0;
const CITE_MARGIN_X: f32 = 1.0;
/// `.cite{vertical-align:2px}` — the marker box rides 2 px above the text
/// baseline; the units align on their bottom edge, so this is a relative rise.
const CITE_RAISE: f32 = 3.0;
/// `.cite:hover{transform:translateY(-1px)}`.
const CITE_LIFT: f32 = 1.0;

/// `.a{margin-bottom:14px}` — the gap between the answer and the sources card.
pub const ANSWER_MARGIN: f32 = 14.0;

/// `.src .hd{height:34px;padding:0 12px;gap:8px;font-weight:600;font-size:12.5px}`.
const HEAD_H: f32 = 34.0;
const HEAD_PAD_X: f32 = 12.0;
const HEAD_GAP: f32 = 8.0;
const HEAD_TEXT: f32 = 12.5;
/// `.tier{padding:6px 12px 4px;font-size:11px;font-weight:600;text-transform:uppercase}`.
const TIER_PAD_TOP: f32 = 6.0;
const TIER_PAD_X: f32 = 12.0;
const TIER_PAD_BOTTOM: f32 = 4.0;
const TIER_TEXT: f32 = 11.0;
/// `.s{gap:10px;padding:6px 12px;font-size:12.5px}`.
const ROW_GAP: f32 = 10.0;
const ROW_PAD_Y: f32 = 6.0;
const ROW_PAD_X: f32 = 12.0;
const ROW_TEXT: f32 = 12.5;
/// `.s .n{width:18px;height:18px;border-radius:4px;font:600 10px/18px mono}`.
const NUM_SIZE: f32 = 18.0;
const NUM_TEXT: f32 = 10.0;
/// `.s span{font-size:11.5px}`. The span is inline, so its line box is as tall
/// as the row's own strut (12.5 × 1.5), not as its smaller text.
const META_TEXT: f32 = 11.5;
const META_LINE: f32 = ROW_TEXT * scale::LH_UI;
/// `.s .conf{gap:5px;font:500 10.5px mono}` with a `36 × 4` track, radius 2.
const CONF_GAP: f32 = 5.0;
const CONF_TEXT: f32 = 10.5;
const CONF_BAR_W: f32 = 36.0;
const CONF_BAR_H: f32 = 4.0;
const CONF_BAR_RADIUS: f32 = 2.0;
/// The book glyph in the header (`.i.lg` is 16, the header uses the 14 px `.i`).
const HEAD_ICON: f32 = 14.0;

/// `.hover{right:24px;top:-8px;width:280px;padding:10px 12px;font-size:12px}`.
const HOVER_W: f32 = 280.0;
const HOVER_RIGHT: f32 = 24.0;
const HOVER_TOP: f32 = -8.0;
const HOVER_PAD_Y: f32 = 10.0;
const HOVER_PAD_X: f32 = 12.0;
const HOVER_TEXT: f32 = 12.0;
/// `@keyframes in{from{transform:translateY(4px)}}`.
const HOVER_RISE: f32 = 4.0;
/// `.hover .q{border-left:2px solid var(--accent);padding-left:8px;margin:6px 0 8px;line-height:1.5}`.
const QUOTE_RAIL: f32 = 2.0;
const QUOTE_PAD_LEFT: f32 = 8.0;
const QUOTE_MARGIN_TOP: f32 = 6.0;
const QUOTE_MARGIN_BOTTOM: f32 = 8.0;
/// `.hover .row{gap:6px}`.
const ACTIONS_GAP: f32 = 6.0;

/// The em-relative base the type scale is expressed against, so a design px
/// size can be turned into the window's real pixels for text measurement.
fn real_px(size: f32, window: &Window) -> gpui::Pixels {
    scaled(size).to_pixels(window.rem_size())
}

// ---------------------------------------------------------------------------
// Citation marker
// ---------------------------------------------------------------------------

type OpenHandler = Rc<dyn Fn(u8, &mut Window, &mut App)>;

/// An inline citation marker (`.cite`): the small accent square carrying a
/// source number. Build with [`citation`].
#[derive(IntoElement)]
pub struct Citation {
    index: u8,
    on_open: Option<OpenHandler>,
}

/// A citation marker for source `index`. Its interaction state is keyed by the
/// index, so one answer shows each number once.
pub fn citation(index: u8) -> Citation {
    Citation { index, on_open: None }
}

impl Citation {
    /// The marker was clicked; the argument is the source number.
    pub fn on_open(mut self, f: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Citation {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id: ElementId = ("aui-citation", self.index as usize).into();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let lift = spring_phase((id.clone(), "lift"), flags.hovered, SpringKind::Press, window, cx) * CITE_LIFT;
        let bg = tween((id.clone(), "bg"), if flags.hovered { p.accent } else { p.accent_soft }, Tween::FAST, window, cx);
        let ink = if flags.hovered { gpui::white() } else { p.accent_ink };
        let mut marker = h_flex()
            .id(id)
            .flex_none()
            .relative()
            .bottom(px(CITE_RAISE + lift))
            .min_w(px(CITE_MIN_W))
            .h(px(CITE_H))
            .px(px(CITE_PAD_X))
            .mx(px(CITE_MARGIN_X))
            .justify_center()
            .rounded(px(scale::R_XS))
            .bg(bg)
            .mono(CITE_TEXT)
            .line_height(relative(1.0))
            .semibold()
            .text_color(ink)
            .cursor_pointer()
            .track_interaction(&state)
            .child(SharedString::from(self.index.to_string()));
        if let Some(h) = self.on_open.clone() {
            let index = self.index;
            marker = marker.on_click(move |_, w, cx| h(index, w, cx));
        }
        marker
    }
}

// ---------------------------------------------------------------------------
// The answer that carries markers
// ---------------------------------------------------------------------------

/// One piece of an answer: a run of text or a citation marker.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Text(String),
    Marker(u8),
}

/// An unbreakable unit: a word with the markers and punctuation glued to it.
type Unit = Vec<Piece>;

/// Splits `text` into units. `[[n]]` is a citation marker; a run of spaces is
/// the only break opportunity, exactly as in the HTML where markers sit
/// directly against the words they follow.
fn units(text: &str) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::new();
    let mut unit: Unit = Vec::new();
    let mut buf = String::new();
    let mut chars = text.chars().peekable();
    let flush_word = |unit: &mut Unit, buf: &mut String| {
        if !buf.is_empty() {
            unit.push(Piece::Text(std::mem::take(buf)));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '[' if chars.peek() == Some(&'[') => {
                chars.next();
                flush_word(&mut unit, &mut buf);
                let mut number = String::new();
                while let Some(&d) = chars.peek() {
                    chars.next();
                    if d == ']' {
                        // consume the second ']'
                        if chars.peek() == Some(&']') {
                            chars.next();
                        }
                        break;
                    }
                    number.push(d);
                }
                if let Ok(index) = number.trim().parse::<u8>() {
                    unit.push(Piece::Marker(index));
                }
            }
            c if c.is_whitespace() => {
                flush_word(&mut unit, &mut buf);
                if !unit.is_empty() {
                    out.push(std::mem::take(&mut unit));
                }
            }
            c => buf.push(c),
        }
    }
    flush_word(&mut unit, &mut buf);
    if !unit.is_empty() {
        out.push(unit);
    }
    out
}

/// How many spaces the measuring line carries. A lone space shapes to a hinted
/// advance, so the real one is taken as the difference between a spaced and a
/// dense line of the same glyphs and divided down.
const SPACE_SAMPLES: usize = 32;

/// Chrome sets this paragraph about two thirds of a pixel per word tighter
/// than gpui does — every word is a flex item whose width is rounded up, and
/// the two shapers round Geist's advances differently — so the row takes that
/// much back out of every space and the line does not drift toward its end.
const SPACE_TIGHTEN: f32 = 0.66;

/// Measures text in the answer's face, in real pixels.
fn measure(text: &str, style: ProseStyle, window: &Window) -> gpui::Pixels {
    let run = TextRun {
        len: text.len(),
        font: font(scale::FONT_UI),
        color: style.ink,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window.text_system().layout_line(text, real_px(style.size, window), &[run], None).width
}

/// The font's space advance at the answer's size, in real pixels.
fn space_advance(style: ProseStyle, window: &Window) -> gpui::Pixels {
    let dense: String = "n".repeat(SPACE_SAMPLES + 1);
    let spaced: String = vec!["n"; SPACE_SAMPLES + 1].join(" ");
    (measure(&spaced, style, window) - measure(&dense, style, window)) / SPACE_SAMPLES as f32
}

/// An assistant answer with inline citation markers (`.a p`). Build with
/// [`cited_answer`].
#[derive(IntoElement)]
pub struct CitedAnswer {
    id: ElementId,
    text: SharedString,
    style: ProseStyle,
    streaming: bool,
    on_open: Option<OpenHandler>,
}

/// An answer paragraph; write markers as `[[1]]` where the HTML has a
/// `<span class="cite">`.
pub fn cited_answer(id: impl Into<ElementId>, text: impl Into<SharedString>, style: ProseStyle) -> CitedAnswer {
    CitedAnswer { id: id.into(), text: text.into(), style, streaming: false, on_open: None }
}

impl CitedAnswer {
    /// Shows the blinking caret after the last word while chunks arrive. The
    /// answer is a wrapping row of word groups, so the caret is simply the
    /// last group and needs no measuring.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// A marker was clicked; the argument is the source number.
    pub fn on_open(mut self, f: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for CitedAnswer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let style = self.style;
        let p = cx.aui().colors;
        let text_scale = cx.aui().text_scale;
        let caret = self.streaming.then(|| {
            let visible = crate::transcript::caret_visible((self.id.clone(), "caret"), window, cx);
            let line_height = px(style.size * style.line_height * text_scale);
            let height = px(crate::transcript::CARET_H * text_scale);
            h_flex().flex_none().h(line_height).ml(px(crate::transcript::CARET_MARGIN_LEFT * text_scale)).child(
                div()
                    .mt(crate::transcript::caret_top_in_line(line_height, height, text_scale))
                    .w(px(crate::transcript::CARET_W * text_scale))
                    .h(height)
                    .bg(p.accent)
                    .opacity(if visible { 1.0 } else { 0.0 }),
            )
        });
        // The gap between units is the font's own space advance, so the flex
        // row wraps where the browser's line breaker does.
        let space = space_advance(style, window) - px(SPACE_TIGHTEN);
        let mut row = div()
            .id(self.id)
            .flex()
            .flex_wrap()
            .w_full()
            .gap_x(space)
            .ui(style.size)
            .line_height(relative(style.line_height))
            .text_color(style.ink);
        for unit in units(&self.text) {
            let mut group = h_flex().flex_none().items_end();
            for piece in unit {
                group = match piece {
                    Piece::Text(t) => group.child(div().flex_none().child(SharedString::from(t))),
                    Piece::Marker(index) => {
                        let mut marker = citation(index);
                        if let Some(h) = self.on_open.clone() {
                            marker = marker.on_open(move |i, w, cx| h(i, w, cx));
                        }
                        group.child(marker)
                    }
                };
            }
            row = row.child(group);
        }
        row.children(caret)
    }
}

// ---------------------------------------------------------------------------
// Sources card
// ---------------------------------------------------------------------------

/// One retrieved source (`.s`).
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// The citation number, or `None` for a retrieved-but-uncited source
    /// (drawn muted, with a dash in place of the number).
    pub index: Option<u8>,
    /// The source title.
    pub title: SharedString,
    /// The meta line: file, locator and what the passage says.
    pub meta: SharedString,
    /// Retrieval confidence in `0..=1`, or `None` for an uncited source.
    pub confidence: Option<f32>,
}

impl Source {
    /// A cited source with its number and confidence.
    pub fn cited(index: u8, title: impl Into<SharedString>, meta: impl Into<SharedString>, confidence: f32) -> Self {
        Self { index: Some(index), title: title.into(), meta: meta.into(), confidence: Some(confidence) }
    }

    /// A retrieved but uncited source: muted row, no number, no bar.
    pub fn uncited(title: impl Into<SharedString>, meta: impl Into<SharedString>) -> Self {
        Self { index: None, title: title.into(), meta: meta.into(), confidence: None }
    }
}

/// A retrieval tier (`.tier`): the caps label, an optional name after the
/// separator, and the sources found in it.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceTier {
    /// The tier: `Role`, `Project`, `Session`.
    pub label: SharedString,
    /// The name of the role or project, shown after a `·`.
    pub name: Option<SharedString>,
    /// The sources of this tier, in citation order.
    pub sources: Vec<Source>,
}

impl SourceTier {
    /// A tier with no sources yet.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self { label: label.into(), name: None, sources: Vec::new() }
    }

    /// The role or project name shown after the separator.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Appends a source.
    pub fn source(mut self, source: Source) -> Self {
        self.sources.push(source);
        self
    }
}

/// The sources list (`.src`). Build with [`sources_card`].
#[derive(IntoElement)]
pub struct SourcesCard {
    id: ElementId,
    tiers: Vec<SourceTier>,
    cited: Option<usize>,
    retrieved: Option<usize>,
    hover: Option<(usize, AnyElement)>,
    on_show_all: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_open: Option<OpenHandler>,
}

/// The sources card for `tiers`.
pub fn sources_card(id: impl Into<ElementId>, tiers: Vec<SourceTier>) -> SourcesCard {
    SourcesCard { id: id.into(), tiers, cited: None, retrieved: None, hover: None, on_show_all: None, on_open: None }
}

impl SourcesCard {
    /// How many sources the answer cites (the header count).
    pub fn cited(mut self, count: usize) -> Self {
        self.cited = Some(count);
        self
    }

    /// How many sources retrieval returned (the header count).
    pub fn retrieved(mut self, count: usize) -> Self {
        self.retrieved = Some(count);
        self
    }

    /// Anchors a [`SourceHoverCard`] to the row at `row` (counted over all
    /// tiers, in render order), as the HTML nests `.hover` inside a `.s`.
    pub fn hover_card(mut self, row: usize, card: impl IntoElement) -> Self {
        self.hover = Some((row, card.into_any_element()));
        self
    }

    /// `Show all` was pressed.
    pub fn on_show_all(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_show_all = Some(Rc::new(f));
        self
    }

    /// A source row was opened; the argument is its citation number.
    pub fn on_open(mut self, f: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(f));
        self
    }
}

/// `· 3 cited · 11 retrieved`, weight 400 in ink-3.
fn header_counts(cited: Option<usize>, retrieved: Option<usize>) -> Option<SharedString> {
    match (cited, retrieved) {
        (None, None) => None,
        (c, r) => {
            let mut s = String::new();
            if let Some(c) = c {
                s.push_str(&format!("· {c} cited "));
            }
            if let Some(r) = r {
                s.push_str(&format!("· {r} retrieved"));
            }
            Some(SharedString::from(s.trim_end().to_string()))
        }
    }
}

/// `.tier`: the caps label with the optional name after a separator.
fn tier_label(p: &Palette, tier: &SourceTier) -> impl IntoElement {
    let text = match &tier.name {
        Some(name) => format!("{} · {}", tier.label, name),
        None => tier.label.to_string(),
    };
    div()
        .w_full()
        .pt(px(TIER_PAD_TOP))
        .pb(px(TIER_PAD_BOTTOM))
        .px(px(TIER_PAD_X))
        .ui(TIER_TEXT)
        .line_height(relative(scale::LH_UI))
        .semibold()
        .text_color(p.ink_3)
        .child(SharedString::from(text.to_uppercase()))
}

/// `.s .conf`: the 36 × 4 track with a success fill and the mono value.
fn confidence(p: &Palette, value: f32) -> impl IntoElement {
    let fraction = value.clamp(0.0, 1.0);
    let text = format!("{:.2}", fraction);
    let text = text.strip_prefix('0').map(str::to_string).unwrap_or(text);
    h_flex()
        .flex_none()
        .ml_auto()
        .gap(px(CONF_GAP))
        .mono(CONF_TEXT)
        // The inline-flex `.conf` is as tall as its own line box, and the row
        // aligns it to the top, so the bar rides on the title's line.
        .line_height(relative(scale::LH_UI))
        .medium()
        .text_color(p.ink_3)
        .child(
            div()
                .flex_none()
                .w(px(CONF_BAR_W))
                .h(px(CONF_BAR_H))
                .rounded(px(CONF_BAR_RADIUS))
                .bg(p.surface_3)
                .overflow_hidden()
                .child(div().h_full().w(relative(fraction)).bg(p.success)),
        )
        .child(SharedString::from(text))
}

/// One `.s` row.
fn source_row(
    id: ElementId,
    source: &Source,
    hover: Option<AnyElement>,
    on_open: Option<OpenHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let p = cx.aui().colors;
    let (state, flags) = interaction_flags(id.clone(), window, cx);
    let ground = tint_fade((id.clone(), "bg"), flags.hovered, p.surface_2, Tween::FAST, window, cx);
    let cited = source.index.is_some();
    let (num_bg, num_ink) = if cited { (p.accent_soft, p.accent_ink) } else { (p.surface_3, p.ink_3) };
    let title_ink = if cited { p.ink } else { p.ink_2 };
    let mut row = h_flex()
        .id(id)
        .w_full()
        .relative()
        .items_start()
        .gap(px(ROW_GAP))
        .py(px(ROW_PAD_Y))
        .px(px(ROW_PAD_X))
        .bg(ground)
        .ui(ROW_TEXT)
        .text_color(if cited { p.ink } else { p.ink_3 })
        .cursor_pointer()
        .track_interaction(&state)
        .child(
            div()
                .flex_none()
                .w(px(NUM_SIZE))
                .h(px(NUM_SIZE))
                .rounded(px(scale::R_XS))
                .bg(num_bg)
                .mono(NUM_TEXT)
                .line_height(px(NUM_SIZE))
                .semibold()
                .text_color(num_ink)
                .text_center()
                .child(match source.index {
                    Some(i) => SharedString::from(i.to_string()),
                    None => SharedString::new_static("–"),
                }),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .child(div().w_full().medium().text_color(title_ink).child(source.title.clone()))
                .child(
                    div()
                        .w_full()
                        .ui(META_TEXT)
                        .line_height(relative(META_LINE / META_TEXT))
                        .text_color(p.ink_3)
                        .child(source.meta.clone()),
                ),
        );
    if let Some(value) = source.confidence {
        row = row.child(confidence(&p, value));
    }
    if let Some(card) = hover {
        // `.hover{z-index:2}`: the popover paints over the rows below it, which
        // in gpui means lifting it onto the popover layer.
        row = row.child(crate::overlay::popover_layer(div().absolute().right(px(HOVER_RIGHT)).top(px(HOVER_TOP)).child(card)));
    }
    if let (Some(h), Some(index)) = (on_open, source.index) {
        row = row.on_click(move |_, w, cx| h(index, w, cx));
    }
    row
}

impl RenderOnce for SourcesCard {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut card = v_flex()
            .w_full()
            .overflow_hidden()
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .child(header(&p, id.clone(), self.cited, self.retrieved, self.on_show_all.take()));
        let mut row_index = 0usize;
        for (t, tier) in self.tiers.iter().enumerate() {
            card = card.child(tier_label(&p, tier));
            for (s, source) in tier.sources.iter().enumerate() {
                let hover = match &self.hover {
                    Some((row, _)) if *row == row_index => self.hover.take().map(|(_, card)| card),
                    _ => None,
                };
                card = card.child(source_row(
                    (id.clone(), SharedString::from(format!("row-{t}-{s}"))).into(),
                    source,
                    hover,
                    self.on_open.clone(),
                    window,
                    cx,
                ));
                row_index += 1;
            }
        }
        card
    }
}

/// `.src .hd`: the book glyph, the title, the counts and `Show all`.
fn header(
    p: &Palette,
    id: ElementId,
    cited: Option<usize>,
    retrieved: Option<usize>,
    on_show_all: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
) -> impl IntoElement {
    let mut show_all = button((id.clone(), "show-all"), "Show all").ghost().size(ButtonSize::Xs);
    if let Some(h) = on_show_all {
        show_all = show_all.on_click(move |_, w, cx| h(w, cx));
    }
    h_flex()
        .w_full()
        .h(px(HEAD_H))
        .flex_none()
        .gap(px(HEAD_GAP))
        .px(px(HEAD_PAD_X))
        .ui(HEAD_TEXT)
        .semibold()
        .text_color(p.ink)
        .child(icon(IconName::Book).size(px(HEAD_ICON)).color(p.ink))
        .child(div().flex_none().child(SharedString::new_static("Sources")))
        .children(
            header_counts(cited, retrieved)
                .map(|counts| div().flex_none().font_weight(gpui::FontWeight::NORMAL).text_color(p.ink_3).child(counts)),
        )
        .child(div().flex_1().min_w(px(0.0)))
        .child(show_all)
}

// ---------------------------------------------------------------------------
// Source hover card
// ---------------------------------------------------------------------------

/// The passage popover (`.hover`). Build with [`source_hover_card`].
#[derive(IntoElement)]
pub struct SourceHoverCard {
    id: ElementId,
    title: SharedString,
    quote: SharedString,
    highlight: Range<usize>,
    page: u32,
    present: bool,
    timing: EnterExit,
    on_open: Option<Rc<dyn Fn(u32, &mut Window, &mut App)>>,
    on_insert: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

/// A hover card for one source: its title, the quoted passage with the matched
/// span (a byte range into `quote`) on accent-soft, and the two actions.
pub fn source_hover_card(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    quote: impl Into<SharedString>,
    highlight: Range<usize>,
    page: u32,
) -> SourceHoverCard {
    SourceHoverCard {
        id: id.into(),
        title: title.into(),
        quote: quote.into(),
        highlight,
        page,
        present: true,
        timing: EnterExit::DEFAULT,
        on_open: None,
        on_insert: None,
    }
}

impl SourceHoverCard {
    /// Whether the card is shown; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the card is drawn at rest on its first frame.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// `Open page N` was pressed; the argument is the page.
    pub fn on_open(mut self, f: impl Fn(u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(f));
        self
    }

    /// `Insert quote` was pressed.
    pub fn on_insert(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_insert = Some(Rc::new(f));
        self
    }
}

/// The quote's text runs: ink-2 throughout, the matched span on accent-soft
/// in full ink (`.hover .q mark`).
fn quote_runs(quote: &str, highlight: &Range<usize>, p: &Palette) -> Vec<TextRun> {
    let ui = font(scale::FONT_UI);
    let plain = |len: usize| TextRun {
        len,
        font: ui.clone(),
        color: p.ink_2,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let start = highlight.start.min(quote.len());
    let end = highlight.end.clamp(start, quote.len());
    if start == end {
        return vec![plain(quote.len())];
    }
    let mut runs = Vec::new();
    if start > 0 {
        runs.push(plain(start));
    }
    runs.push(TextRun {
        len: end - start,
        font: ui.clone(),
        color: p.ink,
        background_color: Some(p.accent_soft),
        underline: None,
        strikethrough: None,
    });
    if end < quote.len() {
        runs.push(plain(quote.len() - end));
    }
    runs
}

impl RenderOnce for SourceHoverCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let style = PresenceStyle::fade_rise(sample, HOVER_RISE);
        let page = self.page;
        let mut open = button((id.clone(), "open"), SharedString::from(format!("Open page {page}"))).size(ButtonSize::Xs);
        if let Some(h) = self.on_open.clone() {
            open = open.on_click(move |_, w, cx| h(page, w, cx));
        }
        let mut insert = button((id.clone(), "insert"), "Insert quote").ghost().size(ButtonSize::Xs);
        if let Some(h) = self.on_insert.clone() {
            insert = insert.on_click(move |_, w, cx| h(w, cx));
        }
        v_flex()
            .flex_none()
            .relative()
            .top(style.offset_y)
            .opacity(style.opacity)
            .w(px(HOVER_W))
            .py(px(HOVER_PAD_Y))
            .px(px(HOVER_PAD_X))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(3))
            .ui(HOVER_TEXT)
            .text_color(p.ink)
            .child(div().w_full().semibold().child(self.title.clone()))
            .child(
                div()
                    .w_full()
                    .mt(px(QUOTE_MARGIN_TOP))
                    .mb(px(QUOTE_MARGIN_BOTTOM))
                    .pl(px(QUOTE_PAD_LEFT))
                    .border_l(px(QUOTE_RAIL))
                    .border_color(p.accent)
                    .ui(HOVER_TEXT)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_2)
                    .child(StyledText::new(self.quote.clone()).with_runs(quote_runs(&self.quote, &self.highlight, &p))),
            )
            .child(h_flex().w_full().gap(px(ACTIONS_GAP)).child(open).child(insert))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_break_only_on_spaces_and_keep_markers_glued() {
        let u = units("a bidder placements[[1]]. The 2024");
        assert_eq!(u.len(), 5);
        assert_eq!(u[2], vec![Piece::Text("placements".into()), Piece::Marker(1), Piece::Text(".".into())]);
        assert_eq!(u[4], vec![Piece::Text("2024".into())]);
    }

    #[test]
    fn confidence_is_printed_without_the_leading_zero() {
        assert_eq!(format!("{:.2}", 0.92f32).strip_prefix('0'), Some(".92"));
    }

    #[test]
    fn quote_runs_cover_the_text() {
        let p = aui_tokens::light();
        let quote = "hold a valid registration";
        let runs = quote_runs(quote, &(5..10), &p);
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), quote.len());
        assert_eq!(runs.len(), 3);
    }
}
