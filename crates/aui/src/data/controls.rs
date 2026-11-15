//! The form controls (docs/83 §7): [`switch`], the tri-state [`checkbox`],
//! [`segmented`] and [`tabs`]. They were spread over feature modules
//! (`skills::controls`, `workbench::diff_review`) and live here now, with
//! the other primitives; hover and press are the one model
//! ([`crate::interact::paint`]).

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;
use gpui_kit::component::switch::Switch as KitSwitch;
use gpui_kit::component::{Disableable, Sizable, Size};

use super::record_ax_label;
use crate::interact::paint::{PaintHover, PaintStates, StatePaint};
use crate::interact::{focus_ring, nested_radius, InteractState, R_ITEM};
use crate::util::indexed_child;

/// What the switch asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchIntent {
    /// The switch was flipped. The caller owns `on` and flips it.
    Toggle,
}

type SwitchHandler = Rc<dyn Fn(SwitchIntent, &mut Window, &mut App)>;
type CheckboxHandler = Rc<dyn Fn(CheckboxIntent, &mut Window, &mut App)>;

/// The toggle (docs/83 §2.5, §7): 36 × 20 with a 16 px thumb, `accent-ring`
/// when on, and a focus ring. It wraps gpui-kit's `Switch` at `Medium`
/// (exactly 36 × 20 / 16); the disabled fade is the kit's alpha-only track
/// fade, so the thumb never shows the track through. Build with [`switch`].
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    on: bool,
    disabled: bool,
    accessibility_label: Option<SharedString>,
    on_intent: Option<SwitchHandler>,
}

/// A switch for `on`, non-interactive when `disabled`.
pub fn switch(id: impl Into<ElementId>, on: bool, disabled: bool) -> Switch {
    Switch { id: id.into(), on, disabled, accessibility_label: None, on_intent: None }
}

impl Switch {
    /// The accessible name (`Turn {name} on`). Without one the switch falls
    /// back to announcing its state alone.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(SwitchIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let state = if self.on { "on" } else { "off" };
        let label = self.accessibility_label.unwrap_or_else(|| SharedString::from(format!("Toggle, {state}")));
        let mut el = KitSwitch::new(self.id)
            .checked(self.on)
            .color(p.accent_ring)
            .with_size(Size::Medium)
            .disabled(self.disabled)
            .accessibility_label(label);
        if !self.disabled {
            if let Some(handler) = self.on_intent {
                el = el.on_click(move |_, w, cx| handler(SwitchIntent::Toggle, w, cx));
            }
        }
        el.into_any_element()
    }
}

/// The three states of an import-preview checkbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckboxState {
    /// Transparent box, line-strong border.
    #[default]
    Off,
    /// Accent fill, white check.
    On,
    /// Accent-soft fill, accent-ink dash: partly selected.
    Mixed,
}

impl CheckboxState {
    /// Whether the box reads as checked (only [`CheckboxState::On`]).
    pub fn checked(&self) -> bool {
        matches!(self, CheckboxState::On)
    }

    /// The state a click lands on: `Off` → `On`, `On` → `Off`, `Mixed` → `On`.
    pub fn toggled(&self) -> Self {
        match self {
            CheckboxState::Off => CheckboxState::On,
            CheckboxState::On => CheckboxState::Off,
            CheckboxState::Mixed => CheckboxState::On,
        }
    }

    /// The state word for accessible names (`off` / `on` / `mixed`).
    pub fn state_word(&self) -> &'static str {
        match self {
            CheckboxState::Off => "off",
            CheckboxState::On => "on",
            CheckboxState::Mixed => "mixed",
        }
    }
}

/// What the checkbox asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxIntent {
    /// The box was pressed; the argument is the state it lands on.
    Set(CheckboxState),
}

/// `.cb`: the 16 px checkbox. gpui-kit's box is boolean-only, so this
/// is the library's own tri-state box in tokens: transparent + line-strong
/// when [`CheckboxState::Off`], accent + a check in the page ground (the
/// primary button's label colour) when [`CheckboxState::On`], accent-soft + accent-ink dash when
/// [`CheckboxState::Mixed`]. Disabled is 45 % opacity like
/// [`crate::data::Button`]. Enter / space on the focused box clicks it, the
/// way a focused button does. Build with [`checkbox`].
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    state: CheckboxState,
    disabled: bool,
    accessibility_label: Option<SharedString>,
    on_intent: Option<CheckboxHandler>,
    preview: Option<InteractState>,
}

/// The 16 px box in `state`, non-interactive when `disabled`.
pub fn checkbox(id: impl Into<ElementId>, state: CheckboxState, disabled: bool) -> Checkbox {
    Checkbox { id: id.into(), state, disabled, accessibility_label: None, on_intent: None, preview: None }
}

/// The checkbox glyph: `.cb svg{width:10px;height:10px}`.
const CHECK_GLYPH: f32 = 10.0;
/// `.cb{width:16px;height:16px;border-radius:4px}`.
const BOX: f32 = 16.0;
const BOX_RADIUS: f32 = 4.0;

