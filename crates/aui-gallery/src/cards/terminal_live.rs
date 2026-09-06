//! `workbench/terminal-live` and `workbench/tui-live` — card 50's two panes,
//! but fed by `aui-terminal` instead of a literal block list.
//!
//! The frame, tab strip and pane split are card 50's
//! (`crate::cards::terminal`). What changed is where the content comes from: a
//! [`FakePty`] replays a recorded session, byte for byte with its OSC 133
//! markers and SGR colour, and [`BlockParser`] turns that stream back into
//! blocks. The panes grow while the card is on screen, so a screenshot taken
//! at 1500 ms and one at 4000 ms show different amounts of the session.

use aui::data::{icon_button, ButtonSize};
use aui::shell::{tab_strip, TabItem};
use aui::workbench::TermPrompt;
use aui_icons::{IconName, Provider};
use aui_terminal::{block_terminal_view, tui_view, BlockParser, FakePty, TerminalState};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// Card 50's frame: the gallery adds 20, the card pulls in by 8.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 980.0 - 24.0;
/// `.term{height:690px}` in the 980×720 card…
pub(super) const TERM_FRAME_H: f32 = 690.0;
/// …and the TUI-only card is 980×560.
pub(super) const TUI_FRAME_H: f32 = 530.0;
/// The tab strip's hint text: `font-size:11px;margin-right:8px`.
const HINT_TEXT: f32 = 11.0;
const HINT_MARGIN: f32 = 8.0;
/// Glyphs in the strip's xs buttons are 12 px.
const XS_GLYPH: f32 = 12.0;

/// A session on the card 50 recording, kept in window state so its poll timer
/// survives between frames.
fn session(key: &'static str, tui: bool, window: &mut Window, cx: &mut App) -> Entity<TerminalState> {
    window.use_keyed_state(SharedString::from(key), cx, move |_, _| {
        let fake = if tui { FakePty::tui() } else { FakePty::card50() };
        // The parser reads the recording's own clock, so the durations are the
        // ones the session was recorded with even though replay is faster.
        let parser = BlockParser::with_clock(fake.clock());
        let mut state = TerminalState::with_parser(Box::new(fake), parser);
        state.set_prompt(TermPrompt {
            text: "pnpm test --filter web-".into(),
            context: vec!["\u{2687} feature/checkout-flow-v2".into(), "~/work/acme".into()],
        });
        state
    })
}

/// Card 50's tab strip, over a fresh key so it can sit beside card 50 itself.
/// `start` is the tab it opens on — the shell for the split card, the agent
/// for the TUI-only one, which is the tab that pane actually belongs to.
pub(super) fn strip(key: &'static str, start: usize, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.aui().colors;
    let active = window.use_keyed_state(SharedString::from(key), cx, move |_, _| start);
    let current = *active.read(cx);
    let tabs = vec![
        TabItem::new("shell", "shell", IconName::Terminal).badge("2 splits").closable(false),
        TabItem::with_mark("pi", "pi", Provider::Pi).closable(false),
        TabItem::with_mark("claude", "claude", Provider::Claude).closable(false),
    ];
    let ids = ["shell", "pi", "claude"];
    let select = active.clone();
    let sub = |name: &str| SharedString::from(format!("{key}-{name}"));
    tab_strip(sub("strip"), tabs, current)
        .on_select(move |id, _, cx| {
            let i = ids.iter().position(|t| *t == id.as_ref()).unwrap_or(0);
            select.update(cx, |v, cx| {
                *v = i;
                cx.notify();
            })
        })
        .after_tabs(icon_button(sub("add"), IconName::Plus).ghost().size(ButtonSize::Xs).icon_size(px(XS_GLYPH)))
        .trailing(div().mr(px(HINT_MARGIN)).ui(HINT_TEXT).text_color(p.ink_3).whitespace_nowrap().child("⌘D split right · ⌘⇧D split down"))
        .trailing(icon_button(sub("search"), IconName::Search).ghost().size(ButtonSize::Xs).icon_size(px(XS_GLYPH)))
        .trailing(icon_button(sub("split"), IconName::Split).ghost().size(ButtonSize::Xs).icon_size(px(XS_GLYPH)))
}

