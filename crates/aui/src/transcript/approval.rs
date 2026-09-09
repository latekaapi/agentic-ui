//! Card 35: the approval card and its resolved states.
//!
//! Pending is the only marked state — a plain 1 px border in the warning
//! colour at 70 % alpha, no halo (`docs/04-design-rules.md`). The card carries
//! the command on a terminal ground, the agent's reason, a two-column
//! definition list and the one action-row pattern: key hints on the left, a
//! spacer, the secondary choices, and the primary one on the right. Once
//! resolved the card goes quiet and single-line.
//!
//! # Server-minted choices, stages and feedback
//!
//! A provider may mint its own choice list rather than the built-in
//! allow/always/deny triad, may stage one subject (a shell pipeline is decided
//! one stage at a time), and may offer a choice that takes free-text feedback
//! with the decision. All three are optional data in:
//!
//! * [`ApprovalCard::choices`] replaces the triad with the server's buttons, in
//!   the server's order, and reports the chosen [`aui_protocol::ApprovalChoice::id`]
//!   through [`ApprovalCard::on_choose`]. Digits 1–9 pick the n-th.
//! * [`ApprovalCard::stages`] draws the stage strip under the command.
//! * [`ApprovalCard::feedback_open`] plus [`ApprovalCard::feedback_slot`] reveal
//!   a field the **host** owns — the card never holds text, exactly as the
//!   composer's editor is the host's.
//! * [`ApprovalCard::resolved_by`] draws the quiet, never-actionable line a
//!   policy or judge resolution collapses to.

use std::rc::Rc;
use std::time::Duration;

use aui_motion::{presence, stagger_delay, EnterExit, PresenceStyle};
use aui_protocol::{ApprovalBadges, ApprovalChoice, ApprovalDecision, ApprovalScope, ApprovalStage, ApprovalState, ResolvedBy};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, font, prelude::*, px, relative, AnyElement, App, ElementId, Font, Hsla, InteractiveText, IntoElement, SharedString, StyledText, TextRun, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, kbd, pill, spinner, Button, PillVariant};
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
/// The stage strip: the same inset as the command, its own row rhythm.
const STAGE_GAP: f32 = 6.0;
const STAGE_ROW_GAP: f32 = 8.0;
const STAGE_TEXT: f32 = 12.0;
const STAGE_GLYPH: f32 = 12.0;
/// The badge pills in the header sit on the title's own line.
const BADGE_GAP: f32 = 6.0;
/// The host's feedback field, inset like the command block, with its own row.
const FEEDBACK_GAP: f32 = 8.0;
/// The quoted feedback line on a denied card.
const QUOTE_TEXT: f32 = 12.0;
/// `@keyframes in{from{transform:translateY(6px)}}` — the card enter.
const CARD_RISE: f32 = 6.0;
/// `@keyframes bin{from{transform:translateY(4px)}}` — the button enter.
const BUTTON_RISE: f32 = 4.0;
/// `.ap .ft .btn:nth-child(n){animation-delay:40ms × n}`.
const BUTTON_STAGGER: Duration = Duration::from_millis(40);
/// The three footer buttons.
const BUTTON_COUNT: usize = 3;

/// The pending question when the host names none. Deliberately provider-free:
/// the host says whose command it is.
const PENDING_TITLE: &str = "Allow this command?";

