//! Card 13 · Toasts and banners. The three-deep toast stack beside the four
//! banner states. Reproduces `design/src/cards/shell/13-toasts.html` at
//! 820×440.

use aui::feedback::{banner, toast_stack, BannerActionStyle, BannerKind, BannerRun, ToastAction, ToastData, ToastKind};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.wrap{grid-template-columns:340px 1fr;gap:28px}`.
const STACK_COLUMN: f32 = 340.0;
const COLUMN_GAP: f32 = 28.0;
/// `.stack{position:relative;height:150px;margin-top:22px}`.
const STACK_HEIGHT: f32 = 150.0;
const STACK_TOP: f32 = 22.0;
/// `.caps` inherits the body line height here; the Banners label adds
/// `margin-bottom:10px`.
const CAPS_LH: f32 = scale::LH_UI;
const BANNERS_CAPS_BOTTOM: f32 = 10.0;
/// `.ban{margin-bottom:8px}`, expressed as the gap between rows: CSS collapses
/// the last row's margin into the note's `margin-top:12px`, gpui does not.
const BANNER_GAP: f32 = scale::SP_3;
/// `.ds-note{margin-top:12px}`; both notes wrap inside their grid column.
const NOTE_TOP: f32 = 12.0;

/// The three toasts of the card, oldest first.
fn toasts() -> Vec<ToastData> {
    vec![
        ToastData::new("finished", "Agent finished", "cart-recovery-email"),
        ToastData::new("permission", "Codex needs permission", "infra/notifier"),
        ToastData::new("pr", "PR #2491 created", "auth-session-refresh → main · 3 commits")
            .kind(ToastKind::Ok)
            .action(ToastAction::new("open-pr", "Open PR"))
            .action(ToastAction::new("copy", "Copy link"))
            .progress(0.6),
    ]
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    h_flex()
        .w_full()
        .items_start()
        .gap(px(COLUMN_GAP))
        .child(
            v_flex()
                .flex_none()
                .w(px(STACK_COLUMN))
                .child(caps(p, "Toast stack · newest in front"))
                .child(
                    div()
                        .relative()
                        .mt(px(STACK_TOP))
                        .h(px(STACK_HEIGHT))
                        .child(toast_stack("card13-stack", toasts()).at_rest().on_action(|_, _, _| {}).on_close(|_, _| {})),
                )
                .child(note(
                    p,
                    "Older toasts tuck behind the newest, scaled and faded. Hovering the stack fans them out. The hairline at the bottom is the auto-dismiss timer; it pauses on hover.",
                )),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .child(caps(p, "Banners").mb(px(BANNERS_CAPS_BOTTOM)))
                .child(
                    v_flex()
                        .gap(px(BANNER_GAP))
                        .child(
                            banner(
                                "card13-ban-warn",
                                BannerKind::Waiting,
                                vec![
                                    BannerRun::Bold("Waiting for you.".into()),
                                    BannerRun::Text(" Allow ".into()),
                                    BannerRun::Mono("pnpm test".into()),
                                    BannerRun::Text(" to run?".into()),
                                ],
                            )
                            .action("Review", BannerActionStyle::Primary),
                        )
                        .child(
                            banner("card13-ban-info", BannerKind::Info, vec![BannerRun::Text("Context is 82% full. The next turn may compact history.".into())])
                                .action("Compact now", BannerActionStyle::Secondary),
                        )
                        .child(
                            banner(
                                "card13-ban-err",
                                BannerKind::Error,
                                vec![BannerRun::Bold("Provider error.".into()), BannerRun::Text(" Rate limited by Anthropic, retrying in 20 s.".into())],
                            )
                            .action("Retry", BannerActionStyle::Secondary),
                        )
                        .child(
                            banner("card13-ban-ok", BannerKind::Success, vec![BannerRun::Text("SSH reconnected to build-box · 12 agents resumed".into())])
                                .action("Dismiss", BannerActionStyle::Ghost),
                        ),
                )
                .child(note(
                    p,
                    "Banners are one line, icon left, one action right. Only waiting and error states carry a tint; everything else stays on the plain surface.",
                )),
        )
        .into_any_element()
}

/// `.caps`.
fn caps(p: Palette, label: &'static str) -> Div {
    div().text_role(TextRole::Caps).line_height(relative(CAPS_LH)).text_color(p.ink_3).child(label.to_uppercase())
}

/// `.ds-note`.
fn note(p: Palette, text: &'static str) -> Div {
    div().mt(px(NOTE_TOP)).ui(scale::FS_12).text_color(p.ink_3).child(text)
}
