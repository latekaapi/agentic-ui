//! Card 52: the diff review pane. A framed grid with the scope segmented
//! control and the summary on top, a 230 px column of changed files and the
//! notes collected on them, the unified diff body with hunks, word-level
//! highlights and per-line notes, and the action row that sends the batch of
//! notes back to an agent.
//!
//! The segmented control ([`segmented`]) is exported on its own: cards 51 and
//! 53 use the same track.

use std::rc::Rc;

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_motion::{child_id, spring_phase, tween, SpringKind, Tween};
use aui_protocol::{ChangeKind, Diff, DiffKind, FileChange};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, relative, App, ElementId, Hsla, IntoElement, SharedString, StyledText, TextRun, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, icon_button, pill, ButtonSize, PillVariant};
use crate::util::{interaction_flags, TrackInteraction};

// ── the frame ────────────────────────────────────────────────────────────────
/// `.dv{height:610px;border:1px solid line-strong;border-radius:12px;grid-template-columns:230px 1fr;grid-template-rows:38px 1fr 44px}`.
const FRAME_H: f32 = 610.0;
const TOP_H: f32 = 38.0;
const FILES_W: f32 = 230.0;
const BOTTOM_H: f32 = 44.0;
/// `.top{gap:8px;padding:0 10px}`.
const TOP_GAP: f32 = 8.0;
const TOP_PAD_X: f32 = 10.0;
/// The summary line (`.subtle{font-size:12px}`).
const SUMMARY_TEXT: f32 = 12.0;

// ── the segmented control ───────────────────────────────────────────────────
/// `.seg{border-radius:r-sm;padding:2px;gap:2px}`; `.seg span{height:24px;padding:0 10px;border-radius:5px;font-size:12px}`.
const SEG_PAD: f32 = 2.0;
const SEG_GAP: f32 = 2.0;
const SEG_H: f32 = 24.0;
const SEG_PAD_X: f32 = 10.0;
const SEG_RADIUS: f32 = 5.0;
const SEG_TEXT: f32 = 12.0;

// ── the files column ────────────────────────────────────────────────────────
/// `.files{padding:8px;font-size:12px}` and `.files .caps{padding:4px 6px 6px}`.
const FILES_PAD: f32 = 8.0;
const FILES_TEXT: f32 = 12.0;
const CAPS_PAD_TOP: f32 = 4.0;
const CAPS_PAD_X: f32 = 6.0;
const CAPS_PAD_BOTTOM: f32 = 6.0;
/// The Notes header carries `margin-top:10px`.
const NOTES_CAPS_TOP: f32 = 10.0;
/// `.f{gap:7px;height:26px;padding:0 6px;border-radius:r-sm}`.
const FILE_ROW_GAP: f32 = 7.0;
const FILE_ROW_PAD_X: f32 = 6.0;
/// `.f .st{width:12px;font:600 10px mono}`.
const STATUS_W: f32 = 12.0;
const STATUS_TEXT: f32 = 10.0;
/// `.f .c{font:500 10.5px mono}`.
const COUNTS_TEXT: f32 = 10.5;
/// `.f .n{width:14px;height:14px;border-radius:4px;font:600 9px/14px mono}`.
const BADGE: f32 = 14.0;
const BADGE_TEXT: f32 = 9.0;
/// The note rows in the column: `height:auto;padding:4px 6px;font-size:11.5px`,
/// the badge pinned to the top with `margin-top:2px`.
const NOTE_ROW_PAD_Y: f32 = 4.0;
const NOTE_ROW_TEXT: f32 = 11.5;
const NOTE_ROW_BADGE_TOP: f32 = 2.0;

