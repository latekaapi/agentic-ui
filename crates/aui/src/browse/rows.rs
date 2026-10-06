//! Pin grid, tab rows, folder rows, task-folder rows and the back /
//! forward / reload cluster for the Browse sidebar header.

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::browse::{face::task_face, Pin, TabRowData, TaskFace};
use crate::data::{icon_button, record_ax_label, tag, ButtonSize};
use crate::nav::chevron;
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// 30 px rows with an 8 px ground inset, like the compact session rows.
const ROW_H: f32 = 30.0;
const ROW_MARGIN_X: f32 = 8.0;
const ROW_GAP: f32 = 8.0;
const ROW_PAD_RIGHT: f32 = 8.0;
/// Each nesting level indents 22 px.
const INDENT: f32 = 22.0;
/// Letter tiles and folder glyphs: 16 px.
const TILE: f32 = 16.0;
const TILE_TEXT: f32 = 10.0;
/// Pin tiles: 34 px squares in a 4-up grid.
const PIN_TILE: f32 = 34.0;
const PIN_GAP: f32 = 8.0;
const PIN_COLS: usize = 4;
/// The tab close `×`: 12 px glyph on an xs ghost button.
const CLOSE_GLYPH: f32 = 12.0;
/// Row titles: 12.5 px, like the sidebar rows.
const TITLE_TEXT: f32 = 12.5;

type KeyHandler = std::rc::Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// The 16 px letter tile shared by tab rows: `surface-2` ground, `ink-2`
/// letter, clipped to the square, showing only the first character (no
/// grapheme segmenter is available here, so a multi-char mark is cut after
/// its first `char`).
fn letter_tile(letter: SharedString, p: &aui_tokens::Palette) -> impl IntoElement {
    let first: SharedString =
        letter.chars().next().map(|c| c.to_string()).unwrap_or_default().into();
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(TILE))
        .rounded(px(4.0))
        .bg(p.surface_2)
        .text_color(p.ink_2)
        .ui(TILE_TEXT)
        .medium()
        .overflow_hidden()
        .child(first)
}

/// Ground for a 30 px row: `surface-3` when active, `surface-2` on hover.
fn row_bg(
    id: &ElementId,
    active: bool,
    hovered: bool,
    p: &aui_tokens::Palette,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Hsla {
    let ground = if active { p.surface_3 } else { p.surface_2 };
    tint_fade((id.clone(), "bg"), active || hovered, ground, Tween::FAST, window, cx)
}

/// The 4-up grid of 34 px pin tiles. Build with [`pin_grid`].
#[derive(IntoElement)]
pub struct PinGrid {
    id: ElementId,
    pins: Vec<Pin>,
    on_select: Option<KeyHandler>,
}

/// A 4-up grid of 34 px tiles showing one letter each.
pub fn pin_grid(id: impl Into<ElementId>, pins: Vec<Pin>) -> PinGrid {
    PinGrid { id: id.into(), pins, on_select: None }
}

impl PinGrid {
    /// A tile was clicked; the argument is the pin id.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for PinGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut grid = v_flex().gap(px(PIN_GAP)).px(px(ROW_MARGIN_X));
        for (chunk, pins) in self.pins.chunks(PIN_COLS).enumerate() {
            let mut line = h_flex().gap(px(PIN_GAP));
            for (i, pin) in pins.iter().enumerate() {
                let tile_id: ElementId = (id.clone(), SharedString::from(format!("pin-{chunk}-{i}"))).into();
                let (state, flags) = interaction_flags(tile_id.clone(), window, cx);
                // Quiet at rest (border + letter only); the `surface-2`
                // tint fades in on hover.
                let bg = tint_fade((tile_id.clone(), "bg"), flags.hovered, p.surface_2, Tween::FAST, window, cx);
                let label: SharedString = format!("Pinned {}: {}", pin.position, pin.title).into();
                record_ax_label(&label);
                let key = pin.id.clone();
                let mut tile = h_flex()
                    .id(tile_id)
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(PIN_TILE))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.line)
                    .bg(bg)
                    .text_color(p.ink_2)
                    .ui(scale::FS_13)
                    .medium()
                    .cursor_pointer()
                    .role(gpui::Role::Button)
                    .aria_label(label)
                    .track_interaction(&state)
                    .child(pin.letter.clone());
                if let Some(on_select) = self.on_select.clone() {
                    tile = tile.on_click(move |_, w, cx| on_select(&key, w, cx));
                }
                line = line.child(tile);
            }
            grid = grid.child(line);
        }
        grid
    }
}

