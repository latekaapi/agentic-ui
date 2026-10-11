//! The three composer chip menus: **model**, **effort** and **approval mode**.
//!
//! One component with three constructors, because they are one control with
//! three data sets: a list of rows with a label, a one-line detail, an optional
//! trailing meta (a model's context limit) and any number of badges
//! (`default`, `active`), plus a check on the selected row.
//!
//! They are the one menu panel ([`crate::interact::Dropdown`], FF's dropdown),
//! painted through [`crate::overlay::popover_layer`] and anchored by the
//! composer to the chip that opened them
//! ([`crate::composer::Composer::chip_menu`]).
//!
//! Stateless: rows in, `selected` in, `open` in; pick, hover and close out. In
//! particular **the selection is not a fact about the rows**. MSP says a client
//! must not assume exactly one `isActive` or `isDefault` model, so "active" is a
//! badge that any number of rows may wear, and `selected` is only where the
//! keyboard is.

use std::rc::Rc;

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;

use crate::data::tag;
use crate::interact::{dropdown, dropdown_present, DropdownEntry, DropdownItem, DropdownSide, DropdownTrailing};
use crate::overlay::popover_layer;

/// `.pick{bottom:28px;left:0;padding:6px}` — measured from the chip it hangs
/// off, which is 28 px tall like every composer toolbar control.
const MENU_BOTTOM: f32 = 28.0;
/// The model menu carries a context limit and two badges, so it is wider than
/// the `+` menu's 200; effort and mode share the floor so the three read as one
/// control.
///
/// It is a **floor**, not a width. A provider names its models what it likes and
/// MSP's catalog is discovered at runtime, so a fixed width would have to cut a
/// name off to fit — and half a model name is not a model name (finding F8).
/// Nothing here ever ellipsises: the menu is at least this wide, grows with its
/// content up to the ceiling, and past that the label wraps rather than being
/// cut.
///
/// The floor is set to the widest name Muse's own catalog ships
/// (`muse-spark-1.2-contributor` with its context limit and two badges) because
/// gpui's absolute layout does not shrink-to-fit a column of stretched rows: a
/// row that fills its parent and a parent that sizes to its rows is circular,
/// and taffy resolves that circle at the floor. So the floor has to be a width
/// that is actually right, not a token minimum nobody expects to see.
const MENU_W_MIN: f32 = 360.0;
const MENU_W_MAX: f32 = 520.0;
/// The gap between a row's meta and its badges.
const ITEM_GAP: f32 = 8.0;
/// The meta's mono size.
const DETAIL_TEXT: f32 = scale::FS_11;

/// One row of a composer picker.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerRow {
    /// Stable identity, handed back by [`PickerMenu::on_pick`].
    pub id: SharedString,
    /// The row's name (`muse-spark-1.3`, `Extra high`, `Read-only`).
    pub label: SharedString,
    /// The one-line description under it; empty draws no second line.
    pub detail: SharedString,
    /// A trailing mono fact — a model's context limit, say. `None` draws none.
    pub meta: Option<SharedString>,
    /// Tags at the right edge (`default`, `active`). A model row may carry
    /// none, one or both, and several rows may carry the same one.
    pub badges: Vec<SharedString>,
}

impl PickerRow {
    /// A row with a label and a description.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), detail: detail.into(), meta: None, badges: Vec::new() }
    }

    /// The trailing mono fact.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Adds one badge at the right edge.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badges.push(badge.into());
        self
    }
}

type PickHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A composer chip menu. Build with [`model_menu`], [`effort_menu`] or
/// [`mode_menu`].
#[derive(IntoElement)]
pub struct PickerMenu {
    id: ElementId,
    title: SharedString,
    rows: Vec<PickerRow>,
    selected: usize,
    open: bool,
    at_rest: bool,
    on_pick: Option<PickHandler>,
    on_hover: Option<HoverHandler>,
    on_close: Option<CloseHandler>,
}

/// The model picker: rows from the provider catalog, `selected` on the row the
/// session is actually using.
///
/// `meta` is where the context limit goes and `badges` where `default` and
/// `active` go; both are the caller's to fill, because MSP's catalog can flag
/// any number of rows either way.
pub fn model_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    menu(id, "Model", rows, selected, open)
}

