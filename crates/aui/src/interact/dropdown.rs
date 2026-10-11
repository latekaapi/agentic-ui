//! [`Dropdown`]: the one menu panel. Fluid Functionalism's dropdown
//! (`registry/base/dropdown.tsx`, `registry/default/menu-item.tsx`) on our
//! radii, and the only way the library draws a menu — the sidebar's menus,
//! the Browse menus, the header and group menus, the composer's `+` menu and
//! pickers all build their rows here, so one change reaches every menu.
//!
//! - **Panel:** [`menu_surface`] (substrate + 2, shadow-3, container radius)
//!   with a 4 px inset; `Role::Menu` and its accessible name.
//! - **Rows:** one `control` tall (36), `item_px` padding, the item radius,
//!   an optional 16 px icon on its own axis, labels muted at rest and lit to
//!   the foreground with the icon's stroke; a second line (Caption, muted)
//!   for two-line rows; checked rows semibold with FF's check; the current
//!   row of a single choice on the `active` block; danger rows red; disabled
//!   rows at 50 % and inert.
//! - **Hover:** FF's FluidHoverHighlight — one block that glides between rows
//!   on the fast spring ([`FluidList`]); the current row's block springs on
//!   the moderate tier. The keyboard drives the same block
//!   ([`Dropdown::highlighted`]).
//! - **Separators:** full bleed through the inset at border/60, 4 px above
//!   and below. **Group labels:** Caption, muted, 28 tall.
//! - **Motion:** enter = opacity 0 → 1 and a 4 px slide toward the trigger
//!   over spring.fast's 80 ms; exit = back over spring.fast.exit's 60 ms
//!   ([`EnterExit::POPUP`]). Adaptation: gpui has no transform, so FF's
//!   `scaleY 0.96` is dropped (the 4 px slide carries the direction alone).
//!
//! The host keeps rendering a closing menu with [`Dropdown::open`]`(false)`
//! until [`dropdown_present`] says the exit has finished.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled, Density, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, Pixels, RenderOnce, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex, PresencePhase};

use super::{fluid_list, menu_surface, Axis, Shape};
use crate::data::record_ax_label;

type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type HighlightHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// What sits at a row's right edge.
#[derive(Default)]
pub enum DropdownTrailing {
    /// Nothing.
    #[default]
    None,
    /// A value (Caption, muted): a count, "Soon", the current choice.
    Value(SharedString),
    /// A shortcut (Micro, muted): "⌘T".
    Keys(SharedString),
    /// The row opens a submenu: an optional value, then a chevron.
    Submenu(SharedString),
    /// The host's own element (a badge, a meter).
    Element(AnyElement),
}

/// One menu row. Build with [`DropdownItem::new`].
pub struct DropdownItem {
    label: SharedString,
    detail: Option<SharedString>,
    icon: Option<IconName>,
    leading: Option<AnyElement>,
    trailing: DropdownTrailing,
    checked: bool,
    current: bool,
    danger: bool,
    disabled: bool,
    wrap: bool,
    on_select: Option<Handler>,
}

impl DropdownItem {
    /// A row labelled `label`.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            icon: None,
            leading: None,
            trailing: DropdownTrailing::None,
            checked: false,
            current: false,
            danger: false,
            disabled: false,
            wrap: false,
            on_select: None,
        }
    }

    /// The label wraps instead of truncating: a name that must be read
    /// whole (a model name the provider chose), in a panel with a ceiling.
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    /// A second line under the label (Caption, muted): a two-line row.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        let detail = detail.into();
        self.detail = (!detail.is_empty()).then_some(detail);
        self
    }

    /// A 16 px icon before the label, muted at rest, lit with the row.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The host's own leading element (a colour swatch, a mark), in the
    /// icon's slot.
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// What sits at the right edge.
    pub fn trailing(mut self, trailing: DropdownTrailing) -> Self {
        self.trailing = trailing;
        self
    }

    /// A shortcut at the right edge (Micro, muted).
    pub fn keys(self, keys: impl Into<SharedString>) -> Self {
        self.trailing(DropdownTrailing::Keys(keys.into()))
    }

    /// A value at the right edge (Caption, muted).
    pub fn value(self, value: impl Into<SharedString>) -> Self {
        self.trailing(DropdownTrailing::Value(value.into()))
    }

    /// A toggle that is on: foreground, semibold, FF's check mark.
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        self
    }

    /// The current row of a single choice: checked, and the `active`
    /// block sits under it.
    pub fn current(mut self, on: bool) -> Self {
        self.current = on;
        self.checked = on;
        self
    }

    /// A destructive row (red label).
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// Greys the row to 50 % and makes it inert.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What a click (or Enter on the highlighted row) does.
    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// The accessibility role this row answers to.
    fn role(&self) -> gpui::Role {
        if self.current {
            gpui::Role::MenuItemRadio
        } else if self.checked {
            gpui::Role::MenuItemCheckBox
        } else {
            gpui::Role::MenuItem
        }
    }
}

