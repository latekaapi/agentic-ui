//! Overlay · Modal dialog. The error-kind dialog over its scrim: icon tile,
//! title, body, mono detail and the one action row. The card shows what the
//! component paints; the focus trap and `esc` belong to the caller, so this
//! static composition has neither.

use aui::overlay::{dialog, DialogKind};
use gpui::*;

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
