//! Screens v2 · sidebar. Six cards that mirror the approved mock's boards
//! one to one (`harness/docs/30-sidebar-spec`): the populated window, the
//! five states, the Browse window, the new tab, collapsed + peek, and the
//! compact density. Same sample data, same sizes, so a capture and the
//! mock PNG diff pixel for pixel.
//!
//! Every card honours `AUI_GALLERY_STEPS` (comma separated): `hover:<id>`
//! puts the pointer highlight on that session (or `search` / `new-session`),
//! `select:<id>` selects a session, `rename:<id>` opens the inline field,
//! `key:<id>` keyboard-highlights a row, `mode:browse` / `mode:sessions`,
//! `collapse`, `menus` (both menus open), `loading`, `empty`, `leave`;
//! Browse: `tab-menu:<tab>`, `folder-menu:<folder>`, `task-menu:<session>`,
//! `space-menu:<space>` (a right-click menu at the row), `name-folder:<id>`,
//! `name-space` (naming a new space), `no-tabs`; `hidden:<n>` sets the
//! hidden-session count the view menu shows.

use std::rc::Rc;

use aui::interact::{surface, FluidHandle, Shape};
use aui::nav::{sidebar_list_state, Grouping};
use aui::screens::sidebar::{
    sample_model, screen_item_count, sidebar_screen, BrowseEditing, BrowseMenu, BrowseMenuTarget, BrowseNaming, EditingState, Mode, SidebarModel,
    SIDEBAR_WIDTH,
};
use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, Density, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, IntoElement, Window};
use gpui_kit::base::{h_flex, v_flex};

const SIDEBAR_W: f32 = SIDEBAR_WIDTH;
const COLUMN_H: f32 = 720.0;
const WINDOW_W: f32 = 1440.0;
const WINDOW_H: f32 = 900.0;
/// The gap between states columns (`SP_7`), with a hairline between them.
const COLUMN_GAP: f32 = scale::SP_7;

fn gallery_steps() -> Vec<String> {
    std::env::var("AUI_GALLERY_STEPS")
        .map(|value| value.split(',').map(|step| step.trim().to_string()).filter(|step| !step.is_empty()).collect())
        .unwrap_or_default()
}

/// Applies the gallery steps to a model.
fn apply_steps(mut model: SidebarModel) -> SidebarModel {
    for step in gallery_steps() {
        if let Some(rest) = step.strip_prefix("hover:") {
            // A hover step pins both the actions (hovered) and the block
            // (highlighted), since a capture has no pointer.
            model.hovered = Some(rest.to_string().into());
            model.highlighted = Some(rest.to_string().into());
        } else if let Some(rest) = step.strip_prefix("select:") {
            model.selected = Some(rest.to_string().into());
        } else if let Some(rest) = step.strip_prefix("rename:") {
            let draft = find_title(&model, rest).unwrap_or_else(|| rest.to_string().into());
            model.editing = Some(EditingState { session: rest.to_string().into(), draft });
        } else if let Some(rest) = step.strip_prefix("key:") {
            model.highlighted = Some(rest.to_string().into());
        } else if step == "mode:browse" {
            model.mode = Mode::Browse;
        } else if step == "mode:sessions" {
            model.mode = Mode::Sessions;
        } else if step == "collapse" {
            model.collapsed = true;
        } else if step == "menus" {
            model.view_menu_open = true;
            model.account.menu_open = true;
        } else if step == "loading" {
            model.loading = true;
        } else if step == "empty" {
            model.grouping = Rc::new(Grouping::Project(Vec::new()));
            model.pinned.clear();
            model.selected = None;
        } else if let Some(rest) = step.strip_prefix("hidden:") {
            model.view_options.hidden_count = rest.parse().unwrap_or(0);
        } else if let Some(browse) = model.browse.as_mut().filter(|_| {
            ["tab-menu:", "folder-menu:", "task-menu:", "space-menu:", "name-folder:"].iter().any(|p| step.starts_with(p)) || step == "name-space" || step == "no-tabs"
        }) {
            // A right-click lands where a person would click: mid-row, at the
            // row's y in the Browse board (the menu seats at the pointer).
            let at = |y: f32| gpui::point(px(150.0), px(y));
            if let Some(rest) = step.strip_prefix("tab-menu:") {
                let in_folder = browse.folders.iter().any(|f| f.tabs.iter().any(|t| t.id.as_ref() == rest));
                browse.menu = Some(BrowseMenu { target: BrowseMenuTarget::Tab { id: rest.to_string().into(), pinned: false, in_folder }, at: at(322.0) });
            } else if let Some(rest) = step.strip_prefix("folder-menu:") {
                browse.menu = Some(BrowseMenu { target: BrowseMenuTarget::Folder(rest.to_string().into()), at: at(386.0) });
            } else if let Some(rest) = step.strip_prefix("task-menu:") {
                browse.menu = Some(BrowseMenu { target: BrowseMenuTarget::TaskFolder { session: rest.to_string().into(), running: true }, at: at(540.0) });
            } else if let Some(rest) = step.strip_prefix("space-menu:") {
                browse.menu = Some(BrowseMenu { target: BrowseMenuTarget::Space { id: rest.to_string().into(), last: false }, at: gpui::point(px(100.0), px(700.0)) });
            } else if let Some(rest) = step.strip_prefix("name-folder:") {
                browse.editing = Some(BrowseEditing { target: BrowseNaming::Folder(rest.to_string().into()), draft: "New folder".into() });
            } else if step == "name-space" {
                browse.editing = Some(BrowseEditing { target: BrowseNaming::Space(None), draft: "Trip".into() });
            } else if step == "no-tabs" {
                browse.pins.clear();
                browse.tabs.clear();
                browse.folders.clear();
                browse.task_folders.clear();
            }
        } else if step == "leave" {
            model.hovered = None;
            model.hovered_group = None;
            model.hovered_projects_header = false;
        }
    }
    model
}

