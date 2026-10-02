//! `.wt` — the worktree / session row in every state — and `.sr`, the
//! compact one-line row used by the project and date groupings.
//!
//! Inline rename keeps the 30 px row: pass [`dense_field`] (or the same chain
//! by hand) to [`CompactSessionRow::editor`]. The composer-grade field is what
//! blows the row up — its 10 px / 8 px paddings and border need ~36 px in a
//! 30 px row — so the rename field is borderless and chromeless at the
//! row-title size with a fixed single-line height, and the 1 px focus border
//! lives on the wrapper instead of the component:
//!
//! ```ignore
//! div()
//!     .w_full()
//!     .h(px(22.0))
//!     .px(px(6.0))
//!     .rounded(px(scale::R_SM))
//!     .border_1()
//!     .border_color(if focused { p.accent } else { p.line })
//!     .bg(p.surface_1)
//!     .child(dense_field(&rename_state))
//! ```
//!
//! 22 px of wrapper in 4 px of row padding is exactly the 30 px row, so
//! siblings never move while a rename is open.

use aui_icons::{icon, provider_mark, IconName};
use aui_motion::{tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{
    div, prelude::*, px, relative, AnyElement, App, Bounds, Div, ElementId, Entity, IntoElement, Pixels, SharedString,
    Window,
};
use gpui_kit::base::input::TextareaState;
use gpui_kit::component::input::Textarea;
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, spinner, status_dot, tag, ButtonSize};
use crate::nav::{ActivityKind, Byline, MetaItem, RowStatusKind, SessionSummary};
use crate::nav::{LEADING_BOX, NAV_GUTTER, NAV_LABEL_X};
use crate::util::{indexed_child, interaction_flags, TrackInteraction};

/// `.wt{grid-template-columns:14px 1fr auto;gap:2px 8px;padding:9px 10px;margin:2px 8px}`.
/// Inside a flex column (the sidebar) CSS keeps both margins, 4 px apart;
/// inside block flow (card 20's list) they collapse to 2 px — see
/// [`SessionRow::collapse_margins`].
const DOT_COL: f32 = 14.0;
const COL_GAP: f32 = 8.0;
/// The label gap after the leading box: `NAV_LABEL_X - NAV_GUTTER -
/// LEADING_BOX`, so the dot in the box puts the title at `NAV_LABEL_X`.
const SR_LEAD_GAP: f32 = NAV_LABEL_X - NAV_GUTTER - LEADING_BOX;
const ROW_GAP: f32 = 2.0;
const PAD_Y: f32 = 9.0;
const PAD_X: f32 = 10.0;
const MARGIN_Y: f32 = 2.0;
/// `.wt .dot{margin-top:5px}`.
const DOT_TOP: f32 = 5.0;
/// `.wt .t{margin-top:3px}`.
const TIME_TOP: f32 = 3.0;
/// `.wt .meta{gap:6px}`.
const META_GAP: f32 = 6.0;
/// `.wt .meta .trunc{max-width:170px}`.
const BRANCH_MAX: f32 = 170.0;
/// Provider marks on the meta line are 13 px and overlap by 4 px.
const META_MARK: f32 = 13.0;
const MARK_OVERLAP: f32 = 4.0;
/// Activity glyphs: spinner 10 px, shield 11 px.
const ACTIVITY_SPINNER: f32 = 10.0;
const ACTIVITY_ICON: f32 = 11.0;
/// The pin at the start of a pinned session's meta line: 11 px ink-3.
const PIN_META: f32 = 11.0;
/// `.acts{right:8px;top:6px;gap:2px;padding:1px}` and its 4 px slide.
const ACTS_RIGHT: f32 = 8.0;
const ACTS_TOP: f32 = 6.0;
const ACTS_GAP: f32 = 2.0;
const ACTS_PAD: f32 = 1.0;
const ACTS_SLIDE: f32 = 4.0;
const ACTS_GLYPH: f32 = 12.0;
/// `.unread{left:2px;width:3px;height:16px;border-radius:2px}`.
const UNREAD_LEFT: f32 = 2.0;
const UNREAD_W: f32 = 3.0;
const UNREAD_H: f32 = 16.0;
const UNREAD_R: f32 = 2.0;
/// `.child{margin-left:22px;padding-left:6px}` with a 1 px rail; `.child .wt{padding-left:8px}`.
const CHILD_INDENT: f32 = 22.0;
const CHILD_PAD: f32 = 6.0;
const CHILD_ROW_PAD_LEFT: f32 = 8.0;
/// `.sr{min-height:30px;padding:4px 10px;margin:0 8px;font-size:12.5px}`.
/// The ground keeps its 8 px inset while the dot cell is the leading box,
/// so the title starts at `NAV_LABEL_X`.
const SR_PAD_Y: f32 = 4.0;
const SR_PAD_RIGHT: f32 = 10.0;
const SR_MARGIN_X: f32 = 8.0;
/// The trailing slot's share of the title line: capped at 35% with an
/// ellipsis, so the title keeps the other ~65% on a narrow sidebar instead
/// of being starved by "Needs approval".
const SR_TRAILING_MAX: f32 = 0.35;
const SR_TEXT: f32 = 12.5;
const SR_META_TEXT: f32 = 11.5;
/// `.sr.child{margin-left:30px;padding-left:10px}` with the rail at `left:-1px; top/bottom 2px`.
const SR_CHILD_INDENT: f32 = 30.0;
const SR_CHILD_PAD: f32 = 10.0;
const SR_RAIL_INSET: f32 = 2.0;

/// The hover actions on a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    /// Open the terminal.
    Terminal,
    /// Open the browser.
    Browser,
    /// Pin the session.
    Pin,
    /// Rename the session in place.
    Rename,
    /// Take the session out of the list.
    Hide,
    /// Archive the session (confirm first; the app owns the flow).
    Archive,
    /// More…
    More,
}

impl RowAction {
    /// The full tray, which is what a row draws when the caller names no
    /// subset.
    pub const ALL: [RowAction; 4] = [RowAction::Terminal, RowAction::Browser, RowAction::Pin, RowAction::More];

    fn glyph(self) -> IconName {
        match self {
            RowAction::Terminal => IconName::Terminal,
            RowAction::Browser => IconName::Globe,
            RowAction::Pin => IconName::Pin,
            RowAction::Rename => IconName::Edit,
            RowAction::Hide => IconName::Eye,
            RowAction::Archive => IconName::Archive,
            RowAction::More => IconName::Dots,
        }
    }

    fn name(self) -> &'static str {
        match self {
            RowAction::Terminal => "terminal",
            RowAction::Browser => "browser",
            RowAction::Pin => "pin",
            RowAction::Rename => "rename",
            RowAction::Hide => "hide",
            RowAction::Archive => "archive",
            RowAction::More => "more",
        }
    }
}

/// Height of the dense rename field: the compact row is 30 px with 4 px of
/// vertical padding, so the field gets 20 px inside a 22 px bordered wrapper.
pub const DENSE_FIELD_H: f32 = 20.0;

/// The dense single-line field for [`CompactSessionRow::editor`]: the row-title
/// size, no appearance and no border, fixed to one line. Wrap it in the 22 px
/// bordered box from the module docs so the focus ring lives on the wrapper
/// and the row keeps its 30 px while a rename is open.
pub fn dense_field(state: &Entity<TextareaState>) -> Textarea {
    Textarea::new(state)
        .appearance(false)
        .bordered(false)
        .text_size(aui_tokens::scaled(SR_TEXT))
        .h(px(DENSE_FIELD_H))
}

/// The tooltip on a row's pin button: `Pin`, or `Unpin` once pinned.
struct RowTip(SharedString);

impl gpui::Render for RowTip {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        div()
            .px(px(scale::SP_3))
            .py(px(scale::SP_2))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .text_color(p.ink)
            .ui(scale::FS_12)
            .whitespace_nowrap()
            .child(self.0.clone())
    }
}

