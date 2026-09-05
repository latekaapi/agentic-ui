//! Card 53: git and pull request. Two panels the workbench shows side by
//! side.
//!
//! [`GitChanges`] is the Changes panel: a header with the file count, one
//! 26 px row per changed file with a checkbox, its M / A / D letter and the
//! line delta, the commit area with the message drafted from the turn and its
//! action row, and the ahead / behind row with Push.
//!
//! [`PrForm`] is the Create pull request form: base branch, title,
//! description, the checks that ran on the last push, and the footer that
//! links the tracker issue and creates the PR.
//!
//! Both are stateless: they draw the data they are given and emit
//! [`GitAction`] / [`PrAction`] intents through one closure. Nothing here
//! talks to git.

use std::rc::Rc;

use aui_motion::{spring_phase, SpringKind};
use aui_protocol::{Check, ChangeKind, FileChange};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, chip, glyph_ok, spinner, tag, ButtonVariant};
use crate::icons::{icon, IconName};
use crate::util::{interaction_flags, TrackInteraction};

/// `.pan{border-radius:var(--r-lg)}`.
const PANEL_RADIUS: f32 = scale::R_LG;
/// `.pan .hd{gap:8px;padding:0 12px;font-weight:600}` at 36 px.
const HEAD_GAP: f32 = 8.0;
const PAD_X: f32 = 12.0;
/// `.f{height:26px;padding:0 12px;gap:8px;font-size:12px;color:var(--ink-2)}`.
const ROW_GAP: f32 = 8.0;
const ROW_TEXT: f32 = scale::FS_12;
/// `.f .cb{width:14px;height:14px;border-radius:4px;border:1.5px}` with a
/// 9 px stroke-3 check in white once it is on.
const CHECKBOX: f32 = 14.0;
const CHECKBOX_BORDER: f32 = 1.5;
const CHECKBOX_RADIUS: f32 = scale::R_XS;
const CHECK_MARK: f32 = 9.0;
/// The check pops in from 40 % on the swap spring, like the question card's
/// option mark (gpui has no transform, so the glyph size is animated).
const CHECK_MARK_FROM: f32 = 0.4;
/// `.f .st{width:12px;text-align:center;font:600 10px var(--font-mono)}`.
const STATUS_W: f32 = 12.0;
const STATUS_TEXT: f32 = 10.0;
/// `.commit{padding:10px 12px;border-top:1px solid var(--line)}`.
const COMMIT_PAD_Y: f32 = 10.0;
/// `.commit .ai{gap:6px;font-size:11px;color:var(--accent-ink);margin:6px 0 8px}`
/// with a 12 px sparkle.
const DRAFT_GAP: f32 = 6.0;
const DRAFT_TEXT: f32 = scale::FS_11;
const DRAFT_ICON: f32 = 12.0;
const DRAFT_MARGIN_TOP: f32 = 6.0;
const DRAFT_MARGIN_BOTTOM: f32 = 8.0;
/// `.commit textarea{padding:8px;font:12.5px/1.45;rows=3}` on surface-2 with a
/// 1 px line border: three lines plus the padding and the border.
const MESSAGE_PAD: f32 = 8.0;
const MESSAGE_TEXT: f32 = 12.5;
const MESSAGE_LINE: f32 = 1.45;
const MESSAGE_ROWS: f32 = 3.0;
const MESSAGE_LINE_H: f32 = MESSAGE_TEXT * MESSAGE_LINE;
const MESSAGE_HEIGHT: f32 = MESSAGE_ROWS * MESSAGE_LINE_H + 2.0 * MESSAGE_PAD + 2.0;
/// `.row{margin-top:8px;gap:6px;justify-content:flex-end}` — plus the 6 px of
/// descender space CSS leaves under the inline-block textarea, which gpui's
/// block flow does not add.
const ACTIONS_MARGIN_TOP: f32 = 8.0;
const ACTIONS_BASELINE_GAP: f32 = 6.0;
const ACTIONS_GAP: f32 = 6.0;
/// `.ab{gap:10px;padding:8px 12px;border-top:1px solid var(--line);font:500 11px var(--font-mono);color:var(--ink-3)}`.
const AHEAD_GAP: f32 = 10.0;
const AHEAD_PAD_Y: f32 = 8.0;
const AHEAD_TEXT: f32 = scale::FS_11;
/// The branch glyph the card prints before the branch name.
const BRANCH_GLYPH: &str = "⎇";
/// `.form{padding:12px}`.
const FORM_PAD: f32 = 12.0;
/// `.lbl{font-size:11px;font-weight:600;color:var(--ink-3);margin:0 0 6px}`;
/// the label keeps the form's 1.5 line height (tracking is not expressible).
const LABEL_LINE: f32 = scale::LH_UI;
const LABEL_MARGIN_BOTTOM: f32 = 6.0;
/// `.in{height:30px;padding:0 10px;gap:8px;font-size:13px;margin-bottom:12px}`.
const FIELD_H: f32 = 30.0;
const FIELD_PAD_X: f32 = 10.0;
const FIELD_GAP: f32 = 8.0;
const FIELD_TEXT: f32 = scale::FS_13;
const FIELD_MARGIN_BOTTOM: f32 = 12.0;
/// `.in .mono{font-size:12px}` and the 11 px chevron of the base-branch select.
const FIELD_MONO_TEXT: f32 = scale::FS_12;
const FIELD_CHEVRON: f32 = 11.0;
/// `.in.ta{height:auto;padding:8px 10px;align-items:flex-start;font-size:12.5px;line-height:1.5}`.
const AREA_PAD_Y: f32 = 8.0;
const AREA_TEXT: f32 = 12.5;
const AREA_LINE: f32 = scale::LH_UI;
/// `.checks{gap:6px;margin-bottom:12px;font-size:12px}` with 8 px between a
/// row's glyph and its label.
const CHECKS_GAP: f32 = 6.0;
const CHECKS_TEXT: f32 = scale::FS_12;
const CHECK_ROW_GAP: f32 = 8.0;
/// `.ftr{gap:8px;padding-top:8px;border-top:1px solid var(--line)}`.
const FOOTER_GAP: f32 = 8.0;
const FOOTER_PAD_TOP: f32 = 8.0;