/// The reasoning-effort picker.
pub fn effort_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    menu(id, "Reasoning effort", rows, selected, open)
}

/// The approval-mode picker.
pub fn mode_menu(id: impl Into<ElementId>, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    menu(id, "Approval mode", rows, selected, open)
}

fn menu(id: impl Into<ElementId>, title: &'static str, rows: Vec<PickerRow>, selected: usize, open: bool) -> PickerMenu {
    PickerMenu {
        id: id.into(),
        title: title.into(),
        rows,
        selected,
        open,
        at_rest: false,
        on_pick: None,
        on_hover: None,
        on_close: None,
    }
}

impl PickerMenu {
    /// Replaces the caps header.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Skips the enter (static captures).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// A row was activated; the argument is its [`PickerRow::id`].
    pub fn on_pick(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(f));
        self
    }

    /// The pointer entered a row; the argument is its index, so the caller can
    /// move the selection to it and keep one highlight on screen.
    pub fn on_hover(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    /// A click outside the menu.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for PickerMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // At rest a closed menu draws nothing (static captures).
        let present = if self.at_rest { self.open } else { dropdown_present((id.clone(), "panel"), self.open, window, cx) };
        if !present {
            return div().invisible().into_any_element();
        }
        // The one menu panel (FF's dropdown): the title as FF's group label,
        // then two-line rows; `selected` is the keyboard's row and wears the
        // check, the pointer drives the same gliding block.
        let mut panel = dropdown((id.clone(), "panel"), self.title.clone())
            .min_width(px(MENU_W_MIN))
            .side(DropdownSide::Above)
            .open(self.open)
            .highlighted(Some(self.selected + 1))
            .entry(DropdownEntry::Label(self.title.clone()));
        if self.at_rest {
            panel = panel.at_rest();
        }
        if let Some(hover) = self.on_hover.clone() {
            panel = panel.on_highlight(move |index, w, cx| {
                if let Some(row) = index.and_then(|i| i.checked_sub(1)) {
                    hover(row, w, cx);
                }
            });
        }
        for (index, row) in self.rows.into_iter().enumerate() {
            let mut trailing = h_flex().gap(px(ITEM_GAP)).items_center();
            if let Some(meta) = &row.meta {
                trailing = trailing.child(div().flex_none().mono(DETAIL_TEXT).text_color(p.ink_3).child(meta.clone()));
            }
            for badge in &row.badges {
                trailing = trailing.child(tag(badge.clone()));
            }
            let mut item = DropdownItem::new(row.label.clone()).detail(row.detail.clone()).wrap().checked(index == self.selected);
            if row.meta.is_some() || !row.badges.is_empty() {
                item = item.trailing(DropdownTrailing::Element(trailing.into_any_element()));
            }
            if let Some(handler) = self.on_pick.clone() {
                let row_id = row.id.clone();
                item = item.on_select(move |w, cx| handler(&row_id, w, cx));
            }
            panel = panel.entry(item);
        }
        let menu = div().id(id.clone()).absolute().bottom(px(MENU_BOTTOM)).left(px(0.0)).max_w(px(MENU_W_MAX)).child(panel);

        // A click anywhere else closes it. The catcher is a sibling of the menu
        // inside the same deferred draw, so it covers the window without
        // covering the menu.
        let mut layer = div().absolute().child(menu);
        if let Some(close) = self.on_close.clone() {
            layer = div()
                .absolute()
                .child(
                    div()
                        .id((id.clone(), "scrim"))
                        .occlude()
                        .absolute()
                        // The catcher is anchored to the chip, so it has to
                        // reach far enough in every direction to cover a
                        // 1440 × 900 window from wherever the chip sits.
                        .top(px(-SCRIM_REACH))
                        .left(px(-SCRIM_REACH))
                        .w(px(SCRIM_REACH * 2.0))
                        .h(px(SCRIM_REACH * 2.0))
                        .on_click(move |_, w, cx| close(w, cx)),
                )
                .child(layer);
        }
        popover_layer(layer).into_any_element()
    }
}

/// How far the click-catcher reaches from the chip in each direction; a window
/// is at most 1440 × 900 and the chip can be anywhere in it.
const SCRIM_REACH: f32 = 4000.0;