/// The pin at the start of a pinned session's meta line.
fn pin_mark(p: &aui_tokens::Palette) -> AnyElement {
    icon(IconName::Pin).size(px(PIN_META)).color(p.ink_3).into_any_element()
}

/// The `.acts` hover tray: the actions, faded and slid in on hover.
///
/// Shared by both rows so a rename affordance looks the same wherever it is.
/// On a pinned session the pin button flips to unpin: it draws `PinOff`
/// with an "Unpin" tooltip, and still reports [`RowAction::Pin`].
#[allow(clippy::too_many_arguments)]
fn action_tray(
    id: &ElementId,
    session_id: &SharedString,
    actions: &[RowAction],
    pinned: bool,
    visible: bool,
    on_action: &Option<ActionHandler>,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let p = cx.aui().colors;
    let opacity = tween((id.clone(), "acts-opacity"), if visible { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);
    let slide = tween(
        (id.clone(), "acts-slide"),
        if visible { 0.0f32 } else { ACTS_SLIDE },
        Tween::BASE.with_easing(aui_tokens::Easing::OUT),
        window,
        cx,
    );
    let mut tray = h_flex()
        .absolute()
        .right(px(ACTS_RIGHT) - px(slide))
        .top(px(ACTS_TOP))
        .gap(px(ACTS_GAP))
        .p(px(ACTS_PAD))
        .rounded(px(scale::R_SM))
        .bg(p.surface_2)
        .opacity(opacity);
    for action in actions {
        let action = *action;
        let unpin = action == RowAction::Pin && pinned;
        let glyph = if unpin { IconName::PinOff } else { action.glyph() };
        let mut b = icon_button((id.clone(), action.name()), glyph).ghost().size(ButtonSize::Xs).icon_size(px(ACTS_GLYPH));
        if let Some(on_action) = on_action.clone() {
            let key = session_id.clone();
            // The tray sits inside the row, and gpui fires every `on_click`
            // up the tree in the bubble phase: without this the pin (or the
            // pencil, or the archive box) also selected the row, and a
            // select is a full re-open of the session. The action is the
            // whole click.
            b = b.on_click(move |_, w, cx| {
                cx.stop_propagation();
                on_action(&key, action, w, cx)
            });
        }
        if action != RowAction::Pin {
            tray = tray.child(b);
            continue;
        }
        // The pin names what it will do: "Pin", or "Unpin" once pinned.
        let tip: SharedString = if unpin { "Unpin".into() } else { "Pin".into() };
        tray = tray.child(
            div().id((id.clone(), SharedString::from(format!("{}-tip", action.name())))).tooltip(move |_, cx| cx.new(|_| RowTip(tip.clone())).into()).child(b),
        );
    }
    if !visible && opacity <= 0.001 {
        tray = tray.invisible();
    }
    tray
}

type SelectHandler = std::rc::Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ActionHandler = std::rc::Rc<dyn Fn(&SharedString, RowAction, &mut Window, &mut App)>;
/// Hover enter/leave with the session id: the hover card's arm. See
/// [`crate::util::TrackInteraction::track_interaction_reported`].
pub(crate) type HoverHandler = std::rc::Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
/// Hover enter/leave with the session id and the row's own window bounds,
/// measured in prepaint: the hover card's arm and seat. The bounds travel
/// with the report, so the caller seats the card from the row instead of the
/// pointer. See [`CompactSessionRow::on_hover_bounds`].
pub(crate) type HoverBoundsHandler =
    std::rc::Rc<dyn Fn(&SharedString, bool, Bounds<Pixels>, &mut Window, &mut App)>;
/// The row's last prepaint bounds, shared between its bounds wrapper and its
/// hover report (see [`CompactSessionRow::on_hover_bounds`]).
type RowBoundsCell = std::rc::Rc<std::cell::RefCell<Option<Bounds<Pixels>>>>;
/// Wrap a finished row for [`CompactSessionRow::on_hover_bounds`]: a plain
/// full-width column reporting its first child's bounds into `cell` — the
/// same shape the selected-row bounds intent wears, so the row stretches
/// inside it exactly as it stretches in the group. No cell, no wrapper, and
/// rows without the handler build exactly as before.
fn wrap_hover_bounds(row: impl IntoElement, cell: &Option<RowBoundsCell>) -> AnyElement {
    match cell.clone() {
        Some(capture) => div()
            .w_full()
            .on_children_prepainted(move |bounds, _, _| {
                *capture.borrow_mut() = bounds.first().copied();
            })
            .child(row)
            .into_any_element(),
        None => row.into_any_element(),
    }
}

/// The full session row. Build with [`session_row`].
#[derive(IntoElement)]
pub struct SessionRow {
    id: ElementId,
    session: SessionSummary,
    selected: bool,
    nested: bool,
    margin_x: f32,
    row_gap: f32,
    name_size: f32,
    meta_size: f32,
    branch_max: f32,
    activity_max: Option<f32>,
    show_actions: bool,
    actions: Option<Vec<RowAction>>,
    margin_bottom: f32,
    on_select: Option<SelectHandler>,
    on_action: Option<ActionHandler>,
}

/// A row for `session`. Children are rendered beneath it, nested.
pub fn session_row(id: impl Into<ElementId>, session: SessionSummary) -> SessionRow {
    SessionRow {
        id: id.into(),
        session,
        selected: false,
        nested: false,
        margin_x: 0.0,
        row_gap: ROW_GAP,
        name_size: scale::FS_13,
        meta_size: scale::FS_12,
        branch_max: BRANCH_MAX,
        activity_max: None,
        show_actions: true,
        actions: None,
        margin_bottom: MARGIN_Y,
        on_select: None,
        on_action: None,
    }
}

impl SessionRow {
    /// Selected: surface-3 ground.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Horizontal margin (8 inside a sidebar, 0 inside a padded list).
    pub fn margin_x(mut self, margin: f32) -> Self {
        self.margin_x = margin;
        self
    }

    /// Gap between the row's lines (2 in card 20, 3 in the shell and sidebar).
    pub fn row_gap(mut self, gap: f32) -> Self {
        self.row_gap = gap;
        self
    }

    /// Name and meta sizes (13 / 12 by default; the sidebar card uses 12.5 / 11.5).
    pub fn text_sizes(mut self, name: f32, meta: f32) -> Self {
        self.name_size = name;
        self.meta_size = meta;
        self
    }

    /// Max width of the truncated branch tag.
    pub fn branch_max(mut self, max: f32) -> Self {
        self.branch_max = max;
        self
    }

    /// Max width of the activity sentence (defaults to the branch max).
    pub fn activity_max(mut self, max: f32) -> Self {
        self.activity_max = Some(max);
        self
    }

    /// Whether the hover action tray exists.
    pub fn show_actions(mut self, show: bool) -> Self {
        self.show_actions = show;
        self
    }

    /// Which actions the tray carries; [`RowAction::ALL`] when unset.
    pub fn actions(mut self, actions: Vec<RowAction>) -> Self {
        self.actions = Some(actions);
        self
    }

    /// For rows in block flow, where CSS collapses adjacent 2 px margins into
    /// one: keeps the top margin only.
    pub fn collapse_margins(mut self) -> Self {
        self.margin_bottom = 0.0;
        self
    }

    /// Row click.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }

    /// Hover-action click.
    pub fn on_action(mut self, f: impl Fn(&SharedString, RowAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }

    fn nested(mut self) -> Self {
        self.nested = true;
        self
    }
}

/// Which second line the compact row draws for a summary: the row model
/// behind the uniform two-line height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondLineKind {
    /// Nothing to show: empty space keeping the two-line height.
    Empty,
    /// The legacy meta tags (and activity sentence), one truncating line.
    Meta,
    /// Muted placeholder text.
    Placeholder,
    /// One line of preview text.
    Preview,
    /// Last ask plus last result on the one second line.
    Byline,
}

/// Which second line `summary` draws. An explicit [`Byline`] wins the slot;
/// otherwise the legacy meta tags show, or empty space when there are none —
/// so every row keeps its second line whatever the caller passes.
pub fn second_line_kind(summary: &SessionSummary) -> SecondLineKind {
    match &summary.byline {
        Some(Byline::Placeholder(_)) => SecondLineKind::Placeholder,
        Some(Byline::Preview(_)) => SecondLineKind::Preview,
        Some(Byline::TwoLines { .. }) => SecondLineKind::Byline,
        None if has_legacy_second_line(summary) => SecondLineKind::Meta,
        None => SecondLineKind::Empty,
    }
}

impl SecondLineKind {
    /// Slot height in lines: one in every state, so a row is always its
    /// title plus this line — two lines tall, dots on their rhythm.
    pub fn lines(self) -> u8 {
        1
    }

    /// Every state ellipsizes at the row's width…
    pub fn truncate(self) -> bool {
        true
    }

    /// …and none wraps onto another line.
    pub fn wraps(self) -> bool {
        false
    }

    /// Ink for the slot's plain text: the placeholder sits a step dimmer
    /// (ink-4, the search-field placeholder tone); everything else reads in
    /// the usual second-line ink-3. The byline's ask runs a step brighter at
    /// ink-2 while its result uses this.
    pub fn ink(self, palette: &aui_tokens::Palette) -> gpui::Hsla {
        match self {
            SecondLineKind::Placeholder => palette.ink_4,
            _ => palette.ink_3,
        }
    }
}

/// Whether the legacy meta tags (or activity) give the row a second line:
/// the repo/branch tags, meta items, provider marks, the pin, or the
/// activity sentence. Mirrors the render below, so the kind and the row
/// cannot disagree about when the legacy line exists.
fn has_legacy_second_line(s: &SessionSummary) -> bool {
    s.repo.is_some() || s.branch.is_some() || !s.meta.is_empty() || !s.providers.is_empty() || s.pinned || s.activity.is_some()
}

/// The meta line: tags, marks and extra items, one line, truncating.
fn meta_line(p: &aui_tokens::Palette, size: f32, items: Vec<gpui::AnyElement>) -> Div {
    h_flex().w_full().min_w(px(0.0)).overflow_hidden().gap(px(META_GAP)).ui(size).text_color(p.ink_3).whitespace_nowrap().children(items)
}

fn meta_items(p: &aui_tokens::Palette, s: &SessionSummary, branch_max: f32) -> Vec<gpui::AnyElement> {
    let mut items: Vec<gpui::AnyElement> = Vec::new();
    if let Some(repo) = &s.repo {
        items.push(tag(repo.clone()).into_any_element());
    }
    if let Some(branch) = &s.branch {
        items.push(tag(branch.clone()).truncate(branch_max).into_any_element());
    }
    for item in &s.meta {
        items.push(match item {
            MetaItem::Text(t) => div().child(t.clone()).into_any_element(),
            MetaItem::Tag(t) => tag(t.clone()).into_any_element(),
            MetaItem::Danger(t) => div().text_color(p.danger).child(t.clone()).into_any_element(),
            MetaItem::Warning(t) => div().text_color(p.warning).child(t.clone()).into_any_element(),
        });
    }
    if !s.providers.is_empty() {
        let mut marks = h_flex().flex_none();
        for (i, provider) in s.providers.iter().enumerate() {
            let mark = provider_mark(*provider).size(px(META_MARK));
            marks = marks.child(if i == 0 { div().child(mark) } else { div().ml(px(-MARK_OVERLAP)).child(mark) });
        }
        items.push(marks.into_any_element());
    }
    items
}

fn activity_line(p: &aui_tokens::Palette, id: &ElementId, s: &SessionSummary, size: f32, max: Option<f32>) -> Option<Div> {
    let activity = s.activity.as_ref()?;
    let (color, glyph): (gpui::Hsla, Option<gpui::AnyElement>) = match activity.kind {
        ActivityKind::Working => (p.ink_2, Some(spinner((id.clone(), "activity-spinner")).size(px(ACTIVITY_SPINNER)).into_any_element())),
        ActivityKind::Waiting => (p.warning, Some(icon(IconName::Shield).size(px(ACTIVITY_ICON)).color(p.warning).into_any_element())),
        ActivityKind::Failed => (p.danger, None),
        ActivityKind::Plain => (p.ink_2, None),
    };
    let mut text = div().min_w(px(0.0)).truncate().child(activity.text.clone());
    if let Some(max) = max {
        text = text.max_w(px(max));
    }
    Some(meta_line(p, size, Vec::new()).text_color(color).children(glyph).child(text))
}

/// Option B's context line: which second line the compact row draws.
///
/// Priority: [`SessionSummary::attention`] (the approval command or pending
/// question) first, then the [`Byline`] two-liner / preview / placeholder,
/// then `project · branch`, then the legacy meta tags, else empty space —
/// and the line keeps its height in every state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextLineKind {
    /// The approval command or pending question, verbatim.
    Attention,
    /// Muted placeholder text.
    Placeholder,
    /// One line of preview text.
    Preview,
    /// Last ask plus last result on the one line.
    Byline,
    /// `project · branch` (or whichever half exists).
    Project,
    /// The legacy meta tags (and activity sentence), one truncating line.
    Meta,
    /// Nothing to show: empty space keeping the line height.
    Empty,
}