/// A 30 px tab row: 16 px letter tile, title, optional port tag, and the
/// close `×` — always in the tree, opacity 0 until hovered or active.
/// Build with [`tab_row`].
#[derive(IntoElement)]
pub struct TabRow {
    id: ElementId,
    data: TabRowData,
    on_select: Option<KeyHandler>,
    on_close: Option<KeyHandler>,
}

/// A 30 px tab row for `data`.
pub fn tab_row(id: impl Into<ElementId>, data: TabRowData) -> TabRow {
    TabRow { id: id.into(), data, on_select: None, on_close: None }
}

impl TabRow {
    /// The row was clicked; the argument is the tab title.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }

    /// The close `×` was clicked; the argument is the tab title. The button
    /// stops propagation, so closing never also selects.
    pub fn on_close(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for TabRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let bg = row_bg(&id, self.data.active, flags.hovered, &p, window, cx);
        let label: SharedString = if self.data.active {
            format!("Tab {}: {}, selected", self.data.position, self.data.title).into()
        } else {
            format!("Tab {}: {}", self.data.position, self.data.title).into()
        };
        record_ax_label(&label);
        let mut row = h_flex()
            .id(id.clone())
            .h(px(ROW_H))
            .flex_none()
            .items_center()
            .gap(px(ROW_GAP))
            .ml(px(ROW_MARGIN_X + INDENT * f32::from(self.data.depth)))
            .mr(px(ROW_MARGIN_X))
            .pr(px(ROW_PAD_RIGHT))
            .rounded(px(scale::R_SM))
            .bg(bg)
            .ui(TITLE_TEXT)
            .text_color(if self.data.active { p.ink } else { p.ink_2 })
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(label)
            .track_interaction(&state)
            .child(letter_tile(self.data.letter.clone(), &p))
            .child(div().flex_1().min_w(px(0.0)).truncate().child(self.data.title.clone()));
        if let Some(port) = self.data.port {
            row = row.child(tag(SharedString::from(format!(":{port}"))));
        }
        // The `×` is always in the tree (so it keeps its role and name);
        // only its opacity changes — 0 until hovered or active.
        let show_close = flags.hovered || self.data.active;
        let close_opacity =
            tween((id.clone(), "close"), if show_close { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);
        let title = self.data.title.clone();
        let close_label: SharedString =
            format!("Close tab {}: {}", self.data.position, self.data.title).into();
        let mut close = icon_button((id, "close"), IconName::X)
            .ghost()
            .size(ButtonSize::Xs)
            .icon_size(px(CLOSE_GLYPH))
            .accessibility_label(close_label);
        if let Some(on_close) = self.on_close.clone() {
            close = close.on_click(move |_, w, cx| {
                cx.stop_propagation();
                on_close(&title, w, cx);
            });
        }
        row = row.child(div().flex_none().opacity(close_opacity).child(close));
        if let Some(on_select) = self.on_select {
            let title = self.data.title.clone();
            row = row.on_click(move |_, w, cx| on_select(&title, w, cx));
        }
        row
    }
}

/// A 30 px folder row: chevron, folder glyph, name. Build with [`folder_row`].
#[derive(IntoElement)]
pub struct FolderRow {
    id: ElementId,
    name: SharedString,
    open: bool,
    depth: u8,
    on_toggle: Option<KeyHandler>,
}

/// A 30 px folder row: chevron (open/closed) + folder glyph + name.
pub fn folder_row(id: impl Into<ElementId>, name: impl Into<SharedString>, open: bool, depth: u8) -> FolderRow {
    FolderRow { id: id.into(), name: name.into(), open, depth, on_toggle: None }
}

impl FolderRow {
    /// The row was clicked; the argument is the folder name.
    pub fn on_toggle(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for FolderRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let bg = row_bg(&id, false, flags.hovered, &p, window, cx);
        let label: SharedString = format!("Folder: {}", self.name).into();
        record_ax_label(&label);
        let mut row = h_flex()
            .id(id.clone())
            .h(px(ROW_H))
            .flex_none()
            .items_center()
            .gap(px(ROW_GAP))
            .ml(px(ROW_MARGIN_X + INDENT * f32::from(self.depth)))
            .mr(px(ROW_MARGIN_X))
            .pr(px(ROW_PAD_RIGHT))
            .rounded(px(scale::R_SM))
            .bg(bg)
            .ui(TITLE_TEXT)
            .medium()
            .text_color(p.ink)
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(label)
            .track_interaction(&state)
            .child(chevron((id.clone(), "chevron"), self.open, p.ink_3, window, cx))
            .child(icon(IconName::Folder).size(px(TILE)).color(p.ink_3))
            .child(div().flex_1().min_w(px(0.0)).truncate().child(self.name.clone()));
        if let Some(on_toggle) = self.on_toggle {
            let name = self.name.clone();
            row = row.on_click(move |_, w, cx| on_toggle(&name, w, cx));
        }
        row
    }
}

/// A 30 px task-folder row: like [`FolderRow`] but the folder glyph is the
/// 16 px [`TaskFace`] and the row's end carries the quiet status word, which
/// opens the chat. Build with [`task_folder_row`].
#[derive(IntoElement)]
pub struct TaskFolderRow {
    id: ElementId,
    name: SharedString,
    face: TaskFace,
    status: SharedString,
    open: bool,
    on_toggle: Option<KeyHandler>,
    on_open_chat: Option<KeyHandler>,
}

/// A 30 px task-folder row: chevron + status face + name, quiet status word
/// at the end. The row toggles; the status word opens the chat.
pub fn task_folder_row(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    face: TaskFace,
    status_text: impl Into<SharedString>,
    open: bool,
) -> TaskFolderRow {
    TaskFolderRow {
        id: id.into(),
        name: name.into(),
        face,
        status: status_text.into(),
        open,
        on_toggle: None,
        on_open_chat: None,
    }
}

impl TaskFolderRow {
    /// The row was clicked; the argument is the folder name.
    pub fn on_toggle(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(std::rc::Rc::new(f));
        self
    }

