//! Host shortcuts and Chromium command filtering (B1b-L).
//!
//! The page is a Chrome-style window: Chromium owns accelerators such as
//! ⌘N/⌘T/⌘⇧N that would open real Chrome windows and tabs Baaz does not own,
//! and a key page window would otherwise swallow host shortcuts such as ⌘K.
//! This module holds the three pieces that fix that:
//!
//! - [`keystroke_of`]: converts a CEF key event into a
//!   `gpui::Keystroke::parse`-compatible string (`"cmd-k"`, `"cmd-shift-t"`).
//! - The host key router ([`set_key_router`]): the host registers a `claims`
//!   predicate plus a `deliver` sink. CEF's `on_pre_key_event` asks `claims`
//!   synchronously; a claimed keystroke is consumed (Chromium never sees it)
//!   and `deliver` runs on a later main-queue turn via the `defer` helper —
//!   never inside the CEF callback. With no router installed nothing is
//!   consumed (the old behaviour).
//! - [`chrome_command_allowed`]: the Chrome command filter behind
//!   `on_chrome_command`. Blocked ids are recorded for [`take_blocked_commands`]
//!   so the host can map them to its own actions later.
//!
//! Everything here is main-thread-only: the router and the blocked list live
//! in thread-locals, and both CEF handlers run on CEF's UI thread, which is
//! the main thread under the external pump.

use std::cell::RefCell;

use cef::{KeyEvent, KeyEventType};

/// `EVENTFLAG_*` bits from `cef_types.h` (the Rust bindings expose no consts).
const FLAG_SHIFT_DOWN: u32 = 1 << 1;
const FLAG_CONTROL_DOWN: u32 = 1 << 2;
const FLAG_ALT_DOWN: u32 = 1 << 3;
const FLAG_COMMAND_DOWN: u32 = 1 << 7;

/// Windows virtual-key codes for the bare modifier keys: never a keystroke.
const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12;
const VK_LWIN: i32 = 0x5B;
const VK_RWIN: i32 = 0x5C;

/// Converts a CEF key event into a `gpui::Keystroke::parse`-compatible string:
/// `ctrl-`, `alt-`, `shift-`, `cmd-` prefixes (in that order) then the key —
/// a lowercase letter/digit/punctuation from `unmodified_character`, or a
/// named key (`enter`, `escape`, `tab`, `backspace`, `delete`, `space`, `up`,
/// `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`, `f1`…`f12`)
/// from `windows_key_code`.
///
/// Returns `None` for `KEYUP`/`CHAR` events (only `RAWKEYDOWN`/`KEYDOWN` are
/// converted) and for bare modifier presses.
pub fn keystroke_of(event: &KeyEvent) -> Option<String> {
    keystroke_of_fields(
        event.type_ == KeyEventType::RAWKEYDOWN || event.type_ == KeyEventType::KEYDOWN,
        event.modifiers,
        event.windows_key_code,
        event.unmodified_character,
    )
}

/// The conversion over plain fields, so it is unit-testable without CEF.
pub fn keystroke_of_fields(
    is_key_down: bool,
    modifiers: u32,
    windows_key_code: i32,
    unmodified_character: u16,
) -> Option<String> {
    if !is_key_down {
        return None;
    }
    // A bare modifier press is not a keystroke.
    if matches!(
        windows_key_code,
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LWIN | VK_RWIN
    ) {
        return None;
    }
    let key = key_name(unmodified_character, windows_key_code)?;
    let mut out = String::new();
    if modifiers & FLAG_CONTROL_DOWN != 0 {
        out.push_str("ctrl-");
    }
    if modifiers & FLAG_ALT_DOWN != 0 {
        out.push_str("alt-");
    }
    if modifiers & FLAG_SHIFT_DOWN != 0 {
        out.push_str("shift-");
    }
    if modifiers & FLAG_COMMAND_DOWN != 0 {
        out.push_str("cmd-");
    }
    out.push_str(&key);
    Some(out)
}

/// The key part of a keystroke: printable characters from
/// `unmodified_character`, named keys from `windows_key_code`.
fn key_name(unmodified: u16, vk: i32) -> Option<String> {
    if let Some(c) = char::from_u32(unmodified as u32) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            return Some(c.to_string());
        }
        if c.is_ascii_uppercase() {
            return Some(c.to_ascii_lowercase().to_string());
        }
        if matches!(c, '`' | '[' | ']' | '\\' | ',' | '.' | '/' | ';' | '\'' | '-' | '=') {
            return Some(c.to_string());
        }
    }
    let named = match vk {
        0x0D => "enter",
        0x1B => "escape",
        0x09 => "tab",
        0x08 => "backspace",
        0x2E => "delete",
        0x20 => "space",
        0x26 => "up",
        0x28 => "down",
        0x25 => "left",
        0x27 => "right",
        0x24 => "home",
        0x23 => "end",
        0x21 => "pageup",
        0x22 => "pagedown",
        0x70..=0x7B => return Some(format!("f{}", vk - 0x6F)),
        // A letter/digit whose character field was empty (dead key, layout gap).
        0x41..=0x5A => return Some(((vk as u8).to_ascii_lowercase() as char).to_string()),
        0x30..=0x39 => return Some((vk as u8 as char).to_string()),
        _ => return None,
    };
    Some(named.to_owned())
}

