//! [`RowList`]: a column of rows that each hover with the one model.
//!
//! It replaced `FluidList` (audit C-11, docs/83 §9): no gliding block, no
//! last-frame geometry, no pointer session. Each highlightable item is
//! wrapped in a rounded box that paints its own states ([`StatePaint`]):
//! hover and press are gpui refinements, the selected item holds
//! `ov-selected`, the keyboard's item holds the hover look. Activation and
//! the pointer report are per item.
//!
//! Rows built outside the list's own children (the render closure of a
//! virtualised `gpui::list`) wrap themselves with the same handlers through
//! [`ListItems`], which a host takes from [`RowList::items`] before the
//! list is consumed.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, App, Bounds, ElementId, IntoElement, Pixels, RenderOnce, Role, SharedString, Window};

use super::paint::{PaintStates, StatePaint};
use super::{ROW_GAP, R_ITEM};
use crate::util::indexed_child;

type HighlightHandler = Rc<dyn Fn(Option<usize>, Option<Bounds<Pixels>>, &mut Window, &mut App)>;
type ActivateHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The per-item wrapping a [`RowList`] applies, shareable with rows built
/// elsewhere (a virtualised `gpui::list` inside the list). Cheap to clone.
#[derive(Clone)]
pub struct ListItems {
    id: ElementId,
    on_activate: Option<ActivateHandler>,
    on_highlight: Option<HighlightHandler>,
    /// The item the pointer report last named, across frames: a leave only
    /// reports `None` when it leaves the item last reported, so moving from
    /// one item to the next never reports `None` in between.
    reported: Rc<Cell<Option<usize>>>,
    selected: Option<usize>,
    highlighted: Option<usize>,
    inert: Option<usize>,
    pointer: bool,
}

impl ListItems {
    /// Wraps `element` as highlightable item `index`: the one model's paint
    /// (selected / highlighted from the list), a click that activates
    /// `index`, and the pointer report.
    pub fn item(&self, index: usize, element: impl IntoElement, cx: &App) -> AnyElement {
        if self.inert == Some(index) {
            return div().w_full().child(element).into_any_element();
        }
        let paint = StatePaint::new()
            .selected(self.selected == Some(index))
            .highlighted(self.highlighted == Some(index))
            .pointer(self.pointer);
        self.wrap(index, element, paint, cx)
    }

    /// Like [`Self::item`] with a caller-built paint (a row that knows its
    /// own selection).
    pub fn wrap(&self, index: usize, element: impl IntoElement, paint: StatePaint, cx: &App) -> AnyElement {
        let mut el = div()
            .id(indexed_child(&self.id, "item", index))
            .relative()
            .w_full()
            .rounded(px(R_ITEM))
            .paint_states(paint, cx);
        if let Some(activate) = self.on_activate.clone() {
            el = el.cursor_pointer().on_click(move |_, w, cx| activate(index, w, cx));
        }
        if let Some(report) = self.on_highlight.clone() {
            let bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
            let measured = bounds.clone();
            let reported = self.reported.clone();
            el = gpui_kit::base::ElementExt::on_prepaint(el, move |b, _, _| measured.set(Some(b)))
                .on_hover(move |hovered, w, cx| {
                    if *hovered {
                        if reported.get() != Some(index) {
                            reported.set(Some(index));
                            report(Some(index), bounds.get(), w, cx);
                        }
                    } else if reported.get() == Some(index) {
                        reported.set(None);
                        report(None, None, w, cx);
                    }
                });
        }
        el.child(element).into_any_element()
    }
}

enum Kind {
    Item,
    Static,
    Fill,
}

/// A column of rows. Build with [`row_list`].
#[derive(IntoElement)]
pub struct RowList {
    id: ElementId,
    items: Vec<(usize, AnyElement, Kind)>,
    selected: Option<usize>,
    highlighted: Option<usize>,
    inert: Option<usize>,
    pointer: bool,
    gap: Pixels,
    fill: bool,
    label: Option<SharedString>,
    on_activate: Option<ActivateHandler>,
    on_highlight: Option<HighlightHandler>,
}

