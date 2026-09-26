//! The skills list row: name, chips, one-line description, tokens, mode chip
//! and switch.

use std::cell::RefCell;
use std::rc::Rc;

use aui_motion::{tint_fade, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, relative, App, Bounds, ElementId, IntoElement, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::pill;
use crate::data::PillVariant;
use crate::nav::{view_menu, MenuRow};
use crate::overlay::{anchored_menu, MenuAlign, MenuSide};
use crate::util::{interaction_flags, TrackInteraction};

use super::controls::{mode_chip, switch, SwitchIntent};

/// How a skill runs: Muse's `on` versus `user-invocable-only`, named plainly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillMode {
    /// Loaded whenever a task matches the description.
    #[default]
    Auto,
    /// Loaded only when the person types `/name`.
    Only,
}

impl SkillMode {
    /// The mode chip label: `Auto`, or `Only /{name}`.
    pub fn chip_label(&self, name: &str) -> String {
        match self {
            SkillMode::Auto => "Auto".to_string(),
            SkillMode::Only => format!("Only /{name}"),
        }
    }

    /// The mode menu row label: `Automatic`, or `Only /{name}`.
    pub fn menu_label(&self, name: &str) -> String {
        match self {
            SkillMode::Auto => "Automatic".to_string(),
            SkillMode::Only => format!("Only /{name}"),
        }
    }
}

/// The tone of a row chip: quiet or carrying a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChipTone {
    /// The override / plugin chip: line border, ink-2.
    #[default]
    Muted,
    /// The issue-count chip: warning-soft ground, warning text.
    Warning,
}

/// One chip on a row (`Overrides built-in git`, `threejs plugin`, `1 issue`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillChip {
    /// The chip text.
    pub label: SharedString,
    /// Quiet or warning.
    pub tone: ChipTone,
}

impl SkillChip {
    /// A chip with a label and a tone.
    pub fn new(label: impl Into<SharedString>, tone: ChipTone) -> Self {
        Self { label: label.into(), tone }
    }
}

/// Everything a skill row draws. The app owns these; the component is
/// stateless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRowModel {
    /// Stable identity, handed back by the intents.
    pub id: SharedString,
    /// The skill name (weight 500).
    pub name: SharedString,
    /// One ellipsised line in ink-3.
    pub description: SharedString,
    /// Override / plugin / issue chips after the name.
    pub chips: Vec<SkillChip>,
    /// Startup tokens, mono ink-4 right-aligned (`290`, `1.1k`, `—`).
    pub tokens: SharedString,
    /// The mode chip; `None` hides it (off and overridden rows).
    pub mode: Option<SkillMode>,
    /// The switch state.
    pub on: bool,
    /// An overridden (shadowed) row: dimmed, and the switch is dead.
    pub dimmed: bool,
    /// Selected: surface-3 ground and ink text.
    pub selected: bool,
}

impl SkillRowModel {
    /// The intent a switch flip reports, or `None`: dimmed rows emit no
    /// `Toggle` — the row renders the switch disabled and drops the press.
    pub fn toggle_intent(&self) -> Option<SkillRowIntent> {
        if self.dimmed { None } else { Some(SkillRowIntent::Toggle) }
    }

    /// The intent a row click reports. Selection still works on dimmed rows:
    /// the loser is viewable, just not flippable.
    pub fn select_intent(&self) -> Option<SkillRowIntent> {
        Some(SkillRowIntent::Select)
    }

    /// The intent a mode-menu pick reports, or `None` on dimmed rows.
    pub fn mode_intent(&self, mode: SkillMode) -> Option<SkillRowIntent> {
        if self.dimmed { None } else { Some(SkillRowIntent::SetMode(mode)) }
    }
}

/// What a skill row asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillRowIntent {
    /// The row was clicked.
    Select,
    /// The switch was flipped. Never emitted by dimmed rows.
    Toggle,
    /// The mode menu picked `mode`.
    SetMode(SkillMode),
}

type RowHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ModeHandler = Rc<dyn Fn(&SharedString, SkillMode, &mut Window, &mut App)>;

