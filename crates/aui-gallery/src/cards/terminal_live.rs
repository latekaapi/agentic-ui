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
const TERM_FRAME_H: f32 = 690.0;
/// …and the TUI-only card is 980×560.
const TUI_FRAME_H: f32 = 530.0;
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
fn strip(key: &'static str, start: usize, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
fn frame(height: f32, strip: impl IntoElement, body: impl IntoElement, cx: &mut App) -> AnyElement {
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