// ------------------------------------------------------------------ the host key router

struct Router {
    claims: Box<dyn Fn(&str) -> bool>,
    deliver: Box<dyn Fn(String)>,
}

thread_local! {
    /// Main-thread-only: `None` until the host calls [`set_key_router`].
    static ROUTER: RefCell<Option<Router>> = const { RefCell::new(None) };
    /// Chrome command ids blocked on the main thread, oldest first.
    static BLOCKED: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
}

/// Installs the host key router (main thread only). `claims` is asked
/// synchronously from `on_pre_key_event`; when it returns true the event is
/// consumed and `deliver(keystroke)` runs on a later main-queue turn.
pub fn set_key_router(
    claims: impl Fn(&str) -> bool + 'static,
    deliver: impl Fn(String) + 'static,
) {
    ROUTER.with(|slot| {
        *slot.borrow_mut() = Some(Router {
            claims: Box::new(claims),
            deliver: Box::new(deliver),
        });
    });
}

/// Removes the router; nothing is consumed afterwards.
pub fn clear_key_router() {
    ROUTER.with(|slot| *slot.borrow_mut() = None);
}

/// The pure routing decision: a claimed keystroke is consumed and queued for
/// later delivery; anything else passes through to Chromium. Unit-testable
/// without a run loop (`defer` cannot run in a unit test).
pub fn decide_consume(keystroke: &str, claims: &dyn Fn(&str) -> bool) -> Option<String> {
    claims(keystroke).then(|| keystroke.to_owned())
}

/// Runs the routing decision for a converted keystroke: consumes claimed
/// keystrokes (queuing `deliver` on the main queue's next turn) and reports
/// whether the CEF event was consumed. Holds no lock; calls neither AppKit
/// nor CEF itself.
pub(crate) fn route_pre_key_event(keystroke: String) -> bool {
    let queued = ROUTER
        .with(|slot| slot.borrow().as_ref().and_then(|r| decide_consume(&keystroke, &r.claims)));
    if let Some(owned) = queued {
        crate::backend::defer(move || deliver_keystroke(owned));
        true
    } else {
        false
    }
}

/// Runs the router's `deliver` sink (a later main-queue turn, never the CEF
/// callback). Drops the keystroke when no router is installed.
fn deliver_keystroke(keystroke: String) {
    ROUTER.with(|slot| {
        if let Some(router) = slot.borrow().as_ref() {
            (router.deliver)(keystroke);
        }
    });
}

// ------------------------------------------------------------------ the Chrome command filter

/// Whether a Chrome command id may run. Allowed: navigation (33000–33999:
/// back/forward/reload/…), the edit commands the header defines (cut 36000,
/// copy 36001, paste 36003 — the 36000 block holds no select-all/undo/redo
/// ids), zoom (38000–38999) and DevTools (40004–40008 per the task's range:
/// DevTools, console, task manager, devices, feedback). Everything else —
/// new window/tab, close tab/window, restore tab, tab selection, find,
/// bookmarks, print, save page, focus location, history, downloads, options,
/// exit, fullscreen, … — is blocked.
pub fn chrome_command_allowed(id: i32) -> bool {
    matches!(
        id,
        33000..=33999 | 36000 | 36001 | 36003 | 38000..=38999 | 40004..=40008
    )
}

/// Records a blocked Chrome command id for [`take_blocked_commands`].
pub(crate) fn record_blocked_command(id: i32) {
    BLOCKED.with(|slot| slot.borrow_mut().push(id));
    eprintln!("aui-cef: blocked Chrome command {id}");
}

