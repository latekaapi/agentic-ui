//! The sidebar screen, built to the approved mock: title bar, one
//! [`RowList`](crate::interact::RowList) holding every row (search, new session, nav, pinned,
//! projects with their virtualised sessions — or Browse mode's address,
//! controls, pins, tabs, folders and spaces), and the footer.
//!
//! The measurements are docs/83 §4's, in tokens (default density):
//!
//! - list rows are `control_lg` tall (32; 28 compact) with a `ROW_GAP` (2)
//!   between them, a pitch of 34; the two-line option is `row_2` (52). The
//!   sidebar is 256 wide with a `list-inset` gutter (8) on both sides, so
//!   rows span 8–248 and the highlight block has the item radius (8);
//! - every row pads `ROW_PAD_X` (10) inside its block: a glyph sits at 18
//!   with its label one `gap` after it (42); a session title has no leading
//!   glyph in any state and starts at 18;
//! - a session row's trailing 20 px slot shows exactly one of: a spinner
//!   (running), the amber dot (needs you), the `accent-ring` dot (finished,
//!   unread), the danger dot (failed) — or, under the pointer, the `⋯` row
//!   menu with the age left of it. Idle and read shows nothing;
//! - group labels (Pinned, Projects, each project, each date bucket) are
//!   Caption, `fg-muted`, sentence case, 28 tall, label at 18; a project's
//!   chevron follows its name while hovered or closed, its actions appear
//!   at the right on hover;
//! - row titles are Body in `fg`, regular weight; selection is the fill
//!   alone. Colour in a row means state, never decoration.
//!
//! The screen is stateless: `model` is plain data, `state` the
//! caller-owned [`ListState`] sized with [`screen_item_count`]. Rows hover
//! plainly with the one model ([`crate::interact::paint`]). Sync the list
//! state like
//! [`virtual_sidebar_view`](crate::nav::virtual_sidebar_view): after a
//! regroup `state.reset(count)`, after an insert/remove `state.splice`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::base::ElementExt as _;

use aui_icons::{icon, IconName};
use aui_motion::{spring_phase, SpringKind};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, TextRole};
use gpui::{
    div, list, prelude::*, px, radians, AnyElement, App, ElementId, Entity, IntoElement, ListState, Pixels, RenderOnce,
    SharedString, Window,
};
use gpui_kit::base::{
    h_flex,
    input::{Input, InputState, Textarea, TextareaState},
    v_flex,
};

use crate::data::{icon_button, record_ax_label, spinner, ButtonSize};
use crate::interact::{interactive, row_list, surface, ListItems, RowList, Shape, StatePaint, ROW_GAP, ROW_PAD_X};
use crate::nav::{flatten_sidebar, Grouping, GroupAction, RowAction, SessionSummary, SidebarRow};

use super::menus::{account_menu, avatar_tile, session_menu, view_options_menu};
use super::{AccountAction, AccountStatus, BrowseIntent, BrowseMenu, BrowseMenuTarget, BrowseNaming, Bylines, FolderKind, Mode, SidebarIntent, SidebarModel};

/// The sidebar's width beside the pane: Fluid Functionalism's 16rem
/// (`sidebar-core.tsx`: `SIDEBAR_WIDTH = "16rem"`), owner 2026-10-11.
pub const SIDEBAR_WIDTH: f32 = 256.0;

/// RowList indices: chrome rows count up from 0, pinned rows from
/// `PINNED_BASE`, virtualised session rows from `VIRTUAL_BASE` (plus their
/// flattened index), Browse rows from `BROWSE_BASE`.
const PINNED_BASE: usize = 100;
const VIRTUAL_BASE: usize = 1000;
const BROWSE_BASE: usize = 500;
/// The task pane's rows (after Browse's scrolling rows, before the virtual
/// sessions): their own clip range.
const TASK_BASE: usize = 800;
/// The Pinned label's hover key (a reserved chrome key, like `"search"`).
const PINNED_KEY: &str = "pinned-head";


type SwitchHandler = Rc<dyn Fn(Mode, &mut Window, &mut App)>;

/// The Sessions | Browse switch: FF tabs (`registry/base/tabs.tsx`) at the
/// icon size — a track with a 2 px inset and ONE indicator that slides
/// between the two glyphs on the moderate spring. v3 (dark): the track is the
/// hover overlay, the indicator a flat Floating fill; light: a surface-2
/// track and a surface-4 indicator, no shadow in either theme. The
/// sidebar's title bar and the closed state's pill draw the same switch.
#[derive(IntoElement)]
pub struct ModeSwitch {
    id: ElementId,
    mode: Mode,
    on_switch: Option<SwitchHandler>,
}

/// A mode switch showing `mode`.
pub fn mode_switch(id: impl Into<ElementId>, mode: Mode) -> ModeSwitch {
    ModeSwitch { id: id.into(), mode, on_switch: None }
}

impl ModeSwitch {
    /// Called with the mode a click picks.
    pub fn on_switch(mut self, f: impl Fn(Mode, &mut Window, &mut App) + 'static) -> Self {
        self.on_switch = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ModeSwitch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let id = self.id;
        let item_w = metrics.control_md;
        let item_h = metrics.control_xs;
        let browse_on = spring_phase((id.clone(), "slide"), self.mode == Mode::Browse, SpringKind::Moderate, window, cx);
        let indicator = div()
            .absolute()
            .top(px(scale::SP_1))
            .left(px(scale::SP_1) + item_w * browse_on)
            .w(item_w)
            .h(item_h)
            .child(surface(if p.is_dark() { aui_tokens::Palette::FLOATING_LEVEL } else { 4 }, div().size_full()).shape(Shape::Item));
        let mode = |key: &str, glyph: IconName, label: &str, target: Mode| {
            let on = self.mode == target;
            let pick = self.on_switch.clone();
            let glyph_el = div()
                .w(item_w)
                .h(item_h)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(glyph).size(metrics.icon).color(if on { p.fg } else { p.fg_muted }).lit(on));
            interactive(gpui::ElementId::NamedChild(std::sync::Arc::new(id.clone()), format!("mode-{key}").into()), glyph_el)
                .role("tab")
                .label(label.to_string())
                .selected(on)
                .on_click(move |_, w, cx| {
                    if let Some(pick) = &pick {
                        pick(target, w, cx)
                    }
                })
        };
        record_ax_label("Sidebar mode");
        let track = h_flex()
            .id(id.clone())
            .relative()
            .p(px(scale::SP_1))
            .role(gpui::Role::TabList)
            .aria_label("Sidebar mode")
            .child(indicator)
            .child(mode("sessions", IconName::List, "Sessions", Mode::Sessions))
            .child(mode("browse", IconName::Globe, "Browse", Mode::Browse));
        if p.is_dark() {
            Shape::Item.rounded(track.bg(p.hover_overlay())).into_any_element()
        } else {
            surface(2, track).shape(Shape::Item).into_any_element()
        }
    }
}

type IntentHandler = Rc<dyn Fn(SidebarIntent, &mut Window, &mut App)>;

/// The live editor behind the rename field: a one-line input, or a
/// host's one-row textarea (Baaz renames through the same state its
/// header field edits).
#[derive(Clone)]
enum RenameInput {
    Input(Entity<InputState>),
    Textarea(Entity<TextareaState>),
}

impl RenameInput {
    fn value(&self, cx: &App) -> SharedString {
        match self {
            RenameInput::Input(input) => input.read(cx).value().to_string().into(),
            RenameInput::Textarea(input) => input.read(cx).value().to_string().into(),
        }
    }

    fn element(&self) -> AnyElement {
        match self {
            RenameInput::Input(input) => Input::new(input).into_any_element(),
            RenameInput::Textarea(input) => Textarea::new(input).into_any_element(),
        }
    }
}


/// The list row height at the active density: `control_lg` (32 / 28).
pub fn list_row_height(cx: &App) -> Pixels {
    cx.aui().metrics.control_lg
}

/// How far gpui-base's `Textarea` draws its text in from its own left edge
/// (measured live; the inline rename field compensates for it).
const TEXTAREA_TEXT_INSET: f32 = 10.0;

/// A session row's height: one line, or `row_2` (52) with the summary line.
fn session_row_height(two_lines: bool, cx: &App) -> Pixels {
    if two_lines {
        cx.aui().metrics.row_2
    } else {
        list_row_height(cx)
    }
}

/// A group label's height (docs/83 §4: 28).
fn label_height(cx: &App) -> Pixels {
    cx.aui().metrics.control_md
}

/// How many virtual rows [`sidebar_screen`] builds for `model`: the
/// flattened project grouping (four skeleton rows while loading, none in
/// Browse mode). Size the caller-owned [`ListState`] with this before
/// rendering.
///
/// The v2 sidebar renders [`Grouping::Project`] (projects with nested
/// sessions) and [`Grouping::Date`] (date headers over flat rows); a Status
/// grouping counts as empty here. Pinned sessions belong in
/// [`SidebarModel::pinned`]: a Date grouping handed in with pinned rows still
/// inside it would flatten a second Pinned header, so hosts lift them first.
pub fn screen_item_count(model: &SidebarModel) -> usize {
    if model.loading {
        4
    } else if model.mode == Mode::Browse {
        0
    } else {
        match model.grouping.as_ref() {
            Grouping::Project(_) | Grouping::Date(_) => flatten_sidebar(&model.grouping, false).len(),
            Grouping::Status(_) => 0,
        }
    }
}

/// The index ranges the list's items are spread over; they must not overlap,
/// since an index names one item for activation and the pointer report.
const _: () = assert!(PINNED_BASE < BROWSE_BASE && BROWSE_BASE < TASK_BASE && TASK_BASE < VIRTUAL_BASE);

/// The task pane's cap: six rows, so the person's own tabs keep the room.
fn task_pane_max(cx: &App) -> Pixels {
    list_row_height(cx) * 6.0 + cx.aui().metrics.item_px
}

/// The sidebar screen for `model`, virtualised through `state`.
///
/// The host creates `state` ([`crate::nav::sidebar_list_state`]) ONCE per
/// sidebar and keeps it across frames, like any `ListState`.
///
/// ```ignore
/// let state = sidebar_list_state(screen_item_count(&model)); // once
/// sidebar_screen("sidebar", model, state.clone()).on_intent(|intent, _, _| { /* host acts */ })
/// ```
pub fn sidebar_screen(id: impl Into<ElementId>, model: SidebarModel, state: ListState) -> SidebarScreen {
    SidebarScreen { id: id.into(), model, state, width: px(SIDEBAR_WIDTH), rename_input: None, on_intent: None, ground: true, external_menus: false }
}

