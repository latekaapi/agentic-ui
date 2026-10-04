//! Host shortcuts and Chromium command filtering (B1b-L2).
//!
//! The page is a Chrome-style window: Chromium owns accelerators such as
//! ⌘N/⌘T/⌘⇧N that would open real Chrome windows and tabs Baaz does not own,
//! and a key page window would otherwise swallow host shortcuts such as ⌘K.
//! This module holds the three pieces that fix that:
//!
//! - [`keystroke_of`]: converts a CEF key event into a
//!   `gpui::Keystroke::parse`-compatible string (`"cmd-k"`, `"shift-cmd-t"`).
//!   Modifiers are emitted in gpui's documented key-syntax order (`ctrl-`,
//!   `alt-`, `shift-`, `cmd-`, matching `Keystroke::unparse`'s canonical
//!   `ctrl-`/`alt-`/`cmd-`/`shift-` sequence up to the `fn-` prefix CEF never
//!   reports). Hosts must parse with `gpui::Keystroke::parse`, never
//!   string-compare: `parse` accepts any modifier order, so the string form
//!   is a transport, not a contract.
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
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

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
/// `ctrl-`, `alt-`, `shift-`, `cmd-` prefixes (in gpui's documented key-syntax
/// order) then the key — a lowercase letter/digit/punctuation from
/// `unmodified_character` (Shift-modified punctuation is mapped back to its
/// unshifted key, keeping `shift-`), or a named key (`enter`, `escape`,
/// `tab`, `backspace`, `delete`, `space`, `up`, `down`, `left`, `right`,
/// `home`, `end`, `pageup`, `pagedown`, `f1`…`f12`, plus the `;=/,-./` `` ` ``
/// `[` `\` `]` `'` OEM punctuation) from `windows_key_code`.
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

/// Maps Shift-modified punctuation back to its unshifted key, keeping the
/// `shift-` prefix: on macOS `unmodified_character` still applies Shift
/// (⌘⇧[ arrives as `{`, ⌘⇧1 as `!`), which would otherwise fall outside the
/// key whitelist and could never be claimed by the host router.
fn unshifted(c: char) -> char {
    match c {
        '{' => '[',
        '}' => ']',
        '~' => '`',
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        '_' => '-',
        '+' => '=',
        '|' => '\\',
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        other => other,
    }
}

