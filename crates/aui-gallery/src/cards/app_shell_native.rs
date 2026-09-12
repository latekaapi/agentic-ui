//! Shell app-shell-native card. The same framed shell as
//! [`super::app_shell`], but with the sidebar collapsed, the header row held
//! steady ([`aui::shell::AppShell::header_follows_sidebar`] off) and the
//! sidebar header reserving macOS's own traffic lights
//! ([`aui::shell::SidebarHeader::native_lights`]): the leading 72 px of the
//! sidebar header stays empty — the gallery window paints its own lights —
//! while back/forward, search and the toggle stay where they are and only the
//! pane below collapses to the rail.

use aui::shell::{app_shell, centre_header, right_header, sidebar_header, tab_strip, TabItem};
use aui::shell::{SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH};
use aui_icons::{IconName, Provider};
use gpui::*;

use super::app_shell::{centre, right_pane, shell_rail, sidebar};

/// Same frame as the app-shell card: 1280×820 at body padding 12.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 1280.0 - 24.0;
const FRAME_H: f32 = 796.0;
/// Same right column as the app-shell card (card 10 uses 392).
const RIGHT_WIDTH: f32 = 392.0;

/// Builds the card content: collapsed sidebar, steady header, native-lights lead.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let strip = tab_strip(
        "card-native-tabs",
        vec![
            TabItem::new("diff", "Diff", IconName::Git).closable(false),
            TabItem::new("files", "Files", IconName::Folder).closable(false),
            TabItem::new("terminal", "Terminal", IconName::Terminal).closable(false),
            TabItem::new("browser", "Browser", IconName::Globe).closable(false),
        ],
        0,
    );
    div()
        .relative()
        .w(px(FRAME_W))
        .h(px(FRAME_H))
        .flex_none()
        .m(px(FRAME_INSET))
        .child(
            app_shell("card-native-shell")
                .framed(true)
                .traffic_lights(true)
                .header_follows_sidebar(false)
                .sidebar_width(px(SIDEBAR_WIDTH))
                .sidebar_min_width(px(SIDEBAR_MIN_WIDTH))
                .sidebar_max_width(px(SIDEBAR_MAX_WIDTH))
                .right_width(px(RIGHT_WIDTH))
                .right_open(true)
                .sidebar_open(false)
                .header_sidebar(sidebar_header("card-native-hd-side").native_lights(true))
                .header_centre(
                    centre_header("card-native-hd-centre", "checkout-flow-v2")
                        .provider(Provider::Claude)
                        .branch("feature/checkout-flow-v2"),
                )
                .header_right(right_header("card-native-hd-right").tabs(strip))
                .sidebar(sidebar(cx))
                .rail(shell_rail())
                .centre(centre(window, cx))
                .right(right_pane(cx)),
        )
        .into_any_element()
}