fn find_title(model: &SidebarModel, id: &str) -> Option<gpui::SharedString> {
    if let Some(session) = model.pinned.iter().find(|s| s.id.as_ref() == id) {
        return Some(session.name.clone());
    }
    if let Grouping::Project(groups) = model.grouping.as_ref() {
        for group in groups {
            if let Some(session) = group.sessions.iter().find(|s| s.id.as_ref() == id) {
                return Some(session.name.clone());
            }
        }
    }
    None
}

thread_local! {
    /// One list state and fluid handle per column id, kept across renders
    /// exactly as a host keeps them: the highlight and selection geometry
    /// is measured one frame and painted the next.
    static COLUMN_STATE: std::cell::RefCell<std::collections::HashMap<&'static str, (gpui::ListState, FluidHandle)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The sidebar at `width` × `height` with its caller-owned list state and
/// fluid handle, on its own ground.
fn sidebar_column(id: &'static str, model: SidebarModel, width: f32, height: f32) -> AnyElement {
    sidebar_column_on(id, model, width, px(height), true)
}

/// [`sidebar_column`] with the ground optional (the peek sits on an
/// elevated surface the card draws).
fn sidebar_column_on(id: &'static str, model: SidebarModel, width: f32, height: gpui::Pixels, ground: bool) -> AnyElement {
    let count = screen_item_count(&model);
    let (state, fluid) = COLUMN_STATE.with(|cell| {
        cell.borrow_mut()
            .entry(id)
            .or_insert_with(|| (sidebar_list_state(count), FluidHandle::new()))
            .clone()
    });
    if state.item_count() != count {
        state.reset(count);
    }
    let mut screen = sidebar_screen(id, model, state, fluid).width(px(width)).on_intent(|_, _, _| {});
    if !ground {
        screen = screen.transparent();
    }
    div().flex_none().w(px(width)).h(height).child(screen).into_any_element()
}

/// The lifted pane beside the sidebar: surface-1, hairline, container
/// radius, shadow-1, inset 8 on three sides; a 44 px header with the
/// session title and the composer at the foot.
fn pane(cx: &App) -> AnyElement {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let header = h_flex()
        .h(metrics.bar)
        .flex_none()
        .w_full()
        .items_center()
        .px(metrics.inset)
        .gap(metrics.gap)
        .border_b_1()
        .border_color(p.line)
        .text_role_in(TextRole::Body, Density::Standard)
        .child(div().text_color(p.fg).child("Design v2 tokens"))
        .child(icon(IconName::ChevronDown).size(metrics.icon).color(p.fg_muted))
        .child(
            surface(2, div().px(px(scale::SP_2) + px(scale::SP_1)).role_in(TextRole::Caption, cx).text_color(p.fg_muted).child("harness"))
                .shape(Shape::Item),
        )
        .child(div().flex_1())
        .child(icon(IconName::Terminal).size(metrics.icon).color(p.fg_muted))
        .child(icon(IconName::Globe).size(metrics.icon).color(p.fg_muted))
        .child(icon(IconName::PanelRight).size(metrics.icon).color(p.fg_muted));
    let composer = surface(
        2,
        div()
            .w_full()
            .h(metrics.bar + metrics.inset + px(scale::SP_2))
            .flex()
            .items_center()
            .px(metrics.ctl_px)
            .text_role_in(TextRole::Body, Density::Standard)
            .text_color(p.fg_muted)
            .child("Type / for commands"),
    )
    .shape(Shape::Container)
    .ring(true);
    div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .pt(metrics.item_px)
        .pr(metrics.item_px)
        .pb(metrics.item_px)
        .child(
            surface(1, v_flex().size_full().child(header).child(div().flex_1()).child(div().px(metrics.inset).pb(metrics.inset).child(composer)))
                .fill()
                .shape(Shape::Container)
                .ring(true)
                .shadow(true),
        )
        .into_any_element()
}

/// The window frame: ground, traffic lights, the sidebar and the pane.
fn window_frame(sidebar: AnyElement, pane_el: AnyElement, cx: &App) -> AnyElement {
    let p = cx.aui().colors;
    h_flex()
        .w(px(WINDOW_W))
        .h(px(WINDOW_H))
        .bg(p.bg)
        .relative()
        .child(sidebar)
        .child(pane_el)
        .child(traffic_lights())
        .into_any_element()
}

/// The three traffic lights at 20,20: macOS window chrome the host draws,
/// in Apple's own colours (not design tokens), so the capture matches the
/// mock's window boards.
fn traffic_lights() -> AnyElement {
    let dot = |colour: gpui::Hsla| Shape::Pill.rounded(div().size(px(scale::SP_4)).bg(colour));
    h_flex()
        .absolute()
        .left(px(scale::SP_5 + scale::SP_2))
        .top(px(scale::SP_5 + scale::SP_2))
        .gap(px(scale::SP_3))
        .child(dot(gpui::rgb(0xFF5F57).into()))
        .child(dot(gpui::rgb(0xFEBC2E).into()))
        .child(dot(gpui::rgb(0x28C840).into()))
        .into_any_element()
}

/// `screens/sidebar`: the populated sidebar beside the lifted pane.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let model = apply_steps(sample_model());
    window_frame(sidebar_column("screens-sidebar", model, SIDEBAR_W, WINDOW_H), pane(cx), cx)
}