/// The rounded, hairlined terminal frame with the strip on top.
pub(super) fn frame(height: f32, strip: impl IntoElement, body: impl IntoElement, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    div()
        .w(px(FRAME_W))
        .h(px(height))
        .flex_none()
        .m(px(FRAME_INSET))
        .child(
            v_flex()
                .size_full()
                .rounded(px(scale::R_LG))
                .border_1()
                .border_color(p.line_strong)
                .bg(p.term_bg)
                .overflow_hidden()
                .child(div().w_full().flex_none().bg(p.surface_1).child(strip))
                .child(body),
        )
        .into_any_element()
}

/// Builds the block-terminal card: the live blocks beside the live TUI.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let blocks = session("terminal-live-shell", false, window, cx);
    let screen = session("terminal-live-tui", true, window, cx);
    let strip = strip("terminal-live-tabs", 0, window, cx);

    let left = block_terminal_view("terminal-live-term", &blocks)
        .marker("restored scrollback · 09:02")
        .on_intent(|intent, _, _| {
            // The gallery has nothing to run, copy to or ask; the card exists
            // to show that the wiring reaches a host.
            let _ = intent;
        });
    let right = tui_view("terminal-live-tui-pane", &screen)
        .input("")
        .footer("⇧⇥ plan mode · ⌘D split · agent runs in a PTY with your login")
        .hint_key("⌘⇧D");

    frame(
        TERM_FRAME_H,
        strip,
        h_flex()
            .w_full()
            .flex_1()
            .min_h(px(0.0))
            .items_stretch()
            .child(div().flex_1().min_w(px(0.0)).h_full().child(left))
            .child(div().flex_1().min_w(px(0.0)).h_full().border_l_1().border_color(p.line).child(right)),
        cx,
    )
}

/// Builds the TUI card: the agent's screen alone, streamed by the same backend.
pub fn build_tui(window: &mut Window, cx: &mut App) -> AnyElement {
    let screen = session("tui-live-screen", true, window, cx);
    let strip = strip("tui-live-tabs", 2, window, cx);
    let pane = tui_view("tui-live-pane", &screen)
        .input("")
        .footer("⇧⇥ plan mode · ⌘D split · agent runs in a PTY with your login")
        .hint_key("⌘⇧D");
    frame(TUI_FRAME_H, strip, div().w_full().flex_1().min_h(px(0.0)).child(pane), cx)
}

// ---------------------------------------------------------------------------
// The real backends (`--features pty` and `--features pty,tui`)
// ---------------------------------------------------------------------------

/// `workbench/terminal-real` and `workbench/tui-real`: the same two panes over
/// a real pseudo-terminal running the user's login shell.
#[cfg(feature = "pty")]
mod real {
    use super::{frame, strip, TERM_FRAME_H, TUI_FRAME_H};
    use aui::workbench::TermPrompt;
    use aui_terminal::{block_terminal_view, login_shell, BlockParser, Pty, TerminalBackend, TerminalState};
    use gpui::*;
    use gpui_kit::base::v_flex;

    /// What the TUI card runs. A full-screen program that redraws in place, so
    /// the grid has something the block parser could not represent; `top`
    /// ships with macOS and needs no configuration.
    const TUI_COMMAND: &str = "top\n";
    /// The grid opens at this size until the pane measures itself.
    const GRID_COLS: u16 = 100;
    /// …and this many rows.
    const GRID_ROWS: u16 = 30;

    /// The bytes a keystroke sends to the shell, or `None` when it is not
    /// something a terminal carries. This is the whole keyboard: the shell
    /// echoes what it receives, so there is no local line editing to keep in
    /// step with it.
    fn key_bytes(keystroke: &Keystroke) -> Option<Vec<u8>> {
        let m = &keystroke.modifiers;
        if m.control {
            // `⌃C`, `⌃D`, `⌃Z`: the control character is the letter's position
            // in the alphabet.
            let c = keystroke.key.chars().next()?.to_ascii_lowercase();
            return c.is_ascii_lowercase().then(|| vec![c as u8 - b'a' + 1]);
        }
        if m.platform {
            return None;
        }
        let named: &[u8] = match keystroke.key.as_str() {
            "enter" => b"\r",
            "backspace" => b"\x7f",
            "delete" => b"\x1b[3~",
            "tab" => b"\t",
            "escape" => b"\x1b",
            "up" => b"\x1b[A",
            "down" => b"\x1b[B",
            "right" => b"\x1b[C",
            "left" => b"\x1b[D",
            "home" => b"\x1b[H",
            "end" => b"\x1b[F",
            "pageup" => b"\x1b[5~",
            "pagedown" => b"\x1b[6~",
            _ => return keystroke.key_char.as_ref().map(|text| text.as_bytes().to_vec()),
        };
        Some(named.to_vec())
    }

