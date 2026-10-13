//! The approval card (D4, docs/41 §2.6) and its resolved rows.
//!
//! Pending is the only marked state: Raised, radius 8, the edge's ring in
//! the warning colour at 70 %. A 26 px tile with the terminal glyph, the
//! title and its subtitle; the command on the Well; a footer of key hints,
//! a spacer, **Deny** and **Always allow** as text controls and **Allow** as
//! the one primary. Once answered the card goes quiet: a surface-less row
//! with the status glyph, the verdict, the command in mono and the result
//! faint (docs/41 §2.4).
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

use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_protocol::{ApprovalBadges, ApprovalBodyKind, ApprovalChoice, ApprovalDecision, ApprovalStage, ApprovalState, ResolvedBy};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, relative, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{pill, spinner, PillVariant};
use crate::icons::{icon, IconName};
use crate::transcript::controls::{control, key_hints, ControlKind};
use crate::transcript::quiet_row::{quiet_status_row, status_check, status_cross};

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
/// The command on the Well: radius 6, 8 / 10.
const CMD_PAD_Y: f32 = 8.0;
const CMD_PAD_X: f32 = 10.0;
const CMD_LINE_HEIGHT: f32 = 1.5;
const WELL_RADIUS: f32 = 6.0;
/// The footer: 8 / 12, gap 8.
const FOOT_PAD_Y: f32 = 8.0;
const FOOT_PAD_X: f32 = 12.0;
const FOOT_GAP: f32 = 8.0;
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
/// `@keyframes in{from{transform:translateY(6px)}}` — the card enter.
const CARD_RISE: f32 = 6.0;

/// The pending question when the host names none. Deliberately provider-free:
/// the host says whose command it is.
const PENDING_TITLE: &str = "Allow this command?";

/// What the person chose, handed back to the caller.
type DecideHandler = Rc<dyn Fn(ApprovalDecision, &mut Window, &mut App)>;
/// A server-minted choice was picked: its id and any feedback typed for it.
type ChooseHandler = Rc<dyn Fn(String, Option<String>, &mut Window, &mut App)>;
/// Open the feedback field for a choice, or close it.
type FeedbackToggleHandler = Rc<dyn Fn(Option<String>, &mut Window, &mut App)>;

/// The approval card. Build with [`approval_card`].
#[derive(IntoElement)]
pub struct ApprovalCard {
    id: ElementId,
    title: SharedString,
    tool: SharedString,
    command: SharedString,
    body_kind: ApprovalBodyKind,
    subtitle: SharedString,
    reason: SharedString,
    cwd: SharedString,
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
        body_kind: ApprovalBodyKind::Command,
        subtitle: SharedString::default(),
        reason: SharedString::default(),
        cwd: SharedString::default(),
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

    /// How the subject body renders: `$ ` prompt for commands, plain path
    /// for file writes/edits, plain otherwise. Defaults to
    /// [`ApprovalBodyKind::Command`], which is what every block written
    /// before the kind existed already is; mirrors
    /// [`aui_protocol::Block::Approval::body_kind`].
    pub fn body_kind(mut self, kind: ApprovalBodyKind) -> Self {
        self.body_kind = kind;
        self
    }

    /// The line under the title: who wants what, where ("Muse wants to run
    /// a shell command in acme-web"). Without one the card says where it
    /// runs, from [`ApprovalCard::cwd`].
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// Why the agent wants it, in its own words: the subtitle when the host
    /// names none.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = reason.into();
        self
    }

    /// The directory the command would run in.
    pub fn cwd(mut self, cwd: impl Into<SharedString>) -> Self {
        self.cwd = cwd.into();
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
}