/// One entry of a [`Dropdown`], in order.
pub enum DropdownEntry {
    /// A selectable row.
    Item(DropdownItem),
    /// FF's separator.
    Separator,
    /// FF's group label (Caption, muted, 28 tall).
    Label(SharedString),
    /// A host-drawn block inside the panel that is not a row (the account
    /// menu's identity line and usage cards): measured, never lit.
    Static(AnyElement),
}

impl From<DropdownItem> for DropdownEntry {
    fn from(item: DropdownItem) -> Self {
        DropdownEntry::Item(item)
    }
}

/// Which way the panel opens from its trigger: its 4 px enter slide comes
/// from the trigger's side (FF `--popup-enter-y`, following the resolved
/// side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropdownSide {
    /// Below the trigger (or at the pointer): slides down into place.
    #[default]
    Below,
    /// Above the trigger: slides up into place.
    Above,
    /// Beside its parent row (a submenu): no slide, fade only.
    Beside,
}

/// The menu panel. Build with [`dropdown`].
#[derive(IntoElement)]
pub struct Dropdown {
    id: ElementId,
    label: SharedString,
    width: Option<Pixels>,
    min_width: Option<Pixels>,
    entries: Vec<DropdownEntry>,
    open: bool,
    at_rest: bool,
    side: DropdownSide,
    highlighted: Option<usize>,
    on_highlight: Option<HighlightHandler>,
    on_backdrop: Option<Handler>,
    max_height: Option<Pixels>,
    focus: Option<gpui::FocusHandle>,
}