impl Checkbox {
    /// Pins a state for a gallery card or a capture.
    pub fn preview(mut self, state: InteractState) -> Self {
        self.preview = Some(state);
        self
    }

    /// The accessible name without the state word (the state is appended:
    /// `Include pdf-tools, on`).
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(CheckboxIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // One focus handle per box id; disabled boxes are not tab stops, the
        // way disabled buttons are not.
        let focus = window
            .use_keyed_state((id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
            .tab_stop(!self.disabled);
        let ring = self.preview.is_some_and(|s| s.focused) || (!self.disabled && focus.is_focused(window) && crate::keys::keyboard_nav(cx));
        // Hover is the one model: the Off box takes the hover overlay under
        // the pointer; filled boxes sit still.
        let (bg, border) = match self.state {
            CheckboxState::Off => (None, p.line_strong),
            CheckboxState::On => (Some(p.accent), gpui::transparent_black()),
            CheckboxState::Mixed => (Some(p.accent_soft), gpui::transparent_black()),
        };
        let paint = StatePaint::new().pressable(false).disabled(self.disabled || bg.is_some()).preview(self.preview);

        let mut mark: Option<gpui::AnyElement> = None;
        match self.state {
            CheckboxState::Off => {}
            CheckboxState::On => {
                // The page ground on the accent fill, as the primary button
                // draws its label: white on the dark theme's near-white
                // accent was a check nobody could see.
                mark = Some(icon(IconName::Check).size(px(CHECK_GLYPH)).color(p.bg).into_any_element());
            }
            CheckboxState::Mixed => {
                mark = Some(
                    div()
                        .w(px(CHECK_GLYPH))
                        .h(px(2.0))
                        .rounded_full()
                        .bg(p.accent_ink)
                        .into_any_element(),
                );
            }
        }

        let mut box_el = div()
            .flex_none()
            .size(px(BOX))
            .rounded(px(BOX_RADIUS))
            .flex()
            .items_center()
            .justify_center()
            .when_some(bg, |d, bg| d.bg(bg))
            .border_1()
            .border_color(border)
            .paint_hover(paint, cx);
        if ring {
            box_el = box_el.relative().child(focus_ring(px(BOX_RADIUS), &p));
        }
        if let Some(mark) = mark {
            box_el = box_el.child(mark);
        }

        let name = self.accessibility_label.unwrap_or_else(|| SharedString::from("Checkbox"));
        let aria = SharedString::from(format!("{name}, {}", self.state.state_word()));
        let mut el = h_flex()
            .id(id)
            .flex_none()
            .track_focus(&focus)
            .child(box_el)
            .role(gpui::Role::CheckBox)
            .aria_label(aria);
        if self.disabled {
            el = el.opacity(0.45);
        } else {
            el = el.cursor_pointer();
            if let Some(handler) = self.on_intent {
                let next = self.state.toggled();
                el = el.on_click(move |_, w, cx| handler(CheckboxIntent::Set(next), w, cx));
            }
        }
        el
    }
}


// ── segmented ──────────────────────────────────────────────────────────────

/// The segmented track's padding and the gap between segments.
const SEG_PAD: f32 = 2.0;
/// A segment's height and side padding.
const SEG_H: f32 = 24.0;
const SEG_PAD_X: f32 = 10.0;

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The segmented control (docs/83 §7): a `surface-4` track on radius 8
/// ([`R_ITEM`]), the active segment on `surface-6` (radius 8 − 2, the
/// concentric rule), the others hovering with the one model. No sliding
/// thumb: the active segment is a fill, not a motion. Build with
/// [`segmented`].
///
/// The track is one tab stop (`radiogroup`); `←` / `→` move the selection
/// while it is focused and report through [`Segmented::on_select`] like a
/// click does. Each segment is a `radio` named by its label.
#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    labels: Vec<SharedString>,
    active: usize,
    accessibility_label: Option<SharedString>,
    on_select: Option<SelectHandler>,
    preview: Option<(usize, InteractState)>,
}

/// A segmented control over `labels` with `active` selected.
pub fn segmented(id: impl Into<ElementId>, labels: Vec<SharedString>, active: usize) -> Segmented {
    Segmented { id: id.into(), labels, active, accessibility_label: None, on_select: None, preview: None }
}

impl Segmented {
    /// Called with the index of the segment that was clicked.
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// The accessible name of the group (the track's `radiogroup` label).
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Pins segment `index` in `state` (gallery cards, captures); a
    /// `focused` state draws the track's focus ring.
    pub fn preview(mut self, index: usize, state: InteractState) -> Self {
        self.preview = Some((index, state));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let count = self.labels.len();

        // One tab stop for the group; the arrows move the selection while it
        // holds the keyboard. Nothing is focused on open: this only arms.
        let focus = window
            .use_keyed_state((id.clone(), "seg-focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
            .tab_stop(true);
        let pinned_focus = self.preview.is_some_and(|(_, s)| s.focused);
        let ring = pinned_focus || (focus.is_focused(window) && crate::keys::keyboard_nav(cx));
        let key_select = self.on_select.clone();
        let key_active = self.active;

        let mut track = h_flex()
            .id((id.clone(), "track"))
            .flex_none()
            .p(px(SEG_PAD))
            .gap(px(SEG_PAD))
            .rounded(px(R_ITEM))
            .bg(p.surface_4)
            .track_focus(&focus)
            .role(gpui::Role::RadioGroup)
            .on_key_down(move |event, window, cx| {
                let held = &event.keystroke.modifiers;
                if held.control || held.alt || held.shift || held.platform || held.function {
                    return;
                }
                let next = match event.keystroke.key.as_str() {
                    "left" => key_active.saturating_sub(1),
                    "right" => (key_active + 1).min(count.saturating_sub(1)),
                    _ => return,
                };
                cx.stop_propagation();
                if next != key_active {
                    if let Some(f) = &key_select {
                        f(next, window, cx);
                    }
                }
            });
        if ring {
            track = track.relative().child(focus_ring(px(R_ITEM), &p));
        }
        if let Some(label) = self.accessibility_label.clone() {
            track = track.aria_label(label);
        }
        for (i, label) in self.labels.into_iter().enumerate() {
            let on = i == self.active;
            let preview = self.preview.and_then(|(at, s)| (at == i).then_some(s));
            let mut paint = StatePaint::new().ink(p.ink_3, p.ink).preview(preview);
            if on {
                paint = paint.on(p.surface_6).ink(p.ink, p.ink).pressable(false);
            }
            let select = self.on_select.clone();
            track = track.child(
                div()
                    .id(indexed_child(&id, "seg", i))
                    .flex_none()
                    .h(px(SEG_H))
                    .px(px(SEG_PAD_X))
                    .rounded(nested_radius(px(R_ITEM), px(SEG_PAD)))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .role_in(TextRole::Caption, cx)
                    .whitespace_nowrap()
                    .paint_states(paint, cx)
                    .role(gpui::Role::RadioButton)
                    .aria_toggled(if on { gpui::Toggled::True } else { gpui::Toggled::False })
                    .aria_label(label.clone())
                    .child(label)
                    .on_click(move |_, w, cx| {
                        if let Some(f) = &select {
                            f(i, w, cx)
                        }
                    }),
            );
        }
        track
    }
}

// ── tabs ───────────────────────────────────────────────────────────────────

/// A tab's height and side padding (docs/83 §6: a tab in a 44 px header).
const TAB_H: f32 = 28.0;
const TAB_PAD_X: f32 = 10.0;
/// The gap between tabs.
const TAB_GAP: f32 = 2.0;

/// A row of tabs (docs/83 §6): Body text, muted at rest; the active tab is
/// the `ov-selected` pill on radius 8 ([`R_ITEM`]) in the foreground; the
/// others hover with the one model. A `tablist` of `tab`s, each named by
/// its label. Build with [`tabs`].
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    labels: Vec<SharedString>,
    active: usize,
    accessibility_label: SharedString,
    on_select: Option<SelectHandler>,
    preview: Option<(usize, InteractState)>,
}

/// Tabs over `labels` with `active` selected, the list named `label`.
pub fn tabs(id: impl Into<ElementId>, label: impl Into<SharedString>, labels: Vec<SharedString>, active: usize) -> Tabs {
    Tabs { id: id.into(), labels, active, accessibility_label: label.into(), on_select: None, preview: None }
}

impl Tabs {
    /// Called with the index of the tab that was clicked.
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// Pins tab `index` in `state` (gallery cards, captures).
    pub fn preview(mut self, index: usize, state: InteractState) -> Self {
        self.preview = Some((index, state));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut row = h_flex()
            .id(self.id.clone())
            .flex_none()
            .gap(px(TAB_GAP))
            .items_center()
            .role(gpui::Role::TabList)
            .aria_label(self.accessibility_label.clone());
        record_ax_label(&self.accessibility_label);
        for (i, label) in self.labels.into_iter().enumerate() {
            let on = i == self.active;
            let preview = self.preview.and_then(|(at, s)| (at == i).then_some(s));
            let paint = StatePaint::new().selected(on).ink(p.fg_muted, p.fg).preview(preview);
            let select = self.on_select.clone();
            record_ax_label(&label);
            row = row.child(
                div()
                    .id(indexed_child(&self.id, "tab", i))
                    .flex_none()
                    .h(px(TAB_H))
                    .px(px(TAB_PAD_X))
                    .rounded(px(R_ITEM))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .role_in(TextRole::Body, cx)
                    .whitespace_nowrap()
                    .paint_states(paint, cx)
                    .role(gpui::Role::Tab)
                    .aria_selected(on)
                    .aria_label(label.clone())
                    .child(label)
                    .on_click(move |_, w, cx| {
                        if let Some(f) = &select {
                            f(i, w, cx)
                        }
                    }),
            );
        }
        row
    }
}
