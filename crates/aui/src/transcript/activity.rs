//! Card 33: the activity group — consecutive quick steps folded into one
//! summary row with a step glyph strip, and a timeline of the steps.

use aui_motion::{pulse_ring, shimmer_text};
use aui_protocol::{ActivityState, Step, StepState};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{glyph_err, glyph_ok, spinner};
use crate::icons::{icon, IconName};
use crate::transcript::transcript_card;
use crate::util::ClickHandler;

/// Header text is 12.5 px.
const HEADER_TEXT: f32 = 12.5;
/// `.steps{gap:3px;margin-left:2px} .steps i{width:14px;height:14px;border-radius:4px} svg 9px`.
const STEP_GAP: f32 = 3.0;
const STEP_ML: f32 = 2.0;
const STEP_TILE: f32 = 14.0;
const STEP_GLYPH: f32 = 9.0;
/// `.tl{padding:6px 10px 8px 10px}`.
const TL_PAD_TOP: f32 = 6.0;
const TL_PAD_X: f32 = 10.0;
const TL_PAD_BOTTOM: f32 = 8.0;
/// `.st{grid-template-columns:18px 1fr auto;gap:0 8px;min-height:30px;font-size:12.5px}` with the rail at 8.5 px.
const NODE: f32 = 18.0;
const ROW_GAP: f32 = 8.0;
const ROW_TEXT: f32 = 12.5;
const RAIL_X: f32 = 8.5;
/// `.st .g i{width:8px;height:8px}` and the 3 px accent-soft ring while running.
const DOT: f32 = 8.0;
const RING: f32 = 3.0;
/// `.st .v{margin-right:6px}`.
const VERB_GAP: f32 = 6.0;

/// The activity group. Build with [`activity_group`].
#[derive(IntoElement)]
pub struct ActivityGroup {
    id: ElementId,
    steps: Vec<Step>,
    summary: SharedString,
    detail: Option<SharedString>,
    elapsed: SharedString,
    state: ActivityState,
    open: bool,
    on_toggle: Option<ClickHandler>,
}

/// A group of `steps` with the header `summary` (`Running tests`, `Ran 3 commands`).
pub fn activity_group(id: impl Into<ElementId>, steps: Vec<Step>, summary: impl Into<SharedString>, elapsed: impl Into<SharedString>, state: ActivityState) -> ActivityGroup {
    ActivityGroup { id: id.into(), steps, summary: summary.into(), detail: None, elapsed: elapsed.into(), state, open: false, on_toggle: None }
}

impl ActivityGroup {
    /// The ink-3 detail after the summary (`· read 2 files · edited 1 file`).
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Whether the timeline is shown.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Header click.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

/// One 14 px tile of the header's step strip.
fn step_tile(p: &aui_tokens::Palette, state: StepState) -> impl IntoElement {
    let (bg, ink, glyph) = match state {
        StepState::Done => (p.success_soft, p.success, Some(IconName::CheckBold)),
        StepState::Running => (p.accent_soft, p.accent_ink, Some(IconName::Terminal)),
        StepState::Failed => (p.danger_soft, p.danger, Some(IconName::XBold)),
        StepState::Pending => (p.surface_3, p.ink_3, None),
    };
    div()
        .flex_none()
        .size(px(STEP_TILE))
        .rounded(px(scale::R_XS))
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .children(glyph.map(|g| icon(g).size(px(STEP_GLYPH)).color(ink)))
}

impl RenderOnce for ActivityGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut card = transcript_card(id.clone(), self.open);
        match self.state {
            ActivityState::Working => {
                card = card
                    .header(spinner((id.clone(), "spinner")))
                    .header(shimmer_text((id.clone(), "label"), self.summary.clone(), cx).text_size(aui_tokens::scaled(HEADER_TEXT)).font_weight(gpui::FontWeight::MEDIUM));
            }
            ActivityState::Done => {
                card = card.header(glyph_ok()).header(div().medium().child(self.summary.clone()));
            }
            ActivityState::Failed => {
                card = card.header(glyph_err()).header(div().medium().child(self.summary.clone()));
            }
        }
        if let Some(detail) = &self.detail {
            card = card.header(div().text_color(p.ink_3).child(detail.clone()));
        }
        let mut strip = h_flex().flex_none().gap(px(STEP_GAP)).ml(px(STEP_ML));
        for step in &self.steps {
            strip = strip.child(step_tile(&p, step.state));
        }
        card = card
            .header(strip)
            .header(div().flex_1())
            .header(div().text_role(TextRole::MonoSmall).font_weight(gpui::FontWeight::MEDIUM).text_color(p.ink_3).child(self.elapsed.clone()));

        let count = self.steps.len();
        let mut timeline = v_flex().w_full().pt(px(TL_PAD_TOP)).px(px(TL_PAD_X)).pb(px(TL_PAD_BOTTOM));
        for (i, step) in self.steps.iter().enumerate() {
            let pending = step.state == StepState::Pending;
            let node: gpui::AnyElement = match step.state {
                StepState::Done => div().size(px(DOT)).rounded_full().bg(p.success).into_any_element(),
                StepState::Failed => div().size(px(DOT)).rounded_full().bg(p.danger).into_any_element(),
                StepState::Pending => div().size(px(DOT)).rounded_full().bg(p.surface_3).border_1().border_color(p.line_strong).into_any_element(),
                StepState::Running => div()
                    .size(px(DOT))
                    .rounded_full()
                    .bg(p.accent_soft)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().absolute().size(px(DOT + 2.0 * RING)).rounded_full().bg(p.accent_soft))
                    .child(pulse_ring((id.clone(), SharedString::from(format!("step-{i}"))), px(DOT), p.accent, true))
                    .into_any_element(),
            };
            let result_color = match step.result.as_deref() {
                Some(r) if r.starts_with('+') => p.success,
                _ => p.ink_3,
            };
            let mut row = h_flex()
                .relative()
                .w_full()
                .min_h(cx.aui().metrics.row)
                .gap(px(ROW_GAP))
                .ui(ROW_TEXT)
                .text_color(if pending { p.ink_3 } else { p.ink })
                .child(
                    // The rail: a 1 px line behind the node, half height on the first and last rows.
                    div()
                        .absolute()
                        .left(px(RAIL_X))
                        .w(px(1.0))
                        .top(if i == 0 { gpui::relative(0.5) } else { gpui::relative(0.0) })
                        .bottom(if i + 1 == count { gpui::relative(0.5) } else { gpui::relative(0.0) })
                        .bg(p.line),
                )
                .child(div().relative().flex_none().size(px(NODE)).rounded_full().bg(p.surface_1).flex().items_center().justify_center().child(node))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(div().medium().mr(px(VERB_GAP)).whitespace_nowrap().child(step.verb.clone()))
                        .child(div().min_w(px(0.0)).truncate().mono(scale::FS_12).text_color(if pending { p.ink_3 } else { p.ink_2 }).child(step.target.clone())),
                );
            if let Some(result) = &step.result {
                row = row.child(div().flex_none().text_role(TextRole::MonoSmall).font_weight(gpui::FontWeight::MEDIUM).text_color(result_color).child(result.clone()));
            }
            timeline = timeline.child(row);
        }
        card = card.body(timeline);
        if let Some(on_toggle) = self.on_toggle {
            card = card.on_toggle(move |e, w, cx| on_toggle(e, w, cx));
        }
        let _ = window;
        card
    }
}
