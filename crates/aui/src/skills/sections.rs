//! The skills page sections: scope headers, the cost meter and the detail
//! pane.

use std::rc::Rc;

use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, relative, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::button;
use crate::workbench::segmented;

use super::row::SkillMode;

/// `.sect`: the caps scope label, the mono discovery-dirs hint, the hairline
/// rule and the mono count (`This project · .agents/skills — 3`). Build with
/// [`scope_section_header`].
#[derive(IntoElement)]
pub struct ScopeSectionHeader {
    label: SharedString,
    hint_mono: Option<SharedString>,
    count: SharedString,
}

/// A scope section header for `label` with the mono `hint_mono` and `count`.
pub fn scope_section_header(
    label: impl Into<SharedString>,
    hint_mono: Option<SharedString>,
    count: impl Into<SharedString>,
) -> ScopeSectionHeader {
    ScopeSectionHeader { label: label.into(), hint_mono, count: count.into() }
}

/// `.cap{font-size:12.1px;font-weight:600;letter-spacing:.08em}` and the
/// section's `20px 12px 6px` seat.
const CAP_TEXT: f32 = 12.1;
const SECT_TOP: f32 = 20.0;
const SECT_PAD_X: f32 = 12.0;
const SECT_BOTTOM: f32 = 6.0;
const SECT_GAP: f32 = 10.0;

impl RenderOnce for ScopeSectionHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut row = h_flex()
            .w_full()
            .items_center()
            .gap(px(SECT_GAP))
            .mt(px(SECT_TOP))
            .px(px(SECT_PAD_X))
            .pb(px(SECT_BOTTOM))
            .child(
                div()
                    .flex_none()
                    .text_role(TextRole::Caps)
                    .text_color(p.ink_3)
                    .child(self.label.to_uppercase()),
            );
        if let Some(hint) = self.hint_mono {
            row = row.child(div().flex_none().mono(CAP_TEXT).text_color(p.ink_4).child(hint));
        }
        row.child(div().flex_1().h(px(1.0)).bg(p.line)).child(
            div().flex_none().mono(CAP_TEXT).text_color(p.ink_4).child(self.count),
        )
    }
}

/// One ink-grey of the cost bar. The meter carries cost in greys only — no
/// hues — so each segment names which ink it wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkLevel {
    /// `ink`.
    Ink,
    /// `ink-2`.
    Ink2,
    /// `ink-3`.
    Ink3,
    /// `ink-4`.
    Ink4,
}

impl InkLevel {
    /// The palette ink for this level.
    pub fn color(&self, p: &aui_tokens::Palette) -> gpui::Hsla {
        match self {
            InkLevel::Ink => p.ink,
            InkLevel::Ink2 => p.ink_2,
            InkLevel::Ink3 => p.ink_3,
            InkLevel::Ink4 => p.ink_4,
        }
    }
}

/// One slice of the cost bar: `label` for the legend, `fraction` of the bar,
/// `level` for its grey.
#[derive(Debug, Clone, PartialEq)]
pub struct CostSegment {
    /// Legend text (`This project 1.2k`).
    pub label: SharedString,
    /// Share of the bar; normalised at draw (`widths`).
    pub fraction: f32,
    /// Which ink grey the slice wears.
    pub level: InkLevel,
}

impl CostSegment {
    /// A bar slice.
    pub fn new(label: impl Into<SharedString>, fraction: f32, level: InkLevel) -> Self {
        Self { label: label.into(), fraction, level }
    }
}

/// Normalises `segments` to widths that sum to 1: negatives clamp to 0, and
/// an all-zero bar spreads nothing (every width 0).
pub fn segment_widths(segments: &[CostSegment]) -> Vec<f32> {
    let clamped: Vec<f32> = segments.iter().map(|s| s.fraction.max(0.0)).collect();
    let total: f32 = clamped.iter().sum();
    if total <= 0.0 {
        return vec![0.0; segments.len()];
    }
    clamped.into_iter().map(|f| f / total).collect()
}

/// The cost meter: the big startup number, its caption, the 6 px segmented
/// bar in ink greys and the legend. `total_label` is the number (`6.9k`),
/// `caption` sits on its baseline (`tokens of context · 13 of 17 on`).
/// Build with [`cost_meter`].
#[derive(IntoElement)]
pub struct CostMeter {
    total_label: SharedString,
    caption: SharedString,
    segments: Vec<CostSegment>,
}