/// What the Changes panel asks the host to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitAction {
    /// Stage or unstage the file at this path.
    Toggle(SharedString),
    /// Draft the commit message again from the turn.
    Regenerate,
    /// Amend the previous commit instead of writing a new one.
    Amend,
    /// Commit the checked files with the message shown.
    Commit,
    /// Push the branch to its remote.
    Push,
}

/// What the pull-request form asks the host to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrAction {
    /// Open the base-branch picker.
    PickBase,
    /// Abandon the form.
    Cancel,
    /// Create the pull request.
    Create,
    /// Open the linked tracker issue.
    Linear,
}

type GitHandler = Rc<dyn Fn(GitAction, &mut Window, &mut App)>;
type PrHandler = Rc<dyn Fn(PrAction, &mut Window, &mut App)>;

/// The Changes panel. Build with [`git_changes`].
#[derive(IntoElement)]
pub struct GitChanges {
    id: ElementId,
    files: Vec<(FileChange, bool)>,
    message: SharedString,
    branch: SharedString,
    ahead: u32,
    behind: u32,
    on_action: Option<GitHandler>,
}

/// The Changes panel: every changed file with its staged flag, the drafted
/// commit `message`, and how far the branch is `ahead` of and `behind` its
/// remote.
pub fn git_changes(
    id: impl Into<ElementId>,
    files: Vec<(FileChange, bool)>,
    message: impl Into<SharedString>,
    ahead: u32,
    behind: u32,
) -> GitChanges {
    GitChanges { id: id.into(), files, message: message.into(), branch: SharedString::default(), ahead, behind, on_action: None }
}

