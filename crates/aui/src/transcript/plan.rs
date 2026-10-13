//! The plan card (D4, docs/41 §2.7): "Proposed plan" with the Plan mode
//! tag, the steps as an ordered list at body size, and a footer that
//! rejects, edits or accepts it.

use aui_protocol::{PlanSection, PlanState};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::transcript::controls::{control, key_hints, ControlKind};
use crate::transcript::{prose, ProseStyle};
use crate::util::ClickHandler;

/// `.plan .hd{padding:10px 12px;gap:8px;font-weight:600}` with a 14 px glyph.
const HEAD_PAD_Y: f32 = 10.0;
const HEAD_PAD_X: f32 = 12.0;
const HEAD_GAP: f32 = 8.0;
const HEAD_TEXT: f32 = 13.0;
/// `.plan ol{padding:0 12px 10px 30px;font-size:12.5px;line-height:1.6}` —
/// the 30 px left padding is the 12 px inset plus the marker column.
const LIST_PAD_X: f32 = 12.0;
const LIST_PAD_BOTTOM: f32 = 10.0;
const MARKER_COL: f32 = 18.0;
/// The gap a browser leaves between an `::marker` and the item's content.
const MARKER_GAP: f32 = 7.0;
const ITEM_TEXT: f32 = 13.0;
const ITEM_LH: f32 = 1.6;
/// A section label is a caps row that owns the whole list width, with a little
/// air above it — and none above the first, which the header already gives.
const SECTION_TEXT: f32 = 11.0;
const SECTION_GAP_TOP: f32 = 10.0;
const SECTION_GAP_BOTTOM: f32 = 2.0;
/// `.actions{gap:8px;padding:10px 12px}` and `.actions .hint{gap:5px;font-size:11px}`.
const ACTIONS_PAD_Y: f32 = 8.0;
const ACTIONS_PAD_X: f32 = 12.0;
const ACTIONS_GAP: f32 = 8.0;

/// The plan card. Build with [`plan_card`].
#[derive(IntoElement)]
pub struct PlanCard {
    id: ElementId,
    items: Vec<SharedString>,
    sections: Vec<PlanSection>,
    state: PlanState,
    on_accept: Option<ClickHandler>,
    on_edit: Option<ClickHandler>,
    on_reject: Option<ClickHandler>,
}

/// A proposed plan over `items`; each item is the small markdown subset, so
/// `` `code` `` spans render in the mono face.
///
/// [`SharedString`] slice, not `Vec<String>`: the card used to allocate a fresh
/// string per step per construction, where a caller that stores the steps as
/// `SharedString` hands them over for a refcount bump (finding
/// `library-hotpaths-8`).
pub fn plan_card(id: impl Into<ElementId>, items: &[SharedString]) -> PlanCard {
    PlanCard {
        id: id.into(),
        items: items.to_vec(),
        sections: Vec::new(),
        state: PlanState::Proposed,
        on_accept: None,
        on_edit: None,
        on_reject: None,
    }
}

impl PlanCard {
    /// The unnumbered labels that group the steps.
    ///
    /// A plan written as markdown has headings over lists, and the two levels
    /// do not share a numbering: the label is drawn before its
    /// [`PlanSection::first_item`] and the numbers keep counting steps only, so
    /// "3." is still the third thing to do however many headings precede it.
    pub fn sections(mut self, sections: Vec<PlanSection>) -> Self {
        self.sections = sections;
        self
    }

    /// Where the plan is in its lifecycle; the action row is only drawn while
    /// the plan is [`PlanState::Proposed`].
    pub fn state(mut self, state: PlanState) -> Self {
        self.state = state;
        self
    }

    /// "Accept and run".
    pub fn on_accept(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_accept = Some(Box::new(f));
        self
    }

    /// "Edit": the person wants to change the plan text first.
    pub fn on_edit(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_edit = Some(Box::new(f));
        self
    }

    /// "Reject": the agent should propose something else.
    pub fn on_reject(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_reject = Some(Box::new(f));
        self
    }
}

impl RenderOnce for PlanCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let style = ProseStyle {
            ink: p.ink,
            code_ink: p.ink,
            code_bg: p.line,
            size: ITEM_TEXT,
            line_height: ITEM_LH,
            paragraph_gap: 0.0,
        };

        let mut list = v_flex().w_full().pl(px(LIST_PAD_X)).pr(px(LIST_PAD_X)).pb(px(LIST_PAD_BOTTOM));
        for (index, item) in self.items.iter().enumerate() {
            // Every label that starts here, in the order it was given: two
            // headings in a row with nothing between them both belong above
            // this step.
            for section in self.sections.iter().filter(|s| s.first_item == index) {
                list = list.child(
                    div()
                        .w_full()
                        .when(index > 0, |d| d.mt(px(SECTION_GAP_TOP)))
                        .mb(px(SECTION_GAP_BOTTOM))
                        .text_role(aui_tokens::TextRole::Caps)
                        .line_height(aui_tokens::scaled(SECTION_TEXT * scale::LH_UI))
                        .text_color(p.ink_3)
                        .child(SharedString::from(section.label.clone())),
                );
            }
            list = list.child(
                h_flex().w_full().items_start().child(
                    div()
                        .flex_none()
                        .w(px(MARKER_COL))
                        .pr(px(MARKER_GAP))
                        .flex()
                        .justify_end()
                        .ui(ITEM_TEXT)
                        // The marker sits on the item's first line box.
                        .line_height(aui_tokens::scaled(ITEM_TEXT * ITEM_LH))
                        .text_color(p.ink)
                        .child(format!("{}.", index + 1)),
                ).child(div().flex_1().min_w(px(0.0)).child(prose((id.clone(), SharedString::from(format!("item-{index}"))), item, style))),
            );
        }

        let mut card = crate::util::raised_block(v_flex().id(id.clone()).w_full().rounded(px(crate::transcript::card::CARD_RADIUS)), &p, p.surface_1)
            .overflow_hidden()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(HEAD_GAP))
                    .py(px(HEAD_PAD_Y))
                    .px(px(HEAD_PAD_X))
                    .child(div().flex_1().min_w(px(0.0)).truncate().ui(HEAD_TEXT).semibold().text_color(p.ink).child("Proposed plan"))
                    .child(div().flex_none().px(px(8.0)).py(px(2.0)).rounded(px(6.0)).bg(p.line).ui(scale::FS_12).text_color(p.ink_2).child("Plan mode")),
            )
            .child(list);

        if self.state == PlanState::Proposed {
            let mut reject = control((id.clone(), "reject"), "Reject", ControlKind::Text, &p);
            if let Some(on_reject) = self.on_reject {
                reject = reject.on_click(move |e, w, cx| on_reject(e, w, cx));
            }
            let mut edit = control((id.clone(), "edit"), "Edit", ControlKind::Text, &p);
            if let Some(on_edit) = self.on_edit {
                edit = edit.on_click(move |e, w, cx| on_edit(e, w, cx));
            }
            let mut accept = control((id.clone(), "accept"), "Accept plan", ControlKind::Primary, &p);
            if let Some(on_accept) = self.on_accept {
                accept = accept.on_click(move |e, w, cx| on_accept(e, w, cx));
            }
            card = card.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(ACTIONS_GAP))
                    .py(px(ACTIONS_PAD_Y))
                    .px(px(ACTIONS_PAD_X))
                    .border_t_1()
                    .border_color(p.line)
                    .child(key_hints("⏎ accept · e edit · ⎋ reject", &p))
                    .child(div().flex_1())
                    .child(reject)
                    .child(edit)
                    .child(accept),
            );
        }
        card
    }
}