/// `screens/sidebar-states`: loading, empty, hover, rename + keyboard,
/// menus — five 260 × 720 columns with a hairline between them.
pub fn build_states(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut loading = sample_model();
    loading.loading = true;
    let mut empty = sample_model();
    empty.grouping = Rc::new(Grouping::Project(Vec::new()));
    empty.pinned.clear();
    empty.selected = None;
    let mut hover = sample_model();
    hover.hovered = Some("checklist".into());
    hover.highlighted = Some("checklist".into());
    hover.hovered_group = Some("harness".into());
    hover.hovered_projects_header = true;
    let mut rename = sample_model();
    rename.editing = Some(EditingState { session: "token-ladder".into(), draft: "Token ladder audit".into() });
    rename.highlighted = Some("relay-capture".into());
    let mut menus = sample_model();
    menus.view_menu_open = true;
    menus.account.menu_open = true;
    let columns = [
        ("screens-sidebar-loading", apply_steps(loading)),
        ("screens-sidebar-empty", apply_steps(empty)),
        ("screens-sidebar-hover", apply_steps(hover)),
        ("screens-sidebar-rename", apply_steps(rename)),
        ("screens-sidebar-menus", apply_steps(menus)),
    ];
    // Five columns, four hairlines between them (`SP_2` = 4 px of borders).
    let mut row = h_flex().w(px(SIDEBAR_W * 5.0 + COLUMN_GAP * 4.0 + scale::SP_2)).h(px(COLUMN_H)).bg(p.bg).gap(px(COLUMN_GAP));
    for (index, (id, model)) in columns.into_iter().enumerate() {
        let mut column = div().flex_none().h(px(COLUMN_H)).flex();
        if index > 0 {
            column = column.border_l_1().border_color(p.line);
        }
        row = row.child(column.child(sidebar_column(id, model, SIDEBAR_W, COLUMN_H)));
    }
    row.into_any_element()
}

/// The web page the Browse window shows: a light site inside the dark
/// app, spanning the full height with no header of its own.
fn site_page(cx: &App) -> AnyElement {
    let metrics = cx.aui().metrics;
    div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .pt(metrics.item_px)
        .pr(metrics.item_px)
        .pb(metrics.item_px)
        .child(surface(1, site_page_content(cx)).fill().shape(Shape::Container).ring(true).shadow(true))
        .into_any_element()
}