/// The sidebar screen. Build with [`sidebar_screen`].
#[derive(IntoElement)]
pub struct SidebarScreen {
    id: ElementId,
    model: SidebarModel,
    state: ListState,
    width: Pixels,
    rename_input: Option<RenameInput>,
    on_intent: Option<IntentHandler>,
    ground: bool,
    external_menus: bool,
}

impl SidebarScreen {

    /// The host draws Browse's right-click menu itself
    /// ([`super::browse_menu`]), e.g. in an overlay window over a native
    /// page, so the screen does not.
    pub fn external_menus(mut self, external: bool) -> Self {
        self.external_menus = external;
        self
    }

    /// Draws no ground of its own: for a host that embeds the sidebar on
    /// a column or surface that paints its own fill (the shell's sidebar
    /// column, the collapsed peek). By default the sidebar paints the
    /// `sidebar` fill.
    pub fn transparent(mut self) -> Self {
        self.ground = false;
        self
    }

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
        self.rename_input = Some(RenameInput::Input(input.clone()));
        self
    }

    /// Like [`Self::rename_input`], for a host whose rename state is a
    /// one-row textarea.
    pub fn rename_textarea(mut self, input: &Entity<TextareaState>) -> Self {
        self.rename_input = Some(RenameInput::Textarea(input.clone()));
        self
    }
}

/// Everything a row builder needs, cloned into the virtual list's render
/// closure.
#[derive(Clone)]
struct Ctx {
    id: ElementId,
    model: Rc<SidebarModel>,
    /// The list's per-item wrapping, for rows built outside its children
    /// (the virtual rows, Browse's scrolling rows).
    items: ListItems,
    /// What each list index activates; filled while the list is built.
    targets: Rc<RefCell<HashMap<usize, Target>>>,
    state: ListState,
    rename_input: Option<RenameInput>,
    on_intent: Option<IntentHandler>,
    /// The column's width: tooltips stay inside it, since the native page
    /// beside it draws above anything that crosses (owner 2026-10-11).
    width: Pixels,
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

    /// A row's leading glyph box: one icon square, content centred. Rows
    /// pad `ROW_PAD_X` before it, so glyphs sit at 18 and labels at 42.
    fn lead(&self, content: impl IntoElement, cx: &App) -> AnyElement {
        let metrics = cx.aui().metrics;
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(metrics.icon)
            .text_color(cx.aui().colors.fg_muted)
            .child(content)
            .into_any_element()
    }

    /// A glyph in the leading slot: muted at rest, foreground when its row
    /// is lit (FF: icons and labels light together on hover / selection).
    fn lead_icon(&self, name: IconName, cx: &App) -> AnyElement {
        self.lead_icon_lit(name, false, cx)
    }

    fn lead_icon_lit(&self, name: IconName, lit: bool, cx: &App) -> AnyElement {
        let metrics = cx.aui().metrics;
        let p = cx.aui().colors;
        self.lead(icon(name).size(metrics.icon).color(if lit { p.fg } else { p.fg_muted }).lit(lit), cx)
    }

    /// Whether the chrome row `key` is lit: the pointer-driven highlight sits
    /// on it.
    fn lit(&self, key: &str) -> bool {
        self.model.hovered.as_deref() == Some(key)
    }

    /// A tab's mark: its favicon image when the host has one (the icon
    /// size, square, as sites draw them), else a favicon-style tile — a
    /// `surface-3` square on the item radius with the initial in `fg-muted`
    /// (the mock's `.mark`).
    fn favicon(&self, initial: SharedString, image: Option<&SharedString>, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        if let Some(path) = image {
            return gpui::img(std::path::PathBuf::from(path.as_ref())).size(metrics.icon).flex_none().into_any_element();
        }
        Shape::Item
            .rounded(
                div()
                    .size(metrics.icon)
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(p.surface(3))
                    .role_in(TextRole::Micro, cx)
                    .semibold()
                    .text_color(p.fg_muted)
                    .child(initial),
            )
            .into_any_element()
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
            .text_role(TextRole::Body)
            .child(text.into())
    }

    /// A row's frame: height, gap, the `ROW_PAD_X` inside its block and
    /// the leading glyph if any — no title slot, so a skeleton bar sits
    /// right after the padding instead of past a flexing empty title.
    /// `nested` indents one `inset` (Browse's folder tabs). Labels are `fg`
    /// (docs/83 §4).
    fn row_frame(&self, lead: Option<AnyElement>, nested: bool, cx: &App) -> gpui::Div {
        let metrics = cx.aui().metrics;
        h_flex()
            .w_full()
            .h(list_row_height(cx))
            .items_center()
            .gap(metrics.gap)
            .pl(px(ROW_PAD_X) + if nested { metrics.inset } else { px(0.0) })
            .pr(px(ROW_PAD_X))
            .text_color(cx.aui().colors.fg)
            .children(lead)
    }

    /// A one-line list row: leading glyph, title, trailing.
    fn row(&self, lead: Option<AnyElement>, title: impl Into<SharedString>, trailing: Option<AnyElement>, nested: bool, cx: &App) -> gpui::Div {
        let mut row = self.row_frame(lead, nested, cx).child(self.title(title, cx));
        if let Some(trailing) = trailing {
            row = row.child(trailing);
        }
        row
    }

    /// A section title: a group label (`control_md` tall, Caption muted,
    /// sentence case, no rule) with `SP_4` above and its label at 18;
    /// optional count (Browse's Today) and hover actions.
    fn section(&self, label: &str, count: Option<SharedString>, actions: Option<AnyElement>, cx: &App) -> gpui::Div {
        let p = cx.aui().colors;
        let mut head = h_flex()
            .w_full()
            .h(label_height(cx))
            .mt(px(scale::SP_4))
            .items_center()
            .pl(px(ROW_PAD_X))
            .pr(px(scale::SP_2))
            .role_in(TextRole::Caption, cx)
            .text_color(p.fg_muted)
            .child(div().child(label.to_string()));
        if let Some(count) = count {
            head = head.child(div().ml(px(scale::SP_2 + scale::SP_1)).text_color(p.fg_muted).child(count));
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
        self.menu_action(key, glyph, label, intent, false, cx)
    }

    /// [`Self::action`] for a button that opens a menu: while `open` it
    /// holds its pressed look and drops its tooltip (FF `data-popup-open`).
    fn menu_action(&self, key: String, glyph: IconName, label: String, intent: SidebarIntent, open: bool, cx: &App) -> AnyElement {
        let metrics = cx.aui().metrics;
        let ctx = self.clone();
        icon_button(self.child(key), glyph)
            .menu_open(open)
            .tooltip_within(self.width)
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

    /// The second line a session row shows, or none.
    fn second_line(&self, session: &SessionSummary) -> Option<SharedString> {
        second_line_for(session, self.model.bylines)
    }

    /// One session row: the title (and, with the summary line on, its
    /// byline) from the row's padding, no leading glyph in any state, and
    /// the trailing state slot. Selection and hover paint come from the row
    /// list's item box, never from the row; the title keeps its weight when
    /// selected (docs/83 §4: selection is the fill).
    fn session_row(&self, session: &SessionSummary, window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let id = session.id.clone();
        if let Some(editing) = self.model.editing.as_ref().filter(|e| e.session == id) {
            return self.rename_field(session, &editing.draft, window, cx);
        }
        let second = self.second_line(session);
        let title = div().truncate().text_role(TextRole::Body).text_color(p.fg).child(session.name.clone());
        let mut text = v_flex().flex_1().min_w(px(0.0)).justify_center().child(title);
        if let Some(line) = second.clone() {
            text = text.child(div().truncate().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(line));
        }
        let label: SharedString = format!("Open session {}", session.name).into();
        record_ax_label(&label);
        h_flex()
            .id(self.child(format!("session-{id}")))
            .w_full()
            .h(session_row_height(second.is_some(), cx))
            .items_center()
            .gap(metrics.gap)
            .pl(px(ROW_PAD_X))
            .pr(px(scale::SP_2))
            .role(gpui::Role::ListItem)
            .aria_label(label)
            .child(text)
            .child(self.state_slot(session, window, cx))
            .into_any_element()
    }

    /// A session row's trailing slot (docs/83 §4): exactly one of a spinner
    /// (running), the amber dot (needs you), the `accent-ring` dot
    /// (finished, unread), the danger dot (failed) — or, while the pointer
    /// is on the row or its menu is open, the `⋯` menu with the age (Micro,
    /// `ink-4`) left of it. Idle and read shows nothing, and the slot keeps
    /// its 20 px either way so titles truncate at one edge.
    fn state_slot(&self, session: &SessionSummary, _window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let id = session.id.clone();
        let slot = metrics.control_xs;
        let menu_open = self.model.row_menu.as_ref() == Some(&id);
        let hovered = self.model.hovered.as_ref() == Some(&id);
        if hovered || menu_open {
            let ctx = self.clone();
            let more = id.clone();
            let mut wrap = div().relative().flex_none().size(slot).child(
                icon_button(self.child(format!("{id}-more")), IconName::Dots)
                    .menu_open(menu_open)
                    .tooltip_within(self.width)
                    .ghost()
                    .size(ButtonSize::Xs)
                    .icon_size(metrics.icon - px(scale::SP_1))
                    .accessibility_label(format!("Options for session {}", session.name))
                    .on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        ctx.emit(SidebarIntent::RowAction(more.clone(), RowAction::More), w, cx);
                    }),
            );
            if menu_open {
                // Under the `⋯`, right edges aligned, on gpui's top layer so
                // the list's clip and the sidebar's edge never cut it.
                let c = self.clone();
                let menu = div()
                    .id(self.child(format!("{id}-menu-wrap")))
                    .mt(px(scale::SP_1))
                    .occlude()
                    .on_mouse_down_out(move |_, w, cx| c.emit(SidebarIntent::DismissMenus, w, cx))
                    .child(session_menu(self.child(format!("{id}-menu")), session, self.on_intent.clone()));
                wrap = wrap.child(
                    div()
                        .absolute()
                        .top(slot)
                        .right(px(0.0))
                        .child(gpui::deferred(gpui::anchored().anchor(gpui::Anchor::TopRight).snap_to_window().child(menu)).with_priority(1)),
                );
            }
            return h_flex()
                .flex_none()
                .items_center()
                .gap(px(scale::SP_2))
                .when(!session.elapsed.is_empty(), |d| d.child(div().flex_none().role_in(TextRole::Micro, cx).text_color(p.ink_4).child(session.elapsed.clone())))
                .child(wrap)
                .into_any_element();
        }
        let dot = |colour: gpui::Hsla| Shape::Pill.rounded(div().size(px(scale::SP_2 + scale::SP_1)).bg(colour)).into_any_element();
        let (mark, said): (Option<AnyElement>, &str) = match session.state {
            AgentState::Running => (Some(spinner(self.child(format!("spin-{id}"))).size(metrics.icon - px(scale::SP_2)).muted(cx).into_any_element()), "Running"),
            AgentState::Waiting => (Some(dot(p.warning)), "Needs you"),
            AgentState::Failed => (Some(dot(p.danger)), "Failed"),
            AgentState::Idle | AgentState::Done if session.unread => (Some(dot(p.accent_ring)), "Unread"),
            AgentState::Idle | AgentState::Done => (None, ""),
        };
        let holder = div().flex_none().size(slot).flex().items_center().justify_center();
        match mark {
            Some(mark) => {
                record_ax_label(said);
                holder.id(self.child(format!("state-{id}"))).role(gpui::Role::Status).aria_label(SharedString::from(said)).child(mark).into_any_element()
            }
            None => holder.into_any_element(),
        }
    }