/// A menu panel named `label` (its accessible name).
pub fn dropdown(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Dropdown {
    Dropdown {
        id: id.into(),
        label: label.into(),
        width: None,
        min_width: None,
        entries: Vec::new(),
        open: true,
        at_rest: false,
        side: DropdownSide::Below,
        highlighted: None,
        on_highlight: None,
        on_backdrop: None,
        max_height: None,
        focus: None,
    }
}

/// Whether a dropdown keyed `id` still needs drawing: open, or closed with
/// its 60 ms exit not yet finished. A host that closes a menu keeps
/// rendering it (with [`Dropdown::open`]`(false)`) while this is true.
pub fn dropdown_present(id: impl Into<ElementId>, open: bool, window: &mut Window, cx: &mut App) -> bool {
    let id: ElementId = id.into();
    open || exiting(presence((id, "presence"), open, EnterExit::POPUP, window, cx))
}

/// A closed menu still fading out. gpui-kit samples a presence that has
/// never been open as "exiting" from 0 for its first frames: progress, not
/// the phase, says whether anything is left to draw (a never-opened menu
/// drew itself into a deferred layer and broke cached views' frames).
fn exiting(sample: aui_motion::PresenceSample) -> bool {
    !matches!(sample.phase, PresencePhase::Absent) && sample.progress > 0.0
}

impl Dropdown {
    /// A fixed width (FF: 288; sidebar menus 240, submenus 224).
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// A minimum width (FF: the trigger's width); the panel otherwise sizes
    /// to its rows.
    pub fn min_width(mut self, width: impl Into<Pixels>) -> Self {
        self.min_width = Some(width.into());
        self
    }

    /// Caps the panel's height (FF: min(480, available)); rows scroll.
    pub fn max_height(mut self, height: impl Into<Pixels>) -> Self {
        self.max_height = Some(height.into());
        self
    }

    /// Adds one entry.
    pub fn entry(mut self, entry: impl Into<DropdownEntry>) -> Self {
        self.entries.push(entry.into());
        self
    }

    /// Adds entries.
    pub fn entries(mut self, entries: impl IntoIterator<Item = DropdownEntry>) -> Self {
        self.entries.extend(entries);
        self
    }

    /// Whether the menu is open; `false` plays the 60 ms exit (keep
    /// rendering while [`dropdown_present`] is true).
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Skips the enter: drawn at rest on its first frame (a static
    /// composition — the gallery, a capture — not a menu just opened).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// Which side of its trigger it opens on (the enter slide's direction).
    pub fn side(mut self, side: DropdownSide) -> Self {
        self.side = side;
        self
    }

    /// The keyboard highlight: the row (entry index) the arrows are on.
    /// The pointer takes over when it moves, as in every fluid list.
    pub fn highlighted(mut self, index: Option<usize>) -> Self {
        self.highlighted = index;
        self
    }

    /// Fires when the pointer moves the highlight (entry index): submenu
    /// triggers open on it, pickers follow it with their selection.
    pub fn on_highlight(mut self, f: impl Fn(Option<usize>, &mut Window, &mut App) + 'static) -> Self {
        self.on_highlight = Some(Rc::new(f));
        self
    }

    /// The panel takes this focus when pressed, so a host's key context
    /// around it (Escape) reaches it. It must sit on the panel itself: the
    /// panel occludes, so an ancestor's hitbox never sees the press.
    pub fn track_focus(mut self, focus: &gpui::FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    /// A click on the panel itself (padding, a separator, a label): the
    /// click never reaches what is under the menu either way.
    pub fn on_backdrop(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_backdrop = Some(Rc::new(f));
        self
    }
}

/// FF's separator: one hairline at border/60, full bleed through the
/// panel's 4 px inset, 4 px above and below.
pub fn dropdown_separator(cx: &App) -> AnyElement {
    let metrics = cx.aui().metrics;
    let mut line = cx.aui().colors.line;
    line.a *= 0.6;
    div().h(px(1.0)).mx(-(metrics.item_px / 2.0)).my(metrics.item_px / 2.0).bg(line).into_any_element()
}

/// FF's group label: Caption, muted, 28 tall.
pub fn dropdown_label(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    let metrics = cx.aui().metrics;
    div()
        .h(metrics.control_md)
        .px(metrics.item_px)
        .flex()
        .items_center()
        .role_in(TextRole::Caption, cx)
        .text_color(cx.aui().colors.fg_muted)
        .child(text.into())
        .into_any_element()
}

/// The height of a [`Dropdown`] with `rows` single-line rows, `separators`
/// separators and `labels` group labels: what a host sizing a window (an
/// overlay over a native page) needs before the first layout.
pub fn dropdown_height(rows: usize, separators: usize, labels: usize, cx: &App) -> Pixels {
    let metrics = cx.aui().metrics;
    let inset = metrics.item_px; // 4 above + 4 below
    let separator = px(1.0) + metrics.item_px; // 1 px + 4 above + 4 below
    metrics.control * rows as f32 + separator * separators as f32 + metrics.control_md * labels as f32 + inset
}

/// One row's body: icon slot, label (+ detail), trailing.
fn row_body(item: DropdownItem, lit: bool, cx: &App) -> (gpui::Div, Option<Handler>, gpui::Role, SharedString, bool) {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let on = lit && !item.disabled;
    let fg = if item.danger {
        p.danger
    } else if on || item.checked {
        p.fg
    } else {
        p.fg_muted
    };
    let role = item.role();
    let label = item.label.clone();
    let disabled = item.disabled;
    let mut body = h_flex().w_full().items_center().px(metrics.item_px).gap(metrics.gap);
    body = match item.detail {
        Some(_) => body.min_h(metrics.control).py(metrics.gap / 2.0),
        None => body.h(metrics.control),
    };
    if let Some(leading) = item.leading {
        body = body.child(div().flex_none().size(metrics.icon).flex().items_center().justify_center().child(leading));
    } else if let Some(glyph) = item.icon {
        let tint = if item.danger { p.danger } else if on || item.checked { p.fg } else { p.fg_muted };
        body = body.child(icon(glyph).size(metrics.icon).color(tint).lit(on || item.checked));
    }
    let title = div()
        .min_w(px(0.0))
        .when(!item.wrap, |d| d.truncate())
        .text_role_in(TextRole::Body, Density::Standard)
        .text_color(fg)
        .when(item.checked, |d| d.semibold())
        .child(item.label);
    let text = match item.detail {
        Some(detail) => v_flex()
            .flex_1()
            .min_w(px(0.0))
            .child(title)
            .child(div().min_w(px(0.0)).truncate().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(detail)),
        None => v_flex().flex_1().min_w(px(0.0)).child(title),
    };
    body = body.child(text);
    body = match item.trailing {
        DropdownTrailing::None => body,
        DropdownTrailing::Value(text) => body.child(div().flex_none().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(text)),
        DropdownTrailing::Keys(keys) => body.child(div().flex_none().role_in(TextRole::Micro, cx).text_color(p.fg_muted).child(keys)),
        // The library has no right chevron glyph: the menus' "›" (Caption).
        DropdownTrailing::Submenu(value) => {
            let text: SharedString = if value.is_empty() { "›".into() } else { format!("{value} ›").into() };
            body.child(div().flex_none().role_in(TextRole::Caption, cx).text_color(if on { p.fg } else { p.fg_muted }).child(text))
        }
        DropdownTrailing::Element(element) => body.child(div().flex_none().child(element)),
    };
    if item.checked {
        body = body.child(icon(IconName::Check).size(metrics.icon).color(p.fg).lit(true));
    }
    let body = if disabled { body.opacity(0.5) } else { body };
    (body, item.on_select, role, label, disabled)
}

impl RenderOnce for Dropdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let metrics = cx.aui().metrics;
        record_ax_label(&self.label);
        let timing = if self.at_rest { EnterExit { enter: std::time::Duration::ZERO, ..EnterExit::POPUP } } else { EnterExit::POPUP };
        let sample = presence((self.id.clone(), "presence"), self.open, timing, window, cx);
        let slide = match self.side {
            DropdownSide::Below => -(scale::SP_2),
            DropdownSide::Above => scale::SP_2,
            DropdownSide::Beside => 0.0,
        };
        let style = PresenceStyle::fade_rise(sample, slide);
        // The lit row: what the fluid block sits on (pointer or keys).
        let lit = window.use_keyed_state((self.id.clone(), "lit"), cx, |_, _| None::<usize>);
        let lit_now = self.highlighted.or(*lit.read(cx));
        let current = self.entries.iter().position(|e| matches!(e, DropdownEntry::Item(item) if item.current));
        let mut selects: Vec<(usize, Handler)> = Vec::new();
        let mut list = fluid_list((self.id.clone(), "rows"), Axis::Y)
            .gap(px(0.0))
            .label(self.label.clone())
            .selected(current)
            .highlighted(self.highlighted);
        for (index, entry) in self.entries.into_iter().enumerate() {
            match entry {
                DropdownEntry::Item(item) => {
                    let (body, on_select, role, label, disabled) = row_body(item, lit_now == Some(index), cx);
                    let row = Shape::Item
                        .rounded(body)
                        .id(crate::util::indexed_child(&self.id, "row", index))
                        .role(role)
                        .aria_label(label.clone());
                    record_ax_label(&label);
                    if disabled {
                        list = list.item_static(index, row);
                    } else {
                        if let Some(on_select) = on_select {
                            selects.push((index, on_select));
                        }
                        list = list.item(index, row);
                    }
                }
                DropdownEntry::Separator => list = list.item_static(index, dropdown_separator(cx)),
                DropdownEntry::Label(text) => list = list.item_static(index, dropdown_label(text, cx)),
                DropdownEntry::Static(element) => list = list.item_static(index, element),
            }
        }
        let selects: Rc<Vec<(usize, Handler)>> = Rc::new(selects);
        let on_activate: IndexHandler = Rc::new(move |index, window, cx| {
            if let Some((_, f)) = selects.iter().find(|(i, _)| *i == index) {
                f(window, cx);
            }
        });
        let report = self.on_highlight.clone();
        let lit_state = lit.clone();
        list = list
            .on_activate(move |index, window, cx| on_activate(index, window, cx))
            .on_highlight(move |index, window, cx| {
                if *lit_state.read(cx) != index {
                    lit_state.update(cx, |lit, cx| {
                        *lit = index;
                        cx.notify();
                    });
                }
                if let Some(report) = report.as_ref() {
                    report(index, window, cx);
                }
            });
        let backdrop = self.on_backdrop.clone();
        let mut panel = v_flex()
            .id((self.id.clone(), "panel"))
            .w_full()
            .p(metrics.item_px / 2.0)
            .role(gpui::Role::Menu)
            .aria_label(self.label.clone())
            .occlude()
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                if let Some(f) = backdrop.as_ref() {
                    f(window, cx);
                }
            })
            .child(list);
        if let Some(focus) = self.focus.as_ref() {
            panel = panel.track_focus(focus);
        }
        if let Some(max) = self.max_height {
            panel = panel.max_h(max).overflow_y_scroll();
        }
        let mut root = div().relative().top(style.offset_y).opacity(style.opacity);
        root = match self.width {
            Some(width) => root.w(width),
            None => root,
        };
        if let Some(min) = self.min_width {
            root = root.min_w(min);
        }
        // Closed and exited: nothing (the host normally stops rendering).
        if !self.open && !exiting(sample) {
            return div().into_any_element();
        }
        root.child(menu_surface(panel)).into_any_element()
    }
}
