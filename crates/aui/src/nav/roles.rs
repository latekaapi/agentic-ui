//! Card 22: the assistant's bordered role sections — role header, project
//! rows, session rows with a rail, the knowledge card.

use std::rc::Rc;

use aui_icons::{icon, IconName, RoleIcon};
use aui_motion::{collapse, tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{chip, tag};
use crate::nav::chevron;
use crate::util::{interaction_flags, TrackInteraction};

/// `.role{height:44px;padding:0 12px 0 10px;gap:8px}`.
const ROLE_PAD_LEFT: f32 = 10.0;
const ROLE_PAD_RIGHT: f32 = 12.0;
const ROLE_GAP: f32 = 8.0;
/// `.gh{height:24px;padding:0 4px 0 12px;gap:8px}` with `.gh .rule{height:1px}`.
const GROUP_H: f32 = 24.0;
const GROUP_PAD_LEFT: f32 = 12.0;
const GROUP_PAD_RIGHT: f32 = 4.0;
const GROUP_GAP: f32 = 8.0;
const RULE_H: f32 = 1.0;
/// `.sub{padding:2px 8px 6px 8px}`.
const SUB_PAD_TOP: f32 = 2.0;
const SUB_PAD_X: f32 = 8.0;
const SUB_PAD_BOTTOM: f32 = 6.0;
/// `.proj{height:28px;padding:0 8px 0 22px;gap:8px;font-size:12.5px}` and
/// `.sess{height:28px;padding:0 8px 0 40px;gap:8px;font-size:12.5px}`.
const ROW_H: f32 = 28.0;
const ROW_GAP: f32 = 8.0;
const ROW_PAD_RIGHT: f32 = 8.0;
const PROJECT_INDENT: f32 = 22.0;
const SESSION_INDENT: f32 = 40.0;
const ROW_TEXT: f32 = 12.5;
/// `.sess::before{left:29px;top:0;bottom:0;width:1px}` — the hairline rail
/// that runs behind a project's sessions.
const RAIL_LEFT: f32 = 29.0;
const RAIL_W: f32 = 1.0;
/// `.kb{margin:4px 8px 0;padding:10px 10px}` with `.kb .row{gap:6px}`.
const KB_MARGIN_TOP: f32 = 4.0;
const KB_MARGIN_X: f32 = 8.0;
const KB_PAD: f32 = 10.0;
const KB_GAP: f32 = 6.0;
/// The `<div style="height:10px">` that closes an open section.
const SECTION_TAIL: f32 = 10.0;

/// Handler for an intent that carries the id of the row it came from.
pub type RoleIntent = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// What kind of work a session is, which fixes its glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// A conversation (`#sparkle`).
    Chat,
    /// A document being written (`#doc`).
    Document,
    /// A spreadsheet (`#sheet`).
    Sheet,
}

impl SessionKind {
    /// The glyph for this kind.
    pub fn icon(self) -> IconName {
        match self {
            SessionKind::Chat => IconName::Sparkle,
            SessionKind::Document => IconName::Doc,
            SessionKind::Sheet => IconName::Sheet,
        }
    }
}

/// One session inside a project (`.sess`).
#[derive(Debug, Clone)]
pub struct RoleSession {
    /// Stable id, used by `on_select_session` and to mark the active row.
    pub id: SharedString,
    /// The session's name.
    pub name: SharedString,
    /// The kind of work, which picks the glyph.
    pub kind: SessionKind,
    /// Elapsed time (`now`, `2h`, `1d`).
    pub elapsed: SharedString,
}

impl RoleSession {
    /// A session of `kind` last touched `elapsed` ago.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, kind: SessionKind, elapsed: impl Into<SharedString>) -> Self {
        Self { id: id.into(), name: name.into(), kind, elapsed: elapsed.into() }
    }
}

/// One project inside a role (`.proj` plus the `.sess` rows under it).
#[derive(Debug, Clone)]
pub struct Project {
    /// Stable id, used by `on_select_project`.
    pub id: SharedString,
    /// The project's name.
    pub name: SharedString,
    /// How many sessions the project holds (shown at the right).
    pub count: usize,
    /// The sessions listed under the project; may be empty.
    pub sessions: Vec<RoleSession>,
}