/// What the person chose, handed back to the caller.
type DecideHandler = Rc<dyn Fn(ApprovalDecision, &mut Window, &mut App)>;
/// A server-minted choice was picked: its id and any feedback typed for it.
type ChooseHandler = Rc<dyn Fn(String, Option<String>, &mut Window, &mut App)>;
/// Open the feedback field for a choice, or close it.
type FeedbackToggleHandler = Rc<dyn Fn(Option<String>, &mut Window, &mut App)>;
/// The "manage rules" link on an auto-allowed card.
type ManageHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The approval card. Build with [`approval_card`].
#[derive(IntoElement)]
pub struct ApprovalCard {
    id: ElementId,
    title: SharedString,
    tool: SharedString,
    command: SharedString,
    reason: SharedString,
    cwd: SharedString,
    capabilities: Vec<SharedString>,
    scope: ApprovalScope,
    state: ApprovalState,
    rule: Option<SharedString>,
    choices: Vec<ApprovalChoice>,
    stages: Vec<ApprovalStage>,
    current_stage: Option<usize>,
    badges: ApprovalBadges,
    resolved_by: Option<ResolvedBy>,
    feedback: Option<SharedString>,
    feedback_open: Option<String>,
    feedback_slot: Option<AnyElement>,
    feedback_text: String,
    present: bool,
    timing: EnterExit,
    at_rest: bool,
    on_decide: Option<DecideHandler>,
    on_choose: Option<ChooseHandler>,
    on_feedback_toggle: Option<FeedbackToggleHandler>,
    on_manage_rules: Option<ManageHandler>,
}

/// A permission request for `command` run through `tool`, in `state`.
///
/// The fields mirror [`aui_protocol::Block::Approval`]; the rest of them are
/// set with the builders below.
pub fn approval_card(id: impl Into<ElementId>, tool: impl Into<SharedString>, command: impl Into<SharedString>, state: ApprovalState) -> ApprovalCard {
    ApprovalCard {
        id: id.into(),
        title: PENDING_TITLE.into(),
        tool: tool.into(),
        command: command.into(),
        reason: SharedString::default(),
        cwd: SharedString::default(),
        capabilities: Vec::new(),
        scope: ApprovalScope::ThisWorktree,
        state,
        rule: None,
        choices: Vec::new(),
        stages: Vec::new(),
        current_stage: None,
        badges: ApprovalBadges::default(),
        resolved_by: None,
        feedback: None,
        feedback_open: None,
        feedback_slot: None,
        feedback_text: String::new(),
        present: true,
        timing: EnterExit::DEFAULT,
        at_rest: false,
        on_decide: None,
        on_choose: None,
        on_feedback_toggle: None,
        on_manage_rules: None,
    }
}

impl ApprovalCard {
    /// The pending question, e.g. `"Allow Muse to run this command?"`.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The server's own choice list, in the server's order.
    ///
    /// A non-empty list replaces the built-in triad: the buttons are exactly
    /// these, and [`ApprovalCard::on_choose`] reports which one was pressed.
    pub fn choices(mut self, choices: Vec<ApprovalChoice>) -> Self {
        self.choices = choices;
        self
    }

    /// The subject's stages and which one is awaiting a decision.
    ///
    /// A single-stage subject draws no strip; pass an empty vector for one.
    pub fn stages(mut self, stages: Vec<ApprovalStage>, current: Option<usize>) -> Self {
        self.stages = stages;
        self.current_stage = current;
        self
    }

    /// The header badges: a protected write, an escalation from the judge.
    pub fn badges(mut self, badges: ApprovalBadges) -> Self {
        self.badges = badges;
        self
    }

    /// Who settled the request, which is what makes a policy or judge
    /// resolution read as one and never actionable.
    pub fn resolved_by(mut self, resolved_by: Option<ResolvedBy>) -> Self {
        self.resolved_by = resolved_by;
        self
    }

    /// The feedback that went out with a refusal, quoted on the resolved card.
    pub fn feedback(mut self, feedback: impl Into<SharedString>) -> Self {
        self.feedback = Some(feedback.into());
        self
    }

    /// Which choice's feedback field is open, by [`ApprovalChoice::id`].
    ///
    /// A choice whose [`ApprovalChoice::accepts_feedback`] is set does not fire
    /// [`ApprovalCard::on_choose`] on its first press: the host opens the field
    /// instead, and the confirming press carries the text.
    pub fn feedback_open(mut self, choice_id: Option<String>) -> Self {
        self.feedback_open = choice_id;
        self
    }