/// Which context line `summary` draws. Presence wins over content: an
/// explicitly set [`Byline`] or `attention` keeps its kind even when empty,
/// so the kind and the row cannot disagree.
pub fn context_line_kind(summary: &SessionSummary) -> ContextLineKind {
    if summary.attention.is_some() {
        return ContextLineKind::Attention;
    }
    match &summary.byline {
        Some(Byline::Placeholder(_)) => ContextLineKind::Placeholder,
        Some(Byline::Preview(_)) => ContextLineKind::Preview,
        Some(Byline::TwoLines { .. }) => ContextLineKind::Byline,
        None if summary.repo.is_some() || summary.branch.is_some() => ContextLineKind::Project,
        None if has_non_project_legacy(summary) => ContextLineKind::Meta,
        None => ContextLineKind::Empty,
    }
}

impl ContextLineKind {
    /// Slot height in lines: one in every state, so a Two-density row is
    /// its title plus this context line — two lines tall.
    pub fn lines(self) -> u8 {
        1
    }

    /// Every state ellipsizes at the row's width…
    pub fn truncate(self) -> bool {
        true
    }

    /// …and none wraps onto another line.
    pub fn wraps(self) -> bool {
        false
    }
}

/// Legacy second-line content other than `repo` / `branch`: meta items,
/// provider marks, the pin, or the activity sentence. Checked after the
/// `project · branch` arm of [`context_line_kind`], so the two cannot both
/// claim a row that only carries tags.
fn has_non_project_legacy(s: &SessionSummary) -> bool {
    !s.meta.is_empty() || !s.providers.is_empty() || s.pinned || s.activity.is_some()
}

/// `project · branch` for the context line: whichever half the caller set,
/// joined with a middot, or `None` when neither exists.
pub fn project_branch_text(summary: &SessionSummary) -> Option<SharedString> {
    match (&summary.repo, &summary.branch) {
        (Some(repo), Some(branch)) => Some(format!("{repo} · {branch}").into()),
        (Some(repo), None) => Some(repo.clone()),
        (None, Some(branch)) => Some(branch.clone()),
        (None, None) => None,
    }
}

/// How many lines a [`CompactSessionRow`] draws: the status verb line is
/// gone (hover holds the whole picture), so state travels on the leading
/// glyph and the trailing slot instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowDensity {
    /// Status glyph · title · trailing age/state: one line.
    One,
    /// Line 1 as [`RowDensity::One`]; line 2 is the context/byline line.
    /// The default for new callers.
    #[default]
    Two,
}