/// The key part of a keystroke: printable characters from
/// `unmodified_character`, named keys from `windows_key_code`.
fn key_name(unmodified: u16, vk: i32) -> Option<String> {
    if let Some(c) = char::from_u32(unmodified as u32) {
        let c = unshifted(c);
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
        // OEM punctuation (no printable character field to read).
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
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
    /// The `Rc` is cloned out of the cell before `claims`/`deliver` run, so
    /// a `deliver` that re-installs or clears the router cannot panic on a
    /// `RefCell` double-borrow inside the `extern "C"` CEF callback.
    static ROUTER: RefCell<Option<Rc<Router>>> = const { RefCell::new(None) };
    /// Chrome command ids blocked on the main thread, oldest first.
    static BLOCKED: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    /// When each blocked id was last logged, for the once-per-second cap.
    static LAST_LOG: RefCell<HashMap<i32, Instant>> = RefCell::new(HashMap::new());
    /// Native key codes consumed on keydown, awaiting their KEYUP.
    static CONSUMED: RefCell<HashSet<i32>> = RefCell::new(HashSet::new());
}

/// Installs the host key router (main thread only). `claims` is asked
/// synchronously from `on_pre_key_event`; when it returns true the event is
/// consumed and `deliver(keystroke)` runs on a later main-queue turn.
pub fn set_key_router(
    claims: impl Fn(&str) -> bool + 'static,
    deliver: impl Fn(String) + 'static,
) {
    ROUTER.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(Router {
            claims: Box::new(claims),
            deliver: Box::new(deliver),
        }));
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

/// The routing decision over the installed router: clones the `Rc` out of the
/// cell before calling `claims`, so re-entrant `set_key_router` /
/// `clear_key_router` calls cannot panic. This is the same decision the CEF
/// handler in [`route_pre_key_event`] uses; tests drive it plus
/// [`deliver_keystroke`] directly to run the deferred step synchronously.
pub(crate) fn claim_keystroke(keystroke: &str) -> Option<String> {
    let router = ROUTER.with(|slot| slot.borrow().clone());
    router.and_then(|r| decide_consume(keystroke, &r.claims))
}

/// Runs the routing decision for a converted keystroke: consumes claimed
/// keystrokes (queuing `deliver` on the main queue's next turn) and reports
/// whether the CEF event was consumed. Holds no lock; calls neither AppKit
/// nor CEF itself.
pub(crate) fn route_pre_key_event(keystroke: String, native_key_code: i32) -> bool {
    if let Some(owned) = claim_keystroke(&keystroke) {
        note_consumed_keydown(native_key_code);
        crate::backend::defer(move || deliver_keystroke(owned));
        true
    } else {
        false
    }
}

/// Runs the router's `deliver` sink (a later main-queue turn, never the CEF
/// callback). Drops the keystroke when no router is installed.
pub(crate) fn deliver_keystroke(keystroke: String) {
    let router = ROUTER.with(|slot| slot.borrow().clone());
    if let Some(router) = router {
        (router.deliver)(keystroke);
    }
}

/// Remembers a claimed keydown's native code so its KEYUP (and any CHAR) can
/// be swallowed instead of reaching the page as an orphan keyup.
pub(crate) fn note_consumed_keydown(native_key_code: i32) {
    CONSUMED.with(|set| {
        let mut set = set.borrow_mut();
        if set.len() >= 32 {
            set.clear();
        }
        set.insert(native_key_code);
    });
}

/// Swallows the trailer of a consumed key: a KEYUP clears its code, a CHAR
/// keeps it (the KEYUP is still coming). Returns true when the event must
/// not reach the page.
pub(crate) fn swallow_trailer(native_key_code: i32, is_key_up: bool) -> bool {
    CONSUMED.with(|set| {
        if is_key_up {
            set.borrow_mut().remove(&native_key_code)
        } else {
            set.borrow().contains(&native_key_code)
        }
    })
}

// ------------------------------------------------------------------ the Chrome command filter

/// "Open in new tab/window/profile/split view" context-menu ids (plus the
/// link/image/AV/search neighbours in the same family): blocked and recorded
/// so a later task can turn them into Baaz tabs instead of Chromium windows.
const OPEN_IN_NEW: &[i32] = &[
    50100, 50101, 50102, 50108, 50109, 50111, 50113, 50123, 50124, 50125, 50135, 50136, 50169,
    50190, 50191, 50192,
];

/// Whether a Chrome command id may run. Allowed: navigation (33000–33999:
/// back/forward/reload/…), edit (36000–36999), zoom (38000–38999), DevTools
/// exactly {40004, 40005, 40007, 40023, 40237}, and context-menu commands
/// (50000–59999) except the [`OPEN_IN_NEW`] "open in new …" ids (and 57316,
/// the same family outside the menu range). Everything else — new
/// window/tab, task manager, feedback, close tab/window, find, bookmarks,
/// print, exit, fullscreen, … — is blocked.
pub fn chrome_command_allowed(id: i32) -> bool {
    if OPEN_IN_NEW.contains(&id) || id == 57316 {
        return false;
    }
    matches!(
        id,
        33000..=33999 | 36000..=36999 | 38000..=38999 | 50000..=59999 | 40004 | 40005 | 40007 | 40023 | 40237
    )
}

/// Maps a context-menu command id to the tab it should open as: `Some(false)`
/// means "open this URL in a background tab" (handled by the context-menu
/// handler through `take_open_requests`), `None` means Chromium handles the
/// command itself.
///
/// Every [`OPEN_IN_NEW`] "open in new …" id (plus 57316, the same family
/// outside the menu range) maps to a background tab: that is where Chromium
/// puts context-menu tabs, and a "new window"/incognito choice degrades to a
/// normal background tab — Baaz owns no other window to put it in. Pure, so
/// the mapping is unit-testable without a browser.
pub fn context_menu_open_foreground(command_id: i32) -> Option<bool> {
    if OPEN_IN_NEW.contains(&command_id) || command_id == 57316 {
        Some(false)
    } else {
        None
    }
}

/// Records a blocked Chrome command id for [`take_blocked_commands`]: at most
/// 64 entries (oldest dropped), logged at most once per second per id.
pub(crate) fn record_blocked_command(id: i32) {
    BLOCKED.with(|slot| {
        let mut blocked = slot.borrow_mut();
        blocked.push(id);
        while blocked.len() > 64 {
            blocked.remove(0);
        }
    });
    LAST_LOG.with(|slot| {
        let mut last = slot.borrow_mut();
        let now = Instant::now();
        let due = match last.get(&id) {
            Some(&t) => now.duration_since(t).as_secs() >= 1,
            None => true,
        };
        if due {
            last.insert(id, now);
            eprintln!("aui-cef: blocked Chrome command {id}");
        }
    });
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
        // ⌘⇧T (modifier order follows gpui's documented key syntax)
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
    fn shifted_punctuation_maps_to_its_unshifted_key() {
        // ⌘⇧[ arrives as `{` on macOS but claims as `shift-cmd-[`.
        assert_eq!(down(CMD | SHIFT, 0xDB, '{'), Some(String::from("shift-cmd-[")));
        assert_eq!(down(CMD | SHIFT, 0xDD, '}'), Some(String::from("shift-cmd-]")));
        assert_eq!(down(CMD | SHIFT, 0xC0, '~'), Some(String::from("shift-cmd-`")));
        assert_eq!(down(CMD | SHIFT, 0xBA, ':'), Some(String::from("shift-cmd-;")));
        assert_eq!(down(CMD | SHIFT, 0xDE, '"'), Some(String::from("shift-cmd-'")));
        assert_eq!(down(SHIFT, 0xBF, '?'), Some(String::from("shift-/")));
        assert_eq!(down(SHIFT, 0xBC, '<'), Some(String::from("shift-,")));
        assert_eq!(down(SHIFT, 0xBD, '_'), Some(String::from("shift--")));
        assert_eq!(down(SHIFT, 0xBB, '+'), Some(String::from("shift-=")));
        // ⌘⇧1 arrives as `!` but claims as the digit, keeping `shift-`.
        assert_eq!(down(CMD | SHIFT, 0x31, '!'), Some(String::from("shift-cmd-1")));
        assert_eq!(down(CMD | SHIFT, 0x32, '@'), Some(String::from("shift-cmd-2")));
        assert_eq!(down(CMD | SHIFT, 0x30, ')'), Some(String::from("shift-cmd-0")));
    }

    #[test]
    fn oem_windows_key_codes_fall_back_to_punctuation() {
        // No printable character field (dead key, layout gap): the VK decides.
        for (vk, key) in [
            (0xBA, ";"),
            (0xBB, "="),
            (0xBC, ","),
            (0xBD, "-"),
            (0xBE, "."),
            (0xBF, "/"),
            (0xC0, "`"),
            (0xDB, "["),
            (0xDC, "\\"),
            (0xDD, "]"),
            (0xDE, "'"),
        ] {
            assert_eq!(keystroke_of_fields(true, 0, vk, 0), Some(String::from(key)), "vk {vk:#X}");
        }
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
    fn filters_chrome_command_boundaries() {
        // Navigation / edit / zoom range edges.
        assert!(chrome_command_allowed(33999));
        assert!(!chrome_command_allowed(34000));
        assert!(chrome_command_allowed(36003));
        assert!(chrome_command_allowed(38999));
        assert!(!chrome_command_allowed(39000));
        // DevTools: task manager and feedback stay blocked, inspect toggles pass.
        assert!(!chrome_command_allowed(40006));
        assert!(!chrome_command_allowed(40008));
        assert!(chrome_command_allowed(40023));
        assert!(chrome_command_allowed(40237));
        // Context menu: ordinary items pass, "open in new …" is blocked.
        assert!(chrome_command_allowed(50154));
        assert!(!chrome_command_allowed(50100));
        assert!(!chrome_command_allowed(50192));
        assert!(!chrome_command_allowed(57316));
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
    fn keyup_and_char_swallowed_after_a_claimed_keydown() {
        clear_key_router();
        // An unconsumed key's trailer always reaches the page.
        assert!(!swallow_trailer(41, false));
        assert!(!swallow_trailer(41, true));
        // Claim a keydown through the installed router, as the CEF handler does.
        set_key_router(|ks| ks == "cmd-k", |_| {});
        assert_eq!(claim_keystroke("cmd-k"), Some(String::from("cmd-k")));
        note_consumed_keydown(41);
        // CHAR is swallowed but keeps the code (the KEYUP is still coming).
        assert!(swallow_trailer(41, false));
        assert!(swallow_trailer(41, false));
        // KEYUP is swallowed and clears the code.
        assert!(swallow_trailer(41, true));
        assert!(!swallow_trailer(41, true));
        clear_key_router();
    }

    #[test]
    fn router_reentrancy_does_not_panic() {
        clear_key_router();
        // A `deliver` that re-installs the router, driven through the same
        // claim + deliver path the CEF handler uses (`route_pre_key_event`
        // defers `deliver_keystroke` to the main queue, which cannot run in
        // a unit test, so the test runs that deferred step synchronously).
        set_key_router(
            |ks| ks == "shift-cmd-[",
            |_| {
                set_key_router(|_: &str| false, |_: String| {});
            },
        );
        let queued = claim_keystroke("shift-cmd-[").expect("claimed");
        note_consumed_keydown(26);
        deliver_keystroke(queued);
        assert!(swallow_trailer(26, true));
        // The router was replaced from inside `deliver` without panicking.
        assert_eq!(claim_keystroke("shift-cmd-["), None);
        // And clearing from inside `deliver` is safe too.
        set_key_router(
            |ks| ks == "cmd-k",
            |_| {
                clear_key_router();
            },
        );
        let queued = claim_keystroke("cmd-k").expect("claimed");
        deliver_keystroke(queued);
        assert_eq!(claim_keystroke("cmd-k"), None);
        clear_key_router();
    }

    #[test]
    fn context_menu_open_ids_become_background_tabs() {
        // Every "open in new …" id lands as a background-tab request …
        for id in [50100, 50101, 50102, 50108, 50109, 50111, 50113, 50123, 50124, 50125, 50135, 50136, 50169, 50190, 50191, 50192, 57316] {
            assert_eq!(context_menu_open_foreground(id), Some(false), "{id} should open a background tab");
        }
        // … including the named ones from the task: new tab, new window,
        // incognito, image/audio-video/search in a new tab.
        for id in [50100, 50101, 50102, 50123, 50125, 50136, 50192] {
            assert_eq!(context_menu_open_foreground(id), Some(false));
        }
        // Ordinary menu items and non-menu commands stay with Chromium.
        for id in [50154, 50000, 33002, 36001, 40004] {
            assert_eq!(context_menu_open_foreground(id), None, "{id} should reach Chromium");
        }
    }

    #[test]
    fn blocked_commands_drain_oldest_first() {
        let _ = take_blocked_commands();
        record_blocked_command(34014);
        record_blocked_command(34000);
        assert_eq!(take_blocked_commands(), vec![34014, 34000]);
        assert!(take_blocked_commands().is_empty());
    }

    #[test]
    fn blocked_list_caps_at_64_dropping_oldest() {
        let _ = take_blocked_commands();
        for id in 0..70 {
            record_blocked_command(id);
        }
        let drained = take_blocked_commands();
        assert_eq!(drained.len(), 64);
        assert_eq!(drained, (6..70).collect::<Vec<_>>());
    }
}
