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
/// What one menu row shows and reports.
struct MenuRow<'a> {
    label: &'a str,
    trailing: Option<&'a str>,
    intent: Option<SidebarIntent>,
    danger: bool,
}

fn menu_row(id: &ElementId, spec: MenuRow<'_>, handler: &Option<IntentHandler>, window: &mut Window, cx: &mut App) -> AnyElement {
    let MenuRow { label, trailing, intent, danger } = spec;
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let row_id: ElementId = (id.clone(), SharedString::from(label.to_string())).into();
    // FF menu rows: one `control` tall (36), label muted at rest and
    // foreground while the row is lit.
    let lit = crate::interact::interactive_state(row_id.clone(), window, cx).hovered;
    let mut body = h_flex()
        .h(metrics.control)
        .w_full()
        .items_center()
        .px(metrics.item_px)
        .gap(metrics.gap)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_role_in(TextRole::Body, Density::Standard)
                .text_color(if danger { p.danger } else if lit { p.fg } else { p.fg_muted })
                .child(label.to_string()),
        );
    if let Some(trailing) = trailing {
        // Values (Project ›, Last activity ›, ✓) are Caption; shortcuts are Micro.
        let role = if trailing.starts_with('⌘') { TextRole::Micro } else { TextRole::Caption };
        body = body.child(div().role_in(role, cx).text_color(p.fg_muted).child(trailing.to_string()));
    }
    let mut row = interactive(row_id, body)
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
pub fn view_options_menu(id: impl Into<ElementId>, options: &ViewOptions, on_intent: Option<IntentHandler>, window: &mut Window, cx: &mut App) -> AnyElement {
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
            .child(menu_row(&id, MenuRow { label: "Group by", trailing: Some(&group_by), intent: Some(SidebarIntent::ViewOption(ViewOption::GroupBy)), danger: false }, &on_intent, window, cx))
            .child(menu_row(&id, MenuRow { label: "Sort by", trailing: Some(&sort_by), intent: Some(SidebarIntent::ViewOption(ViewOption::SortBy)), danger: false }, &on_intent, window, cx))
            .child(separator(cx))
            .child(menu_row(&id, MenuRow { label: "Show empty projects", trailing: check(options.show_empty), intent: Some(SidebarIntent::ViewOption(ViewOption::ToggleEmpty)), danger: false }, &on_intent, window, cx))
            .child(menu_row(&id, MenuRow { label: "Show PR status", trailing: check(options.show_pr_status), intent: Some(SidebarIntent::ViewOption(ViewOption::TogglePrStatus)), danger: false }, &on_intent, window, cx))
            .child(menu_row(&id, MenuRow { label: "Show archived", trailing: check(options.show_archived), intent: Some(SidebarIntent::ViewOption(ViewOption::ToggleArchived)), danger: false }, &on_intent, window, cx)),
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
pub fn account_menu(id: impl Into<ElementId>, account: &Account, on_intent: Option<IntentHandler>, window: &mut Window, cx: &mut App) -> AnyElement {
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
        .child(div().flex_1().min_w(px(0.0)).truncate().text_role_in(TextRole::Body, Density::Standard).text_color(p.fg).child(account.name.clone()))
        // The email only when it says something the name does not.
        .when(!account.email.is_empty() && account.email != account.name, |d| {
            d.child(div().flex_shrink(1.0).min_w(px(0.0)).max_w(px(scale::SP_7 * 3.0)).truncate().role_in(TextRole::Micro, cx).text_color(p.fg_muted).child(account.email.clone()))
        });
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
        .child(menu_row(&id, MenuRow { label: "Settings…", trailing: Some("⌘,"), intent: Some(SidebarIntent::AccountAction(AccountAction::Settings)), danger: false }, &on_intent, window, cx))
        .child(menu_row(&id, MenuRow { label: "Providers…", trailing: None, intent: Some(SidebarIntent::AccountAction(AccountAction::Providers)), danger: false }, &on_intent, window, cx))
        .child(separator(cx))
        .child(menu_row(&id, MenuRow { label: "Sign out of Muse", trailing: None, intent: Some(SidebarIntent::AccountAction(AccountAction::SignOut)), danger: true }, &on_intent, window, cx));
    menu_surface(panel).into_any_element()
}
