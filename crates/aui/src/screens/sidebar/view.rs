//! The sidebar screen, built to the approved mock: title bar, one
//! [`FluidList`] holding every row (search, new session, nav, pinned,
//! projects with their virtualised sessions — or Browse mode's address,
//! controls, pins, tabs, folders and spaces), and the footer.
//!
//! The measurements are the mock's, in tokens (default density):
//!
//! - list rows are `control_lg` tall (32; 28 compact), two-line rows add
//!   one `inset` (48); the sidebar is 260 wide with an `item_px` gutter
//!   (8) on both sides, so rows span 8–252 and the highlight block has the
//!   item radius;
//! - the leading slot is `item_px + icon` wide (24) with its glyph or dot
//!   centred, which puts the icon axis at 20; text starts one `gap` after
//!   it, on the text axis at 40; counts, ages and chips end at 244;
//! - nested rows indent one `inset` (16): dot centre 36, text at 56;
//! - section titles are `control_md` tall (28) with one `SP_4` (12)
//!   above, their label at 16; the Projects header carries its actions
//!   only while hovered;
//! - row titles use the compact Body step (13/20) at both densities, so
//!   the list reads one step quieter than the pane; bylines, counts and
//!   ages are Caption, chips and branches Micro / compact Mono;
//! - only the dot carries status colour; the accent appears nowhere but
//!   the rename field's focus ring.
//!
//! The screen is stateless: `model` is plain data, `state` the
//! caller-owned [`ListState`] sized with [`screen_item_count`], `fluid`
//! the caller-owned [`FluidHandle`]. Sync the list state like
//! [`virtual_sidebar_view`](crate::nav::virtual_sidebar_view): after a
//! regroup `state.reset(count)`, after an insert/remove `state.splice`.

use std::collections::HashMap;
use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{spring_phase, SpringKind};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Density, TextRole};
use gpui::{
    div, list, prelude::*, px, radians, AnyElement, App, ElementId, Entity, IntoElement, ListState, Pixels, RenderOnce,
    SharedString, Window,
};
use gpui_kit::base::{
    h_flex,
    input::{Input, InputState},
    v_flex,
};

use crate::data::{icon_button, record_ax_label, ButtonSize};
use crate::interact::{fluid_list, interactive, surface, Axis, FluidHandle, Shape};
use crate::nav::{flatten_sidebar, project_mark, Grouping, GroupAction, RowAction, SessionSummary, SidebarRow};
use crate::util::{interaction_flags, TrackInteraction};

use super::menus::{account_menu, view_options_menu};
use super::{BrowseIntent, Bylines, Mode, SidebarIntent, SidebarModel};

/// The sidebar's width beside the pane: the mock's 260.
pub const SIDEBAR_WIDTH: f32 = 260.0;

/// FluidList indices: chrome rows count up from 0, pinned rows from
/// `PINNED_BASE`, virtualised session rows from `VIRTUAL_BASE` (plus their
/// flattened index), Browse rows from `BROWSE_BASE`.
const PINNED_BASE: usize = 100;
const VIRTUAL_BASE: usize = 1000;
const BROWSE_BASE: usize = 500;

/// Space left for the traffic lights before the sidebar toggle:
/// `SP_7 × 2 + SP_5 + SP_2` = 84, Claude Desktop's placement, so the toggle
/// sits at the same x whether the sidebar is open or collapsed.
fn traffic_light_gap() -> Pixels {
    px(scale::SP_7 * 2.0 + scale::SP_5 + scale::SP_2)
}

type IntentHandler = Rc<dyn Fn(SidebarIntent, &mut Window, &mut App)>;

/// The list row height at the active density: `control_lg` (32 / 28).
pub fn list_row_height(cx: &App) -> Pixels {
    cx.aui().metrics.control_lg
}

/// How many virtual rows [`sidebar_screen`] builds for `model`: the
/// flattened grouping (three skeleton rows while loading, none in Browse
/// mode). Size the caller-owned [`ListState`] with this before rendering.
pub fn screen_item_count(model: &SidebarModel) -> usize {
    if model.loading {
        4
    } else if model.mode == Mode::Browse {
        0
    } else {
        flatten_sidebar(&model.grouping, false).len()
    }
}

/// The sidebar screen for `model`, virtualised through `state`, with its
/// one highlight kept in `fluid`.
///
/// ```ignore
/// let rows = flatten_sidebar(&model.grouping, false);
/// let state = sidebar_list_state(rows.len());
/// let fluid = FluidHandle::new();
/// sidebar_screen("sidebar", model, state, fluid).on_intent(|intent, _, _| { /* host acts */ })
/// ```
pub fn sidebar_screen(id: impl Into<ElementId>, model: SidebarModel, state: ListState, fluid: FluidHandle) -> SidebarScreen {
    SidebarScreen { id: id.into(), model, state, fluid, width: px(SIDEBAR_WIDTH), rename_input: None, on_intent: None }
}

/// The sidebar screen. Build with [`sidebar_screen`].
#[derive(IntoElement)]
pub struct SidebarScreen {
    id: ElementId,
    model: SidebarModel,
    state: ListState,
    fluid: FluidHandle,
    width: Pixels,
    rename_input: Option<Entity<InputState>>,
    on_intent: Option<IntentHandler>,
}

impl SidebarScreen {
    /// Reports what the person asked the app to do.
    pub fn on_intent(mut self, f: impl Fn(SidebarIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }

    /// Overrides the sidebar width ([`SIDEBAR_WIDTH`] by default).
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// The live text input for the rename in flight. Without it the
    /// field shows the model's draft as static text (captures, tests).
    pub fn rename_input(mut self, input: &Entity<InputState>) -> Self {
        self.rename_input = Some(input.clone());
        self
    }
}

/// Everything a row builder needs, cloned into the virtual list's render
/// closure.
#[derive(Clone)]
struct Ctx {
    id: ElementId,
    model: Rc<SidebarModel>,
    fluid: FluidHandle,
    state: ListState,
    rename_input: Option<Entity<InputState>>,
    on_intent: Option<IntentHandler>,
}

impl Ctx {
    fn emit(&self, intent: SidebarIntent, window: &mut Window, cx: &mut App) {
        if let Some(handler) = &self.on_intent {
            handler(intent, window, cx);
        }
    }

