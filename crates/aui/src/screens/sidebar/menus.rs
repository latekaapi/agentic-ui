//! The sidebar's menus: the view-options menu (with its Group by and Sort
//! by submenus) under the Projects header, the account menu above the
//! footer, and Browse mode's four right-click menus (tab, folder, task
//! folder, space).
//!
//! Every menu is Fluid Functionalism's dropdown (`registry/base/dropdown.tsx`,
//! `registry/default/menu-item.tsx`) on our radii: a [`menu_surface`] panel
//! (substrate + 2, shadow-3, container radius) with a 4 px inset, rows one
//! `control` tall (36) on the item radius, labels muted at rest and lit to
//! the foreground, checked rows in the foreground at the semibold weight
//! with the check mark, the current row of a single-choice submenu on the
//! `active` overlay, separators full-bleed at border/60, disabled rows at
//! 50 %. A menu enters on the popup tier (fade and a 4 px slide); a
//! submenu opens on hover, 2 px clear of its menu, its first row level with
//! the row that opened it.
//!
//! The host owns what each row does through [`SidebarIntent`]; the screen
//! never closes a menu itself.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled, Density, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::record_ax_label;
use crate::interact::{interactive, menu_surface, Shape};
use crate::nav::project_mark;

use super::{
    Account, AccountAction, BrowseIntent, BrowseMenu, BrowseMenuTarget, BrowseModel, FolderAction, GroupKey,
    SidebarIntent, SortKey, SpaceAction, TabAction, TaskFolderAction, ViewOption, ViewOptions,
};

type IntentHandler = Rc<dyn Fn(SidebarIntent, &mut Window, &mut App)>;
/// A row's pointer entry / exit report.
type HoverHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
/// One Browse menu: its state key, accessible name, rows, and the Move to
/// folder submenu (label + rows) when it has one.
type MenuSpec = (String, SharedString, Vec<MenuRow>, Option<(SharedString, Vec<MenuRow>)>);

/// A meter at or past this fraction turns `warning`, as Baaz's account panel does.
const USAGE_WARN: f32 = 0.8;

/// FF's sidebar-anchored menus are at least 240 wide (`sidebar-menu-grid.ts`).
pub(super) fn menu_width() -> Pixels {
    px(scale::SP_7 * 7.5)
}

/// FF's submenu width (`w-56`, 224).
fn submenu_width() -> Pixels {
    px(scale::SP_7 * 7.0)
}

/// What sits at a menu row's right edge.
#[derive(Clone)]
enum Trailing {
    None,
    /// A value (Caption, muted): "Project ›", a count, "Soon".
    Value(String),
    /// A shortcut (Micro, muted).
    Keys(&'static str),
    /// FF's check mark: the row is on.
    Check,
}

/// What one menu row shows and reports.
#[derive(Clone)]
struct MenuRow {
    label: SharedString,
    trailing: Trailing,
    intent: Option<SidebarIntent>,
    danger: bool,
    /// A toggle that is on: foreground, semibold, check mark.
    checked: bool,
    /// The current row of a single-choice submenu: the `active` overlay.
    current: bool,
    disabled: bool,
    /// A separator goes above this row.
    sep_before: bool,
}

impl MenuRow {
    fn new(label: impl Into<SharedString>, intent: SidebarIntent) -> Self {
        Self { label: label.into(), trailing: Trailing::None, intent: Some(intent), danger: false, checked: false, current: false, disabled: false, sep_before: false }
    }

    fn after_separator(mut self) -> Self {
        self.sep_before = true;
        self
    }

    fn value(mut self, text: impl Into<String>) -> Self {
        self.trailing = Trailing::Value(text.into());
        self
    }

    fn keys(mut self, keys: &'static str) -> Self {
        self.trailing = Trailing::Keys(keys);
        self
    }

    fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        if on {
            self.trailing = Trailing::Check;
        }
        self
    }

    fn current(mut self, on: bool) -> Self {
        self.current = on;
        self.checked(on)
    }

    fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// One menu row; `on_hover` reports pointer entry and exit (a submenu
/// trigger opens its submenu on entry).
fn menu_row(id: &ElementId, spec: MenuRow, handler: &Option<IntentHandler>, on_hover: Option<HoverHandler>, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let row_id: ElementId = (id.clone(), spec.label.clone()).into();
    let lit = !spec.disabled && crate::interact::interactive_state(row_id.clone(), window, cx).hovered;
    let label_color = if spec.danger {
        p.danger
    } else if lit || spec.checked {
        p.fg
    } else {
        p.fg_muted
    };
    let mut body = h_flex()
        .h(metrics.control)
        .w_full()
        .items_center()
        .px(metrics.item_px)
        .gap(metrics.gap)
        .when(spec.current, |d| d.bg(p.selected_overlay()))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_role_in(TextRole::Body, Density::Standard)
                .text_color(label_color)
                .when(spec.checked, |d| d.semibold())
                .child(spec.label.clone()),
        );
    body = match &spec.trailing {
        Trailing::None => body,
        Trailing::Value(text) => body.child(div().flex_none().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(text.clone())),
        Trailing::Keys(keys) => body.child(div().flex_none().role_in(TextRole::Micro, cx).text_color(p.fg_muted).child(*keys)),
        Trailing::Check => body.child(icon(IconName::Check).size(metrics.icon).color(p.fg).lit(true)),
    };
    let body = Shape::Item.rounded(body);
    let role = match spec.trailing {
        Trailing::Check if !spec.current => "menuitemcheckbox",
        _ if spec.current => "menuitemradio",
        _ => "menuitem",
    };
    let mut row = interactive(row_id.clone(), body).role(role).shape(Shape::Item).label(spec.label.clone()).disabled(spec.disabled);
    let emit = handler.clone();
    let intent = spec.intent.clone();
    // A menu click never reaches the list beneath the menu.
    row = row.on_click(move |_, w, cx| {
        cx.stop_propagation();
        if let (Some(emit), Some(intent)) = (emit.as_ref(), intent.as_ref()) {
            emit(intent.clone(), w, cx);
        }
    });
    let mut wrap = div().id((row_id, "hover")).w_full().child(row);
    if let Some(on_hover) = on_hover {
        wrap = wrap.on_hover(move |hovered, w, cx| on_hover(*hovered, w, cx));
    }
    wrap.into_any_element()
}

/// FF's separator: one hairline at border/60, full-bleed through the
/// menu's 4 px inset, 4 px above and below.
fn separator(cx: &App) -> AnyElement {
    let metrics = cx.aui().metrics;
    let mut line = cx.aui().colors.line;
    line.a *= 0.6;
    div().h(px(1.0)).mx(-(metrics.item_px / 2.0)).my(metrics.item_px / 2.0).bg(line).into_any_element()
}

/// FF's group label: Caption, muted, 28 tall.
fn group_label(text: &'static str, cx: &App) -> AnyElement {
    let metrics = cx.aui().metrics;
    div().h(metrics.control_md).px(metrics.item_px).flex().items_center().role_in(TextRole::Caption, cx).text_color(cx.aui().colors.fg_muted).child(text).into_any_element()
}

/// A menu panel: the popup surface, the 4 px inset, the menu role, a click
/// on its own surface reported as [`SidebarIntent::MenuBackdrop`], and the
/// popup enter (fade + a 4 px slide down, FF `--popup-enter-y`).
fn panel(id: &ElementId, label: SharedString, width: Pixels, rows: Vec<AnyElement>, on_intent: &Option<IntentHandler>, window: &mut Window, cx: &mut App) -> AnyElement {
    let metrics = cx.aui().metrics;
    record_ax_label(&label);
    let backdrop = on_intent.clone();
    let enter = PresenceStyle::fade_rise(presence((id.clone(), "enter"), true, EnterExit::POPUP, window, cx), -(scale::SP_2));
    div()
        .relative()
        .top(enter.offset_y)
        .opacity(enter.opacity)
        .w(width)
        .child(menu_surface(
            v_flex()
                .id((id.clone(), "list"))
                .w_full()
                .p(metrics.item_px / 2.0)
                .role(gpui::Role::Menu)
                .aria_label(label)
                .occlude()
                .on_click(move |_, w, cx| {
                    cx.stop_propagation();
                    if let Some(emit) = backdrop.as_ref() {
                        emit(SidebarIntent::MenuBackdrop, w, cx);
                    }
                })
                .children(rows),
        ))
        .into_any_element()
}

/// Which submenu of the view-options menu is open (hover-driven, kept in
/// window state under the menu's id: the host never sees it).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewSub {
    Group,
    Sort,
}

