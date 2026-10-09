//! The sidebar screen: header, mode control, nav, the virtualised session
//! list and the footer on the v2 tokens.
//!
//! One rhythm everywhere: every row is `metrics.control` tall, leading
//! glyphs sit on the 16 px icon axis, text starts on the text axis
//! (`item_px + icon + gap`), and counts hang at the trailing edge in
//! `Caption` `fg_muted`. Colour stays calm: the only accent is the new
//! session's plus and the running states; everything else is `fg` /
//! `fg_muted` on `surface(1)` with hairlines for structure only.
//!
//! The screen is stateless: `model` is plain data, `state` is the
//! caller-owned [`ListState`] sized with [`screen_item_count`]. Sync the two
//! like [`virtual_sidebar_view`](crate::nav::virtual_sidebar_view): after a
//! regroup `state.reset(count)`, after an insert/remove `state.splice`, after
//! a text-only change `remeasure_items`. A mismatch never panics — the list
//! paints blanks past the model — but the scrollbar goes stale until the
//! counts agree.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{
    div, list, prelude::*, px, relative, AnyElement, App, ElementId, IntoElement, ListState,
    Pixels, RenderOnce, SharedString, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::browse::{address_line, folder_row, pin_grid, tab_row};
use crate::data::{icon_button, record_ax_label, ButtonSize};
use crate::interact::{fluid_list, interactive, row, Axis};
use crate::nav::{
    flatten_sidebar, project_mark, Grouping, RowAction, SessionSummary, SidebarRow,
};

use super::{account_menu, project_menu, Mode, SidebarIntent, SidebarModel};

/// The sidebar's default width: 260 px beside the window pane.
pub const SIDEBAR_WIDTH: f32 = 260.0;

type IntentHandler = Rc<dyn Fn(SidebarIntent, &mut Window, &mut App)>;

/// How many virtual rows [`sidebar_screen`] builds for `model`: three
/// skeleton rows while loading, otherwise the flattened grouping. Size the
/// caller-owned [`ListState`] with this before rendering.
pub fn screen_item_count(model: &SidebarModel) -> usize {
    if model.loading {
        3
    } else {
        flatten_sidebar(&model.grouping, false).len()
    }
}

/// The sidebar screen for `model`, virtualised through `state`.
///
/// ```ignore
/// let rows = flatten_sidebar(&model.grouping, false);
/// let state = sidebar_list_state(rows.len());
/// sidebar_screen("sidebar", model, state).on_intent(|intent, _, _| { /* host acts */ })
/// ```
pub fn sidebar_screen(
    id: impl Into<ElementId>,
    model: SidebarModel,
    state: ListState,
) -> SidebarScreen {
    SidebarScreen {
        id: id.into(),
        model,
        state,
        width: px(SIDEBAR_WIDTH),
        on_intent: None,
    }
}

/// The sidebar screen. Build with [`sidebar_screen`].
#[derive(IntoElement)]
pub struct SidebarScreen {
    id: ElementId,
    model: SidebarModel,
    state: ListState,
    width: Pixels,
    on_intent: Option<IntentHandler>,
}

impl SidebarScreen {
    /// Reports what the person asked the app to do.
    pub fn on_intent(
        mut self,
        f: impl Fn(SidebarIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }

    /// Overrides the sidebar width ([`SIDEBAR_WIDTH`] by default).
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }
}

/// The leading slot every row shares: `item_px + icon` wide with its child
/// centred, so glyphs sit on one 16 px axis and text starts on one axis.
fn leading_box(content: impl IntoElement, cx: &App) -> AnyElement {
    let metrics = cx.aui().metrics;
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .w(metrics.item_px + metrics.icon)
        .h_full()
        .child(content)
        .into_any_element()
}

