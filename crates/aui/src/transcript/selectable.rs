//! Selectable text for transcript markdown cells: the selection model, the
//! pure range helpers and the [`SelectableText`] element.
//!
//! gpui-pre 0.3.3 has no selectable text element — [`InteractiveText`] only
//! reports click/hover over character ranges — so markdown cells paint
//! [`SelectableText`], which maps pointer positions back to byte indices
//! through the shaped layout's `index_for_position` exactly like
//! `InteractiveText` does, and paints the selection behind the glyphs by
//! splitting runs around it with the theme's `selection` token as their
//! background (the same ground code spans already paint).
//!
//! The component is stateless: the app owns one [`Option<TextSelection>`]
//! per markdown view and passes it back through
//! [`Markdown::selection`](super::Markdown::selection); drags and word /
//! paragraph picks leave as
//! [`Markdown::on_selection_change`](super::Markdown::on_selection_change)
//! intents. Copy is
//! [`Markdown::selected_text`](super::Markdown::selected_text) plus the app's
//! own ⌘C binding.
//!
//! Limitations, stated plainly:
//! - A selection lives inside one cell (a paragraph, heading, list item,
//!   table cell, or code block — a fenced block's lines share one key with
//!   byte offsets over the whole block text). Dragging across cells keeps the
//!   first cell's range; there is no cross-cell model.
//! - Drag moves are handled window-wide while a press started inside the
//!   cell, with out-of-cell positions clamped to the nearer edge, so no
//!   capture overlay is needed; a release outside the window still ends the
//!   drag wherever the pointer left it, because the release event never
//!   arrives.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, CursorStyle, DispatchPhase, Element, ElementId, GlobalElementId, Hitbox,
    HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, StyledText, TextLayout, TextRun,
    Window,
};

use super::markdown::{LinkHandler, LinkRange, LinkTarget};

/// Identifies one selectable cell inside a
/// [`Markdown`](super::Markdown) render: a paragraph, heading, list item,
/// table cell or fenced code block. Keys are scoped to the markdown view that
/// rendered them — the app holds one selection per markdown instance, so two
/// views never compare keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectionKey(String);

impl SelectionKey {
    /// Builds a key from its encoded form (`p0`, `q2-p1`, …).
    pub fn new(name: impl Into<String>) -> Self {
        SelectionKey(name.into())
    }

    /// The encoded key.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Key of the `index`-th paragraph under `prefix`.
    pub fn paragraph(prefix: &str, index: usize) -> Self {
        SelectionKey(format!("{prefix}p{index}"))
    }

    /// Key of the `index`-th heading under `prefix`.
    pub fn heading(prefix: &str, index: usize) -> Self {
        SelectionKey(format!("{prefix}h{index}"))
    }

    /// Key of one list item: `index` is the block, `item` the row;
    /// `ordered` picks the `o` (numbered) or `b` (bullet) arm.
    pub fn list_item(prefix: &str, index: usize, ordered: bool, item: usize) -> Self {
        let kind = if ordered { 'o' } else { 'b' };
        SelectionKey(format!("{prefix}{kind}{index}-{item}"))
    }

    /// Key of the `index`-th fenced code block under `prefix`. Every line of
    /// the block shares this key; ranges are byte offsets over the whole
    /// block text (newlines included).
    pub fn code(prefix: &str, index: usize) -> Self {
        SelectionKey(format!("{prefix}code{index}"))
    }

    /// Key of one table cell: `index` is the block, `row` is `None` for a
    /// header cell and `Some` for a body row, `col` the column.
    pub fn table_cell(prefix: &str, index: usize, row: Option<usize>, col: usize) -> Self {
        match row {
            None => SelectionKey(format!("{prefix}t{index}-h{col}")),
            Some(row) => SelectionKey(format!("{prefix}t{index}-{row}-{col}")),
        }
    }

    /// The key prefix for blocks nested inside the `index`-th quote: inner
    /// keys read `q{index}-…`, so quote cells never collide with siblings.
    pub fn quote_prefix(prefix: &str, index: usize) -> String {
        format!("{prefix}q{index}-")
    }
}

impl std::fmt::Display for SelectionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A text selection inside one markdown cell: which cell, and the byte range
/// over that cell's shaped text. An empty range is never stored — clearing is
/// `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSelection {
    /// The cell holding the selection.
    pub cell: SelectionKey,
    /// Byte range over the cell's shaped text, normalized (`start <= end`).
    pub range: Range<usize>,
}

/// A selection intent: `Some` replaces the app's stored selection, `None`
/// clears it.
pub type SelectionHandler = Rc<dyn Fn(Option<TextSelection>, &mut Window, &mut App)>;

/// One end of a cross-cell span: which cell, and the byte offset into that
/// cell's shaped text. Offsets come from the same `index_for_position`
/// mapping drags use, so they sit on char boundaries of the text the cell
/// painted; stale offsets (the source edited mid-drag) clamp at render and
/// copy time instead of panicking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionEndpoint {
    /// The cell holding this end of the span.
    pub cell: SelectionKey,
    /// Byte offset into the cell's shaped text.
    pub offset: usize,
}

/// A text selection spanning cells: one owner-level selection (anchor cell +
/// offset, focus cell + offset) with the cells in between fully selected.
/// The app holds one `Option<MessageSelection>` per markdown view — the
/// anchor and focus live in app state, never in element state, so a cell
/// entering or leaving the viewport mid-drag cannot drop the span.
///
/// A span whose ends share a cell behaves exactly like the legacy
/// [`TextSelection`] over that range (see [`single_cell`](Self::single_cell));
/// equal ends are a caret and are never stored — [`new`](Self::new) returns
/// `None` for them, mirroring the single-cell rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageSelection {
    /// Where the press started (cell + offset).
    pub anchor: SelectionEndpoint,
    /// Where the drag currently ends (cell + offset); may precede the anchor
    /// in document order after a reversed drag.
    pub focus: SelectionEndpoint,
}

impl MessageSelection {
    /// Builds a span; `None` when both ends are the same cell at the same
    /// offset (a caret is not a selection).
    pub fn new(anchor: SelectionEndpoint, focus: SelectionEndpoint) -> Option<Self> {
        if anchor == focus {
            None
        } else {
            Some(MessageSelection { anchor, focus })
        }
    }

