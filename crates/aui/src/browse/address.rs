//! The 28 px address line: lock glyph (or the `>_` glyph when local),
//! host in `ink` at weight 500, path in `ink-3`, mono port tag when local.
//! The whole row is one quiet button; the host opens the Field on click.

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;

use crate::browse::Address;
use crate::data::{record_ax_label, tag};
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// The row height: 28 px, quiet (tint on hover only, no border).
const ROW_H: f32 = 28.0;
/// The row's horizontal padding and inner gap.
const ROW_PAD_X: f32 = 8.0;
const ROW_GAP: f32 = 6.0;
/// The lock / terminal glyph: 12 px in `ink-3`.
const GLYPH: f32 = 12.0;

/// The 28 px address line for `address`. Build with [`address_line`].
#[derive(IntoElement)]
pub struct AddressLine {
    id: ElementId,
    address: Address,
    on_click: Option<ClickHandler>,
}

/// The 28 px address line for `address`: lock (or `>_` when local), host in
/// `ink` 500, path in `ink-3`, mono port tag when local.
pub fn address_line(id: impl Into<ElementId>, address: Address) -> AddressLine {
    AddressLine { id: id.into(), address, on_click: None }
}

impl AddressLine {
    /// Click handler: the host opens the Field.
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for AddressLine {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let bg = tint_fade((id.clone(), "bg"), flags.hovered, p.surface_2, Tween::FAST, window, cx);
        let label: SharedString = format!("Address, {}{}", self.address.host, self.address.path).into();
        record_ax_label(&label);
        let glyph = if self.address.local { IconName::Terminal } else { IconName::Shield };
        // No `w_full`: as a column item the row stretches between its
        // 8 px margins on its own; full width plus margins would overflow
        // the column (see `CompactSessionRow`).
        let mut row = h_flex()
            .id(id)
            .h(px(ROW_H))
            .flex_none()
            .items_center()
            .gap(px(ROW_GAP))
            .px(px(ROW_PAD_X))
            .mx(px(ROW_PAD_X))
            .rounded(px(scale::R_SM))
            .bg(bg)
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(label)
            .track_interaction(&state)
            .child(icon(glyph).size(px(GLYPH)).color(p.ink_3))
            .child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .ui(scale::FS_12)
                    .child(
                        div()
                            .flex_none()
                            .min_w(px(0.0))
                            .max_w(gpui::relative(0.6))
                            .truncate()
                            .medium()
                            .text_color(p.ink)
                            .child(self.address.host.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .truncate()
                            .text_color(p.ink_3)
                            .child(self.address.path.clone()),
                    ),
            );
        if let (true, Some(port)) = (self.address.local, self.address.port) {
            row = row.child(tag(SharedString::from(format!(":{port}"))));
        }
        if let Some(on_click) = self.on_click {
            row = row.on_click(move |e, w, cx| on_click(e, w, cx));
        }
        row
    }
}

/// Splits `url` into host, path, locality and port: `local` is `localhost`,
/// `127.0.0.1`, `*.localhost` or `*.test`; the path keeps any query or hash
/// and defaults to `/`; `secure` follows the scheme (`https:`, or a bare
/// host assumed `https:`).
pub fn split_address(url: &str) -> Address {
    let (scheme, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (Some(scheme), rest),
        None => (None, url),
    };
    let secure = match scheme {
        Some("https") => true,
        Some("http") => false,
        Some(_) => false,
        None => true,
    };
    let (authority, path) = match rest.find('/') {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((head, tail)) if !head.is_empty() => match tail.parse::<u16>() {
            Ok(port) => (head, Some(port)),
            Err(_) => (authority, None),
        },
        _ => (authority, None),
    };
    let lower = host.to_ascii_lowercase();
    let local =
        lower == "localhost" || lower == "127.0.0.1" || lower.ends_with(".localhost") || lower.ends_with(".test");
    Address::new(host, path, local, port, secure)
}