    /// A child id under the screen's id.
    fn child(&self, name: impl Into<String>) -> ElementId {
        (self.id.clone(), SharedString::from(name.into())).into()
    }

    /// The leading slot every row shares: `item_px + icon` wide, content
    /// centred, so glyphs and dots sit on the icon axis.
    fn lead(&self, content: impl IntoElement, cx: &App) -> AnyElement {
        let metrics = cx.aui().metrics;
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .w(metrics.item_px + metrics.icon)
            .h(metrics.item_px + metrics.icon)
            .text_color(cx.aui().colors.fg_muted)
            .child(content)
            .into_any_element()
    }

    /// A glyph in the leading slot.
    fn lead_icon(&self, name: IconName, cx: &App) -> AnyElement {
        let metrics = cx.aui().metrics;
        self.lead(icon(name).size(metrics.icon).color(cx.aui().colors.fg_muted), cx)
    }

    /// The status dot: 8 px, hollow `ink-4` when idle, filled in the state
    /// colour when running / waiting / failed (running adds a soft ring),
    /// hollow `success` when done.
    fn dot(&self, state: AgentState, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let size = px(scale::SP_3);
        let dot = match state {
            AgentState::Idle => Shape::Pill.rounded(div().size(size).border_1().border_color(p.ink_4)),
            AgentState::Done => Shape::Pill.rounded(div().size(size).border_1().border_color(p.success)),
            AgentState::Running => Shape::Pill.rounded(div().size(size).bg(p.info)),
            AgentState::Waiting => Shape::Pill.rounded(div().size(size).bg(p.warning)),
            AgentState::Failed => Shape::Pill.rounded(div().size(size).bg(p.danger)),
        };
        let halo = if state == AgentState::Running {
            Shape::Pill.rounded(div().size(size + px(scale::SP_2) + px(scale::SP_1)).bg(p.info_soft).flex().items_center().justify_center().child(dot))
        } else {
            div().child(dot)
        };
        self.lead(halo, cx)
    }

    /// A trailing count or age: Caption, muted, tabular.
    fn trailing(&self, text: impl Into<SharedString>, cx: &App) -> AnyElement {
        div().flex_none().role_in(TextRole::Caption, cx).text_color(cx.aui().colors.fg_muted).child(text.into()).into_any_element()
    }

    /// A trailing shortcut chip: Micro, muted, no box.
    fn chip(&self, keys: &str, cx: &App) -> AnyElement {
        div().flex_none().role_in(TextRole::Micro, cx).text_color(cx.aui().colors.fg_muted).child(keys.to_string()).into_any_element()
    }

    /// The row title: compact Body at both densities, truncating.
    fn title(&self, text: impl Into<SharedString>, _cx: &App) -> gpui::Div {
        div()
            .flex_1()
            .min_w(px(0.0))
            .truncate()
            .text_role_in(TextRole::Body, Density::Compact)
            .child(text.into())
    }

    /// A one-line list row: leading slot, title, trailing. `nested`
    /// indents one `inset`.
    fn row(&self, lead: AnyElement, title: impl Into<SharedString>, trailing: Option<AnyElement>, nested: bool, cx: &App) -> gpui::Div {
        let metrics = cx.aui().metrics;
        let mut row = h_flex()
            .w_full()
            .h(list_row_height(cx))
            .items_center()
            .gap(metrics.gap)
            .pr(metrics.item_px)
            .text_color(cx.aui().colors.fg)
            .when(nested, |d| d.pl(metrics.inset))
            .child(lead)
            .child(self.title(title, cx));
        if let Some(trailing) = trailing {
            row = row.child(trailing);
        }
        row
    }

    /// A section title: `control_md` tall with `SP_4` above, Caption muted
    /// at x 16; optional count and hover actions.
    fn section(&self, label: &str, count: Option<SharedString>, actions: Option<AnyElement>, cx: &App) -> gpui::Div {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let mut head = h_flex()
            .w_full()
            .h(metrics.control_md)
            .mt(px(scale::SP_4))
            .items_center()
            .pl(metrics.item_px)
            .pr(px(scale::SP_2))
            .role_in(TextRole::Caption, cx)
            .text_color(p.fg_muted)
            .child(div().child(label.to_string()));
        if let Some(count) = count {
            head = head.child(div().ml(px(scale::SP_2 + scale::SP_1)).text_color(p.ink_4).child(count));
        }
        head = head.child(div().flex_1());
        if let Some(actions) = actions {
            head = head.child(actions);
        }
        head
    }

    /// A 22 px ghost icon button for section and row actions; clicks never
    /// reach the row beneath.
    fn action(&self, key: String, glyph: IconName, label: String, intent: SidebarIntent, cx: &App) -> AnyElement {
        let metrics = cx.aui().metrics;
        let ctx = self.clone();
        icon_button(self.child(key), glyph)
            .ghost()
            .size(ButtonSize::Sm)
            .icon_size(metrics.icon - px(scale::SP_1))
            .accessibility_label(label)
            .on_click(move |_, w, cx| {
                cx.stop_propagation();
                ctx.emit(intent.clone(), w, cx);
            })
            .into_any_element()
    }

    /// The second line a session row shows, or none: the status sentence
    /// where there is one, else the byline when [`Bylines::All`].
    fn second_line(&self, session: &SessionSummary) -> Option<SharedString> {
        if let Some(status) = session.status.as_ref() {
            return Some(status.text());
        }
        if self.model.bylines == Bylines::StatusOnly {
            return None;
        }
        match session.byline.as_ref()? {
            crate::nav::Byline::Placeholder(text) | crate::nav::Byline::Preview(text) => Some(text.clone()),
            crate::nav::Byline::TwoLines { ask, result } => Some(format!("{ask} · {result}").into()),
        }
    }

