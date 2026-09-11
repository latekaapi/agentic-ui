//! The three composer chip menus: **model**, **effort** and **approval mode**.
//!
//! One component with three constructors, because they are one control with
//! three data sets: a list of rows with a label, a one-line detail, an optional
//! trailing meta (a model's context limit) and any number of badges
//! (`default`, `active`), plus a check on the selected row.
//!
//! They are built on the `+` menu's primitives — the same quick rise-and-fade
//! enter, the same overlay ground, painted through
//! [`crate::overlay::popover_layer`] — and are anchored by the composer to the
//! chip that opened them ([`crate::composer::Composer::chip_menu`]).
//!
//! Stateless: rows in, `selected` in, `open` in; pick, hover and close out. In
//! particular **the selection is not a fact about the rows**. MSP says a client
//! must not assume exactly one `isActive` or `isDefault` model, so "active" is a
//! badge that any number of rows may wear, and `selected` is only where the
//! keyboard is.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, tint_fade, tween, EnterExit, PresenceStyle, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::tag;
use crate::overlay::popover_layer;
use crate::util::{indexed_child, interaction_flags};

/// `.pick{bottom:28px;left:0;padding:6px}` — measured from the chip it hangs
/// off, which is 28 px tall like every composer toolbar control.
const MENU_BOTTOM: f32 = 28.0;
const MENU_PAD: f32 = 6.0;
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
/// The overlay's own shadow step.
const MENU_SHADOW: u8 = 3;
/// The enter rises 6 px at scale .98 — the one presence tween every composer
/// menu shares, like `plus_menu`.
const POP_RISE: f32 = 6.0;
const POP_FROM_SCALE: f32 = 0.98;
/// `.pick .it{padding:6px 8px;gap:8px;border-radius:var(--r-sm)}` — two lines,
/// so the row is taller than a menu row and sizes to its content.
const ITEM_PAD_X: f32 = 8.0;
const ITEM_PAD_Y: f32 = 6.0;
const ITEM_GAP: f32 = 8.0;
/// `.pick .it b{font-size:12.5px}` and `.pick .it span{font-size:11px}`.
const LABEL_TEXT: f32 = 12.5;
const DETAIL_TEXT: f32 = scale::FS_11;
/// The check on the selected row.
const CHECK: f32 = 12.0;
/// The check column is reserved on every row so the labels line up.
const CHECK_COL: f32 = 14.0;
/// `.pick .caps{padding:6px 8px 4px}`.
const CAPS_PAD_TOP: f32 = 6.0;
const CAPS_PAD_X: f32 = 8.0;
const CAPS_PAD_BOTTOM: f32 = 4.0;

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
        // At rest the menu is either fully out or fully gone; `open` still
        // decides which, or a closed menu would draw itself in a static capture.
        let style = if self.at_rest {
            PresenceStyle { opacity: if self.open { 1.0 } else { 0.0 }, offset_y: px(0.0), scale: 1.0 }
        } else {
            let sample = presence((id.clone(), "enter"), self.open, EnterExit::QUICK, window, cx);
            PresenceStyle::fade_rise_scale(sample, POP_RISE, POP_FROM_SCALE)
        };
        if !self.open && style.opacity <= 0.001 {
            return div().invisible().into_any_element();
        }
        let scale_now = style.scale;
        let rise = style.offset_y;

        // The pointer wins over the caller's `selected`, so the arrow keys and
        // the mouse never light two rows at once.
        let hovered = (0..self.rows.len()).find(|index| {
            let key: ElementId = indexed_child(&id, "row-", *index);
            interaction_flags(key, window, cx).1.hovered
        });
        let active = hovered.unwrap_or(self.selected);

        let mut menu = v_flex()
            .id(id.clone())
            .absolute()
            .bottom(px(MENU_BOTTOM) - rise)
            .left(px(0.0))
            .min_w(px(MENU_W_MIN * scale_now))
            .max_w(px(MENU_W_MAX * scale_now))
            .p(px(MENU_PAD))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(MENU_SHADOW))
            .opacity(style.opacity)
            .occlude()
            .child(
                div()
                    .flex_none()
                    .pt(px(CAPS_PAD_TOP))
                    .px(px(CAPS_PAD_X))
                    .pb(px(CAPS_PAD_BOTTOM))
                    .text_role(aui_tokens::TextRole::Caps)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(self.title.to_uppercase()),
            );
        for (index, row) in self.rows.into_iter().enumerate() {
            menu = menu.child(picker_row(
                indexed_child(&id, "row-", index),
                &p,
                row,
                index,
                index == active,
                index == self.selected,
                &self.on_pick,
                &self.on_hover,
                window,
                cx,
            ));
        }

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

#[allow(clippy::too_many_arguments)]
fn picker_row(
    id: ElementId,
    p: &Palette,
    row: PickerRow,
    index: usize,
    on: bool,
    checked: bool,
    on_pick: &Option<PickHandler>,
    on_hover: &Option<HoverHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let (state, _) = interaction_flags(id.clone(), window, cx);
    let ground = tint_fade((id.clone(), "bg"), on, p.accent_soft, Tween::FAST, window, cx);
    let label_ink = tween((id.clone(), "label"), if on { p.ink } else { p.ink_2 }, Tween::FAST, window, cx);

    // No `w_full` anywhere on a row: the menu's own width is `auto` between a
    // floor and a ceiling, and a child that asks for "100 % of the parent"
    // makes that circular — the parent collapses to its floor and the label
    // wraps inside it. Stretch does the job instead: the container sizes to the
    // widest row, and every row is stretched to match.
    let mut head = h_flex()
        .gap(px(ITEM_GAP))
        // No `truncate`: the menu grows to the label, and where it cannot the
        // label wraps. A model you cannot read the name of is one you cannot
        // choose.
        .child(div().flex_1().min_w(px(0.0)).ui(LABEL_TEXT).medium().text_color(label_ink).child(row.label.clone()));
    if let Some(meta) = &row.meta {
        head = head.child(div().flex_none().mono(DETAIL_TEXT).text_color(p.ink_3).child(meta.clone()));
    }
    for badge in &row.badges {
        head = head.child(tag(badge.clone()));
    }

    let mut body = v_flex().flex_1().min_w(px(0.0)).child(head);
    if !row.detail.is_empty() {
        body = body.child(div().ui(DETAIL_TEXT).text_color(p.ink_3).child(row.detail.clone()));
    }

    let check = div()
        .flex_none()
        .w(px(CHECK_COL))
        .flex()
        .justify_center()
        .children(checked.then(|| icon(IconName::Check).size(px(CHECK)).color(p.accent_ink)));

    let mut element = h_flex()
        .id(id)
        .items_start()
        .gap(px(ITEM_GAP))
        .px(px(ITEM_PAD_X))
        .py(px(ITEM_PAD_Y))
        .rounded(px(scale::R_SM))
        .bg(ground)
        .cursor_pointer()
        .child(check)
        .child(body);

    // gpui allows one `on_hover` per element, so the row's own hover tint and
    // the caller's hover intent share a single handler.
    let hovered_state = state.clone();
    let handler = on_hover.clone();
    element = element.on_hover(move |now, w, cx| {
        hovered_state.update(cx, |s, cx| {
            if s.hovered != *now {
                s.hovered = *now;
                cx.notify();
            }
        });
        if *now {
            if let Some(handler) = &handler {
                handler(index, w, cx);
            }
        }
    });
    if let Some(handler) = on_pick.clone() {
        let row_id = row.id.clone();
        element = element.on_click(move |_, w, cx| handler(&row_id, w, cx));
    }
    element
}
