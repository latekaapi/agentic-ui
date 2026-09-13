//! Card 21: the sidebar — header with workspace switcher, primary nav,
//! collapsible groups of session rows, account footer.

use aui_icons::{icon, IconName, Provider};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, ButtonSize};
use crate::nav::{group_header, group_row, nav_item, project_mark, rail, sidebar_footer, session_row, RailItem, SessionSummary};
use crate::util::{interaction_flags, TrackInteraction};

/// `.side{width:256px}`.
pub const SIDEBAR_WIDTH: f32 = 256.0;
/// `.hd{gap:8px;height:44px;padding:0 10px 0 12px}`.
const HEADER_GAP: f32 = 8.0;
const HEADER_PAD_LEFT: f32 = 12.0;
const HEADER_PAD_RIGHT: f32 = 10.0;
/// `.hd .ws{gap:6px;font-weight:600}` with the 18 px project mark in accent
/// and an 11 px chevron.
const WS_GAP: f32 = 6.0;
const WS_MARK: f32 = 18.0;
const WS_CHEVRON: f32 = 11.0;
/// `.nav{padding:8px}`.
const NAV_PAD: f32 = 8.0;
/// `.wt{margin:2px 8px;gap:3px 8px}` and `.wt .nm{font-size:12.5px}`,
/// `.wt .meta{font-size:11.5px}` in this card.
const ROW_MARGIN_X: f32 = 8.0;
const ROW_GAP: f32 = 3.0;
const ROW_NAME: f32 = 12.5;
const ROW_META: f32 = 11.5;
/// The group header's trailing branch: `.mono.subtle{font-size:10px}`.
const TRAILING_TEXT: f32 = 10.0;
/// `.ft{padding:8px 12px}` — the sidebar cards use 8, the shell 10.
const FOOTER_PAD_Y: f32 = 8.0;

/// One primary nav row (`Tasks`, `Automations`, `Inbox`).
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarNavItem {
    /// Stable name, reported by `on_action`.
    pub name: SharedString,
    /// Visible label.
    pub label: SharedString,
    /// The 14 px glyph.
    pub icon: IconName,
    /// The mono count at the right.
    pub count: Option<SharedString>,
    /// Colours the count in warning (the inbox needing the person).
    pub warning: bool,
}

impl SidebarNavItem {
    /// A nav row with no count.
    pub fn new(name: impl Into<SharedString>, label: impl Into<SharedString>, glyph: IconName) -> Self {
        Self { name: name.into(), label: label.into(), icon: glyph, count: None, warning: false }
    }

    /// Sets the mono count.
    pub fn count(mut self, count: impl Into<SharedString>) -> Self {
        self.count = Some(count.into());
        self
    }

    /// Colours the count in warning.
    pub fn warning(mut self) -> Self {
        self.warning = true;
        self
    }
}

/// A collapsible group of sessions.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarGroup {
    /// Stable id, reported by `on_toggle_group`.
    pub id: SharedString,
    /// Header label (`Pinned`, `acme-web`).
    pub label: SharedString,
    /// The mono count after the label.
    pub count: Option<SharedString>,
    /// Open groups show their sessions; closed ones show only the header.
    pub open: bool,
    /// Muted mono text at the far right of the header (the `main` branch).
    pub trailing: Option<SharedString>,
    /// The sessions inside.
    pub sessions: Vec<SessionSummary>,
}

impl SidebarGroup {
    /// An open group with no sessions.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), count: None, open: true, trailing: None, sessions: Vec::new() }
    }

    /// Sets the count.
    pub fn count(mut self, count: impl Into<SharedString>) -> Self {
        self.count = Some(count.into());
        self
    }

    /// Closes the group.
    pub fn closed(mut self) -> Self {
        self.open = false;
        self
    }

    /// Sets the trailing mono text.
    pub fn trailing(mut self, text: impl Into<SharedString>) -> Self {
        self.trailing = Some(text.into());
        self
    }

    /// Adds a session.
    pub fn session(mut self, session: SessionSummary) -> Self {
        self.sessions.push(session);
        self
    }
}

/// The account footer's data.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarAccount {
    /// The avatar's initial.
    pub initial: SharedString,
    /// Name and plan (`Bharani · Max`).
    pub name: SharedString,
    /// The provider whose usage the meter shows.
    pub provider: Provider,
    /// Usage as a fraction in `0..=1`.
    pub usage: f32,
}

