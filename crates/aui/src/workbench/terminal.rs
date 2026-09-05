//! Card 50: the block terminal — every command is a block with a status
//! dot, the command, who ran it and how long it took, its output lines, a
//! fold row; old blocks dim, the live block is lifted, failures tint the
//! command row. Plus the restored-scrollback marker and the prompt row.

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_motion::{looping, tween, Loop, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, StyledText, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, spinner, ButtonSize};
use crate::transcript::{ansi_runs, parse_ansi};
use crate::util::{interaction_flags, TrackInteraction};

/// `.scroll{padding:10px 12px;gap:8px;font:12px/1.55 mono}`.
const SCROLL_PAD_Y: f32 = 10.0;
const SCROLL_PAD_X: f32 = 12.0;
const BLOCK_GAP: f32 = 8.0;
const TERM_TEXT: f32 = 12.0;
const TERM_LH: f32 = 1.55;
/// `.blk .cmd{gap:8px;padding:5px 10px;border-radius:8px}`.
const CMD_GAP: f32 = 8.0;
const CMD_PAD_Y: f32 = 5.0;
const CMD_PAD_X: f32 = 10.0;
/// `.ex{width:6px;height:6px}` with a 3 px accent-soft ring while running.
const EXIT_DOT: f32 = 6.0;
const EXIT_RING: f32 = 3.0;
/// `.who{gap:4px;font:500 10px/1 ui;padding-left:8px}` with an 11 px mark.
const WHO_GAP: f32 = 4.0;
const WHO_TEXT: f32 = 10.0;
const WHO_PAD: f32 = 8.0;
const WHO_MARK: f32 = 11.0;
/// `.dur{font-size:11px}` with a 10 px spinner while live.
const DUR_TEXT: f32 = 11.0;
const LIVE_SPINNER: f32 = 10.0;
/// `.out{padding:2px 10px 8px 26px}`.
const OUT_PAD_TOP: f32 = 2.0;
const OUT_PAD_RIGHT: f32 = 10.0;
const OUT_PAD_BOTTOM: f32 = 8.0;
const OUT_PAD_LEFT: f32 = 26.0;
/// `.blk.old{opacity:.72}`.
const OLD_OPACITY: f32 = 0.72;
/// `.fold{gap:6px;padding:0 10px 6px 26px;font:11px ui}` with an 11 px chevron.
const FOLD_GAP: f32 = 6.0;
const FOLD_PAD_BOTTOM: f32 = 6.0;
const FOLD_GLYPH: f32 = 11.0;
/// `.acts{right:8px;top:6px;gap:2px}` with 11 px glyphs.
const ACTS_RIGHT: f32 = 8.0;
const ACTS_TOP: f32 = 6.0;
const ACTS_GAP: f32 = 2.0;
const ACTS_GLYPH: f32 = 11.0;
/// `.marker{gap:8px;font:10.5px ui;padding:0 2px}`.
const MARKER_GAP: f32 = 8.0;
const MARKER_TEXT: f32 = 10.5;
const MARKER_PAD: f32 = 2.0;
/// `.prompt{gap:8px;padding:8px 12px;font:12px mono}`; cursor 7 × 14 blinking 1 s; `.ctx{font:500 10.5px ui;gap:8px}`.
const PROMPT_GAP: f32 = 8.0;
const PROMPT_PAD_Y: f32 = 8.0;
const PROMPT_PAD_X: f32 = 12.0;
const CURSOR_W: f32 = 7.0;
const CURSOR_H: f32 = 14.0;
const CURSOR_PERIOD: std::time::Duration = std::time::Duration::from_millis(1000);
const CTX_TEXT: f32 = 10.5;
const CTX_GAP: f32 = 8.0;

/// How a block ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockState {
    /// Exit 0.
    Done,
    /// Non-zero exit.
    Failed,
    /// Still running (the live block).
    Running,
}