/// The view-options menu under the Projects header, `width` wide (the
/// sidebar less its gutters), with the Group by / Sort by submenu beside it
/// while the pointer rests on either row. Every row reports its own
/// [`ViewOption`]; the values and checks come from `options`.
pub fn view_options_menu(id: impl Into<ElementId>, options: &ViewOptions, width: Pixels, on_intent: Option<IntentHandler>, window: &mut Window, cx: &mut App) -> AnyElement {
    let id: ElementId = id.into();
    let metrics = cx.aui().metrics;
    let sub = window.use_keyed_state((id.clone(), "sub"), cx, |_, _| None::<ViewSub>);
    let hover_for = |target: Option<ViewSub>| -> HoverHandler {
        let sub = sub.clone();
        Rc::new(move |hovered: bool, _w: &mut Window, cx: &mut App| {
            if hovered && *sub.read(cx) != target {
                sub.update(cx, |s, cx| {
                    *s = target;
                    cx.notify();
                });
            }
        })
    };
    let hidden = if options.show_hidden || options.hidden_count == 0 { None } else { Some(options.hidden_count.to_string()) };
    let mut hidden_row = MenuRow::new("Show hidden", SidebarIntent::ViewOption(ViewOption::ToggleHidden)).checked(options.show_hidden);
    if let Some(count) = hidden {
        hidden_row = hidden_row.value(count);
    }
    let pr_row = if options.pr_status_available {
        MenuRow::new("Show PR status", SidebarIntent::ViewOption(ViewOption::TogglePrStatus)).checked(options.show_pr_status)
    } else {
        MenuRow { intent: None, ..MenuRow::new("Show PR status", SidebarIntent::MenuBackdrop) }.value("Soon").disabled(true)
    };
    let rows: Vec<(MenuRow, Option<ViewSub>)> = vec![
        (MenuRow::new("Group by", SidebarIntent::MenuBackdrop).value(format!("{} ›", options.group_by.label())), Some(ViewSub::Group)),
        (MenuRow::new("Sort by", SidebarIntent::MenuBackdrop).value(format!("{} ›", options.sort_by.label())), Some(ViewSub::Sort)),
    ];
    let mut children: Vec<AnyElement> = Vec::new();
    for (row, target) in rows {
        children.push(menu_row(&id, row, &on_intent, Some(hover_for(target)), window, cx));
    }
    children.push(separator(cx));
    for row in [
        MenuRow::new("Show empty projects", SidebarIntent::ViewOption(ViewOption::ToggleEmpty)).checked(options.show_empty),
        hidden_row,
        MenuRow::new("Show archived", SidebarIntent::ViewOption(ViewOption::ToggleArchived)).checked(options.show_archived),
        pr_row,
    ] {
        children.push(menu_row(&id, row, &on_intent, Some(hover_for(None)), window, cx));
    }
    children.push(separator(cx));
    for row in [
        MenuRow::new("Search all projects", SidebarIntent::ViewOption(ViewOption::ToggleSearchAll)).checked(options.search_all_projects),
        MenuRow::new("Clear empty sessions", SidebarIntent::ViewOption(ViewOption::ClearEmpty)),
    ] {
        children.push(menu_row(&id, row, &on_intent, Some(hover_for(None)), window, cx));
    }
    let main = panel(&id, "View options".into(), width, children, &on_intent, window, cx);
    let open = *sub.read(cx);
    let submenu = open.map(|which| {
        let sub_id: ElementId = (id.clone(), if which == ViewSub::Group { "group-by" } else { "sort-by" }).into();
        let rows: Vec<AnyElement> = match which {
            ViewSub::Group => GroupKey::ALL
                .iter()
                .map(|key| menu_row(&sub_id, MenuRow::new(key.label(), SidebarIntent::ViewOption(ViewOption::GroupBy(*key))).current(*key == options.group_by), &on_intent, None, window, cx))
                .collect(),
            ViewSub::Sort => SortKey::ALL
                .iter()
                .map(|key| menu_row(&sub_id, MenuRow::new(key.label(), SidebarIntent::ViewOption(ViewOption::SortBy(*key))).current(*key == options.sort_by), &on_intent, None, window, cx))
                .collect(),
        };
        let label = if which == ViewSub::Group { "Group by" } else { "Sort by" };
        // First row level with its trigger: the trigger's top less the 4 px inset.
        let top = if which == ViewSub::Group { px(0.0) } else { metrics.control };
        div().absolute().left(width + px(scale::SP_1)).top(top).child(panel(&sub_id, label.into(), submenu_width(), rows, &on_intent, window, cx)).into_any_element()
    });
    div().relative().w(width).child(main).children(submenu).into_any_element()
}