/// The status dot: filled while running or needing attention, hollow while
/// idle, in the session's state colour on the icon axis.
fn status_dot(session: &SessionSummary, cx: &App) -> AnyElement {
    let p = cx.aui().colors;
    let ink = p.agent_state(session.state);
    let filled = !matches!(
        session.state,
        aui_tokens::AgentState::Idle | aui_tokens::AgentState::Done
    );
    let mut dot = div().flex_none().size(px(scale::SP_3)).rounded_full();
    dot = if filled {
        dot.bg(ink)
    } else {
        dot.border_1().border_color(ink)
    };
    leading_box(
        div()
            .size(cx.aui().metrics.icon)
            .flex()
            .items_center()
            .justify_center()
            .child(dot),
        cx,
    )
}

/// The `⌘K` / `⌘N` chip: quiet bordered micro text, decorative — the row's
/// own label names the control, so the chip records no ax label of its own.
fn chip(keys: &str, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .px(cx.aui().metrics.item_px)
        .border_1()
        .border_color(cx.aui().colors.line)
        .role_in(TextRole::Micro, cx)
        .text_color(cx.aui().colors.fg_muted)
        .child(keys.to_string())
        .into_any_element()
}

/// A trailing count in `Caption` `fg_muted`.
fn count_text(count: &str, cx: &App) -> AnyElement {
    div()
        .role_in(TextRole::Caption, cx)
        .text_color(cx.aui().colors.fg_muted)
        .child(count.to_string())
        .into_any_element()
}

