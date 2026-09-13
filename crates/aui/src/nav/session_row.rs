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
use gpui::{div, prelude::*, px, AnyElement, App, Div, ElementId, Entity, IntoElement, SharedString, Window};
use gpui_kit::base::input::TextareaState;
use gpui_kit::component::input::Textarea;
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, spinner, status_dot, tag, ButtonSize};
use crate::nav::{ActivityKind, MetaItem, SessionSummary};
use crate::util::{indexed_child, interaction_flags, TrackInteraction};

/// `.wt{grid-template-columns:14px 1fr auto;gap:2px 8px;padding:9px 10px;margin:2px 8px}`.
/// Inside a flex column (the sidebar) CSS keeps both margins, 4 px apart;
/// inside block flow (card 20's list) they collapse to 2 px — see
/// [`SessionRow::collapse_margins`].
const DOT_COL: f32 = 14.0;
const COL_GAP: f32 = 8.0;
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
/// `.sr{min-height:30px;padding:4px 10px 4px 12px;margin:0 8px;gap:2px 8px;font-size:12.5px}`.
const SR_PAD_Y: f32 = 4.0;
const SR_PAD_LEFT: f32 = 12.0;
const SR_PAD_RIGHT: f32 = 10.0;
const SR_MARGIN_X: f32 = 8.0;
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
    nested: bool,
    actions: Vec<RowAction>,
    editor: Option<AnyElement>,
    on_select: Option<SelectHandler>,
    on_action: Option<ActionHandler>,
}

/// A compact row for `session`; children nest beneath with a hairline rail.
pub fn compact_session_row(id: impl Into<ElementId>, session: SessionSummary) -> CompactSessionRow {
    CompactSessionRow {
        id: id.into(),
        session,
        selected: false,
        nested: false,
        actions: Vec::new(),
        editor: None,
        on_select: None,
        on_action: None,
    }
}

impl CompactSessionRow {
    /// Selected: surface-3 ground and ink text.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
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
}

impl RenderOnce for CompactSessionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let s = self.session;
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let tint = if self.selected { p.surface_3 } else { p.surface_2 };
        let bg = tint_fade((id.clone(), "bg"), self.selected || flags.hovered, tint, Tween::FAST, window, cx);
        let text = if self.selected { p.ink } else { p.ink_2 };

        let editing = self.editor.is_some();
        let acts_visible = flags.hovered && !self.actions.is_empty() && !editing;
        let mut lines = v_flex().flex_1().min_w(px(0.0)).gap(px(ROW_GAP));
        let title: AnyElement = match self.editor {
            Some(editor) => div().flex_1().min_w(px(0.0)).child(editor).into_any_element(),
            None => div().flex_1().min_w(px(0.0)).medium().truncate().child(s.name.clone()).into_any_element(),
        };
        lines = lines.child(
            h_flex()
                .w_full()
                .gap(px(COL_GAP))
                .child(title)
                .child(div().flex_none().text_role(TextRole::MonoSmall).font_weight(gpui::FontWeight::MEDIUM).text_color(p.ink_3).child(s.elapsed.clone())),
        );
        let mut items = Vec::new();
        if let Some(activity) = &s.activity {
            let color = match activity.kind {
                ActivityKind::Working | ActivityKind::Plain => p.ink_3,
                ActivityKind::Waiting => p.warning,
                ActivityKind::Failed => p.danger,
            };
            if activity.kind == ActivityKind::Working {
                items.push(spinner((id.clone(), "activity-spinner")).size(px(ACTIVITY_SPINNER)).into_any_element());
            }
            items.push(div().min_w(px(0.0)).truncate().text_color(color).child(activity.text.clone()).into_any_element());
        }
        let mut meta = meta_items(&p, &s, BRANCH_MAX);
        meta.append(&mut items);
        if s.pinned {
            meta.insert(0, pin_mark(&p));
        }
        if !meta.is_empty() {
            lines = lines.child(meta_line(&p, SR_META_TEXT, meta));
        }

        // No `w_full`: at full width the 8 px margins overflow the column and
        // the shell clips them, leaving ~0 px on the right. As a flex item the
        // row stretches to the column minus its margins, so both gutters stay 8.
        let mut row = h_flex()
            .id(id.clone())
            .relative()
            .flex_1()
            .min_w(px(0.0))
            .min_h(cx.aui().metrics.row)
            .items_center()
            .gap(px(COL_GAP))
            .py(px(SR_PAD_Y))
            .pl(px(if self.nested { SR_CHILD_PAD } else { SR_PAD_LEFT }))
            .pr(px(SR_PAD_RIGHT))
            .ml(px(if self.nested { SR_CHILD_INDENT } else { SR_MARGIN_X }))
            .mr(px(SR_MARGIN_X))
            .rounded(px(scale::R_MD))
            .bg(bg)
            .ui(SR_TEXT)
            .text_color(text)
            .cursor_pointer()
            .track_interaction(&state)
            .child(div().flex_none().w(px(DOT_COL)).flex().items_center().child(status_dot((id.clone(), "dot"), s.state).pulse(s.pulse)))
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
            return row.into_any_element();
        }
        let mut col = v_flex().w_full().child(row);
        for (i, child) in s.children.into_iter().enumerate() {
            let child_id: ElementId = indexed_child(&id, "child-", i);
            let mut r = compact_session_row(child_id, child).actions(self.actions.clone());
            r.nested = true;
            if let Some(h) = self.on_select.clone() {
                r = r.on_select(move |k, w, cx| h(k, w, cx));
            }
            if let Some(h) = self.on_action.clone() {
                r = r.on_action(move |k, a, w, cx| h(k, a, w, cx));
            }
            col = col.child(r);
        }
        col.into_any_element()
    }
}