    /// The inline rename field (owner 2026-10-11): it stands in the row's
    /// own pill rect — the row's height less the 1 px block inset, the full
    /// row width — as FF's focused InputField (`bg-card` = surface-3,
    /// `ring-border`, item radius, no focus ring), with no state slot and no
    /// hover or selected block under it (the row is the list's `inert`
    /// item). The text sits on the titles' axis, so nothing jumps; the live
    /// input when the host gives one.
    fn rename_field(&self, session: &SessionSummary, draft: &SharedString, _window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let height = session_row_height(self.second_line(session).is_some(), cx);
        let pill = px(scale::SP_1 / 2.0);
        let hair = px(1.0);
        // The titles' x: the row's padding (no leading glyph).
        let axis = px(ROW_PAD_X);
        let label: SharedString = format!("Rename session {}", session.name).into();
        record_ax_label(&label);
        let body: AnyElement = match self.rename_input.as_ref() {
            // gpui-base's textarea draws its text 10 px in from its own
            // left edge (measured live 2026-10-11 with a marker on the
            // container: container at the titles' axis, caret 10 px right
            // of it); pull it back so the text sits on that axis.
            Some(input @ RenameInput::Textarea(_)) => div().flex_1().min_w(px(0.0)).ml(-px(TEXTAREA_TEXT_INSET)).child(input.element()).into_any_element(),
            Some(input) => div().flex_1().min_w(px(0.0)).child(input.element()).into_any_element(),
            None => h_flex()
                .flex_1()
                .min_w(px(0.0))
                .items_center()
                .text_color(p.fg)
                .child(div().truncate().text_role(TextRole::Body).child(draft.clone()))
                .child(div().flex_none().w(px(1.0)).h(metrics.icon).bg(p.fg))
                .into_any_element(),
        };
        let boxed = Shape::Item.rounded(
            h_flex()
                .w_full()
                .h(height - pill * 2.0)
                .items_center()
                .pl(axis - hair)
                .pr(metrics.item_px - hair)
                .bg(p.surface(3))
                .border_1()
                .border_color(p.line)
                .text_color(p.fg)
                .text_role(TextRole::Body)
                .child(body),
        );
        let ctx = self.clone();
        let commit = session.id.clone();
        let text = draft.clone();
        let live = self.rename_input.clone();
        div()
            .id(self.child(format!("rename-{}", session.id)))
            .w_full()
            .h(height)
            .flex()
            .items_center()
            .role(gpui::Role::TextInput)
            .aria_label(label)
            // Keys bubble up from the focused input inside; Enter commits
            // the input's live text (the model draft when there is none),
            // Escape abandons. A static field (no input) takes no keys.
            .on_key_down(move |event: &gpui::KeyDownEvent, w: &mut Window, cx: &mut App| match event.keystroke.key.as_str() {
                "enter" => {
                    let value: SharedString = match live.as_ref() {
                        Some(input) => input.value(cx),
                        None => text.clone(),
                    };
                    ctx.emit(SidebarIntent::Rename(commit.clone(), value), w, cx);
                }
                "escape" => ctx.emit(SidebarIntent::RenameCancel, w, cx),
                _ => {}
            })
            // A click in the field stays in the field: never a row activation.
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(boxed)
            .into_any_element()
    }

    /// A Browse name field (a folder being named): the row's geometry on
    /// `surface-2` with the focus ring, the live input when the host gives
    /// one; Enter reports [`BrowseIntent::Name`], Escape
    /// [`BrowseIntent::NameCancel`].
    #[allow(clippy::too_many_arguments)]
    fn name_field(&self, key: String, lead: AnyElement, draft: &SharedString, target: BrowseNaming, label: &str, nested: bool, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        record_ax_label(label);
        let body = self.field_body(draft, cx);
        let field = h_flex().w_full().h(list_row_height(cx)).items_center().gap(metrics.gap).pl(px(ROW_PAD_X)).pr(px(ROW_PAD_X)).child(lead).child(body);
        // The rename field's chrome (FF focused InputField), not the accent.
        let boxed = Shape::Item.rounded(div().w_full().bg(p.surface(3)).border_1().border_color(p.line).child(field));
        div()
            .id(self.child(key))
            .w_full()
            .when(nested, |d| d.pl(metrics.inset))
            .role(gpui::Role::TextInput)
            .aria_label(SharedString::from(label.to_string()))
            .on_key_down(self.name_keys(target, draft.clone()))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(boxed)
            .into_any_element()
    }

    /// A space being named: a chip-high pill on `surface-3` with the border
    /// ring (FF InputField focused: `bg-card ring-border`), the live input
    /// inside.
    fn space_field(&self, draft: &SharedString, target: BrowseNaming, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        record_ax_label("Space name");
        let pill = Shape::Pill.rounded(
            div()
                .h(metrics.control_sm)
                .min_w(px(scale::SP_7 * 2.0))
                .max_w(px(scale::SP_7 * 4.0))
                .px(metrics.item_px + px(scale::SP_1))
                .flex()
                .items_center()
                .overflow_hidden()
                .bg(p.surface(3))
                .border_1()
                .border_color(p.line)
                .role_in(TextRole::Caption, cx)
                .text_color(p.fg)
                .child(self.field_body(draft, cx)),
        );
        div()
            .id(self.child("space-name"))
            .flex_shrink(1.0)
            .min_w(px(0.0))
            .role(gpui::Role::TextInput)
            .aria_label("Space name")
            .on_key_down(self.name_keys(target, draft.clone()))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(pill)
            .into_any_element()
    }

    /// The text inside a name field: the live input, or the static draft
    /// with a caret.
    fn field_body(&self, draft: &SharedString, cx: &App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        match self.rename_input.as_ref() {
            Some(input) => div().flex_1().min_w(px(0.0)).child(input.element()).into_any_element(),
            None => h_flex()
                .flex_1()
                .min_w(px(0.0))
                .items_center()
                .text_color(p.fg)
                .child(div().truncate().child(draft.clone()))
                .child(div().flex_none().w(px(1.0)).h(metrics.icon - px(scale::SP_1)).bg(p.fg))
                .into_any_element(),
        }
    }