/// One command block.
#[derive(Debug, Clone, PartialEq)]
pub struct TermBlock {
    /// Stable id.
    pub id: SharedString,
    /// The command line.
    pub command: SharedString,
    /// Outcome.
    pub state: BlockState,
    /// Output lines (ANSI escapes allowed).
    pub output: Vec<String>,
    /// Lines hidden behind the fold row.
    pub folded: usize,
    /// Which agent ran it, if any.
    pub agent: Option<Provider>,
    /// Duration label (`6.2 s`, `3.2 s · exit 1`, `12 s`).
    pub duration: SharedString,
    /// Older blocks dim to .72.
    pub old: bool,
}

impl TermBlock {
    /// A finished block.
    pub fn new(id: impl Into<SharedString>, command: impl Into<SharedString>, state: BlockState, duration: impl Into<SharedString>) -> Self {
        Self { id: id.into(), command: command.into(), state, output: Vec::new(), folded: 0, agent: None, duration: duration.into(), old: false }
    }

    /// Output lines.
    pub fn output(mut self, lines: Vec<String>) -> Self {
        self.output = lines;
        self
    }

    /// Folded line count.
    pub fn folded(mut self, folded: usize) -> Self {
        self.folded = folded;
        self
    }

    /// The agent tag.
    pub fn agent(mut self, provider: Provider) -> Self {
        self.agent = Some(provider);
        self
    }

    /// Dims the block.
    pub fn old(mut self) -> Self {
        self.old = true;
        self
    }
}

/// What a block asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalAction {
    /// Copy a block's output.
    Copy(SharedString),
    /// Ask the agent about a block.
    Ask(SharedString),
    /// Unfold a block.
    Unfold(SharedString),
}

type ActionHandler = std::rc::Rc<dyn Fn(TerminalAction, &mut Window, &mut App)>;

/// The prompt row's state.
#[derive(Debug, Clone, PartialEq)]
pub struct TermPrompt {
    /// Text typed so far.
    pub text: SharedString,
    /// Context tags at the right (branch, cwd).
    pub context: Vec<SharedString>,
}

/// The block terminal. Build with [`block_terminal`].
#[derive(IntoElement)]
pub struct BlockTerminal {
    id: ElementId,
    marker: Option<SharedString>,
    blocks: Vec<TermBlock>,
    prompt: Option<TermPrompt>,
    on_action: Option<ActionHandler>,
}

/// A terminal pane over `blocks`.
pub fn block_terminal(id: impl Into<ElementId>, blocks: Vec<TermBlock>) -> BlockTerminal {
    BlockTerminal { id: id.into(), marker: None, blocks, prompt: None, on_action: None }
}

impl BlockTerminal {
    /// The restored-scrollback marker at the top.
    pub fn marker(mut self, text: impl Into<SharedString>) -> Self {
        self.marker = Some(text.into());
        self
    }

    /// The prompt row at the bottom.
    pub fn prompt(mut self, prompt: TermPrompt) -> Self {
        self.prompt = Some(prompt);
        self
    }