impl GitChanges {
    /// The branch name shown in the ahead / behind row.
    pub fn branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.branch = branch.into();
        self
    }

    /// The panel's intents: toggling a file, redrafting, amending, committing
    /// and pushing.
    pub fn on_action(mut self, f: impl Fn(GitAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

/// `.f .st`: the M / A / D letter and the status colour it carries.
fn change_letter(kind: ChangeKind, p: &Palette) -> (&'static str, Hsla) {
    match kind {
        ChangeKind::Modified => ("M", p.warning),
        ChangeKind::Added => ("A", p.success),
        ChangeKind::Deleted => ("D", p.danger),
    }
}

/// `.c`: `+8 −3`, with the minus sign the design writes (not a hyphen), and
/// nothing at all for a side that did not change.
fn delta_text(file: &FileChange) -> String {
    let mut out = String::new();
    if file.added > 0 {
        out.push_str(&format!("+{}", file.added));
    }
    if file.removed > 0 {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&format!("−{}", file.removed));
    }
    out
}

/// `.f .cb`: the 14 px row checkbox, its check popping in on the swap spring.
fn row_checkbox(id: ElementId, on: bool, p: &Palette, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let phase = spring_phase(id, on, SpringKind::Swap, window, cx).clamp(0.0, 1.0);
    let mark = CHECK_MARK * (CHECK_MARK_FROM + (1.0 - CHECK_MARK_FROM) * phase);
    div()
        .flex_none()
        .size(px(CHECKBOX))
        .rounded(px(CHECKBOX_RADIUS))
        .border(px(CHECKBOX_BORDER))
        .border_color(if on { p.accent } else { p.line_strong })
        .when(on, |d| d.bg(p.accent))
        .flex()
        .items_center()
        .justify_center()
        .when(phase > 0.0, |d| d.child(icon(IconName::CheckBold).size(px(mark)).color(gpui::white().alpha(phase))))
}

/// The commit message, drawn as the card's static three-row field: paragraphs
/// wrap, a blank line stays blank, and the rest is clipped by the box.
fn message_lines(message: &SharedString) -> Vec<SharedString> {
    message.split('\n').map(|line| SharedString::from(line.to_string())).collect()
}

impl RenderOnce for GitChanges {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let checked = self.files.iter().filter(|(_, on)| *on).count();

        let head = h_flex()
            .w_full()
            .flex_none()
            .h(cx.aui().metrics.panel_header)
            .gap(px(HEAD_GAP))
            .px(px(PAD_X))
            .border_b_1()
            .border_color(p.line)
            .text_color(p.ink)
            .ui(scale::FS_13)
            .semibold()
            .child(icon(IconName::Git).color(p.ink))
            .child("Changes")
            .child(div().flex_1())
            .child(crate::data::pill(self.files.len().to_string()));

        let mut rows = v_flex().w_full().flex_none();
        for (file, on) in &self.files {
            let (letter, colour) = change_letter(file.change, &p);
            let path = SharedString::from(file.path.clone());
            let row_id: ElementId = (id.clone(), SharedString::from(format!("file:{path}"))).into();
            let (state, _) = interaction_flags(row_id.clone(), window, cx);
            let mut row = h_flex()
                .id(row_id.clone())
                .w_full()
                .flex_none()
                .h(cx.aui().metrics.row_sm)
                .gap(px(ROW_GAP))
                .px(px(PAD_X))
                .text_color(p.ink_2)
                .ui(ROW_TEXT)
                .cursor_pointer()
                .track_interaction(&state)
                .child(row_checkbox((row_id.clone(), "check").into(), *on, &p, window, cx))
                .child(
                    div()
                        .flex_none()
                        .w(px(STATUS_W))
                        .text_align(gpui::TextAlign::Center)
                        .font_family(scale::FONT_MONO)
                        .text_px(STATUS_TEXT)
                        .semibold()
                        .text_color(colour)
                        .child(letter),
                )
                .child(div().min_w(px(0.0)).truncate().child(path.clone()))
                .child(div().flex_1())
                .child(tag(delta_text(file)));
            if let Some(handler) = self.on_action.clone() {
                row = row.on_click(move |_, window, cx| handler(GitAction::Toggle(path.clone()), window, cx));
            }
            rows = rows.child(row);
        }

        let mut draft = h_flex()
            .w_full()
            .gap(px(DRAFT_GAP))
            .mt(px(DRAFT_MARGIN_TOP))
            .mb(px(DRAFT_MARGIN_BOTTOM))
            .text_color(p.accent_ink)
            .ui(DRAFT_TEXT)
            .child(icon(IconName::Sparkle).size(px(DRAFT_ICON)).color(p.accent_ink))
            .child(div().flex_none().child("Drafted from this turn ·"));
        let mut regenerate = div().id((id.clone(), "regenerate")).flex_none().underline().cursor_pointer().child("regenerate");
        if let Some(handler) = self.on_action.clone() {
            regenerate = regenerate.on_click(move |_, window, cx| handler(GitAction::Regenerate, window, cx));
        }
        draft = draft.child(regenerate);

        let mut message = v_flex()
            .w_full()
            .flex_none()
            .h(px(MESSAGE_HEIGHT))
            .p(px(MESSAGE_PAD))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .text_color(p.ink)
            .font_family(scale::FONT_UI)
            .text_px(MESSAGE_TEXT)
            .line_height(gpui::relative(MESSAGE_LINE))
            .overflow_hidden();
        for line in message_lines(&self.message) {
            message = message.child(if line.is_empty() {
                div().flex_none().h(px(MESSAGE_LINE_H))
            } else {
                div().w_full().flex_none().child(line)
            });
        }

        let action_button = |label: &'static str, variant: ButtonVariant, intent: GitAction, handler: &Option<GitHandler>| {
            let mut b = button((id.clone(), label), label).sm().variant(variant);
            if let Some(handler) = handler.clone() {
                b = b.on_click(move |_, window, cx| handler(intent.clone(), window, cx));
            }
            b
        };
        let commit_label = match checked {
            1 => "Commit 1 file".to_string(),
            n => format!("Commit {n} files"),
        };
        let mut commit_button = button((id.clone(), "commit"), commit_label).sm().primary();
        if let Some(handler) = self.on_action.clone() {
            commit_button = commit_button.on_click(move |_, window, cx| handler(GitAction::Commit, window, cx));
        }
        let actions = h_flex()
            .w_full()
            .justify_end()
            .gap(px(ACTIONS_GAP))
            .mt(px(ACTIONS_MARGIN_TOP + ACTIONS_BASELINE_GAP))
            .child(action_button("Amend", ButtonVariant::Ghost, GitAction::Amend, &self.on_action))
            .child(commit_button);

        let commit = v_flex()
            .w_full()
            .flex_none()
            .py(px(COMMIT_PAD_Y))
            .px(px(PAD_X))
            .border_t_1()
            .border_color(p.line)
            .child(draft)
            .child(message)
            .child(actions);

        let mut push = button((id.clone(), "push"), "Push").xs();
        if let Some(handler) = self.on_action.clone() {
            push = push.on_click(move |_, window, cx| handler(GitAction::Push, window, cx));
        }
        let ahead_behind = h_flex()
            .w_full()
            .flex_none()
            .gap(px(AHEAD_GAP))
            .py(px(AHEAD_PAD_Y))
            .px(px(PAD_X))
            .border_t_1()
            .border_color(p.line)
            .text_color(p.ink_3)
            .mono(AHEAD_TEXT)
            .medium()
            .child(div().child(format!("{BRANCH_GLYPH} {}", self.branch)))
            .child(div().flex_1())
            .child(div().flex_none().text_color(p.success).child(format!("↑{}", self.ahead)))
            .child(div().flex_none().child(format!("↓{}", self.behind)))
            .child(push);

        v_flex()
            .id(id)
            .size_full()
            .rounded(px(PANEL_RADIUS))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .child(head)
            .child(rows)
            .child(commit)
            .child(ahead_behind)
    }
}