    /// The feedback field itself — the host's element, because the card never
    /// owns text. The same division as the composer's editor.
    pub fn feedback_slot(mut self, slot: impl IntoElement) -> Self {
        self.feedback_slot = Some(slot.into_any_element());
        self
    }

    /// What the open field currently holds. Data in, so that "Send" can hand it
    /// straight back through [`ApprovalCard::on_choose`] without the card ever
    /// keeping a character of it.
    pub fn feedback_text(mut self, text: impl Into<String>) -> Self {
        self.feedback_text = text.into();
        self
    }

    /// Open the feedback field for a choice (`Some(id)`) or close it (`None`).
    ///
    /// A choice that accepts feedback opens the field on its first press rather
    /// than deciding; "Cancel" closes it again.
    pub fn on_feedback_toggle(mut self, f: impl Fn(Option<String>, &mut Window, &mut App) + 'static) -> Self {
        self.on_feedback_toggle = Some(Rc::new(f));
        self
    }

    /// A server-minted choice was pressed: its id, and the feedback typed for
    /// it when the open field was confirmed.
    pub fn on_choose(mut self, f: impl Fn(String, Option<String>, &mut Window, &mut App) + 'static) -> Self {
        self.on_choose = Some(Rc::new(f));
        self
    }

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

impl ApprovalCard {
    /// The action row for a server-minted choice list.
    ///
    /// The buttons are the server's, in the server's order: the row wraps rather
    /// than truncating, because a label like "Always allow in this workspace:
    /// echo ..." is the rule the person is being asked to install and a
    /// half-shown rule is worse than a taller row.
    fn choices_row(self, id: ElementId, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let count = self.choices.len();
        let hints = h_flex()
            .flex_none()
            .items_center()
            .gap(px(KEYS_GAP))
            .ui(scale::FS_11)
            .text_color(p.ink_3)
            .child(h_flex().items_center().gap(px(KEY_GAP)).child(kbd("1")).child(if count > 1 { "–" } else { "" }))
            .child(h_flex().items_center().gap(px(KEY_GAP)).children((count > 1).then(|| kbd(count.min(9).to_string()))).child("to choose"));

        let mut buttons: Vec<AnyElement> = Vec::new();
        for (index, choice) in self.choices.iter().enumerate() {
            let key: SharedString = SharedString::from(format!("choice-{index}"));
            let mut b: Button = button((id.clone(), key.clone()), choice.label.clone()).sm();
            b = match choice.decision {
                ApprovalDecision::Once | ApprovalDecision::ApprovedForSession => b.primary(),
                ApprovalDecision::Deny | ApprovalDecision::DeniedPolicyAmendment | ApprovalDecision::Abort => b.danger(),
                _ => b,
            };
            // The rule a policy amendment would install, spelled out in the mono
            // face — unless the label already is that rule, which is the usual
            // case and does not want saying twice.
            if let Some(rule) = choice.rule_preview.clone().filter(|r| *r != choice.label) {
                b = b.trailing(div().font_family(scale::FONT_MONO).opacity(RULE_OPACITY).child(SharedString::from(rule)));
            }
            let choice_id = choice.id.clone();
            let opens_field = choice.accepts_feedback && self.feedback_open.as_deref() != Some(choice.id.as_str());
            if opens_field {
                if let Some(on_toggle) = self.on_feedback_toggle.clone() {
                    b = b.on_click(move |_, w, cx| on_toggle(Some(choice_id.clone()), w, cx));
                }
            } else if let Some(on_choose) = self.on_choose.clone() {
                let feedback = self.feedback_text.clone();
                let carries = choice.accepts_feedback;
                b = b.on_click(move |_, w, cx| on_choose(choice_id.clone(), carries.then(|| feedback.clone()), w, cx));
            }
            // Each button enters after the one before it, as the triad does.
            let mut timing = EnterExit::DEFAULT.with_delay(stagger_delay(index, count.max(1), BUTTON_STAGGER));
            if self.at_rest {
                timing.enter = Duration::ZERO;
                timing.delay = Duration::ZERO;
            }
            let sample = presence((id.clone(), key), self.present, timing, window, cx);
            let style = PresenceStyle::fade_rise(sample, BUTTON_RISE);
            buttons.push(div().relative().flex_none().top(style.offset_y).opacity(style.opacity).child(b).into_any_element());
        }

        h_flex()
            .w_full()
            .flex_wrap()
            .items_center()
            .gap(px(FOOT_GAP))
            .px(px(FOOT_PAD_X))
            .py(px(FOOT_PAD_Y))
            .border_t_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .child(hints)
            .child(div().flex_1().min_w(px(0.0)))
            .children(buttons)
    }
}

/// The quiet headline a resolution nobody was asked for collapses to.
fn resolution_title(by: ResolvedBy, allowed: bool) -> &'static str {
    match (by, allowed) {
        (ResolvedBy::Policy, true) => "Allowed by policy",
        (ResolvedBy::Policy, false) => "Denied by policy",
        (ResolvedBy::LlmJudge, true) => "Allowed by the approval judge",
        (ResolvedBy::LlmJudge, false) => "Denied by the approval judge",
        (ResolvedBy::User, true) => "Allowed",
        (ResolvedBy::User, false) => "Denied",
    }
}

