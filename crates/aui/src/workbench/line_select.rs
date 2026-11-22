//! Line-granular selection in the pane's code views (docs/82 7.1): the
//! Changes diff ([`DiffBody`](super::DiffBody)) and the file preview
//! ([`CodeView`](super::CodeView)) let a person pick whole lines and quote
//! them into the composer with the transcript's own selection bar
//! ([`quote_bar`](crate::transcript::quote_bar)).
//!
//! The host owns the selection: it passes `.selected(Some(range))` (a range
//! over the view's selectable lines, end exclusive) and hears
//! `on_select_lines(range)`. A press on a line starts a selection of that
//! line, dragging over further lines extends it, a shift-press extends the
//! held one from its anchor; clearing (Esc, a press elsewhere) is the host's
//! call. Selected lines take the selection tint (`selection`, the same token
//! the transcript's text selections paint with).
//!
//! The bar: the view records where its selected lines were drawn
//! ([`LineSelectGeometry`], window coordinates, filled at prepaint), and the
//! host hangs the bar over its own body with [`line_quote_bar`] — 6 px above
//! the first selected line, or 6 under the last when above would come within
//! 38 px of the body's top, exactly as the transcript's bar
//! ([`quote_bar_top`](crate::transcript::quote_bar_top)).

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, App, Bounds, ClickEvent, ElementId, Entity, MouseButton, Pixels, Window};
use gpui_kit::base::ElementExt as _;

use crate::transcript::{quote_bar, QUOTE_BAR_H};

/// A line selection's report: the new range over the view's selectable
/// lines (end exclusive, never empty).
pub type SelectLinesHandler = Rc<dyn Fn(Range<usize>, &mut Window, &mut App) + 'static>;

/// The lines from `anchor` to `head`, inclusive of both, in order.
pub fn line_range(anchor: usize, head: usize) -> Range<usize> {
    anchor.min(head)..anchor.max(head) + 1
}

/// `range` held to `len` lines; `None` when nothing of it is left.
pub fn clamp_lines(range: &Range<usize>, len: usize) -> Option<Range<usize>> {
    let r = range.start.min(len)..range.end.min(len);
    (!r.is_empty()).then_some(r)
}

/// The text of `lines[range]` as written, one per line; `None` when the
/// range selects nothing.
pub fn selected_lines(lines: &[String], range: &Range<usize>) -> Option<String> {
    let r = clamp_lines(range, lines.len())?;
    Some(lines[r].join("\n"))
}

/// What a press on line `line` makes of the held selection: a shift-press
/// extends from the anchor (the drag's own, else the held range's start),
/// any other press selects the one line.
pub fn press_range(line: usize, shift: bool, held: Option<&Range<usize>>, anchor: Option<usize>) -> (usize, Range<usize>) {
    match (shift, held) {
        (true, Some(held)) => {
            let anchor = anchor.filter(|a| *a == held.start || *a + 1 == held.end).unwrap_or(held.start);
            (anchor, line_range(anchor, line))
        }
        _ => (line, line..line + 1),
    }
}

/// A view's drag between events: the line the press landed on, and whether
/// the button is still down.
#[derive(Default)]
pub(crate) struct DragState {
    pub anchor: Option<usize>,
    pub dragging: bool,
}

/// Makes `row` (selectable line `line`) answer the pointer: a press
/// selects it (or, with shift, extends the held selection from its anchor),
/// dragging over further rows extends from the press, the release ends the
/// drag. The press stops there, so a host's clear-on-press around the view
/// never sees it.
pub(crate) fn selectable_row(row: gpui::Stateful<gpui::Div>, line: usize, held: Option<Range<usize>>, drag: Entity<DragState>, on_select: SelectLinesHandler) -> gpui::Stateful<gpui::Div> {
    let (down, moved, up) = (drag.clone(), drag.clone(), drag);
    let (press, extend) = (on_select.clone(), on_select);
    let held_now = held.clone();
    row.on_mouse_down(MouseButton::Left, move |e, window, cx| {
        cx.stop_propagation();
        let anchor = down.read(cx).anchor;
        let (anchor, range) = press_range(line, e.modifiers.shift, held.as_ref(), anchor);
        down.update(cx, |d, _| {
            d.anchor = Some(anchor);
            d.dragging = true;
        });
        press(range, window, cx);
    })
    .on_mouse_move(move |e, window, cx| {
        let (anchor, dragging) = {
            let d = moved.read(cx);
            (d.anchor, d.dragging)
        };
        if !dragging {
            return;
        }
        if e.pressed_button != Some(MouseButton::Left) {
            moved.update(cx, |d, _| d.dragging = false);
            return;
        }
        let Some(anchor) = anchor else { return };
        let range = line_range(anchor, line);
        if held_now.as_ref() != Some(&range) {
            extend(range, window, cx);
        }
    })
    .on_mouse_up(MouseButton::Left, move |_, _, cx| up.update(cx, |d, _| d.dragging = false))
}