impl Project {
    /// A project with `count` sessions and no expanded children.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, count: usize) -> Self {
        Self { id: id.into(), name: name.into(), count, sessions: Vec::new() }
    }

    /// Adds a session row under the project.
    pub fn session(mut self, session: RoleSession) -> Self {
        self.sessions.push(session);
        self
    }
}

/// One role of the assistant (`.sec`): the person's hat, its projects and the
/// knowledge sources the assistant is grounded in while wearing it.
#[derive(Debug, Clone)]
pub struct Role {
    /// Stable id, used by `on_toggle_role` and `on_add_knowledge`.
    pub id: SharedString,
    /// The role's name (`Director, Education`).
    pub name: SharedString,
    /// The muted glyph in front of the name.
    pub icon: RoleIcon,
    /// Whether the section is expanded.
    pub open: bool,
    /// The count shown on a closed role.
    pub count: usize,
    /// The projects of the role.
    pub projects: Vec<Project>,
    /// The knowledge sources attached to the role.
    pub knowledge: Vec<SharedString>,
}

impl Role {
    /// A closed role with `count` sessions.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, glyph: RoleIcon, count: usize) -> Self {
        Self { id: id.into(), name: name.into(), icon: glyph, open: false, count, projects: Vec::new(), knowledge: Vec::new() }
    }

    /// Expands the section.
    pub fn open(mut self) -> Self {
        self.open = true;
        self
    }

    /// Adds a project.
    pub fn project(mut self, project: Project) -> Self {
        self.projects.push(project);
        self
    }

    /// Adds a knowledge source.
    pub fn knowledge(mut self, source: impl Into<SharedString>) -> Self {
        self.knowledge.push(source.into());
        self
    }
}

/// A project row (`.proj`). Build with [`project_row`].
#[derive(IntoElement)]
pub struct ProjectRow {
    id: ElementId,
    project_id: SharedString,
    name: SharedString,
    count: usize,
    on_select: Option<RoleIntent>,
}

/// A project row: folder glyph, name, session count.
pub fn project_row(id: impl Into<ElementId>, project: &Project) -> ProjectRow {
    ProjectRow { id: id.into(), project_id: project.id.clone(), name: project.name.clone(), count: project.count, on_select: None }
}

impl ProjectRow {
    /// Selection intent, carrying the project id.
    pub fn on_select(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ProjectRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        // `.proj` has no hover rule in the card; the row is clickable, so it
        // takes the same quiet surface-2 tint as `.sess:hover`.
        let bg = tint_fade((id.clone(), "bg"), flags.hovered, p.surface_2, Tween::FAST, window, cx);
        let mut row = h_flex()
            .id(id)
            .w_full()
            .h(px(ROW_H))
            .flex_none()
            .gap(px(ROW_GAP))
            .pl(px(PROJECT_INDENT))
            .pr(px(ROW_PAD_RIGHT))
            .rounded(px(scale::R_SM))
            .bg(bg)
            .ui(ROW_TEXT)
            .medium()
            .text_color(p.ink_2)
            .cursor_pointer()
            .track_interaction(&state)
            .child(icon(IconName::Folder))
            .child(div().min_w(px(0.0)).truncate().child(self.name))
            .child(div().flex_1())
            .child(
                div()
                    .flex_none()
                    .text_role(TextRole::MonoSmall)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(p.ink_3)
                    .child(SharedString::from(self.count.to_string())),
            );
        if let Some(on_select) = self.on_select {
            let key = self.project_id.clone();
            row = row.on_click(move |_, w, cx| on_select(&key, w, cx));
        }
        row
    }
}

/// A session row under a project (`.sess`). Build with [`role_session_row`].
#[derive(IntoElement)]
pub struct RoleSessionRow {
    id: ElementId,
    session: RoleSession,
    active: bool,
    on_select: Option<RoleIntent>,
}

/// A session row: the hairline rail, the kind glyph, the name, the elapsed time.
pub fn role_session_row(id: impl Into<ElementId>, session: RoleSession) -> RoleSessionRow {
    RoleSessionRow { id: id.into(), session, active: false, on_select: None }
}