    /// The status word was clicked; the argument is the folder name. There
    /// is no context menu yet, so the word itself is the affordance.
    pub fn on_open_chat(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_chat = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for TaskFolderRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // Hover is tracked on the whole row (both buttons tint together);
        // the toggle and the status word keep their own roles and clicks.
        let row_id: ElementId = (id.clone(), "row").into();
        let (state, flags) = interaction_flags(row_id.clone(), window, cx);
        let bg = row_bg(&row_id, false, flags.hovered, &p, window, cx);
        let label: SharedString = format!("Task folder: {}, {}", self.name, self.status).into();
        record_ax_label(&label);
        // The status button's name is its visible text with the folder
        // name, so it stays unique in the tree.
        let chat_label: SharedString = format!("{}, {}", self.status, self.name).into();
        record_ax_label(&chat_label);
        let status_ink = if self.face == TaskFace::NeedsYou { p.warning } else { p.ink_3 };
        let mut toggle = h_flex()
            .id((id.clone(), "toggle"))
            .flex_1()
            .min_w(px(0.0))
            .items_center()
            .gap(px(ROW_GAP))
            .rounded(px(scale::R_SM))
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(label)
            .child(chevron((id.clone(), "chevron"), self.open, p.ink_3, window, cx))
            .child(task_face(self.face))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .ui(TITLE_TEXT)
                    .medium()
                    .text_color(p.ink)
                    .child(self.name.clone()),
            );
        if let Some(on_toggle) = self.on_toggle.clone() {
            let name = self.name.clone();
            toggle = toggle.on_click(move |_, w, cx| on_toggle(&name, w, cx));
        }
        let mut status = div()
            .id((id, "chat"))
            .flex_none()
            .ui(scale::FS_11)
            .text_color(status_ink)
            .whitespace_nowrap()
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(chat_label);
        if let Some(on_open_chat) = self.on_open_chat.clone() {
            let name = self.name.clone();
            status = status.on_click(move |_, w, cx| {
                cx.stop_propagation();
                on_open_chat(&name, w, cx);
            });
        }
        h_flex()
            .id(row_id)
            .h(px(ROW_H))
            .flex_none()
            .items_center()
            .gap(px(ROW_GAP))
            .ml(px(ROW_MARGIN_X))
            .mr(px(ROW_MARGIN_X))
            .pr(px(ROW_PAD_RIGHT))
            .rounded(px(scale::R_SM))
            .bg(bg)
            .track_interaction(&state)
            .child(toggle)
            .child(status.child(self.status))
    }
}

