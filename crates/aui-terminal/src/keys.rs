//! A pure, unit-tested key encoder: key descriptions plus terminal modes in,
//! bytes for the pty out. No gpui types here on purpose, so the whole module
//! is testable without an application.

#![warn(missing_docs)]

/// A key that is not printable text: return, arrows, function keys and the
/// navigation block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    /// Return / enter.
    Enter,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Escape.
    Escape,
    /// Left arrow.
    Left,
    /// Up arrow.
    Up,
    /// Right arrow.
    Right,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Insert.
    Insert,
    /// Delete (forward delete).
    Delete,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// A function key, 1–20.
    F(u8),
}

/// What was pressed: printable text (already resolved through the layout and
/// the IME) or a special key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyInput {
    /// Printable text, usually a single character.
    Text(char),
    /// A non-printable key.
    Key(SpecialKey),
}

/// The modifiers held with the key. `meta` is the platform key (Super on
/// macOS); `alt` is Option, which the host may treat as Meta instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers {
    /// Shift.
    pub shift: bool,
    /// Alt / Option.
    pub alt: bool,
    /// Control.
    pub ctrl: bool,
    /// Platform / Super / Command.
    pub meta: bool,
}

impl KeyModifiers {
    /// No modifiers.
    pub fn none() -> Self {
        Self::default()
    }

    /// The xterm modifier parameter: `1 + shift + 2*alt + 4*ctrl + 8*meta`.
    /// Returns `None` when no modifier is held, since unmodified keys keep
    /// their short form.
    pub fn param(&self) -> Option<u8> {
        let mut m = 1u8;
        if self.shift {
            m += 1;
        }
        if self.alt {
            m += 2;
        }
        if self.ctrl {
            m += 4;
        }
        if self.meta {
            m += 8;
        }
        (m > 1).then_some(m)
    }
}

/// The terminal modes that change what a key sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModes {
    /// DECCKM application-cursor mode: arrows (and Home/End) send `SS3`
    /// `O` sequences instead of `CSI` `[` ones.
    pub app_cursor: bool,
    /// Keypad application mode: Enter sends the keypad result.
    pub app_keypad: bool,
}

/// Encodes one key press into the bytes the program expects.
///
/// Rules, in order:
/// - `⌃`+letter sends the ASCII control code (`⌃A` → `0x01`); with Alt/Meta
///   held as well the code is ESC-prefixed.
/// - Alt (or Option when `option_as_meta` is set) ESC-prefixes printable
///   text; otherwise text goes through unchanged.
/// - Arrows and Home/End follow DECCKM (`app_cursor`); every modified
///   special key takes the `CSI 1;{mod}X` form (Insert/Delete/PgUp/PgDn and
///   F5+ take the `CSI [{n};{mod}~` form); F1–F4 are `SS3` unmodified.
/// - `⇧Tab` is `CSI Z`; other modified Tab/Enter/Backspace use the
///   `CSI {u}` form; keypad Enter follows `app_keypad`.
pub fn encode(
    input: &KeyInput,
    mods: &KeyModifiers,
    modes: &KeyModes,
    option_as_meta: bool,
) -> Vec<u8> {
    let meta = mods.alt || (option_as_meta && mods.alt) || mods.meta;
    // `option_as_meta` only matters for the Option key: without the setting
    // Option already arrived as `alt`, so this is just explicit.
    let _ = option_as_meta;
    match input {
        KeyInput::Text(c) => encode_text(*c, mods, meta),
        KeyInput::Key(k) => encode_special(*k, mods, meta, modes),
    }
}

/// Encodes printable text.
fn encode_text(c: char, mods: &KeyModifiers, meta: bool) -> Vec<u8> {
    // ⌃+letter (and ⌃+? for DEL) beats every other rule.
    if mods.ctrl && !mods.meta {
        if let Some(code) = ctrl_code(c) {
            if meta {
                return vec![0x1b, code];
            }
            return vec![code];
        }
    }
    let mut out = Vec::new();
    if meta {
        out.push(0x1b);
    }
    let mut buf = [0u8; 4];
    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    out
}

/// Maps `⌃`+key onto its ASCII control code.
fn ctrl_code(c: char) -> Option<u8> {
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1),
        'A'..='Z' => Some(c as u8 - b'A' + 1),
        // `⌃[` is ESC and `⌃?`…`⌃_` are the trailing codes; `@` is NUL.
        '@' => Some(0x00),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' => Some(0x1f),
        '?' => Some(0x7f),
        ' ' => Some(0x00),
        _ => None,
    }
}