/// One provider's usage card: mark, name, plan, then a meter per line.
fn provider_card(id: &ElementId, usage: &super::ProviderUsage, cx: &App) -> AnyElement {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let mut card = v_flex().w_full().px(metrics.item_px).py(metrics.gap / 2.0);
    card = card.child(
        h_flex()
            .h(metrics.control_sm)
            .items_center()
            .gap(metrics.gap / 2.0)
            .w_full()
            .child(project_mark(usage.initial.clone(), p.label(usage.tint)).size(metrics.icon - px(scale::SP_1)))
            .child(div().flex_none().text_role_in(TextRole::Body, Density::Standard).text_color(p.fg).child(usage.name.clone()))
            .child(div().flex_1().min_w(px(0.0)).truncate().role_in(TextRole::Micro, cx).text_color(p.fg_muted).child(usage.plan.clone())),
    );
    for (index, line) in usage.lines.iter().enumerate() {
        let fraction = line.fraction.clamp(0.0, 1.0);
        let hot = fraction >= USAGE_WARN;
        let percent = format!("{}%", (fraction * 100.0).round() as u32);
        // Label left, `percent · resets` right; the label and the reset words
        // truncate so a long reading ("5h reset – no new reading yet") can
        // never push past the menu's edge.
        card = card
            .child(
                h_flex()
                    .w_full()
                    .h(metrics.control_xs)
                    .items_center()
                    .gap(metrics.gap)
                    .role_in(TextRole::Caption, cx)
                    .text_color(p.fg_muted)
                    .child(div().flex_1().min_w(px(0.0)).truncate().child(line.label.clone()))
                    .child(div().flex_none().text_color(if hot { p.warning } else { p.ink_2 }).medium().child(percent))
                    .when(!line.resets.is_empty(), |d| {
                        d.child(div().flex_shrink(1.0).min_w(px(0.0)).max_w(px(scale::SP_7 * 3.0)).truncate().child(line.resets.clone()))
                    }),
            )
            .child(
                Shape::Pill.rounded(
                    div()
                        .id((id.clone(), SharedString::from(format!("meter-{}-{index}", usage.name))))
                        .w_full()
                        .h(px(scale::SP_1))
                        .my(px(scale::SP_1))
                        .bg(p.surface(4))
                        .overflow_hidden()
                        .child(Shape::Pill.rounded(div().h_full().w(gpui::relative(fraction)).bg(if hot { p.warning } else { p.ink_2 }))),
                ),
            );
    }
    card.into_any_element()
}

/// The account menu above the footer: identity, every provider's usage,
/// then Settings, Providers and Sign out, as Baaz's account panel has them.
pub fn account_menu(id: impl Into<ElementId>, account: &Account, width: Pixels, on_intent: Option<IntentHandler>, window: &mut Window, cx: &mut App) -> AnyElement {
    let id: ElementId = id.into();
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let label: SharedString = format!("Account menu for {}", account.name).into();
    let identity = h_flex()
        .h(metrics.control_lg)
        .w_full()
        .items_center()
        .px(metrics.item_px)
        .gap(metrics.gap)
        .child(project_mark(account.initial.clone(), p.label(account.tint)).size(metrics.icon + metrics.gap / 2.0))
        .child(div().flex_1().min_w(px(0.0)).truncate().text_role_in(TextRole::Body, Density::Standard).text_color(p.fg).child(account.name.clone()))
        // The email only when it says something the name does not.
        .when(!account.email.is_empty() && account.email != account.name, |d| {
            d.child(div().flex_shrink(1.0).min_w(px(0.0)).max_w(px(scale::SP_7 * 3.0)).truncate().role_in(TextRole::Micro, cx).text_color(p.fg_muted).child(account.email.clone()))
        });
    let mut rows: Vec<AnyElement> = vec![identity.into_any_element(), separator(cx), group_label("Usage", cx)];
    for usage in &account.usage {
        rows.push(provider_card(&id, usage, cx));
    }
    rows.push(separator(cx));
    rows.push(menu_row(&id, MenuRow::new("Settings…", SidebarIntent::AccountAction(AccountAction::Settings)).keys("⌘,"), &on_intent, None, window, cx));
    rows.push(menu_row(&id, MenuRow::new("Providers…", SidebarIntent::AccountAction(AccountAction::Providers)), &on_intent, None, window, cx));
    rows.push(separator(cx));
    rows.push(menu_row(&id, MenuRow::new("Sign out of Muse", SidebarIntent::AccountAction(AccountAction::SignOut)).danger(), &on_intent, None, window, cx));
    panel(&id, label, width, rows, &on_intent, window, cx)
}

