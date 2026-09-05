//! Card 50 · Terminal: the block terminal beside the agent's TUI, under a
//! shared tab strip. Reproduces `design/src/cards/workbench/50-terminal.html`
//! at 980×720 (body padding 12).

use aui::shell::{tab_strip, TabItem};
use aui::workbench::{block_terminal, tui_pane, BlockState, TermBlock, TermPrompt};
use aui::data::{icon_button, ButtonSize};
use aui_icons::{IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `body.ds{padding:12px}` — the gallery adds 20, the card pulls in by 8; `.term{height:690px}`.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 980.0 - 24.0;
const FRAME_H: f32 = 690.0;
/// The tab strip's hint text: `font-size:11px;margin-right:8px`.
const HINT_TEXT: f32 = 11.0;
const HINT_MARGIN: f32 = 8.0;
/// Glyphs in the strip's xs buttons are 12 px.
const XS_GLYPH: f32 = 12.0;

/// The blocks of the left pane.
pub fn sample_blocks() -> Vec<TermBlock> {
    let d = "\u{1b}[2m";
    let r = "\u{1b}[0m";
    vec![
        TermBlock::new("status", "git status -sb", BlockState::Done, "0.1 s").old().output(vec![
            "\u{1b}[34m## feature/checkout-flow-v2...origin/feature/checkout-flow-v2\u{1b}[0m".into(),
            "\u{1b}[33m M\u{1b}[0m src/checkout/validators.ts".into(),
            "\u{1b}[32mA \u{1b}[0m src/checkout/validators.test.ts".into(),
        ]),
        TermBlock::new("install", "pnpm i", BlockState::Done, "6.2 s")
            .old()
            .agent(Provider::Claude)
            .output(vec![format!("{d}Lockfile is up to date, resolution step is skipped{r}"), format!("{d}Already up to date{r}")])
            .folded(38),
        TermBlock::new("lint", "pnpm lint", BlockState::Failed, "3.2 s · exit 1").agent(Provider::Claude).output(vec![
            "\u{1b}[31m✖\u{1b}[0m src/checkout/validators.ts".into(),
            format!("  46:5  \u{1b}[31merror\u{1b}[0m  'validateCanadianPostal' is not defined  {d}no-undef{r}"),
            "".into(),
            "\u{1b}[31m✖ 1 problem\u{1b}[0m (1 error, 0 warnings)".into(),
        ]),
        TermBlock::new("vitest", "pnpm vitest run src/checkout", BlockState::Running, "12 s").output(vec![
            format!("\u{1b}[32m✓\u{1b}[0m validators.test.ts {d}(18){r} {d}412ms{r}"),
            format!("\u{1b}[32m✓\u{1b}[0m AddressForm.test.tsx {d}(9){r} {d}1.1s{r}"),
            format!("{d}⠼{r} checkout.e2e.ts {d}running…{r}"),
        ]),
    ]
}

/// The lines of the agent's TUI.
pub fn sample_tui() -> Vec<String> {
    let d = "\u{1b}[2m";
    let r = "\u{1b}[0m";
    let m = "\u{1b}[35m";
    let b = "\u{1b}[34m";
    let y = "\u{1b}[33m";
    vec![
        format!("{m}✱{r} \u{1b}[1mClaude Code{r} {d}v2.1.174{r}"),
        format!("{d}Opus 4.6 · ~/work/acme/checkout-flow-v2{r}"),
        "".into(),
        format!("{d}›{r} tighten address validation and add coverage"),
        "".into(),
        format!("{m}●{r} Read {b}src/checkout/validators.ts{r}"),
        format!("  {d}⎿  Read 180 lines{r}"),
        format!("{m}●{r} Update {b}src/checkout/validators.ts{r}"),
        format!("  {d}⎿  Added 8 lines, removed 3 lines{r}"),
        format!("{m}●{r} Bash {b}pnpm vitest run src/checkout{r}"),
        format!("  {d}⎿  Running…{r}"),
        "".into(),
        format!("{y}⠼ Thinking…{r} {d}(12s · esc to interrupt){r}"),
    ]
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let active = window.use_keyed_state("card50-tab", cx, |_, _| 0usize);
    let current = *active.read(cx);
    let tabs = vec![
        TabItem::new("shell", "shell", IconName::Terminal).badge("2 splits").closable(false),
        TabItem::with_mark("pi", "pi", Provider::Pi).closable(false),
        TabItem::with_mark("claude", "claude", Provider::Claude).closable(false),
    ];
    let ids = ["shell", "pi", "claude"];
    let select = active.clone();
    let strip = tab_strip("card50-tabs", tabs, current)
        .on_select(move |id, _, cx| {
            let i = ids.iter().position(|t| *t == id.as_ref()).unwrap_or(0);
            select.update(cx, |v, cx| {
                *v = i;
                cx.notify();
            })
        })
        .after_tabs(icon_button("card50-add", IconName::Plus).ghost().size(ButtonSize::Xs).icon_size(px(XS_GLYPH)))
        .trailing(div().mr(px(HINT_MARGIN)).ui(HINT_TEXT).text_color(p.ink_3).whitespace_nowrap().child("⌘D split right · ⌘⇧D split down"))
        .trailing(icon_button("card50-search", IconName::Search).ghost().size(ButtonSize::Xs).icon_size(px(XS_GLYPH)))
        .trailing(icon_button("card50-split", IconName::Split).ghost().size(ButtonSize::Xs).icon_size(px(XS_GLYPH)));

    let left = block_terminal("card50-term", sample_blocks())
        .marker("restored scrollback · 09:02")
        .prompt(TermPrompt { text: "pnpm test --filter web-".into(), context: vec!["⎇ feature/checkout-flow-v2".into(), "~/work/acme".into()] });
    let right = tui_pane("card50-tui", sample_tui())
        .input("")
        .footer("⇧⇥ plan mode · ⌘D split · agent runs in a PTY with your login")
        .hint_key("⌘⇧D");

    div()
        .w(px(FRAME_W))
        .h(px(FRAME_H))
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
                .child(
                    h_flex()
                        .w_full()
                        .flex_1()
                        .min_h(px(0.0))
                        .items_stretch()
                        .child(div().flex_1().min_w(px(0.0)).h_full().child(left))
                        .child(div().flex_1().min_w(px(0.0)).h_full().border_l_1().border_color(p.line).child(right)),
                ),
        )
        .into_any_element()
}