// ── the diff body ───────────────────────────────────────────────────────────
/// `.body{font:11.5px/1.65 mono}`.
const DIFF_TEXT: f32 = 11.5;
const DIFF_LH: f32 = 1.65;
/// `.fh{height:30px;gap:8px;padding:0 12px;font:12px ui}` with a 12 px file glyph.
const FILE_HEAD_H: f32 = 30.0;
const FILE_HEAD_GAP: f32 = 8.0;
const FILE_HEAD_PAD_X: f32 = 12.0;
const FILE_HEAD_TEXT: f32 = 12.0;
const FILE_HEAD_GLYPH: f32 = 12.0;
/// `.pill{height:16px;padding:0 6px}` in the file header.
const COUNT_PILL_H: f32 = 16.0;
const COUNT_PILL_PAD_X: f32 = 6.0;
/// `.hunk{padding:2px 12px;font-size:11px;gap:8px}` with the 12 px `.chev` turned 90°.
const HUNK_PAD_Y: f32 = 2.0;
const HUNK_PAD_X: f32 = 12.0;
const HUNK_TEXT: f32 = 11.0;
const HUNK_GAP: f32 = 8.0;
const HUNK_CHEV: f32 = 12.0;
/// `.ln{padding-right:8px}`; `.ln .gutter{width:36px;padding-right:8px}`; `.ln .g2{width:26px;padding-right:8px}`.
const LINE_PAD_R: f32 = 8.0;
const OLD_W: f32 = 36.0;
const NEW_W: f32 = 26.0;
const GUTTER_PAD: f32 = 8.0;
/// `.plus{left:4px;top:2px;width:16px;height:16px;border-radius:4px}` with a 10 px glyph.
const PLUS_LEFT: f32 = 4.0;
const PLUS_TOP: f32 = 2.0;
const PLUS: f32 = 16.0;
const PLUS_GLYPH: f32 = 10.0;

// ── the note under a line ───────────────────────────────────────────────────
/// `.note{margin:4px 12px 6px 70px;border:1px solid var(--line);border-radius:r-sm;padding:6px 8px;font:12px/1.4 ui}`.
const NOTE_MT: f32 = 4.0;
const NOTE_MR: f32 = 12.0;
const NOTE_MB: f32 = 6.0;
const NOTE_ML: f32 = 70.0;
const NOTE_PAD_Y: f32 = 6.0;
const NOTE_PAD_X: f32 = 8.0;
const NOTE_TEXT: f32 = 12.0;
const NOTE_LH: f32 = 1.4;
/// `.note .caps{margin-bottom:2px}`; the saved row is `margin-top:4px;gap:4px`,
/// the pending one `margin-top:6px;gap:6px`.
const NOTE_CAPS_GAP: f32 = 2.0;
const NOTE_SAVED_TOP: f32 = 4.0;
const NOTE_SAVED_GAP: f32 = 4.0;
const NOTE_PENDING_TOP: f32 = 6.0;
/// The pending note's `textarea` keeps the user agent's 2 px vertical padding.
const NOTE_TEXTAREA_PAD: f32 = 2.0;
const NOTE_PENDING_GAP: f32 = 6.0;

// ── the action row ──────────────────────────────────────────────────────────
/// `.bot{gap:8px;padding:0 12px}` with `.actions .hint{font-size:11px}`.
const BOTTOM_GAP: f32 = 8.0;
const BOTTOM_PAD_X: f32 = 12.0;
const HINT_TEXT: f32 = 11.0;
/// `.mark{width:12px;height:12px;margin:0 2px}` inside the primary button,
/// whose own `gap:6px` sits either side of it.
const SEND_MARK: f32 = 12.0;
const SEND_MARK_MARGIN: f32 = 2.0;
const SEND_MARK_GAP: f32 = 8.0;

/// Which change set the pane reviews.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffScope {
    /// Only what the current turn changed.
    ThisTurn,
    /// Everything on the branch against its base.
    Branch,
    /// The working tree's unstaged changes.
    Unstaged,
}

impl DiffScope {
    /// Every scope, in the order the control lists them.
    pub const ALL: &'static [DiffScope] = &[DiffScope::ThisTurn, DiffScope::Branch, DiffScope::Unstaged];

