//! One catalog row per skill across providers (harness docs/85 16.2, W8):
//! the skill's name and description, where it lives, and a switch per
//! provider that sees it — each provider's own switch, so one can be off
//! for Codex and on for Claude Code. A provider that loads no skill folder
//! reads "pasted" instead of a switch: the skill reaches it as text.

use std::rc::Rc;

use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{record_ax_label, switch, SwitchIntent};

/// How one provider sees the skill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogReach {
    /// It loads the skill itself; `on` is its switch.
    Loads {
        /// Its switch.
        on: bool,
    },
    /// It loads no skill folder: the `/` menu pastes the skill in.
    Pasted,
}

/// One provider on a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogProvider {
    /// The provider's wire id (what the row's intent reports).
    pub id: SharedString,
    /// Its name, as its own badge says it.
    pub name: SharedString,
    /// How it sees the skill.
    pub reach: CatalogReach,
}

/// One skill's row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogRowModel {
    /// The skill's key (what the row's intent reports).
    pub id: SharedString,
    /// Its name.
    pub name: SharedString,
    /// Its description, or "".
    pub description: SharedString,
    /// Where it lives (`~/.agents/skills`).
    pub place: SharedString,
    /// The providers, in order.
    pub providers: Vec<CatalogProvider>,
}

/// What a row asks for: switch `provider` `on` for the skill `skill`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogIntent {
    /// The row's id.
    pub skill: SharedString,
    /// The provider's id.
    pub provider: SharedString,
    /// On or off.
    pub on: bool,
}

type Handler = Rc<dyn Fn(CatalogIntent, &mut Window, &mut App)>;

/// The row. Build with [`catalog_row`].
#[derive(IntoElement)]
pub struct CatalogRow {
    model: CatalogRowModel,
    on_intent: Option<Handler>,
}

/// A row for `model`.
pub fn catalog_row(model: CatalogRowModel) -> CatalogRow {
    CatalogRow { model, on_intent: None }
}

impl CatalogRow {
    /// What a switch asks for.
    pub fn on_intent(mut self, f: impl Fn(CatalogIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for CatalogRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let m = self.model;
        let mut reach = h_flex().flex_none().gap(px(scale::SP_4)).items_center();
        for provider in &m.providers {
            let cell = h_flex().gap(px(scale::SP_2)).items_center().child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(provider.name.clone()));
            let cell = match provider.reach {
                CatalogReach::Loads { on } => {
                    let label = format!("{} for {}", m.name, provider.name);
                    record_ax_label(&label);
                    let handler = self.on_intent.clone();
                    let (skill, id) = (m.id.clone(), provider.id.clone());
                    cell.child(
                        switch(ElementId::Name(SharedString::from(format!("catalog-{}-{}", m.id, provider.id))), on, false)
                            .accessibility_label(label)
                            .on_intent(move |SwitchIntent::Toggle, w, cx| {
                                if let Some(h) = handler.as_ref() {
                                    h(CatalogIntent { skill: skill.clone(), provider: id.clone(), on: !on }, w, cx);
                                }
                            }),
                    )
                }
                CatalogReach::Pasted => cell.child(div().role_in(TextRole::Caption, cx).text_color(p.ink_4).child("pasted")),
            };
            reach = reach.child(cell);
        }
        h_flex()
            .w_full()
            .gap(px(scale::SP_4))
            .py(px(scale::SP_3))
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .gap(px(scale::SP_1))
                    .child(h_flex().gap(px(scale::SP_2)).items_center().child(div().role_in(TextRole::Body, cx).text_color(p.ink).child(m.name.clone())).child(div().role_in(TextRole::Micro, cx).text_color(p.ink_4).child(m.place.clone())))
                    .when(!m.description.is_empty(), |d| d.child(div().truncate().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(m.description.clone()))),
            )
            .child(reach)
    }
}