impl RoleSessionRow {
    /// Marks the row as the open session (`.sess.on`).
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Selection intent, carrying the session id.
    pub fn on_select(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for RoleSessionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let s = self.session;
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let tint = if self.active { p.surface_3 } else { p.surface_2 };
        let bg = tint_fade((id.clone(), "bg"), self.active || flags.hovered, tint, Tween::FAST, window, cx);
        let rest_text = if self.active { p.ink } else { p.ink_3 };
        let text = tween((id.clone(), "text"), if flags.hovered { p.ink } else { rest_text }, Tween::FAST, window, cx);
        let mut row = h_flex()
            .id(id)
            .relative()
            .w_full()
            .h(px(ROW_H))
            .flex_none()
            .gap(px(ROW_GAP))
            .pl(px(SESSION_INDENT))
            .pr(px(ROW_PAD_RIGHT))
            .rounded(px(scale::R_SM))
            .bg(bg)
            .ui(ROW_TEXT)
            .text_color(text)
            .cursor_pointer()
            .track_interaction(&state)
            .child(div().absolute().left(px(RAIL_LEFT)).top_0().bottom_0().w(px(RAIL_W)).bg(p.line))
            .child(icon(s.kind.icon()).color(p.ink_3))
            .child(div().min_w(px(0.0)).truncate().child(s.name.clone()))
            .child(div().flex_1())
            .child(
                div()
                    .flex_none()
                    .text_role(TextRole::MonoSmall)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(p.ink_3)
                    .child(s.elapsed.clone()),
            );
        if let Some(on_select) = self.on_select {
            let key = s.id.clone();
            row = row.on_click(move |_, w, cx| on_select(&key, w, cx));
        }
        row
    }
}

/// The knowledge sources of a role, as a bordered card of chips (`.kb`).
/// Build with [`knowledge_card`].
#[derive(IntoElement)]
pub struct KnowledgeCard {
    id: ElementId,
    role_id: SharedString,
    sources: Vec<SharedString>,
    on_add: Option<RoleIntent>,
}

/// A knowledge card for `role_id` listing `sources`.
pub fn knowledge_card(id: impl Into<ElementId>, role_id: impl Into<SharedString>, sources: Vec<SharedString>) -> KnowledgeCard {
    KnowledgeCard { id: id.into(), role_id: role_id.into(), sources, on_add: None }
}

impl KnowledgeCard {
    /// Intent for the `+ add` chip, carrying the role id.
    pub fn on_add(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_add = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for KnowledgeCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut row = div().flex().flex_wrap().gap(px(KB_GAP));
        for (i, source) in self.sources.into_iter().enumerate() {
            row = row.child(chip((id.clone(), SharedString::from(format!("kb-{i}"))), source).icon(IconName::Book));
        }
        let mut add = chip((id.clone(), "add"), "+ add").accent();
        if let Some(on_add) = self.on_add {
            let key = self.role_id.clone();
            add = add.on_click(move |_, w, cx| on_add(&key, w, cx));
        }
        div()
            .id(id)
            .mt(px(KB_MARGIN_TOP))
            .mx(px(KB_MARGIN_X))
            .p(px(KB_PAD))
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .child(row.child(add))
    }
}

/// A bordered role section (`.sec`): the 44 px role header and, when open,
/// the Projects and Knowledge groups. Build with [`role_section`].
#[derive(IntoElement)]
pub struct RoleSection {
    id: ElementId,
    role: Role,
    active_session: Option<SharedString>,
    on_toggle_role: Option<RoleIntent>,
    on_select_project: Option<RoleIntent>,
    on_select_session: Option<RoleIntent>,
    on_add_knowledge: Option<RoleIntent>,
}

/// A role section for `role`.
pub fn role_section(id: impl Into<ElementId>, role: &Role) -> RoleSection {
    RoleSection {
        id: id.into(),
        role: role.clone(),
        active_session: None,
        on_toggle_role: None,
        on_select_project: None,
        on_select_session: None,
        on_add_knowledge: None,
    }
}

impl RoleSection {
    /// The id of the session that is open, highlighted with `.sess.on`.
    pub fn active_session(mut self, id: Option<&str>) -> Self {
        self.active_session = id.map(SharedString::from);
        self
    }

