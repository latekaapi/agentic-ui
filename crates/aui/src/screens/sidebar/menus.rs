//! The two menus the sidebar opens: the project menu under the header and
//! the account menu above the footer.
//!
//! Both are quiet [`menu_surface`] panels with one [`Interactive`] row per
//! action — the same builders every other menu uses, so the rows read as
//! kin. The screen seats them in flow (under the header, above the footer);
//! the host owns what each row does through [`SidebarIntent`](super::SidebarIntent).

use std::rc::Rc;

use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{prelude::*, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::record_ax_label;
use crate::interact::{interactive, menu_surface};

use super::SidebarIntent;

type IntentHandler = Rc<dyn Fn(SidebarIntent, &mut Window, &mut App)>;

/// One menu row: a control-tall [`Interactive`] with its human label.
fn menu_row(
    id: &ElementId,
    label: &str,
    intent: SidebarIntent,
    handler: &Option<IntentHandler>,
    cx: &App,
) -> AnyElement {
    let metrics = cx.aui().metrics;
    let mut row = interactive((id.clone(), SharedString::from(label.to_string())), h_flex()
        .h(metrics.control)
        .w_full()
        .items_center()
        .px(metrics.item_px)
        .role_in(TextRole::Body, cx)
        .text_color(cx.aui().colors.fg)
        .child(label.to_string()))
    .label(label.to_string());
    if let Some(emit) = handler.clone() {
        row = row.on_click(move |_, w, cx| emit(intent.clone(), w, cx));
    }
    row.into_any_element()
}

/// The project menu for `workspace_name`: switch, new workspace, settings.
pub fn project_menu(
    id: impl Into<ElementId>,
    workspace_name: &SharedString,
    on_intent: Option<IntentHandler>,
    cx: &App,
) -> AnyElement {
    let id: ElementId = id.into();
    record_ax_label(&format!("Project menu for {workspace_name}"));
    let rows = vec![
        menu_row(&id, "Switch workspace", SidebarIntent::OpenProjectMenu, &on_intent, cx),
        menu_row(&id, "Workspace settings", SidebarIntent::ViewOptions, &on_intent, cx),
    ];
    menu_surface(
        v_flex()
            .id((id.clone(), "project-menu-list"))
            .w_full()
            .p(cx.aui().metrics.item_px)
            .role(gpui::Role::Menu)
            .aria_label(format!("Project menu for {workspace_name}"))
            .children(rows),
    )
    .into_any_element()
}

/// The account menu for `name`: the usage line plus settings and sign-out.
pub fn account_menu(
    id: impl Into<ElementId>,
    name: &SharedString,
    plan: &SharedString,
    usage: f32,
    on_intent: Option<IntentHandler>,
    cx: &App,
) -> AnyElement {
    let id: ElementId = id.into();
    let p = cx.aui().colors;
    let metrics = cx.aui().metrics;
    let used = format!("{}% of {} used", (usage.clamp(0.0, 1.0) * 100.0).round() as u32, plan);
    record_ax_label(&format!("Account menu for {name}"));
    let head = h_flex()
        .h(metrics.control)
        .w_full()
        .items_center()
        .px(metrics.item_px)
        .gap(metrics.gap)
        .child(
            gpui::div()
                .role_in(TextRole::Body, cx)
                .text_color(p.fg)
                .child(name.clone()),
        )
        .child(
            gpui::div()
                .role_in(TextRole::Caption, cx)
                .text_color(p.fg_muted)
                .child(used),
        );
    let rows = vec![
        menu_row(&id, "Account settings", SidebarIntent::Account, &on_intent, cx),
        menu_row(&id, "Manage plan", SidebarIntent::Account, &on_intent, cx),
    ];
    menu_surface(
        v_flex()
            .id((id.clone(), "account-menu-list"))
            .w_full()
            .p(metrics.item_px)
            .role(gpui::Role::Menu)
            .aria_label(format!("Account menu for {name}"))
            .child(head)
            .children(rows),
    )
    .into_any_element()
}
