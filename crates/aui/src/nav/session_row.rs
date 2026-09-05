//! `.wt` — the worktree / session row in every state — and `.sr`, the
//! compact one-line row used by the project and date groupings.

use aui_icons::{icon, provider_mark, IconName};
use aui_motion::{tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, Div, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, spinner, status_dot, tag, ButtonSize};
use crate::nav::{ActivityKind, MetaItem, SessionSummary};
use crate::util::{interaction_flags, TrackInteraction};

/// `.wt{grid-template-columns:14px 1fr auto;gap:2px 8px;padding:9px 10px;margin:2px 8px}`.
/// CSS collapses the vertical margins between rows to 2 px; gpui does not,
/// so rows carry only a top margin.
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
    /// More…
    More,
}

impl RowAction {
    const ALL: [RowAction; 4] = [RowAction::Terminal, RowAction::Browser, RowAction::Pin, RowAction::More];

    fn glyph(self) -> IconName {
        match self {
            RowAction::Terminal => IconName::Terminal,
            RowAction::Browser => IconName::Globe,
            RowAction::Pin => IconName::Pin,
            RowAction::More => IconName::Dots,
        }
    }

    fn name(self) -> &'static str {
        match self {
            RowAction::Terminal => "terminal",
            RowAction::Browser => "browser",
            RowAction::Pin => "pin",
            RowAction::More => "more",
        }
    }
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

        let rest_bg = if self.selected { p.surface_3 } else { gpui::transparent_black() };
        let bg = tween((id.clone(), "bg"), if hovered && !self.selected { p.surface_2 } else { rest_bg }, Tween::FAST, window, cx);
        let acts_visible = hovered && self.show_actions;
        let acts_opacity = tween((id.clone(), "acts-opacity"), if acts_visible { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);
        let acts_slide = tween((id.clone(), "acts-slide"), if acts_visible { 0.0f32 } else { ACTS_SLIDE }, Tween::BASE.with_easing(aui_tokens::Easing::OUT), window, cx);
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
        let items = meta_items(&p, &s, self.branch_max);
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
            let mut tray = h_flex()
                .absolute()
                .right(px(ACTS_RIGHT) - px(acts_slide))
                .top(px(ACTS_TOP))
                .gap(px(ACTS_GAP))
                .p(px(ACTS_PAD))
                .rounded(px(scale::R_SM))
                .bg(p.surface_2)
                .opacity(acts_opacity);
            for action in RowAction::ALL {
                let mut b = icon_button((id.clone(), action.name()), action.glyph()).ghost().size(ButtonSize::Xs).icon_size(px(ACTS_GLYPH));
                if let Some(on_action) = self.on_action.clone() {
                    let key = s.id.clone();
                    b = b.on_click(move |_, w, cx| on_action(&key, action, w, cx));
                }
                tray = tray.child(b);
            }
            if !acts_visible && acts_opacity <= 0.001 {
                tray = tray.invisible();
            }
            row = row.child(tray);
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
            let child_id: ElementId = (id.clone(), SharedString::from(format!("child-{i}"))).into();
            let mut r = session_row(child_id, child)
                .nested()
                .row_gap(self.row_gap)
                .text_sizes(self.name_size, self.meta_size)
                .branch_max(self.branch_max)
                .show_actions(self.show_actions);
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
    on_select: Option<SelectHandler>,
}

/// A compact row for `session`; children nest beneath with a hairline rail.
pub fn compact_session_row(id: impl Into<ElementId>, session: SessionSummary) -> CompactSessionRow {
    CompactSessionRow { id: id.into(), session, selected: false, nested: false, on_select: None }
}

impl CompactSessionRow {
    /// Selected: surface-3 ground and ink text.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Row click.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for CompactSessionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let s = self.session;
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let rest_bg = if self.selected { p.surface_3 } else { gpui::transparent_black() };
        let bg = tween((id.clone(), "bg"), if flags.hovered && !self.selected { p.surface_2 } else { rest_bg }, Tween::FAST, window, cx);
        let text = if self.selected { p.ink } else { p.ink_2 };

        let mut lines = v_flex().flex_1().min_w(px(0.0)).gap(px(ROW_GAP));
        lines = lines.child(
            h_flex()
                .w_full()
                .gap(px(COL_GAP))
                .child(div().flex_1().min_w(px(0.0)).medium().truncate().child(s.name.clone()))
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
        if !meta.is_empty() {
            lines = lines.child(meta_line(&p, SR_META_TEXT, meta));
        }

        let mut row = h_flex()
            .id(id.clone())
            .relative()
            .w_full()
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
        if let Some(on_select) = self.on_select.clone() {
            let key = s.id.clone();
            row = row.on_click(move |_, w, cx| on_select(&key, w, cx));
        }
        if s.children.is_empty() {
            return row.into_any_element();
        }
        let mut col = v_flex().w_full().child(row);
        for (i, child) in s.children.into_iter().enumerate() {
            let child_id: ElementId = (id.clone(), SharedString::from(format!("child-{i}"))).into();
            let mut r = compact_session_row(child_id, child);
            r.nested = true;
            if let Some(h) = self.on_select.clone() {
                r = r.on_select(move |k, w, cx| h(k, w, cx));
            }
            col = col.child(r);
        }
        col.into_any_element()
    }
}
