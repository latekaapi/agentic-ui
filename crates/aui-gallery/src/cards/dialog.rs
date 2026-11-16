//! Overlay · Modal dialog. The error-kind dialog over its scrim: icon tile,
//! title, body, mono detail and the one action row. The card shows what the
//! component paints; the focus trap and `esc` belong to the caller, so this
//! static composition has neither.

use aui::data::{button, field, icon_button};
use aui::overlay::{dialog, modal_footer, modal_header, modal_panel, DialogKind, MODAL_PAD};
use aui::screens::{dialog_sheet, sheet_scrim, SHEET_PAD_X};
use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

/// The dialog's title, body and the mono lease line under it.
const TITLE: &str = "Session already in use";
const BODY: &str = "Another window holds the writer lease on this session, so this one opened read-only. Take the lease and the other window drops to read-only instead.";
const DETAIL: &str = "lease: muse-2f19 · held since 09:41";

/// Builds the card content.
pub fn build(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div()
        .relative()
        .size_full()
        .child(
            dialog("card-dialog", TITLE)
                .kind(DialogKind::Error)
                .body(BODY)
                .detail(DETAIL)
                .secondary("Dismiss")
                .primary("Open another session")
                // A static capture: no enter motion to blur it.
                .at_rest()
                .on_primary(|_, _| {})
                .on_secondary(|_, _| {})
                .on_dismiss(|_, _| {}),
        )
        .into_any_element()
}

/// `overlay/dialog-leading`: a critical error whose host element (an app's
/// mascot) takes the kind tile's place. It is too tall for the header, so
/// it leads the body; the header keeps the title alone.
pub fn build_leading(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    // A stand-in mascot: a 56 px tile with a large glyph.
    let mascot = div()
        .size(px(56.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(scale::SHAPE_CONTAINER))
        .bg(p.surface_4)
        .child(icon(IconName::Shield).size(px(scale::SP_7)).color(p.danger));
    div()
        .relative()
        .size_full()
        .child(
            dialog("card-dialog-leading", "The agent stopped")
                .kind(DialogKind::Error)
                .leading(mascot)
                .body("The agent process exited before the turn finished. Your conversation is saved; restart to pick up where it left off.")
                .detail("exit status 137 · after 4 m 12 s")
                .secondary("Dismiss")
                .primary("Restart")
                .at_rest()
                .on_primary(|_, _| {})
                .on_secondary(|_, _| {})
                .on_dismiss(|_, _| {}),
        )
        .into_any_element()
}

/// `overlay/dialog-sheet`: the sheet frame with no provider mark, for an
/// app's own sheets (an import): header (title, ✕), the host's body, the
/// action row, on the one scrim.
pub fn build_dialog_sheet(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let text = |t: &'static str, cx: &App| div().role_in(TextRole::Body, cx).text_color(cx.aui().colors.fg_muted).child(t);
    let body = v_flex()
        .w_full()
        .gap(px(scale::SP_3))
        .px(px(SHEET_PAD_X))
        .pt(px(scale::SP_5))
        .child(text("Bring your bookmarks, history and open tabs from Chrome. Nothing in Chrome changes.", cx))
        .child(
            div()
                .w_full()
                .py(px(scale::SP_3))
                .px(px(scale::SP_4))
                .rounded(px(scale::SHAPE_ITEM))
                .bg(p.surface_3)
                .mono(TextRole::Micro)
                .text_color(p.fg_muted)
                .child("Profile 1 · 1,204 bookmarks · 9,871 history entries"),
        );
    let sheet = dialog_sheet("card-dialog-sheet", "Import from Chrome")
        .child(body)
        .actions(vec![
            button("card-dialog-sheet-cancel", "Cancel").on_click(|_, _, _| {}).into_any_element(),
            button("card-dialog-sheet-import", "Import").primary().on_click(|_, _, _| {}).into_any_element(),
        ])
        .on_close(|_, _| {});
    div().relative().size_full().child(sheet_scrim("card-dialog-sheet-scrim", sheet, None)).into_any_element()
}

/// `overlay/modal-panel`: a host-built modal on the dialog card's ground —
/// [`modal_panel`] at a host width, with [`modal_header`], the host's form
/// and [`modal_footer`] (a hint left, Cancel and the primary right).
pub fn build_modal_panel(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let label = |t: &'static str, cx: &App| div().role_in(TextRole::Caption, cx).medium().text_color(cx.aui().colors.fg_muted).child(t);
    let close = icon_button("card-modal-panel-close", IconName::X).ghost().accessibility_label("Close New skill").on_click(|_, _, _| {});
    let form = v_flex()
        .w_full()
        .gap(px(scale::SP_3))
        .p(px(MODAL_PAD))
        .child(label("Name", cx))
        .child(field("card-modal-panel-name", true, cx).w_full().role_in(TextRole::Body, cx).child("review-pr"))
        .child(label("Description", cx))
        .child(field("card-modal-panel-desc", false, cx).w_full().role_in(TextRole::Body, cx).text_color(p.fg_muted).child("What the skill does, in one line"));
    let hint = div().role_in(TextRole::Caption, cx).text_color(p.ink_4).child("Saved to ~/.skills").into_any_element();
    let panel = modal_panel(480.0, cx)
        .id("card-modal-panel")
        .role(gpui::Role::Dialog)
        .aria_label("New skill")
        .child(modal_header("New skill", None, Some(close.into_any_element()), cx))
        .child(form)
        .child(modal_footer(
            Some(hint),
            vec![
                button("card-modal-panel-cancel", "Cancel").on_click(|_, _, _| {}).into_any_element(),
                button("card-modal-panel-create", "Create").primary().on_click(|_, _, _| {}).into_any_element(),
            ],
        ));
    div().relative().size_full().child(sheet_scrim("card-modal-panel-scrim", panel, None)).into_any_element()
}