impl SidebarScreen {
    /// The workspace tile: mark, name and chevron as one row opening the
    /// project menu.
    fn workspace_row(&self, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let workspace = &self.model.workspace;
        let label: SharedString = format!("Workspace {}", workspace.name).into();
        let mut row_el = interactive(
            (self.id.clone(), "workspace"),
            h_flex()
                .h(metrics.control)
                .w_full()
                .items_center()
                .gap(metrics.gap)
                .pr(metrics.item_px)
                .child(leading_box(
                    project_mark(workspace.initial.clone(), p.label(workspace.tint)),
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .role_in(TextRole::Body, cx)
                        .text_color(p.fg)
                        .child(workspace.name.clone()),
                )
                .child(icon(IconName::ChevronDown).size(metrics.icon).color(p.fg_muted)),
        )
        .label(label);
        if let Some(emit) = self.on_intent.clone() {
            row_el = row_el.on_click(move |_, w, cx| emit(SidebarIntent::OpenProjectMenu, w, cx));
        }
        row_el.into_any_element()
    }

    /// The search row: a quiet field with the icon on the icon axis, the
    /// placeholder on the text axis and the `⌘K` chip at the trailing edge.
    fn search_row(&self, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let text = self.model.search.clone().unwrap_or_else(|| "Search".into());
        let dimmed = self.model.search.is_none();
        let mut row_el = interactive(
            (self.id.clone(), "search"),
            h_flex()
                .h(metrics.control)
                .w_full()
                .items_center()
                .gap(metrics.gap)
                .pr(metrics.item_px)
                .child(leading_box(
                    icon(IconName::Search).size(metrics.icon).color(p.fg_muted),
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .role_in(TextRole::Body, cx)
                        .text_color(if dimmed { p.fg_muted } else { p.fg })
                        .child(text),
                )
                .child(chip("⌘K", cx)),
        )
        .label("Search");
        if let Some(emit) = self.on_intent.clone() {
            row_el = row_el.on_click(move |_, w, cx| emit(SidebarIntent::Search, w, cx));
        }
        row_el.into_any_element()
    }

    /// The `+ New session` row, reading as the list's first row. The plus
    /// carries the one accent; the title stays calm.
    fn new_session_row(&self, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let mut row_el = interactive(
            (self.id.clone(), "new-session"),
            h_flex()
                .h(metrics.control)
                .w_full()
                .items_center()
                .gap(metrics.gap)
                .pr(metrics.item_px)
                .child(leading_box(
                    icon(IconName::Plus).size(metrics.icon).color(p.accent),
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .role_in(TextRole::Body, cx)
                        .text_color(p.fg)
                        .child("New session"),
                )
                .child(chip("⌘N", cx)),
        )
        .label("New session");
        if let Some(emit) = self.on_intent.clone() {
            row_el = row_el.on_click(move |_, w, cx| emit(SidebarIntent::NewSession, w, cx));
        }
        row_el.into_any_element()
    }

    /// The segmented Sessions / Browse control on the control ladder, with
    /// counts in `Caption` `fg_muted`.
    fn mode_control(&self, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        record_ax_label("Mode control");
        let segment = |key: &'static str, label: &'static str, count: u32, mode: Mode, cx: &App| {
            let mut control = interactive(
                (self.id.clone(), key),
                h_flex()
                    .h(metrics.control)
                    .flex_1()
                    .min_w(px(0.0))
                    .items_center()
                    .justify_center()
                    .gap(metrics.gap)
                    .child(
                        div()
                            .truncate()
                            .role_in(TextRole::Body, cx)
                            .text_color(p.fg)
                            .child(label.to_string()),
                    )
                    .child(count_text(&count.to_string(), cx)),
            )
            .role("tab")
            .label(label.to_string())
            .selected(self.model.mode == mode);
            if let Some(emit) = self.on_intent.clone() {
                control = control.on_click(move |_, w, cx| emit(SidebarIntent::SwitchMode(mode), w, cx));
            }
            control.into_any_element()
        };
        div()
            .id((self.id.clone(), "mode"))
            .w_full()
            .flex_none()
            .role(gpui::Role::TabList)
            .aria_label("Mode control")
            .child(
                h_flex()
                    .h(metrics.control)
                    .w_full()
                    .items_center()
                    .px(metrics.item_px)
                    .gap(metrics.gap)
                    .child(segment("mode-sessions", "Sessions", self.model.counts.0, Mode::Sessions, cx))
                    .child(segment("mode-browse", "Browse", self.model.counts.1, Mode::Browse, cx)),
            )
            .into_any_element()
    }

    /// The primary nav rows under the mode control.
    fn nav_rows(&self, cx: &App) -> Vec<AnyElement> {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        self.model
            .nav
            .iter()
            .map(|item| {
                let trailing = if item.soon {
                    div()
                        .role_in(TextRole::Micro, cx)
                        .text_color(p.fg_muted)
                        .child("Soon")
                        .into_any_element()
                } else if let Some(count) = item.count {
                    count_text(&count.to_string(), cx)
                } else {
                    div().into_any_element()
                };
                let mut nav = row((self.id.clone(), item.id.clone()), item.label.clone())
                    .leading(icon(item.icon).size(metrics.icon).color(p.fg_muted))
                    .trailing(trailing);
                if let Some(emit) = self.on_intent.clone() {
                    let id = item.id.clone();
                    nav = nav.on_click(move |_, w, cx| emit(SidebarIntent::Nav(id.clone()), w, cx));
                }
                nav.into_any_element()
            })
            .collect()
    }

    /// One session row: the status dot on the icon axis, the title in
    /// `Body`, the age or status sentence at the trailing edge. The hovered
    /// row swaps the trailing slot for up to three actions; the label stays
    /// readable. Selected rows wear the selected overlay through
    /// [`Interactive`](crate::interact::Interactive).
    fn session_row(&self, session: &SessionSummary, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let id = session.id.clone();
        let title = session.name.clone();
        let selected = self.model.selected.as_ref() == Some(&id);

        if let Some(editing) = self.model.editing.as_ref().filter(|e| e.session == id) {
            return self.rename_field(&id, &title, &editing.draft, cx);
        }

        let hovered = self.model.hovered.as_ref() == Some(&id);
        let trailing: AnyElement = if hovered {
            let mut actions = h_flex().flex_none().items_center().gap(metrics.gap);
            for (key, glyph, name, action) in [
                ("pin", IconName::Pin, "Pin", RowAction::Pin),
                ("rename", IconName::Edit, "Rename", RowAction::Rename),
                ("archive", IconName::Archive, "Archive", RowAction::Archive),
            ] {
                let action_id: ElementId =
                    (self.id.clone(), SharedString::from(format!("{id}-{key}"))).into();
                let mut button = icon_button(action_id, glyph)
                    .ghost()
                    .size(ButtonSize::Sm)
                    .icon_size(metrics.icon)
                    .accessibility_label(format!("{name} session {title}"));
                if let Some(emit) = self.on_intent.clone() {
                    let session_id = id.clone();
                    button = button.on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        emit(SidebarIntent::RowAction(session_id.clone(), action), w, cx);
                    });
                }
                actions = actions.child(button);
            }
            actions.into_any_element()
        } else if let Some(status) = session.status.as_ref() {
            div()
                .role_in(TextRole::Caption, cx)
                .text_color(status.ink(&p))
                .child(status.text())
                .into_any_element()
        } else {
            count_text(&session.elapsed, cx)
        };

        let row_id: ElementId =
            (self.id.clone(), SharedString::from(format!("session-{id}"))).into();
        let mut content = row(row_id, title.clone())
            .leading(status_dot(session, cx))
            .trailing(trailing)
            .selected(selected);
        if let Some(byline) = second_line(session) {
            content = content.two_line(byline);
        }
        let open_id: ElementId =
            (self.id.clone(), SharedString::from(format!("open-{id}"))).into();
        let mut wrap = interactive(open_id, content)
            .selected(selected)
            .label(format!("Open session {title}"));
        if let Some(emit) = self.on_intent.clone() {
            let session_id = id.clone();
            wrap = wrap.on_click(move |_, w, cx| emit(SidebarIntent::Select(session_id.clone()), w, cx));
        }
        // Hover reports through the row's own events, so the host learns
        // about the pointer even when nothing re-renders.
        let hover_id = id.clone();
        let enter_emit = self.on_intent.clone();
        let leave_emit = self.on_intent.clone();
        let hover_wrap: ElementId =
            (self.id.clone(), SharedString::from(format!("hover-{id}"))).into();
        div()
            .id(hover_wrap)
            .w_full()
            .on_hover(move |hovered: &bool, w: &mut Window, cx: &mut App| {
                if *hovered {
                    if let Some(emit) = enter_emit.clone() {
                        emit(SidebarIntent::Hover(Some(hover_id.clone())), w, cx);
                    }
                } else if let Some(emit) = leave_emit.clone() {
                    emit(SidebarIntent::Hover(None), w, cx);
                }
            })
            .child(wrap)
            .into_any_element()
    }

    /// The inline rename field: the draft at control height, focus-accent
    /// border, named for the session it renames.
    fn rename_field(
        &self,
        session: &SharedString,
        title: &SharedString,
        draft: &SharedString,
        cx: &App,
    ) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let label: SharedString = format!("Rename session {title}").into();
        record_ax_label(&label);
        let commit = session.clone();
        let text = draft.clone();
        let emit = self.on_intent.clone();
        let field_id: ElementId =
            (self.id.clone(), SharedString::from(format!("rename-{session}"))).into();
        div()
            .id(field_id)
            .h(metrics.control)
            .w_full()
            .flex()
            .items_center()
            .px(metrics.item_px)
            .border_1()
            .border_color(p.accent)
            .bg(p.surface(2))
            .role(gpui::Role::TextInput)
            .aria_label(label)
            .on_key_down(move |event: &gpui::KeyDownEvent, w: &mut Window, cx: &mut App| {
                if event.keystroke.key == "enter" {
                    if let Some(emit) = emit.clone() {
                        emit(SidebarIntent::Rename(commit.clone(), text.clone()), w, cx);
                    }
                }
            })
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .role_in(TextRole::Body, cx)
                    .text_color(p.fg)
                    .child(draft.clone()),
            )
            .into_any_element()
    }

    /// A section header: the sentence-case title in `Caption` `fg_muted`
    /// with the count, plus the section's ghost actions.
    fn section_header(&self, label: &SharedString, count: usize, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let mut actions = h_flex().flex_none().items_center();
        for (key, glyph, name, action) in [
            ("new", IconName::Plus, "New session", crate::nav::GroupAction::New),
            ("options", IconName::Dots, "options", crate::nav::GroupAction::Menu),
        ] {
            let button_label: SharedString = if action == crate::nav::GroupAction::New {
                format!("{name} in {label}").into()
            } else {
                format!("{label} {name}").into()
            };
            let action_id: ElementId =
                (self.id.clone(), SharedString::from(format!("{label}-{key}"))).into();
            let mut button = icon_button(action_id, glyph)
                .ghost()
                .size(ButtonSize::Sm)
                .icon_size(metrics.icon)
                .accessibility_label(button_label);
            if let Some(emit) = self.on_intent.clone() {
                let group = label.clone();
                button = button.on_click(move |_, w, cx| {
                    cx.stop_propagation();
                    emit(SidebarIntent::GroupAction(group.clone(), action), w, cx);
                });
            }
            actions = actions.child(button);
        }
        let group = label.clone();
        let header_id: ElementId =
            (self.id.clone(), SharedString::from(format!("section-{label}"))).into();
        let mut header = interactive(
            header_id,
            h_flex()
                .h(metrics.control)
                .w_full()
                .items_center()
                .gap(metrics.gap)
                .px(metrics.item_px)
                .child(
                    div()
                        .role_in(TextRole::Caption, cx)
                        .text_color(p.fg_muted)
                        .child(label.clone()),
                )
                .child(count_text(&count.to_string(), cx))
                .child(div().flex_1())
                .child(actions),
        )
        .label(format!("{label} sessions"));
        if let Some(emit) = self.on_intent.clone() {
            header = header.on_click(move |_, w, cx| emit(SidebarIntent::Toggle(group.clone()), w, cx));
        }
        header.into_any_element()
    }

    /// One flattened row of the virtualised session list.
    fn flat_row(&self, item: &SidebarRow, cx: &App) -> AnyElement {
        let grouping = &self.model.grouping;
        let blank = || div().w_full().into_any_element();
        match item {
            SidebarRow::Caption => blank(),
            SidebarRow::StatusHeader { group } => match grouping.as_ref() {
                Grouping::Status(groups) => match groups.get(*group) {
                    Some(status) => self.section_header(&status.label, status.sessions.len(), cx),
                    None => blank(),
                },
                _ => blank(),
            },
            SidebarRow::ProjectHead { group } => match grouping.as_ref() {
                Grouping::Project(groups) => match groups.get(*group) {
                    Some(project) => self.section_header(&project.name, project.sessions.len(), cx),
                    None => blank(),
                },
                _ => blank(),
            },
            SidebarRow::PinnedHeader => {
                let mut count = 0;
                if let Grouping::Date(groups) = grouping.as_ref() {
                    for group in groups {
                        count += group.sessions.iter().filter(|s| s.pinned).count();
                    }
                }
                let label: SharedString = "Pinned".into();
                self.section_header(&label, count, cx)
            }
            SidebarRow::DateHeader { group, .. } => match grouping.as_ref() {
                Grouping::Date(groups) => match groups.get(*group) {
                    Some(bucket) => {
                        let rest = bucket.sessions.iter().filter(|s| !s.pinned).count();
                        self.section_header(&bucket.label, rest, cx)
                    }
                    None => blank(),
                },
                _ => blank(),
            },
            SidebarRow::Session { scope, group, session } => {
                let summary = match (grouping.as_ref(), scope) {
                    (Grouping::Status(groups), crate::nav::SessionScope::Status) => {
                        groups.get(*group).and_then(|g| g.sessions.get(*session))
                    }
                    (Grouping::Project(groups), crate::nav::SessionScope::Project) => {
                        groups.get(*group).and_then(|g| g.sessions.get(*session))
                    }
                    (Grouping::Date(groups), _) => {
                        groups.get(*group).and_then(|g| g.sessions.get(*session))
                    }
                    _ => None,
                };
                match summary {
                    Some(summary) => {
                        div().w_full().child(self.session_row(summary, cx)).into_any_element()
                    }
                    None => blank(),
                }
            }
            SidebarRow::Fold { group } => match grouping.as_ref() {
                Grouping::Project(groups) => match groups.get(*group) {
                    Some(project) => {
                        let (hidden, _) = project.fold.unwrap_or((0, false));
                        let label: SharedString = format!("Show {} more", hidden.max(1)).into();
                        let fold_id: ElementId = (
                            self.id.clone(),
                            SharedString::from(format!("fold-{}", project.id)),
                        )
                            .into();
                        let mut fold = row(fold_id, label.clone());
                        if let Some(emit) = self.on_intent.clone() {
                            let id = project.id.clone();
                            fold = fold.on_click(move |_, w, cx| {
                                emit(
                                    SidebarIntent::GroupAction(
                                        id.clone(),
                                        crate::nav::GroupAction::ToggleMore,
                                    ),
                                    w,
                                    cx,
                                );
                            });
                        }
                        div().w_full().child(fold).into_any_element()
                    }
                    None => blank(),
                },
                _ => blank(),
            },
        }
    }

    /// Three skeleton rows while loading: no centred text, just the sweep.
    fn skeleton_list(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        let metrics = cx.aui().metrics;
        let mut column = v_flex().w_full();
        for index in 0..3 {
            let skeleton_id: ElementId =
                (self.id.clone(), SharedString::from(format!("skeleton-{index}"))).into();
            let line =
                aui_motion::skeleton(skeleton_id, px(scale::SP_7), px(scale::SP_3), window, cx)
                    .flex_1()
                    .min_w(px(0.0));
            column = column.child(
                div()
                    .w_full()
                    .h(metrics.control)
                    .flex()
                    .items_center()
                    .px(metrics.item_px)
                    .child(line),
            );
        }
        column.into_any_element()
    }

    /// The quiet empty state: one section title, one line, no hero. The
    /// line records its words with the ax probe, so the empty state is
    /// announced like any other row.
    fn empty_list(&self, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        record_ax_label("No sessions yet");
        v_flex()
            .w_full()
            .px(metrics.item_px)
            .py(metrics.gap)
            .gap(metrics.gap)
            .child(
                div()
                    .role_in(TextRole::Caption, cx)
                    .text_color(p.fg_muted)
                    .child("Sessions"),
            )
            .child(
                div()
                    .role_in(TextRole::Caption, cx)
                    .text_color(p.fg_muted)
                    .child("No sessions yet"),
            )
            .into_any_element()
    }

    /// Browse mode's rows from `browse/rows.rs`, in the same middle region.
    fn browse_list(&self, cx: &App) -> AnyElement {
        let metrics = cx.aui().metrics;
        let Some(browse) = self.model.browse.clone() else {
            return self.empty_list(cx);
        };
        let mut column = v_flex()
            .w_full()
            .py(metrics.gap)
            .gap(metrics.gap)
            .child(
                div()
                    .w_full()
                    .px(metrics.item_px)
                    .child(address_line((self.id.clone(), "address"), browse.address.clone()).on_click({
                        let emit = self.on_intent.clone();
                        move |_, w, cx| {
                            if let Some(emit) = emit.clone() {
                                emit(SidebarIntent::Search, w, cx);
                            }
                        }
                    })),
            )
            .child(pin_grid((self.id.clone(), "pins"), browse.pins.clone()).on_select({
                let emit = self.on_intent.clone();
                move |id: &SharedString, w: &mut Window, cx: &mut App| {
                    if let Some(emit) = emit.clone() {
                        emit(SidebarIntent::Nav(id.clone()), w, cx);
                    }
                }
            }));
        for (index, tab) in browse.tabs.iter().enumerate() {
            let select_emit = self.on_intent.clone();
            let close_emit = self.on_intent.clone();
            let tab_id: ElementId =
                (self.id.clone(), SharedString::from(format!("tab-{index}"))).into();
            column = column.child(
                tab_row(tab_id, tab.clone())
                    .on_select(move |name: &SharedString, w: &mut Window, cx: &mut App| {
                        if let Some(emit) = select_emit.clone() {
                            emit(SidebarIntent::Select(name.clone()), w, cx);
                        }
                    })
                    .on_close(move |name: &SharedString, w: &mut Window, cx: &mut App| {
                        if let Some(emit) = close_emit.clone() {
                            emit(SidebarIntent::RowAction(name.clone(), RowAction::Archive), w, cx);
                        }
                    }),
            );
        }
        for (folder_ix, folder) in browse.folders.iter().enumerate() {
            let name = folder.name.clone();
            let toggle_emit = self.on_intent.clone();
            let folder_id: ElementId =
                (self.id.clone(), SharedString::from(format!("folder-{folder_ix}"))).into();
            column = column.child(
                folder_row(folder_id, name.clone(), folder.open, 0)
                    .on_toggle(move |folder: &SharedString, w: &mut Window, cx: &mut App| {
                        if let Some(emit) = toggle_emit.clone() {
                            emit(SidebarIntent::Toggle(folder.clone()), w, cx);
                        }
                    }),
            );
            if folder.open {
                for (index, tab) in folder.tabs.iter().enumerate() {
                    let select_emit = self.on_intent.clone();
                    let close_emit = self.on_intent.clone();
                    let tab_id: ElementId = (
                        self.id.clone(),
                        SharedString::from(format!("folder-{folder_ix}-tab-{index}")),
                    )
                        .into();
                    column = column.child(
                        tab_row(tab_id, tab.clone())
                            .on_select(move |tab: &SharedString, w: &mut Window, cx: &mut App| {
                                if let Some(emit) = select_emit.clone() {
                                    emit(SidebarIntent::Select(tab.clone()), w, cx);
                                }
                            })
                            .on_close(move |tab: &SharedString, w: &mut Window, cx: &mut App| {
                                if let Some(emit) = close_emit.clone() {
                                    emit(SidebarIntent::RowAction(tab.clone(), RowAction::Archive), w, cx);
                                }
                            }),
                    );
                }
            }
        }
        column.into_any_element()
    }

    /// The footer: avatar, name and plan on the text axis, the usage meter
    /// as a thin bar, the chevron — one row, `control` tall, hairline above.
    fn footer(&self, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let account = &self.model.account;
        let who: SharedString = format!("{} · {}", account.name, account.plan).into();
        let label: SharedString = format!("Account {}", who).into();
        let usage = account.usage.clamp(0.0, 1.0);
        let meter = div().w_full().h(px(2.0)).flex().items_center().bg(p.surface(3)).child(
            div().h_full().w(relative(usage)).bg(p.fg_muted),
        );
        let mut foot = interactive(
            (self.id.clone(), "footer"),
            h_flex()
                .h(metrics.control)
                .w_full()
                .items_center()
                .gap(metrics.gap)
                .px(metrics.item_px)
                .child(project_mark(account.avatar.clone(), p.label(account.tint)))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .justify_center()
                        .gap(px(2.0))
                        .child(
                            div()
                                .w_full()
                                .truncate()
                                .role_in(TextRole::Body, cx)
                                .text_color(p.fg)
                                .child(who),
                        )
                        .child(meter),
                )
                .child(icon(IconName::ChevronDown).size(metrics.icon).color(p.fg_muted)),
        )
        .label(label);
        if let Some(emit) = self.on_intent.clone() {
            foot = foot.on_click(move |_, w, cx| emit(SidebarIntent::Account, w, cx));
        }
        v_flex()
            .w_full()
            .child(div().flex_none().h(px(1.0)).w_full().bg(p.line))
            .child(foot)
            .into_any_element()
    }

    /// The collapsed state: the 12 px strip only. Peeking on hover is the
    /// host's job; here the strip is one labelled control.
    fn collapsed_strip(&self, _cx: &App) -> AnyElement {
        let mut strip = interactive(
            (self.id.clone(), "strip"),
            div().w(px(scale::SP_4)).h_full().bg(_cx.aui().colors.surface(1)),
        )
        .label("Expand sidebar");
        if let Some(emit) = self.on_intent.clone() {
            strip = strip.on_click(move |_, w, cx| emit(SidebarIntent::Collapse, w, cx));
        }
        div()
            .h_full()
            .flex()
            .border_r_1()
            .border_color(_cx.aui().colors.line)
            .child(strip)
            .into_any_element()
    }
}

