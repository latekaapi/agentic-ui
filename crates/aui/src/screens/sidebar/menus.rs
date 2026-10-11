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

use aui_tokens::{scale, ActiveAui, AuiStyled, Density, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::interact::{dropdown, Dropdown, DropdownEntry, DropdownItem, DropdownSide, DropdownTrailing, Shape};
use crate::nav::project_mark;

use super::{
    Account, AccountAction, BrowseIntent, BrowseMenu, BrowseMenuTarget, BrowseModel, FolderAction, GroupKey,
    PageAction, SidebarIntent, SortKey, SpaceAction, TabAction, TaskFolderAction, ViewOption, ViewOptions,
};

type IntentHandler = Rc<dyn Fn(SidebarIntent, &mut Window, &mut App)>;
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
    /// Opens a submenu: the current value (may be empty), then "›".
    Submenu(String),
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

    fn submenu(mut self, value: impl Into<String>) -> Self {
        self.trailing = Trailing::Submenu(value.into());
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

/// The [`DropdownItem`] for one sidebar row: its intent emitted on select.
fn item(spec: MenuRow, handler: &Option<IntentHandler>) -> DropdownItem {
    let mut item = DropdownItem::new(spec.label.clone()).checked(spec.checked).disabled(spec.disabled);
    if spec.current {
        item = item.current(true);
    }
    if spec.danger {
        item = item.danger();
    }
    item = match spec.trailing {
        Trailing::None | Trailing::Check => item,
        Trailing::Value(text) => item.value(text),
        Trailing::Keys(keys) => item.keys(keys),
        Trailing::Submenu(value) => item.trailing(DropdownTrailing::Submenu(value.into())),
    };
    if let (Some(emit), Some(intent)) = (handler.clone(), spec.intent) {
        item = item.on_select(move |w, cx| emit(intent.clone(), w, cx));
    }
    item
}

/// The entries for `rows` (separators where a row asks for one) and, for
/// each row, its entry index — what the dropdown's highlight reports.
fn entries(rows: Vec<MenuRow>, handler: &Option<IntentHandler>) -> (Vec<DropdownEntry>, Vec<usize>) {
    let mut out = Vec::new();
    let mut at = Vec::new();
    for row in rows {
        if row.sep_before {
            out.push(DropdownEntry::Separator);
        }
        at.push(out.len());
        out.push(DropdownEntry::Item(item(row, handler)));
    }
    (out, at)
}

/// A sidebar menu panel: the one [`Dropdown`], a click on its own surface
/// reported as [`SidebarIntent::MenuBackdrop`].
fn panel(id: &ElementId, label: SharedString, width: Pixels, entries: Vec<DropdownEntry>, on_intent: &Option<IntentHandler>) -> Dropdown {
    let backdrop = on_intent.clone();
    dropdown(id.clone(), label).width(width).entries(entries).on_backdrop(move |w, cx| {
        if let Some(emit) = backdrop.as_ref() {
            emit(SidebarIntent::MenuBackdrop, w, cx);
        }
    })
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
    let rows = vec![
        MenuRow::new("Group by", SidebarIntent::MenuBackdrop).submenu(options.group_by.label()),
        MenuRow::new("Sort by", SidebarIntent::MenuBackdrop).submenu(options.sort_by.label()),
        MenuRow::new("Show empty projects", SidebarIntent::ViewOption(ViewOption::ToggleEmpty)).checked(options.show_empty).after_separator(),
        hidden_row,
        MenuRow::new("Show archived", SidebarIntent::ViewOption(ViewOption::ToggleArchived)).checked(options.show_archived),
        pr_row,
        MenuRow::new("Search all projects", SidebarIntent::ViewOption(ViewOption::ToggleSearchAll)).checked(options.search_all_projects).after_separator(),
        MenuRow::new("Clear empty sessions", SidebarIntent::ViewOption(ViewOption::ClearEmpty)),
    ];
    let (rows, at) = entries(rows, &on_intent);
    // The submenu follows the highlight: open on Group by / Sort by, closed
    // on any other row (FF opens submenus on hover, never on focus).
    let (group_at, sort_at) = (at[0], at[1]);
    let flag = sub.clone();
    let main = panel(&id, "View options".into(), width, rows, &on_intent).on_highlight(move |index, _w, cx| {
        let Some(index) = index else { return };
        let target = if index == group_at {
            Some(ViewSub::Group)
        } else if index == sort_at {
            Some(ViewSub::Sort)
        } else {
            None
        };
        if *flag.read(cx) != target {
            flag.update(cx, |s, cx| {
                *s = target;
                cx.notify();
            });
        }
    });
    let open = *sub.read(cx);
    let submenu = open.map(|which| {
        let sub_id: ElementId = (id.clone(), if which == ViewSub::Group { "group-by" } else { "sort-by" }).into();
        let rows: Vec<MenuRow> = match which {
            ViewSub::Group => GroupKey::ALL.iter().map(|key| MenuRow::new(key.label(), SidebarIntent::ViewOption(ViewOption::GroupBy(*key))).current(*key == options.group_by)).collect(),
            ViewSub::Sort => SortKey::ALL.iter().map(|key| MenuRow::new(key.label(), SidebarIntent::ViewOption(ViewOption::SortBy(*key))).current(*key == options.sort_by)).collect(),
        };
        let label = if which == ViewSub::Group { "Group by" } else { "Sort by" };
        // First row level with its trigger: the trigger's top less the 4 px inset.
        let top = if which == ViewSub::Group { px(0.0) } else { metrics.control };
        let (rows, _) = entries(rows, &on_intent);
        div().absolute().left(width + px(scale::SP_1)).top(top).child(panel(&sub_id, label.into(), submenu_width(), rows, &on_intent).side(DropdownSide::Beside)).into_any_element()
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
pub fn account_menu(id: impl Into<ElementId>, account: &Account, width: Pixels, on_intent: Option<IntentHandler>, _window: &mut Window, cx: &mut App) -> AnyElement {
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
    let mut rows: Vec<DropdownEntry> = vec![DropdownEntry::Static(identity.into_any_element()), DropdownEntry::Separator, DropdownEntry::Label("Usage".into())];
    for usage in &account.usage {
        rows.push(DropdownEntry::Static(provider_card(&id, usage, cx)));
    }
    let (actions, _) = entries(
        vec![
            MenuRow::new("Settings…", SidebarIntent::AccountAction(AccountAction::Settings)).keys("⌘,").after_separator(),
            MenuRow::new("Providers…", SidebarIntent::AccountAction(AccountAction::Providers)),
            MenuRow::new("Sign out of Muse", SidebarIntent::AccountAction(AccountAction::SignOut)).danger().after_separator(),
        ],
        &on_intent,
    );
    rows.extend(actions);
    panel(&id, label, width, rows, &on_intent).side(DropdownSide::Above).into_any_element()
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
        BrowseMenuTarget::Page { safari, devtools, annotating } => {
            let act = |a: PageAction| b(BrowseIntent::Page(a));
            let mut rows = vec![
                MenuRow::new("New tab", act(PageAction::NewTab)).keys("⌘T"),
                MenuRow::new("Find in page", act(PageAction::Find)).keys("⌘F"),
                MenuRow::new("Copy link", act(PageAction::CopyLink)).keys("⇧⌘C").after_separator(),
            ];
            if *safari {
                rows.push(MenuRow::new("Open in Safari", act(PageAction::OpenInSafari)));
            }
            rows.push(MenuRow::new(if *annotating { "Stop annotating" } else { "Annotate page" }, act(PageAction::Annotate)).after_separator());
            rows.push(MenuRow::new("Screenshot", act(PageAction::Screenshot)));
            let mut tools = vec![];
            if *devtools {
                tools.push(MenuRow::new("Developer tools", act(PageAction::DevTools)).keys("⌥⌘I"));
            }
            tools.push(MenuRow::new("Browser settings", act(PageAction::Settings)));
            tools[0] = tools[0].clone().after_separator();
            rows.extend(tools);
            ("page".to_owned(), "Page".into(), rows, None)
        }
    };
    let menu_id: ElementId = (id.clone(), SharedString::from(key)).into();
    let sub_open = window.use_keyed_state((menu_id.clone(), "sub"), cx, |_, _| false);
    let trigger_index = rows.iter().position(|r| r.label.as_ref() == "Move to folder");
    let separators_above_trigger = trigger_index.map_or(0, |t| rows.iter().take(t + 1).filter(|r| r.sep_before).count());
    let rows: Vec<MenuRow> = rows.into_iter().map(|r| if r.label.as_ref() == "Move to folder" { r.submenu("") } else { r }).collect();
    let (rows, at) = entries(rows, &on_intent);
    let trigger_entry = trigger_index.map(|t| at[t]);
    let flag = sub_open.clone();
    let main = panel(&menu_id, label, menu_width(), rows, &on_intent).on_highlight(move |index, _w, cx| {
        let Some(index) = index else { return };
        let open = Some(index) == trigger_entry;
        if *flag.read(cx) != open {
            flag.update(cx, |o, cx| {
                *o = open;
                cx.notify();
            });
        }
    });
    let submenu = match (sub, trigger_index) {
        (Some((sub_label, mut sub_rows)), Some(trigger)) if *sub_open.read(cx) => {
            let sub_id: ElementId = (menu_id.clone(), "sub-menu").into();
            let new_folder_at = sub_rows.len().saturating_sub(1);
            if new_folder_at > 0 {
                sub_rows[new_folder_at] = sub_rows[new_folder_at].clone().after_separator();
            }
            let (sub_entries, _) = entries(sub_rows, &on_intent);
            let top = metrics.control * trigger as f32 + (px(1.0) + metrics.item_px) * separators_above_trigger as f32;
            Some(
                div()
                    .absolute()
                    .left(menu_width() + px(scale::SP_1))
                    .top(top)
                    .child(panel(&sub_id, sub_label, submenu_width(), sub_entries, &on_intent).side(DropdownSide::Beside))
                    .into_any_element(),
            )
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
        BrowseMenuTarget::Page { safari, devtools, .. } => (3 + usize::from(*safari) + 2 + usize::from(*devtools) + 1, 3, None),
    };
    let height = metrics.control * rows as f32 + separator * separators as f32 + inset;
    let width = if submenu.is_some() { menu_width() + px(scale::SP_1) + submenu_width() } else { menu_width() };
    gpui::size(width, submenu.map_or(height, |sub| height.max(sub)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, TestAppContext};

    fn menu(target: BrowseMenuTarget) -> BrowseMenu {
        BrowseMenu { target, at: point(px(0.0), px(0.0)) }
    }

    fn tab(in_folder: bool) -> BrowseMenu {
        menu(BrowseMenuTarget::Tab { id: "t".into(), pinned: false, in_folder })
    }

    /// Owner screenshot 2026-10-12: the page controls' ⋯ opened the Sessions
    /// header menu. The page menu is a Browse menu: no submenu, its rows
    /// (8 with Safari and DevTools, 6 without) under three separators.
    #[gpui::test]
    fn the_page_menu_sizes_its_optional_rows(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(aui_tokens::ThemeKind::Dark, cx));
        cx.update(|cx| {
            let page = |safari, devtools| menu(BrowseMenuTarget::Page { safari, devtools, annotating: false });
            let full = browse_menu_size(&page(true, true), 0, cx);
            let bare = browse_menu_size(&page(false, false), 0, cx);
            assert_eq!(full.width, menu_width(), "no submenu room");
            assert_eq!(full.height - bare.height, cx.aui().metrics.control * 2.0, "Safari and DevTools are a row each");
        });
    }

    #[gpui::test]
    fn tab_menu_width_includes_the_submenu_others_do_not(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(aui_tokens::ThemeKind::Dark, cx));
        cx.update(|cx| {
            let plain = menu_width();
            assert!(browse_menu_size(&tab(false), 0, cx).width > plain);
            let others = [
                menu(BrowseMenuTarget::Folder("f".into())),
                menu(BrowseMenuTarget::TaskFolder { session: "s".into(), running: true }),
                menu(BrowseMenuTarget::Space { id: "p".into(), last: false }),
            ];
            for m in &others {
                assert_eq!(browse_menu_size(m, 0, cx).width, plain, "{m:?}");
            }
        });
    }

    #[gpui::test]
    fn menu_heights_grow_by_one_control_per_extra_row(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(aui_tokens::ThemeKind::Dark, cx));
        cx.update(|cx| {
            let control = cx.aui().metrics.control;
            let close = |a: Pixels, b: Pixels| assert!((f32::from(a) - f32::from(b)).abs() < 0.01, "{a:?} vs {b:?}");
            let task_idle = menu(BrowseMenuTarget::TaskFolder { session: "s".into(), running: false });
            let task_run = menu(BrowseMenuTarget::TaskFolder { session: "s".into(), running: true });
            close(browse_menu_size(&task_run, 0, cx).height - browse_menu_size(&task_idle, 0, cx).height, control);
            let space = menu(BrowseMenuTarget::Space { id: "p".into(), last: true });
            let folder = menu(BrowseMenuTarget::Folder("f".into()));
            close(browse_menu_size(&folder, 0, cx).height - browse_menu_size(&space, 0, cx).height, control);
            close(browse_menu_size(&task_idle, 0, cx).height, browse_menu_size(&space, 0, cx).height);
            // The folder count only matters through the tab menu's submenu.
            assert_eq!(browse_menu_size(&folder, 5, cx), browse_menu_size(&folder, 0, cx));
        });
    }

    #[gpui::test]
    fn tab_menu_height_grows_with_folders_and_in_folder(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(aui_tokens::ThemeKind::Dark, cx));
        cx.update(|cx| {
            let control = cx.aui().metrics.control;
            let h = |m: &BrowseMenu, n| browse_menu_size(m, n, cx).height;
            // Many folders: the submenu is the taller box and each adds one control.
            let (a, b) = (h(&tab(false), 20), h(&tab(false), 21));
            assert!((f32::from(b - a) - f32::from(control)).abs() < 0.01, "{a:?} {b:?}");
            assert!(h(&tab(false), 3) >= h(&tab(false), 0));
            // In a folder the main column gains "Remove from folder": one more row.
            let (plain, inside) = (h(&tab(false), 0), h(&tab(true), 0));
            assert!((f32::from(inside - plain) - f32::from(control)).abs() < 0.01, "{plain:?} {inside:?}");
        });
    }
}