/// A meter for `total_label` / `caption` over `segments`.
pub fn cost_meter(
    total_label: impl Into<SharedString>,
    caption: impl Into<SharedString>,
    segments: Vec<CostSegment>,
) -> CostMeter {
    CostMeter { total_label: total_label.into(), caption: caption.into(), segments }
}

/// `.bar{height:6px}` with a 2 px gutter between slices; the legend is
/// `12.1px` ink-3 on a 16 px gap.
const BAR_H: f32 = 6.0;
const BAR_GAP: f32 = 2.0;
const TOTAL_TEXT: f32 = 26.4;
const CAP_LINE_TEXT: f32 = 14.3;
const LEGEND_TEXT: f32 = 12.1;
const LEGEND_GAP: f32 = 16.0;
/// The summary seats itself like the list it heads.
const METER_PAD_X: f32 = 12.0;
const METER_BOTTOM: f32 = 16.0;
const METER_GAP: f32 = 8.0;

impl RenderOnce for CostMeter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let widths = segment_widths(&self.segments);
        let mut bar = h_flex()
            .w_full()
            .h(px(BAR_H))
            .gap(px(BAR_GAP))
            .rounded_full()
            .overflow_hidden()
            .bg(p.surface_2);
        for (seg, w) in self.segments.iter().zip(widths) {
            if w <= 0.0 {
                continue;
            }
            bar = bar.child(div().h_full().w(relative(w)).flex_none().bg(seg.level.color(&p)));
        }
        let mut legend = h_flex().w_full().items_center().gap(px(LEGEND_GAP));
        for seg in &self.segments {
            legend = legend.child(div().flex_none().ui(LEGEND_TEXT).text_color(p.ink_3).child(seg.label.clone()));
        }
        v_flex()
            .w_full()
            .px(px(METER_PAD_X))
            .pb(px(METER_BOTTOM))
            .gap(px(METER_GAP))
            .child(
                h_flex()
                    .w_full()
                    .items_baseline()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_none()
                            .ui(TOTAL_TEXT)
                            .line_height(relative(1.0))
                            .semibold()
                            .text_color(p.ink)
                            .child(self.total_label),
                    )
                    .child(div().flex_none().ui(CAP_LINE_TEXT).text_color(p.ink_3).child(self.caption)),
            )
            .child(bar)
            .child(legend)
    }
}

/// The detail pane's three states, in segment order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillState {
    /// Loaded whenever a task matches (`Automatic`).
    #[default]
    Automatic,
    /// Loaded only on `/name` (`Only when I type /`).
    OnlyMention,
    /// Off.
    Off,
}

impl SkillState {
    /// The segment index of this state.
    pub fn index(&self) -> usize {
        match self {
            SkillState::Automatic => 0,
            SkillState::OnlyMention => 1,
            SkillState::Off => 2,
        }
    }

    /// The state for a segment index; anything past `Off` is `Automatic`.
    pub fn from_index(ix: usize) -> Self {
        match ix {
            1 => SkillState::OnlyMention,
            2 => SkillState::Off,
            _ => SkillState::Automatic,
        }
    }

    /// The three segment labels in order.
    pub fn segment_labels() -> [SharedString; 3] {
        ["Automatic".into(), "Only when I type /".into(), "Off".into()]
    }

    /// The state for an `on` switch and a mode.
    pub fn from_activation(on: bool, mode: &SkillMode) -> Self {
        match (on, mode) {
            (false, _) => SkillState::Off,
            (true, SkillMode::Auto) => SkillState::Automatic,
            (true, SkillMode::Only) => SkillState::OnlyMention,
        }
    }

    /// The helper line under the control for this state.
    pub fn hint(&self) -> &'static str {
        match self {
            SkillState::Automatic => "Muse loads it when a task matches the description below.",
            SkillState::OnlyMention => "Muse loads it only when you type its /name.",
            SkillState::Off => "Muse never loads it. The row stays for later.",
        }
    }
}

/// One footer action: a secondary button, or the primary on the far right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailAction {
    /// Stable identity, handed back by
    /// [`SkillDetailIntent::Action`].
    pub id: SharedString,
    /// The button label.
    pub label: SharedString,
    /// Drawn accent, at the row's far right. Only one should be set.
    pub primary: bool,
}

impl DetailAction {
    /// A footer action.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, primary: bool) -> Self {
        Self { id: id.into(), label: label.into(), primary }
    }
}

/// One titled section of the detail pane; the body is the caller's slot
/// (description text, rendered SKILL.md, the file list, diagnostics).
pub struct DetailSection {
    /// The caps title (`What the model sees`).
    pub title: SharedString,
    /// The section body.
    pub body: AnyElement,
}