/// Encodes a special key.
fn encode_special(k: SpecialKey, mods: &KeyModifiers, meta: bool, modes: &KeyModes) -> Vec<u8> {
    let param = mods.param();
    match k {
        SpecialKey::Escape => {
            // ESC with modifiers is `CSI 27;{mod}u`; a bare extra ESC would
            // read as two presses.
            match param {
                Some(m) => csi_u(27, m),
                None => vec![0x1b],
            }
        }
        SpecialKey::Enter => {
            if modes.app_keypad && param.is_none() && !meta {
                return vec![0x1b, b'O', b'M'];
            }
            match param {
                Some(m) => csi_u(13, m),
                None if meta => vec![0x1b, b'\r'],
                None => vec![b'\r'],
            }
        }
        SpecialKey::Tab => {
            // ⇧Tab is the one modified Tab with a legacy short form.
            if mods.shift && param == Some(2) {
                return vec![0x1b, b'[', b'Z'];
            }
            match param {
                Some(m) => csi_u(9, m),
                None if meta => vec![0x1b, b'\t'],
                None => vec![b'\t'],
            }
        }
        SpecialKey::Backspace => match param {
            Some(m) => csi_u(127, m),
            None if meta => vec![0x1b, 0x7f],
            None => vec![0x7f],
        },
        SpecialKey::Left => arrow(b'D', param, modes.app_cursor, meta),
        SpecialKey::Up => arrow(b'A', param, modes.app_cursor, meta),
        SpecialKey::Right => arrow(b'C', param, modes.app_cursor, meta),
        SpecialKey::Down => arrow(b'B', param, modes.app_cursor, meta),
        SpecialKey::Home => cursor_key(b'H', param, modes.app_cursor, meta),
        SpecialKey::End => cursor_key(b'F', param, modes.app_cursor, meta),
        SpecialKey::Insert => tilde(2, param, meta),
        SpecialKey::Delete => tilde(3, param, meta),
        SpecialKey::PageUp => tilde(5, param, meta),
        SpecialKey::PageDown => tilde(6, param, meta),
        SpecialKey::F(n) => function(n, param, meta),
    }
}

/// An arrow key: `SS3` in application-cursor mode, otherwise `CSI`, with the
/// modifier form winning over both when modifiers are held.
fn arrow(final_byte: u8, param: Option<u8>, app_cursor: bool, meta: bool) -> Vec<u8> {
    if let Some(m) = param {
        let mut out = vec![0x1b, b'[', b'1', b';'];
        out.extend_from_slice(m.to_string().as_bytes());
        out.push(final_byte);
        meta_prefix(out, meta)
    } else if app_cursor {
        meta_prefix(vec![0x1b, b'O', final_byte], meta)
    } else {
        meta_prefix(vec![0x1b, b'[', final_byte], meta)
    }
}

/// Home/End behave like arrows under DECCKM.
fn cursor_key(final_byte: u8, param: Option<u8>, app_cursor: bool, meta: bool) -> Vec<u8> {
    arrow(final_byte, param, app_cursor, meta)
}

/// A `~`-terminated key (Insert/Delete/PgUp/PgDn).
fn tilde(n: u8, param: Option<u8>, meta: bool) -> Vec<u8> {
    let mut out = vec![0x1b, b'['];
    out.extend_from_slice(n.to_string().as_bytes());
    if let Some(m) = param {
        out.push(b';');
        out.extend_from_slice(m.to_string().as_bytes());
    }
    out.push(b'~');
    meta_prefix(out, meta)
}

/// A `CSI {code}u` sequence for a modified key without a legacy form.
fn csi_u(code: u32, modifier: u8) -> Vec<u8> {
    let mut out = vec![0x1b, b'['];
    out.extend_from_slice(code.to_string().as_bytes());
    out.push(b';');
    out.extend_from_slice(modifier.to_string().as_bytes());
    out.push(b'u');
    out
}

/// Function keys 1–20. F1–F4 are `SS3` unmodified; the rest are `~`-style.
fn function(n: u8, param: Option<u8>, meta: bool) -> Vec<u8> {
    const SS3: [u8; 4] = *b"PQRS";
    if (1..=4).contains(&n) && param.is_none() {
        return meta_prefix(vec![0x1b, b'O', SS3[(n - 1) as usize]], meta);
    }
    if (1..=4).contains(&n) {
        let m = param.unwrap_or(2).to_string();
        let mut out = vec![0x1b, b'[', b'1', b';'];
        out.extend_from_slice(m.as_bytes());
        out.push(SS3[(n - 1) as usize]);
        return meta_prefix(out, meta);
    }
    // F5 F6 F7 F8 F9 F10 F11 F12 map to 15 17 18 19 20 21 23 24.
    const TILDE: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
    if (5..=12).contains(&n) {
        return tilde(TILDE[(n - 5) as usize], param, meta);
    }
    // F13+ have no legacy form: `CSI {n};{mod}u` past F12's block.
    let code = 25u32 + (n as u32 - 13);
    match param {
        Some(m) => csi_u(code, m),
        // Unmodified: `CSI {code} u`, carrying no modifier parameter. Built
        // directly rather than spliced out of `csi_u`, whose `;1` does not sit
        // at a fixed offset — and every code here (25–32) is two digits.
        // Meta then ESC-prefixes it, the same way every other branch does.
        None => meta_prefix(csi_u_plain(code), meta),
    }
}