/// `.row`: the ≈56 px skill row — name (500) with its chips, the one-line
/// description in ink-3, mono ink-4 tokens right-aligned, the mode chip, the
/// switch. Off and overridden rows sit at reduced ink. Build with
/// [`skill_row`].
#[derive(IntoElement)]
pub struct SkillRow {
    model: SkillRowModel,
    on_select: Option<RowHandler>,
    on_toggle: Option<RowHandler>,
    on_set_mode: Option<ModeHandler>,
}

/// A row for `model`.
pub fn skill_row(model: SkillRowModel) -> SkillRow {
    SkillRow { model, on_select: None, on_toggle: None, on_set_mode: None }
}

/// `.row{gap:16px;padding:10px 12px}` with the 64 px token column.
const ROW_GAP: f32 = 16.0;
const ROW_PAD_Y: f32 = 10.0;
const ROW_PAD_X: f32 = 12.0;
const ROW_H: f32 = 56.0;
const TOK_W: f32 = 64.0;
/// `.rname{font-size:14.3px}` and `.rdesc{font-size:13.2px}`.
const NAME_TEXT: f32 = 14.3;
const DESC_TEXT: f32 = 13.2;
const TOK_TEXT: f32 = 12.1;
/// Off and overridden rows sit below full ink.
const DIM_OPACITY: f32 = 0.55;

impl SkillRow {
    /// The row was clicked; the argument is [`SkillRowModel::id`].
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// The switch was flipped; the argument is [`SkillRowModel::id`].
    pub fn on_toggle(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    /// The mode menu picked a mode; the arguments are
    /// [`SkillRowModel::id`] and the mode.
    pub fn on_set_mode(mut self, f: impl Fn(&SharedString, SkillMode, &mut Window, &mut App) + 'static) -> Self {
        self.on_set_mode = Some(Rc::new(f));
        self
    }
}

/// The mode menu's open flag and trigger bounds, kept in element state keyed
/// by the row id like hover flags are.
#[derive(Default)]
struct ModeMenuState {
    open: bool,
    trigger: Option<Bounds<Pixels>>,
}

impl RenderOnce for SkillRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let model = self.model;
        let row_id: ElementId = model.id.clone().into();
        let (istate, flags) = interaction_flags(row_id.clone(), window, cx);
        let lit = flags.hovered && !model.selected;
        let ground = tint_fade((row_id.clone(), "bg"), lit, p.surface_2, Tween::FAST, window, cx);

        let menu_state = window
            .use_keyed_state((row_id.clone(), "mode-menu"), cx, |_, _| Rc::new(RefCell::new(ModeMenuState::default())))
            .read(cx)
            .clone();

        // Name + chips line.
        let mut head = h_flex().flex_none().items_center().gap(px(8.0));
        head = head.child(
            div()
                .flex_none()
                .ui(NAME_TEXT)
                .line_height(relative(1.0))
                .medium()
                .text_color(p.ink)
                .child(model.name.clone()),
        );
        for chip_model in &model.chips {
            let variant = match chip_model.tone {
                ChipTone::Muted => PillVariant::Line,
                ChipTone::Warning => PillVariant::Warning,
            };
            head = head.child(pill(chip_model.label.clone()).variant(variant));
        }

        let text = v_flex()
            .flex_1()
            .min_w(px(0.0))
            .justify_center()
            .child(head)
            .child(
                div()
                    .flex_none()
                    .w_full()
                    .truncate()
                    .ui(DESC_TEXT)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(model.description.clone()),
            );

        let mut row = h_flex()
            .id(row_id.clone())
            .w_full()
            .h(px(ROW_H))
            .items_center()
            .gap(px(ROW_GAP))
            .px(px(ROW_PAD_X))
            .py(px(ROW_PAD_Y))
            .rounded(px(scale::R_SM))
            .cursor_pointer()
            .track_interaction(&istate)
            .child(text)
            .child(
                div()
                    .flex_none()
                    .w(px(TOK_W))
                    .flex()
                    .justify_end()
                    .mono(TOK_TEXT)
                    .line_height(relative(1.0))
                    .medium()
                    .text_color(p.ink_4)
                    .child(model.tokens.clone()),
            );