/// Records where `row` (selectable line `ix`) is drawn when it is the
/// selection's first or last line; `text_left` is the text column's offset
/// in the row.
pub(crate) fn note_line<E: ParentElement>(row: E, ix: usize, selected: Option<&Range<usize>>, geometry: Option<&LineSelectGeometry>, text_left: f32) -> E {
    let (Some(r), Some(g)) = (selected, geometry) else { return row };
    let (first, last) = (ix == r.start, ix + 1 == r.end);
    if !first && !last {
        return row;
    }
    let g = g.clone();
    row.on_prepaint(move |bounds, window, _| g.note(bounds, first, last, text_left, window))
}

/// Where the selected lines were drawn, in window coordinates: the top of
/// the selection's first line and the foot of its last, each when that line
/// was laid out this frame, and the left edge of the text.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineSpan {
    /// The first selected line's top, when it was drawn.
    pub first_top: Option<f32>,
    /// The last selected line's foot, when it was drawn.
    pub last_bottom: Option<f32>,
    /// The text column's left edge.
    pub left: f32,
}

#[derive(Default)]
struct GeomState {
    prev: Option<LineSpan>,
    cur: Option<LineSpan>,
}

/// The selected lines' geometry, shared between a view (which records it at
/// prepaint) and its host (which reads last frame's to place the bar). A
/// change asks for one more frame, so the bar lands one frame after the
/// lines move and then rests.
#[derive(Clone, Default)]
pub struct LineSelectGeometry(Rc<RefCell<GeomState>>);

impl LineSelectGeometry {
    /// Where the selected lines were last drawn; `None` when none was.
    pub fn span(&self) -> Option<LineSpan> {
        self.0.borrow().cur
    }

    /// A view starts a frame: what it records next replaces the last.
    pub(crate) fn begin(&self) {
        let mut g = self.0.borrow_mut();
        g.prev = g.cur.take();
    }

    /// A selected line was laid out at `bounds`; `first` / `last` say
    /// whether it is the selection's first or last line (a wrapped line
    /// reports every row: the first row's top and the last row's foot win).
    pub(crate) fn note(&self, bounds: Bounds<Pixels>, first: bool, last: bool, text_left: f32, window: &mut Window) {
        let mut g = self.0.borrow_mut();
        let span = g.cur.get_or_insert(LineSpan { first_top: None, last_bottom: None, left: f32::from(bounds.origin.x) + text_left });
        let top = f32::from(bounds.origin.y);
        let bottom = f32::from(bounds.origin.y + bounds.size.height);
        if first {
            span.first_top = Some(span.first_top.map_or(top, |t| t.min(top)));
        }
        if last {
            span.last_bottom = Some(span.last_bottom.map_or(bottom, |b| b.max(bottom)));
        }
        let moved = match (g.prev, g.cur) {
            (Some(a), Some(b)) => !close(a.first_top, b.first_top) || !close(a.last_bottom, b.last_bottom) || (a.left - b.left).abs() > 0.5,
            _ => true,
        };
        if moved {
            // Keep `prev` in step so one change asks for one frame.
            g.prev = g.cur;
            drop(g);
            window.request_animation_frame();
        }
    }
}

fn close(a: Option<f32>, b: Option<f32>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= 0.5,
        (None, None) => true,
        _ => false,
    }
}

/// The bar's top in a body `height` tall, for a selection whose first line
/// starts at `first_top` and whose last ends at `last_bottom` (body
/// coordinates; `None` for a line not drawn). The bar sits **beside** the
/// selection, level with its first line (the end of that line, see
/// [`line_quote_bar`]), so it never hides a line the person did not select —
/// hung above, it covered the line over the selection (22 Nov). When the first
/// line has scrolled off the top, it stays level with the last line's foot
/// instead; `None` when neither end is in view.
pub fn line_bar_top(first_top: Option<f32>, last_bottom: Option<f32>, height: f32) -> Option<f32> {
    let lowest = (height - QUOTE_BAR_H).max(0.0);
    match (first_top, last_bottom) {
        (Some(first), _) if first < height => Some(first.clamp(0.0, lowest)),
        (None, Some(last)) if last > 0.0 => Some((last - QUOTE_BAR_H).clamp(0.0, lowest)),
        _ => None,
    }
}