    /// The degenerate span behind a legacy single-cell selection: the range's
    /// start is the anchor, its end the focus.
    pub fn from_single(selection: TextSelection) -> Self {
        MessageSelection {
            anchor: SelectionEndpoint {
                cell: selection.cell.clone(),
                offset: selection.range.start,
            },
            focus: SelectionEndpoint {
                cell: selection.cell,
                offset: selection.range.end,
            },
        }
    }

    /// The legacy single-cell selection when both ends share a cell, with the
    /// range normalized (`start <= end`); `None` for cross-cell spans. A
    /// caret cannot be built through [`new`](Self::new), so this only returns
    /// `None` across cells.
    pub fn single_cell(&self) -> Option<TextSelection> {
        if self.anchor.cell != self.focus.cell {
            return None;
        }
        normalize_range(self.anchor.offset, self.focus.offset).map(|range| TextSelection {
            cell: self.anchor.cell.clone(),
            range,
        })
    }

    /// Position of `cell` in `order` (document order); `None` for keys the
    /// current source no longer renders — stale selections select nothing,
    /// the way unknown keys select nothing on the legacy path.
    fn position(order: &[SelectionKey], cell: &SelectionKey) -> Option<usize> {
        order.iter().position(|key| key == cell)
    }

    /// The visible range over a cell holding `text_len` bytes, given the
    /// cells' document `order`: a partial slice in an endpoint cell, the full
    /// `0..text_len` for cells strictly between the ends, and the normalized
    /// range when both ends share the cell. `None` when the cell takes no
    /// part in the span (outside it, an empty slice, or a key the source no
    /// longer renders). Reversed drags (focus before anchor) render exactly
    /// like forward ones.
    pub fn range_for_cell(
        &self,
        cell: &SelectionKey,
        text_len: usize,
        order: &[SelectionKey],
    ) -> Option<Range<usize>> {
        let here = Self::position(order, cell)?;
        let anchor_ix = Self::position(order, &self.anchor.cell)?;
        let focus_ix = Self::position(order, &self.focus.cell)?;
        if self.anchor.cell == self.focus.cell {
            if *cell != self.anchor.cell {
                return None;
            }
            let mut range = normalize_range(self.anchor.offset, self.focus.offset)?;
            range.start = range.start.min(text_len);
            range.end = range.end.min(text_len);
            return if range.start < range.end {
                Some(range)
            } else {
                None
            };
        }
        let (lo_ix, lo_off, hi_ix, hi_off) = if anchor_ix < focus_ix {
            (anchor_ix, self.anchor.offset, focus_ix, self.focus.offset)
        } else {
            (focus_ix, self.focus.offset, anchor_ix, self.anchor.offset)
        };
        if here < lo_ix || here > hi_ix {
            return None;
        }
        if text_len == 0 {
            return None;
        }
        if here > lo_ix && here < hi_ix {
            return Some(0..text_len);
        }
        if here == lo_ix {
            let start = lo_off.min(text_len);
            return if start < text_len {
                Some(start..text_len)
            } else {
                None
            };
        }
        if here == hi_ix {
            let end = hi_off.min(text_len);
            return if end > 0 { Some(0..end) } else { None };
        }
        None
    }
}

/// One cross-cell selection event from a [`SelectableText`] cell (or a code
/// block line, translated to block-wide offsets): the app folds these into
/// its held span through [`SpanSession::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanEvent {
    /// A single-click press landed at `offset` in `cell`: the session opens,
    /// holding whatever the app held until a hover commits a span.
    Press {
        /// The cell the press landed in.
        cell: SelectionKey,
        /// Byte offset into the cell's shaped text.
        offset: usize,
    },
    /// The pointer moved over `cell` at `offset` while a mouse button is
    /// held. Emitted regardless of which cell the press started in — that is
    /// what lets a drag span cells — and ignored by [`SpanSession::apply`]
    /// when no session is open, so drags that started in other views never
    /// leak in.
    Hover {
        /// The hovered cell.
        cell: SelectionKey,
        /// Byte offset into the cell's shaped text.
        offset: usize,
    },
    /// The button released after a press that started in `cell`: `hovered`
    /// tells whether the pointer is still over that cell, `link` whether the
    /// press-release landed on one link range (a link click keeps the held
    /// span). Every cell's release handler fires window-wide; only the press
    /// cell's release folds, the rest are ignored.
    Release {
        /// The cell the press started in.
        cell: SelectionKey,
        /// Whether the pointer is still over the press cell.
        hovered: bool,
        /// Whether the press-release landed on one link range.
        link: bool,
    },
    /// A double-click word or triple-click paragraph pick replaces the held
    /// span; `None` clears (a double-click on whitespace picks nothing,
    /// exactly like the legacy path).
    Pick {
        /// The picked span, or `None` to clear.
        selection: Option<MessageSelection>,
    },
}

/// A cross-cell selection intent: each [`SpanEvent`] folded through the
/// view's [`SpanSession`]. Kept separate from [`SelectionHandler`] so views
/// that never span (and the harness's `select-text:` step) keep compiling
/// untouched.
pub type SpanHandler = Rc<dyn Fn(SpanEvent, &mut Window, &mut App)>;

/// The caller-owned cross-cell drag session: the pending anchor while the
/// button is held, and whether the pointer has moved since the press. The app
/// holds one per markdown view next to its `Option<MessageSelection>` and
/// folds every [`SpanEvent`] through [`apply`](Self::apply); both live in app
/// state, so scrolling, virtualization and re-renders never reset them
/// mid-drag.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SpanSession {
    /// The press endpoint while the button is held; `None` at rest.
    anchor: Option<SelectionEndpoint>,
    /// Whether a hover has extended the press yet.
    moved: bool,
}

impl SpanSession {
    /// Whether a press is currently held open.
    pub fn is_active(&self) -> bool {
        self.anchor.is_some()
    }