    /// The segment label.
    pub fn label(self) -> &'static str {
        match self {
            DiffScope::ThisTurn => "This turn",
            DiffScope::Branch => "Branch",
            DiffScope::Unstaged => "Unstaged",
        }
    }
}

/// How the diff is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffView {
    /// One column, additions and deletions interleaved.
    Unified,
    /// Old and new side by side.
    Split,
}

impl DiffView {
    /// Both views, in the order the control lists them.
    pub const ALL: &'static [DiffView] = &[DiffView::Unified, DiffView::Split];

    /// The segment label.
    pub fn label(self) -> &'static str {
        match self {
            DiffView::Unified => "Unified",
            DiffView::Split => "Split",
        }
    }
}

/// One row of the changed-files column.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewFile {
    /// The change itself: path, M/A/D and the line counts.
    pub change: FileChange,
    /// How many notes sit on this file; `0` hides the badge.
    pub notes: usize,
    /// Whether this is the file the diff body shows.
    pub selected: bool,
}

/// One note the reviewer left on a line.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewNote {
    /// Path of the file the note is on, matched against [`Diff::path`].
    pub file: SharedString,
    /// The new-side line number.
    pub line: u32,
    /// The note body.
    pub text: SharedString,
    /// Short form for the Notes column; falls back to [`ReviewNote::text`].
    pub summary: Option<SharedString>,
    /// Still being written: Cancel / Add note instead of Edit / Delete. The frame
    /// is the same either way.
    pub pending: bool,
}

/// A word-level `diff-add-strong` / `diff-del-strong` span inside one row,
/// addressed by hunk and row index with byte offsets into [`aui_protocol::DiffLine::text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffHighlight {
    /// Index of the hunk in [`Diff::hunks`].
    pub hunk: usize,
    /// Index of the row in that hunk.
    pub line: usize,
    /// First byte of the span.
    pub start: usize,
    /// One past the last byte of the span.
    pub end: usize,
}

/// What the pane asks the app to do.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffReviewAction {
    /// Review a different change set.
    Scope(DiffScope),
    /// Switch the diff layout.
    View(DiffView),
    /// Open the search field.
    Search,
    /// Collapse every hunk.
    CollapseAll,
    /// Show the diff of this path.
    SelectFile(SharedString),
    /// Open the shown file in the editor.
    OpenInEditor,
    /// Stage the shown file.
    Stage,
    /// Start a note on a new-side line.
    AddNote(u32),
    /// Reopen the note at this index for editing.
    EditNote(usize),
    /// Remove the note at this index.
    DeleteNote(usize),
    /// Keep the pending note at this index.
    SaveNote(usize),
    /// Discard the pending note at this index.
    CancelNote(usize),
    /// Drop every note.
    Clear,
    /// Send the batch of notes to the agent.
    Send,
}

type Handler = Rc<dyn Fn(DiffReviewAction, &mut Window, &mut App)>;

// ── segmented control ───────────────────────────────────────────────────────

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// `.seg`: a surface-2 track of 24 px segments; the active one is a surface-1
/// thumb at elevation 1. Build with [`segmented`].
#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    labels: Vec<SharedString>,
    active: usize,
    on_select: Option<SelectHandler>,
}

/// A segmented control over `labels` with `active` selected.
pub fn segmented(id: impl Into<ElementId>, labels: Vec<SharedString>, active: usize) -> Segmented {
    Segmented { id: id.into(), labels, active, on_select: None }
}