/// An empty [`RowList`] keyed `id`, rows [`ROW_GAP`] apart.
pub fn row_list(id: impl Into<ElementId>) -> RowList {
    RowList {
        id: id.into(),
        items: Vec::new(),
        selected: None,
        highlighted: None,
        inert: None,
        pointer: true,
        gap: px(ROW_GAP),
        fill: false,
        label: None,
        on_activate: None,
        on_highlight: None,
    }
}

impl RowList {
    /// Adds highlightable item `index` (indices are the caller's: they come
    /// back through `on_activate` and `on_highlight`).
    pub fn item(mut self, index: usize, element: impl IntoElement) -> Self {
        self.items.push((index, element.into_any_element(), Kind::Item));
        self
    }

    /// Adds an item that is never lit and takes no activation: a label, a
    /// separator, a toolbar, an open diff under its row.
    pub fn item_static(mut self, index: usize, element: impl IntoElement) -> Self {
        self.items.push((index, element.into_any_element(), Kind::Static));
        self
    }

    /// A static item that grows to fill the list's height (a virtualised
    /// `gpui::list` needs a definite height). Pair with [`Self::fill`].
    pub fn item_static_fill(mut self, index: usize, element: impl IntoElement) -> Self {
        self.items.push((index, element.into_any_element(), Kind::Fill));
        self
    }

    /// The space between items ([`ROW_GAP`] by default).
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
    }

    /// The selected item: it holds `ov-selected`.
    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// The keyboard's item: it holds the hover look.
    pub fn highlighted(mut self, highlighted: Option<usize>) -> Self {
        self.highlighted = highlighted;
        self
    }

    /// An item that paints nothing and takes nothing (a row being renamed:
    /// the field is the row).
    pub fn inert(mut self, index: Option<usize>) -> Self {
        self.inert = index;
        self
    }

    /// Whether the pointer's hover refinement paints (default true). A list
    /// whose host lights one row from both the pointer report and the keys
    /// turns it off and passes that row as [`Self::highlighted`].
    pub fn pointer(mut self, pointer: bool) -> Self {
        self.pointer = pointer;
        self
    }

    /// The list fills its parent's height.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// The list's accessible name.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A click on item `index` (or Enter from the host's keyboard handling).
    pub fn on_activate(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }

    /// The pointer entered item `index`, or left the list's items (`None`).
    pub fn on_highlight(mut self, f: impl Fn(Option<usize>, &mut Window, &mut App) + 'static) -> Self {
        self.on_highlight = Some(Rc::new(move |index, _, w, cx| f(index, w, cx)));
        self
    }

    /// Like [`Self::on_highlight`], with the item's bounds in window
    /// coordinates (a hover card anchors to them).
    pub fn on_highlight_at(mut self, f: impl Fn(Option<usize>, Option<Bounds<Pixels>>, &mut Window, &mut App) + 'static) -> Self {
        self.on_highlight = Some(Rc::new(f));
        self
    }

    /// The wrapping this list applies, for rows built outside it. Call
    /// after the list's selection, handlers and pointer rule are set.
    pub fn items(&self, window: &mut Window, cx: &mut App) -> ListItems {
        let reported = window
            .use_keyed_state((self.id.clone(), "aui-list-reported"), cx, |_, _| Rc::new(Cell::new(None::<usize>)))
            .read(cx)
            .clone();
        ListItems {
            id: self.id.clone(),
            on_activate: self.on_activate.clone(),
            on_highlight: self.on_highlight.clone(),
            reported,
            selected: self.selected,
            highlighted: self.highlighted,
            inert: self.inert,
            pointer: self.pointer,
        }
    }
}

impl RenderOnce for RowList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let wrap = self.items(window, cx);
        let mut col = div()
            .id((self.id.clone(), "aui-row-list"))
            .flex()
            .flex_col()
            .w_full()
            .gap(self.gap)
            .role(Role::List)
            .when(self.fill, |d| d.h_full().min_h(px(0.0)));
        if let Some(label) = self.label {
            col = col.aria_label(label);
        }
        for (index, element, kind) in self.items {
            col = col.child(match kind {
                Kind::Item => wrap.item(index, element, cx),
                Kind::Static => element,
                Kind::Fill => div().w_full().flex_1().min_h(px(0.0)).flex().flex_col().child(element).into_any_element(),
            });
        }
        col
    }
}