/// `CSI {code} u` — the unmodified form, which carries no modifier parameter.
fn csi_u_plain(code: u32) -> Vec<u8> {
    let mut out = vec![0x1b, b'['];
    out.extend_from_slice(code.to_string().as_bytes());
    out.push(b'u');
    out
}

/// ESC-prefixes an already-encoded sequence for Meta.
fn meta_prefix(mut out: Vec<u8>, meta: bool) -> Vec<u8> {
    if meta {
        out.insert(0, 0x1b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encodes with no modifiers and no modes.
    fn plain(input: &KeyInput) -> Vec<u8> {
        encode(input, &KeyModifiers::none(), &KeyModes::default(), false)
    }

    #[test]
    fn plain_text_passes_through_as_utf8() {
        assert_eq!(plain(&KeyInput::Text('a')), b"a");
        assert_eq!(plain(&KeyInput::Text('é')), "é".as_bytes());
    }

    #[test]
    fn ctrl_plus_letter_sends_the_control_code() {
        let ctrl = KeyModifiers { ctrl: true, ..KeyModifiers::none() };
        let modes = KeyModes::default();
        assert_eq!(encode(&KeyInput::Text('a'), &ctrl, &modes, false), vec![0x01]);
        assert_eq!(encode(&KeyInput::Text('A'), &ctrl, &modes, false), vec![0x01]);
        assert_eq!(encode(&KeyInput::Text('c'), &ctrl, &modes, false), vec![0x03]);
        assert_eq!(encode(&KeyInput::Text('['), &ctrl, &modes, false), vec![0x1b]);
        assert_eq!(encode(&KeyInput::Text('?'), &ctrl, &modes, false), vec![0x7f]);
    }

    #[test]
    fn ctrl_plus_alt_esc_prefixes_the_control_code() {
        let mods = KeyModifiers { ctrl: true, alt: true, ..KeyModifiers::none() };
        assert_eq!(encode(&KeyInput::Text('c'), &mods, &KeyModes::default(), false), vec![0x1b, 0x03]);
    }

    #[test]
    fn alt_prefixes_text_with_esc() {
        let alt = KeyModifiers { alt: true, ..KeyModifiers::none() };
        assert_eq!(encode(&KeyInput::Text('b'), &alt, &KeyModes::default(), false), vec![0x1b, b'b']);
    }

    #[test]
    fn option_as_meta_sends_esc_for_option() {
        let alt = KeyModifiers { alt: true, ..KeyModifiers::none() };
        // Option-as-Meta is a setting, but plain Alt already ESC-prefixes, so
        // both readings agree here; the setting matters to the host, which
        // decides whether an Option keystroke arrives with `alt` set.
        assert_eq!(encode(&KeyInput::Text('b'), &alt, &KeyModes::default(), true), vec![0x1b, b'b']);
        assert_eq!(plain(&KeyInput::Text('b')), b"b");
    }

    #[test]
    fn arrows_use_csi_normally_and_ss3_in_app_cursor_mode() {
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Up)), vec![0x1b, b'[', b'A']);
        let app = KeyModes { app_cursor: true, ..KeyModes::default() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Up), &KeyModifiers::none(), &app, false),
            vec![0x1b, b'O', b'A']
        );
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Left)), vec![0x1b, b'[', b'D']);
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Left), &KeyModifiers::none(), &app, false),
            vec![0x1b, b'O', b'D']
        );
    }

    #[test]
    fn modified_arrows_take_the_modifier_form() {
        let shift = KeyModifiers { shift: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Up), &shift, &KeyModes::default(), false),
            vec![0x1b, b'[', b'1', b';', b'2', b'A']
        );
        let app = KeyModes { app_cursor: true, ..KeyModes::default() };
        // Modifiers win over application-cursor mode.
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Up), &shift, &app, false),
            vec![0x1b, b'[', b'1', b';', b'2', b'A']
        );
        let ctrl_alt = KeyModifiers { ctrl: true, alt: true, ..KeyModifiers::none() };
        // Alt ESC-prefixes the whole modified sequence, as xterm does.
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Right), &ctrl_alt, &KeyModes::default(), false),
            vec![0x1b, 0x1b, b'[', b'1', b';', b'7', b'C']
        );
    }

    #[test]
    fn home_and_end_follow_decckm() {
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Home)), vec![0x1b, b'[', b'H']);
        assert_eq!(plain(&KeyInput::Key(SpecialKey::End)), vec![0x1b, b'[', b'F']);
        let app = KeyModes { app_cursor: true, ..KeyModes::default() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Home), &KeyModifiers::none(), &app, false),
            vec![0x1b, b'O', b'H']
        );
        let shift = KeyModifiers { shift: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::End), &shift, &KeyModes::default(), false),
            vec![0x1b, b'[', b'1', b';', b'2', b'F']
        );
    }

    #[test]
    fn tilde_keys_encode_with_and_without_modifiers() {
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Insert)), b"\x1b[2~");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Delete)), b"\x1b[3~");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::PageUp)), b"\x1b[5~");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::PageDown)), b"\x1b[6~");
        let shift = KeyModifiers { shift: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::PageUp), &shift, &KeyModes::default(), false),
            b"\x1b[5;2~"
        );
        let ctrl = KeyModifiers { ctrl: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Delete), &ctrl, &KeyModes::default(), false),
            b"\x1b[3;5~"
        );
    }

    #[test]
    fn function_keys_f1_to_f4_are_ss3_unmodified() {
        assert_eq!(plain(&KeyInput::Key(SpecialKey::F(1))), vec![0x1b, b'O', b'P']);
        assert_eq!(plain(&KeyInput::Key(SpecialKey::F(4))), vec![0x1b, b'O', b'S']);
        let shift = KeyModifiers { shift: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::F(2)), &shift, &KeyModes::default(), false),
            vec![0x1b, b'[', b'1', b';', b'2', b'Q']
        );
    }

    #[test]
    fn function_keys_f5_to_f12_are_tilde_style() {
        assert_eq!(plain(&KeyInput::Key(SpecialKey::F(5))), b"\x1b[15~");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::F(11))), b"\x1b[23~");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::F(12))), b"\x1b[24~");
        let ctrl = KeyModifiers { ctrl: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::F(5)), &ctrl, &KeyModes::default(), false),
            b"\x1b[15;5~"
        );
    }

    #[test]
    fn function_keys_f13_to_f20_are_csi_u() {
        // Codes 25..=32 — every one of them two digits, which is what makes
        // splicing a `;1` out of the modified form by a fixed offset wrong.
        assert_eq!(plain(&KeyInput::Key(SpecialKey::F(13))), b"\x1b[25u");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::F(14))), b"\x1b[26u");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::F(20))), b"\x1b[32u");

        let ctrl = KeyModifiers { ctrl: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::F(13)), &ctrl, &KeyModes::default(), false),
            b"\x1b[25;5u"
        );

        // Option/Alt reaches the modifier parameter (alt adds 2, so 1+2 = 3)
        // rather than an ESC prefix: `param()` is Some whenever meta is true,
        // so the unmodified-but-meta arm is unreachable for every special key.
        let alt = KeyModifiers { alt: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::F(13)), &alt, &KeyModes::default(), true),
            b"\x1b[25;3u"
        );
    }

    #[test]
    fn enter_tab_backspace_and_escape() {
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Enter)), b"\r");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Tab)), b"\t");
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Backspace)), vec![0x7f]);
        assert_eq!(plain(&KeyInput::Key(SpecialKey::Escape)), vec![0x1b]);
        // ⇧Tab is the one modified Tab with a legacy form.
        let shift = KeyModifiers { shift: true, ..KeyModifiers::none() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Tab), &shift, &KeyModes::default(), false),
            vec![0x1b, b'[', b'Z']
        );
        let keypad = KeyModes { app_keypad: true, ..KeyModes::default() };
        assert_eq!(
            encode(&KeyInput::Key(SpecialKey::Enter), &KeyModifiers::none(), &keypad, false),
            vec![0x1b, b'O', b'M']
        );
    }

    #[test]
    fn modifier_params_follow_the_xterm_sum() {
        assert_eq!(KeyModifiers::none().param(), None);
        let shift = KeyModifiers { shift: true, ..KeyModifiers::none() };
        assert_eq!(shift.param(), Some(2));
        let all = KeyModifiers { shift: true, alt: true, ctrl: true, meta: true };
        assert_eq!(all.param(), Some(16));
    }
}