    /// Folds `event` into the session, returning the span the app should
    /// hold: presses only open the session (the held span is untouched, so a
    /// link click or a press-release outside the cell never disturbs it);
    /// hovers with an open session commit the anchor-to-focus span (`None`
    /// when the focus sits back on the anchor — a caret is not a selection);
    /// the press cell's release ends the session, clearing on a plain click
    /// and keeping whatever is held otherwise; every other cell's release and
    /// every hover without an open session leave the held span alone.
    pub fn apply(
        &mut self,
        held: Option<MessageSelection>,
        event: &SpanEvent,
    ) -> Option<MessageSelection> {
        match event {
            SpanEvent::Press { cell, offset } => {
                self.anchor = Some(SelectionEndpoint {
                    cell: cell.clone(),
                    offset: *offset,
                });
                self.moved = false;
                held
            }
            SpanEvent::Hover { cell, offset } => {
                let Some(anchor) = self.anchor.clone() else {
                    return held;
                };
                self.moved = true;
                MessageSelection::new(
                    anchor,
                    SelectionEndpoint {
                        cell: cell.clone(),
                        offset: *offset,
                    },
                )
            }
            SpanEvent::Release {
                cell,
                hovered,
                link,
            } => {
                let Some(anchor) = self.anchor.take() else {
                    return held;
                };
                let moved = std::mem::replace(&mut self.moved, false);
                if anchor.cell != *cell {
                    self.anchor = Some(anchor);
                    return held;
                }
                if *link || moved {
                    held
                } else if *hovered {
                    None
                } else {
                    held
                }
            }
            SpanEvent::Pick { selection } => {
                self.anchor = None;
                self.moved = false;
                selection.clone()
            }
        }
    }
}

/// Orders an anchor/focus pair into a selection range; `None` when the two
/// are equal (a caret is not a selection).
pub(crate) fn normalize_range(anchor: usize, focus: usize) -> Option<Range<usize>> {
    if anchor == focus {
        None
    } else if anchor < focus {
        Some(anchor..focus)
    } else {
        Some(focus..anchor)
    }
}

/// Clamps a range to `text`, snapping both ends down to char boundaries;
/// `None` when nothing remains (empty text, or an empty / fully out-of-range
/// span). Every slice into cell text goes through here, so event indices can
/// never panic a copy.
pub(crate) fn clamp_range(range: Range<usize>, text: &str) -> Option<Range<usize>> {
    if text.is_empty() {
        return None;
    }
    let len = text.len();
    let mut start = range.start.min(len);
    let mut end = range.end.min(len);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    if start >= end {
        None
    } else {
        Some(start..end)
    }
}

/// Intersects a cell-global `range` with the local span `base..base + len`,
/// returning local coordinates; `None` when they do not overlap. Fenced code
/// lines use this to show their slice of the block-wide selection.
pub(crate) fn intersect_range(
    range: &Range<usize>,
    base: usize,
    len: usize,
) -> Option<Range<usize>> {
    let start = range.start.max(base);
    let end = range.end.min(base.saturating_add(len));
    if start < end {
        Some(start - base..end - base)
    } else {
        None
    }
}

/// Whether `c` joins a double-click word: letters, numbers and underscore.
/// Whitespace, punctuation and symbols break words.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The double-click word at byte `index`: the maximal run of word characters
/// around it, or a collapsed range when the index sits on a non-word
/// character. A press past the last glyph belongs to the trailing word when
/// there is one, and selects nothing otherwise.
pub(crate) fn word_range_at(text: &str, index: usize) -> Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let mut ix = index.min(text.len());
    while ix > 0 && !text.is_char_boundary(ix) {
        ix -= 1;
    }
    if ix == text.len() {
        let (last_start, last) = text.char_indices().next_back().expect("non-empty text");
        if !is_word_char(last) {
            return text.len()..text.len();
        }
        ix = last_start;
    }
    let current = text[ix..].chars().next().expect("index is in bounds");
    if !is_word_char(current) {
        return ix..ix;
    }
    let mut start = ix;
    while start > 0 {
        let prev_start = text[..start]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .expect("non-empty head");
        if !is_word_char(text[prev_start..].chars().next().expect("char at boundary")) {
            break;
        }
        start = prev_start;
    }
    let mut end = ix + current.len_utf8();
    while let Some(c) = text[end..].chars().next() {
        if !is_word_char(c) {
            break;
        }
        end += c.len_utf8();
    }
    start..end
}

/// The triple-click paragraph: the whole cell text.
pub(crate) fn paragraph_range(text: &str) -> Range<usize> {
    0..text.len()
}

/// Splits `runs` around `range` and paints the inside pieces with `color`
/// behind the glyphs, so the selection reads exactly like the code-span
/// grounds the transcript already paints. `range` is clamped to `text_len`
/// first; pieces outside it keep their own background (a selected code span
/// swaps its ground for the selection colour).
/// The runs a cell paints before the selection is applied.
///
/// Caller runs win. With none, the cell is one implicit run built by
/// `inherited` — the window's RESOLVED text style — and NOT
/// `TextRun::default()`, whose colour is `Hsla::default()`: fully
/// transparent. `apply_selection` clones each run and sets only
/// `background_color`, so a default base painted the highlight and no glyphs.
///
/// `inherited` is a closure because it needs the `Window`, which tests do not
/// have; passing a run directly is what makes this rule checkable offline.
pub(crate) fn base_runs(
    given: Vec<TextRun>,
    text_len: usize,
    inherited: impl FnOnce() -> TextRun,
) -> Vec<TextRun> {
    if given.is_empty() {
        let mut run = inherited();
        run.len = text_len;
        vec![run]
    } else {
        given
    }
}

pub(crate) fn apply_selection(
    runs: Vec<TextRun>,
    text_len: usize,
    range: &Range<usize>,
    color: Hsla,
) -> Vec<TextRun> {
    let start = range.start.min(text_len);
    let end = range.end.min(text_len);
    if start >= end {
        return runs;
    }
    let mut out = Vec::with_capacity(runs.len() + 2);
    let mut cursor = 0;
    for run in runs {
        let run_end = cursor + run.len;
        if run_end <= start || cursor >= end {
            out.push(run);
        } else {
            if cursor < start {
                out.push(TextRun {
                    len: start - cursor,
                    ..run.clone()
                });
            }
            out.push(TextRun {
                len: run_end.min(end) - start.max(cursor),
                background_color: Some(color),
                ..run.clone()
            });
            if run_end > end {
                out.push(TextRun {
                    len: run_end - end,
                    ..run.clone()
                });
            }
        }
        cursor = run_end;
    }
    out
}