    /// One session row: dot, title (+ byline), age — or the three hover
    /// actions in the trailing slot while the pointer is on it. Selection
    /// and hover paint come from the FluidList, never from the row.
    fn session_row(&self, session: &SessionSummary, nested: bool, window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let id = session.id.clone();
        if let Some(editing) = self.model.editing.as_ref().filter(|e| e.session == id) {
            return self.rename_field(session, &editing.draft, nested, window, cx);
        }
        let hovered = self.model.hovered.as_ref() == Some(&id);
        let second = self.second_line(session);
        let mut text = v_flex().flex_1().min_w(px(0.0)).justify_center().child(
            div().truncate().text_role_in(TextRole::Body, Density::Compact).text_color(p.fg).child(session.name.clone()),
        );
        if let Some(line) = second.clone() {
            text = text.child(div().truncate().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(line));
        }
        let trailing: AnyElement = if hovered {
            let title = session.name.clone();
            h_flex()
                .flex_none()
                .items_center()
                .gap(px(scale::SP_1))
                .child(self.action(format!("{id}-pin"), IconName::Pin, format!("Pin session {title}"), SidebarIntent::RowAction(id.clone(), RowAction::Pin), cx))
                .child(self.action(format!("{id}-rename"), IconName::Edit, format!("Rename session {title}"), SidebarIntent::RowAction(id.clone(), RowAction::Rename), cx))
                .child(self.action(format!("{id}-archive"), IconName::Archive, format!("Archive session {title}"), SidebarIntent::RowAction(id.clone(), RowAction::Archive), cx))
                .into_any_element()
        } else if session.elapsed.is_empty() {
            div().into_any_element()
        } else {
            self.trailing(session.elapsed.clone(), cx)
        };
        let height = if second.is_some() { list_row_height(cx) + metrics.inset } else { list_row_height(cx) };
        let label: SharedString = format!("Open session {}", session.name).into();
        record_ax_label(&label);
        h_flex()
            .id(self.child(format!("session-{id}")))
            .w_full()
            .h(height)
            .items_center()
            .gap(metrics.gap)
            .pr(metrics.item_px)
            .when(nested, |d| d.pl(metrics.inset))
            .role(gpui::Role::ListItem)
            .aria_label(label)
            .child(self.dot(session.state, cx))
            .child(text)
            .child(trailing)
            .into_any_element()
    }

    /// The inline rename field: the row's own geometry on `surface-2`
    /// with the accent ring; the live input when the host gives one.
    fn rename_field(&self, session: &SessionSummary, draft: &SharedString, nested: bool, _window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let label: SharedString = format!("Rename session {}", session.name).into();
        record_ax_label(&label);
        let body: AnyElement = match self.rename_input.as_ref() {
            Some(input) => div().flex_1().min_w(px(0.0)).child(Input::new(input)).into_any_element(),
            None => h_flex()
                .flex_1()
                .min_w(px(0.0))
                .items_center()
                .text_color(p.fg)
                .child(div().truncate().text_role_in(TextRole::Body, Density::Compact).child(draft.clone()))
                .child(div().flex_none().w(px(1.0)).h(metrics.icon).bg(p.fg))
                .into_any_element(),
        };
        let field = h_flex()
            .w_full()
            .h(list_row_height(cx))
            .items_center()
            .gap(metrics.gap)
            .pr(metrics.item_px)
            .child(self.dot(session.state, cx))
            .child(body);
        let ctx = self.clone();
        let commit = session.id.clone();
        let text = draft.clone();
        div()
            .id(self.child(format!("rename-{}", session.id)))
            .w_full()
            .when(nested, |d| d.pl(metrics.inset))
            .role(gpui::Role::TextInput)
            .aria_label(label)
            .on_key_down(move |event: &gpui::KeyDownEvent, w: &mut Window, cx: &mut App| match event.keystroke.key.as_str() {
                "enter" => ctx.emit(SidebarIntent::Rename(commit.clone(), text.clone()), w, cx),
                "escape" => ctx.emit(SidebarIntent::RenameCancel, w, cx),
                _ => {}
            })
            .child(surface(2, field).shape(Shape::Item).ring(true))
            .into_any_element()
    }