    /// Open / close intent, carrying the role id.
    pub fn on_toggle_role(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_role = Some(Rc::new(f));
        self
    }

    /// Project selection intent, carrying the project id.
    pub fn on_select_project(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select_project = Some(Rc::new(f));
        self
    }

    /// Session selection intent, carrying the session id.
    pub fn on_select_session(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select_session = Some(Rc::new(f));
        self
    }

    /// Add-knowledge intent, carrying the role id.
    pub fn on_add_knowledge(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_add_knowledge = Some(Rc::new(f));
        self
    }

    /// The `.sub` list: every project with its sessions.
    fn projects(&self) -> AnyElement {
        let mut sub = v_flex().w_full().pt(px(SUB_PAD_TOP)).px(px(SUB_PAD_X)).pb(px(SUB_PAD_BOTTOM));
        for (pi, project) in self.role.projects.iter().enumerate() {
            let mut row = project_row((self.id.clone(), SharedString::from(format!("proj-{pi}"))), project);
            if let Some(h) = self.on_select_project.clone() {
                row = row.on_select(move |key, w, cx| h(key, w, cx));
            }
            sub = sub.child(row);
            for (si, session) in project.sessions.iter().enumerate() {
                let active = self.active_session.as_deref() == Some(session.id.as_ref());
                let mut row = role_session_row((self.id.clone(), SharedString::from(format!("sess-{pi}-{si}"))), session.clone()).active(active);
                if let Some(h) = self.on_select_session.clone() {
                    row = row.on_select(move |key, w, cx| h(key, w, cx));
                }
                sub = sub.child(row);
            }
        }
        sub.into_any_element()
    }
}

/// The `.gh` group label: caps label, hairline rule, mono count.
fn group_label(label: &'static str, count: usize, cx: &App) -> impl IntoElement {
    let p = cx.aui().colors;
    h_flex()
        .w_full()
        .h(px(GROUP_H))
        .flex_none()
        .gap(px(GROUP_GAP))
        .pl(px(GROUP_PAD_LEFT))
        .pr(px(GROUP_PAD_RIGHT))
        .child(div().flex_none().text_role(TextRole::Caps).text_color(p.ink_3).child(label.to_uppercase()))
        .child(div().flex_1().h(px(RULE_H)).bg(p.line))
        .child(tag(count.to_string()))
}

impl RenderOnce for RoleSection {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let role = self.role.clone();
        let open = role.open;

        let mut header = h_flex()
            .id((id.clone(), "role"))
            .w_full()
            .h(cx.aui().metrics.header)
            .flex_none()
            .gap(px(ROLE_GAP))
            .pl(px(ROLE_PAD_LEFT))
            .pr(px(ROLE_PAD_RIGHT))
            .ui(scale::FS_13)
            .semibold()
            .text_color(if open { p.ink } else { p.ink_2 })
            .cursor_pointer()
            .child(chevron((id.clone(), "chev"), open, if open { p.ink } else { p.ink_2 }, window, cx))
            .child(icon(role.icon.icon()).color(p.ink_3))
            .child(div().min_w(px(0.0)).truncate().child(role.name.clone()));
        if !open {
            header = header.child(div().flex_1()).child(tag(role.count.to_string()));
        }
        if let Some(h) = self.on_toggle_role.clone() {
            let key = role.id.clone();
            header = header.on_click(move |_, w, cx| h(&key, w, cx));
        }

        let mut body = v_flex().w_full();
        if !role.projects.is_empty() {
            body = body.child(group_label("Projects", role.projects.len(), cx)).child(self.projects());
        }
        if !role.knowledge.is_empty() {
            let mut card = knowledge_card((id.clone(), "kb"), role.id.clone(), role.knowledge.clone());
            if let Some(h) = self.on_add_knowledge.clone() {
                card = card.on_add(move |key, w, cx| h(key, w, cx));
            }
            body = body.child(group_label("Knowledge", role.knowledge.len(), cx)).child(card);
        }
        body = body.child(div().h(px(SECTION_TAIL)).flex_none());

        let (reveal, _) = collapse((id.clone(), "body"), open, body.into_any_element(), window, cx);

        v_flex().w_full().flex_none().border_b_1().border_color(p.line).child(header).child(reveal)
    }
}