    /// Enter commits the live text (the draft without one), Escape cancels.
    fn name_keys(&self, target: BrowseNaming, draft: SharedString) -> impl Fn(&gpui::KeyDownEvent, &mut Window, &mut App) + 'static {
        let ctx = self.clone();
        let live = self.rename_input.clone();
        move |event: &gpui::KeyDownEvent, w: &mut Window, cx: &mut App| match event.keystroke.key.as_str() {
            "enter" => {
                let value = live.as_ref().map_or_else(|| draft.clone(), |input| input.value(cx));
                ctx.emit(SidebarIntent::Browse(BrowseIntent::Name(target.clone(), value)), w, cx);
            }
            "escape" => ctx.emit(SidebarIntent::Browse(BrowseIntent::NameCancel), w, cx),
            _ => {}
        }
    }

    /// A project's group label (docs/83 §4): Caption, `fg-muted`, sentence
    /// case, 28 tall, its name at 18 with the chevron after it while
    /// hovered or closed; the branch (when the host shows it) and, while
    /// hovered, New session in it and its menu at the right. No count, no
    /// rule.
    fn project_head(&self, group: &crate::nav::ProjectGroup, window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;
        let hovered = self.model.hovered_group.as_ref() == Some(&group.id);
        let mut head = h_flex()
            .w_full()
            .h(label_height(cx))
            .items_center()
            .gap(px(scale::SP_2))
            .pl(px(ROW_PAD_X))
            .pr(px(scale::SP_2))
            .role_in(TextRole::Caption, cx)
            .text_color(if hovered { p.fg } else { p.fg_muted })
            .child(div().flex_shrink(1.0).min_w(px(0.0)).truncate().child(group.name.clone()))
            .when(hovered || !group.open, |d| d.child(self.label_chevron(format!("chevron-{}", group.id), group.open, hovered, window, cx)))
            .child(div().flex_1().min_w(px(0.0)));
        if let Some(branch) = group.trailing.as_ref() {
            head = head.child(
                div()
                    .flex_shrink(1.0)
                    .min_w(px(0.0))
                    .max_w(px(scale::SP_7 * 3.0))
                    .truncate()
                    .mono(TextRole::Caption)
                    .text_color(p.fg_muted)
                    .child(branch.clone()),
            );
        }
        if hovered {
            head = head.child(
                h_flex()
                    .flex_none()
                    .gap(px(scale::SP_1))
                    .child(self.action(format!("{}-new", group.id), IconName::Plus, format!("New session in {}", group.name), SidebarIntent::GroupAction(group.id.clone(), GroupAction::New), cx))
                    .child(self.action(format!("{}-menu", group.id), IconName::Dots, format!("{} options", group.name), SidebarIntent::GroupAction(group.id.clone(), GroupAction::Menu), cx)),
            );
        }
        let label: SharedString = format!("Project {}", group.name).into();
        record_ax_label(&label);
        head.id(self.child(format!("project-{}", group.id))).role(gpui::Role::Button).aria_label(label).into_any_element()
    }

    /// The chevron after a group label: down while open, right while
    /// closed, turning on the swap spring; `fg` while its label is lit.
    fn label_chevron(&self, key: String, open: bool, lit: bool, window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let turn = spring_phase((self.child(key), "turn"), open, SpringKind::Swap, window, cx);
        icon(IconName::ChevronDown)
            .size(metrics.icon - px(scale::SP_2))
            .color(if lit { p.fg } else { p.fg_muted })
            .rotate(radians(-std::f32::consts::FRAC_PI_2 * (1.0 - turn)))
            .into_any_element()
    }

    /// A date bucket's header in the Date grouping: the section title's
    /// geometry (Caption, muted, label at x 16, `SP_4` above) — sentence
    /// case, never caps.
    fn date_head(&self, label: &SharedString, cx: &App) -> AnyElement {
        self.section(label.as_ref(), None, None, cx).into_any_element()
    }

    /// One flattened row of the virtualised list.
    fn flat_row(&self, item: &SidebarRow, window: &mut Window, cx: &mut App) -> AnyElement {
        if let Grouping::Date(buckets) = self.model.grouping.as_ref() {
            return match item {
                // The first bucket's label is the section header itself
                // ("Today" with the view actions), so its own row collapses:
                // never "Sessions" over "Today".
                SidebarRow::DateHeader { group: 0, .. } => div().into_any_element(),
                SidebarRow::DateHeader { group, .. } => match buckets.get(*group) {
                    Some(bucket) => self.date_head(&bucket.label, cx),
                    None => div().into_any_element(),
                },
                SidebarRow::Session { group, session, .. } => match buckets.get(*group).and_then(|b| b.sessions.get(*session)) {
                    Some(summary) => self.session_row(summary, window, cx),
                    None => div().into_any_element(),
                },
                _ => div().into_any_element(),
            };
        }
        let Grouping::Project(groups) = self.model.grouping.as_ref() else {
            return div().into_any_element();
        };
        match item {
            SidebarRow::ProjectHead { group } => match groups.get(*group) {
                Some(project) => self.project_head(project, window, cx),
                None => div().into_any_element(),
            },
            SidebarRow::Session { group, session, .. } => match groups.get(*group).and_then(|g| g.sessions.get(*session)) {
                Some(summary) => self.session_row(summary, window, cx),
                None => div().into_any_element(),
            },
            SidebarRow::Fold { group } => match groups.get(*group) {
                Some(project) => {
                    let (hidden, expanded) = project.fold.unwrap_or((0, false));
                    let text = if expanded { "Show less".to_string() } else { format!("Show {} more", hidden.max(1)) };
                    let p = cx.aui().colors;
                    record_ax_label(&text);
                    self.row(None, text.clone(), None, false, cx)
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

    /// Four skeleton rows while loading: a group label's bar and three
    /// session titles' bars, no text, no leading glyph.
    fn skeleton_row(&self, index: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let bar = if index == 0 { px(scale::SP_3) } else { px(scale::SP_4) };
        let widths = [scale::SP_7 * 3.0, scale::SP_7 * 4.5, scale::SP_7 * 3.5, scale::SP_7 * 4.0];
        let line = Shape::Item.rounded(aui_motion::skeleton(self.child(format!("sk-line-{index}")), px(widths[index % widths.len()]), bar, window, cx));
        let frame = self.row_frame(None, false, cx).child(line);
        if index == 0 { frame.h(label_height(cx)) } else { frame }.into_any_element()
    }
}

/// What a session row says under its title, with the summary line on
/// ([`Bylines::Summary`]; [`Bylines::Off`] says nothing): a live status
/// sentence (working, waiting on you, or just "Failed") first — that is
/// news; otherwise what the session last said (the latest reply's summary,
/// else the last ask, else a preview or placeholder), else its git branch.
/// A settled or never-replied status says nothing the trailing slot and the
/// age do not already say, so it never takes the line (owner note
/// 2026-10-10: "Settled · 6d · 1 turn" was noise).
pub fn second_line_for(session: &SessionSummary, bylines: Bylines) -> Option<SharedString> {
    use crate::nav::{Byline, RowStatusKind};
    if bylines == Bylines::Off {
        return None;
    }
    if let Some(status) = session.status.as_ref() {
        match status.kind {
            RowStatusKind::Settled | RowStatusKind::NoReply => {}
            // A failure's detail is the session's age, which the row already
            // shows at its right edge: the word alone (the hover card has
            // the error).
            RowStatusKind::Failed => return Some("Failed".into()),
            _ => return Some(status.text()),
        }
    }
    let nonblank = |text: &SharedString| (!text.trim().is_empty()).then(|| text.clone());
    let said = match session.byline.as_ref() {
        Some(Byline::Placeholder(text)) | Some(Byline::Preview(text)) => nonblank(text),
        Some(Byline::TwoLines { ask, result }) => nonblank(result).or_else(|| nonblank(ask)),
        None => None,
    };
    said.or_else(|| session.branch.as_ref().and_then(nonblank))
}

/// Builds the Sessions-mode chrome and list, or the Browse-mode rows, into
/// `list` (the screen's [`RowList`], its handlers already set); filling
/// `ctx.targets`, the index → target map the handlers translate through.
fn build_list(ctx: &Ctx, mut list: RowList, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let model = ctx.model.clone();
    let targets = ctx.targets.clone();
    let mut index_of_session: HashMap<SharedString, usize> = HashMap::new();
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
                .pl(px(ROW_PAD_X))
                .pr(px(ROW_PAD_X))
                .child({
                    // The site button: connection and certificate details.
                    let c = ctx.clone();
                    div()
                        .id(ctx.child("site-info"))
                        .flex_none()
                        .cursor_pointer()
                        .role(gpui::Role::Button)
                        .aria_label("View site information")
                        .tooltip(crate::data::tip_within("View site information", ctx.width))
                        .tooltip_show_delay(crate::data::TIP_DELAY)
                        .on_click(move |_, w, cx| {
                            cx.stop_propagation();
                            c.emit(SidebarIntent::Browse(BrowseIntent::SiteInfo), w, cx);
                        })
                        .child(ctx.lead_icon(IconName::Shield, cx))
                })
                .child(
                    h_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .text_role(TextRole::Body)
                        .child(div().text_color(p.fg).child(browse.host.clone()))
                        .child(div().truncate().text_color(p.fg_muted).child(browse.path.clone())),
                )
                .when(browse.bookmarked, |d| {
                    // The filled ★: this page is bookmarked; a click opens
                    // the bookmark popover (name, folder, Remove).
                    let c = ctx.clone();
                    record_ax_label("Edit bookmark");
                    d.child(
                        div()
                            .id(ctx.child("bookmark"))
                            .flex_none()
                            .cursor_pointer()
                            .role(gpui::Role::Button)
                            .aria_label("Edit bookmark")
                            .tooltip(crate::data::tip_within("Edit bookmark  ⌘D", ctx.width))
                            .tooltip_show_delay(crate::data::TIP_DELAY)
                            .on_click(move |_, w, cx| {
                                cx.stop_propagation();
                                c.emit(SidebarIntent::Browse(BrowseIntent::Bookmark), w, cx);
                            })
                            .child(icon(IconName::StarFill).size(px(14.0)).color(p.fg)),
                    )
                })
.when(model.hovered.as_deref() == Some("address"), |d| d.child(ctx.chip("⌘L", cx)));
            record_ax_label("Address");
            list = list.item(next, surface(2, address).shape(Shape::Item).into_any_element());
            targets.borrow_mut().insert(next, Target::Intent(SidebarIntent::Browse(BrowseIntent::Address)));
            next += 1;
            // Page controls: back, forward, reload, new tab | ask, menu.
            let control = |ctx: &Ctx, key: &str, glyph: IconName, label: &str, intent: BrowseIntent, enabled: bool, cx: &App| {
                let c = ctx.clone();
                let mut button = icon_button(ctx.child(format!("browse-{key}")), glyph).tooltip_within(ctx.width)
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
                .child({
                    // The page menu opens under its button, so the click
                    // carries the button's bounds from this frame's layout.
                    let open = browse.menu.as_ref().is_some_and(|m| matches!(m.target, BrowseMenuTarget::Page { .. }));
                    let anchor: Rc<Cell<Option<gpui::Bounds<Pixels>>>> = Rc::new(Cell::new(None));
                    let at = anchor.clone();
                    let c = ctx.clone();
                    let button = icon_button(ctx.child("browse-page-menu"), IconName::Dots)
                        .tooltip_within(ctx.width)
                        .ghost()
                        .size(ButtonSize::Md)
                        .icon_size(cx.aui().metrics.icon)
                        .accessibility_label("Page menu")
                        .menu_open(open)
                        .on_click(move |_, w, cx| {
                            cx.stop_propagation();
                            c.emit(SidebarIntent::Browse(BrowseIntent::PageMenu(at.get())), w, cx);
                        });
                    div().on_prepaint(move |bounds, _, _| anchor.set(Some(bounds))).child(button)
                });
            // A toolbar: the pointer over its buttons lights no row.
            list = list.item_static(next, controls);
            next += 1;
        }
    }

    // Search: the quiet first row; ⌘K only while the pointer is on it.
    let search_hovered = model.hovered.as_deref() == Some("search");
    let search_text = model.search.clone().unwrap_or_else(|| "Search".into());
    let search = ctx.row(Some(ctx.lead_icon_lit(IconName::Search, search_hovered, cx)), search_text, if search_hovered { Some(ctx.chip("⌘K", cx)) } else { None }, false, cx);
    record_ax_label("Search");
    list = list.item(next, search.id(ctx.child("search")).role(gpui::Role::Button).aria_label("Search"));
    targets.borrow_mut().insert(next, Target::Intent(SidebarIntent::Search));
    next += 1;

    if model.mode == Mode::Sessions {
        // New session, then the nav rows.
        let new_hovered = model.hovered.as_deref() == Some("new-session");
        let new_row = ctx.row(Some(ctx.lead_icon_lit(IconName::Plus, new_hovered, cx)), "New session", if new_hovered { Some(ctx.chip("⌘N", cx)) } else { None }, false, cx);
        record_ax_label("New session");
        list = list.item(next, new_row.id(ctx.child("new-session")).role(gpui::Role::Button).aria_label("New session"));
        targets.borrow_mut().insert(next, Target::Intent(SidebarIntent::NewSession));
        next += 1;
        for item in &model.nav {
            // No trailing count unless non-zero (docs/83 §4).
            let trailing = item.count.filter(|count| *count > 0).map(|count| ctx.trailing(count.to_string(), cx));
            let lit = ctx.lit(&format!("nav-{}", item.id));
            let row = ctx.row(Some(ctx.lead_icon_lit(item.icon, lit, cx)), item.label.clone(), trailing, false, cx);
            record_ax_label(&item.label);
            list = list.item(next, row.id(ctx.child(format!("nav-{}", item.id))).role(gpui::Role::Button).aria_label(item.label.clone()));
            targets.borrow_mut().insert(next, Target::Intent(SidebarIntent::Nav(item.id.clone())));
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
                // The space above the label sits outside the highlightable
                // head: with the list's gaps, the 14 the Projects header has.
                list = list.item_static(next, div().w_full().h(px(scale::SP_4 - ROW_GAP)));
                next += 1;
                // A group label: the chevron follows it while hovered or closed.
                let lit = ctx.lit(PINNED_KEY);
                let head = h_flex()
                    .w_full()
                    .h(label_height(cx))
                    .items_center()
                    .gap(px(scale::SP_2))
                    .pl(px(ROW_PAD_X))
                    .role_in(TextRole::Caption, cx)
                    .text_color(if lit { p.fg } else { p.fg_muted })
                    .child(div().child("Pinned"))
                    .when(lit || !model.pinned_open, |d| d.child(ctx.label_chevron("pinned-chevron".into(), model.pinned_open, lit, window, cx)));
                record_ax_label("Pinned");
                list = list.item(next, head.id(ctx.child("pinned-head")).role(gpui::Role::Button).aria_label("Pinned"));
                targets.borrow_mut().insert(next, Target::Intent(SidebarIntent::TogglePinned));
                next += 1;
                if model.pinned_open {
                    for (i, session) in model.pinned.iter().enumerate() {
                        let index = PINNED_BASE + i;
                        list = list.item(index, ctx.session_row(session, window, cx));
                        targets.borrow_mut().insert(index, Target::Session(session.id.clone()));
                        index_of_session.insert(session.id.clone(), index);
                    }
                }
            }
            // Projects header with its hover actions, then the virtual list.
            // Grouped by date the same header reads the first bucket's label
            // ("Today") and counts nothing; that bucket's own header row
            // collapses (see `flat_row`).
            let by_date = matches!(model.grouping.as_ref(), Grouping::Date(_));
            let project_count = match model.grouping.as_ref() {
                Grouping::Project(groups) => groups.len(),
                Grouping::Date(buckets) => buckets.iter().map(|b| b.sessions.len()).sum(),
                Grouping::Status(_) => 0,
            };
            // Actions appear on hover; with no projects the Add button is
            // always there (it is the only thing to do).
            // The view-options button stays while its menu is open: the
            // pointer leaves the header for the menu (FF keeps a trigger up,
            // pressed, while its menu is open).
            let header_live = model.hovered_projects_header || model.view_menu_open;
            let actions = (header_live || project_count == 0).then(|| {
                let mut acts = h_flex().flex_none().gap(px(scale::SP_1)).child(ctx.action("add-project".into(), IconName::Plus, "Add project".into(), SidebarIntent::AddProject, cx));
                if header_live {
                    acts = acts.child(ctx.menu_action("view-options".into(), IconName::Sliders, "View options".into(), SidebarIntent::ViewOptions, model.view_menu_open, cx));
                }
                acts.into_any_element()
            });
            let hover_ctx = ctx.clone();
            let header = ctx
                .section(
                    &match model.grouping.as_ref() {
                        Grouping::Date(buckets) => buckets.first().map_or_else(|| "Sessions".to_string(), |b| b.label.to_string()),
                        _ => "Projects".to_string(),
                    },
                    None,
                    actions,
                    cx,
                )
                .id(ctx.child("projects-head"))
                .on_hover(move |hovered: &bool, w: &mut Window, cx: &mut App| {
                    hover_ctx.emit(SidebarIntent::HoverProjectsHeader(*hovered), w, cx);
                });
            // A toolbar: its hover actions answer the pointer, never a row.
            list = list.item_static(next, header);
            next += 1;
            if project_count == 0 {
                let words = if by_date { "No sessions yet" } else { "No projects yet" };
                let empty = ctx.row(None, words, None, false, cx).text_color(p.fg_muted);
                record_ax_label(words);
                list = list.item_static(next, empty);
            } else {
                let rows = Rc::new(flatten_sidebar(&model.grouping, false));
                for (ix, row) in rows.iter().enumerate() {
                    let index = VIRTUAL_BASE + ix;
                    match row {
                        SidebarRow::ProjectHead { group } => {
                            if let Grouping::Project(groups) = model.grouping.as_ref() {
                                if let Some(project) = groups.get(*group) {
                                    targets.borrow_mut().insert(index, Target::Intent(SidebarIntent::Toggle(project.id.clone())));
                                }
                            }
                        }
                        SidebarRow::Session { group, session, .. } => {
                            let summary = match model.grouping.as_ref() {
                                Grouping::Project(groups) => groups.get(*group).and_then(|g| g.sessions.get(*session)),
                                Grouping::Date(buckets) => buckets.get(*group).and_then(|b| b.sessions.get(*session)),
                                Grouping::Status(_) => None,
                            };
                            if let Some(summary) = summary {
                                targets.borrow_mut().insert(index, Target::Session(summary.id.clone()));
                                index_of_session.insert(summary.id.clone(), index);
                            }
                        }
                        SidebarRow::Fold { group } => {
                            if let Grouping::Project(groups) = model.grouping.as_ref() {
                                if let Some(project) = groups.get(*group) {
                                    targets.borrow_mut().insert(index, Target::Intent(SidebarIntent::ToggleMore(project.id.clone())));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let row_ctx = ctx.clone();
                // Every row keeps the list's 2 px gap under its block (pitch
                // 34), so a selected row and a hovered neighbour never touch;
                // a project label after the first stands 6 more clear of the
                // group above (a closed group's label pitch is 36).
                let render = move |ix: usize, window: &mut Window, cx: &mut App| -> AnyElement {
                    let built = match rows.get(ix) {
                        // Date headers are titles, never highlight targets.
                        Some(row @ SidebarRow::DateHeader { .. }) => row_ctx.flat_row(row, window, cx),
                        Some(row) => {
                            let built = row_ctx.flat_row(row, window, cx);
                            row_ctx.wrap_virtual(VIRTUAL_BASE + ix, built, cx)
                        }
                        None => return div().into_any_element(),
                    };
                    let apart = ix > 0 && matches!(rows.get(ix), Some(SidebarRow::ProjectHead { .. }));
                    div().w_full().pb(px(ROW_GAP)).when(apart, |d| d.pt(px(scale::SP_3 - ROW_GAP))).child(built).into_any_element()
                };
                list = list.item_static_fill(next, list_host(ctx, render, cx));
            }
        }
    } else if let Some(browse) = model.browse.as_ref() {
        let menu_at = |ctx: &Ctx, target: BrowseMenuTarget| {
            let c = ctx.clone();
            move |event: &gpui::MouseDownEvent, w: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                c.emit(SidebarIntent::Browse(BrowseIntent::OpenMenu(BrowseMenu { target: target.clone(), at: event.position })), w, cx);
            }
        };
        // Pins, Today's tabs, your folders and New tab, built for the
        // scrolling area below the fixed address / controls / search rows.
        let mut scroll_rows: Vec<AnyElement> = Vec::new();
        // Pinned sites as tiles, only when the space has any.
        if !browse.pins.is_empty() {
            scroll_rows.push(ctx.section("Pinned", None, None, cx).into_any_element());
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
                        .text_role(TextRole::Body)
                        .medium()
                        .text_color(p.fg_muted)
                        .child(match pin.favicon.as_ref() {
                            Some(path) => gpui::img(std::path::PathBuf::from(path.as_ref())).size(metrics.icon).into_any_element(),
                            None => pin.initial.clone().into_any_element(),
                        }),
                )
                .shape(Shape::Item);
                let c = ctx.clone();
                let id = pin.id.clone();
                tiles = tiles.child(
                    div().id(ctx.child(format!("pin-wrap-{}", pin.id))).on_mouse_down(gpui::MouseButton::Right, menu_at(ctx, BrowseMenuTarget::Tab { id: pin.id.clone(), pinned: true, in_folder: false })).child(
                        interactive(ctx.child(format!("pin-{}", pin.id)), tile)
                            .label(pin.label.clone())
                            .on_click(move |_, w, cx| {
                                cx.stop_propagation();
                                c.emit(SidebarIntent::Browse(BrowseIntent::OpenPin(id.clone())), w, cx);
                            }),
                    ),
                );
            }
            scroll_rows.push(tiles.into_any_element());
            next += 1;
        }
        // Today: your tabs and folders, with new-folder / close-all while hovered.
        let today_actions = browse.hovered_today_header.then(|| {
            h_flex()
                .flex_none()
                .gap(px(scale::SP_1))
                .child(ctx.action("new-folder".into(), IconName::Folder, "New folder".into(), SidebarIntent::Browse(BrowseIntent::NewFolder), cx))
                .child(ctx.action("close-all".into(), IconName::XBold, "Close all tabs".into(), SidebarIntent::Browse(BrowseIntent::CloseAll), cx))
                .into_any_element()
        });
        let tab_total = browse.tabs.len() + browse.folders.iter().map(|f| f.tabs.len()).sum::<usize>();
        let hover_ctx = ctx.clone();
        let today = ctx
            .section("Today", (tab_total > 0).then(|| tab_total.to_string().into()), today_actions, cx)
            .id(ctx.child("today-head"))
            .on_hover(move |hovered: &bool, w: &mut Window, cx: &mut App| {
                hover_ctx.emit(SidebarIntent::Browse(BrowseIntent::HoverTodayHeader(*hovered)), w, cx);
            });
        scroll_rows.push(today.into_any_element());
        next += 1;
        if browse.tabs.is_empty() && browse.folders.is_empty() {
            let words = "No tabs open";
            record_ax_label(words);
            scroll_rows.push(ctx.row(Some(ctx.lead(div(), cx)), words, None, false, cx).text_color(p.fg_muted).into_any_element());
            next += 1;
        }
        let tab_row = |ctx: &Ctx, rows: &mut Vec<AnyElement>, index: usize, tab: &super::BrowseTab, nested: bool, in_folder: bool, trailing: Option<AnyElement>, cx: &App| {
            // A loading page spins in the favicon slot (owner 2026-10-11).
            let mark = if tab.loading {
                div()
                    .id(ctx.child(format!("loading-{}", tab.id)))
                    .role(gpui::Role::ProgressIndicator)
                    .aria_label(format!("Loading {}", tab.title))
                    .child(spinner(ctx.child(format!("spin-{}", tab.id))).size(metrics.icon).fluid(cx))
                    .into_any_element()
            } else {
                ctx.favicon(tab.initial.clone(), tab.favicon.as_ref(), cx)
            };
            let lit = model.hovered.as_ref() == Some(&tab.id) || tab.active;
            let close = if lit {
                let c = ctx.clone();
                let id = tab.id.clone();
                Some(
                    icon_button(ctx.child(format!("close-{}", tab.id)), IconName::XBold)
                        .ghost()
                        .size(ButtonSize::Xs)
                        .icon_size(metrics.icon - px(scale::SP_2))
                        .accessibility_label(format!("Close tab {}", tab.title))
                        .no_tooltip()
                        .on_click(move |_, w, cx| {
                            cx.stop_propagation();
                            c.emit(SidebarIntent::Browse(BrowseIntent::CloseTab(id.clone())), w, cx);
                        })
                        .into_any_element(),
                )
            } else {
                trailing
            };
            let row = ctx.row(Some(ctx.lead(mark, cx)), tab.title.clone(), close, nested, cx);
            let label: SharedString = format!("Tab {}", tab.title).into();
            record_ax_label(&label);
            let built = row
                .id(ctx.child(format!("tab-{}", tab.id)))
                .role(gpui::Role::Tab)
                .aria_label(label)
                .on_mouse_down(gpui::MouseButton::Right, menu_at(ctx, BrowseMenuTarget::Tab { id: tab.id.clone(), pinned: false, in_folder }));
            let highlighted = model.highlighted.as_ref() == Some(&tab.id);
            rows.push(ctx.items.wrap(index, built, StatePaint::new().selected(tab.active).highlighted(highlighted), cx));
        };
        let mut selected_index: Option<usize> = None;
        for tab in &browse.tabs {
            let index = BROWSE_BASE + next;
            tab_row(ctx, &mut scroll_rows, index, tab, false, false, None, cx);
            targets.borrow_mut().insert(index, Target::Intent(SidebarIntent::Browse(BrowseIntent::SelectTab(tab.id.clone()))));
            index_of_session.insert(tab.id.clone(), index);
            if tab.active {
                selected_index = Some(index);
            }
            next += 1;
        }
        // A folder head: your folder's chevron (or the name field while it is
        // being named), or a task folder's status dot and status word.
        let folder_head = |ctx: &Ctx, folder: &super::BrowseFolder, window: &mut Window, cx: &mut App| -> AnyElement {
            let p = cx.aui().colors;
            let metrics = cx.aui().metrics;
            let hovered = ctx.lit(&format!("folder-{}", folder.id));
            let naming = model.browse.as_ref().and_then(|b| b.editing.as_ref()).filter(|e| e.target == BrowseNaming::Folder(folder.id.clone()));
            let lead: AnyElement = match &folder.kind {
                FolderKind::Yours => {
                    let turn = spring_phase((ctx.child(format!("folder-chevron-{}", folder.id)), "turn"), folder.open, SpringKind::Swap, window, cx);
                    ctx.lead(icon(IconName::ChevronDown).size(metrics.icon).color(if hovered { p.fg } else { p.fg_muted }).lit(hovered).rotate(radians(-std::f32::consts::FRAC_PI_2 * (1.0 - turn))), cx)
                }
                FolderKind::Task { face, .. } => ctx.lead(crate::browse::task_face(*face), cx),
            };
            if let Some(editing) = naming {
                return ctx.name_field(format!("folder-name-{}", folder.id), lead, &editing.draft, BrowseNaming::Folder(folder.id.clone()), "Folder name", false, cx);
            }
            let mut head = h_flex()
                .w_full()
                .h(list_row_height(cx))
                .items_center()
                .gap(metrics.gap)
                .pl(px(ROW_PAD_X))
                .pr(px(ROW_PAD_X))
                .child(lead)
                .child(div().flex_shrink(1.0).min_w(px(0.0)).truncate().text_role(TextRole::Body).text_color(if hovered { p.fg } else { p.fg_muted }).child(folder.name.clone()));
            let target = match &folder.kind {
                FolderKind::Yours => {
                    head = head
                        .child(div().flex_none().ml(px(scale::SP_2 + scale::SP_1) - metrics.gap).role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(folder.tabs.len().to_string()))
                        .child(div().flex_1());
                    BrowseMenuTarget::Folder(folder.id.clone())
                }
                FolderKind::Task { state, status, needs_you, .. } => {
                    head = head.child(div().flex_1());
                    if !status.is_empty() {
                        head = head.child(div().flex_none().role_in(TextRole::Caption, cx).text_color(if *needs_you { p.warning } else { p.fg_muted }).child(status.clone()));
                    }
                    BrowseMenuTarget::TaskFolder { session: folder.id.clone(), running: *state == AgentState::Running }
                }
            };
            let label: SharedString = match folder.kind {
                FolderKind::Yours => format!("Folder {}", folder.name).into(),
                FolderKind::Task { .. } => format!("Task folder {}, open its chat", folder.name).into(),
            };
            record_ax_label(&label);
            head.id(ctx.child(format!("folder-{}", folder.id)))
                .role(gpui::Role::Button)
                .aria_label(label)
                .on_mouse_down(gpui::MouseButton::Right, menu_at(ctx, target))
                .into_any_element()
        };
        for folder in &browse.folders {
            let index = BROWSE_BASE + next;
            let head = folder_head(ctx, folder, window, cx);
            let highlighted = model.highlighted.as_deref() == Some(format!("folder-{}", folder.id).as_str());
            scroll_rows.push(ctx.items.wrap(index, head, StatePaint::new().highlighted(highlighted), cx));
            targets.borrow_mut().insert(index, Target::Intent(SidebarIntent::Browse(BrowseIntent::ToggleFolder(folder.id.clone()))));
            index_of_session.insert(format!("folder-{}", folder.id).into(), index);
            next += 1;
            if folder.open {
                for tab in &folder.tabs {
                    let index = BROWSE_BASE + next;
                    tab_row(ctx, &mut scroll_rows, index, tab, true, true, None, cx);
                    targets.borrow_mut().insert(index, Target::Intent(SidebarIntent::Browse(BrowseIntent::SelectTab(tab.id.clone()))));
                    index_of_session.insert(tab.id.clone(), index);
                    if tab.active {
                        selected_index = Some(index);
                    }
                    next += 1;
                }
            }
        }
        // New tab: the one filled row, directly under your tabs (owner
        // 2026-10-11), with its 8 px above outside the highlightable box.
        scroll_rows.push(div().w_full().h(metrics.item_px).into_any_element());
        next += 1;
        let new_tab = ctx.row(Some(ctx.lead_icon(IconName::Plus, cx)), "New tab", Some(ctx.chip("⌘T", cx)), false, cx);
        record_ax_label("New tab");
        let index = BROWSE_BASE + next;
        scroll_rows.push(ctx.items.wrap(
            index,
            surface(2, new_tab.id(ctx.child("new-tab")).role(gpui::Role::Button).aria_label("New tab")).shape(Shape::Item),
            StatePaint::new(),
            cx,
        ));
        scroll_rows.push(div().w_full().h(metrics.item_px).into_any_element());
        targets.borrow_mut().insert(index, Target::Intent(SidebarIntent::Browse(BrowseIntent::NewTab)));
        next += 1;
        // Your tabs scroll in the main area, clipped to it.
        let scroll_host = div().id(ctx.child("browse-scroll")).size_full().overflow_y_scroll().child(v_flex().w_full().children(scroll_rows));
        list = list.item_static_fill(next, scroll_host);
        next += 1;
        // Task folders: a shorter pane of their own at the foot (owner
        // 2026-10-11), under a divider that spans the sidebar edge to edge,
        // capped in height and scrolling on its own.
        if !browse.task_folders.is_empty() {
            let mut task_rows: Vec<AnyElement> = Vec::new();
            let mut ti = TASK_BASE;
            for folder in &browse.task_folders {
                let head = folder_head(ctx, folder, window, cx);
                let highlighted = model.highlighted.as_deref() == Some(format!("folder-{}", folder.id).as_str());
                task_rows.push(ctx.items.wrap(ti, head, StatePaint::new().highlighted(highlighted), cx));
                targets.borrow_mut().insert(ti, Target::Intent(SidebarIntent::Browse(BrowseIntent::OpenTaskChat(folder.id.clone()))));
                index_of_session.insert(format!("folder-{}", folder.id).into(), ti);
                ti += 1;
                for tab in &folder.tabs {
                    tab_row(ctx, &mut task_rows, ti, tab, true, false, None, cx);
                    targets.borrow_mut().insert(ti, Target::Intent(SidebarIntent::Browse(BrowseIntent::SelectTab(tab.id.clone()))));
                    index_of_session.insert(tab.id.clone(), ti);
                    if tab.active {
                        selected_index = Some(ti);
                    }
                    ti += 1;
                }
            }
            // No heading (owner 2026-11-08): the hairline alone separates the
            // task folders from your tabs, as docs/27 §4.3 draws it.
            let pane = v_flex()
                .w_full()
                .flex_none()
                .child(div().mx(-metrics.item_px).h(px(1.0)).bg(p.line))
                .child(
                    div()
                        .w_full()
                        .max_h(task_pane_max(cx))
                        .child(
                            div().id(ctx.child("task-scroll")).w_full().max_h(task_pane_max(cx)).overflow_y_scroll().py(metrics.item_px / 2.0).child(v_flex().w_full().children(task_rows)),
                        ),
                );
            list = list.item_static(next, pane);
            next += 1;
        }
        // The spaces, pinned above the footer.
        let naming_space = browse.editing.as_ref().filter(|e| matches!(e.target, BrowseNaming::Space(_)));
        let mut chips = h_flex().id(ctx.child("spaces")).w_full().items_center().gap(px(scale::SP_2)).py(metrics.item_px).role(gpui::Role::TabList).aria_label("Spaces");
        record_ax_label("Spaces");
        for (i, space) in browse.spaces.iter().enumerate() {
            if let Some(editing) = naming_space.filter(|e| e.target == BrowseNaming::Space(Some(space.id.clone()))) {
                chips = chips.child(ctx.space_field(&editing.draft, editing.target.clone(), cx));
                continue;
            }
            let on = i == browse.space;
            let chip = div()
                .h(metrics.control_sm)
                .px(metrics.item_px + px(scale::SP_1))
                .flex()
                .items_center()
                .role_in(TextRole::Caption, cx)
                .gap(px(scale::SP_1))
                .text_color(if on { p.fg } else { p.fg_muted })
                .child(space.name.clone())
                .when(space.own_signins, |d| d.child(icon(IconName::Lock).size(px(11.0)).color(p.fg_muted)));
            let chip: AnyElement = if on { surface(3, chip).shape(Shape::Pill).into_any_element() } else { chip.into_any_element() };
            let c = ctx.clone();
            chips = chips.child(
                div().id(ctx.child(format!("space-wrap-{}", space.id))).on_mouse_down(gpui::MouseButton::Right, menu_at(ctx, BrowseMenuTarget::Space { id: space.id.clone(), last: browse.spaces.len() == 1 })).child(
                    interactive(ctx.child(format!("space-{}", space.id)), chip)
                        .role("tab")
                        .shape(Shape::Pill)
                        .selected(on)
                        .label(if space.own_signins { format!("Space {}, its own sign-ins", space.name) } else { format!("Space {}", space.name) })
                        .on_click(move |_, w, cx| {
                            cx.stop_propagation();
                            c.emit(SidebarIntent::Browse(BrowseIntent::SwitchSpace(i)), w, cx);
                        }),
                ),
            );
        }
        if let Some(editing) = naming_space.filter(|e| e.target == BrowseNaming::Space(None)) {
            chips = chips.child(ctx.space_field(&editing.draft, editing.target.clone(), cx));
        } else {
            chips = chips.child(div().flex_1());
            // R4 (board R4Daily 4a): downloads and extensions live in the
            // footer; each popover opens above its button.
            let running = browse.downloads == Some(true);
            let foot_button = |id: &str, glyph: IconName, label: &'static str, intent: fn(Option<gpui::Bounds<Pixels>>) -> BrowseIntent, dot: bool, cx: &mut App| {
                let anchor: Rc<Cell<Option<gpui::Bounds<Pixels>>>> = Rc::new(Cell::new(None));
                let at = anchor.clone();
                let c = ctx.clone();
                let button = icon_button(ctx.child(id.to_owned()), glyph)
                    .tooltip_within(ctx.width)
                    .ghost()
                    .size(ButtonSize::Sm)
                    .icon_size(cx.aui().metrics.icon)
                    .accessibility_label(label)
                    .on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        c.emit(SidebarIntent::Browse(intent(at.get())), w, cx);
                    });
                div()
                    .relative()
                    .on_prepaint(move |bounds, _, _| anchor.set(Some(bounds)))
                    .child(button)
                    .when(dot, |d| d.child(div().absolute().top(px(5.0)).right(px(5.0)).size(px(6.0)).rounded_full().bg(p.fg)))
            };
            chips = chips
                .child(foot_button("browse-downloads", IconName::Download, if running { "Downloads, one running" } else { "Downloads" }, BrowseIntent::Downloads, running, cx))
                .child(foot_button("browse-extensions", IconName::Puzzle, "Extensions", BrowseIntent::Extensions, false, cx))
                .child(ctx.action("new-space".into(), IconName::Plus, "New space".into(), SidebarIntent::Browse(BrowseIntent::NewSpace), cx));
        }
        // Always under a divider spanning the sidebar edge to edge, the
        // task pane's own (owner 2026-10-11): the strip is the foot's, not
        // the list's, whether or not task folders sit above it.
        let foot = v_flex().w_full().flex_none().child(div().mx(-metrics.item_px).h(px(1.0)).bg(p.line)).child(chips);
        // A toolbar: its chips and + answer the pointer, never the row above.
        list = list.item_static(next, foot);
        if let Some(index) = selected_index {
            list = list.selected(Some(index));
        }
    }

    // Selection and keyboard highlight, translated to indices.
    if model.mode == Mode::Sessions {
        let selected = model.selected.as_ref().and_then(|id| index_of_session.get(id).copied());
        list = list.selected(selected);
        // A row being renamed is the field's: no block paints under it.
        let editing = model.editing.as_ref().and_then(|e| index_of_session.get(&e.session).copied());
        list = list.inert(editing);
    }
    let highlighted = model.highlighted.as_ref().and_then(|id| index_of_session.get(id).copied());
    if highlighted.is_some() {
        list = list.highlighted(highlighted);
    }
    list.into_any_element()
}