    /// A project head: chevron on the icon axis, name on the text axis,
    /// count, branch, and the two actions while hovered.
    fn project_head(&self, group: &crate::nav::ProjectGroup, window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let hovered = self.model.hovered_group.as_ref() == Some(&group.id);
        let turn = spring_phase((self.child(format!("chevron-{}", group.id)), "turn"), group.open, SpringKind::Swap, window, cx);
        let chevron = icon(IconName::ChevronDown)
            .size(metrics.icon)
            .color(p.fg_muted)
            .rotate(radians(-std::f32::consts::FRAC_PI_2 * (1.0 - turn)));
        let mut head = h_flex()
            .w_full()
            .h(list_row_height(cx))
            .items_center()
            .gap(metrics.gap)
            .pr(px(scale::SP_2))
            .child(self.lead(chevron, cx))
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .text_role_in(TextRole::Body, Density::Compact)
                    .medium()
                    .text_color(p.fg)
                    .child(group.name.clone()),
            )
            .child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(group.count.clone()))
            .child(div().flex_1().min_w(px(0.0)));
        if let Some(branch) = group.trailing.as_ref().filter(|_| !hovered) {
            head = head.child(
                div()
                    .flex_none()
                    .max_w(px(scale::SP_7 * 3.0))
                    .truncate()
                    .text_role_in(TextRole::Mono, Density::Compact)
                    .text_color(p.fg_muted)
                    .child(branch.clone()),
            );
        }
        if hovered {
            head = head.child(
                h_flex()
                    .flex_none()
                    .ml(px(scale::SP_2))
                    .gap(px(scale::SP_1))
                    .child(self.action(format!("{}-new", group.id), IconName::Plus, format!("New session in {}", group.name), SidebarIntent::GroupAction(group.id.clone(), GroupAction::New), cx))
                    .child(self.action(format!("{}-menu", group.id), IconName::Dots, format!("{} options", group.name), SidebarIntent::GroupAction(group.id.clone(), GroupAction::Menu), cx)),
            );
        }
        let label: SharedString = format!("Project {}", group.name).into();
        record_ax_label(&label);
        head.id(self.child(format!("project-{}", group.id))).role(gpui::Role::Button).aria_label(label).into_any_element()
    }

    /// One flattened row of the virtualised project list.
    fn flat_row(&self, item: &SidebarRow, window: &mut Window, cx: &mut App) -> AnyElement {
        let Grouping::Project(groups) = self.model.grouping.as_ref() else {
            return div().into_any_element();
        };
        match item {
            SidebarRow::ProjectHead { group } => match groups.get(*group) {
                Some(project) => self.project_head(project, window, cx),
                None => div().into_any_element(),
            },
            SidebarRow::Session { group, session, .. } => match groups.get(*group).and_then(|g| g.sessions.get(*session)) {
                Some(summary) => self.session_row(summary, true, window, cx),
                None => div().into_any_element(),
            },
            SidebarRow::Fold { group } => match groups.get(*group) {
                Some(project) => {
                    let (hidden, expanded) = project.fold.unwrap_or((0, false));
                    let text = if expanded { "Show less".to_string() } else { format!("Show {} more", hidden.max(1)) };
                    let p = cx.aui().colors;
                    record_ax_label(&text);
                    self.row(self.lead(div(), cx), text.clone(), None, true, cx)
                        .text_color(p.fg_muted)
                        .id(self.child(format!("fold-{}", project.id)))
                        .role(gpui::Role::Button)
                        .aria_label(SharedString::from(text))
                        .into_any_element()
                }
                None => div().into_any_element(),
            },
            _ => div().into_any_element(),
        }
    }

    /// Four skeleton rows while loading: a project head and three nested
    /// sessions, no text.
    fn skeleton_row(&self, index: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let metrics = cx.aui().metrics;
        let bar = px(scale::SP_4);
        let widths = [scale::SP_7 * 3.0, scale::SP_7 * 4.5, scale::SP_7 * 3.5, scale::SP_7 * 4.0];
        let nested = index > 0;
        let lead = if nested {
            Shape::Pill.rounded(aui_motion::skeleton(self.child(format!("sk-dot-{index}")), px(scale::SP_3), px(scale::SP_3), window, cx))
        } else {
            Shape::Item.rounded(aui_motion::skeleton(self.child(format!("sk-mark-{index}")), metrics.icon, metrics.icon, window, cx))
        };
        let line = Shape::Item.rounded(aui_motion::skeleton(self.child(format!("sk-line-{index}")), px(widths[index % widths.len()]), bar, window, cx));
        self.row(self.lead(lead, cx), "", None, nested, cx).child(line).into_any_element()
    }
}