    /// Action handler.
    pub fn on_action(mut self, f: impl Fn(TerminalAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }
}

/// One output line as mono text runs on the terminal palette (ink-2 base).
fn output_line(p: &Palette, line: &str, base: gpui::Hsla) -> impl IntoElement {
    let spans = parse_ansi(line);
    let (text, mut runs) = ansi_runs(&spans, p, scale::FONT_MONO);
    // Plain runs take the block's base ink rather than term-fg.
    for (run, span) in runs.iter_mut().zip(spans.iter()) {
        if span.color.is_none() && !span.dim {
            run.color = base;
        }
    }
    let text = if text.is_empty() { " ".to_string() } else { text };
    let runs = if runs.iter().map(|r| r.len).sum::<usize>() == text.len() && !runs.is_empty() {
        runs
    } else {
        vec![gpui::TextRun { len: text.len(), font: gpui::font(scale::FONT_MONO), color: base, background_color: None, underline: None, strikethrough: None }]
    };
    div().w_full().overflow_hidden().whitespace_nowrap().child(StyledText::new(text).with_runs(runs))
}

fn block(p: &Palette, root: &ElementId, b: &TermBlock, on_action: Option<ActionHandler>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let id: ElementId = (root.clone(), SharedString::from(format!("blk-{}", b.id))).into();
    let (state, flags) = interaction_flags(id.clone(), window, cx);
    let live = b.state == BlockState::Running;
    let failed = b.state == BlockState::Failed;
    let lifted = live || flags.hovered;
    let bg = tween((id.clone(), "bg"), if lifted { p.surface_1 } else { p.term_bg.alpha(0.0) }, Tween::FAST, window, cx);
    let border = tween((id.clone(), "border"), if lifted { p.line } else { p.line.alpha(0.0) }, Tween::FAST, window, cx);
    let acts_opacity = tween((id.clone(), "acts"), if flags.hovered { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);

    let dot: gpui::AnyElement = match b.state {
        BlockState::Done => div().size(px(EXIT_DOT)).rounded_full().bg(p.success).into_any_element(),
        BlockState::Failed => div().size(px(EXIT_DOT)).rounded_full().bg(p.danger).into_any_element(),
        BlockState::Running => div()
            .size(px(EXIT_DOT))
            .rounded_full()
            .bg(p.accent)
            .relative()
            .child(div().absolute().left(px(-EXIT_RING)).top(px(-EXIT_RING)).size(px(EXIT_DOT + 2.0 * EXIT_RING)).rounded_full().bg(p.accent_soft).border_1().border_color(p.accent_soft))
            .into_any_element(),
    };
    let mut cmd = h_flex()
        .w_full()
        .gap(px(CMD_GAP))
        .py(px(CMD_PAD_Y))
        .px(px(CMD_PAD_X))
        .rounded(px(scale::R_MD))
        .text_color(p.term_fg)
        .when(failed, |d| d.bg(p.danger_soft))
        .child(div().flex_none().size(px(EXIT_DOT)).flex().items_center().justify_center().child(dot))
        .child(div().semibold().text_color(p.accent).child("$"))
        .child(div().min_w(px(0.0)).truncate().child(b.command.clone()));
    if let Some(agent) = b.agent {
        cmd = cmd.child(div().flex_1()).child(
            h_flex()
                .flex_none()
                .gap(px(WHO_GAP))
                .pl(px(WHO_PAD))
                .ui(WHO_TEXT)
                .line_height(relative(1.0))
                .medium()
                .text_color(p.ink_3)
                .child(provider_mark(agent).size(px(WHO_MARK)))
                .child(agent_label(agent)),
        );
    } else {
        cmd = cmd.child(div().flex_1());
    }
    let mut dur = h_flex().flex_none().gap(px(scale::SP_2)).mono(DUR_TEXT).line_height(relative(1.0)).text_color(p.term_dim);
    if live {
        dur = dur.child(spinner((id.clone(), "spinner")).size(px(LIVE_SPINNER)));
    }
    dur = dur.child(b.duration.clone());
    cmd = cmd.child(dur);

    let base_ink = if failed { p.ink } else { p.ink_2 };
    let mut out = v_flex().w_full().pt(px(OUT_PAD_TOP)).pr(px(OUT_PAD_RIGHT)).pb(px(OUT_PAD_BOTTOM)).pl(px(OUT_PAD_LEFT)).text_color(base_ink);
    for line in &b.output {
        out = out.child(output_line(p, line, base_ink));
    }

    let mut el = v_flex()
        .id(id.clone())
        .relative()
        .w_full()
        .rounded(px(scale::R_MD))
        .border_1()
        .border_color(border)
        .bg(bg)
        .when(b.old && !flags.hovered, |d| d.opacity(OLD_OPACITY))
        .track_interaction(&state)
        .child(cmd);
    if !b.output.is_empty() {
        el = el.child(out);
    }
    if b.folded > 0 {
        let unfold = on_action.clone();
        let key = b.id.clone();
        el = el.child(
            h_flex()
                .id((id.clone(), "fold"))
                .gap(px(FOLD_GAP))
                .pr(px(OUT_PAD_RIGHT))
                .pb(px(FOLD_PAD_BOTTOM))
                .pl(px(OUT_PAD_LEFT))
                .ui(scale::FS_11)
                .line_height(relative(1.0))
                .text_color(p.ink_3)
                .cursor_pointer()
                .on_click(move |_, w, cx| {
                    if let Some(h) = &unfold {
                        h(TerminalAction::Unfold(key.clone()), w, cx)
                    }
                })
                .child(icon(IconName::Chevron).size(px(FOLD_GLYPH)))
                .child(format!("{} lines folded", b.folded)),
        );
    }
    // Hover actions: copy, ask the agent.
    let mut acts = h_flex().absolute().right(px(ACTS_RIGHT)).top(px(ACTS_TOP)).gap(px(ACTS_GAP)).opacity(acts_opacity);
    for (name, glyph, make) in [
        ("copy", IconName::Copy, TerminalAction::Copy as fn(SharedString) -> TerminalAction),
        ("ask", IconName::Sparkle, TerminalAction::Ask as fn(SharedString) -> TerminalAction),
    ] {
        let mut btn = icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(ACTS_GLYPH));
        if let Some(h) = on_action.clone() {
            let key = b.id.clone();
            btn = btn.on_click(move |_, w, cx| h(make(key.clone()), w, cx));
        }
        acts = acts.child(btn);
    }
    if acts_opacity <= 0.001 {
        acts = acts.invisible();
    }
    el.child(acts)
}

fn agent_label(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
        Provider::Grok => "grok",
        Provider::Gemini => "gemini",
        Provider::Pi => "pi",
        Provider::Cursor => "cursor",
    }
}