        // The mode chip, or its empty seat so the switch stays aligned.
        if let Some(mode) = model.mode {
            let chip_id: ElementId = (row_id.clone(), SharedString::from("mode")).into();
            let state_for_menu = menu_state.clone();
            let mut holder = div().flex_none().relative().on_children_prepainted(move |bounds, _, _| {
                if let Some(first) = bounds.first() {
                    state_for_menu.borrow_mut().trigger = Some(*first);
                }
            });
            let mut chip_el = mode_chip(chip_id, &mode, model.name.clone());
            {
                let state_for_click = menu_state.clone();
                chip_el = chip_el.on_click(move |_, w, cx| {
                    state_for_click.borrow_mut().open = !state_for_click.borrow().open;
                    w.refresh();
                    let _ = cx;
                });
            }
            holder = holder.child(chip_el);
            row = row.child(holder);
        } else {
            row = row.child(div().flex_none().w(px(0.0)));
        }

        // The switch: dead on dimmed rows — no handler, native disabled.
        let switch_label =
            SharedString::from(format!("Turn skill {} {}", model.name, if model.on { "off" } else { "on" }));
        let switch_id: ElementId = (row_id.clone(), SharedString::from("toggle")).into();
        let mut toggle = switch(switch_id, model.on, model.dimmed).accessibility_label(switch_label);
        if model.toggle_intent().is_some() {
            if let Some(handler) = self.on_toggle.clone() {
                let row_key = model.id.clone();
                toggle = toggle.on_intent(move |_: SwitchIntent, w, cx| handler(&row_key, w, cx));
            }
        }
        row = row.child(toggle);

        if model.selected {
            row = row.bg(p.surface_3).border_1().border_color(p.line);
        } else {
            row = row.bg(ground);
        }
        if !model.on || model.dimmed {
            row = row.opacity(DIM_OPACITY);
        }
        row = row.role(gpui::Role::Button).aria_label(SharedString::from(format!("Show skill {}", model.name)));
        if let Some(handler) = self.on_select {
            let row_key = model.id.clone();
            row = row.on_click(move |_, w, cx| handler(&row_key, w, cx));
        }

        // The mode menu, hung below-end of the chip in the popover layer.
        let mut root = div().relative().child(row);
        let is_open = menu_state.borrow().open;
        if is_open {
            if let Some(mode) = model.mode {
                let state_for_close = menu_state.clone();
                let catcher = div()
                    .id((row_id.clone(), "catcher"))
                    .absolute()
                    .inset_0()
                    .cursor_default()
                    .on_click(move |_, w, _| {
                        state_for_close.borrow_mut().open = false;
                        w.refresh();
                    });
                let rows = [SkillMode::Auto, SkillMode::Only]
                    .into_iter()
                    .map(|m| MenuRow::Toggle {
                        label: SharedString::from(m.menu_label(&model.name)),
                        checked: m == mode,
                    })
                    .collect();
                let mut menu = view_menu((row_id.clone(), "mode-menu"), rows).at_rest();
                if let Some(handler) = self.on_set_mode.clone() {
                    let row_key = model.id.clone();
                    let state_for_pick = menu_state.clone();
                    menu = menu.on_activate(move |ix, w, cx| {
                        let next = if ix == 1 { SkillMode::Only } else { SkillMode::Auto };
                        state_for_pick.borrow_mut().open = false;
                        handler(&row_key, next, w, cx);
                    });
                }
                let trigger = menu_state.borrow().trigger;
                root = root.child(crate::overlay::popover_layer(catcher));
                if let Some(trigger) = trigger {
                    root = root.child(anchored_menu(trigger, MenuSide::Below, MenuAlign::End, menu));
                } else {
                    root = root.child(crate::overlay::popover_layer(menu));
                }
            }
        }
        root
    }
}
