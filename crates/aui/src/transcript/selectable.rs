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
    if start >= end { None } else { Some(start..end) }
}

/// Intersects a cell-global `range` with the local span `base..base + len`,
/// returning local coordinates; `None` when they do not overlap. Fenced code
/// lines use this to show their slice of the block-wide selection.
pub(crate) fn intersect_range(range: &Range<usize>, base: usize, len: usize) -> Option<Range<usize>> {
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

    /// Builds the painted text, baking the selection into the runs. Runs once
    /// per frame in `request_layout`, mirroring how `StyledText` is otherwise
    /// constructed fresh in `render`.
    fn build_styled(&self) -> StyledText {
        let text_len = self.text.len();
        // Without caller runs the whole text is one implicit run; with runs
        // the selection splits them. Either way an unhighlighted cell keeps
        // exactly the runs it was given.
        let base = if self.runs.is_empty() {
            vec![TextRun {
                len: text_len,
                ..TextRun::default()
            }]
        } else {
            self.runs.clone()
        };
        let styled = StyledText::new(self.text.clone());
        match (&self.selection, self.color) {
            (Some(range), Some(color)) => {
                styled.with_runs(apply_selection(base, text_len, range, color))
            }
            _ if self.runs.is_empty() => styled,
            _ => styled.with_runs(self.runs.clone()),
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
        let mut styled = self.build_styled();
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
        window.with_optional_element_state::<SelectDrag, _>(
            global_id,
            |state, window| {
                let state = state.map(|state| state.unwrap_or_default());
                let styled = self
                    .styled
                    .as_mut()
                    .expect("request_layout builds the text");
                let mut unit = ();
                styled.prepaint(None, inspector_id, bounds, &mut unit, window, cx);
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                (hitbox, state)
            },
        )
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
        window.with_element_state::<SelectDrag, _>(
            global_id.unwrap(),
            |state, window| {
                let state = state.unwrap_or_default();
                let on_link = self.on_link.take();
                let on_change = self.on_change.take();
                let links = std::mem::take(&mut self.links);
                let targets = std::mem::take(&mut self.targets);
                let key = self.key.clone();
                let interactive = on_link.is_some() || on_change.is_some();

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
                        let hitbox = hitbox.clone();
                        window.on_mouse_event(
                            move |event: &MouseDownEvent, phase, window, cx| {
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
                                    } else {
                                        anchor.set(Some(ix));
                                        moved.set(false);
                                        pressed.set(
                                            links.iter().position(|range| range.contains(&ix)),
                                        );
                                    }
                                    window.refresh();
                                }
                            },
                        );
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
                        window.on_mouse_event(
                            move |event: &MouseMoveEvent, phase, window, cx| {
                                if phase == DispatchPhase::Bubble && event.dragging() {
                                    if let Some(a) = anchor.get() {
                                        let ix = index_at(&layout, event.position, &text);
                                        if ix != a {
                                            moved.set(true);
                                            if let Some(range) = normalize_range(a, ix) {
                                                emit_clamped(
                                                    &emit, &key, range, &text, window, cx,
                                                );
                                            }
                                        }
                                    }
                                }
                            },
                        );
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
                        let hitbox = hitbox.clone();
                        window.on_mouse_event(
                            move |event: &MouseUpEvent, phase, window, cx| {
                                if phase == DispatchPhase::Bubble
                                    && event.button == MouseButton::Left
                                {
                                    if let Some(a) = anchor.take() {
                                        let was_moved = moved.replace(false);
                                        let pressed_ix = pressed.take();
                                        if !was_moved {
                                            if hitbox.is_hovered(window) {
                                                let ix = index_at(
                                                    &layout,
                                                    event.position,
                                                    &text,
                                                );
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
                                            let ix =
                                                index_at(&layout, event.position, &text);
                                            match normalize_range(a, ix) {
                                                Some(range) => emit_clamped(
                                                    &emit, &key, range, &text, window, cx,
                                                ),
                                                None => {
                                                    if let Some(emit) = &emit {
                                                        emit(None, window, cx);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        );
                    }
                }

                self.styled
                    .as_mut()
                    .expect("request_layout builds the text")
                    .paint(None, inspector_id, bounds, &mut (), &mut (), window, cx);
                ((), state)
            },
        );
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
        assert_eq!(out.iter().filter(|run| run.background_color == Some(red)).count(), 2);
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