/// Where the bar was last placed, so its 60 ms exit fades in place.
#[derive(Default)]
struct BarSeat(Option<(f32, f32)>);

/// The selection bar over a line selection: `quote_bar` hung from the
/// lines `geometry` recorded, inside a body whose last-drawn bounds are
/// `body` (window coordinates; the result is absolutely placed in that
/// body, so the body must be `relative`). `present` is whether a selection
/// is held and quoting is possible. `None` once it has left or while
/// neither end of the selection is in view.
pub fn line_quote_bar(
    id: impl Into<ElementId>,
    present: bool,
    geometry: &LineSelectGeometry,
    body: Option<Bounds<Pixels>>,
    on_quote: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let id: ElementId = id.into();
    let seat = window.use_keyed_state((id.clone(), "seat"), cx, |_, _| BarSeat::default());
    let placed = match (present, geometry.span(), body) {
        (true, Some(span), Some(body)) => {
            let origin_y = f32::from(body.origin.y);
            let top = line_bar_top(span.first_top.map(|t| t - origin_y), span.last_bottom.map(|b| b - origin_y), f32::from(body.size.height));
            top.map(|top| {
                let width = f32::from(body.size.width);
                // Right-aligned: the end of the line is the part of a code line
                // that is usually empty.
                let left = (width - 120.0).max(8.0);
                (top, left)
            })
        }
        _ => None,
    };
    if let Some(at) = placed {
        seat.update(cx, |s, _| s.0 = Some(at));
    }
    let (top, left) = seat.read(cx).0?;
    let bar = quote_bar(id, placed.is_some(), on_quote, window, cx)?;
    Some(
        div()
            .absolute()
            .top(px(top))
            .left(px(left))
            // A press on the bar is not a press "elsewhere": the host's
            // clear-on-press never sees it.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(bar)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_runs_either_way_and_includes_both_ends() {
        assert_eq!(line_range(3, 7), 3..8);
        assert_eq!(line_range(7, 3), 3..8);
        assert_eq!(line_range(4, 4), 4..5);
    }

    #[test]
    fn a_shift_press_extends_from_the_anchor() {
        // A plain press selects its one line.
        assert_eq!(press_range(5, false, Some(&(1..3)), Some(1)), (5, 5..6));
        // Shift extends from the drag's anchor…
        assert_eq!(press_range(6, true, Some(&(2..5)), Some(4)), (4, 4..7));
        // …or from the held range's start when the anchor is stale.
        assert_eq!(press_range(0, true, Some(&(2..5)), Some(9)), (2, 0..3));
        // Shift with nothing held is a plain press.
        assert_eq!(press_range(3, true, None, None), (3, 3..4));
    }

    #[test]
    fn selected_lines_join_as_written_and_clamp_to_the_file() {
        let lines: Vec<String> = ["a", "  b", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(selected_lines(&lines, &(1..3)).as_deref(), Some("  b\nc"));
        assert_eq!(selected_lines(&lines, &(2..9)).as_deref(), Some("c"));
        assert_eq!(selected_lines(&lines, &(5..9)), None);
        assert_eq!(clamp_lines(&(0..0), 3), None);
    }

    #[test]
    fn the_bar_sits_level_with_the_selection_never_above_it() {
        // Level with the first line, wherever it is.
        assert_eq!(line_bar_top(Some(200.0), Some(260.0), 600.0), Some(200.0));
        assert_eq!(line_bar_top(Some(40.0), Some(100.0), 600.0), Some(40.0));
        // A first line a little above the body's top (partly scrolled): the top edge.
        assert_eq!(line_bar_top(Some(-8.0), Some(60.0), 600.0), Some(0.0));
        // Near the foot: kept inside the body.
        assert_eq!(line_bar_top(Some(590.0), Some(610.0), 600.0), Some(568.0));
        // The last line is off the foot: still level with the first.
        assert_eq!(line_bar_top(Some(40.0), None, 600.0), Some(40.0));
        // Only the last line in view (the first scrolled off the top): ends at its foot.
        assert_eq!(line_bar_top(None, Some(80.0), 600.0), Some(48.0));
        // Neither end in view.
        assert_eq!(line_bar_top(None, None, 600.0), None);
        assert_eq!(line_bar_top(Some(900.0), Some(960.0), 600.0), None);
    }
}
