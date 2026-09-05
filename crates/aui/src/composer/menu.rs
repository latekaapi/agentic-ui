//! `.menu`: the `+` popover — 200 wide, overlay ground, morphs out of the
//! button's corner on the gentle spring.

use aui_icons::{icon, IconName};
use aui_motion::{spring_phase, SpringKind};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::kbd;
use crate::util::{interaction_flags, TrackInteraction};

/// `.menu{bottom:38px;left:0;width:200px;padding:6px}`.
const MENU_BOTTOM: f32 = 38.0;
const MENU_W: f32 = 200.0;
const MENU_PAD: f32 = 6.0;
/// `.menu .it{gap:8px;height:30px;padding:0 8px;font-size:12.5px}`.
const ITEM_GAP: f32 = 8.0;
const ITEM_PAD: f32 = 8.0;
const ITEM_TEXT: f32 = 12.5;
/// The morph starts at scale .85 from the bottom-left corner.
const MORPH_FROM: f32 = 0.85;

/// One row of the menu.
#[derive(Debug, Clone, PartialEq)]
pub struct PlusMenuItem {
    /// Stable id, reported on activation.
    pub id: SharedString,
    /// Glyph.
    pub icon: IconName,
    /// Label.
    pub label: SharedString,
    /// Optional keycap at the right.
    pub key: Option<SharedString>,
}

impl PlusMenuItem {
    /// An item.
    pub fn new(id: impl Into<SharedString>, icon: IconName, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), icon, label: label.into(), key: None }
    }

    /// Adds the keycap.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }
}

type ActivateHandler = std::rc::Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// The menu. Build with [`plus_menu`].
#[derive(IntoElement)]
pub struct PlusMenu {
    id: ElementId,
    items: Vec<PlusMenuItem>,
    open: bool,
    at_rest: bool,
    on_activate: Option<ActivateHandler>,
}

/// A menu over `items`; render it as a child of the `+` button's holder.
pub fn plus_menu(id: impl Into<ElementId>, items: Vec<PlusMenuItem>, open: bool) -> PlusMenu {
    PlusMenu { id: id.into(), items, open, at_rest: false, on_activate: None }
}

impl PlusMenu {
    /// Skips the enter morph (static captures).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// Activation handler.
    pub fn on_activate(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for PlusMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let phase = if self.at_rest { 1.0 } else { spring_phase((id.clone(), "morph"), self.open, SpringKind::Gentle, window, cx).clamp(0.0, 1.0) };
        if !self.open && phase <= 0.001 {
            return div().invisible().into_any_element();
        }
        let scale_now = MORPH_FROM + (1.0 - MORPH_FROM) * phase;
        let mut menu = v_flex()
            .id(id.clone())
            .absolute()
            .bottom(px(MENU_BOTTOM))
            .left(px(0.0))
            .w(px(MENU_W * scale_now))
            .p(px(MENU_PAD))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(3))
            .opacity(phase)
            .overflow_hidden();
        for item in self.items {
            let item_id: ElementId = (id.clone(), SharedString::from(format!("item-{}", item.id))).into();
            let (state, flags) = interaction_flags(item_id.clone(), window, cx);
            let mut row = h_flex()
                .id(item_id)
                .w_full()
                .h(cx.aui().metrics.row)
                .gap(px(ITEM_GAP))
                .px(px(ITEM_PAD))
                .rounded(px(scale::R_SM))
                .ui(ITEM_TEXT)
                .text_color(if flags.hovered { p.ink } else { p.ink_2 })
                .when(flags.hovered, |d| d.bg(p.surface_2))
                .cursor_pointer()
                .track_interaction(&state)
                .child(icon(item.icon))
                .child(div().flex_1().min_w(px(0.0)).truncate().child(item.label.clone()));
            if let Some(key) = &item.key {
                row = row.child(kbd(key.clone()));
            }
            if let Some(h) = self.on_activate.clone() {
                let key = item.id.clone();
                row = row.on_click(move |_, w, cx| h(&key, w, cx));
            }
            menu = menu.child(row);
        }
        menu.into_any_element()
    }
}
