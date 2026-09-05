//! Card 38: the summary card — the end-of-work rollup. `.sm`: an ok glyph and
//! the headline over the changed files, the checks that ran, and one action
//! row (hint, spacer, Create PR, Commit…, Review diff).

use std::rc::Rc;

use aui_protocol::{Check, ChangeKind, FileChange};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, glyph_ok, ButtonVariant};
use crate::icons::{icon, IconName};

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
const FILE_STATUS_SIZE: f32 = 10.0;
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
const ACTIONS_PAD_Y: f32 = 10.0;

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
        ChangeKind::Modified => ("M", p.warning),
        ChangeKind::Added => ("A", p.success),
        ChangeKind::Deleted => ("D", p.danger),
    }
}

impl RenderOnce for SummaryCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        let head = h_flex()
            .w_full()
            .gap(px(HEAD_GAP))
            .py(px(HEAD_PAD_Y))
            .px(px(PAD_X))
            .semibold()
            .text_color(p.ink)
            .child(glyph_ok())
            .child(div().min_w(px(0.0)).truncate().child(self.title.clone()))
            .child(div().flex_1())
            .child(div().flex_none().text_role(TextRole::MonoSmall).text_color(p.ink_3).child(self.meta.clone()));

        let mut files = v_flex().w_full().px(px(PAD_X)).pb(px(FILES_PAD_BOTTOM)).gap(px(FILES_GAP)).mono(scale::FS_12).text_color(p.ink);
        for file in &self.files {
            let (letter, colour) = change_letter(file.change, &p);
            let mut delta = h_flex().flex_none().gap(px(DELTA_GAP)).text_px(FILE_DELTA_SIZE);
            if file.added > 0 {
                delta = delta.child(div().text_color(p.success).child(format!("+{}", file.added)));
            }
            if file.removed > 0 {
                // The design writes removals with a minus sign, not a hyphen.
                delta = delta.child(div().text_color(p.danger).child(format!("−{}", file.removed)));
            }
            files = files.child(
                h_flex()
                    .w_full()
                    .h(px(FILE_ROW_H))
                    .gap(px(FILE_ROW_GAP))
                    .child(div().flex_none().w(px(FILE_STATUS_W)).text_align(gpui::TextAlign::Center).semibold().text_px(FILE_STATUS_SIZE).text_color(colour).child(letter))
                    .child(div().min_w(px(0.0)).truncate().child(file.path.clone()))
                    .child(div().flex_1())
                    .child(delta),
            );
        }

        let mut checks = h_flex().w_full().px(px(PAD_X)).pb(px(CHECKS_PAD_BOTTOM)).gap(px(CHECKS_GAP)).ui(scale::FS_12).text_color(p.ink_2);
        for check in &self.checks {
            let (glyph, colour) = if check.passed { (IconName::Check, p.success) } else { (IconName::X, p.danger) };
            checks = checks.child(
                h_flex()
                    .flex_none()
                    .gap(px(CHECK_LABEL_GAP))
                    .child(icon(glyph).size(px(CHECK_GLYPH)).color(colour))
                    .child(SharedString::from(check.label.clone())),
            );
        }

        let hint = match self.files.len() {
            1 => "1 file changed".to_string(),
            n => format!("{n} files changed"),
        };
        let action = |label: &'static str, variant: ButtonVariant, intent: SummaryAction, handler: &Option<ActionHandler>| {
            let mut b = button((id.clone(), label), label).sm().variant(variant);
            if let Some(handler) = handler.clone() {
                b = b.on_click(move |_, window, cx| handler(intent, window, cx));
            }
            b
        };
        let actions = h_flex()
            .w_full()
            .gap(px(ACTIONS_GAP))
            .py(px(ACTIONS_PAD_Y))
            .px(px(PAD_X))
            .border_t_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .child(div().flex_none().text_role(TextRole::Meta).text_color(p.ink_3).whitespace_nowrap().child(hint))
            .child(div().flex_1())
            .child(action("Create PR", ButtonVariant::Ghost, SummaryAction::CreatePr, &self.on_action))
            .child(action("Commit…", ButtonVariant::Secondary, SummaryAction::Commit, &self.on_action))
            .child(action("Review diff", ButtonVariant::Primary, SummaryAction::ReviewDiff, &self.on_action));

        v_flex()
            .id(id)
            .w_full()
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .child(head)
            .when(!self.files.is_empty(), |d| d.child(files))
            .when(!self.checks.is_empty(), |d| d.child(checks))
            .child(actions)
    }
}