/// The `‹ › ⟳` trailing cluster for the Browse traffic-light row: three
/// ghost icon buttons. Build with [`browse_nav_cluster`].
#[derive(IntoElement)]
pub struct BrowseNavCluster {
    id: ElementId,
    can_go_back: bool,
    can_go_forward: bool,
    on_back: Option<ClickHandler>,
    on_forward: Option<ClickHandler>,
    on_reload: Option<ClickHandler>,
}

/// The `‹ › ⟳` trailing cluster: Back, Forward, Reload as ghost buttons.
/// The host seats it after a spacer in its header row.
pub fn browse_nav_cluster(id: impl Into<ElementId>) -> BrowseNavCluster {
    BrowseNavCluster {
        id: id.into(),
        can_go_back: false,
        can_go_forward: false,
        on_back: None,
        on_forward: None,
        on_reload: None,
    }
}

impl BrowseNavCluster {
    /// Enables the back arrow.
    pub fn can_go_back(mut self, on: bool) -> Self {
        self.can_go_back = on;
        self
    }

    /// Enables the forward arrow (disabled at 45 % otherwise).
    pub fn can_go_forward(mut self, on: bool) -> Self {
        self.can_go_forward = on;
        self
    }

    /// Back click.
    pub fn on_back(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Box::new(f));
        self
    }

    /// Forward click.
    pub fn on_forward(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_forward = Some(Box::new(f));
        self
    }

    /// Reload click.
    pub fn on_reload(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_reload = Some(Box::new(f));
        self
    }
}

impl RenderOnce for BrowseNavCluster {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let mut cluster = h_flex().gap(px(2.0));
        let mut back = icon_button((id.clone(), "back"), IconName::ArrowLeft)
            .ghost()
            .muted()
            .size(ButtonSize::Sm)
            .accessibility_label("Back")
            .disabled(!self.can_go_back);
        if let Some(on_back) = self.on_back {
            back = back.on_click(move |e, w, cx| on_back(e, w, cx));
        }
        let mut forward = icon_button((id.clone(), "forward"), IconName::ArrowRight)
            .ghost()
            .muted()
            .size(ButtonSize::Sm)
            .accessibility_label("Forward")
            .disabled(!self.can_go_forward);
        if let Some(on_forward) = self.on_forward {
            forward = forward.on_click(move |e, w, cx| on_forward(e, w, cx));
        }
        let mut reload = icon_button((id, "reload"), IconName::Refresh)
            .ghost()
            .muted()
            .size(ButtonSize::Sm)
            .accessibility_label("Reload");
        if let Some(on_reload) = self.on_reload {
            reload = reload.on_click(move |e, w, cx| on_reload(e, w, cx));
        }
        cluster = cluster.child(back).child(forward).child(reload);
        cluster
    }
}

