//! Card 35: the approval card and its resolved states.
//!
//! Pending is the only marked state — a plain 1 px border in the warning
//! colour at 70 % alpha, no halo (`docs/04-design-rules.md`). The card carries
//! the command on a terminal ground, the agent's reason, a two-column
//! definition list and the one action-row pattern: key hints on the left, a
//! spacer, `Deny`, `Always allow <rule>`, and the primary `Allow once` on the
//! right. Once resolved the card goes quiet and single-line.

use std::rc::Rc;
use std::time::Duration;

use aui_motion::{presence, stagger_delay, EnterExit, PresenceStyle};
use aui_protocol::{ApprovalDecision, ApprovalScope, ApprovalState};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, font, prelude::*, px, relative, App, ElementId, Font, Hsla, InteractiveText, IntoElement, SharedString, StyledText, TextRun, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, kbd, pill, spinner, PillVariant};
use crate::icons::{icon, IconName};

/// `.ap .hd{padding:10px 12px;gap:10px}`.
const HEAD_PAD_Y: f32 = 10.0;
const HEAD_PAD_X: f32 = 12.0;
const HEAD_GAP: f32 = 10.0;
/// `.ap .ic{width:26px;height:26px;border-radius:7px}`.
const TILE: f32 = 26.0;
const TILE_RADIUS: f32 = 7.0;
/// The glyph inside the tile: `.i{width:14px;height:14px}`.
const TILE_GLYPH: f32 = 14.0;
/// `.ap .cmd{margin:0 12px 10px}` — the same inset as `.ap .det`.
const BODY_INSET_X: f32 = 12.0;
const BODY_INSET_BOTTOM: f32 = 10.0;
/// `.ap .cmd{padding:8px 10px;gap:8px;font:12px/1.5 var(--font-mono)}`.
const CMD_PAD_Y: f32 = 8.0;
const CMD_PAD_X: f32 = 10.0;
const CMD_GAP: f32 = 8.0;
const CMD_LINE_HEIGHT: f32 = 1.5;
/// `.ap .det dl{gap:3px 12px;margin:6px 0 0}`.
const DL_ROW_GAP: f32 = 3.0;
const DL_COL_GAP: f32 = 12.0;
const DL_MARGIN_TOP: f32 = 6.0;
/// `.ap .ft{gap:8px;padding:8px 12px}`.
const FOOT_PAD_Y: f32 = 8.0;
const FOOT_PAD_X: f32 = 12.0;
const FOOT_GAP: f32 = 8.0;
/// `.ap .ft .k{gap:8px;font-size:11px}` and `.ap .ft .k span{gap:4px}`.
const KEYS_GAP: f32 = 8.0;
const KEY_GAP: f32 = 4.0;
/// The mono rule on the `Always allow` button: `style="opacity:.7"`.
const RULE_OPACITY: f32 = 0.7;
/// `@keyframes in{from{transform:translateY(6px)}}` — the card enter.
const CARD_RISE: f32 = 6.0;
/// `@keyframes bin{from{transform:translateY(4px)}}` — the button enter.
const BUTTON_RISE: f32 = 4.0;
/// `.ap .ft .btn:nth-child(n){animation-delay:40ms × n}`.
const BUTTON_STAGGER: Duration = Duration::from_millis(40);
/// The three footer buttons.
const BUTTON_COUNT: usize = 3;

/// The pending question, which is the same for every command.
const PENDING_TITLE: &str = "Allow Claude Code to run this command?";

/// What the person chose, handed back to the caller.
type DecideHandler = Rc<dyn Fn(ApprovalDecision, &mut Window, &mut App)>;
/// The "manage rules" link on an auto-allowed card.
type ManageHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The approval card. Build with [`approval_card`].
#[derive(IntoElement)]
pub struct ApprovalCard {
    id: ElementId,
    tool: SharedString,
    command: SharedString,
    reason: SharedString,
    cwd: SharedString,
    capabilities: Vec<SharedString>,
    scope: ApprovalScope,
    state: ApprovalState,
    rule: Option<SharedString>,
    present: bool,
    timing: EnterExit,
    at_rest: bool,
    on_decide: Option<DecideHandler>,
    on_manage_rules: Option<ManageHandler>,
}