/// The one-line subtitle under it: the rule, for a policy resolution that
/// names one; the judge names none.
fn resolution_segments(by: ResolvedBy, rule: Option<SharedString>) -> Vec<(SharedString, SubFace)> {
    match (by, rule) {
        (ResolvedBy::Policy, Some(rule)) => vec![("Rule ".into(), SubFace::Ui), (rule, SubFace::Mono)],
        (ResolvedBy::LlmJudge, _) => vec![("The approval judge decided this without asking.".into(), SubFace::Ui)],
        _ => Vec::new(),
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
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
                self.title.clone(),
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
                    (". The agent will try another approach.".into(), SubFace::Ui),
                ],
            ),
            ApprovalState::AutoDenied { rule } => (
                "Auto-denied".into(),
                head_tile(p.danger_soft, p.danger, Some(IconName::X), (id.clone(), "tile").into()),
                "Denied".into(),
                PillVariant::Danger,
                vec![
                    ("Refused by rule ".into(), SubFace::Ui),
                    (SharedString::from(rule.clone()), SubFace::Mono),
                    (" · ".into(), SubFace::Ui),
                    ("manage rules".into(), SubFace::Link),
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

        // A resolution nobody was asked for reads as one: policy and the
        // approval judge get the quiet single line and no actions at all.
        let (title, segments) = match (self.resolved_by, &self.state) {
            (Some(by @ (ResolvedBy::Policy | ResolvedBy::LlmJudge)), state) if !pending => {
                let allowed = matches!(state, ApprovalState::AutoAllowed { .. } | ApprovalState::AllowedOnce { .. } | ApprovalState::Approving);
                let rule = match state {
                    ApprovalState::AutoAllowed { rule } | ApprovalState::AutoDenied { rule } => Some(SharedString::from(rule.clone())),
                    _ => None,
                };
                (resolution_title(by, allowed).into(), resolution_segments(by, rule))
            }
            // The person's own refusal keeps its sentence, and quotes the
            // feedback that went out with it.
            (Some(ResolvedBy::User), ApprovalState::Denied) => ("Denied".into(), segments),
            _ => (title, segments),
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

        // The badges sit beside the title, warning-tinted, and only when set.
        let mut title_line = h_flex()
            .w_full()
            .items_center()
            .gap(px(BADGE_GAP))
            .child(div().flex_none().ui(scale::FS_13).semibold().text_color(p.ink).child(title));
        if self.badges.protected_write {
            title_line = title_line.child(pill("Protected write").variant(PillVariant::Warning));
        }
        if self.badges.judge_escalated {
            title_line = title_line.child(pill("Judge escalated").variant(PillVariant::Warning));
        }

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
                    .child(title_line)
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
            // A refusal the person typed a reason into quotes it, so the card
            // still says what the agent was told.
            if let Some(feedback) = self.feedback.filter(|_| matches!(self.state, ApprovalState::Denied)) {
                card = card.child(
                    div()
                        .mx(px(BODY_INSET_X))
                        .mb(px(BODY_INSET_BOTTOM))
                        .pl(px(CMD_PAD_X))
                        .border_l_1()
                        .border_color(p.line_strong)
                        .ui(QUOTE_TEXT)
                        .text_color(p.ink_2)
                        .child(feedback),
                );
            }
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

        // The stage strip: a pipeline is decided one stage at a time, so the
        // card says which stage this decision is for. One stage needs no strip.
        if self.stages.len() > 1 {
            let total = self.stages.first().map(|s| s.total).unwrap_or(self.stages.len() as u32);
            let position = self.current_stage.and_then(|index| self.stages.get(index)).map(|s| s.position).unwrap_or(1);
            let mut strip = v_flex()
                .mx(px(BODY_INSET_X))
                .mb(px(BODY_INSET_BOTTOM))
                .gap(px(STAGE_GAP))
                .child(div().flex_none().child(pill(format!("Stage {position}/{total}")).variant(PillVariant::Quiet)));
            for (index, stage) in self.stages.iter().enumerate() {
                let current = self.current_stage == Some(index);
                let ink = if stage.resolved {
                    p.ink_3
                } else if current {
                    p.ink
                } else {
                    p.ink_2
                };
                let mut row = h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(STAGE_ROW_GAP))
                    .mono(STAGE_TEXT)
                    .text_color(ink)
                    .child(
                        div()
                            .flex_none()
                            .size(px(STAGE_GLYPH))
                            .children(stage.resolved.then(|| icon(IconName::Check).size(px(STAGE_GLYPH)).color(p.success))),
                    )
                    .child(div().flex_1().min_w(px(0.0)).child(stage.argv.join(" ")));
                if !stage.argv_complete {
                    row = row.child(div().flex_none().ui(scale::FS_11).text_color(p.warning).child("(partial parse)"));
                }
                strip = strip.child(row);
            }
            card = card.child(strip);
        }

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

        // The feedback field the host owns, revealed by a choice that takes one.
        let open_choice = self.feedback_open.clone().filter(|id| self.choices.iter().any(|c| &c.id == id && c.accepts_feedback));
        if let (Some(choice_id), Some(slot)) = (open_choice.clone(), self.feedback_slot.take()) {
            let mut send = button((id.clone(), "feedback-send"), "Send").sm().primary();
            let mut cancel = button((id.clone(), "feedback-cancel"), "Cancel").sm().ghost();
            if let Some(on_choose) = self.on_choose.clone() {
                let confirm = choice_id.clone();
                let text = self.feedback_text.clone();
                send = send.on_click(move |_, w, cx| on_choose(confirm.clone(), Some(text.clone()), w, cx));
            }
            if let Some(on_toggle) = self.on_feedback_toggle.clone() {
                cancel = cancel.on_click(move |_, w, cx| on_toggle(None, w, cx));
            }
            card = card.child(
                v_flex()
                    .mx(px(BODY_INSET_X))
                    .mb(px(BODY_INSET_BOTTOM))
                    .gap(px(FEEDBACK_GAP))
                    .child(div().w_full().child(slot))
                    .child(h_flex().w_full().items_center().gap(px(FOOT_GAP)).child(div().flex_1()).child(cancel).child(send)),
            );
        }

        // The action row: hints left, spacer, then the choices in server order
        // with the primary one on the right.
        if !self.choices.is_empty() {
            return card.child(self.choices_row(id, window, cx));
        }

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
            // The card still renders the built-in triad; the wider MSP
            // decision set is carried by the protocol but not yet by this card.
            let key: SharedString = match decision {
                ApprovalDecision::Deny => "deny".into(),
                ApprovalDecision::Always => "always".into(),
                _ => "once".into(),
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
                _ => button((id.clone(), key.clone()), "Allow once").sm().primary(),
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