    /// Wraps `body` so it holds the keyboard and forwards every keystroke to
    /// `state`'s session.
    fn keyboard(id: &'static str, state: &Entity<TerminalState>, window: &mut Window, cx: &mut App, body: impl IntoElement) -> AnyElement {
        let focus = window.use_keyed_state(SharedString::from(format!("{id}-focus")), cx, |_, cx| cx.focus_handle());
        let handle = focus.read(cx).clone();
        // The card is the only thing on the stage, so it takes the keyboard as
        // soon as it is drawn; without this the shell would never see a key.
        if !handle.is_focused(window) {
            window.focus(&handle, cx);
        }
        let state = state.clone();
        v_flex()
            .track_focus(&handle)
            .size_full()
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if let Some(bytes) = key_bytes(&event.keystroke) {
                    state.update(cx, |state, cx| {
                        state.write(&bytes);
                        cx.notify();
                    });
                }
            })
            .child(body)
            .into_any_element()
    }

    /// A session on a real pty running the user's login shell, kept in window
    /// state so the shell outlives a frame — and dies with the window, because
    /// dropping the `Pty` kills it.
    fn session(key: &'static str, grid: bool, window: &mut Window, cx: &mut App) -> Entity<TerminalState> {
        window.use_keyed_state(SharedString::from(key), cx, move |_, _| {
            let mut pty = Pty::new();
            let home = std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
            let shell = login_shell();
            if let Err(error) = pty.spawn(&shell, &home) {
                eprintln!("aui-gallery: no pty ({error})");
            }
            if grid {
                #[cfg(feature = "tui")]
                {
                    // The shell reads this as soon as it is ready, so there is
                    // nothing to wait for.
                    pty.write(TUI_COMMAND.as_bytes());
                    return TerminalState::with_grid(Box::new(pty), GRID_COLS, GRID_ROWS);
                }
            }
            let mut state = TerminalState::with_parser(Box::new(pty), BlockParser::new());
            state.set_prompt(TermPrompt { text: SharedString::default(), context: vec![SharedString::from(shell)] });
            state
        })
    }

    /// Builds the block-terminal card over the real shell.
    pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
        let blocks = session("terminal-real-shell", false, window, cx);
        let strip = strip("terminal-real-tabs", 0, window, cx);
        let pane = block_terminal_view("terminal-real-term", &blocks).on_intent(|intent, _, _| {
            eprintln!("aui-gallery: terminal intent {intent:?}");
        });
        let body = keyboard("terminal-real", &blocks, window, cx, div().w_full().flex_1().min_h(px(0.0)).child(pane));
        frame(TERM_FRAME_H, strip, div().w_full().flex_1().min_h(px(0.0)).child(body), cx)
    }

    /// Builds the TUI card over a full-screen program in the same real pty.
    #[cfg(feature = "tui")]
    pub fn build_tui(window: &mut Window, cx: &mut App) -> AnyElement {
        let screen = session("tui-real-screen", true, window, cx);
        let strip = strip("tui-real-tabs", 2, window, cx);
        let pane = aui_terminal::tui_grid_view("tui-real-pane", &screen).hint_key("⌘⇧D");
        let body = keyboard("tui-real", &screen, window, cx, div().w_full().flex_1().min_h(px(0.0)).child(pane));
        frame(TUI_FRAME_H, strip, div().w_full().flex_1().min_h(px(0.0)).child(body), cx)
    }
}

#[cfg(feature = "pty")]
pub use real::build as build_real;
#[cfg(all(feature = "pty", feature = "tui"))]
pub use real::build_tui as build_tui_real;