/// The page itself: a light site, filling its parent.
pub(crate) fn site_page_content(cx: &App) -> gpui::Div {
    let light = aui_tokens::Palette::for_kind(aui_tokens::ThemeKind::Light);
    let metrics = cx.aui().metrics;
    let line = |width: f32| Shape::Item.rounded(div().h(px(scale::SP_3)).w(gpui::relative(width)).bg(light.surface(2)));
    let stat = |label: &'static str, value: &'static str| {
        Shape::Container.rounded(
            v_flex()
                .flex_1()
                .p(metrics.inset)
                .border_1()
                .border_color(light.line)
                .child(div().role_in(TextRole::Caption, cx).text_color(light.fg_muted).child(label))
                .child(div().mt(px(scale::SP_2)).role_in(TextRole::Display, cx).text_color(light.fg).child(value)),
        )
    };
    v_flex()
        .size_full()
        .bg(light.surface(1))
        .text_color(light.fg)
        .child(
            h_flex()
                .h(metrics.bar + px(scale::SP_4))
                .flex_none()
                .items_center()
                .px(px(scale::SP_7))
                .gap(px(scale::SP_6))
                .border_b_1()
                .border_color(light.line)
                .role_in(TextRole::Body, cx)
                .child(div().role_in(TextRole::Subtitle, cx).semibold().child("UDISE+ · PGI"))
                .child(div().text_color(light.ink_2).child("Home"))
                .child(div().text_color(light.ink_2).child("State scores"))
                .child(div().text_color(light.ink_2).child("Report cards"))
                .child(div().text_color(light.ink_2).child("Open data")),
        )
        .child(
            v_flex()
                .p(px(scale::SP_7 + scale::SP_3))
                .gap(px(scale::SP_6))
                .max_w(px(scale::SP_7 * 32.0))
                .child(div().role_in(TextRole::Caption, cx).text_color(light.fg_muted).child("Performance Grading Index · 2023-24"))
                .child(div().role_in(TextRole::Display, cx).child("State scores 2023-24"))
                .child(h_flex().gap(metrics.inset).child(stat("States graded", "36")).child(stat("Highest band", "Prachesta-2")).child(stat("Updated", "29 Sept 2026")))
                .child(v_flex().gap(px(scale::SP_3)).child(line(0.92)).child(line(0.84)).child(line(0.88)).child(line(0.6))),
        )
}

/// `screens/sidebar-browse`: Browse mode beside the full-height page.
pub fn build_browse(_window: &mut Window, cx: &mut App) -> AnyElement {
    let mut model = sample_model();
    model.mode = Mode::Browse;
    let model = apply_steps(model);
    window_frame(sidebar_column("screens-sidebar-browse", model, SIDEBAR_W, WINDOW_H), site_page(cx), cx)
}

/// `screens/sidebar-newtab`: Browse mode with a new tab: the app's own
/// surface, one omnibox with the accent ring, the pins as tiles.
pub fn build_newtab(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let mut model = sample_model();
    model.mode = Mode::Browse;
    if let Some(browse) = model.browse.as_mut() {
        for tab in browse.tabs.iter_mut() {
            tab.active = false;
        }
    }
    let model = apply_steps(model);
    // The omnibox: surface-2 on the container radius with the accent ring,
    // the one focused control on the page.
    let omnibox = Shape::Container.rounded(h_flex().bg(p.surface(2)).border_1().border_color(p.accent_ring))
        .w(px(scale::SP_7 * 17.5))
        .h(metrics.bar)
        .items_center()
        .px(metrics.ctl_px + px(scale::SP_1))
        .gap(metrics.gap + px(scale::SP_1))
        .child(icon(IconName::Search).size(metrics.icon + px(scale::SP_1)).color(p.fg_muted))
        .child(div().flex_1().role_in(TextRole::Subtitle, cx).text_color(p.fg_muted).child("Search or type a URL"))
        .child(div().role_in(TextRole::Micro, cx).text_color(p.fg_muted).child("⌘L"));
    let tile = |initial: &'static str, label: &'static str| {
        v_flex()
            .w(px(scale::SP_7 * 2.25))
            .items_center()
            .gap(px(scale::SP_3))
            .child(
                surface(
                    3,
                    div()
                        .size(px(scale::SP_7 + scale::SP_5))
                        .flex()
                        .items_center()
                        .justify_center()
                        .role_in(TextRole::Title, cx)
                        .medium()
                        .text_color(p.ink_2)
                        .child(initial),
                )
                .shape(Shape::Container),
            )
            .child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(label))
    };
    let content = v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(scale::SP_7))
        .child(div().child(omnibox).child(div()))
        .child(
            h_flex()
                .gap(metrics.inset)
                .child(tile("U", "Unified dashboard"))
                .child(tile("P", "Performance"))
                .child(tile("D", "Open data"))
                .child(tile("N", "Survey reports"))
                .child(tile("G", "GitHub"))
                .child(tile("L", "Linear")),
        )
        .child(div().role_in(TextRole::Caption, cx).text_color(p.ink_4).child("Ask Claude to open a site, or ⌘⇧L to browse with it"));
    let page = div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .pt(metrics.item_px)
        .pr(metrics.item_px)
        .pb(metrics.item_px)
        .child(surface(1, content).fill().shape(Shape::Container).ring(true).shadow(true))
        .into_any_element();
    window_frame(sidebar_column("screens-sidebar-newtab", model, SIDEBAR_W, WINDOW_H), page, cx)
}

