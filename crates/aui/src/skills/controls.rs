//! The skills page controls: the switch, the tri-state checkbox and the mode
//! chip. The segmented control lives where it always has
//! ([`crate::workbench::segmented`], re-exported from [`crate::skills`]).

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, Tween};
use aui_tokens::ActiveAui;
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;
use gpui_kit::component::switch::Switch as KitSwitch;
use gpui_kit::component::{Disableable, Sizable, Size};

use crate::data::chip;
use crate::util::{interaction_flags, TrackInteraction};

use super::row::SkillMode;

/// What the skills switch asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchIntent {
    /// The switch was flipped. The caller owns `on` and flips it.
    Toggle,
}

type SwitchHandler = Rc<dyn Fn(SwitchIntent, &mut Window, &mut App)>;
type CheckboxHandler = Rc<dyn Fn(CheckboxIntent, &mut Window, &mut App)>;

/// `.sw`: the 30 × 18 track with a knob, an accent fill when on and a focus
/// ring. This wraps the same gpui-kit `Switch` the settings dialog's switch
/// rows use (at `Small`, 28 × 16); the disabled fade is the kit's alpha-only
/// track fade, so the thumb never shows the track through. Build with
/// [`switch`].
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
            .color(p.accent)
            .with_size(Size::Small)
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

/// What the skills checkbox asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxIntent {
    /// The box was pressed; the argument is the state it lands on.
    Set(CheckboxState),
}

/// `.cb`: the 16 px skills checkbox. gpui-kit's box is boolean-only, so this
/// is the library's own tri-state box in tokens: transparent + line-strong
/// when [`CheckboxState::Off`], accent + white check when
/// [`CheckboxState::On`], accent-soft + accent-ink dash when
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
}

/// The 16 px box in `state`, non-interactive when `disabled`.
pub fn checkbox(id: impl Into<ElementId>, state: CheckboxState, disabled: bool) -> Checkbox {
    Checkbox { id: id.into(), state, disabled, accessibility_label: None, on_intent: None }
}

/// The checkbox glyph: `.cb svg{width:10px;height:10px}`.
const CHECK_GLYPH: f32 = 10.0;
/// `.cb{width:16px;height:16px;border-radius:4px}`.
const BOX: f32 = 16.0;
const BOX_RADIUS: f32 = 4.0;

impl Checkbox {
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
        let ring = !self.disabled && focus.is_focused(window) && crate::keys::keyboard_nav(cx);
        let (istate, flags) = interaction_flags(id.clone(), window, cx);
        let hovered = flags.hovered && !self.disabled;

        // Hover only ever fades a ground's alpha (never a colour tween): the
        // Off box gains surface-2 under the pointer, filled boxes sit still.
        let (bg, border) = match self.state {
            CheckboxState::Off => (
                tint_fade((id.clone(), "bg"), hovered, p.surface_2, Tween::FAST, window, cx),
                p.line_strong,
            ),
            CheckboxState::On => (p.accent, gpui::transparent_black()),
            CheckboxState::Mixed => (p.accent_soft, gpui::transparent_black()),
        };

        let mut mark: Option<gpui::AnyElement> = None;
        match self.state {
            CheckboxState::Off => {}
            CheckboxState::On => {
                mark = Some(icon(IconName::Check).size(px(CHECK_GLYPH)).color(gpui::white()).into_any_element());
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
            .bg(bg)
            .border_1()
            .border_color(border);
        if ring {
            box_el = box_el.shadow(vec![gpui::BoxShadow {
                color: p.accent_ring,
                offset: gpui::point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(3.0),
                inset: false,
            }]);
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
            .track_interaction(&istate)
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

/// The mode chip: `Auto` or `Only /{name}`, a data [`chip`](crate::data::chip)
/// with a chevron that opens its menu in
/// [`popover_layer`](crate::overlay::popover_layer). The menu itself is the
/// caller's (the skill row hangs a two-row `Automatic` / `Only /{name}` menu
/// off it); this only builds the chip with its human label.
pub fn mode_chip(id: impl Into<ElementId>, mode: &SkillMode, name: impl Into<SharedString>) -> crate::data::Chip {
    let name = name.into();
    let label = SharedString::from(mode.chip_label(&name));
    chip(id, label.clone())
        .chevron()
        .accessibility_label(SharedString::from(format!("Skill mode, {label}. Opens the mode menu.")))
}