impl RenderOnce for SidebarScreen {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.model.collapsed {
            return self.collapsed_strip(cx).into_any_element();
        }
        let model = self.model.clone();
        let grouping = model.grouping.clone();

        // The fixed chrome glides as one highlight: workspace, search, new
        // session, the mode control and the nav rows.
        let mut chrome = fluid_list((self.id.clone(), "chrome"), Axis::Y).label("Sidebar");
        chrome = chrome.item(0, self.workspace_row(cx));
        if model.workspace.menu_open {
            chrome = chrome.item_static(
                100,
                project_menu(
                    (self.id.clone(), "project-menu"),
                    &model.workspace.name,
                    self.on_intent.clone(),
                    cx,
                ),
            );
        }
        chrome = chrome.item(1, self.search_row(cx));
        chrome = chrome.item(2, self.new_session_row(cx));
        chrome = chrome.item(3, self.mode_control(cx));
        for (index, nav) in self.nav_rows(cx).into_iter().enumerate() {
            chrome = chrome.item(4 + index, nav);
        }

        // The middle region: the virtualised session list, the skeleton,
        // the empty line or the browse rows.
        let middle: AnyElement = if model.loading {
            div()
                .w_full()
                .flex_1()
                .min_h(px(0.0))
                .child(self.skeleton_list(window, cx))
                .into_any_element()
        } else if model.mode == Mode::Browse {
            div()
                .w_full()
                .flex_1()
                .min_h(px(0.0))
                .child(self.browse_list(cx))
                .into_any_element()
        } else {
            let rows = flatten_sidebar(&grouping, false);
            if rows.is_empty() {
                div()
                    .w_full()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(self.empty_list(cx))
                    .into_any_element()
            } else {
                let screen = Self {
                    id: self.id.clone(),
                    model: model.clone(),
                    state: self.state.clone(),
                    width: self.width,
                    on_intent: self.on_intent.clone(),
                };
                let render = move |ix: usize, _window: &mut Window, cx: &mut App| -> AnyElement {
                    match rows.get(ix) {
                        Some(row) => screen.flat_row(row, cx),
                        None => div().into_any_element(),
                    }
                };
                div()
                    .w_full()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(list(self.state.clone(), render).w_full().h_full())
                    .into_any_element()
            }
        };

