//! The 28 px address line: `✦` for internal pages, `>_` when local, the
//! lock when secure, the warning lock otherwise; host in `ink` at weight
//! 500, path in `ink-3`, mono port tag for non-default ports. The whole row
//! is one quiet button; the host opens the Field on click.

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
        // Internal pages never show the insecure state; a non-local,
        // non-internal `http:` page does (`not secure` in the name).
        let insecure = !self.address.is_internal() && !self.address.local && !self.address.secure;
        let label: SharedString = if insecure {
            format!("Address, not secure, {}{}", self.address.host, self.address.path).into()
        } else {
            format!("Address, {}{}", self.address.host, self.address.path).into()
        };
        record_ax_label(&label);
        let glyph = if self.address.is_internal() {
            IconName::Sparkle
        } else if self.address.local {
            IconName::Terminal
        } else {
            IconName::Shield
        };
        // No triangle / alert glyph exists in the sprite (adding one is out
        // of scope), so the insecure state reuses the shield in `warning`.
        let glyph_color = if insecure { p.warning } else { p.ink_3 };
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
            .child(icon(glyph).size(px(GLYPH)).color(glyph_color))
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
        // `split_address` already hides default ports, so any port left
        // shows — local or not.
        if let Some(port) = self.address.port {
            row = row.child(tag(SharedString::from(format!(":{port}"))));
        }
        if let Some(on_click) = self.on_click {
            row = row.on_click(move |e, w, cx| on_click(e, w, cx));
        }
        row
    }
}

/// Splits `url` into host, path, locality, port, security and scheme.
///
/// The authority ends at the first `/`, `?` or `#`, so
/// `https://example.com?x=1` keeps host `example.com`; the path keeps the
/// query and fragment (gaining a leading `/` when it starts with `?` or
/// `#`) and defaults to `/`. The scheme matches case-insensitively, and
/// userinfo is stripped (`https://user:pw@host/` → host `host`).
///
/// Internal schemes (`about:`, `chrome:`, `file:`, `data:`) are always
/// secure, never local, and carry no port: `about:blank` → host
/// `about:blank`, path `/`; `file:///Users/x` → host `file`, path
/// `/Users/x`; `data:` URLs show host `data` with the mediatype as the
/// path. `chrome://settings/` keeps host `settings` with `scheme`
/// `Some("chrome")` (the scheme tells callers apart what the bare host
/// cannot) — this one representation, not `chrome://settings` as the host.
pub fn split_address(url: &str) -> Address {
    if let Some(address) = split_internal(url) {
        return address;
    }
    let (scheme, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (Some(scheme.to_ascii_lowercase()), rest),
        None => (None, url),
    };
    let secure = match scheme.as_deref() {
        Some("https") => true,
        Some("http") => false,
        Some(_) => false,
        None => true,
    };
    let (authority, raw_path) = match rest.find(['/', '?', '#']) {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, "/"),
    };
    // Keep the query / fragment, but keep host + path reading as one URL.
    let path = if raw_path.starts_with(['?', '#']) { format!("/{raw_path}") } else { raw_path.to_owned() };
    // Strip userinfo; credentials never reach the host.
    let authority = match authority.rsplit_once('@') {
        Some((_, host)) => host,
        None => authority,
    };
    let (host, port) = split_host_port(authority);
    // Default ports stay hidden; any other port shows, local or not.
    let port = match (scheme.as_deref(), port) {
        (Some("http"), Some(80)) | (Some("https") | None, Some(443)) => None,
        _ => port,
    };
    let local = is_local(host);
    Address::new(host, path, local, port, secure, scheme)
}