/// Maps a window position to a byte index in `text`, clamping out-of-cell
/// positions to the nearer edge and snapping down to a char boundary. Both
/// `Ok` (inside) and `Err` (past an edge) arms of `index_for_position` carry
/// a usable index.
fn index_at(layout: &TextLayout, position: Point<Pixels>, text: &str) -> usize {
    let ix = match layout.index_for_position(position) {
        Ok(ix) | Err(ix) => ix,
    };
    let mut ix = ix.min(text.len());
    while ix > 0 && !text.is_char_boundary(ix) {
        ix -= 1;
    }
    ix
}

/// Emits a word / paragraph pick over `key` through `emit` as a degenerate
/// single-cell span, clamping to `text`; an empty or unclampable range
/// clears, exactly like [`emit_clamped`] on the legacy path.
fn emit_pick(
    emit: &Option<SpanHandler>,
    key: &SelectionKey,
    range: Range<usize>,
    text: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(emit) = emit else {
        return;
    };
    let selection = clamp_range(range, text).and_then(|range| {
        MessageSelection::new(
            SelectionEndpoint {
                cell: key.clone(),
                offset: range.start,
            },
            SelectionEndpoint {
                cell: key.clone(),
                offset: range.end,
            },
        )
    });
    emit(SpanEvent::Pick { selection }, window, cx);
}

/// Emits `range` over `key` through `emit`, clamping to `text`; an empty or
/// unclampable range clears instead of storing a caret.
fn emit_clamped(
    emit: &Option<SelectionHandler>,
    key: &SelectionKey,
    range: Range<usize>,
    text: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(emit) = emit else {
        return;
    };
    match clamp_range(range, text) {
        Some(range) => emit(
            Some(TextSelection {
                cell: key.clone(),
                range,
            }),
            window,
            cx,
        ),
        None => emit(None, window, cx),
    }
}

/// One selectable run of shaped text. Build with [`selectable_text`].
pub struct SelectableText {
    id: ElementId,
    text: SharedString,
    runs: Vec<TextRun>,
    key: SelectionKey,
    selection: Option<Range<usize>>,
    color: Option<Hsla>,
    links: Vec<Range<usize>>,
    targets: Vec<LinkTarget>,
    on_link: Option<LinkHandler>,
    on_change: Option<SelectionHandler>,
    on_span: Option<SpanHandler>,
    styled: Option<StyledText>,
}

/// Builds a selectable text cell: `key` scopes the selection, `text` is the
/// shaped string. Chain [`SelectableText::runs`] for styling,
/// [`SelectableText::selection`] for the visible range, and
/// [`SelectableText::on_selection_change`] for intents.
pub fn selectable_text(
    id: impl Into<ElementId>,
    key: SelectionKey,
    text: impl Into<SharedString>,
) -> SelectableText {
    SelectableText {
        id: id.into(),
        text: text.into(),
        runs: Vec::new(),
        key,
        selection: None,
        color: None,
        links: Vec::new(),
        targets: Vec::new(),
        on_link: None,
        on_change: None,
        on_span: None,
        styled: None,
    }
}

impl SelectableText {
    /// The text runs (same shape as `StyledText::with_runs`).
    pub fn runs(mut self, runs: Vec<TextRun>) -> Self {
        self.runs = runs;
        self
    }

    /// The visible selection, in local byte indices; `None` (the default)
    /// paints plain text. The range splits the runs around it at paint time.
    pub fn selection(mut self, range: Option<Range<usize>>) -> Self {
        self.selection = range;
        self
    }

    /// The highlight colour behind selected glyphs — the theme's `selection`
    /// token. Without it a selection range paints nothing.
    pub fn selection_color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Clickable link ranges with their targets, in local byte indices. A
    /// press-release without movement on one fires
    /// [`SelectableText::on_link`]; movement starts a selection instead.
    pub fn links(mut self, links: Vec<LinkRange>) -> Self {
        let (ranges, targets) = links
            .into_iter()
            .map(|link| (link.range, link.target))
            .unzip();
        self.links = ranges;
        self.targets = targets;
        self
    }

    /// Fires when a press-release without movement lands on a link range.
    pub fn on_link(mut self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(Rc::new(f));
        self
    }

