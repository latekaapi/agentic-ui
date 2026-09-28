//! Card `nav/account-menu`. The sidebar-footer account menu: one sectioned
//! panel with a header, a usage section with a compact row per provider,
//! then the menu rows — plus a narrow variant proving long reasons wrap.

use aui::nav::{account_menu, AccountMenuItem};
use aui::screens::{UsageRowData, UsageRowState, UsageWindow};
use aui_icons::Provider;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// One labelled band: a caption over the component.
fn band(caption: &'static str, content: impl IntoElement) -> AnyElement {
    v_flex()
        .w_full()
        .gap(px(6.0))
        .child(
            div()
                .text_px(scale::FS_11)
                .line_height(gpui::relative(scale::LH_UI))
                .child(caption),
        )
        .child(content)
        .into_any_element()
}

/// The realistic panel: header, three usage rows, four item rows.
fn panel(id: impl Into<ElementId>, width: Option<f32>) -> impl IntoElement {
    let mut menu = account_menu(id)
        .header("latekaapi@gmail.com", None)
        .usage(
            "Usage",
            vec![
                UsageRowData::new(
                    Provider::Claude,
                    UsageRowState::Windows(
                        vec![
                            UsageWindow::new("5h", 0.42, "2h 10m"),
                            UsageWindow::new("Weekly", 0.81, "3d"),
                        ],
                        None,
                    ),
                )
                .plan("Max"),
                UsageRowData::new(
                    Provider::Codex,
                    UsageRowState::Windows(vec![UsageWindow::new("Weekly", 0.12, "5d")], None),
                )
                .plan("Pro"),
                UsageRowData::new(
                    Provider::Muse,
                    UsageRowState::Unavailable("No reading yet — appears after your first Muse turn".into()),
                ),
            ],
        )
        .rows(vec![
            AccountMenuItem::new("Settings…"),
            AccountMenuItem::new("Providers…"),
            AccountMenuItem::Separator,
            AccountMenuItem::new("Sign out of Muse").destructive(),
        ])
        .at_rest()
        .on_activate(|_, _, _| {});
    if let Some(width) = width {
        menu = menu.width(width);
    }
    menu
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    h_flex()
        .w_full()
        .items_start()
        .gap(px(16.0))
        .child(band("Account menu · at rest", panel("gallery-account-menu", None)))
        .child(band("Narrow · 220 px, long reasons wrap", panel("gallery-account-menu-narrow", Some(220.0))))
        .text_color(p.ink_2)
        .into_any_element()
}