impl RowDensity {
    /// Lines every row draws in this density: one count, so one height —
    /// whatever the caller passes.
    pub fn lines(self) -> u8 {
        match self {
            RowDensity::One => 1,
            RowDensity::Two => 2,
        }
    }
}

/// What the compact row's trailing slot draws: the caller's relative time,
/// or the state verb when the session waits on the person or has failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowTrailing {
    /// The caller's relative time (`49m`), muted.
    Age,
    /// Waiting on the person, in the warning colour.
    NeedsApproval,
    /// Failed, in the danger colour.
    Failed,
}

/// Which trailing slot `summary` draws. Failure wins over waiting; anything
/// else reads the age. Mirrors the render, so the kind and the row cannot
/// disagree.
pub fn row_trailing_kind(summary: &SessionSummary) -> RowTrailing {
    let failed = summary.state == aui_tokens::AgentState::Failed
        || summary.status.as_ref().is_some_and(|s| s.kind == RowStatusKind::Failed);
    if failed {
        return RowTrailing::Failed;
    }
    let waiting = summary.state == aui_tokens::AgentState::Waiting
        || summary
            .status
            .as_ref()
            .is_some_and(|s| matches!(s.kind, RowStatusKind::NeedsApproval | RowStatusKind::Asked));
    if waiting {
        return RowTrailing::NeedsApproval;
    }
    RowTrailing::Age
}

impl RowTrailing {
    /// The slot's words: the age, or the state verb.
    pub fn text(self, summary: &SessionSummary) -> SharedString {
        match self {
            RowTrailing::Age => summary.elapsed.clone(),
            RowTrailing::NeedsApproval => SharedString::from("Needs approval"),
            RowTrailing::Failed => SharedString::from("Failed"),
        }
    }

    /// The slot's ink: muted for the age, the state colour otherwise.
    pub fn ink(self, palette: &aui_tokens::Palette) -> gpui::Hsla {
        match self {
            RowTrailing::Age => palette.ink_3,
            RowTrailing::NeedsApproval => palette.warning,
            RowTrailing::Failed => palette.danger,
        }
    }
}

/// The compact row's ground: selected fills surface-3, a step above hover's
/// surface-2 — so the active row reads apart from a hovered one.
pub fn compact_row_ground(selected: bool, palette: &aui_tokens::Palette) -> gpui::Hsla {
    if selected { palette.surface_3 } else { palette.surface_2 }
}

/// The compact row's title ink: full ink when selected, ink-2 otherwise.
pub fn compact_row_title_ink(selected: bool, palette: &aui_tokens::Palette) -> gpui::Hsla {
    if selected { palette.ink } else { palette.ink_2 }
}

/// The compact row's context (second) line: always exactly one line —
/// attention, byline, `project · branch`, legacy meta, or empty space —
/// truncating with an ellipsis at the row's width, never wrapping.
fn context_line(p: &aui_tokens::Palette, id: &ElementId, s: &SessionSummary) -> AnyElement {
    if let Some(text) = &s.attention {
        let shown = if text.trim().is_empty() { SharedString::from(" ") } else { text.clone() };
        return meta_line(p, SR_META_TEXT, Vec::new())
            .text_color(p.ink_2)
            .child(div().min_w(px(0.0)).truncate().child(shown))
            .into_any_element();
    }
    match &s.byline {
        Some(Byline::Placeholder(text)) => {
            let shown = if text.is_empty() { SharedString::from(" ") } else { text.clone() };
            meta_line(p, SR_META_TEXT, Vec::new())
                .text_color(SecondLineKind::Placeholder.ink(p))
                .child(div().min_w(px(0.0)).truncate().child(shown))
                .into_any_element()
        }
        Some(Byline::Preview(text)) => {
            let shown = if text.is_empty() { SharedString::from(" ") } else { text.clone() };
            meta_line(p, SR_META_TEXT, Vec::new())
                .text_color(SecondLineKind::Preview.ink(p))
                .child(div().min_w(px(0.0)).truncate().child(shown))
                .into_any_element()
        }
        Some(Byline::TwoLines { ask, result }) => {
            let line = meta_line(p, SR_META_TEXT, Vec::new()).text_color(SecondLineKind::Byline.ink(p));
            match (ask.is_empty(), result.is_empty()) {
                (true, true) => line.child(div().child(" ")),
                (true, false) => line.child(div().min_w(px(0.0)).truncate().child(result.clone())),
                (false, true) => line.child(div().min_w(px(0.0)).truncate().text_color(p.ink_2).child(ask.clone())),
                // One line, not two `flex_1` halves: each side hugs its
                // content, so a short ask no longer leaves half the row
                // empty before the separator. When the row runs out of
                // room both shrink in proportion to their content with an
                // ellipsis (`min_w(0)` lets each shrink past its content —
                // the same shape as the attention and preview lines above).
                (false, false) => line
                    .child(div().min_w(px(0.0)).truncate().text_color(p.ink_2).child(ask.clone()))
                    .child(div().flex_none().child("·"))
                    .child(div().min_w(px(0.0)).truncate().child(result.clone())),
            }
            .into_any_element()
        }
        None => {
            if let Some(text) = project_branch_text(s) {
                return meta_line(p, SR_META_TEXT, Vec::new())
                    .text_color(p.ink_3)
                    .child(div().min_w(px(0.0)).truncate().child(text))
                    .into_any_element();
            }
            if !has_non_project_legacy(s) {
                return meta_line(p, SR_META_TEXT, Vec::new()).child(div().child(" ")).into_any_element();
            }
            let mut items = Vec::new();
            if let Some(activity) = &s.activity {
                let color = match activity.kind {
                    ActivityKind::Working | ActivityKind::Plain => p.ink_3,
                    ActivityKind::Waiting => p.warning,
                    ActivityKind::Failed => p.danger,
                };
                if activity.kind == ActivityKind::Working {
                    items.push(spinner((id.clone(), "context-spinner")).size(px(ACTIVITY_SPINNER)).into_any_element());
                }
                items.push(div().min_w(px(0.0)).truncate().text_color(color).child(activity.text.clone()).into_any_element());
            }
            let mut meta = meta_items(p, s, BRANCH_MAX);
            meta.append(&mut items);
            if s.pinned {
                meta.insert(0, pin_mark(p));
            }
            meta_line(p, SR_META_TEXT, meta).into_any_element()
        }
    }
}

