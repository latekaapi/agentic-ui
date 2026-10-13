//! Card 38: the summary card — the end-of-work rollup. `.sm`: an ok glyph and
//! the headline over the changed files, the checks that ran, and one action
//! row (hint, spacer, Create PR, Commit…, Review diff).

use std::rc::Rc;

use aui_protocol::{Check, ChangeKind, FileChange};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::icons::{icon, IconName};
use crate::transcript::controls::{control, ControlKind};
use crate::transcript::quiet_row::status_check;

/// `.sm .hd{gap:8px;padding:10px 12px;font-weight:600}` at the body size.
const HEAD_GAP: f32 = 8.0;
const HEAD_PAD_Y: f32 = 10.0;
const PAD_X: f32 = 12.0;
/// `.sm .files{padding:0 12px 8px;gap:2px;font-size:12px}`.
const FILES_GAP: f32 = 2.0;
const FILES_PAD_BOTTOM: f32 = 8.0;
/// `.sm .f{gap:8px;height:24px;font-family:mono}` with `.st{width:14px;font:600 10px}`
/// and `.d{font-size:11px}`.
const FILE_ROW_H: f32 = 24.0;
const FILE_ROW_GAP: f32 = 8.0;
const FILE_STATUS_W: f32 = 14.0;
const FILE_DELTA_SIZE: f32 = 11.0;
/// The space between the `+8` and the `−3` of a file's delta.
const DELTA_GAP: f32 = 4.0;
/// `.sm .checks{gap:12px;padding:0 12px 10px;font-size:12px}` with 12 px checks;
/// the glyph and its label are separated by one space in the markup.
const CHECKS_GAP: f32 = 12.0;
const CHECKS_PAD_BOTTOM: f32 = 10.0;
const CHECK_GLYPH: f32 = 12.0;
const CHECK_LABEL_GAP: f32 = 4.0;
/// `.actions{gap:8px;padding:10px 12px;border-top:1px solid line}` with an
/// 11 px ink-3 hint (`.actions .hint`).
const ACTIONS_GAP: f32 = 8.0;
const ACTIONS_PAD_Y: f32 = 8.0;

/// What the summary card's action row asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryAction {
    /// Open a pull request for the change.
    CreatePr,
    /// Commit the change (opens the commit sheet).
    Commit,
    /// Show the diff of every changed file.
    ReviewDiff,
}

type ActionHandler = Rc<dyn Fn(SummaryAction, &mut Window, &mut App)>;

/// The end-of-work summary card. Build with [`summary_card`].
#[derive(IntoElement)]
pub struct SummaryCard {
    id: ElementId,
    title: SharedString,
    meta: SharedString,
    files: Vec<FileChange>,
    checks: Vec<Check>,
    on_action: Option<ActionHandler>,
}

/// A summary headed by `title` (`Done · address validation tightened`) with
/// `meta` on the right (`4 m 12 s · $0.31`).
pub fn summary_card(id: impl Into<ElementId>, title: impl Into<SharedString>, meta: impl Into<SharedString>) -> SummaryCard {
    SummaryCard { id: id.into(), title: title.into(), meta: meta.into(), files: Vec::new(), checks: Vec::new(), on_action: None }
}

impl SummaryCard {
    /// The changed files, in display order.
    pub fn files(mut self, files: Vec<FileChange>) -> Self {
        self.files = files;
        self
    }

    /// The verifications that ran.
    pub fn checks(mut self, checks: Vec<Check>) -> Self {
        self.checks = checks;
        self
    }