/// Browse mode's open right-click menu for `browse.menu`, with the Move to
/// folder submenu beside the tab menu while the pointer rests on that row.
/// `None` when no menu is open. The element is the panel alone, unpositioned:
/// the screen seats it at the pointer, and a host that draws it over a
/// native page (an overlay window) seats it there instead — see
/// [`browse_menu_size`].
pub fn browse_menu(id: impl Into<ElementId>, browse: &BrowseModel, on_intent: Option<IntentHandler>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let menu = browse.menu.as_ref()?;
    let id: ElementId = id.into();
    let metrics = cx.aui().metrics;
    let b = |intent: BrowseIntent| SidebarIntent::Browse(intent);
    let (key, label, rows, sub): MenuSpec = match &menu.target {
        BrowseMenuTarget::Tab { id: tab, pinned, in_folder } => {
            let title = browse
                .tabs
                .iter()
                .chain(browse.folders.iter().flat_map(|f| f.tabs.iter()))
                .chain(browse.task_folders.iter().flat_map(|f| f.tabs.iter()))
                .find(|t| &t.id == tab)
                .map(|t| t.title.clone())
                .unwrap_or_default();
            let act = |a: TabAction| b(BrowseIntent::Tab(tab.clone(), a));
            let mut rows = vec![
                MenuRow::new("Duplicate", act(TabAction::Duplicate)),
                MenuRow::new("Copy link", act(TabAction::CopyLink)).keys("⇧⌘C"),
                MenuRow::new(if *pinned { "Unpin tab" } else { "Pin tab" }, act(TabAction::TogglePin)),
                MenuRow::new("Move to folder", SidebarIntent::MenuBackdrop).value("›"),
            ];
            if *in_folder {
                rows.push(MenuRow::new("Remove from folder", act(TabAction::RemoveFromFolder)));
            }
            rows.push(MenuRow::new("Close tab", act(TabAction::Close)).keys("⌘W").after_separator());
            rows.push(MenuRow::new("Close other tabs", act(TabAction::CloseOthers)));
            let mut sub_rows: Vec<MenuRow> = browse
                .folders
                .iter()
                .map(|folder| MenuRow::new(folder.name.clone(), act(TabAction::MoveToFolder(folder.id.clone()))))
                .collect();
            sub_rows.push(MenuRow::new("New folder", act(TabAction::MoveToNewFolder)));
            (format!("tab-{tab}"), format!("Tab {title}").into(), rows, Some(("Move to folder".into(), sub_rows)))
        }
        BrowseMenuTarget::Folder(folder) => {
            let name = browse.folders.iter().find(|f| &f.id == folder).map(|f| f.name.clone()).unwrap_or_default();
            let act = |a: FolderAction| b(BrowseIntent::Folder(folder.clone(), a));
            (
                format!("folder-{folder}"),
                format!("Folder {name}").into(),
                vec![
                    MenuRow::new("Rename", act(FolderAction::Rename)),
                    MenuRow::new("Close its tabs", act(FolderAction::CloseTabs)),
                    MenuRow::new("Delete folder", act(FolderAction::Delete)).danger().after_separator(),
                ],
                None,
            )
        }
        BrowseMenuTarget::TaskFolder { session, running } => {
            let name = browse.task_folders.iter().find(|f| &f.id == session).map(|f| f.name.clone()).unwrap_or_default();
            let act = |a: TaskFolderAction| b(BrowseIntent::TaskFolder(session.clone(), a));
            let mut rows = vec![MenuRow::new("Open chat", act(TaskFolderAction::OpenChat))];
            if *running {
                rows.push(MenuRow::new("Stop", act(TaskFolderAction::Stop)));
            }
            rows.push(MenuRow::new("Close its tabs", act(TaskFolderAction::CloseTabs)).after_separator());
            (format!("task-{session}"), format!("Task folder {name}").into(), rows, None)
        }
        BrowseMenuTarget::Space { id: space, last } => {
            let name = browse.spaces.iter().find(|s| &s.id == space).map(|s| s.name.clone()).unwrap_or_default();
            let act = |a: SpaceAction| b(BrowseIntent::Space(space.clone(), a));
            (
                format!("space-{space}"),
                format!("Space {name}").into(),
                vec![MenuRow::new("Rename", act(SpaceAction::Rename)), MenuRow::new("Delete space", act(SpaceAction::Delete)).danger().disabled(*last).after_separator()],
                None,
            )
        }
    };
    let menu_id: ElementId = (id.clone(), SharedString::from(key)).into();
    let sub_open = window.use_keyed_state((menu_id.clone(), "sub"), cx, |_, _| false);
    let mut children: Vec<AnyElement> = Vec::new();
    let trigger_index = rows.iter().position(|r| r.label.as_ref() == "Move to folder");
    let mut separators_above_trigger = 0usize;
    for (index, row) in rows.into_iter().enumerate() {
        let trigger = Some(index) == trigger_index;
        if row.sep_before {
            children.push(separator(cx));
            if trigger_index.is_some_and(|t| index <= t) {
                separators_above_trigger += 1;
            }
        }
        let flag = sub_open.clone();
        let hover: HoverHandler = Rc::new(move |hovered, _w, cx| {
            if hovered && *flag.read(cx) != trigger {
                flag.update(cx, |open, cx| {
                    *open = trigger;
                    cx.notify();
                });
            }
        });
        children.push(menu_row(&menu_id, row, &on_intent, Some(hover), window, cx));
    }
    let main = panel(&menu_id, label, menu_width(), children, &on_intent, window, cx);
    let submenu = match (sub, trigger_index) {
        (Some((sub_label, sub_rows)), Some(trigger)) if *sub_open.read(cx) => {
            let sub_id: ElementId = (menu_id.clone(), "sub-menu").into();
            let new_folder_at = sub_rows.len().saturating_sub(1);
            let mut sub_children: Vec<AnyElement> = Vec::new();
            for (index, row) in sub_rows.into_iter().enumerate() {
                if index == new_folder_at && index > 0 {
                    sub_children.push(separator(cx));
                }
                sub_children.push(menu_row(&sub_id, row, &on_intent, None, window, cx));
            }
            let top = metrics.control * trigger as f32 + (px(1.0) + metrics.item_px) * separators_above_trigger as f32;
            Some(div().absolute().left(menu_width() + px(scale::SP_1)).top(top).child(panel(&sub_id, sub_label, submenu_width(), sub_children, &on_intent, window, cx)).into_any_element())
        }
        _ => None,
    };
    Some(div().relative().w(menu_width()).child(main).children(submenu).into_any_element())
}