/// `screens/sidebar-collapsed`: the pane fills the window; the toggle
/// stays beside the traffic lights; the peek floats the sidebar in as an
/// elevated surface inset 8 px.
pub fn build_collapsed(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let mut collapsed = sample_model();
    collapsed.collapsed = true;
    let collapsed = apply_steps(collapsed);
    let peek = apply_steps(sample_model());
    let toggle = aui::data::icon_button("collapsed-toggle", IconName::Sidebar)
        .ghost()
        .size(aui::data::ButtonSize::Sm)
        .icon_size(metrics.icon)
        .accessibility_label("Show sidebar");
    let title_strip = metrics.bar + metrics.item_px;
    let pane_full = div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .pt(title_strip)
        .pl(metrics.item_px)
        .pr(metrics.item_px)
        .pb(metrics.item_px)
        .child(
            surface(
                1,
                v_flex()
                    .size_full()
                    .child(
                        h_flex()
                            .h(metrics.bar)
                            .flex_none()
                            .items_center()
                            .px(metrics.inset)
                            .gap(metrics.gap)
                            .border_b_1()
                            .border_color(p.line)
                            .text_role_in(TextRole::Body, Density::Standard)
                            .child(div().text_color(p.fg).child("Design v2 tokens"))
                            .child(icon(IconName::ChevronDown).size(metrics.icon).color(p.fg_muted)),
                    )
                    .child(div().flex_1()),
            )
            .fill()
            .shape(Shape::Container)
            .ring(true)
            .shadow(true),
        );
    let peek_h = px(WINDOW_H) - title_strip - metrics.item_px;
    let peek_card = div()
        .absolute()
        .left(metrics.item_px)
        .top(title_strip)
        .bottom(metrics.item_px)
        .w(px(SIDEBAR_W))
        .child(
            surface(2, div().size_full().child(sidebar_column_on("screens-sidebar-peek", peek, SIDEBAR_W, peek_h, false)))
                .fill()
                .shape(Shape::Container)
                .ring(true)
                .shadow(true),
        );
    h_flex()
        .w(px(WINDOW_W))
        .h(px(WINDOW_H))
        .bg(p.bg)
        .relative()
        .child(traffic_lights())
        .child(div().absolute().left(px(scale::SP_7 * 2.0 + scale::SP_5 + scale::SP_2)).top(px(scale::SP_5 - scale::SP_1)).child(toggle))
        // The 12 px strip is a hover region over the pane's edge, not a column.
        .child(div().absolute().left(px(0.0)).top(title_strip).bottom(px(0.0)).child(sidebar_column_on("screens-sidebar-collapsed", collapsed, scale::SP_4, px(WINDOW_H) - title_strip, true)))
        .child(pane_full)
        .child(peek_card)
        .into_any_element()
}

/// `screens/sidebar-compact`: the populated sidebar under the compact
/// density. The density is a global the card flips when it builds; the
/// gallery renders every capture in its own process, and the interactive
/// gallery's density switch resets it, so the flip does not leak between
/// captures.
pub fn build_compact(window: &mut Window, cx: &mut App) -> AnyElement {
    aui_tokens::AuiTheme::set_density(Density::Compact, Some(window), cx);
    let model = apply_steps(sample_model());
    v_flex().child(sidebar_column("screens-sidebar-compact", model, SIDEBAR_W, COLUMN_H)).into_any_element()
}