/// A permission request for `command` run through `tool`, in `state`.
///
/// The fields mirror [`aui_protocol::Block::Approval`]; the rest of them are
/// set with the builders below.
pub fn approval_card(id: impl Into<ElementId>, tool: impl Into<SharedString>, command: impl Into<SharedString>, state: ApprovalState) -> ApprovalCard {
    ApprovalCard {
        id: id.into(),
        tool: tool.into(),
        command: command.into(),
        reason: SharedString::default(),
        cwd: SharedString::default(),
        capabilities: Vec::new(),
        scope: ApprovalScope::ThisWorktree,
        state,
        rule: None,
        present: true,
        timing: EnterExit::DEFAULT,
        at_rest: false,
        on_decide: None,
        on_manage_rules: None,
    }
}

impl ApprovalCard {
    /// Why the agent wants it, in its own words.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = reason.into();
        self
    }

    /// The directory the command would run in.
    pub fn cwd(mut self, cwd: impl Into<SharedString>) -> Self {
        self.cwd = cwd.into();
        self
    }

    /// The capabilities being granted, e.g. `["modify files", "network"]`.
    pub fn capabilities(mut self, capabilities: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.capabilities = capabilities.into_iter().map(Into::into).collect();
        self
    }

    /// How far an "always allow" would reach.
    pub fn scope(mut self, scope: ApprovalScope) -> Self {
        self.scope = scope;
        self
    }

    /// The rule an "always allow" would remember, spelled out on the button.
    pub fn rule(mut self, rule: impl Into<SharedString>) -> Self {
        self.rule = Some(rule.into());
        self
    }

    /// Whether the card is on screen; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the card and its buttons are drawn at rest on the
    /// first frame (parity captures, restored transcripts).
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = Duration::ZERO;
        self.at_rest = true;
        self
    }

    /// The person pressed `Deny`, `Always allow` or `Allow once`.
    pub fn on_decide(mut self, f: impl Fn(ApprovalDecision, &mut Window, &mut App) + 'static) -> Self {
        self.on_decide = Some(Rc::new(f));
        self
    }

    /// The "manage rules" link on an auto-allowed card was clicked.
    pub fn on_manage_rules(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_manage_rules = Some(Rc::new(f));
        self
    }
}

/// One piece of a subtitle: the face it is set in.
#[derive(Clone, Copy, PartialEq)]
enum SubFace {
    /// The UI face in ink-2.
    Ui,
    /// The mono face in ink-2.
    Mono,
    /// The UI face in accent-ink: a link.
    Link,
}

/// Builds the `.sub` string and its runs. Mixed faces have to share one text
/// element so the line wraps between them the way the CSS does.
fn sub_runs(segments: &[(SharedString, SubFace)], p: &Palette) -> (String, Vec<TextRun>, Vec<std::ops::Range<usize>>) {
    let ui: Font = font(scale::FONT_UI);
    let mono: Font = font(scale::FONT_MONO);
    let mut text = String::new();
    let mut runs = Vec::new();
    let mut links = Vec::new();
    for (segment, face) in segments {
        let start = text.len();
        let (f, color) = match face {
            SubFace::Ui => (ui.clone(), p.ink_2),
            SubFace::Mono => (mono.clone(), p.ink_2),
            SubFace::Link => (ui.clone(), p.accent_ink),
        };
        text.push_str(segment);
        runs.push(TextRun { len: segment.len(), font: f, color, background_color: None, underline: None, strikethrough: None });
        if *face == SubFace::Link {
            links.push(start..text.len());
        }
    }
    (text, runs, links)
}

/// `capabilities` as the tail of the pending subtitle: `can modify files and network`.
fn capability_sentence(capabilities: &[SharedString]) -> String {
    match capabilities.len() {
        0 => String::new(),
        1 => format!("can {}", capabilities[0]),
        n => {
            let head = capabilities[..n - 1].iter().map(|c| c.as_ref()).collect::<Vec<_>>().join(", ");
            format!("can {head} and {}", capabilities[n - 1])
        }
    }
}

/// The definition-list label for a scope.
fn scope_label(scope: ApprovalScope) -> &'static str {
    match scope {
        ApprovalScope::ThisCommand => "this command",
        ApprovalScope::ThisWorktree => "this worktree",
        ApprovalScope::ThisSession => "this session",
        ApprovalScope::Global => "everywhere",
    }
}