        // The root paints `surface(1)` itself instead of seating a
        // [`Surface`](crate::interact::Surface) wrapper: an auto-height
        // wrapper between the host's definite height and this column would
        // leave `h_full` indefinite, and the virtual list's flex region
        // would collapse to its measured rows instead of filling the
        // window. The ambient step is 1 either way, so menus inside still
        // elevate from the same ground.
        let ground = cx.aui().colors.surface(1);
        let mut column =
            v_flex().w(self.width).h_full().bg(ground).child(chrome).child(middle);
        if model.account.menu_open {
            column = column.child(account_menu(
                (self.id.clone(), "account-menu"),
                &model.account.name,
                &model.account.plan,
                model.account.usage,
                self.on_intent.clone(),
                cx,
            ));
        }
        column = column.child(self.footer(cx));
        column.into_any_element()
    }
}

/// The second line a session row shows: the attention override first, then
/// the explicit byline, joined flat — the row keeps its two-line height
/// either way.
fn second_line(session: &SessionSummary) -> Option<SharedString> {
    if let Some(attention) = session.attention.as_ref() {
        return Some(attention.clone());
    }
    match session.byline.as_ref()? {
        crate::nav::Byline::Placeholder(text) | crate::nav::Byline::Preview(text) => {
            Some(text.clone())
        }
        crate::nav::Byline::TwoLines { ask, result } => {
            Some(format!("{ask} · {result}").into())
        }
    }
}