/// One run of the PR description: prose in ink-2 or an identifier in the mono
/// face at the field's ink. Runs flow together and wrap; `\n\n` inside a
/// text run starts a new paragraph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrDescription {
    /// Prose. A `\n` starts a new line, an empty line stays blank.
    Text(SharedString),
    /// An identifier, in mono at the field's own ink.
    Mono(SharedString),
}

/// One row of the "checks on last push" list: a protocol [`Check`], whether it
/// is still running, and the trailing detail the card prints after a `·`.
#[derive(Clone, Debug, PartialEq)]
pub struct PrCheck {
    /// The check itself; `passed` decides the glyph once it has finished.
    pub check: Check,
    /// Still running: a spinner instead of the ok glyph.
    pub running: bool,
    /// What follows the name (`2 m 10 s`, `running`); empty for no detail.
    pub detail: SharedString,
}

/// A finished check.
pub fn pr_check(label: impl Into<String>, passed: bool) -> PrCheck {
    PrCheck { check: Check { label: label.into(), passed }, running: false, detail: SharedString::default() }
}

impl PrCheck {
    /// Marks the check as still running.
    pub fn running(mut self) -> Self {
        self.running = true;
        self
    }

    /// The detail after the name.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = detail.into();
        self
    }

    /// `ci / unit · 2 m 10 s`.
    fn line(&self) -> SharedString {
        if self.detail.is_empty() {
            SharedString::from(self.check.label.clone())
        } else {
            SharedString::from(format!("{} · {}", self.check.label, self.detail))
        }
    }
}