    /// Fires on drags and word / paragraph picks (`Some`) and on plain
    /// clicks elsewhere in the cell (`None`, clearing the selection).
    pub fn on_selection_change(
        mut self,
        f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Fires cross-cell selection events for the view's [`SpanSession`]: a
    /// single-click press, hovers over this cell while any button is held
    /// (regardless of which cell the press started in), the release after a
    /// press that started here, and double-click word / triple-click
    /// paragraph picks. A view in span mode wires this instead of
    /// [`on_selection_change`](Self::on_selection_change); wiring both
    /// double-reports drags.
    pub fn on_span_event(mut self, f: impl Fn(SpanEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_span = Some(Rc::new(f));
        self
    }

    /// Builds the painted text, baking the selection into the runs. Runs once
    /// per frame in `request_layout`, mirroring how `StyledText` is otherwise
    /// constructed fresh in `render`.
    ///
    /// The runs move into the `StyledText` rather than being cloned: a cell
    /// is consumed by one layout pass, and nothing reads `self.runs` after
    /// this. Text is a `SharedString`, so its clone is a refcount bump.
    fn build_styled(&mut self, window: &mut Window) -> StyledText {
        let text_len = self.text.len();
        let given = std::mem::take(&mut self.runs);
        let had_runs = !given.is_empty();
        // Without caller runs the whole text is one implicit run; with runs
        // the selection splits them. Either way an unhighlighted cell keeps
        // exactly the runs it was given.
        //
        // The implicit run is built from the window's RESOLVED TEXT STYLE, not
        // from `TextRun::default()`. That default carries `Hsla::default()`,
        // which is transparent — and `apply_selection` clones each run and
        // sets only `background_color`, so a run-less cell with a selection
        // colour painted THE HIGHLIGHT AND NO GLYPHS. Reported from Cockpit's
        // document preview: drag a passage and the text vanishes, leaving a
        // bare coloured bar.
        //
        // The bug needed both conditions — no caller runs AND a selection
        // colour — so aui's own two call sites never hit it: markdown.rs and
        // code.rs both pass real runs. The unselected arms did not hit it
        // either, because `StyledText` with no runs inherits the element
        // style. Only the `(Some(range), Some(color))` arm reads the base run,
        // which is why this survived until a caller used it bare.
        let base = base_runs(given, text_len, || window.text_style().to_run(text_len));
        let styled = StyledText::new(self.text.clone());
        match (&self.selection, self.color) {
            (Some(range), Some(color)) => {
                styled.with_runs(apply_selection(base, text_len, range, color))
            }
            _ if !had_runs => styled,
            _ => styled.with_runs(base),
        }
    }
}

/// The in-flight press: the anchor index, whether the pointer has moved off
/// it (a click versus a drag), and which link range the press started on, if
/// any. Element state, like `InteractiveText`'s pending click index.
#[derive(Default)]
struct SelectDrag {
    anchor: Rc<Cell<Option<usize>>>,
    moved: Rc<Cell<bool>>,
    link: Rc<Cell<Option<usize>>>,
}

impl Element for SelectableText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut styled = self.build_styled(window);
        let layout = styled.request_layout(None, inspector_id, window, cx);
        self.styled = Some(styled);
        (layout.0, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        window.with_optional_element_state::<SelectDrag, _>(global_id, |state, window| {
            let state = state.map(|state| state.unwrap_or_default());
            let styled = self
                .styled
                .as_mut()
                .expect("request_layout builds the text");
            let mut unit = ();
            styled.prepaint(None, inspector_id, bounds, &mut unit, window, cx);
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            (hitbox, state)
        })
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self
            .styled
            .as_ref()
            .expect("request_layout builds the text")
            .layout()
            .clone();
        let text = self.text.clone();
        window.with_element_state::<SelectDrag, _>(global_id.unwrap(), |state, window| {
            let state = state.unwrap_or_default();
            let on_link = self.on_link.take();
            let on_change = self.on_change.take();
            let on_span = self.on_span.take();
            let links = std::mem::take(&mut self.links);
            let targets = std::mem::take(&mut self.targets);
            let key = self.key.clone();
            let interactive = on_link.is_some() || on_change.is_some() || on_span.is_some();

            // Cursor affordance: the hand over links when they click
            // (as `InteractiveText` paints it), the I-beam over
            // selectable text.
            if interactive {
                let over_link = layout
                    .index_for_position(window.mouse_position())
                    .ok()
                    .is_some_and(|ix| links.iter().any(|range| range.contains(&ix)));
                if over_link && on_link.is_some() {
                    window.set_cursor_style(CursorStyle::PointingHand, hitbox);
                } else if hitbox.is_hovered(window) {
                    window.set_cursor_style(CursorStyle::IBeam, hitbox);
                }
            }

            // Press, drag and release wire up whenever either intent is
            // present: links must click even when the app never asked for
            // selection intents, and each arm no-ops when its handler is
            // absent (`emit_clamped` returns early on `None`).
            if interactive {
                // Press: record the anchor; double-click takes the word,
                // triple-click the paragraph, both committed at once.
                {
                    let anchor = state.anchor.clone();
                    let moved = state.moved.clone();
                    let pressed = state.link.clone();
                    let layout = layout.clone();
                    let text = text.clone();
                    let links = links.clone();
                    let key = key.clone();
                    let emit = on_change.clone();
                    let span = on_span.clone();
                    let hitbox = hitbox.clone();
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble
                            && event.button == MouseButton::Left
                            && hitbox.is_hovered(window)
                        {
                            let ix = index_at(&layout, event.position, &text);
                            if event.click_count == 2 {
                                anchor.set(None);
                                moved.set(true);
                                pressed.set(None);
                                emit_clamped(
                                    &emit,
                                    &key,
                                    word_range_at(&text, ix),
                                    &text,
                                    window,
                                    cx,
                                );
                                emit_pick(&span, &key, word_range_at(&text, ix), &text, window, cx);
                            } else if event.click_count >= 3 {
                                anchor.set(None);
                                moved.set(true);
                                pressed.set(None);
                                emit_clamped(
                                    &emit,
                                    &key,
                                    paragraph_range(&text),
                                    &text,
                                    window,
                                    cx,
                                );
                                emit_pick(&span, &key, paragraph_range(&text), &text, window, cx);
                            } else {
                                anchor.set(Some(ix));
                                moved.set(false);
                                pressed.set(links.iter().position(|range| range.contains(&ix)));
                                if let Some(emit) = &span {
                                    emit(
                                        SpanEvent::Press {
                                            cell: key.clone(),
                                            offset: ix,
                                        },
                                        window,
                                        cx,
                                    );
                                }
                            }
                            window.refresh();
                        }
                    });
                }
                // Drag: extend while the button is held. Deliberately not
                // hover-gated — the anchor proves the press started here —
                // so leaving the cell keeps extending with the clamped
                // edge index instead of stalling at the border.
                {
                    let anchor = state.anchor.clone();
                    let moved = state.moved.clone();
                    let layout = layout.clone();
                    let text = text.clone();
                    let key = key.clone();
                    let emit = on_change.clone();
                    let span = on_span.clone();
                    let hover_box = hitbox.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && event.dragging() {
                            // Cross-cell hovers are deliberately not
                            // anchor-gated — the press may have
                            // started in another cell — only
                            // hover-gated, and only while a button is
                            // held. Without an open session the app
                            // folds these away, so drags from other
                            // views never leak in; no layout work
                            // happens here beyond this cell's own
                            // hover check and index mapping.
                            if let Some(emit) = &span {
                                if hover_box.is_hovered(window) {
                                    let ix = index_at(&layout, event.position, &text);
                                    emit(
                                        SpanEvent::Hover {
                                            cell: key.clone(),
                                            offset: ix,
                                        },
                                        window,
                                        cx,
                                    );
                                }
                            }
                            if let Some(a) = anchor.get() {
                                let ix = index_at(&layout, event.position, &text);
                                if ix != a {
                                    moved.set(true);
                                    if let Some(range) = normalize_range(a, ix) {
                                        emit_clamped(&emit, &key, range, &text, window, cx);
                                    }
                                }
                            }
                        }
                    });
                }
                // Release: a motionless press-release on a link range is a
                // click; anywhere else in the cell it clears. A drag
                // commits the clamped end when released inside, and keeps
                // the last move's range when released outside.
                {
                    let anchor = state.anchor.clone();
                    let moved = state.moved.clone();
                    let pressed = state.link.clone();
                    let layout = layout.clone();
                    let text = text.clone();
                    let key = key.clone();
                    let emit = on_change.clone();
                    let span = on_span.clone();
                    let hitbox = hitbox.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                            if let Some(a) = anchor.take() {
                                let was_moved = moved.replace(false);
                                let pressed_ix = pressed.take();
                                // Double- and triple-click presses
                                // clear the anchor at press time, so
                                // this only runs for single-click
                                // presses — the session's press cell.
                                if let Some(emit) = &span {
                                    let hovered = hitbox.is_hovered(window);
                                    let link = hovered
                                        && pressed_ix.and_then(|li| links.get(li)).is_some_and(
                                            |range| {
                                                range.contains(&a)
                                                    && range.contains(&index_at(
                                                        &layout,
                                                        event.position,
                                                        &text,
                                                    ))
                                            },
                                        );
                                    emit(
                                        SpanEvent::Release {
                                            cell: key.clone(),
                                            hovered,
                                            link,
                                        },
                                        window,
                                        cx,
                                    );
                                }
                                if !was_moved {
                                    if hitbox.is_hovered(window) {
                                        let ix = index_at(&layout, event.position, &text);
                                        let clicked = pressed_ix.and_then(|li| {
                                            let range = links.get(li)?;
                                            (range.contains(&a) && range.contains(&ix))
                                                .then_some(li)
                                        });
                                        match (clicked, &on_link) {
                                            (Some(li), Some(handler)) => {
                                                if let Some(target) = targets.get(li) {
                                                    handler(target.clone(), window, cx);
                                                }
                                            }
                                            _ => {
                                                if let Some(emit) = &emit {
                                                    emit(None, window, cx);
                                                }
                                            }
                                        }
                                    }
                                } else if hitbox.is_hovered(window) {
                                    let ix = index_at(&layout, event.position, &text);
                                    match normalize_range(a, ix) {
                                        Some(range) => {
                                            emit_clamped(&emit, &key, range, &text, window, cx)
                                        }
                                        None => {
                                            if let Some(emit) = &emit {
                                                emit(None, window, cx);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            }

            self.styled
                .as_mut()
                .expect("request_layout builds the text")
                .paint(None, inspector_id, bounds, &mut (), &mut (), window, cx);
            ((), state)
        });
    }
}

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink() -> Hsla {
        gpui::black()
    }

    fn run(len: usize) -> TextRun {
        TextRun {
            len,
            ..TextRun::default()
        }
    }

    #[test]
    fn a_run_less_cell_inherits_the_element_colour_not_a_transparent_default() {
        // Fails if the implicit run is built from `TextRun::default()`:
        // its colour is `Hsla::default()`, alpha 0, so the glyphs paint
        // invisibly and the cell shows a bare selection bar. Reported from
        // Cockpit's document preview.
        let inherited = TextRun {
            len: 0,
            color: ink(),
            ..TextRun::default()
        };
        let out = base_runs(Vec::new(), 11, || inherited.clone());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len, 11, "the implicit run must span the whole text");
        assert_eq!(
            out[0].color,
            ink(),
            "the implicit run must carry the element colour"
        );
        assert_ne!(
            out[0].color,
            TextRun::default().color,
            "a transparent default is the defect this test exists for"
        );

        // And the colour must survive the selection pass, which only sets a
        // background: highlight AND glyphs, never highlight alone.
        let selected = apply_selection(out, 11, &(0..5), gpui::red());
        assert!(
            selected.iter().all(|r| r.color == ink()),
            "apply_selection must not change the glyph colour"
        );
        assert!(
            selected
                .iter()
                .any(|r| r.background_color == Some(gpui::red())),
            "the selected run must carry the highlight"
        );
    }

    #[test]
    fn caller_runs_are_kept_exactly() {
        // Fails if the implicit run ever replaces runs a caller supplied.
        let given = vec![run(3), run(4)];
        let out = base_runs(given.clone(), 7, || panic!("must not be called"));
        assert_eq!(out.len(), given.len());
    }

    #[test]
    fn normalize_orders_and_drops_carets() {
        assert_eq!(normalize_range(2, 7), Some(2..7));
        assert_eq!(normalize_range(7, 2), Some(2..7));
        assert_eq!(normalize_range(4, 4), None);
    }

    #[test]
    fn clamp_snaps_and_rejects_empty() {
        assert_eq!(clamp_range(2..7, "hello world"), Some(2..7));
        assert_eq!(clamp_range(0..100, "hi"), Some(0..2));
        assert_eq!(clamp_range(5..5, "hello world"), None);
        assert_eq!(clamp_range(0..2, ""), None);
        // Mid-char ends snap down to boundaries.
        assert_eq!(clamp_range(0..2, "héllo"), Some(0..1));
        assert_eq!(clamp_range(1..2, "héllo"), None);
    }

    #[test]
    fn words_expand_over_letters_digits_underscore() {
        assert_eq!(word_range_at("validateAddress now", 3), 0..15);
        assert_eq!(word_range_at("run test_12!", 4), 4..11);
        // Whitespace and punctuation select nothing.
        assert_eq!(word_range_at("hi there", 2), 2..2);
        assert_eq!(word_range_at("a.b", 1), 1..1);
        // Edges: past-the-end belongs to the trailing word; empty is empty.
        assert_eq!(word_range_at("hello", 5), 0..5);
        assert_eq!(word_range_at("hello ", 6), 6..6);
        assert_eq!(word_range_at("", 0), 0..0);
    }

    #[test]
    fn words_handle_multibyte_text() {
        let text = "héllo wörld";
        let word = word_range_at(text, 1);
        assert_eq!(&text[word], "héllo");
        // A mid-char index snaps to its char and still finds the word.
        let snapped = word_range_at(text, 2);
        assert_eq!(&text[snapped], "héllo");
        assert_eq!(paragraph_range(text), 0..text.len());
    }

    #[test]
    fn selection_splits_runs_and_keeps_the_rest() {
        let red = gpui::red();
        let runs = vec![run(5), run(5)];
        let out = apply_selection(runs, 10, &(3..8), red);
        let lens: Vec<usize> = out.iter().map(|run| run.len).collect();
        assert_eq!(lens, vec![3, 2, 3, 2]);
        assert_eq!(
            out.iter()
                .filter(|run| run.background_color == Some(red))
                .count(),
            2
        );
        assert!(out[0].background_color.is_none());
        assert!(out[3].background_color.is_none());
        // Lengths still cover the text exactly.
        assert_eq!(out.iter().map(|run| run.len).sum::<usize>(), 10);
    }

    #[test]
    fn selection_overrides_code_grounds_inside_and_clamps_outside() {
        let red = gpui::red();
        let code = TextRun {
            len: 4,
            background_color: Some(ink()),
            ..TextRun::default()
        };
        let out = apply_selection(vec![code], 4, &(1..3), red);
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].background_color, Some(red));
        assert_eq!(out[0].background_color, Some(ink()));
        // Out-of-range selections leave runs untouched.
        let runs = vec![run(4)];
        assert_eq!(apply_selection(runs.clone(), 4, &(9..12), red), runs);
        assert_eq!(apply_selection(runs.clone(), 4, &(2..2), red), runs);
    }

    #[test]
    fn ranges_intersect_code_lines() {
        assert_eq!(intersect_range(&(3..10), 0, 5), Some(3..5));
        assert_eq!(intersect_range(&(3..10), 5, 4), Some(0..4));
        assert_eq!(intersect_range(&(3..5), 5, 4), None);
        assert_eq!(intersect_range(&(0..2), 5, 0), None);
    }

    fn endpoint(cell: &SelectionKey, offset: usize) -> SelectionEndpoint {
        SelectionEndpoint {
            cell: cell.clone(),
            offset,
        }
    }

    fn three_cells() -> (SelectionKey, SelectionKey, SelectionKey, Vec<SelectionKey>) {
        let a = SelectionKey::paragraph("", 0);
        let b = SelectionKey::list_item("", 1, false, 0);
        let c = SelectionKey::code("", 2);
        let order = vec![a.clone(), b.clone(), c.clone()];
        (a, b, c, order)
    }

    #[test]
    fn spans_drop_carets_like_single_cells() {
        let a = SelectionKey::paragraph("", 0);
        assert!(MessageSelection::new(endpoint(&a, 3), endpoint(&a, 3)).is_none());
        assert!(MessageSelection::new(endpoint(&a, 3), endpoint(&a, 7)).is_some());
        // Same offset across different cells is still a span.
        let b = SelectionKey::paragraph("", 1);
        assert!(MessageSelection::new(endpoint(&a, 3), endpoint(&b, 3)).is_some());
    }

    #[test]
    fn single_cell_spans_round_trip_through_legacy() {
        let single = TextSelection {
            cell: SelectionKey::paragraph("", 0),
            range: 2..7,
        };
        let span = MessageSelection::from_single(single.clone());
        assert_eq!(span.single_cell(), Some(single));
        // Direction survives the round trip normalized.
        let reversed = MessageSelection {
            anchor: endpoint(&span.anchor.cell, 7),
            focus: endpoint(&span.anchor.cell, 2),
        };
        assert_eq!(
            reversed.single_cell().as_ref().map(|s| s.range.clone()),
            Some(2..7)
        );
        // Cross-cell spans have no legacy form.
        let (a, b, _, _) = three_cells();
        let cross =
            MessageSelection::new(endpoint(&a, 0), endpoint(&b, 1)).expect("distinct cells span");
        assert_eq!(cross.single_cell(), None);
    }

    #[test]
    fn ranges_cover_one_cell_end_to_end() {
        let (a, b, _, order) = three_cells();
        let text = "hello world";
        let span = MessageSelection::new(endpoint(&a, 2), endpoint(&a, 7)).expect("a range spans");
        assert_eq!(span.range_for_cell(&a, text.len(), &order), Some(2..7));
        assert_eq!(span.range_for_cell(&b, text.len(), &order), None);
        // Reversed offsets paint the same range.
        let reversed = MessageSelection {
            anchor: endpoint(&a, 7),
            focus: endpoint(&a, 2),
        };
        assert_eq!(reversed.range_for_cell(&a, text.len(), &order), Some(2..7));
    }

    #[test]
    fn ranges_span_two_cells_with_partial_ends() {
        let (a, b, c, order) = three_cells();
        let span = MessageSelection::new(endpoint(&a, 6), endpoint(&b, 3)).expect("two cells span");
        assert_eq!(span.range_for_cell(&a, 11, &order), Some(6..11));
        assert_eq!(span.range_for_cell(&b, 9, &order), Some(0..3));
        assert_eq!(span.range_for_cell(&c, 9, &order), None);
    }

    #[test]
    fn ranges_fill_every_cell_between_reversed_drags_too() {
        let (a, b, c, order) = three_cells();
        // Focus before anchor: the same cells highlight.
        let span =
            MessageSelection::new(endpoint(&c, 4), endpoint(&a, 6)).expect("many cells span");
        assert_eq!(span.range_for_cell(&a, 11, &order), Some(6..11));
        assert_eq!(span.range_for_cell(&b, 9, &order), Some(0..9));
        assert_eq!(span.range_for_cell(&c, 8, &order), Some(0..4));
        let forward =
            MessageSelection::new(endpoint(&a, 6), endpoint(&c, 4)).expect("forward spans");
        for (cell, len) in [(&a, 11), (&b, 9), (&c, 8)] {
            assert_eq!(
                span.range_for_cell(cell, len, &order),
                forward.range_for_cell(cell, len, &order),
                "reversed drags render like forward ones"
            );
        }
    }

    #[test]
    fn ranges_reject_unknown_keys_empty_cells_and_past_end_offsets() {
        let (a, b, _, order) = three_cells();
        let span = MessageSelection::new(endpoint(&a, 6), endpoint(&b, 3)).expect("two cells span");
        let missing = SelectionKey::paragraph("", 9);
        assert_eq!(span.range_for_cell(&missing, 11, &order), None);
        // Empty cells never highlight, even in between.
        assert_eq!(span.range_for_cell(&b, 0, &order), None);
        // Past-the-end offsets clamp; a fully past-the-end end selects nothing.
        assert_eq!(span.range_for_cell(&a, 4, &order), None);
        assert_eq!(span.range_for_cell(&b, 1, &order), Some(0..1));
        // A stale endpoint key drops the whole span.
        let stale = MessageSelection {
            anchor: endpoint(&missing, 0),
            focus: endpoint(&b, 3),
        };
        assert_eq!(stale.range_for_cell(&b, 9, &order), None);
    }

    #[test]
    fn sessions_fold_press_hover_and_release() {
        let (a, b, _, _) = three_cells();
        let mut session = SpanSession::default();
        assert!(!session.is_active());
        let held = session.apply(
            None,
            &SpanEvent::Press {
                cell: a.clone(),
                offset: 6,
            },
        );
        assert_eq!(held, None);
        assert!(session.is_active());
        // Hovers without effect on other views cannot happen here, but a
        // hover back on the anchor collapses to no selection.
        let held = session.apply(
            held,
            &SpanEvent::Hover {
                cell: a.clone(),
                offset: 6,
            },
        );
        assert_eq!(held, None);
        let held = session.apply(
            held,
            &SpanEvent::Hover {
                cell: b.clone(),
                offset: 3,
            },
        );
        let span = held.clone().expect("a drag commits a span");
        assert_eq!(span.anchor, endpoint(&a, 6));
        assert_eq!(span.focus, endpoint(&b, 3));
        // The press cell's release ends the session and keeps the span.
        let held = session.apply(
            held,
            &SpanEvent::Release {
                cell: a.clone(),
                hovered: true,
                link: false,
            },
        );
        assert_eq!(held, Some(span));
        assert!(!session.is_active());
    }

    #[test]
    fn sessions_clear_on_plain_clicks_and_keep_otherwise() {
        let (a, b, _, _) = three_cells();
        let span = MessageSelection::new(endpoint(&a, 1), endpoint(&b, 2)).expect("a held span");
        // A plain click in a fresh press clears.
        let mut session = SpanSession::default();
        let held = session.apply(
            Some(span.clone()),
            &SpanEvent::Press {
                cell: a.clone(),
                offset: 0,
            },
        );
        assert_eq!(held, Some(span.clone()));
        let held = session.apply(
            held,
            &SpanEvent::Release {
                cell: a.clone(),
                hovered: true,
                link: false,
            },
        );
        assert_eq!(held, None);
        // A press released over another cell keeps what was held. Every
        // cell's release handler fires window-wide, so the other cell's
        // release arrives first and is ignored, then the press cell's own
        // release (not hovered) ends the session.
        let mut session = SpanSession::default();
        let held = session.apply(
            Some(span.clone()),
            &SpanEvent::Press {
                cell: a.clone(),
                offset: 0,
            },
        );
        let held = session.apply(
            held,
            &SpanEvent::Release {
                cell: b.clone(),
                hovered: true,
                link: false,
            },
        );
        assert_eq!(held, Some(span.clone()));
        assert!(session.is_active());
        let held = session.apply(
            held,
            &SpanEvent::Release {
                cell: a.clone(),
                hovered: false,
                link: false,
            },
        );
        assert_eq!(held, Some(span.clone()));
        assert!(!session.is_active());
        // Link clicks never disturb the held span.
        let mut session = SpanSession::default();
        let held = session.apply(
            Some(span.clone()),
            &SpanEvent::Press {
                cell: a.clone(),
                offset: 0,
            },
        );
        let held = session.apply(
            held,
            &SpanEvent::Release {
                cell: a.clone(),
                hovered: true,
                link: true,
            },
        );
        assert_eq!(held, Some(span.clone()));
        // Releases from cells that never pressed are ignored.
        let mut session = SpanSession::default();
        let held = session.apply(
            Some(span.clone()),
            &SpanEvent::Release {
                cell: b.clone(),
                hovered: true,
                link: false,
            },
        );
        assert_eq!(held, Some(span.clone()));
        // Hovers with no open session are ignored.
        let mut session = SpanSession::default();
        let held = session.apply(
            Some(span.clone()),
            &SpanEvent::Hover {
                cell: b.clone(),
                offset: 5,
            },
        );
        assert_eq!(held, Some(span.clone()));
    }

    #[test]
    fn sessions_take_picks_and_reset_on_new_presses() {
        let (a, b, _, _) = three_cells();
        let word = MessageSelection::new(endpoint(&b, 2), endpoint(&b, 7)).expect("a word spans");
        let mut session = SpanSession::default();
        session.apply(
            None,
            &SpanEvent::Press {
                cell: a.clone(),
                offset: 0,
            },
        );
        let held = session.apply(
            None,
            &SpanEvent::Pick {
                selection: Some(word.clone()),
            },
        );
        assert_eq!(held, Some(word));
        assert!(!session.is_active());
        // A whitespace double-click clears.
        let held = session.apply(held, &SpanEvent::Pick { selection: None });
        assert_eq!(held, None);
        // A new press overwrites a session a lost release left open.
        let mut session = SpanSession::default();
        session.apply(
            None,
            &SpanEvent::Press {
                cell: a.clone(),
                offset: 1,
            },
        );
        let held = session.apply(
            None,
            &SpanEvent::Press {
                cell: b.clone(),
                offset: 2,
            },
        );
        assert!(session.is_active());
        let held = session.apply(
            held,
            &SpanEvent::Hover {
                cell: b.clone(),
                offset: 4,
            },
        );
        let span = held.expect("the new press anchors the span");
        assert_eq!(span.anchor, endpoint(&b, 2));
    }

    #[test]
    fn keys_scope_quotes_and_tables() {
        assert_eq!(SelectionKey::paragraph("", 0).as_str(), "p0");
        assert_eq!(SelectionKey::heading("", 2).as_str(), "h2");
        assert_eq!(SelectionKey::list_item("", 1, false, 0).as_str(), "b1-0");
        assert_eq!(SelectionKey::list_item("", 1, true, 2).as_str(), "o1-2");
        assert_eq!(SelectionKey::code("", 3).as_str(), "code3");
        assert_eq!(SelectionKey::table_cell("", 4, None, 1).as_str(), "t4-h1");
        assert_eq!(
            SelectionKey::table_cell("", 4, Some(0), 1).as_str(),
            "t4-0-1"
        );
        let prefix = SelectionKey::quote_prefix("", 2);
        assert_eq!(SelectionKey::paragraph(&prefix, 1).as_str(), "q2-p1");
    }
}