impl ApprovalCard {
    /// The footer for a server-minted choice list: the server's choices in
    /// the server's order, as text controls, the approving one primary. The
    /// row wraps rather than truncating, because a label like "Always allow
    /// in this workspace: echo ..." is the rule being installed.
    fn choices_row(self, id: ElementId, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let count = self.choices.len();
        let hints = if count > 1 { format!("1–{} choose · ⎋ deny", count.min(9)) } else { "1 choose · ⎋ deny".to_owned() };
        let primary = self.choices.iter().position(|c| matches!(c.decision, ApprovalDecision::Once | ApprovalDecision::ApprovedForSession));
        let mut buttons: Vec<AnyElement> = Vec::new();
        for (index, choice) in self.choices.iter().enumerate() {
            let kind = if Some(index) == primary { ControlKind::Primary } else { ControlKind::Text };
            let mut b = control((id.clone(), SharedString::from(format!("choice-{index}"))), choice.label.clone(), kind, &p);
            // The rule a policy amendment would install, spelled out in the
            // mono face — unless the label already is that rule.
            if let Some(rule) = choice.rule_preview.clone().filter(|r| *r != choice.label) {
                b = b.child(div().font_family(scale::FONT_MONO).opacity(RULE_OPACITY).child(SharedString::from(rule)));
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
            buttons.push(b.into_any_element());
        }
        footer(&p).flex_wrap().child(key_hints(hints, &p)).child(div().flex_1().min_w(px(0.0))).children(buttons)
    }
}

/// A card footer: 8 / 12 over a 7 % rule.
fn footer(p: &Palette) -> gpui::Div {
    h_flex().w_full().items_center().gap(px(FOOT_GAP)).px(px(FOOT_PAD_X)).py(px(FOOT_PAD_Y)).border_t_1().border_color(p.line)
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

/// `8_200` → `8.2 s`, the way the resolved row spells a duration.
fn duration_label(ms: u64) -> String {
    if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        format!("{} m {:02} s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

impl RenderOnce for ApprovalCard {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        if self.state != ApprovalState::Pending {
            return self.resolved_row(cx).into_any_element();
        }

        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let style = PresenceStyle::fade_rise(sample, CARD_RISE);

        // Header: the tile, the title (with any badges) over its subtitle.
        let mut title_line = h_flex()
            .w_full()
            .items_center()
            .gap(px(BADGE_GAP))
            .child(div().flex_none().ui(scale::FS_13).semibold().text_color(p.ink).child(self.title.clone()));
        if self.badges.protected_write {
            title_line = title_line.child(pill("Protected write").variant(PillVariant::Warning));
        }
        if self.badges.judge_escalated {
            title_line = title_line.child(pill("Judge escalated").variant(PillVariant::Warning));
        }
        let subtitle: SharedString = if !self.subtitle.is_empty() {
            self.subtitle.clone()
        } else if !self.reason.is_empty() {
            self.reason.clone()
        } else if !self.cwd.is_empty() {
            format!("Runs in {}", self.cwd).into()
        } else {
            self.tool.clone()
        };
        let header = h_flex()
            .w_full()
            .items_center()
            .gap(px(HEAD_GAP))
            .px(px(HEAD_PAD_X))
            .py(px(HEAD_PAD_Y))
            .child(
                div()
                    .flex_none()
                    .size(px(TILE))
                    .rounded(px(TILE_RADIUS))
                    .bg(crate::util::raised_lit(&p, p.surface_2))
                    .shadow(p.edge())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(IconName::Terminal).size(px(TILE_GLYPH)).color(p.ink_2)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(title_line)
                    .when(!subtitle.is_empty(), |d| d.child(div().w_full().ui(scale::FS_12).text_color(p.ink_2).child(subtitle))),
            );

        let mut card = crate::util::raised_block(v_flex().relative().top(style.offset_y).opacity(style.opacity).w_full().rounded(px(crate::transcript::card::CARD_RADIUS)), &p, p.surface_1)
            .shadow(crate::transcript::card::attention_edge(p.warning))
            .overflow_hidden()
            .child(header);

        // The subject on the Well: a command or a file path in mono, anything
        // else in the UI face.
        let subject = div()
            .mx(px(BODY_INSET_X))
            .mb(px(BODY_INSET_BOTTOM))
            .px(px(CMD_PAD_X))
            .py(px(CMD_PAD_Y))
            .rounded(px(WELL_RADIUS))
            .bg(p.well)
            .line_height(relative(CMD_LINE_HEIGHT))
            .text_color(p.ink)
            .child(self.command.clone());
        card = card.child(match self.body_kind {
            ApprovalBodyKind::Command | ApprovalBodyKind::FileWrite => subject.font_family(scale::FONT_MONO).text_px(scale::FS_12),
            ApprovalBodyKind::Other => subject.ui(scale::FS_12),
        });

        // The stage strip: a pipeline is decided one stage at a time, so the
        // card says which stage this decision is for. One stage needs no strip.
        if self.stages.len() > 1 {
            let total = self.stages.first().map(|s| s.total).unwrap_or(self.stages.len() as u32);
            let position = self.current_stage.and_then(|index| self.stages.get(index)).map(|s| s.position).unwrap_or(1);
            let mut strip = v_flex()
                .mx(px(BODY_INSET_X))
                .mb(px(BODY_INSET_BOTTOM))
                .gap(px(STAGE_GAP))
                .child(div().flex_none().ui(scale::FS_12).text_color(p.ink_2).child(format!("Stage {position} of {total}")));
            for (index, stage) in self.stages.iter().enumerate() {
                let current = self.current_stage == Some(index);
                let ink = if stage.resolved {
                    p.ink_4
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
                    .child(div().flex_none().size(px(STAGE_GLYPH)).children(stage.resolved.then(|| icon(IconName::Check).size(px(STAGE_GLYPH)).color(p.success))))
                    .child(div().flex_1().min_w(px(0.0)).child(stage.argv.join(" ")));
                if !stage.argv_complete {
                    row = row.child(div().flex_none().ui(scale::FS_11).text_color(p.warning).child("(partial parse)"));
                }
                strip = strip.child(row);
            }
            card = card.child(strip);
        }

        // The feedback field the host owns, revealed by a choice that takes one.
        let open_choice = self.feedback_open.clone().filter(|id| self.choices.iter().any(|c| &c.id == id && c.accepts_feedback));
        if let (Some(choice_id), Some(slot)) = (open_choice.clone(), self.feedback_slot.take()) {
            let mut send = control((id.clone(), "feedback-send"), "Send", ControlKind::Primary, &p);
            let mut cancel = control((id.clone(), "feedback-cancel"), "Cancel", ControlKind::Text, &p);
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

        if !self.choices.is_empty() {
            return card.child(self.choices_row(id, cx)).into_any_element();
        }

        // The built-in footer: hints, spacer, Deny · Always allow as text
        // controls, Allow the one primary.
        let mut deny = control((id.clone(), "deny"), "Deny", ControlKind::Text, &p);
        let mut always = control((id.clone(), "always"), "Always allow", ControlKind::Text, &p);
        if let Some(rule) = self.rule.clone() {
            always = always.child(div().font_family(scale::FONT_MONO).opacity(RULE_OPACITY).child(rule));
        }
        let mut allow = control((id.clone(), "once"), "Allow", ControlKind::Primary, &p).aria_label("Allow once");
        crate::data::record_ax_label("Allow once");
        if let Some(on_decide) = self.on_decide.clone() {
            let (d, a, o) = (on_decide.clone(), on_decide.clone(), on_decide);
            deny = deny.on_click(move |_, w, cx| d(ApprovalDecision::Deny, w, cx));
            always = always.on_click(move |_, w, cx| a(ApprovalDecision::Always, w, cx));
            allow = allow.on_click(move |_, w, cx| o(ApprovalDecision::Once, w, cx));
        }
        card.child(
            footer(&p)
                .child(key_hints("⏎ allow · ⎋ deny", &p))
                .child(div().flex_1().min_w(px(0.0)))
                .child(deny)
                .child(always)
                .child(allow),
        )
        .into_any_element()
    }
}

impl ApprovalCard {
    /// The quiet row an answered request leaves (docs/41 §2.4): the status
    /// glyph, the verdict, the command in mono, the result faint. Not
    /// clickable.
    fn resolved_row(self, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let policy = matches!(self.resolved_by, Some(ResolvedBy::Policy | ResolvedBy::LlmJudge));
        let (glyph, verdict, tail): (AnyElement, SharedString, Option<String>) = match &self.state {
            ApprovalState::Approving => (spinner((id.clone(), "spinner")).size(px(12.0)).fluid(cx).into_any_element(), "Approving…".into(), None),
            ApprovalState::AllowedOnce { exit_code, duration_ms } => (
                status_check(p.success).into_any_element(),
                if policy { resolution_title(self.resolved_by.unwrap_or(ResolvedBy::User), true).into() } else { "Allowed once".into() },
                Some(format!("· exit {exit_code} · {}", duration_label(*duration_ms))),
            ),
            ApprovalState::Denied => (
                status_cross(p.danger).into_any_element(),
                if policy { resolution_title(self.resolved_by.unwrap_or(ResolvedBy::User), false).into() } else { "Denied".into() },
                self.feedback.as_ref().map(|f| format!("· “{f}”")),
            ),
            ApprovalState::AutoDenied { rule } => (status_cross(p.danger).into_any_element(), "Denied by rule".into(), Some(format!("· {rule}"))),
            ApprovalState::AutoAllowed { rule } => (status_check(p.success).into_any_element(), "Allowed by rule".into(), Some(format!("· {rule}"))),
            ApprovalState::Pending => unreachable!("a pending card is not a row"),
        };
        let mut row = quiet_status_row(id, glyph)
            .part(div().flex_none().child(verdict))
            .part(div().min_w(px(0.0)).truncate().mono(scale::FS_12).child(self.command.clone()));
        if let Some(tail) = tail {
            row = row.part(div().flex_none().mono(scale::FS_12).text_color(p.ink_4).child(tail));
        }
        row
    }
}