impl DetailSection {
    /// A titled section around `body`.
    pub fn new(title: impl Into<SharedString>, body: impl IntoElement) -> Self {
        Self { title: title.into(), body: body.into_any_element() }
    }
}

/// What the detail pane asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillDetailIntent {
    /// The state control picked `state`.
    SetState(SkillState),
    /// A footer action was pressed; the argument is [`DetailAction::id`].
    Action(SharedString),
}

type DetailHandler = Rc<dyn Fn(SkillDetailIntent, &mut Window, &mut App)>;

/// The skill detail pane: header (name, scope chip, mono path), the state
/// segmented control, the 2-column stats grid, titled sections with caller
/// slots, and the footer action row — hint left, spacer, secondary buttons,
/// primary right, per the design rules. Build with [`skill_detail`].
#[derive(IntoElement)]
pub struct SkillDetail {
    id: ElementId,
    name: SharedString,
    scope: Option<SharedString>,
    path: Option<SharedString>,
    state: SkillState,
    stats: Vec<(SharedString, SharedString)>,
    sections: Vec<DetailSection>,
    hint: Option<SharedString>,
    actions: Vec<DetailAction>,
    overflow_label: Option<SharedString>,
    on_intent: Option<DetailHandler>,
}

/// A detail pane for `name`.
pub fn skill_detail(id: impl Into<ElementId>, name: impl Into<SharedString>) -> SkillDetail {
    SkillDetail {
        id: id.into(),
        name: name.into(),
        scope: None,
        path: None,
        state: SkillState::Automatic,
        stats: Vec::new(),
        sections: Vec::new(),
        hint: None,
        actions: Vec::new(),
        overflow_label: None,
        on_intent: None,
    }
}

/// `.name{font-size:19.8px}` with the scope chip; the path mono line under it.
const DETAIL_NAME: f32 = 19.8;
const DETAIL_PATH: f32 = 12.1;
/// The stats card: `14px 16px` padding, radius 8, `14px 16px` gaps.
const STATS_PAD_Y: f32 = 14.0;
const STATS_PAD_X: f32 = 16.0;
const STATS_GAP_Y: f32 = 14.0;
const STATS_GAP_X: f32 = 16.0;
const STAT_KEY: f32 = 12.1;
const STAT_VALUE: f32 = 14.3;
/// Section titles sit `22px` past the block above; bodies `8px` under.
const SECTION_TOP: f32 = 22.0;
const SECTION_GAP: f32 = 8.0;
/// The footer: `12px 24px` padding, hairline top, 8 px action gap.
const FOOT_PAD_Y: f32 = 12.0;
const FOOT_PAD_X: f32 = 24.0;
const FOOT_GAP: f32 = 8.0;
const FOOT_HINT: f32 = 12.1;

