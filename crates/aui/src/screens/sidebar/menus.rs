//! The two menus the sidebar opens: the view-options menu under the
//! Projects header and the account menu above the footer.
//!
//! Both are [`menu_surface`] panels (step +2, item radius, shadow) with one
//! [`Interactive`](crate::interact::Interactive) row per action, 28 px tall
//! on the `control_md` step, exactly as the mock draws them. The host owns
//! what each row does through [`SidebarIntent`].

use std::rc::Rc;

use aui_tokens::{scale, ActiveAui, AuiStyled, Density, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::record_ax_label;
use crate::interact::{interactive, menu_surface, Shape};
use crate::nav::project_mark;

use super::{Account, AccountAction, SidebarIntent, ViewOption, ViewOptions};

type IntentHandler = Rc<dyn Fn(SidebarIntent, &mut Window, &mut App)>;

/// A meter at or past this fraction turns `warning`, as Baaz's account panel does.
const USAGE_WARN: f32 = 0.8;

/// One menu row: a `control_md`-tall menu item with its label on the
/// left and an optional value or shortcut on the right.
fn menu_row(
    id: &ElementId,
    label: &str,
    trailing: Option<&str>,
    intent: Option<SidebarIntent>,
    handler: &Option<IntentHandler>,
    danger: bool,
    cx: &App,
) -> AnyElement {
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let mut body = h_flex()
        .h(metrics.control_md)
        .w_full()
        .items_center()
        .px(metrics.item_px)
        .gap(metrics.gap)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_role_in(TextRole::Body, Density::Compact)
                .text_color(if danger { p.danger } else { p.fg })
                .child(label.to_string()),
        );
    if let Some(trailing) = trailing {
        // Values (Project ›, Last activity ›, ✓) are Caption; shortcuts are Micro.
        let role = if trailing.starts_with('⌘') { TextRole::Micro } else { TextRole::Caption };
        body = body.child(div().role_in(role, cx).text_color(p.fg_muted).child(trailing.to_string()));
    }
    let mut row = interactive((id.clone(), SharedString::from(label.to_string())), body)
        .role("menuitem")
        .shape(Shape::Item)
        .label(label.to_string());
    let emit = handler.clone();
    // A menu click never reaches the list beneath the menu.
    row = row.on_click(move |_, w, cx| {
        cx.stop_propagation();
        if let (Some(emit), Some(intent)) = (emit.as_ref(), intent.as_ref()) {
            emit(intent.clone(), w, cx);
        }
    });
    row.into_any_element()
}

/// A hairline between menu groups.
fn separator(cx: &App) -> AnyElement {
    let metrics = cx.aui().metrics;
    div().h(px(1.0)).w_full().my(metrics.gap / 2.0).bg(cx.aui().colors.line).into_any_element()
}