    /// The action row's intent.
    pub fn on_action(mut self, f: impl Fn(SummaryAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

/// `.st`: the M / A / D letter and its colour (warning / success / danger).
fn change_letter(kind: ChangeKind, p: &Palette) -> (&'static str, gpui::Hsla) {
    match kind {
        ChangeKind::Modified => ("M", p.ink_2),
        ChangeKind::Added => ("A", p.success),
        ChangeKind::Deleted => ("D", p.ink_2),
    }
}

impl RenderOnce for SummaryCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // D4 (docs/41 §2.7): the ok glyph, the headline 600, `time · cost`
        // faint mono at the right.
        let head = h_flex()
            .w_full()
            .items_center()
            .gap(px(HEAD_GAP))
            .py(px(HEAD_PAD_Y))
            .px(px(PAD_X))
            .child(status_check(p.success))
            .child(div().min_w(px(0.0)).truncate().ui(scale::FS_13).semibold().text_color(p.ink).child(self.title.clone()))
            .child(div().flex_1())
            .child(div().flex_none().mono(scale::FS_11).text_color(p.ink_4).child(self.meta.clone()));

        // Files 2 apart in 24 px rows: the status letter (mono 11, 14 wide,
        // success for an addition, else muted), the path in mono, `+N −M`.
        let mut files = v_flex().w_full().px(px(PAD_X)).pb(px(FILES_PAD_BOTTOM)).gap(px(FILES_GAP)).mono(scale::FS_12).text_color(p.ink);
        for file in &self.files {
            let (letter, colour) = change_letter(file.change, &p);
            let delta = h_flex()
                .flex_none()
                .gap(px(DELTA_GAP))
                .text_px(FILE_DELTA_SIZE)
                .child(div().text_color(p.success).child(format!("+{}", file.added)))
                // The design writes removals with a minus sign, not a hyphen.
                .child(div().text_color(p.danger).child(format!("−{}", file.removed)));
            files = files.child(
                h_flex()
                    .w_full()
                    .h(px(FILE_ROW_H))
                    .gap(px(FILE_ROW_GAP))
                    .child(div().flex_none().w(px(FILE_STATUS_W)).mono(scale::FS_11).text_color(colour).child(letter))
                    .child(div().min_w(px(0.0)).truncate().child(file.path.clone()))
                    .child(div().flex_1())
                    .child(delta),
            );
        }

        // The checks as one muted caption line with success ticks, 12 apart.
        let mut checks = h_flex().w_full().px(px(PAD_X)).pb(px(CHECKS_PAD_BOTTOM)).gap(px(CHECKS_GAP)).ui(scale::FS_12).text_color(p.ink_2);
        for check in &self.checks {
            let (glyph, colour) = if check.passed { (IconName::Check, p.success) } else { (IconName::X, p.danger) };
            checks = checks.child(
                h_flex()
                    .flex_none()
                    .gap(px(CHECK_LABEL_GAP))
                    .child(icon(glyph).size(px(CHECK_GLYPH)).color(colour).lit(true))
                    .child(SharedString::from(check.label.clone())),
            );
        }

        let mut card = crate::util::raised_block(v_flex().id(id.clone()).w_full().rounded(px(crate::transcript::card::CARD_RADIUS)), &p, p.surface_1)
            .overflow_hidden()
            .child(head)
            .when(!self.files.is_empty(), |d| d.child(files))
            .when(!self.checks.is_empty(), |d| d.child(checks));
        // The next steps are peers — text controls, Create PR secondary, none
        // primary — and exist only when the host can act on them.
        if let Some(handler) = self.on_action.clone() {
            let action = |label: &'static str, kind: ControlKind, intent: SummaryAction| {
                let handler = handler.clone();
                control((id.clone(), label), label, kind, &p).on_click(move |_, window, cx| handler(intent, window, cx))
            };
            card = card.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(ACTIONS_GAP))
                    .py(px(ACTIONS_PAD_Y))
                    .px(px(PAD_X))
                    .border_t_1()
                    .border_color(p.line)
                    .child(div().flex_1())
                    .child(action("Review diff", ControlKind::Text, SummaryAction::ReviewDiff))
                    .child(action("Commit…", ControlKind::Text, SummaryAction::Commit))
                    .child(action("Create PR", ControlKind::Secondary, SummaryAction::CreatePr)),
            );
        }
        card
    }
}