impl SidebarAccount {
    /// A footer for `name`.
    pub fn new(initial: impl Into<SharedString>, name: impl Into<SharedString>, provider: Provider, usage: f32) -> Self {
        Self { initial: initial.into(), name: name.into(), provider, usage }
    }
}

/// Everything the sidebar renders. An app derives this from its own state;
/// the sidebar reads nothing else.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarNav {
    /// The workspace shown in the switcher.
    pub workspace: SharedString,
    /// The primary nav rows.
    pub items: Vec<SidebarNavItem>,
    /// The caps label of the group row (`Workspaces`).
    pub groups_label: SharedString,
    /// The collapsible groups, in order.
    pub groups: Vec<SidebarGroup>,
    /// The selected session, highlighted with a surface-3 ground.
    pub selected: Option<SharedString>,
    /// The account footer.
    pub footer: SidebarAccount,
}

impl SidebarNav {
    /// A sidebar for `workspace` with the given footer; add items and groups
    /// with the builder methods.
    pub fn new(workspace: impl Into<SharedString>, footer: SidebarAccount) -> Self {
        Self {
            workspace: workspace.into(),
            items: Vec::new(),
            groups_label: "Workspaces".into(),
            groups: Vec::new(),
            selected: None,
            footer,
        }
    }

    /// Adds a primary nav row.
    pub fn item(mut self, item: SidebarNavItem) -> Self {
        self.items.push(item);
        self
    }

    /// Overrides the caps label of the group row.
    pub fn groups_label(mut self, label: impl Into<SharedString>) -> Self {
        self.groups_label = label.into();
        self
    }

    /// Adds a group.
    pub fn group(mut self, group: SidebarGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Selects a session.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// The collapsed form of this data: the nav glyphs (the warning count
    /// becomes the badge), the separator, then one cell per active session —
    /// every session that is not [`AgentState::Idle`], in group order.
    pub fn rail_items(&self) -> Vec<RailItem> {
        let mut items: Vec<RailItem> = self
            .items
            .iter()
            .map(|i| {
                let cell = RailItem::nav(i.name.clone(), i.icon);
                if i.warning && i.count.is_some() {
                    cell.badge()
                } else {
                    cell
                }
            })
            .collect();
        items.push(RailItem::separator());
        for group in &self.groups {
            for s in &group.sessions {
                if s.state == AgentState::Idle {
                    continue;
                }
                let mut cell = RailItem::session(s.id.clone(), s.state).selected(self.selected.as_ref() == Some(&s.id));
                if s.pulse {
                    cell = cell.pulse();
                }
                items.push(cell);
            }
        }
        items
    }
}

type SelectHandler = std::rc::Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ToggleHandler = std::rc::Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ActionHandler = std::rc::Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The sidebar. Build with [`sidebar`].
#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    nav: SidebarNav,
    collapsed: bool,
    on_select: Option<SelectHandler>,
    on_toggle_group: Option<ToggleHandler>,
    on_action: Option<ActionHandler>,
}

/// A sidebar rendering `nav`.
pub fn sidebar(id: impl Into<ElementId>, nav: SidebarNav) -> Sidebar {
    Sidebar { id: id.into(), nav, collapsed: false, on_select: None, on_toggle_group: None, on_action: None }
}

impl Sidebar {
    /// Collapsed (⌘B): renders the [`crate::nav::Rail`] instead. The width
    /// change itself is the caller's (the shell animates the column).
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// A session row was clicked; the argument is the session id.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }

    /// A group header was clicked; the argument is the group id.
    pub fn on_toggle_group(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_group = Some(std::rc::Rc::new(f));
        self
    }

    /// A named action: the nav rows report their own name; the header
    /// reports `"workspace"`, `"search"`, `"new"` and `"collapse"`; the group
    /// row reports `"view-options"` and `"add"`; the footer `"account"`.
    pub fn on_action(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }

    /// `.hd`: the workspace switcher and the three quiet actions.
    fn header(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let switcher_id: ElementId = (id.clone(), "workspace").into();
        let (state, _) = interaction_flags(switcher_id.clone(), window, cx);
        let initial: String = self
            .nav
            .workspace
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "\u{b7}".to_owned());
        let mut switcher = h_flex()
            .id(switcher_id)
            .flex_none()
            .gap(px(WS_GAP))
            .ui(scale::FS_13)
            .semibold()
            .text_color(p.ink)
            .cursor_pointer()
            .track_interaction(&state)
            .child(project_mark(initial, p.accent).size(px(WS_MARK)))
            .child(div().min_w(px(0.0)).truncate().child(self.nav.workspace.clone()))
            .child(icon(IconName::ChevronDown).size(px(WS_CHEVRON)).color(p.ink_3));
        if let Some(h) = self.on_action.clone() {
            switcher = switcher.on_click(move |_, w, cx| h("workspace", w, cx));
        }