/// The view-options menu under the Projects header: grouping, sort and
/// the three list toggles, as Baaz's Sessions menu has them. Every row
/// reports its own [`ViewOption`]; the values and checks come from
/// `options`.
pub fn view_options_menu(id: impl Into<ElementId>, options: &ViewOptions, on_intent: Option<IntentHandler>, cx: &App) -> AnyElement {
    let id: ElementId = id.into();
    let metrics = cx.aui().metrics;
    record_ax_label("View options");
    let group_by = format!("{} ›", options.group_by);
    let sort_by = format!("{} ›", options.sort_by);
    let check = |on: bool| if on { Some("✓") } else { None };
    let backdrop = on_intent.clone();
    menu_surface(
        v_flex()
            .id((id.clone(), "view-options-list"))
            .w_full()
            .p(metrics.item_px / 2.0)
            .role(gpui::Role::Menu)
            .aria_label("View options")
            .occlude()
            .on_click(move |_, w, cx| {
                cx.stop_propagation();
                if let Some(emit) = backdrop.as_ref() {
                    emit(SidebarIntent::MenuBackdrop, w, cx);
                }
            })
            .child(menu_row(&id, "Group by", Some(&group_by), Some(SidebarIntent::ViewOption(ViewOption::GroupBy)), &on_intent, false, cx))
            .child(menu_row(&id, "Sort by", Some(&sort_by), Some(SidebarIntent::ViewOption(ViewOption::SortBy)), &on_intent, false, cx))
            .child(separator(cx))
            .child(menu_row(&id, "Show empty projects", check(options.show_empty), Some(SidebarIntent::ViewOption(ViewOption::ToggleEmpty)), &on_intent, false, cx))
            .child(menu_row(&id, "Show PR status", check(options.show_pr_status), Some(SidebarIntent::ViewOption(ViewOption::TogglePrStatus)), &on_intent, false, cx))
            .child(menu_row(&id, "Show archived", check(options.show_archived), Some(SidebarIntent::ViewOption(ViewOption::ToggleArchived)), &on_intent, false, cx)),
    )
    .into_any_element()
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
            .child(project_mark(usage.initial.clone(), p.label(usage.tint)).size(metrics.icon - px(scale::SP_1)))
            .child(div().text_role_in(TextRole::Body, Density::Compact).text_color(p.fg).child(usage.name.clone()))
            .child(div().role_in(TextRole::Micro, cx).text_color(p.fg_muted).child(usage.plan.clone())),
    );
    for (index, line) in usage.lines.iter().enumerate() {
        let fraction = line.fraction.clamp(0.0, 1.0);
        let hot = fraction >= USAGE_WARN;
        let percent = format!("{}%", (fraction * 100.0).round() as u32);
        card = card
            .child(
                h_flex()
                    .h(metrics.control_xs)
                    .items_center()
                    .role_in(TextRole::Caption, cx)
                    .text_color(p.fg_muted)
                    .child(line.label.clone())
                    .child(div().flex_1())
                    .child(div().text_color(if hot { p.warning } else { p.ink_2 }).medium().child(percent))
                    .child(div().ml(metrics.gap / 2.0).child(line.resets.clone())),
            )
            .child(
                Shape::Pill.rounded(
                    div()
                        .id((id.clone(), SharedString::from(format!("meter-{}-{index}", usage.name))))
                        .w_full()
                        .h(px(2.0))
                        .my(px(2.0))
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
pub fn account_menu(id: impl Into<ElementId>, account: &Account, on_intent: Option<IntentHandler>, cx: &App) -> AnyElement {
    let id: ElementId = id.into();
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let label: SharedString = format!("Account menu for {}", account.name).into();
    record_ax_label(&label);
    let identity = h_flex()
        .h(metrics.control_lg)
        .w_full()
        .items_center()
        .px(metrics.item_px)
        .gap(metrics.gap)
        .child(project_mark(account.initial.clone(), p.label(account.tint)).size(metrics.icon + metrics.gap / 2.0))
        .child(div().text_role_in(TextRole::Body, Density::Compact).text_color(p.fg).child(account.name.clone()))
        .child(div().flex_1())
        .child(div().role_in(TextRole::Micro, cx).text_color(p.fg_muted).truncate().child(account.email.clone()));
    let backdrop = on_intent.clone();
    let mut panel = v_flex()
        .id((id.clone(), "account-menu-list"))
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
        .child(identity)
        .child(separator(cx))
        .child(
            div()
                .h(metrics.control_md)
                .px(metrics.item_px)
                .flex()
                .items_center()
                .role_in(TextRole::Caption, cx)
                .text_color(p.fg_muted)
                .child("Usage"),
        );
    for usage in &account.usage {
        panel = panel.child(provider_card(&id, usage, cx));
    }
    panel = panel
        .child(separator(cx))
        .child(menu_row(&id, "Settings…", Some("⌘,"), Some(SidebarIntent::AccountAction(AccountAction::Settings)), &on_intent, false, cx))
        .child(menu_row(&id, "Providers…", None, Some(SidebarIntent::AccountAction(AccountAction::Providers)), &on_intent, false, cx))
        .child(separator(cx))
        .child(menu_row(&id, "Sign out of Muse", None, Some(SidebarIntent::AccountAction(AccountAction::SignOut)), &on_intent, true, cx));
    menu_surface(panel).into_any_element()
}