/// What activating a list index means.
#[derive(Clone)]
enum Target {
    Session(SharedString),
    Intent(SidebarIntent),
}

/// The sidebar's one [`RowList`], its handlers set: activation emits the
/// index's target; the pointer report emits `Hover` (sessions their id,
/// chrome rows their key) and `HoverGroup` (a project head), with the
/// item's bounds for the hover card.
fn screen_list(id: &ElementId, targets: Rc<RefCell<HashMap<usize, Target>>>, on_intent: Option<IntentHandler>) -> RowList {
    let emit = move |intent: SidebarIntent, w: &mut Window, cx: &mut App| {
        if let Some(handler) = on_intent.as_ref() {
            handler(intent, w, cx);
        }
    };
    let emit = Rc::new(emit);
    let hover_targets = targets.clone();
    let hover_emit = emit.clone();
    let activate_targets = targets;
    row_list((id.clone(), "rows"))
        .label("Sidebar")
        .gap(px(ROW_GAP))
        .fill()
        .on_highlight_at(move |index, bounds, w, cx| {
            let targets = hover_targets.borrow();
            let target = index.and_then(|i| targets.get(&i));
            let key: Option<SharedString> = match target {
                Some(Target::Session(id)) => Some(id.clone()),
                Some(Target::Intent(SidebarIntent::Search)) => Some("search".into()),
                Some(Target::Intent(SidebarIntent::NewSession)) => Some("new-session".into()),
                Some(Target::Intent(SidebarIntent::TogglePinned)) => Some(PINNED_KEY.into()),
                Some(Target::Intent(SidebarIntent::Nav(nav))) => Some(format!("nav-{nav}").into()),
                Some(Target::Intent(SidebarIntent::Browse(BrowseIntent::Address))) => Some("address".into()),
                Some(Target::Intent(SidebarIntent::Browse(BrowseIntent::SelectTab(id)))) => Some(id.clone()),
                Some(Target::Intent(SidebarIntent::Browse(BrowseIntent::ToggleFolder(id) | BrowseIntent::OpenTaskChat(id)))) => {
                    Some(format!("folder-{id}").into())
                }
                _ => None,
            };
            // A project head under the pointer shows its actions.
            let group = match target {
                Some(Target::Intent(SidebarIntent::Toggle(id))) => Some(id.clone()),
                _ => None,
            };
            drop(targets);
            hover_emit(SidebarIntent::HoverGroup(group), w, cx);
            let bounds = bounds.filter(|_| key.is_some());
            hover_emit(SidebarIntent::Hover(key, bounds), w, cx);
        })
        .on_activate(move |index, w, cx| {
            let target = activate_targets.borrow().get(&index).cloned();
            match target {
                Some(Target::Session(id)) => emit(SidebarIntent::Select(id), w, cx),
                Some(Target::Intent(intent)) => emit(intent, w, cx),
                None => {}
            }
        })
}