/// Drains the blocked Chrome command ids (main thread), oldest first, so the
/// host can map them to its own actions later.
pub fn take_blocked_commands() -> Vec<i32> {
    BLOCKED.with(|slot| std::mem::take(&mut *slot.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Command down: `EVENTFLAG_COMMAND_DOWN` (`cef_types.h`: `1 << 7`).
    const CMD: u32 = 128;
    const SHIFT: u32 = 2;
    const CTRL: u32 = 4;
    const ALT: u32 = 8;

    fn down(modifiers: u32, vk: i32, ch: char) -> Option<String> {
        keystroke_of_fields(true, modifiers, vk, ch as u16)
    }

    #[test]
    fn converts_host_shortcuts() {
        // ⌘K
        assert_eq!(down(CMD, 0x4B, 'k'), Some(String::from("cmd-k")));
        // ⌘⇧T (modifier order follows gpui's documented syntax)
        assert_eq!(down(CMD | SHIFT, 0x54, 't'), Some(String::from("shift-cmd-t")));
        // ⌃`
        assert_eq!(down(CTRL, 0xC0, '`'), Some(String::from("ctrl-`")));
        // ⌥⌘B
        assert_eq!(down(ALT | CMD, 0x42, 'b'), Some(String::from("alt-cmd-b")));
        // Plain `a`
        assert_eq!(down(0, 0x41, 'a'), Some(String::from("a")));
        // Uppercase input still lowercases.
        assert_eq!(down(SHIFT, 0x41, 'A'), Some(String::from("shift-a")));
    }

    #[test]
    fn converts_named_keys() {
        assert_eq!(down(0, 0x0D, '\r'), Some(String::from("enter")));
        assert_eq!(down(0, 0x74, '\0'), Some(String::from("f5")));
        assert_eq!(down(CMD, 0x25, '\0'), Some(String::from("cmd-left")));
        assert_eq!(down(0, 0x21, '\0'), Some(String::from("pageup")));
        assert_eq!(down(0, 0x20, ' '), Some(String::from("space")));
    }

    #[test]
    fn ignores_keyups_chars_and_bare_modifiers() {
        // KEYUP / CHAR are never converted.
        assert_eq!(keystroke_of_fields(false, CMD, 0x4B, 'k' as u16), None);
        // A bare Shift press.
        assert_eq!(keystroke_of_fields(true, SHIFT, VK_SHIFT, 0), None);
        assert_eq!(keystroke_of_fields(true, CTRL, VK_CONTROL, 0), None);
        assert_eq!(keystroke_of_fields(true, ALT, VK_MENU, 0), None);
    }

    #[test]
    fn converts_real_key_events() {
        let event = KeyEvent {
            size: 0,
            type_: KeyEventType::RAWKEYDOWN,
            modifiers: CMD,
            windows_key_code: 0x4B,
            native_key_code: 0,
            is_system_key: 0,
            character: 0,
            unmodified_character: b'k' as u16,
            focus_on_editable_field: 0,
        };
        assert_eq!(keystroke_of(&event), Some(String::from("cmd-k")));
        let up = KeyEvent {
            type_: KeyEventType::KEYUP,
            ..event.clone()
        };
        assert_eq!(keystroke_of(&up), None);
        let ch = KeyEvent {
            type_: KeyEventType::CHAR,
            ..event.clone()
        };
        assert_eq!(keystroke_of(&ch), None);
    }

    #[test]
    fn filters_chrome_commands() {
        // Allowed: reload, zoom in.
        assert!(chrome_command_allowed(33002));
        assert!(chrome_command_allowed(38001));
        assert!(chrome_command_allowed(33000));
        assert!(chrome_command_allowed(36001));
        assert!(chrome_command_allowed(38002));
        assert!(chrome_command_allowed(40004));
        // Blocked: new tab/window, incognito, find, location, print, exit.
        for blocked in [34014, 34000, 34001, 37000, 39001, 35003, 34031] {
            assert!(!chrome_command_allowed(blocked), "{blocked} should be blocked");
        }
    }

    #[test]
    fn routes_claimed_keystrokes_to_delivery() {
        let claim_all = |_: &str| true;
        let claim_none = |_: &str| false;
        // A claim-everything router consumes the key for exactly-once delivery.
        assert_eq!(decide_consume("cmd-k", &claim_all), Some(String::from("cmd-k")));
        // An unclaimed key passes through to Chromium.
        assert_eq!(decide_consume("cmd-c", &claim_none), None);
        // A selective router only takes what it claims.
        let claim_k = |ks: &str| ks == "cmd-k";
        assert_eq!(decide_consume("cmd-k", &claim_k), Some(String::from("cmd-k")));
        assert_eq!(decide_consume("cmd-c", &claim_k), None);
    }

    #[test]
    fn blocked_commands_drain_oldest_first() {
        let _ = take_blocked_commands();
        record_blocked_command(34014);
        record_blocked_command(34000);
        assert_eq!(take_blocked_commands(), vec![34014, 34000]);
        assert!(take_blocked_commands().is_empty());
    }
}