/// The box an overlay window needs to show [`browse_menu`] for `menu`:
/// the menu's width plus room for its submenu, and its height — rows,
/// separators and the inset. Hosts size their overlay with this before
/// the first frame lays the menu out.
pub fn browse_menu_size(menu: &BrowseMenu, folders: usize, cx: &App) -> gpui::Size<Pixels> {
    let metrics = cx.aui().metrics;
    let inset = metrics.item_px; // 4 above + 4 below
    let separator = px(1.0) + metrics.item_px; // 1 px + 4 above + 4 below
    let (rows, separators, submenu) = match &menu.target {
        BrowseMenuTarget::Tab { in_folder, .. } => {
            let rows = 4 + usize::from(*in_folder) + 2;
            let sub_rows = folders + 1;
            let sub_height = metrics.control * sub_rows as f32 + inset + if folders > 0 { separator } else { px(0.0) };
            (rows, 1, Some(sub_height + metrics.control * 3.0))
        }
        BrowseMenuTarget::Folder(_) => (3, 1, None),
        BrowseMenuTarget::TaskFolder { running, .. } => (2 + usize::from(*running), 1, None),
        BrowseMenuTarget::Space { .. } => (2, 1, None),
    };
    let height = metrics.control * rows as f32 + separator * separators as f32 + inset;
    let width = if submenu.is_some() { menu_width() + px(scale::SP_1) + submenu_width() } else { menu_width() };
    gpui::size(width, submenu.map_or(height, |sub| height.max(sub)))
}