/// Builds the Sessions-mode chrome and list, or the Browse-mode rows, into
/// one FluidList; returns the list element and the index → session map the
/// highlight and activation handlers translate through.
fn build_list(ctx: &Ctx, window: &mut Window, cx: &mut App) -> (AnyElement, Rc<HashMap<usize, Target>>) {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let model = ctx.model.clone();
    let mut targets: HashMap<usize, Target> = HashMap::new();
    let mut index_of_session: HashMap<SharedString, usize> = HashMap::new();
    let mut list = fluid_list((ctx.id.clone(), "rows"), Axis::Y)
        .label("Sidebar")
        .gap(px(0.0))
        .gap_click(false)
        .fill()
        .handle(&ctx.fluid);
    let mut next = 0usize;

    if model.mode == Mode::Browse {
        if let Some(browse) = model.browse.as_ref() {
            // Address: a quiet surface-2 field with the lock on the icon
            // axis, host in fg, path muted; ⌘L only while hovered.
            let address = h_flex()
                .w_full()
                .h(list_row_height(cx))
                .items_center()
                .gap(metrics.gap)
                .pr(metrics.item_px)
                .child(ctx.lead_icon(IconName::Shield, cx))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .text_role_in(TextRole::Body, Density::Compact)
                        .child(div().text_color(p.fg).child(browse.host.clone()))
                        .child(div().truncate().text_color(p.fg_muted).child(browse.path.clone())),
                );
            record_ax_label("Address");
            list = list.item(next, surface(2, address).shape(Shape::Item).into_any_element());
            targets.insert(next, Target::Intent(SidebarIntent::Browse(BrowseIntent::Address)));
            next += 1;
            // Page controls: back, forward, reload, new tab | ask, menu.
            let control = |ctx: &Ctx, key: &str, glyph: IconName, label: &str, intent: BrowseIntent, enabled: bool, cx: &App| {
                let c = ctx.clone();
                let mut button = icon_button(ctx.child(format!("browse-{key}")), glyph)
                    .ghost()
                    .size(ButtonSize::Md)
                    .icon_size(cx.aui().metrics.icon)
                    .accessibility_label(label.to_string());
                if enabled {
                    button = button.on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        c.emit(SidebarIntent::Browse(intent.clone()), w, cx);
                    });
                } else {
                    button = button.disabled(true);
                }
                button.into_any_element()
            };
            let controls = h_flex()
                .w_full()
                .h(metrics.control)
                .items_center()
                .gap(px(scale::SP_2))
                .pl(px(scale::SP_1))
                .child(control(ctx, "back", IconName::ArrowLeft, "Back", BrowseIntent::Back, browse.can_go_back, cx))
                .child(control(ctx, "forward", IconName::ArrowRight, "Forward", BrowseIntent::Forward, browse.can_go_forward, cx))
                .child(control(ctx, "reload", IconName::Refresh, "Reload", BrowseIntent::Reload, true, cx))
                .child(control(ctx, "new-tab-button", IconName::Plus, "New tab", BrowseIntent::NewTab, true, cx))
                .child(div().flex_1())
                .child(control(ctx, "ask", IconName::Sparkle, "Ask Claude about this page", BrowseIntent::Ask, true, cx))
                .child(control(ctx, "page-menu", IconName::Dots, "Page menu", BrowseIntent::PageMenu, true, cx));
            list = list.item_static(next, controls);
            next += 1;
        }
    }

    // Search: the quiet first row; ⌘K only while the pointer is on it.
    let search_hovered = model.hovered.as_deref() == Some("search");
    let search_text = model.search.clone().unwrap_or_else(|| "Search".into());
    let search = ctx
        .row(ctx.lead_icon(IconName::Search, cx), search_text, if search_hovered { Some(ctx.chip("⌘K", cx)) } else { None }, false, cx)
        .when(model.search.is_none(), |d| d.text_color(p.fg_muted));
    record_ax_label("Search");
    list = list.item(next, search.id(ctx.child("search")).role(gpui::Role::Button).aria_label("Search"));
    targets.insert(next, Target::Intent(SidebarIntent::Search));
    next += 1;

    if model.mode == Mode::Sessions {
        // New session, then the nav rows.
        let new_hovered = model.hovered.as_deref() == Some("new-session");
        let new_row = ctx.row(ctx.lead_icon(IconName::Plus, cx), "New session", if new_hovered { Some(ctx.chip("⌘N", cx)) } else { None }, false, cx);
        record_ax_label("New session");
        list = list.item(next, new_row.id(ctx.child("new-session")).role(gpui::Role::Button).aria_label("New session"));
        targets.insert(next, Target::Intent(SidebarIntent::NewSession));
        next += 1;
        for item in &model.nav {
            let trailing = if item.soon {
                Some(ctx.trailing("Soon", cx))
            } else {
                item.count.map(|count| ctx.trailing(count.to_string(), cx))
            };
            let row = ctx.row(ctx.lead_icon(item.icon, cx), item.label.clone(), trailing, false, cx);
            record_ax_label(&item.label);
            list = list.item(next, row.id(ctx.child(format!("nav-{}", item.id))).role(gpui::Role::Button).aria_label(item.label.clone()));
            targets.insert(next, Target::Intent(SidebarIntent::Nav(item.id.clone())));
            next += 1;
        }

        if model.loading {
            list = list.item_static(next, ctx.section("Projects", None, None, cx));
            next += 1;
            for index in 0..4 {
                list = list.item_static(next, ctx.skeleton_row(index, window, cx));
                next += 1;
            }
        } else {
            // Pinned: a collapsible section whose rows are static items.
            if !model.pinned.is_empty() {
                let turn = spring_phase((ctx.child("pinned-chevron"), "turn"), model.pinned_open, SpringKind::Swap, window, cx);
                let chevron = icon(IconName::ChevronDown).size(metrics.icon).color(p.fg_muted).rotate(radians(-std::f32::consts::FRAC_PI_2 * (1.0 - turn)));
                let head = h_flex()
                    .w_full()
                    .h(metrics.control_md)
                    .mt(px(scale::SP_4))
                    .items_center()
                    .gap(metrics.gap)
                    .role_in(TextRole::Caption, cx)
                    .text_color(p.fg_muted)
                    .child(ctx.lead(chevron, cx))
                    .child(div().child("Pinned"))
                    .child(div().ml(px(scale::SP_1)).text_color(p.ink_4).child(model.pinned.len().to_string()));
                record_ax_label("Pinned");
                list = list.item(next, head.id(ctx.child("pinned-head")).role(gpui::Role::Button).aria_label("Pinned"));
                targets.insert(next, Target::Intent(SidebarIntent::TogglePinned));
                next += 1;
                if model.pinned_open {
                    for (i, session) in model.pinned.iter().enumerate() {
                        let index = PINNED_BASE + i;
                        list = list.item(index, ctx.session_row(session, false, window, cx));
                        targets.insert(index, Target::Session(session.id.clone()));
                        index_of_session.insert(session.id.clone(), index);
                    }
                }
            }
            // Projects header with its hover actions, then the virtual list.
            let project_count = match model.grouping.as_ref() {
                Grouping::Project(groups) => groups.len(),
                _ => 0,
            };
            let actions = model.hovered_projects_header.then(|| {
                h_flex()
                    .flex_none()
                    .gap(px(scale::SP_1))
                    .child(ctx.action("add-project".into(), IconName::Plus, "Add project".into(), SidebarIntent::AddProject, cx))
                    .child(ctx.action("view-options".into(), IconName::Sliders, "View options".into(), SidebarIntent::ViewOptions, cx))
                    .into_any_element()
            });
            let (header_state, _) = interaction_flags(ctx.child("projects-head"), window, cx);
            let hover_ctx = ctx.clone();
            let header = ctx
                .section("Projects", Some(project_count.to_string().into()), actions, cx)
                .id(ctx.child("projects-head"))
                .track_interaction_reported(&header_state, move |hovered: bool, w: &mut Window, cx: &mut App| {
                    hover_ctx.emit(SidebarIntent::HoverProjectsHeader(hovered), w, cx);
                });
            list = list.item_static(next, header);
            next += 1;
            if project_count == 0 {
                let empty = ctx.row(ctx.lead(div(), cx), "No projects yet", None, false, cx).text_color(p.fg_muted);
                record_ax_label("No projects yet");
                list = list.item_static(next, empty);
            } else {
                let rows = Rc::new(flatten_sidebar(&model.grouping, false));
                for (ix, row) in rows.iter().enumerate() {
                    let index = VIRTUAL_BASE + ix;
                    match row {
                        SidebarRow::ProjectHead { group } => {
                            if let Grouping::Project(groups) = model.grouping.as_ref() {
                                if let Some(project) = groups.get(*group) {
                                    targets.insert(index, Target::Intent(SidebarIntent::Toggle(project.id.clone())));
                                }
                            }
                        }
                        SidebarRow::Session { group, session, .. } => {
                            if let Grouping::Project(groups) = model.grouping.as_ref() {
                                if let Some(summary) = groups.get(*group).and_then(|g| g.sessions.get(*session)) {
                                    targets.insert(index, Target::Session(summary.id.clone()));
                                    index_of_session.insert(summary.id.clone(), index);
                                }
                            }
                        }
                        SidebarRow::Fold { group } => {
                            if let Grouping::Project(groups) = model.grouping.as_ref() {
                                if let Some(project) = groups.get(*group) {
                                    targets.insert(index, Target::Intent(SidebarIntent::ToggleMore(project.id.clone())));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let row_ctx = ctx.clone();
                let render = move |ix: usize, window: &mut Window, cx: &mut App| -> AnyElement {
                    match rows.get(ix) {
                        Some(row) => row_ctx.fluid.item(VIRTUAL_BASE + ix, row_ctx.flat_row(row, window, cx)),
                        None => div().into_any_element(),
                    }
                };
                list = list.item_static_fill(next, list_host(ctx, render));
            }
        }
    } else if let Some(browse) = model.browse.as_ref() {
        // Pinned sites as tiles.
        list = list.item_static(next, ctx.section("Pinned", None, None, cx));
        next += 1;
        let mut tiles = h_flex().w_full().gap(metrics.gap).pl(metrics.item_px).pb(px(scale::SP_2));
        for pin in &browse.pins {
            let tile = surface(
                2,
                div()
                    .size(metrics.control)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_role_in(TextRole::Body, Density::Compact)
                    .medium()
                    .text_color(p.ink_2)
                    .child(pin.initial.clone()),
            )
            .shape(Shape::Item);
            let c = ctx.clone();
            let id = pin.id.clone();
            tiles = tiles.child(
                interactive(ctx.child(format!("pin-{}", pin.id)), tile)
                    .label(pin.label.clone())
                    .on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        c.emit(SidebarIntent::Browse(BrowseIntent::OpenPin(id.clone())), w, cx);
                    }),
            );
        }
        list = list.item_static(next, tiles);
        next += 1;
        // Today: the tabs, with new-folder / close-all while hovered.
        let today_actions = browse.hovered_today_header.then(|| {
            h_flex()
                .flex_none()
                .gap(px(scale::SP_1))
                .child(ctx.action("new-folder".into(), IconName::Folder, "New folder".into(), SidebarIntent::Browse(BrowseIntent::NewFolder), cx))
                .child(ctx.action("close-all".into(), IconName::XBold, "Close all tabs".into(), SidebarIntent::Browse(BrowseIntent::CloseAll), cx))
                .into_any_element()
        });
        let tab_total = browse.tabs.len() + browse.folders.iter().map(|f| f.tabs.len()).sum::<usize>();
        let (today_state, _) = interaction_flags(ctx.child("today-head"), window, cx);
        let hover_ctx = ctx.clone();
        let today = ctx
            .section("Today", Some(tab_total.to_string().into()), today_actions, cx)
            .id(ctx.child("today-head"))
            .track_interaction_reported(&today_state, move |hovered: bool, w: &mut Window, cx: &mut App| {
                hover_ctx.emit(SidebarIntent::Browse(BrowseIntent::HoverTodayHeader(hovered)), w, cx);
            });
        list = list.item_static(next, today);
        next += 1;
        let tab_row = |ctx: &Ctx, list: &mut Option<crate::interact::FluidList>, index: usize, tab: &super::BrowseTab, nested: bool, cx: &App| {
            let mark = project_mark(tab.initial.clone(), p.surface(6)).size(metrics.icon);
            let close = if model.hovered.as_ref() == Some(&tab.id) || tab.active {
                Some(ctx.action(format!("close-{}", tab.id), IconName::XBold, format!("Close tab {}", tab.title), SidebarIntent::Browse(BrowseIntent::CloseTab(tab.id.clone())), cx))
            } else {
                None
            };
            let row = ctx.row(ctx.lead(mark, cx), tab.title.clone(), close, nested, cx);
            let label: SharedString = format!("Tab {}", tab.title).into();
            record_ax_label(&label);
            let built = row.id(ctx.child(format!("tab-{}", tab.id))).role(gpui::Role::Tab).aria_label(label);
            *list = Some(list.take().expect("list").item(index, built));
        };
        let mut slot = Some(list);
        let mut selected_index: Option<usize> = None;
        for tab in &browse.tabs {
            let index = BROWSE_BASE + next;
            tab_row(ctx, &mut slot, index, tab, false, cx);
            targets.insert(index, Target::Intent(SidebarIntent::Browse(BrowseIntent::SelectTab(tab.id.clone()))));
            index_of_session.insert(tab.id.clone(), index);
            if tab.active {
                selected_index = Some(index);
            }
            next += 1;
        }
        for folder in &browse.folders {
            let turn = spring_phase((ctx.child(format!("folder-chevron-{}", folder.name)), "turn"), folder.open, SpringKind::Swap, window, cx);
            let chevron = icon(IconName::ChevronDown).size(metrics.icon).color(p.fg_muted).rotate(radians(-std::f32::consts::FRAC_PI_2 * (1.0 - turn)));
            let head = ctx.row(ctx.lead(chevron, cx), folder.name.clone(), Some(ctx.trailing(folder.tabs.len().to_string(), cx)), false, cx);
            let label: SharedString = format!("Folder {}", folder.name).into();
            record_ax_label(&label);
            let index = BROWSE_BASE + next;
            slot = Some(slot.take().expect("list").item(index, head.id(ctx.child(format!("folder-{}", folder.name))).role(gpui::Role::Button).aria_label(label)));
            targets.insert(index, Target::Intent(SidebarIntent::Browse(BrowseIntent::ToggleFolder(folder.name.clone()))));
            next += 1;
            if folder.open {
                for tab in &folder.tabs {
                    let index = BROWSE_BASE + next;
                    tab_row(ctx, &mut slot, index, tab, true, cx);
                    targets.insert(index, Target::Intent(SidebarIntent::Browse(BrowseIntent::SelectTab(tab.id.clone()))));
                    index_of_session.insert(tab.id.clone(), index);
                    if tab.active {
                        selected_index = Some(index);
                    }
                    next += 1;
                }
            }
        }
        list = slot.take().expect("list");
        // New tab: the one filled row, below the tabs.
        let new_tab = ctx.row(ctx.lead_icon(IconName::Plus, cx), "New tab", Some(ctx.chip("⌘T", cx)), false, cx).mt(metrics.item_px);
        record_ax_label("New tab");
        let index = BROWSE_BASE + next;
        list = list.item(index, surface(2, new_tab.id(ctx.child("new-tab")).role(gpui::Role::Button).aria_label("New tab")).shape(Shape::Item).ring(true));
        targets.insert(index, Target::Intent(SidebarIntent::Browse(BrowseIntent::NewTab)));
        next += 1;
        // A spacer, then the spaces above the footer.
        list = list.item_static_fill(next, div().w_full().flex_1().min_h(px(0.0)));
        next += 1;
        let mut chips = h_flex().w_full().items_center().gap(px(scale::SP_2)).py(metrics.item_px);
        for (i, space) in browse.spaces.iter().enumerate() {
            let on = i == browse.space;
            let chip = div()
                .h(metrics.control_sm)
                .px(metrics.item_px + px(scale::SP_1))
                .flex()
                .items_center()
                .role_in(TextRole::Caption, cx)
                .text_color(if on { p.fg } else { p.fg_muted })
                .child(space.clone());
            let chip: AnyElement = if on { surface(3, chip).shape(Shape::Pill).into_any_element() } else { chip.into_any_element() };
            let c = ctx.clone();
            chips = chips.child(
                interactive(ctx.child(format!("space-{i}")), chip)
                    .role("tab")
                    .shape(Shape::Pill)
                    .selected(false)
                    .label(format!("Space {space}"))
                    .on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        c.emit(SidebarIntent::Browse(BrowseIntent::SwitchSpace(i)), w, cx);
                    }),
            );
        }
        chips = chips.child(div().flex_1()).child(ctx.action("new-space".into(), IconName::Plus, "New space".into(), SidebarIntent::Browse(BrowseIntent::NewSpace), cx));
        list = list.item_static(next, chips);
        if let Some(index) = selected_index {
            list = list.selected(Some(index));
        }
    }

    // Selection and keyboard highlight, translated to indices.
    if model.mode == Mode::Sessions {
        let selected = model.selected.as_ref().and_then(|id| index_of_session.get(id).copied());
        list = list.selected(selected);
    }
    let highlighted = model.highlighted.as_ref().and_then(|id| index_of_session.get(id).copied());
    if highlighted.is_some() {
        list = list.highlighted(highlighted);
    }
    let targets = Rc::new(targets);
    // Hover: the FluidList's highlight is the one source; sessions report
    // their id, chrome rows report their key, nothing reports None twice.
    let hover_targets = targets.clone();
    let hover_ctx = ctx.clone();
    list = list.on_highlight(move |index: Option<usize>, w: &mut Window, cx: &mut App| {
        let id = index.and_then(|i| match hover_targets.get(&i) {
            Some(Target::Session(id)) => Some(id.clone()),
            Some(Target::Intent(SidebarIntent::Search)) => Some("search".into()),
            Some(Target::Intent(SidebarIntent::NewSession)) => Some("new-session".into()),
            Some(Target::Intent(SidebarIntent::Browse(BrowseIntent::SelectTab(id)))) => Some(id.clone()),
            _ => None,
        });
        hover_ctx.emit(SidebarIntent::Hover(id), w, cx);
    });
    let activate_targets = targets.clone();
    let activate_ctx = ctx.clone();
    list = list.on_activate(move |index: usize, w: &mut Window, cx: &mut App| match activate_targets.get(&index) {
        Some(Target::Session(id)) => activate_ctx.emit(SidebarIntent::Select(id.clone()), w, cx),
        Some(Target::Intent(intent)) => activate_ctx.emit(intent.clone(), w, cx),
        None => {}
    });
    (list.into_any_element(), targets)
}