/// The Create pull request form. Build with [`pr_form`].
#[derive(IntoElement)]
pub struct PrForm {
    id: ElementId,
    base: SharedString,
    title: SharedString,
    description: Vec<PrDescription>,
    checks: Vec<PrCheck>,
    provider: SharedString,
    issue: Option<SharedString>,
    on_action: Option<PrHandler>,
}

/// The pull-request form: the `base` branch the PR targets, its `title`, the
/// `description` runs and the `checks` from the last push.
pub fn pr_form(
    id: impl Into<ElementId>,
    base: impl Into<SharedString>,
    title: impl Into<SharedString>,
    description: Vec<PrDescription>,
    checks: Vec<PrCheck>,
) -> PrForm {
    PrForm {
        id: id.into(),
        base: base.into(),
        title: title.into(),
        description,
        checks,
        provider: SharedString::default(),
        issue: None,
        on_action: None,
    }
}

impl PrForm {
    /// The forge chip in the header (`GitHub`).
    pub fn provider(mut self, provider: impl Into<SharedString>) -> Self {
        self.provider = provider.into();
        self
    }

    /// The tracker issue chip in the footer (`Linear ACM-412`).
    pub fn issue(mut self, issue: impl Into<SharedString>) -> Self {
        self.issue = Some(issue.into());
        self
    }