impl Ctx {
    /// Wraps a virtual row (index `index`) in the list's item box: a session
    /// row holds `ov-selected` when it is the selection and the hover look
    /// when the keyboard is on it; the row being renamed is the field's and
    /// paints nothing.
    fn wrap_virtual(&self, index: usize, element: AnyElement, cx: &App) -> AnyElement {
        let session = match self.targets.borrow().get(&index) {
            Some(Target::Session(id)) => Some(id.clone()),
            _ => None,
        };
        let model = &self.model;
        if session.is_some() && model.editing.as_ref().map(|e| &e.session) == session.as_ref() {
            return element;
        }
        let selected = session.is_some() && model.selected.as_ref() == session.as_ref();
        let highlighted = session.is_some() && model.highlighted.as_ref() == session.as_ref();
        self.items.wrap(index, element, StatePaint::new().selected(selected).highlighted(highlighted), cx)
    }
}

/// The virtualised list, filling its slot in the row list.
fn list_host(ctx: &Ctx, render: impl Fn(usize, &mut Window, &mut App) -> AnyElement + 'static, cx: &App) -> AnyElement {
    // No scroll fade at the list's edges (docs/83 §1 rule 1): rows pass
    // under the Projects header and the footer's line, cut flat.
    let state = ctx_state(ctx);
    div()
        .id(ctx.child("virtual"))
        .relative()
        .w_full()
        .h_full()
        .flex_1()
        .min_h(px(0.0))
        .child(list(state, render).w_full().h_full())
        // The bar stands in the body's right gutter (owner 2026-10-11): its
        // host reaches past the rows by the body's padding, so the track is
        // flush with the sidebar's edge like FF's on its scroll container,
        // and the thumb never sits over a row's trailing slot.
        .child(div().absolute().top(px(0.0)).bottom(px(0.0)).left(px(0.0)).right(-cx.aui().metrics.item_px).child(list_scrollbar(ctx, cx)))
        .into_any_element()
}