impl Segmented {
    /// Called with the index of the segment that was clicked.
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut track = h_flex().flex_none().p(px(SEG_PAD)).gap(px(SEG_GAP)).rounded(px(scale::R_SM)).bg(p.surface_2);
        for (i, label) in self.labels.into_iter().enumerate() {
            let on = i == self.active;
            let seg_id: ElementId = child_id(id.clone(), i);
            // The thumb swaps on the swap spring; the label colour follows on the fast tween.
            let phase = spring_phase((seg_id.clone(), "thumb"), on, SpringKind::Swap, window, cx).clamp(0.0, 1.0);
            let text = tween((seg_id.clone(), "text"), if on { p.ink } else { p.ink_3 }, Tween::FAST, window, cx);
            let select = self.on_select.clone();
            track = track.child(
                div()
                    .id(seg_id)
                    .relative()
                    .flex_none()
                    .h(px(SEG_H))
                    .px(px(SEG_PAD_X))
                    .rounded(px(SEG_RADIUS))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .ui(SEG_TEXT)
                    .line_height(relative(1.0))
                    .text_color(text)
                    .whitespace_nowrap()
                    .child(div().absolute().inset_0().rounded(px(SEG_RADIUS)).bg(p.surface_1).shadow(p.shadow(1)).opacity(phase))
                    .child(div().relative().child(label))
                    .on_click(move |_, w, cx| {
                        if let Some(f) = &select {
                            f(i, w, cx)
                        }
                    }),
            );
        }
        track
    }
}

// ── the pane ────────────────────────────────────────────────────────────────

/// The diff review pane. Build with [`diff_review`].
#[derive(IntoElement)]
pub struct DiffReview {
    id: ElementId,
    files: Vec<ReviewFile>,
    diff: Diff,
    notes: Vec<ReviewNote>,
    scope: DiffScope,
    view: DiffView,
    highlights: Vec<DiffHighlight>,
    summary: Option<(SharedString, u32, u32)>,
    on_action: Option<Handler>,
}

/// A review pane showing `diff` for the file selected in `files`, with `notes`
/// collected across the change set.
pub fn diff_review(id: impl Into<ElementId>, files: Vec<ReviewFile>, diff: Diff, notes: Vec<ReviewNote>, scope: DiffScope, view: DiffView) -> DiffReview {
    DiffReview { id: id.into(), files, diff, notes, scope, view, highlights: Vec::new(), summary: None, on_action: None }
}

impl DiffReview {
    /// The summary beside the scope control: a lead (`"vs main · 3 files"`)
    /// followed by the totals, tinted success and danger.
    pub fn summary(mut self, lead: impl Into<SharedString>, added: u32, removed: u32) -> Self {
        self.summary = Some((lead.into(), added, removed));
        self
    }

    /// Word-level highlights inside diff rows.
    pub fn highlights(mut self, highlights: Vec<DiffHighlight>) -> Self {
        self.highlights = highlights;
        self
    }