    /// The form's intents: picking the base branch, cancelling, creating, and
    /// opening the linked issue.
    pub fn on_action(mut self, f: impl Fn(PrAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}


/// `.lbl`: a caps label over a field.
fn field_label(label: &'static str, p: &Palette) -> impl IntoElement {
    div()
        .w_full()
        .flex_none()
        .mb(px(LABEL_MARGIN_BOTTOM))
        .text_role(TextRole::Caps)
        .line_height(gpui::relative(LABEL_LINE))
        .text_color(p.ink_3)
        .child(label.to_uppercase())
}

impl RenderOnce for PrForm {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        let mut head = h_flex()
            .w_full()
            .flex_none()
            .h(cx.aui().metrics.panel_header)
            .gap(px(HEAD_GAP))
            .px(px(PAD_X))
            .border_b_1()
            .border_color(p.line)
            .text_color(p.ink)
            .ui(scale::FS_13)
            .semibold()
            .child(icon(IconName::Git).color(p.ink))
            .child("Create pull request")
            .child(div().flex_1());
        if !self.provider.is_empty() {
            head = head.child(chip((id.clone(), "provider"), self.provider.clone()));
        }

        let field = |content: gpui::Div| {
            content
                .w_full()
                .flex_none()
                .mb(px(FIELD_MARGIN_BOTTOM))
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(p.line)
                .bg(p.surface_2)
        };

        let mut base = field(h_flex().h(px(FIELD_H)).px(px(FIELD_PAD_X)).gap(px(FIELD_GAP)))
            .id((id.clone(), "base"))
            .cursor_pointer()
            .text_color(p.ink)
            .ui(FIELD_TEXT)
            .child(div().flex_none().font_family(scale::FONT_MONO).text_px(FIELD_MONO_TEXT).child(self.base.clone()))
            .child(div().flex_1())
            .child(icon(IconName::ChevronDown).size(px(FIELD_CHEVRON)).color(p.ink_3));
        if let Some(handler) = self.on_action.clone() {
            base = base.on_click(move |_, window, cx| handler(PrAction::PickBase, window, cx));
        }

        let title = field(h_flex().h(px(FIELD_H)).px(px(FIELD_PAD_X)).gap(px(FIELD_GAP)))
            .text_color(p.ink)
            .ui(FIELD_TEXT)
            .child(div().min_w(px(0.0)).truncate().child(self.title.clone()));

        // Paragraphs of mixed runs: split text runs on blank lines, keep mono
        // runs inline in the mono face at the field's ink.
        let mono_font = gpui::font(scale::FONT_MONO);
        let ui_font = gpui::font(scale::FONT_UI);
        let mut paragraphs: Vec<(String, Vec<gpui::TextRun>)> = vec![(String::new(), Vec::new())];
        for run in &self.description {
            match run {
                PrDescription::Mono(text) => {
                    let (t, runs) = paragraphs.last_mut().expect("one paragraph");
                    t.push_str(text);
                    runs.push(gpui::TextRun { len: text.len(), font: mono_font.clone(), color: p.ink, background_color: None, underline: None, strikethrough: None });
                }
                PrDescription::Text(text) => {
                    for (n, part) in text.split("\n\n").enumerate() {
                        if n > 0 {
                            paragraphs.push((String::new(), Vec::new()));
                        }
                        if part.is_empty() {
                            continue;
                        }
                        let (t, runs) = paragraphs.last_mut().expect("one paragraph");
                        t.push_str(part);
                        runs.push(gpui::TextRun { len: part.len(), font: ui_font.clone(), color: p.ink_2, background_color: None, underline: None, strikethrough: None });
                    }
                }
            }
        }
        let mut description = field(v_flex().py(px(AREA_PAD_Y)).px(px(FIELD_PAD_X)))
            .text_color(p.ink_2)
            .font_family(scale::FONT_UI)
            .text_px(AREA_TEXT)
            .line_height(gpui::relative(AREA_LINE));
        let count = paragraphs.len();
        for (n, (text, runs)) in paragraphs.into_iter().enumerate() {
            if text.is_empty() {
                continue;
            }
            // A blank line between paragraphs, as the `<br><br>` in the card.
            description = description.child(div().w_full().when(n + 1 < count, |d| d.mb(px(AREA_TEXT * AREA_LINE))).child(gpui::StyledText::new(text).with_runs(runs)));
        }

        let mut checks = v_flex()
            .w_full()
            .flex_none()
            .gap(px(CHECKS_GAP))
            .mb(px(FIELD_MARGIN_BOTTOM))
            .text_color(p.ink)
            .ui(CHECKS_TEXT);
        for (index, check) in self.checks.iter().enumerate() {
            let lead = if check.running {
                spinner((id.clone(), SharedString::from(format!("check{index}")))).into_any_element()
            } else if check.check.passed {
                glyph_ok().into_any_element()
            } else {
                crate::data::glyph_err().into_any_element()
            };
            checks = checks.child(h_flex().w_full().gap(px(CHECK_ROW_GAP)).child(lead).child(div().min_w(px(0.0)).truncate().child(check.line())));
        }

        let footer_button = |label: &'static str, variant: ButtonVariant, intent: PrAction, handler: &Option<PrHandler>| {
            let mut b = button((id.clone(), label), label).sm().variant(variant);
            if let Some(handler) = handler.clone() {
                b = b.on_click(move |_, window, cx| handler(intent, window, cx));
            }
            b
        };
        let mut footer = h_flex().w_full().flex_none().gap(px(FOOTER_GAP)).pt(px(FOOTER_PAD_TOP)).border_t_1().border_color(p.line);
        if let Some(issue) = self.issue.clone() {
            let mut issue_chip = chip((id.clone(), "issue"), issue).icon(IconName::Link);
            if let Some(handler) = self.on_action.clone() {
                issue_chip = issue_chip.on_click(move |_, window, cx| handler(PrAction::Linear, window, cx));
            }
            footer = footer.child(issue_chip);
        }
        footer = footer
            .child(div().flex_1())
            .child(footer_button("Cancel", ButtonVariant::Ghost, PrAction::Cancel, &self.on_action))
            .child(footer_button("Create PR", ButtonVariant::Primary, PrAction::Create, &self.on_action));

        v_flex()
            .id(id)
            .size_full()
            .rounded(px(PANEL_RADIUS))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .child(head)
            .child(
                v_flex()
                    .w_full()
                    .flex_none()
                    .p(px(FORM_PAD))
                    .child(field_label("Base branch", &p))
                    .child(base)
                    .child(field_label("Title", &p))
                    .child(title)
                    .child(field_label("Description", &p))
                    .child(description)
                    .child(field_label("Checks on last push", &p))
                    .child(checks)
                    .child(footer),
            )
    }
}