/// What activating a FluidList index means.
#[derive(Clone)]
enum Target {
    Session(SharedString),
    Intent(SidebarIntent),
}

/// The virtualised list, filling its FluidList slot.
fn list_host(ctx: &Ctx, render: impl Fn(usize, &mut Window, &mut App) -> AnyElement + 'static) -> AnyElement {
    div()
        .id(ctx.child("virtual"))
        .w_full()
        .h_full()
        .flex_1()
        .min_h(px(0.0))
        .child(list(ctx_state(ctx), render).w_full().h_full())
        .into_any_element()
}

fn ctx_state(ctx: &Ctx) -> ListState {
    ctx.state.clone()
}

impl RenderOnce for SidebarScreen {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        if self.model.collapsed {
            return collapsed_strip(&self, cx).into_any_element();
        }
        let ctx = Ctx {
            id: self.id.clone(),
            model: Rc::new(self.model.clone()),
            fluid: self.fluid.clone(),
            rename_input: self.rename_input.clone(),
            on_intent: self.on_intent.clone(),
            state: self.state.clone(),
        };

        // Title bar: traffic-light gap, the toggle at x 84, the mode switch
        // at the right. `bar + item_px` = 52.
        let toggle_ctx = ctx.clone();
        let toggle = icon_button(ctx.child("toggle"), IconName::Sidebar)
            .ghost()
            .size(ButtonSize::Sm)
            .icon_size(metrics.icon)
            .accessibility_label("Hide sidebar")
            .on_click(move |_, w, cx| toggle_ctx.emit(SidebarIntent::Collapse, w, cx));
        let mode = |ctx: &Ctx, key: &str, glyph: IconName, label: &str, target: Mode| {
            let on = ctx.model.mode == target;
            let c = ctx.clone();
            let glyph_el = div()
                .w(metrics.control_md)
                .h(metrics.control_xs)
                .flex()
                .items_center()
                .justify_center()
                .text_color(if on { p.fg } else { p.fg_muted })
                .child(icon(glyph).size(metrics.icon).color(if on { p.fg } else { p.fg_muted }));
            let body: AnyElement = if on { surface(4, glyph_el).shape(Shape::Item).shadow(true).into_any_element() } else { glyph_el.into_any_element() };
            interactive(ctx.child(format!("mode-{key}")), body)
                .role("tab")
                .label(label.to_string())
                .selected(on)
                .on_click(move |_, w, cx| c.emit(SidebarIntent::SwitchMode(target), w, cx))
        };
        record_ax_label("Sidebar mode");
        let switch = surface(
            2,
            h_flex()
                .id(ctx.child("modes"))
                .p(px(scale::SP_1))
                .role(gpui::Role::TabList)
                .aria_label("Sidebar mode")
                .child(mode(&ctx, "sessions", IconName::List, "Sessions", Mode::Sessions))
                .child(mode(&ctx, "browse", IconName::Globe, "Browse", Mode::Browse)),
        )
        .shape(Shape::Item);
        let title_bar = h_flex()
            .w_full()
            .flex_none()
            .h(metrics.bar + metrics.item_px)
            .items_center()
            .pl(traffic_light_gap())
            .pr(px(scale::SP_4))
            .gap(px(scale::SP_2 + scale::SP_1))
            .child(toggle)
            .child(div().flex_1())
            .child(switch);