        let mut row = h_flex()
            .w_full()
            .flex_none()
            .h(cx.aui().metrics.header)
            .gap(px(HEADER_GAP))
            .pl(px(HEADER_PAD_LEFT))
            .pr(px(HEADER_PAD_RIGHT))
            .border_b_1()
            .border_color(p.line)
            .child(switcher)
            .child(div().flex_1());
        for (name, glyph) in [("search", IconName::Search), ("new", IconName::Plus), ("collapse", IconName::Sidebar)] {
            let mut b = icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Sm);
            if let Some(h) = self.on_action.clone() {
                b = b.on_click(move |_, w, cx| h(name, w, cx));
            }
            row = row.child(b);
        }
        row
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        if self.collapsed {
            let mut r = rail((id, "rail"), self.nav.rail_items()).avatar(self.nav.footer.initial.clone());
            if let Some(h) = self.on_select.clone() {
                r = r.on_select(move |k, w, cx| h(k, w, cx));
            }
            if let Some(h) = self.on_action.clone() {
                r = r.on_action(move |name, w, cx| h(name, w, cx));
            }
            return r.into_any_element();
        }

        let header = self.header(window, cx);

        // `.nav{padding:8px}`.
        let mut nav = v_flex().w_full().flex_none().p(px(NAV_PAD));
        for item in &self.nav.items {
            let mut row = nav_item((id.clone(), item.name.clone()), item.icon, item.label.clone());
            if let Some(count) = &item.count {
                row = row.count(count.clone());
                if item.warning {
                    row = row.count_warning();
                }
            }
            if let Some(h) = self.on_action.clone() {
                let name = item.name.clone();
                row = row.on_click(move |_, w, cx| h(&name, w, cx));
            }
            nav = nav.child(row);
        }

        // The `.grp` caps row; card 21 gives it no top margin.
        let mut groups_row = group_row((id.clone(), "groups"), self.nav.groups_label.clone()).margin_top(0.0);
        if let Some(h) = self.on_action.clone() {
            let h2 = h.clone();
            groups_row = groups_row.on_view_options(move |_, w, cx| h("view-options", w, cx)).on_add(move |_, w, cx| h2("add", w, cx));
        }

        let mut body = v_flex().id((id.clone(), "body")).w_full().flex_1().min_h(px(0.0)).overflow_y_scroll().child(nav).child(groups_row);

        for group in self.nav.groups {
            let mut header = group_header((id.clone(), group.id.clone()), group.label.clone(), group.open).margin_top(0.0);
            if let Some(count) = group.count {
                header = header.count(count);
            }
            if let Some(trailing) = group.trailing {
                header = header.trailing(
                    div().flex_none().font_family(scale::FONT_MONO).text_px(TRAILING_TEXT).line_height(gpui::relative(1.0)).text_color(p.ink_3).child(trailing),
                );
            }
            if let Some(h) = self.on_toggle_group.clone() {
                let key = group.id.clone();
                header = header.on_toggle(move |_, w, cx| h(&key, w, cx));
            }
            body = body.child(header);
            if !group.open {
                continue;
            }
            for (i, session) in group.sessions.into_iter().enumerate() {
                let row_id: ElementId = (id.clone(), SharedString::from(format!("{}-{i}", group.id))).into();
                let selected = self.nav.selected.as_ref() == Some(&session.id);
                let mut row = session_row(row_id, session)
                    .selected(selected)
                    .margin_x(ROW_MARGIN_X)
                    .row_gap(ROW_GAP)
                    .text_sizes(ROW_NAME, ROW_META);
                if let Some(h) = self.on_select.clone() {
                    row = row.on_select(move |k, w, cx| h(k, w, cx));
                }
                body = body.child(row);
            }
        }

        let mut footer = sidebar_footer((id, "footer"), self.nav.footer.initial, self.nav.footer.name)
            .meter(self.nav.footer.provider, self.nav.footer.usage)
            .pad_y(FOOTER_PAD_Y);
        if let Some(h) = self.on_action.clone() {
            footer = footer.on_click(move |_, w, cx| h("account", w, cx));
        }

        v_flex()
            .flex_none()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .child(header)
            .child(body)
            .child(footer)
            .into_any_element()
    }
}