impl RenderOnce for SessionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let s = self.session;
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let hovered = flags.hovered;

        let tint = if self.selected { p.surface_3 } else { p.surface_2 };
        let bg = tint_fade((id.clone(), "bg"), self.selected || hovered, tint, Tween::FAST, window, cx);
        let acts_visible = hovered && self.show_actions;
        let acts_opacity = tween((id.clone(), "acts-opacity"), if acts_visible { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);
        let time_opacity = 1.0 - acts_opacity;

        let dot = div().flex_none().w(px(DOT_COL)).mt(px(DOT_TOP)).child(status_dot((id.clone(), "dot"), s.state).pulse(s.pulse));
        let name_size = if self.nested { scale::FS_12 } else { self.name_size };
        let name = div().flex_1().min_w(px(0.0)).ui(name_size).medium().text_color(p.ink).truncate().child(s.name.clone());
        let time = div()
            .flex_none()
            .mt(px(TIME_TOP))
            .text_role(TextRole::MonoSmall)
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(p.ink_3)
            .opacity(time_opacity)
            .child(s.elapsed.clone());

        let mut lines = v_flex().flex_1().min_w(px(0.0)).gap(px(self.row_gap));
        lines = lines.child(h_flex().w_full().items_start().gap(px(COL_GAP)).child(name).child(time));
        let mut items = meta_items(&p, &s, self.branch_max);
        if s.pinned {
            items.insert(0, pin_mark(&p));
        }
        if !items.is_empty() {
            lines = lines.child(meta_line(&p, self.meta_size, items));
        }
        // `.meta .trunc{max-width}` applies to the activity sentence too.
        let activity_max = self.activity_max.or(Some(self.branch_max));
        if let Some(activity) = activity_line(&p, &id, &s, self.meta_size, activity_max) {
            lines = lines.child(activity);
        }

        let mut row = h_flex()
            .id(id.clone())
            .relative()
            .w_full()
            .items_start()
            .gap(px(COL_GAP))
            .py(px(PAD_Y))
            .pl(px(if self.nested { CHILD_ROW_PAD_LEFT } else { PAD_X }))
            .pr(px(PAD_X))
            .mt(px(MARGIN_Y))
            .mb(px(self.margin_bottom))
            .mx(px(self.margin_x))
            .rounded(px(scale::R_MD))
            .bg(bg)
            .cursor_pointer()
            .track_interaction(&state)
            .child(dot)
            .child(lines);

        if s.unread {
            row = row.child(
                div()
                    .absolute()
                    .left(px(UNREAD_LEFT))
                    .top(gpui::relative(0.5))
                    .mt(px(-UNREAD_H / 2.0))
                    .w(px(UNREAD_W))
                    .h(px(UNREAD_H))
                    .rounded(px(UNREAD_R))
                    .bg(p.accent),
            );
        }

        if self.show_actions {
            let actions = self.actions.clone().unwrap_or_else(|| RowAction::ALL.to_vec());
            row = row.child(action_tray(&id, &s.id, &actions, s.pinned, acts_visible, &self.on_action, window, cx));
        }

        if let Some(on_select) = self.on_select.clone() {
            let key = s.id.clone();
            row = row.on_click(move |_, w, cx| on_select(&key, w, cx));
        }

        if s.children.is_empty() {
            return row.into_any_element();
        }
        let mut children = v_flex()
            .ml(px(CHILD_INDENT))
            .pl(px(CHILD_PAD))
            .border_l_1()
            .border_color(p.line);
        for (i, child) in s.children.into_iter().enumerate() {
            let child_id: ElementId = indexed_child(&id, "child-", i);
            let mut r = session_row(child_id, child)
                .nested()
                .row_gap(self.row_gap)
                .text_sizes(self.name_size, self.meta_size)
                .branch_max(self.branch_max)
                .show_actions(self.show_actions);
            r.margin_bottom = self.margin_bottom;
            if let Some(h) = self.on_select.clone() {
                r = r.on_select(move |k, w, cx| h(k, w, cx));
            }
            if let Some(h) = self.on_action.clone() {
                r = r.on_action(move |k, a, w, cx| h(k, a, w, cx));
            }
            children = children.child(r);
        }
        v_flex().mx(px(self.margin_x)).child(row.mx(px(0.0))).child(children).into_any_element()
    }
}

/// The compact row (`.sr`). Build with [`compact_session_row`].
#[derive(IntoElement)]
pub struct CompactSessionRow {
    id: ElementId,
    session: SessionSummary,
    selected: bool,
    density: RowDensity,
    nested: bool,
    actions: Vec<RowAction>,
    editor: Option<AnyElement>,
    pulse_phase: Option<f32>,
    on_select: Option<SelectHandler>,
    on_action: Option<ActionHandler>,
    on_hover: Option<HoverHandler>,
    on_hover_bounds: Option<HoverBoundsHandler>,
}

/// A compact row for `session`; children nest beneath with a hairline rail.
pub fn compact_session_row(id: impl Into<ElementId>, session: SessionSummary) -> CompactSessionRow {
    CompactSessionRow {
        id: id.into(),
        density: session.density.unwrap_or(RowDensity::Two),
        session,
        selected: false,
        nested: false,
        actions: Vec::new(),
        editor: None,
        pulse_phase: None,
        on_select: None,
        on_action: None,
        on_hover: None,
        on_hover_bounds: None,
    }
}

impl CompactSessionRow {
    /// Selected: surface-3 ground and ink text.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Row density: [`RowDensity::One`] draws status glyph · title ·
    /// trailing age/state on one line; [`RowDensity::Two`] (the default)
    /// adds the context/byline line under it.
    pub fn density(mut self, density: RowDensity) -> Self {
        self.density = density;
        self
    }

    /// The hover action tray, off by default on a compact row.
    ///
    /// The full row's tray is four fixed affordances; a compact row is used in
    /// so many groupings that it takes the caller's list or draws nothing.
    pub fn actions(mut self, actions: Vec<RowAction>) -> Self {
        self.actions = actions;
        self
    }

    /// Samples the status dot's pulse ring at `phase` instead of mounting
    /// the looping animation: no frame is requested, so the ring only moves
    /// when the caller re-renders (a view on its own timer). Unset: the dot
    /// loops as before.
    pub fn pulse_phase(mut self, phase: f32) -> Self {
        self.pulse_phase = Some(phase);
        self
    }

    /// Replace the name with a field the caller owns: an inline rename.
    ///
    /// The same slot pattern the composer uses. The row is stateless, so the
    /// text, the focus and what Enter and Escape mean all live with whoever
    /// passed the element in — and a row that is being renamed does not open
    /// the session when it is clicked.
    pub fn editor(mut self, editor: impl IntoElement) -> Self {
        self.editor = Some(editor.into_any_element());
        self
    }

    /// Row click.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }

    /// Hover-action click.
    pub fn on_action(mut self, f: impl Fn(&SharedString, RowAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }

    /// Hover enter/leave with the session id. Shares the row's single
    /// `on_hover` slot with the interaction state (a second handler would
    /// replace the tracking), so the caller learns about the pointer even
    /// when nothing re-renders.
    pub fn on_hover(mut self, f: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(std::rc::Rc::new(f));
        self
    }

    /// Hover enter/leave with the session id and the row's own window bounds,
    /// measured in prepaint. Supersedes [`Self::on_hover`]: when set, the row
    /// reports through this alone, so a hover card seats from the row's rect
    /// instead of the pointer. Rows without this handler build exactly as
    /// before — no wrapper, no extra prepaint.
    pub fn on_hover_bounds(
        mut self,
        f: impl Fn(&SharedString, bool, Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hover_bounds = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for CompactSessionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let s = self.session;
        let density = self.density;
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        // The active row fills surface-3, a step above hover's surface-2,
        // with full-ink title — visibly different from hover.
        let tint = compact_row_ground(self.selected, &p);
        let bg = tint_fade((id.clone(), "bg"), self.selected || flags.hovered, tint, Tween::FAST, window, cx);
        let text = compact_row_title_ink(self.selected, &p);
        let label = s.name.clone();

        // State travels on the leading glyph (pulsing when running, the
        // state colour for needs-approval/failed) and the trailing slot:
        // the age, or "Needs approval" / "Failed" in the state colour.
        let trailing_kind = row_trailing_kind(&s);
        let trailing = trailing_kind.text(&s);
        let trailing_ink = trailing_kind.ink(&p);

        let editing = self.editor.is_some();
        let acts_visible = flags.hovered && !self.actions.is_empty() && !editing;
        let mut lines = v_flex().flex_1().min_w(px(0.0)).gap(px(ROW_GAP));
        let title: AnyElement = match self.editor {
            Some(editor) => div().flex_1().min_w(px(0.0)).child(editor).into_any_element(),
            None => div()
                .flex_1()
                .min_w(px(0.0))
                .medium()
                .truncate()
                .debug_selector(|| "compact-title".into())
                .child(s.name.clone())
                .into_any_element(),
        };
        lines = lines.child(
            h_flex()
                .w_full()
                .gap(px(COL_GAP))
                .child(title)
                .child(
                    div()
                        .flex_none()
                        .min_w(px(0.0))
                        .max_w(relative(SR_TRAILING_MAX))
                        .truncate()
                        .text_role(TextRole::MonoSmall)
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(trailing_ink)
                        .debug_selector(|| "compact-trailing".into())
                        .child(trailing),
                ),
        );
        // Two density adds the context line under the title line; One stops
        // here. The context line keeps its line when empty, so every row in
        // a density is the same height in every state.
        if density == RowDensity::Two {
            lines = lines.child(context_line(&p, &id, &s));
        }

        // No `w_full`: at full width the 8 px margins overflow the column and
        // the shell clips them, leaving ~0 px on the right. As a flex item the
        // row stretches to the column minus its margins, so both gutters stay 8.
        let row_base = h_flex()
            .id(id.clone())
            .relative()
            .flex_1()
            .min_w(px(0.0))
            .role(gpui::Role::Button)
            .aria_label(label)
            .min_h(cx.aui().metrics.row)
            .items_center()
            .gap(px(if self.nested { COL_GAP } else { SR_LEAD_GAP }))
            .py(px(SR_PAD_Y))
            .pl(px(if self.nested { SR_CHILD_PAD } else { 0.0 }))
            .pr(px(SR_PAD_RIGHT))
            .ml(px(if self.nested { SR_CHILD_INDENT } else { SR_MARGIN_X }))
            .mr(px(SR_MARGIN_X))
            .rounded(px(scale::R_MD))
            .bg(bg)
            .ui(SR_TEXT)
            .text_color(text)
            .cursor_pointer();
        // The bounds-carrying hover report: the row's own rect travels with the
        // enter/leave, so the caller seats a hover card from the row instead
        // of the pointer. The cell is filled by the full-width wrapper
        // [`wrap_hover_bounds`] applies to the finished row below — a hover
        // needs a paint, and the paint's prepaint fills it first, so the
        // bounds are always there when the report fires.
        let hover_bounds = self.on_hover_bounds.clone();
        let bounds_cell: Option<RowBoundsCell> =
            hover_bounds.as_ref().map(|_| std::rc::Rc::new(std::cell::RefCell::new(None)));
        let mut row = match (hover_bounds, self.on_hover.clone()) {
            (Some(report), _) => {
                let key = s.id.clone();
                let cell = bounds_cell.clone().expect("cell made with the bounds report");
                row_base.track_interaction_reported(&state, move |hovered, w, cx| {
                    if let Some(bounds) = *cell.borrow() {
                        report(&key, hovered, bounds, w, cx);
                    }
                })
            }
            (None, Some(report)) => {
                let key = s.id.clone();
                row_base.track_interaction_reported(&state, move |hovered, w, cx| report(&key, hovered, w, cx))
            }
            (None, None) => row_base.track_interaction(&state),
        };
        row = row.child(
                div()
                    .flex_none()
                    .w(px(if self.nested { COL_GAP } else { LEADING_BOX }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child({
                        let mut dot = status_dot((id.clone(), "dot"), s.state).pulse(s.pulse);
                        if let Some(phase) = self.pulse_phase {
                            dot = dot.phase(phase);
                        }
                        dot
                    }),
            )
            .child(lines);
        if self.nested {
            row = row.child(div().absolute().left(px(-1.0)).top(px(SR_RAIL_INSET)).bottom(px(SR_RAIL_INSET)).w(px(1.0)).bg(p.line));
        }
        if !self.actions.is_empty() {
            row = row.child(action_tray(&id, &s.id, &self.actions, s.pinned, acts_visible, &self.on_action, window, cx));
        }
        // A row being renamed is not a row waiting to be opened: a click on the
        // field it is holding would otherwise close the field it just opened.
        if let (Some(on_select), false) = (self.on_select.clone(), editing) {
            let key = s.id.clone();
            row = row.on_click(move |_, w, cx| on_select(&key, w, cx));
        }
        if s.children.is_empty() {
            return wrap_hover_bounds(row, &bounds_cell).into_any_element();
        }
        let mut col = v_flex().w_full().child(row);
        for (i, child) in s.children.into_iter().enumerate() {
            let child_id: ElementId = indexed_child(&id, "child-", i);
            let mut r = compact_session_row(child_id, child).density(density).actions(self.actions.clone());
            r.nested = true;
            if let Some(h) = self.on_select.clone() {
                r = r.on_select(move |k, w, cx| h(k, w, cx));
            }
            if let Some(h) = self.on_action.clone() {
                r = r.on_action(move |k, a, w, cx| h(k, a, w, cx));
            }
            if let Some(h) = self.on_hover.clone() {
                r = r.on_hover(move |k, hovered, w, cx| h(k, hovered, w, cx));
            }
            if let Some(h) = self.on_hover_bounds.clone() {
                r = r.on_hover_bounds(move |k, hovered, bounds, w, cx| h(k, hovered, bounds, w, cx));
            }
            col = col.child(r);
        }
        wrap_hover_bounds(col, &bounds_cell).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aui_tokens::{Palette, ThemeKind};

    fn summary(id: &str) -> SessionSummary {
        SessionSummary::new(id, id, aui_tokens::AgentState::Idle, "1h")
    }

    /// The four second-line states, as the gallery's sidebar card shows them:
    /// title only, placeholder, one-line preview, two-line byline.
    fn four_states() -> [SessionSummary; 4] {
        [
            summary("blank"),
            summary("titling").placeholder("Working…"),
            summary("preview").preview("Fixed the flaky checkout test"),
            summary("byline").byline("tighten address validation", "patched the validator"),
        ]
    }

    /// Every state reserves exactly one second line, so each row is its title
    /// plus this line — two lines tall, dots on their rhythm — whether the
    /// caller passes `Some(text)`, `None`, or empty.
    #[test]
    fn every_state_reserves_exactly_one_second_line() {
        let kinds: Vec<SecondLineKind> = four_states().iter().map(second_line_kind).collect();
        assert_eq!(
            kinds,
            vec![SecondLineKind::Empty, SecondLineKind::Placeholder, SecondLineKind::Preview, SecondLineKind::Byline],
            "each state maps to its own kind"
        );
        for kind in kinds {
            assert_eq!(kind.lines(), 1, "{kind:?} keeps one second line");
        }
        // `None`, empty and blank all read as empty space, never a collapsed row.
        assert_eq!(second_line_kind(&summary("none")), SecondLineKind::Empty);
        assert_eq!(second_line_kind(&summary("empty").preview("")), SecondLineKind::Preview);
        assert_eq!(second_line_kind(&summary("empty2").placeholder("")), SecondLineKind::Placeholder);
        assert_eq!(second_line_kind(&summary("empty3").byline("", "")), SecondLineKind::Byline);
    }

    /// Legacy callers that pass only the meta preview keep their line: tags,
    /// the pin, and the activity sentence all still count as a second line.
    #[test]
    fn legacy_meta_tags_keep_their_second_line() {
        assert_eq!(second_line_kind(&summary("repo").repo("acme-web")), SecondLineKind::Meta);
        assert_eq!(
            second_line_kind(&summary("meta").meta(MetaItem::Text("PR #2491 open".into()))),
            SecondLineKind::Meta
        );
        assert_eq!(
            second_line_kind(&summary("activity").activity(ActivityKind::Working, "running tests")),
            SecondLineKind::Meta
        );
        assert_eq!(second_line_kind(&summary("pinned").pinned()), SecondLineKind::Meta);
        // An explicit byline wins the slot over legacy tags.
        assert_eq!(
            second_line_kind(&summary("both").meta(MetaItem::Text("PR #2491 open".into())).preview("new words")),
            SecondLineKind::Preview
        );
    }

    /// The line ellipsizes at the row's width and never wraps onto another
    /// line — in every state, and for a byline far longer than any sidebar
    /// width the round-5 work supports (240–520 px).
    #[test]
    fn the_second_line_truncates_and_never_wraps() {
        let long = "tighten address validation and add coverage for non-US postal codes across every checkout form";
        let states = [
            summary("blank"),
            summary("titling").placeholder(format!("{long}…")),
            summary("preview").preview(long),
            summary("byline").byline(long, long),
        ];
        for s in &states {
            let kind = second_line_kind(s);
            assert!(kind.truncate(), "{kind:?} ellipsizes at the row's width");
            assert!(!kind.wraps(), "{kind:?} never wraps to a third line");
        }
    }

    /// The placeholder reads dimmer than ordinary preview text, in both
    /// themes: ink-4 against the second line's usual ink-3.
    #[test]
    fn placeholder_is_dimmer_than_preview_in_both_themes() {
        for theme in [ThemeKind::Dark, ThemeKind::Light] {
            let p: Palette = Palette::for_kind(theme);
            assert_eq!(SecondLineKind::Placeholder.ink(&p), p.ink_4, "placeholder ink in {theme:?}");
            assert_eq!(SecondLineKind::Preview.ink(&p), p.ink_3, "preview ink in {theme:?}");
            assert_eq!(SecondLineKind::Byline.ink(&p), p.ink_3, "byline result ink in {theme:?}");
            assert_ne!(
                SecondLineKind::Placeholder.ink(&p),
                SecondLineKind::Preview.ink(&p),
                "placeholder differs from preview in {theme:?}"
            );
        }
    }

    /// Option B's context priority: attention first, then the byline the
    /// owner kept, then preview, then `project · branch`, then legacy meta,
    /// else empty space that still holds the line.
    #[test]
    fn context_line_priority_is_attention_byline_preview_project_meta_empty() {
        assert_eq!(context_line_kind(&summary("none")), ContextLineKind::Empty);
        assert_eq!(
            context_line_kind(&summary("meta").meta(MetaItem::Text("PR #2491 open".into()))),
            ContextLineKind::Meta
        );
        assert_eq!(context_line_kind(&summary("proj").repo("acme-web").branch("main")), ContextLineKind::Project);
        assert_eq!(context_line_kind(&summary("proj-only").branch("main")), ContextLineKind::Project);
        assert_eq!(context_line_kind(&summary("prev").preview("Fixed it")), ContextLineKind::Preview);
        assert_eq!(context_line_kind(&summary("ph").placeholder("Working…")), ContextLineKind::Placeholder);
        assert_eq!(context_line_kind(&summary("bl").byline("ask", "result")), ContextLineKind::Byline);
        // Attention wins over everything on the context line.
        assert_eq!(
            context_line_kind(&summary("att").byline("ask", "result").preview("x").repo("p").attention("sudo apt install notifierd")),
            ContextLineKind::Attention
        );
        // The byline survives: TwoLines still wins over preview-shaped and
        // project-shaped fallbacks.
        assert_eq!(
            context_line_kind(&summary("both").preview("new words").byline("ask", "result")),
            ContextLineKind::Byline
        );
        assert_eq!(
            context_line_kind(&summary("bp").repo("acme-web").byline("ask", "result")),
            ContextLineKind::Byline
        );
        // `project · branch` joins with a middot.
        assert_eq!(project_branch_text(&summary("none")), None);
        assert_eq!(
            project_branch_text(&summary("pb").repo("acme-web").branch("feature/checkout-flow-v2")),
            Some("acme-web · feature/checkout-flow-v2".into())
        );
    }

    /// Option B's status vocabulary: the caller supplies the variable words,
    /// the library owns the sentence, colour and weight.
    #[test]
    fn status_vocabulary_and_colours_come_from_tokens() {
        use crate::nav::{RowStatus, RowStatusKind};
        for theme in [ThemeKind::Dark, ThemeKind::Light] {
            let p: Palette = Palette::for_kind(theme);
            let text = |kind: RowStatusKind, detail: &str| RowStatus::new(kind, detail).text();
            assert_eq!(text(RowStatusKind::Working, "14m"), "Working · 14m");
            assert_eq!(text(RowStatusKind::NeedsApproval, ""), "Needs approval");
            assert_eq!(text(RowStatusKind::Asked, "Which bucket for staging?"), "Asked: \"Which bucket for staging?\"");
            assert_eq!(text(RowStatusKind::Settled, "12m · 5 turns"), "Settled · 12m · 5 turns");
            assert_eq!(text(RowStatusKind::Failed, "1h"), "Failed · 1h");
            assert_eq!(text(RowStatusKind::NoReply, ""), "No reply yet");
            assert_eq!(text(RowStatusKind::NoReply, "2d"), "No reply yet · 2d");
            assert_eq!(RowStatus::new(RowStatusKind::Working, "14m").ink(&p), p.accent_ink, "working ink in {theme:?}");
            assert_eq!(
                RowStatus::new(RowStatusKind::NeedsApproval, "").ink(&p),
                p.warning,
                "approval ink in {theme:?}"
            );
            assert_eq!(RowStatus::new(RowStatusKind::Asked, "q").ink(&p), p.warning, "asked ink in {theme:?}");
            assert_eq!(RowStatus::new(RowStatusKind::Settled, "x").ink(&p), p.ink_3, "settled ink in {theme:?}");
            assert_eq!(RowStatus::new(RowStatusKind::Failed, "x").ink(&p), p.danger, "failed ink in {theme:?}");
            assert_eq!(RowStatus::new(RowStatusKind::NoReply, "").ink(&p), p.ink_3, "no-reply ink in {theme:?}");
        }
    }

    /// Every row state the sidebar shows: working, waiting on approval,
    /// waiting with a question, settled, failed, quiet, and blank.
    fn all_states() -> Vec<SessionSummary> {
        use crate::nav::RowStatusKind;
        vec![
            summary("working").repo("acme-web").branch("feature/checkout-flow-v2").status(RowStatusKind::Working, "14m"),
            summary("approval").attention("sudo apt install notifierd").status(RowStatusKind::NeedsApproval, ""),
            summary("asked").preview("Refresh the session tokens").status(RowStatusKind::Asked, "Which bucket for staging?"),
            summary("settled")
                .byline("Draft the cart recovery email", "Drafted three variants")
                .status(RowStatusKind::Settled, "12m · 5 turns"),
            summary("failed").byline("Add the observability tiles", "2 tests failed").status(RowStatusKind::Failed, "1h"),
            summary("empty").repo("acme-internal").branch("fix/webhook-retry").status(RowStatusKind::NoReply, "2d"),
            summary("blank"),
        ]
    }

    /// A host view rendering one compact row: rows carry interaction state
    /// (which reads the current view), so layout tests draw through an
    /// entity — the same shape an app uses — instead of a bare element.
    struct RowProbe {
        summary: SessionSummary,
        density: RowDensity,
        selected: bool,
        width: f32,
    }

    impl gpui::Render for RowProbe {
        fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
            gpui::div().w(gpui::px(self.width)).debug_selector(|| "probe".into()).child(
                compact_session_row("probe-row", self.summary.clone()).density(self.density).selected(self.selected),
            )
        }
    }

    /// Draws one compact row at `width` px through the real render path, in
    /// a host view.
    fn draw_compact_row(
        cx: &mut gpui::VisualTestContext,
        summary: SessionSummary,
        density: RowDensity,
        selected: bool,
        width: f32,
    ) {
        cx.draw(
            gpui::point(gpui::px(0.0), gpui::px(0.0)),
            gpui::size(gpui::px(width + 100.0), gpui::px(400.0)),
            |_, cx| cx.new(|_| RowProbe { summary, density, selected, width }).into_any_element(),
        );
    }

    /// Draws one compact row at `width` px through the real render path and
    /// returns the laid-out row height: the probe wrapper hugs the row, so
    /// its height is the row's height.
    fn compact_row_laid_out_height(
        cx: &mut gpui::VisualTestContext,
        summary: SessionSummary,
        density: RowDensity,
        width: f32,
    ) -> f32 {
        draw_compact_row(cx, summary, density, false, width);
        cx.debug_bounds("probe").map(|bounds| f32::from(bounds.size.height)).expect("the probe row is painted")
    }

    /// One-density rows share one laid-out height in every state: all seven
    /// summaries drawn at a 260 px sidebar width measure the same — the
    /// status verb line is gone, so each row is its title line.
    #[gpui::test]
    fn one_density_rows_share_one_laid_out_height_in_every_state(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let cx = cx.add_empty_window();
        let heights: Vec<f32> =
            all_states().into_iter().map(|s| compact_row_laid_out_height(cx, s, RowDensity::One, 260.0)).collect();
        assert_eq!(heights.len(), 7, "every state draws");
        for height in &heights {
            assert!((height - heights[0]).abs() < 0.5, "One-density laid-out heights differ: {heights:?}");
        }
    }

    /// Two-density rows share one laid-out height in every state: all seven
    /// summaries drawn at a 260 px sidebar width measure the same — title
    /// plus the one context line, blank space included.
    #[gpui::test]
    fn two_density_rows_share_one_laid_out_height_in_every_state(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let cx = cx.add_empty_window();
        let heights: Vec<f32> =
            all_states().into_iter().map(|s| compact_row_laid_out_height(cx, s, RowDensity::Two, 260.0)).collect();
        assert_eq!(heights.len(), 7, "every state draws");
        for height in &heights {
            assert!((height - heights[0]).abs() < 0.5, "Two-density laid-out heights differ: {heights:?}");
        }
    }

    /// On a narrow sidebar the trailing state text caps at
    /// [`SR_TRAILING_MAX`] of the title line with an ellipsis: a 200 px
    /// waiting row lays "Needs approval" out within the cap while the title
    /// keeps the rest of the line.
    #[gpui::test]
    fn trailing_state_text_caps_so_the_title_keeps_a_narrow_row(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let cx = cx.add_empty_window();
        let waiting = SessionSummary::new(
            "w",
            "a long session title that needs room to read",
            aui_tokens::AgentState::Waiting,
            "3h",
        );
        draw_compact_row(cx, waiting, RowDensity::One, false, 200.0);
        let row = cx.debug_bounds("probe").expect("the probe row is painted");
        let title = cx.debug_bounds("compact-title").expect("the title is painted");
        let trailing = cx.debug_bounds("compact-trailing").expect("the trailing slot is painted");
        let row_w = f32::from(row.size.width);
        let trailing_w = f32::from(trailing.size.width);
        let title_w = f32::from(title.size.width);
        assert!(
            trailing_w <= row_w * SR_TRAILING_MAX + 1.0,
            "trailing {trailing_w} exceeds the {SR_TRAILING_MAX} cap of row {row_w}"
        );
        assert!(title_w >= trailing_w, "the title {title_w} keeps at least as much as the trailing slot {trailing_w}");
    }

    /// The trailing slot carries the age, or the state verb in the state
    /// colour: "Needs approval" when waiting, "Failed" when failed.
    /// Failure wins when both apply; the age reads muted.
    #[test]
    fn trailing_slot_carries_age_or_state() {
        use crate::nav::RowStatusKind;
        use aui_tokens::AgentState;
        // The idle row reads its own age, muted.
        let idle = summary("idle");
        assert_eq!(row_trailing_kind(&idle), RowTrailing::Age);
        assert_eq!(RowTrailing::Age.text(&idle), SharedString::from("1h"));
        // Waiting (by state or by status) reads "Needs approval".
        let waiting = SessionSummary::new("w", "w", AgentState::Waiting, "3h");
        assert_eq!(row_trailing_kind(&waiting), RowTrailing::NeedsApproval);
        assert_eq!(row_trailing_kind(&summary("asked").status(RowStatusKind::Asked, "q?")), RowTrailing::NeedsApproval);
        assert_eq!(row_trailing_kind(&summary("appr").status(RowStatusKind::NeedsApproval, "")), RowTrailing::NeedsApproval);
        assert_eq!(RowTrailing::NeedsApproval.text(&waiting), SharedString::from("Needs approval"));
        // Failed (by state or by status) reads "Failed", even when waiting.
        let failed = SessionSummary::new("f", "f", AgentState::Failed, "1d");
        assert_eq!(row_trailing_kind(&failed), RowTrailing::Failed);
        assert_eq!(row_trailing_kind(&summary("fs").status(RowStatusKind::Failed, "1h")), RowTrailing::Failed);
        assert_eq!(
            row_trailing_kind(&SessionSummary::new("fw", "fw", AgentState::Waiting, "1h").status(RowStatusKind::Failed, "x")),
            RowTrailing::Failed,
            "failure wins over waiting"
        );
        assert_eq!(RowTrailing::Failed.text(&failed), SharedString::from("Failed"));
        // Ink: muted age, warning wait, danger failure — in both themes.
        for theme in [ThemeKind::Dark, ThemeKind::Light] {
            let p: Palette = Palette::for_kind(theme);
            assert_eq!(RowTrailing::Age.ink(&p), p.ink_3, "age ink in {theme:?}");
            assert_eq!(RowTrailing::NeedsApproval.ink(&p), p.warning, "waiting ink in {theme:?}");
            assert_eq!(RowTrailing::Failed.ink(&p), p.danger, "failed ink in {theme:?}");
        }
    }

    /// The selected row renders on surface-3 — a step above the surface-2 a
    /// hovered row rests on — with a full-ink title: both rows go through the
    /// real render path in a window (so the test proves the path wires the
    /// states, not just the helpers), and the fills the path resolves for
    /// the two states differ, in both themes.
    #[gpui::test]
    fn selected_row_renders_a_fill_above_the_hover_fill(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let cx = cx.add_empty_window();
        // Both states through the render path: each paints, at the same row
        // height — selection changes the fill, never the layout.
        let plain_h = compact_row_laid_out_height(cx, summary("sel"), RowDensity::Two, 260.0);
        draw_compact_row(cx, summary("sel"), RowDensity::Two, true, 260.0);
        let selected_h =
            cx.debug_bounds("probe").map(|bounds| f32::from(bounds.size.height)).expect("the selected row is painted");
        assert!((selected_h - plain_h).abs() < 0.5, "selection keeps the laid-out height: {plain_h} vs {selected_h}");
        // The fills the render path resolves for selected vs rest differ.
        for theme in [ThemeKind::Dark, ThemeKind::Light] {
            let p: Palette = Palette::for_kind(theme);
            assert_eq!(compact_row_ground(true, &p), p.surface_3, "selected ground in {theme:?}");
            assert_eq!(compact_row_ground(false, &p), p.surface_2, "hover ground in {theme:?}");
            assert_ne!(compact_row_ground(true, &p), compact_row_ground(false, &p), "active differs from hover in {theme:?}");
            assert_eq!(compact_row_title_ink(true, &p), p.ink, "selected title ink in {theme:?}");
            assert_eq!(compact_row_title_ink(false, &p), p.ink_2, "resting title ink in {theme:?}");
        }
    }

    /// New callers get Two density without opting in.
    #[test]
    fn new_compact_rows_default_to_two_density() {
        assert_eq!(RowDensity::default(), RowDensity::Two);
    }

    /// The context line keeps its line in every state — title plus context,
    /// each truncating, none wrapping — including the empty states that hold
    /// their line as blank space. (Laid-out heights for both densities are
    /// covered by the `*_share_one_laid_out_height_in_every_state` tests.)
    #[test]
    fn context_line_truncates_and_never_wraps_in_every_state() {
        for s in &all_states() {
            let context = context_line_kind(s);
            assert_eq!(context.lines(), 1, "{context:?} keeps one context line");
            assert!(context.truncate(), "{context:?} ellipsizes at the row's width");
            assert!(!context.wraps(), "{context:?} never wraps");
        }
    }
}