/// Splits the internal schemes (`about:`, `chrome:`, `file:`, `data:`);
/// `None` for anything else. See [`split_address`] for the representations.
fn split_internal(url: &str) -> Option<Address> {
    let (scheme, rest) = url.split_once(':')?;
    let scheme = scheme.to_ascii_lowercase();
    let internal = |host: &str, path: &str| {
        Address::new(host, path, false, None, true, Some(scheme.clone()))
    };
    match scheme.as_str() {
        "about" => {
            let (authority, path) = split_path_rest(rest);
            Some(internal(&format!("about:{authority}"), &path))
        }
        "chrome" => {
            let rest = rest.strip_prefix("//").unwrap_or(rest);
            let (authority, path) = split_path_rest(rest);
            Some(internal(authority, &path))
        }
        "file" => {
            let rest = rest.strip_prefix("//").unwrap_or(rest);
            let (authority, path) = split_path_rest(rest);
            let host = if authority.is_empty() { "file" } else { authority };
            Some(internal(host, &path))
        }
        "data" => Some(internal("data", &format!("/{rest}"))),
        // Any other `scheme:` form with no `//` (`javascript:`, `mailto:`,
        // `blob:`, `view-source:`, `tel:`) is a scheme, not a host, and is
        // never secure. `host:port` is told apart by its all-digit rest.
        _ if is_scheme_like(&scheme) && !rest.starts_with("//") && !rest.bytes().all(|b| b.is_ascii_digit()) => {
            Some(Address::new(format!("{scheme}:"), "/", false, None, false, Some(scheme.clone())))
        }
        _ => None,
    }
}

/// A scheme is ASCII letters, digits, `+`, `-` or `.`, starting with a
/// letter, and never contains a dot run that reads as a host (`a.b`).
fn is_scheme_like(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-')
}

/// Splits `rest` (no scheme) at the first `/`, `?` or `#`; the second half
/// keeps the query / fragment and gains a leading `/` when it starts with
/// `?` or `#`, defaulting to `/`.
fn split_path_rest(rest: &str) -> (&str, String) {
    match rest.find(['/', '?', '#']) {
        Some(at) => {
            let path = &rest[at..];
            let path = if path.starts_with(['?', '#']) { format!("/{path}") } else { path.to_owned() };
            (&rest[..at], path)
        }
        None => (rest, "/".to_owned()),
    }
}

/// Splits an authority (userinfo already stripped) into host and port,
/// bracket-aware so `[::1]:8080` keeps host `[::1]`. A non-numeric port is
/// not a port: the authority stays intact.
fn split_host_port(authority: &str) -> (&str, Option<u16>) {
    if let Some(inner) = authority.strip_prefix('[') {
        return match inner.find(']') {
            // `end` is relative to `inner` (past the `[`), so the `]` sits
            // at `end + 1` in `authority` and the host ends past it.
            Some(end) => {
                let host = &authority[..end + 2];
                let port = authority[end + 2..].strip_prefix(':').and_then(|tail| tail.parse::<u16>().ok());
                (host, port)
            }
            None => (authority, None),
        };
    }
    match authority.rsplit_once(':') {
        Some((head, tail)) if !head.is_empty() => match tail.parse::<u16>() {
            Ok(port) => (head, Some(port)),
            // An empty or out-of-range numeric port is still a port
            // separator: drop it so `localhost:99999` keeps host
            // `localhost`. A non-numeric tail is not a port at all.
            Err(_) if tail.bytes().all(|b| b.is_ascii_digit()) => (head, None),
            Err(_) => (authority, None),
        },
        _ => (authority, None),
    }
}

/// True for the loopbacks: `localhost` (a trailing dot counts),
/// `*.localhost`, `*.test`, `127.0.0.0/8`, `0.0.0.0` and `[::1]`.
fn is_local(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    let bare = lower.strip_suffix('.').unwrap_or(&lower);
    if bare == "localhost" || bare.ends_with(".localhost") || bare.ends_with(".test") {
        return true;
    }
    if bare == "[::1]" || bare == "0.0.0.0" {
        return true;
    }
    // Any `127.x.y.z`: every part numeric, leading `127`.
    if !bare.split('.').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    bare == "127" || bare.starts_with("127.")
}