/// `8_200` → `8.2 s`, the way the resolved subtitle spells a duration.
fn duration_label(ms: u64) -> String {
    if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        format!("{} m {:02} s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

/// The 26 px header tile: ground, ink and glyph depend on the state; `id` keys
/// the spinner of the approving state.
fn head_tile(ground: Hsla, ink: Hsla, glyph: Option<IconName>, id: ElementId) -> gpui::AnyElement {
    let tile = div().flex_none().size(px(TILE)).rounded(px(TILE_RADIUS)).bg(ground).flex().items_center().justify_center().text_color(ink);
    match glyph {
        Some(name) => tile.child(icon(name).size(px(TILE_GLYPH)).color(ink)).into_any_element(),
        None => tile.child(spinner(id)).into_any_element(),
    }
}

impl RenderOnce for ApprovalCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let pending = self.state == ApprovalState::Pending;

        // The card enters with a 6 px rise; the border is the only marking.
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let style = PresenceStyle::fade_rise(sample, CARD_RISE);
        let border = if pending { p.attention_border(p.warning) } else { p.line };

        // Header: tile, title + subtitle, status pill.
        let (title, tile, status, variant, segments): (SharedString, gpui::AnyElement, SharedString, PillVariant, Vec<(SharedString, SubFace)>) = match &self.state {
            ApprovalState::Pending => (
                PENDING_TITLE.into(),
                head_tile(p.warning_soft, p.warning, Some(IconName::Shield), (id.clone(), "tile").into()),
                "Waiting".into(),
                PillVariant::Warning,
                vec![
                    ("Runs in ".into(), SubFace::Ui),
                    (self.cwd.clone(), SubFace::Mono),
                    (format!(" · {}", capability_sentence(&self.capabilities)).into(), SubFace::Ui),
                ],
            ),
            ApprovalState::Approving => (
                "Approving…".into(),
                head_tile(p.surface_3, p.ink_3, None, (id.clone(), "tile").into()),
                "Running".into(),
                PillVariant::Accent,
                vec![(self.command.clone(), SubFace::Mono)],
            ),
            ApprovalState::AllowedOnce { exit_code, duration_ms } => (
                "Allowed once".into(),
                head_tile(p.success_soft, p.success, Some(IconName::Check), (id.clone(), "tile").into()),
                "Complete".into(),
                PillVariant::Success,
                vec![(format!("{} · exit {exit_code} · {}", self.command, duration_label(*duration_ms)).into(), SubFace::Mono)],
            ),
            ApprovalState::Denied => (
                "Denied".into(),
                head_tile(p.danger_soft, p.danger, Some(IconName::X), (id.clone(), "tile").into()),
                "Denied".into(),
                PillVariant::Danger,
                vec![
                    ("You denied ".into(), SubFace::Ui),
                    (self.command.clone(), SubFace::Mono),
                    (". Claude will try another approach.".into(), SubFace::Ui),
                ],
            ),
            ApprovalState::AutoAllowed { rule } => (
                "Auto-allowed".into(),
                head_tile(p.accent_soft, p.accent_ink, Some(IconName::Shield), (id.clone(), "tile").into()),
                "Rule".into(),
                PillVariant::Quiet,
                vec![
                    ("Remembered rule ".into(), SubFace::Ui),
                    (SharedString::from(rule.clone()), SubFace::Mono),
                    (" · ".into(), SubFace::Ui),
                    ("manage rules".into(), SubFace::Link),
                ],
            ),
        };

        let (sub_text, runs, links) = sub_runs(&segments, &p);
        let styled = StyledText::new(sub_text).with_runs(runs);
        let sub: gpui::AnyElement = if links.is_empty() {
            styled.into_any_element()
        } else {
            let on_manage = self.on_manage_rules.clone();
            InteractiveText::new((id.clone(), "sub"), styled)
                .on_click(links, move |_, w, cx| {
                    if let Some(h) = on_manage.clone() {
                        h(w, cx)
                    }
                })
                .into_any_element()
        };

        let header = h_flex()
            .w_full()
            .items_center()
            .gap(px(HEAD_GAP))
            .px(px(HEAD_PAD_X))
            .py(px(HEAD_PAD_Y))
            .child(tile)
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().w_full().ui(scale::FS_13).semibold().text_color(p.ink).child(title))
                    .child(div().w_full().ui(scale::FS_12).text_color(p.ink_2).child(sub)),
            )
            .child(pill(status).variant(variant));

        let mut card = v_flex()
            .relative()
            .top(style.offset_y)
            .opacity(style.opacity)
            .w_full()
            .overflow_hidden()
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(border)
            .bg(p.surface_1)
            .child(header);

        if !pending {
            return card;
        }

        // `$ command` on the terminal ground.
        card = card.child(
            h_flex()
                .mx(px(BODY_INSET_X))
                .mb(px(BODY_INSET_BOTTOM))
                .gap(px(CMD_GAP))
                .px(px(CMD_PAD_X))
                .py(px(CMD_PAD_Y))
                .rounded(px(scale::R_SM))
                .bg(p.term_bg)
                .font_family(scale::FONT_MONO)
                .text_px(scale::FS_12)
                .line_height(relative(CMD_LINE_HEIGHT))
                .text_color(p.term_fg)
                .child(div().flex_none().text_color(p.term_dim).child("$"))
                .child(div().flex_1().min_w(px(0.0)).child(self.command.clone())),
        );

        // The reason and the two-column definition list.
        let rows: [(SharedString, SharedString); 2] = [("Tool".into(), self.tool.clone()), ("Scope".into(), scope_label(self.scope).into())];
        let mut terms = v_flex().flex_none().gap(px(DL_ROW_GAP)).text_color(p.ink_3);
        let mut definitions = v_flex().flex_1().min_w(px(0.0)).gap(px(DL_ROW_GAP)).font_family(scale::FONT_MONO);
        for (term, definition) in rows {
            terms = terms.child(div().child(term));
            definitions = definitions.child(div().min_w(px(0.0)).truncate().child(definition));
        }
        card = card.child(
            v_flex()
                .mx(px(BODY_INSET_X))
                .mb(px(BODY_INSET_BOTTOM))
                .ui(scale::FS_12)
                .text_color(p.ink_2)
                .child(div().child(self.reason.clone()))
                .child(h_flex().items_start().mt(px(DL_MARGIN_TOP)).gap(px(DL_COL_GAP)).child(terms).child(definitions)),
        );

        // The action row: hints left, spacer, secondary, primary right.
        let keys = h_flex()
            .flex_1()
            .min_w(px(0.0))
            .items_center()
            .gap(px(KEYS_GAP))
            .ui(scale::FS_11)
            .text_color(p.ink_3)
            .child(h_flex().items_center().gap(px(KEY_GAP)).child(kbd("Y")).child("allow"))
            .child(h_flex().items_center().gap(px(KEY_GAP)).child(kbd("A")).child("always"))
            .child(h_flex().items_center().gap(px(KEY_GAP)).child(kbd("N")).child("deny"));

        let rule = self.rule.clone();
        let mut buttons: Vec<gpui::AnyElement> = Vec::new();
        for (index, decision) in [ApprovalDecision::Deny, ApprovalDecision::Always, ApprovalDecision::Once].into_iter().enumerate() {
            let key: SharedString = match decision {
                ApprovalDecision::Deny => "deny".into(),
                ApprovalDecision::Always => "always".into(),
                ApprovalDecision::Once => "once".into(),
            };
            let mut b = match decision {
                ApprovalDecision::Deny => button((id.clone(), key.clone()), "Deny").sm().danger(),
                ApprovalDecision::Always => {
                    let mut b = button((id.clone(), key.clone()), "Always allow").sm();
                    if let Some(rule) = &rule {
                        b = b.trailing(div().font_family(scale::FONT_MONO).opacity(RULE_OPACITY).child(rule.clone()));
                    }
                    b
                }
                ApprovalDecision::Once => button((id.clone(), key.clone()), "Allow once").sm().primary(),
            };
            if let Some(on_decide) = self.on_decide.clone() {
                b = b.on_click(move |_, w, cx| on_decide(decision, w, cx));
            }
            // Each button enters 40 ms after the one before it.
            let mut timing = EnterExit::DEFAULT.with_delay(stagger_delay(index, BUTTON_COUNT, BUTTON_STAGGER));
            if self.at_rest {
                timing.enter = Duration::ZERO;
                timing.delay = Duration::ZERO;
            }
            let sample = presence((id.clone(), key), self.present, timing, window, cx);
            let style = PresenceStyle::fade_rise(sample, BUTTON_RISE);
            buttons.push(div().relative().flex_none().top(style.offset_y).opacity(style.opacity).child(b).into_any_element());
        }

        card.child(
            h_flex()
                .w_full()
                .items_center()
                .gap(px(FOOT_GAP))
                .px(px(FOOT_PAD_X))
                .py(px(FOOT_PAD_Y))
                .border_t_1()
                .border_color(p.line)
                .bg(p.surface_2)
                .child(keys)
                .children(buttons),
        )
    }
}