impl SkillDetail {
    /// The scope chip (`This project`, `Personal`, `Built-in`).
    pub fn scope(mut self, scope: impl Into<SharedString>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// The mono path under the name.
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// The state the segmented control shows.
    pub fn state(mut self, state: SkillState) -> Self {
        self.state = state;
        self
    }

    /// A stats-grid cell (`At startup` / `290 tokens`).
    pub fn stat(mut self, key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.stats.push((key.into(), value.into()));
        self
    }

    /// A titled section around the caller's body slot.
    pub fn section(mut self, title: impl Into<SharedString>, body: impl IntoElement) -> Self {
        self.sections.push(DetailSection::new(title, body));
        self
    }

    /// The footer hint on the left (`Applies to open sessions`).
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// A footer action (secondary, or the one primary on the far right).
    pub fn action(mut self, action: DetailAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Draws the `⋯` overflow button with this accessible name.
    pub fn overflow(mut self, label: impl Into<SharedString>) -> Self {
        self.overflow_label = Some(label.into());
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(SkillDetailIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SkillDetail {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        let mut head = h_flex().w_full().items_center().gap(px(10.0)).child(
            div()
                .flex_none()
                .ui(DETAIL_NAME)
                .line_height(relative(1.0))
                .semibold()
                .text_color(p.ink)
                .child(self.name.clone()),
        );
        if let Some(scope) = self.scope {
            head = head.child(crate::data::chip((id.clone(), "scope"), scope));
        }

        // The pane seats its body on 24 px sides, like the mockup's 460 px
        // detail column; the footer carries its own padding below.
        let mut body =
            v_flex().flex_1().w_full().min_h(px(0.0)).overflow_hidden().px(px(24.0)).pt(px(24.0)).child(head);
        if let Some(path) = self.path {
            body = body.child(
                div()
                    .flex_none()
                    .w_full()
                    .truncate()
                    .mono(DETAIL_PATH)
                    .line_height(relative(scale::LH_MONO))
                    .text_color(p.ink_3)
                    .mt(px(6.0))
                    .child(path),
            );
        }

        // The state control + its helper line.
        let seg_id: ElementId = (id.clone(), SharedString::from("state")).into();
        let mut seg = segmented(seg_id, SkillState::segment_labels().to_vec(), self.state.index())
            .accessibility_label(SharedString::from(format!("{} state", self.name)));
        if let Some(handler) = self.on_intent.clone() {
            seg = seg.on_select(move |ix, w, cx| handler(SkillDetailIntent::SetState(SkillState::from_index(ix)), w, cx));
        }
        body = body.child(div().flex_none().w_full().mt(px(20.0)).child(seg)).child(
            div()
                .flex_none()
                .w_full()
                .ui(STAT_KEY)
                .line_height(relative(scale::LH_UI))
                .text_color(p.ink_3)
                .mt(px(8.0))
                .child(self.state.hint()),
        );

        // The 2-column stats grid.
        if !self.stats.is_empty() {
            let mut grid = v_flex()
                .flex_none()
                .w_full()
                .gap(px(STATS_GAP_Y))
                .mt(px(20.0))
                .p(px(STATS_PAD_Y))
                .px(px(STATS_PAD_X))
                .rounded(px(scale::R_MD))
                .border_1()
                .border_color(p.line)
                .bg(p.surface_1);
            for pair in self.stats.chunks(2) {
                let mut row = h_flex().w_full().gap(px(STATS_GAP_X));
                for (key, value) in pair {
                    row = row.child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(div().flex_none().ui(STAT_KEY).text_color(p.ink_3).child(key.clone()))
                            .child(
                                div()
                                    .flex_none()
                                    .ui(STAT_VALUE)
                                    .line_height(relative(scale::LH_UI))
                                    .text_color(p.ink)
                                    .mt(px(2.0))
                                    .child(value.clone()),
                            ),
                    );
                }
                if pair.len() == 1 {
                    row = row.child(div().flex_1());
                }
                grid = grid.child(row);
            }
            body = body.child(grid);
        }

        // Titled sections with caller slots.
        for section in self.sections {
            body = body
                .child(
                    div()
                        .flex_none()
                        .text_role(TextRole::Caps)
                        .text_color(p.ink_3)
                        .mt(px(SECTION_TOP))
                        .child(section.title.to_uppercase()),
                )
                .child(div().flex_none().w_full().mt(px(SECTION_GAP)).child(section.body));
        }

        // The footer action row: hint left, spacer, secondaries, primary.
        let mut foot = h_flex()
            .flex_none()
            .w_full()
            .items_center()
            .gap(px(FOOT_GAP))
            .px(px(FOOT_PAD_X))
            .py(px(FOOT_PAD_Y))
            .border_t_1()
            .border_color(p.line);
        if let Some(hint) = self.hint {
            foot = foot.child(div().flex_none().ui(FOOT_HINT).text_color(p.ink_3).child(hint));
        }
        foot = foot.child(div().flex_1());
        if let Some(label) = self.overflow_label {
            let mut dots = crate::data::icon_button((id.clone(), "overflow"), aui_icons::IconName::Dots).ghost();
            dots = dots.accessibility_label(label);
            if let Some(handler) = self.on_intent.clone() {
                // The overflow menu is the caller's; the press itself reports
                // through the shared intent as the `overflow` action.
                dots = dots.on_click(move |_, w, cx| handler(SkillDetailIntent::Action("overflow".into()), w, cx));
            }
            foot = foot.child(dots);
        }
        let (secondaries, primaries): (Vec<_>, Vec<_>) = self.actions.iter().partition(|a| !a.primary);
        for action in secondaries.into_iter().chain(primaries) {
            let mut btn = button((id.clone(), action.id.clone()), action.label.clone());
            if action.primary {
                btn = btn.primary();
            }
            btn = btn.accessibility_label(action.label.clone());
            if let Some(handler) = self.on_intent.clone() {
                let action_id = action.id.clone();
                btn = btn.on_click(move |_, w, cx| handler(SkillDetailIntent::Action(action_id.clone()), w, cx));
            }
            foot = foot.child(btn);
        }

        v_flex().w_full().h_full().child(div().flex_none().w_full().child(body)).child(foot)
    }
}