/// Fluid Functionalism's scrollbar (`scroll-area.tsx`): shown while the list
/// is hovered or scrolled, a 10 px track flush to the sidebar's edge, a 4 px thumb
/// nudged 2 px off it at the overlay colour 8 %, widening to 6 px at 12 %
/// over the track and 16 % while dragged, fully round.
fn list_scrollbar(ctx: &Ctx, cx: &App) -> AnyElement {
    use aui_tokens::ThumbState;
    use gpui_kit::base::{Scrollbar, ScrollbarMode};
    let p = cx.aui().colors;
    let track = px(scale::SP_4 - scale::SP_1); // 10
    let rest = px(scale::SP_2); // 4
    let wide = px(scale::SP_2 + scale::SP_1); // 6
    let inset = px(scale::SP_1); // 2
    let round = px(scale::SP_2 + scale::SP_1);
    Scrollbar::vertical(&ctx.state)
        .id(ctx.child("scrollbar"))
        // Its host's box (reaching into the gutter), not the list's viewport.
        .viewport_from_layout()
        .mode(ScrollbarMode::Hover)
        .styles(|s| {
            s.track(|t| t.width(track).bg(gpui::transparent_black()))
                .thumb(|t| t.width(rest).inset(inset).radius(round).bg(p.scrollbar_thumb(ThumbState::Rest)))
                .thumb_hover(|t| t.width(wide).inset(inset).radius(round).bg(p.scrollbar_thumb(ThumbState::Hover)))
                .thumb_active(|t| t.width(wide).inset(inset).radius(round).bg(p.scrollbar_thumb(ThumbState::Active)))
        })
        .into_any_element()
}