    /// Action handler.
    pub fn on_action(mut self, f: impl Fn(DiffReviewAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

/// The M / A / D letter and its status colour.
fn change_letter(kind: ChangeKind, p: &Palette) -> (&'static str, Hsla) {
    match kind {
        ChangeKind::Modified => ("M", p.warning),
        ChangeKind::Added => ("A", p.success),
        ChangeKind::Deleted => ("D", p.danger),
    }
}

/// `+8 −3`, dropping a zero side; the minus is U+2212 as in the design.
fn counts(added: u32, removed: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if added > 0 {
        parts.push(format!("+{added}"));
    }
    if removed > 0 {
        parts.push(format!("−{removed}"));
    }
    parts.join(" ")
}

/// A text run of `len` bytes in `family`, optionally on a highlight ground.
fn run(len: usize, family: &'static str, color: Hsla, background: Option<Hsla>) -> TextRun {
    TextRun { len, font: gpui::font(family), color, background_color: background, underline: None, strikethrough: None }
}

/// `.n`: the 14 px accent disc carrying a note number or a note count.
fn note_badge(p: &Palette, label: String) -> impl IntoElement {
    div()
        .flex_none()
        .size(px(BADGE))
        .rounded(px(scale::R_XS))
        .bg(p.accent)
        .flex()
        .items_center()
        .justify_center()
        .mono(BADGE_TEXT)
        .line_height(px(BADGE))
        .semibold()
        .text_color(gpui::white())
        .child(label)
}

/// `.caps`: the column headers, uppercased by the caller because gpui has no
/// text-transform. `.caps` fixes no line height, so it keeps the inherited
/// `--lh-ui` rather than the 1.0 of [`TextRole::Caps`].
fn column_caps(p: &Palette, label: &'static str, top: f32) -> impl IntoElement {
    div().pt(px(top)).px(px(CAPS_PAD_X)).pb(px(CAPS_PAD_BOTTOM)).ui(scale::FS_11).semibold().text_color(p.ink_3).child(label)
}

/// `.note` under a diff row: the numbered caps line, the body (a textarea while
/// pending) and the Edit / Delete or Cancel / Add note actions.
fn review_note(p: &Palette, id: ElementId, index: usize, note: &ReviewNote, on_action: Option<Handler>) -> impl IntoElement {
    let emit = |action: DiffReviewAction| {
        let h = on_action.clone();
        move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
            if let Some(h) = &h {
                h(action.clone(), w, cx)
            }
        }
    };
    // One uniform note card: a hairline `line` border all the way round on
    // surface-2, the same frame the approval card uses. Pending and saved
    // differ in content (Cancel / Add note vs Edit / Delete), never in border.
    let el = v_flex()
        .id(id.clone())
        .mt(px(NOTE_MT))
        .mr(px(NOTE_MR))
        .mb(px(NOTE_MB))
        .ml(px(NOTE_ML))
        .py(px(NOTE_PAD_Y))
        .px(px(NOTE_PAD_X))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_2)
        .ui(NOTE_TEXT)
        .line_height(relative(NOTE_LH))
        .text_color(p.ink);
    let el = el
        .child(
            div()
                .mb(px(NOTE_CAPS_GAP))
                .text_role(TextRole::Caps)
                .line_height(relative(NOTE_LH))
                .text_color(p.accent_ink)
                .child(format!("NOTE {} · LINE {}", index + 1, note.line)),
        )
        .child(div().w_full().when(note.pending, |d| d.py(px(NOTE_TEXTAREA_PAD))).child(note.text.clone()));
    let el = if note.pending {
        el.child(
            h_flex()
                .w_full()
                .mt(px(NOTE_PENDING_TOP))
                .gap(px(NOTE_PENDING_GAP))
                .justify_end()
                .child(button((id.clone(), "cancel"), "Cancel").xs().ghost().on_click(emit(DiffReviewAction::CancelNote(index))))
                .child(button((id, "save"), "Add note").xs().primary().on_click(emit(DiffReviewAction::SaveNote(index)))),
        )
    } else {
        el.child(
            h_flex()
                .mt(px(NOTE_SAVED_TOP))
                .gap(px(NOTE_SAVED_GAP))
                .child(button((id.clone(), "edit"), "Edit").xs().ghost().on_click(emit(DiffReviewAction::EditNote(index))))
                .child(button((id, "delete"), "Delete").xs().ghost().on_click(emit(DiffReviewAction::DeleteNote(index)))),
        )
    };
    el
}

impl RenderOnce for DiffReview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let emit = |action: DiffReviewAction| {
            let h = self.on_action.clone();
            move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(h) = &h {
                    h(action.clone(), w, cx)
                }
            }
        };

        // ── top row ─────────────────────────────────────────────────────────
        let scope_handler = self.on_action.clone();
        let view_handler = self.on_action.clone();
        let scopes = segmented(
            (id.clone(), "scope"),
            DiffScope::ALL.iter().map(|s| SharedString::from(s.label())).collect(),
            DiffScope::ALL.iter().position(|s| *s == self.scope).unwrap_or(0),
        )
        .on_select(move |i, w, cx| {
            if let (Some(h), Some(s)) = (&scope_handler, DiffScope::ALL.get(i)) {
                h(DiffReviewAction::Scope(*s), w, cx)
            }
        });
        let views = segmented(
            (id.clone(), "view"),
            DiffView::ALL.iter().map(|v| SharedString::from(v.label())).collect(),
            DiffView::ALL.iter().position(|v| *v == self.view).unwrap_or(0),
        )
        .on_select(move |i, w, cx| {
            if let (Some(h), Some(v)) = (&view_handler, DiffView::ALL.get(i)) {
                h(DiffReviewAction::View(*v), w, cx)
            }
        });
        let summary = self.summary.as_ref().map(|(lead, added, removed)| {
            let plus = format!("+{added}");
            let minus = format!("−{removed}");
            let text = format!("{lead} · {plus} {minus}");
            let head = text.len() - plus.len() - minus.len() - 1;
            let runs = vec![
                run(head, scale::FONT_UI, p.ink_3, None),
                run(plus.len(), scale::FONT_UI, p.success, None),
                run(1, scale::FONT_UI, p.ink_3, None),
                run(minus.len(), scale::FONT_UI, p.danger, None),
            ];
            div().flex_none().ui(SUMMARY_TEXT).text_color(p.ink_3).whitespace_nowrap().child(StyledText::new(text).with_runs(runs))
        });
        let top = h_flex()
            .w_full()
            .h(px(TOP_H))
            .flex_none()
            .gap(px(TOP_GAP))
            .px(px(TOP_PAD_X))
            .border_b_1()
            .border_color(p.line)
            .child(scopes)
            .children(summary)
            .child(div().flex_1())
            .child(views)
            .child(icon_button((id.clone(), "search"), IconName::Search).ghost().size(ButtonSize::Sm).on_click(emit(DiffReviewAction::Search)))
            .child(button((id.clone(), "collapse"), "Collapse all").sm().ghost().on_click(emit(DiffReviewAction::CollapseAll)));

        // ── files column ────────────────────────────────────────────────────
        let mut files = v_flex()
            .w(px(FILES_W))
            .flex_none()
            .h_full()
            .p(px(FILES_PAD))
            .overflow_hidden()
            .border_r_1()
            .border_color(p.line)
            .ui(FILES_TEXT)
            .child(column_caps(&p, "CHANGED FILES", CAPS_PAD_TOP));
        for (i, file) in self.files.iter().enumerate() {
            let row_id: ElementId = (id.clone(), SharedString::from(format!("file-{i}"))).into();
            let (state, flags) = interaction_flags(row_id.clone(), window, cx);
            let (letter, letter_color) = change_letter(file.change.change, &p);
            let name = file.change.path.rsplit('/').next().unwrap_or(&file.change.path).to_string();
            let bg = if file.selected {
                Some(p.surface_3)
            } else if flags.hovered {
                Some(p.surface_2)
            } else {
                None
            };
            let path = SharedString::from(file.change.path.clone());
            let mut row = h_flex()
                .id(row_id)
                .w_full()
                .flex_none()
                .h(cx.aui().metrics.row_sm)
                .gap(px(FILE_ROW_GAP))
                .px(px(FILE_ROW_PAD_X))
                .rounded(px(scale::R_SM))
                .cursor_pointer()
                .text_color(if file.selected { p.ink } else { p.ink_2 })
                .track_interaction(&state)
                .on_click(emit(DiffReviewAction::SelectFile(path)))
                .child(div().flex_none().w(px(STATUS_W)).flex().justify_center().mono(STATUS_TEXT).line_height(relative(1.0)).semibold().text_color(letter_color).child(letter))
                .child(div().min_w(px(0.0)).truncate().child(name));
            if let Some(bg) = bg {
                row = row.bg(bg);
            }
            if file.notes > 0 {
                row = row.child(note_badge(&p, file.notes.to_string()));
            }
            files = files.child(
                row.child(div().flex_1())
                    .child(div().flex_none().mono(COUNTS_TEXT).line_height(relative(1.0)).medium().text_color(p.ink_3).child(counts(file.change.added, file.change.removed))),
            );
        }
        files = files.child(column_caps(&p, "NOTES", NOTES_CAPS_TOP));
        for (i, note) in self.notes.iter().enumerate() {
            let summary = note.summary.clone().unwrap_or_else(|| note.text.clone());
            files = files.child(
                h_flex()
                    .w_full()
                    .flex_none()
                    .items_start()
                    .gap(px(FILE_ROW_GAP))
                    .py(px(NOTE_ROW_PAD_Y))
                    .px(px(FILE_ROW_PAD_X))
                    .ui(NOTE_ROW_TEXT)
                    .text_color(p.ink_2)
                    .child(div().flex_none().mt(px(NOTE_ROW_BADGE_TOP)).child(note_badge(&p, (i + 1).to_string())))
                    .child(div().min_w(px(0.0)).child(format!("line {} · {}", note.line, summary))),
            );
        }

        // ── diff body ───────────────────────────────────────────────────────
        let file_head = h_flex()
            .w_full()
            .h(px(FILE_HEAD_H))
            .flex_none()
            .gap(px(FILE_HEAD_GAP))
            .px(px(FILE_HEAD_PAD_X))
            .bg(p.surface_2)
            .border_b_1()
            .border_color(p.line)
            .ui(FILE_HEAD_TEXT)
            .line_height(relative(1.0))
            .text_color(p.ink)
            .child(icon(IconName::File).size(px(FILE_HEAD_GLYPH)).color(p.ink))
            .child(div().min_w(px(0.0)).truncate().mono(FILE_HEAD_TEXT).child(SharedString::from(self.diff.path.clone())))
            .child(pill(counts(self.diff.added, self.diff.removed)).variant(PillVariant::Success).height(COUNT_PILL_H).padding_x(COUNT_PILL_PAD_X))
            .child(div().flex_1())
            .child(button((id.clone(), "open"), "Open in editor").xs().ghost().on_click(emit(DiffReviewAction::OpenInEditor)))
            .child(button((id.clone(), "stage"), "Stage").xs().ghost().on_click(emit(DiffReviewAction::Stage)));

        let mut body = v_flex().flex_1().min_w(px(0.0)).h_full().overflow_hidden().mono(DIFF_TEXT).line_height(relative(DIFF_LH)).text_color(p.ink).child(file_head);
        for (h, hunk) in self.diff.hunks.iter().enumerate() {
            body = body.child(
                h_flex()
                    .w_full()
                    .flex_none()
                    .gap(px(HUNK_GAP))
                    .py(px(HUNK_PAD_Y))
                    .px(px(HUNK_PAD_X))
                    .bg(p.surface_2)
                    .border_t_1()
                    .border_b_1()
                    .border_color(p.line)
                    .text_px(HUNK_TEXT)
                    .text_color(p.ink_3)
                    .child(icon(IconName::Chev).size(px(HUNK_CHEV)).color(p.ink_3).rotate(gpui::radians(std::f32::consts::FRAC_PI_2)))
                    .child(div().min_w(px(0.0)).truncate().child(SharedString::from(hunk.header.clone()))),
            );
            for (i, line) in hunk.lines.iter().enumerate() {
                let line_id: ElementId = (id.clone(), SharedString::from(format!("h{h}-l{i}"))).into();
                let (line_state, line_flags) = interaction_flags(line_id.clone(), window, cx);
                let (ground, hover_ground, old_color) = match line.kind {
                    DiffKind::Add => (Some(p.diff_add), p.diff_add_strong, p.success),
                    DiffKind::Del => (Some(p.diff_del), p.diff_del_strong, p.danger),
                    DiffKind::Context => (None, p.surface_2, p.ink_4),
                };
                let strong = match line.kind {
                    DiffKind::Add => p.diff_add_strong,
                    _ => p.diff_del_strong,
                };
                // `.hl` / `.hd2`: the changed words carry the strong ground.
                let span = self.highlights.iter().find(|s| s.hunk == h && s.line == i);
                let text = line.text.clone();
                let styled = match span.filter(|s| s.end <= text.len() && s.start < s.end) {
                    Some(s) => StyledText::new(text.clone()).with_runs(vec![
                        run(s.start, scale::FONT_MONO, p.ink, None),
                        run(s.end - s.start, scale::FONT_MONO, p.ink, Some(strong)),
                        run(text.len() - s.end, scale::FONT_MONO, p.ink, None),
                    ]),
                    None => StyledText::new(text),
                };
                let plus = tween((line_id.clone(), "plus"), if line_flags.hovered { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);
                let target = line.new_no.or(line.old_no).unwrap_or(0);
                let mut row = h_flex()
                    .id(line_id)
                    .relative()
                    .w_full()
                    .flex_none()
                    .pr(px(LINE_PAD_R))
                    .track_interaction(&line_state)
                    .on_click(emit(DiffReviewAction::AddNote(target)))
                    .child(
                        div()
                            .absolute()
                            .left(px(PLUS_LEFT))
                            .top(px(PLUS_TOP))
                            .size(px(PLUS))
                            .rounded(px(scale::R_XS))
                            .bg(p.accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(plus)
                            .cursor_pointer()
                            .child(icon(IconName::Plus).size(px(PLUS_GLYPH)).color(gpui::white())),
                    )
                    .child(div().flex_none().w(px(OLD_W)).pr(px(GUTTER_PAD)).flex().justify_end().text_color(old_color).child(line.old_no.map(|n| n.to_string()).unwrap_or_default()))
                    .child(div().flex_none().w(px(NEW_W)).pr(px(GUTTER_PAD)).flex().justify_end().text_color(p.ink_4).child(line.new_no.map(|n| n.to_string()).unwrap_or_default()))
                    .child(div().flex_1().min_w(px(0.0)).overflow_hidden().child(styled));
                row = if line_flags.hovered {
                    row.bg(hover_ground)
                } else if let Some(ground) = ground {
                    row.bg(ground)
                } else {
                    row
                };
                body = body.child(row);
                for (n, note) in self.notes.iter().enumerate() {
                    if note.file.as_ref() == self.diff.path && Some(note.line) == line.new_no {
                        body = body.child(review_note(&p, (id.clone(), SharedString::from(format!("note-{n}"))).into(), n, note, self.on_action.clone()));
                    }
                }
            }
        }

        // ── action row ──────────────────────────────────────────────────────
        let note_count = self.notes.len();
        let mut touched: Vec<&SharedString> = self.notes.iter().map(|n| &n.file).collect();
        touched.sort();
        touched.dedup();
        let hint = format!(
            "{note_count} note{} on {} file{}",
            if note_count == 1 { "" } else { "s" },
            touched.len(),
            if touched.len() == 1 { "" } else { "s" }
        );
        let send = button((id.clone(), "send"), "Send to").sm().primary().trailing(
            h_flex()
                .items_center()
                .ml(px(SEND_MARK_MARGIN))
                .gap(px(SEND_MARK_GAP))
                .child(provider_mark(Provider::Claude).size(px(SEND_MARK)))
                .child("Claude Code"),
        );
        let bottom = h_flex()
            .w_full()
            .h(px(BOTTOM_H))
            .flex_none()
            .gap(px(BOTTOM_GAP))
            .px(px(BOTTOM_PAD_X))
            .bg(p.surface_2)
            .border_t_1()
            .border_color(p.line)
            .child(div().flex_none().ui(HINT_TEXT).text_color(p.ink_3).whitespace_nowrap().child(hint))
            .child(div().flex_1())
            .child(button((id.clone(), "clear"), "Clear").sm().ghost().on_click(emit(DiffReviewAction::Clear)))
            .child(send.on_click(emit(DiffReviewAction::Send)));

        v_flex()
            .id(id)
            .w_full()
            .h(px(FRAME_H))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.surface_1)
            .overflow_hidden()
            .child(top)
            .child(h_flex().w_full().flex_1().min_h(px(0.0)).items_start().child(files).child(body))
            .child(bottom)
    }
}