        let (rows, _targets) = build_list(&ctx, window, cx);
        let body = div().w_full().flex_1().min_h(px(0.0)).flex().flex_col().px(metrics.item_px).relative().child(rows);

        // Footer: hairline, then avatar · name · plan · chevron.
        let account = &self.model.account;
        let who = h_flex()
            .flex_1()
            .min_w(px(0.0))
            .truncate()
            .text_role_in(TextRole::Body, Density::Compact)
            .child(div().text_color(p.fg).child(account.name.clone()))
            .child(div().ml(px(scale::SP_2)).text_color(p.fg_muted).child(format!("· {}", account.plan)));
        let foot_ctx = ctx.clone();
        let foot_label: SharedString = format!("Account {} · {}", account.name, account.plan).into();
        let foot_row = h_flex()
            .w_full()
            .h(list_row_height(cx))
            .items_center()
            .gap(metrics.gap)
            .pl(px(scale::SP_2 + scale::SP_1))
            .pr(metrics.item_px)
            .child(project_mark(account.initial.clone(), p.label(account.tint)).size(metrics.icon + px(scale::SP_2)))
            .child(who)
            .child(icon(IconName::ChevronDown).size(metrics.icon).color(p.fg_muted).rotate(radians(std::f32::consts::PI)));
        let footer = v_flex()
            .w_full()
            .flex_none()
            .child(div().h(px(1.0)).w_full().bg(p.line))
            .child(
                div().p(metrics.item_px).child(
                    interactive(ctx.child("footer"), foot_row)
                        .label(foot_label)
                        .on_click(move |_, w, cx| foot_ctx.emit(SidebarIntent::Account, w, cx)),
                ),
            );