fn ctx_state(ctx: &Ctx) -> ListState {
    ctx.state.clone()
}

impl RenderOnce for SidebarScreen {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let targets: Rc<RefCell<HashMap<usize, Target>>> = Rc::new(RefCell::new(HashMap::new()));
        let rows = screen_list(&self.id, targets.clone(), self.on_intent.clone());
        let items = rows.items(window, cx);
        let ctx = Ctx {
            id: self.id.clone(),
            model: Rc::new(self.model.clone()),
            items,
            targets,
            rename_input: self.rename_input.clone(),
            on_intent: self.on_intent.clone(),
            state: self.state.clone(),
            width: self.width,
        };

        // The bar above the column (the shell's) carries the toggle and the
        // mode switch; the column starts with its rows.
        let rows = build_list(&ctx, rows, window, cx);
        let body = div().w_full().flex_1().min_h(px(0.0)).flex().flex_col().px(metrics.item_px).pt(metrics.item_px).relative().child(rows);

        // Footer (docs/83 §4): the neutral avatar tile, the name (Body) and
        // the plan (Caption, muted), the settings gear at the right, in a
        // 44 px row with a 1 px `line-region` on top (§3). While providers
        // are checked the tile spins and the name reads muted.
        let account = &self.model.account;
        let checking = account.status == AccountStatus::Checking;
        let who = h_flex()
            .flex_1()
            .min_w(px(0.0))
            .items_center()
            // A long name truncates; the plan stays whole beside it.
            .child(
                div()
                    .flex_shrink(1.0)
                    .min_w(px(0.0))
                    .truncate()
                    .text_role(TextRole::Body)
                    .text_color(if checking { p.fg_muted } else { p.fg })
                    .child(account.name.clone()),
            )
            .when(!account.plan.is_empty(), |d| {
                d.child(div().flex_none().ml(px(scale::SP_2)).role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(format!("· {}", account.plan)))
            });
        let foot_ctx = ctx.clone();
        let foot_label: SharedString = if account.plan.is_empty() {
            format!("Account {}", account.name).into()
        } else {
            format!("Account {} · {}", account.name, account.plan).into()
        };
        let foot_row = h_flex()
            .w_full()
            .h(list_row_height(cx))
            .items_center()
            .gap(metrics.gap)
            .pl(px(ROW_PAD_X))
            .pr(px(ROW_PAD_X))
            .child(avatar_tile(account.initial.clone(), account.status, ctx.child("avatar"), cx))
            .child(who);
        let gear_ctx = ctx.clone();
        let gear = icon_button(ctx.child("settings"), IconName::Gear)
            .tooltip_within(self.width)
            .ghost()
            .size(ButtonSize::Sm)
            .icon_size(metrics.icon)
            .accessibility_label("Settings")
            .on_click(move |_, w, cx| gear_ctx.emit(SidebarIntent::AccountAction(AccountAction::Settings), w, cx));
        let footer = h_flex()
            .w_full()
            .h(metrics.bar)
            .flex_none()
            .items_center()
            .gap(px(scale::SP_1))
            .px(metrics.item_px)
            .border_t_1()
            .border_color(p.line_region)
            .child(
                div().flex_1().min_w(px(0.0)).child(
                    interactive(ctx.child("footer"), foot_row)
                        .label(foot_label)
                        .on_click(move |_, w, cx| foot_ctx.emit(SidebarIntent::Account, w, cx)),
                ),
            )
            .child(gear);

        let mut column = v_flex().w(self.width).h_full().relative().when(self.ground, |d| d.bg(p.sidebar)).child(body).child(footer);

        // Menus, positioned from the ladder: the view-options menu under
        // the Projects header, the account menu standing on the footer's
        // line (no gap: list text showed through one, seen live). Both
        // paint on gpui's top layer (`deferred`), so a submenu can reach
        // past the sidebar's edge and nothing clips them.
        let menu_w = self.width - metrics.item_px * 2.0;
        // A press outside an open menu dismisses it (FF: the dropdown closes
        // on an outside pointer-down).
        let dismiss = |ctx: &Ctx| {
            let c = ctx.clone();
            move |_: &gpui::MouseDownEvent, w: &mut Window, cx: &mut App| c.emit(SidebarIntent::DismissMenus, w, cx)
        };
        if self.model.view_menu_open {
            let top = projects_header_bottom(&self.model, cx);
            column = column.child(
                div()
                    .absolute()
                    .left(metrics.item_px)
                    .top(top)
                    .child(
                        gpui::deferred(
                            gpui::anchored().child(
                                div()
                                    .id(ctx.child("view-menu-wrap"))
                                    .on_mouse_down_out(dismiss(&ctx))
                                    .child(view_options_menu(ctx.child("view-menu"), &self.model.view_options, menu_w, self.on_intent.clone(), window, cx)),
                            ),
                        )
                        .with_priority(1),
                    ),
            );
        }
        if account.menu_open {
            column = column.child(
                div()
                    .absolute()
                    .left(metrics.item_px)
                    .right(metrics.item_px)
                    .bottom(metrics.bar)
                    .occlude()
                    .on_mouse_down_out(dismiss(&ctx))
                    .child(account_menu(ctx.child("account-menu"), account, menu_w, self.on_intent.clone(), window, cx)),
            );
        }
        // Browse's right-click menu at the pointer — unless the host draws it
        // itself over a native page (an overlay window, [`Self::external_menus`]).
        if !self.external_menus {
            if let Some(browse) = self.model.browse.as_ref().filter(|_| self.model.mode == Mode::Browse) {
                if let (Some(menu), Some(element)) = (browse.menu.as_ref(), super::menus::browse_menu(ctx.child("browse-menu"), browse, self.on_intent.clone(), window, cx)) {
                    let at = menu.at;
                    column = column.child(gpui::deferred(gpui::anchored().position(at).snap_to_window().child(element)).with_priority(1));
                }
            }
        }
        column.into_any_element()
    }
}

/// Where the Projects header ends, measured down the ladder from the top
/// of the sidebar: the menu that drops from it starts here. Every child of
/// the row list stands `ROW_GAP` after the one before it.
fn projects_header_bottom(model: &SidebarModel, cx: &App) -> Pixels {
    let metrics = cx.aui().metrics;
    let row = list_row_height(cx);
    let gap = px(ROW_GAP);
    let mut y = metrics.header + metrics.item_px; // title bar + body pad
    y += row + gap; // search
    y += row + gap; // new session
    y += (row + gap) * model.nav.len() as f32;
    if !model.pinned.is_empty() {
        y += px(scale::SP_4 - ROW_GAP) + gap + label_height(cx) + gap;
        if model.pinned_open {
            for session in &model.pinned {
                y += session_row_height(second_line_for(session, model.bylines).is_some(), cx) + gap;
            }
        }
    }
    y + px(scale::SP_4) + label_height(cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::{Byline, RowStatus, RowStatusKind};

    fn session() -> SessionSummary {
        SessionSummary::new("s", "Session", AgentState::Idle, "1h")
    }

    fn two(ask: &str, result: &str) -> Byline {
        Byline::TwoLines { ask: ask.to_string().into(), result: result.to_string().into() }
    }

    #[test]
    fn settled_status_never_takes_the_line_so_the_result_shows() {
        let mut s = session();
        s.status = Some(RowStatus::new(RowStatusKind::Settled, "6d · 1 turn"));
        s.byline = Some(two("the ask", "the result"));
        assert_eq!(second_line_for(&s, Bylines::Summary).as_deref(), Some("the result"));
    }

    #[test]
    fn blank_result_falls_back_to_the_ask() {
        let mut s = session();
        s.status = Some(RowStatus::new(RowStatusKind::Settled, ""));
        s.byline = Some(two("the ask", "   "));
        assert_eq!(second_line_for(&s, Bylines::Summary).as_deref(), Some("the ask"));
    }

    #[test]
    fn working_status_wins_over_the_byline() {
        let mut s = session();
        s.status = Some(RowStatus::new(RowStatusKind::Working, "2m"));
        s.byline = Some(two("the ask", "the result"));
        assert_eq!(second_line_for(&s, Bylines::Summary).as_deref(), Some("Working · 2m"));
    }

    #[test]
    fn no_reply_with_no_byline_shows_the_branch() {
        let mut s = session();
        s.status = Some(RowStatus::new(RowStatusKind::NoReply, ""));
        s.branch = Some("main".into());
        assert_eq!(second_line_for(&s, Bylines::Summary).as_deref(), Some("main"));
    }

    #[test]
    fn preview_and_placeholder_bylines_show_their_text() {
        let mut s = session();
        s.byline = Some(Byline::Preview("a preview".into()));
        assert_eq!(second_line_for(&s, Bylines::Summary).as_deref(), Some("a preview"));
        s.byline = Some(Byline::Placeholder("  ".into()));
        s.branch = Some("dev".into());
        assert_eq!(second_line_for(&s, Bylines::Summary).as_deref(), Some("dev"));
    }

    #[test]
    fn summary_off_says_nothing_under_any_title() {
        let mut s = session();
        s.byline = Some(two("a", "b"));
        assert_eq!(second_line_for(&s, Bylines::Off), None);
        s.status = Some(RowStatus::new(RowStatusKind::Failed, "boom"));
        assert_eq!(second_line_for(&s, Bylines::Off), None, "state rides the trailing slot");
        assert_eq!(second_line_for(&s, Bylines::Summary).as_deref(), Some("Failed"));
    }
}