impl RenderOnce for BlockTerminal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut scroll = v_flex()
            .id((id.clone(), "scroll"))
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .overflow_hidden()
            .py(px(SCROLL_PAD_Y))
            .px(px(SCROLL_PAD_X))
            .gap(px(BLOCK_GAP))
            .mono(TERM_TEXT)
            .line_height(relative(TERM_LH))
            .text_color(p.term_fg);
        if let Some(marker) = self.marker {
            let rule = || div().flex_1().h(px(1.0)).bg(p.line);
            scroll = scroll.child(
                h_flex().w_full().gap(px(MARKER_GAP)).px(px(MARKER_PAD)).ui(MARKER_TEXT).text_color(p.ink_3).whitespace_nowrap().child(rule()).child(marker).child(rule()),
            );
        }
        for b in &self.blocks {
            scroll = scroll.child(block(&p, &id, b, self.on_action.clone(), window, cx));
        }
        let mut pane = v_flex().id(id.clone()).size_full().bg(p.term_bg).child(scroll);
        if let Some(prompt) = self.prompt {
            let on = looping((id.clone(), "cursor"), Loop::linear(CURSOR_PERIOD).resting(1.0), window, cx) < 0.5;
            let mut row = h_flex()
                .w_full()
                .flex_none()
                .gap(px(PROMPT_GAP))
                .py(px(PROMPT_PAD_Y))
                .px(px(PROMPT_PAD_X))
                .border_t_1()
                .border_color(p.line)
                .bg(p.surface_1)
                .mono(TERM_TEXT)
                .text_color(p.term_fg)
                .child(div().semibold().text_color(p.accent).child("$"))
                .child(div().child(prompt.text))
                .child(div().flex_none().w(px(CURSOR_W)).h(px(CURSOR_H)).bg(p.term_cursor).opacity(if on { 1.0 } else { 0.0 }));
            let mut ctx = h_flex().flex_none().gap(px(CTX_GAP)).ui(CTX_TEXT).line_height(relative(1.0)).medium().text_color(p.ink_3);
            for tag in prompt.context {
                ctx = ctx.child(tag);
            }
            row = row.child(div().flex_1()).child(ctx);
            pane = pane.child(row);
        }
        pane
    }
}