        let mut column = v_flex().w(self.width).h_full().relative().bg(p.bg).child(title_bar).child(body).child(footer);

        // Menus, positioned from the ladder: the view-options menu under
        // the Projects header, the account menu above the footer.
        if self.model.view_menu_open {
            let top = projects_header_bottom(&self.model, cx);
            column = column.child(
                div()
                    .absolute()
                    .left(metrics.item_px)
                    .right(metrics.item_px)
                    .top(top)
                    .child(view_options_menu(ctx.child("view-menu"), self.on_intent.clone(), cx)),
            );
        }
        if account.menu_open {
            column = column.child(
                div()
                    .absolute()
                    .left(metrics.item_px)
                    .right(metrics.item_px)
                    .bottom(list_row_height(cx) + metrics.item_px * 2.0 + px(1.0) + metrics.item_px)
                    .child(account_menu(ctx.child("account-menu"), account, self.on_intent.clone(), cx)),
            );
        }
        column.into_any_element()
    }
}

/// Where the Projects header ends, measured down the ladder from the top
/// of the sidebar: the menu that drops from it starts here.
fn projects_header_bottom(model: &SidebarModel, cx: &App) -> Pixels {
    let metrics = cx.aui().metrics;
    let row = list_row_height(cx);
    let mut y = metrics.bar + metrics.item_px; // title bar
    y += row; // search
    y += row; // new session
    y += row * model.nav.len() as f32;
    if !model.pinned.is_empty() {
        y += px(scale::SP_4) + metrics.control_md;
        if model.pinned_open {
            for session in &model.pinned {
                let two = session.status.is_some() || (model.bylines == Bylines::All && session.byline.is_some());
                y += if two { row + metrics.inset } else { row };
            }
        }
    }
    y + px(scale::SP_4) + metrics.control_md
}

/// The collapsed state: only the 12 px edge strip, one labelled control
/// that expands the sidebar. The toggle itself lives in the window's
/// title strip beside the traffic lights, drawn by the host.
fn collapsed_strip(screen: &SidebarScreen, cx: &App) -> impl IntoElement {
    let p = cx.aui().colors;
    let handler = screen.on_intent.clone();
    let strip = div().w(px(scale::SP_4)).h_full().bg(p.bg);
    let mut control = interactive((screen.id.clone(), "strip"), strip).label("Show sidebar").pressable(false);
    if let Some(emit) = handler {
        control = control.on_click(move |_, w, cx| emit(SidebarIntent::Collapse, w, cx));
    }
    div().h_full().flex().child(control)
}
