//! D6's Changes screen (docs/43 §2.6; canvas v49 `D6Changes`,
//! `D6StatesChanges`, `D6PRForm`): section captions, change rows that open
//! their diff in place, the inline unified diff, and the commit box at the
//! foot whose single primary follows the branch — Commit, Push, Create PR,
//! then the PR card. Git itself lives in the host; these only draw and report.

use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use aui_icons::{icon, IconName};
use aui_motion::{collapse_fluid, collapse_fluid_opacity, open_close, SpringKind, COLLAPSE_EXIT};
use aui_protocol::{ChangeKind, Diff, DiffKind, DiffLine, FileChange};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, radians, AnyElement, App, ClickEvent, ElementId, FontWeight, Hsla, IntoElement, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::interact::paint::{PaintHover, PaintStates, StatePaint};
use crate::data::{icon_button, spinner};
use super::line_select::{note_line, selectable_row, DragState, LineSelectGeometry, SelectLinesHandler};


type Click = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
/// A click on the unchanged run after hunk `i`.
type GapHandler = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

// ─────────────────────────────────────────────────────────────── captions

/// A section caption ("This session 3", "Other changes 2"): 32 tall (the
/// first 28), the text at the foot, 13.2 muted, the count in faint mono 12.1.
pub fn changes_caption(id: impl Into<ElementId>, label: impl Into<SharedString>, count: Option<usize>, first: bool, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let label: SharedString = label.into();
    h_flex()
        .id(id.into())
        .role(gpui::Role::Heading)
        .aria_label(match count {
            Some(n) => SharedString::from(format!("{label}, {n}")),
            None => label.clone(),
        })
        .h(px(if first { 28.0 } else { 32.0 }))
        .items_end()
        .gap(px(6.0))
        .px(px(8.0))
        .pb(px(6.0))
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .line_height(px(18.0))
        .text_color(p.fg_muted)
        .child(label)
        .when_some(count, |d, n| d.child(div().font_family(scale::FONT_MONO).role_size(TextRole::Caption).text_color(p.ink_4).child(n.to_string())))
}

// ─────────────────────────────────────────────────────────────── change rows

/// The status letter and its colour: M muted, A success, D danger.
pub fn change_letter(kind: ChangeKind, p: &Palette) -> (&'static str, Hsla) {
    match kind {
        ChangeKind::Added => ("A", p.success),
        ChangeKind::Deleted => ("D", p.danger),
        _ => ("M", p.fg_muted),
    }
}

/// `crates/baaz/src/right.rs` → (`right.rs`, `crates/baaz/src`).
pub fn split_path(path: &str) -> (SharedString, SharedString) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (SharedString::from(name.to_string()), SharedString::from(dir.to_string())),
        None => (SharedString::from(path.to_string()), SharedString::default()),
    }
}

/// The folder leaves a change row under this pane width.
pub const CHANGE_FOLDER_MIN_WIDTH: f32 = 340.0;

/// One changed file (docs/43 §2.6): 32 tall, r 6, gap 8 — chevron 12 muted in
/// a 16 slot 4 in, the status letter `t-micro`, the name 14.3, the folder
/// 13.2 muted (hidden under 340 wide), `+N −M` at the right; on hover one
/// "open file" button after the counts. The whole row toggles its diff. The
/// row paints no hover itself: the screen's [`RowList`](crate::interact::RowList)
/// does (the one model's overlay).
#[derive(IntoElement)]
pub struct ChangeRow {
    id: ElementId,
    change: FileChange,
    open: bool,
    width: Option<Pixels>,
    flash: f32,
    on_toggle: Option<Click>,
    on_open_file: Option<Click>,
    on_open_in: Option<(SharedString, Click)>,
}

/// A change row for `change`; `open` = its diff shows.
pub fn change_row(id: impl Into<ElementId>, change: FileChange, open: bool) -> ChangeRow {
    ChangeRow { id: id.into(), change, open, width: None, flash: 0.0, on_toggle: None, on_open_file: None, on_open_in: None }
}

impl ChangeRow {
    /// The pane's width (the folder leaves under 340).
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// The once-only 9 % flash when the transcript opened this turn's files
    /// (0..=1, the host tweens it down).
    pub fn flash(mut self, amount: f32) -> Self {
        self.flash = amount;
        self
    }

    /// The row's click: toggle the diff.
    pub fn on_toggle(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    /// The hover button: open the file in Files.
    pub fn on_open_file(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_file = Some(Rc::new(f));
        self
    }

    /// The second hover button (docs/82 7.5): open the file outside Baaz,
    /// in the host's chosen editor. `label` names where ("Open in Zed").
    pub fn on_open_in(mut self, label: impl Into<SharedString>, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_in = Some((label.into(), Rc::new(f)));
        self
    }
}

impl RenderOnce for ChangeRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let (letter, letter_color) = change_letter(self.change.change, &p);
        let (name, folder) = split_path(&self.change.path);
        let show_folder = self.width.is_none_or(|w| f32::from(w) >= CHANGE_FOLDER_MIN_WIDTH) && !folder.is_empty();
        let turn = open_close((self.id.clone(), "chevron"), self.open, SpringKind::Moderate, COLLAPSE_EXIT, window, cx);
        let label = format!("{} the diff for {}", if self.open { "Hide" } else { "Show" }, self.change.path);
        let mut counts = h_flex().flex_none().gap(px(6.0)).font_family(scale::FONT_MONO).role_size(TextRole::Caption);
        if self.change.added > 0 {
            counts = counts.child(div().text_color(p.success).child(format!("+{}", self.change.added)));
        }
        if self.change.removed > 0 {
            counts = counts.child(div().text_color(p.danger).child(format!("−{}", self.change.removed)));
        }
        let mut row = h_flex()
            .id(self.id.clone())
            .w_full()
            .h(px(scale::CHANGE_ROW))
            .gap(px(8.0))
            .pr(px(8.0))
            .rounded(px(6.0))
            .items_center()
            .whitespace_nowrap()
            .cursor_pointer()
            .font_family(scale::FONT_UI)
            .role_size(TextRole::Body)
            .text_color(p.fg)
            .role(gpui::Role::Button)
            .aria_label(label)
            .aria_expanded(self.open)
            .hover_group()
            .when(self.flash > 0.0, |d| d.bg(p.selected_overlay().alpha(p.selected_overlay().a * self.flash.clamp(0.0, 1.0))))
            .child(
                div()
                    .w(px(16.0))
                    .ml(px(4.0))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .child(icon(IconName::Chev).size(px(12.0)).color(p.fg_muted).rotate(radians(turn * std::f32::consts::FRAC_PI_2))),
            )
            .child(div().w(px(12.0)).flex_none().text_center().text_role(TextRole::Micro).text_color(letter_color).child(letter))
            .child(div().flex_none().child(name.clone()))
            .when(show_folder, |d| d.child(div().min_w(px(0.0)).overflow_hidden().text_ellipsis().role_size(TextRole::Body).text_color(p.fg_muted).child(folder)))
            .child(div().flex_1().min_w(px(0.0)))
            .child(counts);
        // The "open file" button shows while the row is hovered; its box is
        // always laid out, so the counts never shift under the pointer.
        {
            if let Some((label, f)) = self.on_open_in.clone() {
                row = row.child(
                    div().ml(px(4.0)).reveal_on_hover(false).child(
                        icon_button((self.id.clone(), "open-in"), IconName::External)
                            .ghost()
                            .accessibility_label(label)
                            .on_click(move |e, w, cx| {
                                cx.stop_propagation();
                                f(e, w, cx)
                            }),
                    ),
                );
            }
            if let Some(f) = self.on_open_file.clone() {
                row = row.child(
                    div().ml(px(4.0)).reveal_on_hover(false).child(
                        icon_button((self.id.clone(), "open"), IconName::File)
                            .ghost()
                            .accessibility_label(format!("Open {name}"))
                            .on_click(move |e, w, cx| {
                                cx.stop_propagation();
                                f(e, w, cx)
                            }),
                    ),
                );
            }
        }
        if let Some(f) = self.on_toggle {
            row = row.on_click(move |e, w, cx| f(e, w, cx));
        }
        row
    }
}

// ─────────────────────────────────────────────────────────────── the diff

/// `@@ -2820,10 +2820,12 @@` → (old start, old length).
pub fn hunk_old_range(header: &str) -> Option<(u32, u32)> {
    let minus = header.split_whitespace().find(|t| t.starts_with('-'))?;
    let body = &minus[1..];
    let (start, len) = match body.split_once(',') {
        Some((s, l)) => (s.parse().ok()?, l.parse().ok()?),
        None => (body.parse().ok()?, 1),
    };
    Some((start, len))
}

/// `@@ -2820,10 +2820,12 @@` → the new side's start.
pub fn hunk_new_start(header: &str) -> Option<u32> {
    let plus = header.split_whitespace().find(|t| t.starts_with('+'))?;
    plus[1..].split(',').next()?.parse().ok()
}

/// The unchanged lines between hunk `i` and hunk `i + 1`, as a 1-based range
/// on the **new** side (where the file is read from to expand them). Empty
/// when the hunks touch.
pub fn unchanged_between(diff: &Diff, i: usize) -> Range<u32> {
    let (Some(a), Some(b)) = (diff.hunks.get(i), diff.hunks.get(i + 1)) else { return 0..0 };
    let (Some(a_new), Some(b_new)) = (hunk_new_start(&a.header), hunk_new_start(&b.header)) else { return 0..0 };
    let a_new_len = a.lines.iter().filter(|l| l.kind != DiffKind::Del).count() as u32;
    let from = a_new + a_new_len;
    if b_new > from { from..b_new } else { 0..0 }
}

/// The inline diff under an open change row (docs/43 §2.6): full-bleed (−6),
/// 2 above and 6 below, mono 12.1 / 20, no wrap. Hunk headers 24 on
/// Raised-lit, muted. Rows 36 / 14 / 1fr: one gutter (the new line; the old
/// for deletions), the sign, the text; adds and deletes tinted. Unchanged
/// runs between hunks fold into a 24 px "N unchanged lines" row that expands
/// in place from `gap_lines` (the host reads the file).
///
/// Lines select (docs/82 7.1, [`super::line_select`]): the index space is
/// [`diff_rows`] — every drawn line in order, hunk headers and the folded
/// "N unchanged lines" rows left out, an opened gap's lines counted.
#[derive(IntoElement)]
pub struct DiffBody {
    id: ElementId,
    diff: Arc<Diff>,
    open_gaps: Vec<(usize, Arc<Vec<String>>)>,
    on_gap: Option<GapHandler>,
    selected: Option<Range<usize>>,
    on_select: Option<SelectLinesHandler>,
    geometry: Option<LineSelectGeometry>,
}

/// The diff body for `diff`.
pub fn diff_body(id: impl Into<ElementId>, diff: Arc<Diff>) -> DiffBody {
    DiffBody { id: id.into(), diff, open_gaps: Vec::new(), on_gap: None, selected: None, on_select: None, geometry: None }
}

impl DiffBody {
    /// The gap after hunk `i` is open and shows `lines` (the new side's
    /// unchanged lines, in order).
    pub fn open_gap(mut self, i: usize, lines: Arc<Vec<String>>) -> Self {
        self.open_gaps.push((i, lines));
        self
    }

    /// A click on the "N unchanged lines" row after hunk `i`.
    pub fn on_gap(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_gap = Some(Rc::new(f));
        self
    }

    /// The selected lines, over [`diff_rows`]' indices (end exclusive).
    pub fn selected(mut self, range: Option<Range<usize>>) -> Self {
        self.selected = range;
        self
    }

    /// A press or drag over the lines: the new range. Without it the lines
    /// do not select.
    pub fn on_select_lines(mut self, f: impl Fn(Range<usize>, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// Where the selected lines are drawn, for the host's selection bar.
    pub fn selection_geometry(mut self, geometry: &LineSelectGeometry) -> Self {
        self.geometry = Some(geometry.clone());
        self
    }
}

/// One drawn piece of a diff, in order.
enum DiffItem {
    /// Hunk `i`'s `@@` header.
    Header(usize),
    /// A selectable line.
    Line(DiffLine),
    /// The folded run after hunk `i`: `n` unchanged lines.
    Gap(usize, u32),
}

/// The diff as drawn: headers, lines (opened gaps' lines among them, as
/// context on both sides) and folded gaps.
fn diff_items(diff: &Diff, open_gaps: &[(usize, Arc<Vec<String>>)]) -> Vec<DiffItem> {
    let mut out = Vec::new();
    let hunks = diff.hunks.len();
    for (i, hunk) in diff.hunks.iter().enumerate() {
        out.push(DiffItem::Header(i));
        out.extend(hunk.lines.iter().cloned().map(DiffItem::Line));
        if i + 1 < hunks {
            let gap = unchanged_between(diff, i);
            if gap.is_empty() {
                continue;
            }
            match open_gaps.iter().find(|(g, _)| *g == i) {
                Some((_, lines)) => {
                    // The old side runs parallel to the new across a gap:
                    // offset by where the next hunk starts on each side.
                    let next = &diff.hunks[i + 1].header;
                    let shift = match (hunk_old_range(next), hunk_new_start(next)) {
                        (Some((old, _)), Some(new)) => old as i64 - new as i64,
                        _ => 0,
                    };
                    for (k, text) in lines.iter().enumerate() {
                        let new_no = gap.start + k as u32;
                        let old_no = u32::try_from(new_no as i64 + shift).ok();
                        out.push(DiffItem::Line(DiffLine { kind: DiffKind::Context, old_no, new_no: Some(new_no), text: text.clone() }));
                    }
                }
                None => out.push(DiffItem::Gap(i, gap.end - gap.start)),
            }
        }
    }
    out
}

/// The diff's selectable lines, in drawing order: what a [`DiffBody`]'s
/// selection ranges index.
pub fn diff_rows(diff: &Diff, open_gaps: &[(usize, Arc<Vec<String>>)]) -> Vec<DiffLine> {
    diff_items(diff, open_gaps)
        .into_iter()
        .filter_map(|item| match item {
            DiffItem::Line(line) => Some(line),
            _ => None,
        })
        .collect()
}

/// Which file a diff quote is numbered by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffSide {
    /// The new file: the selection holds an added or unchanged line.
    New,
    /// The old file: the selection holds removed lines only.
    Old,
}

/// Selected diff lines as a quote: unified-diff text (`+`, `-`, space
/// markers, so a `diff` fence reads it), and the lines it covers — on the
/// new side, or on the old side when every selected line was removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffQuote {
    /// The lines, each with its marker.
    pub text: String,
    /// How many lines.
    pub lines: usize,
    /// The side `first..=last` count on.
    pub side: DiffSide,
    /// The first line number on `side`.
    pub first: u32,
    /// The last line number on `side`.
    pub last: u32,
}

/// `rows[range]` (from [`diff_rows`]) as a [`DiffQuote`]; `None` when the
/// range selects nothing.
pub fn diff_quote(rows: &[DiffLine], range: &Range<usize>) -> Option<DiffQuote> {
    let r = super::line_select::clamp_lines(range, rows.len())?;
    let picked = &rows[r];
    let text = picked
        .iter()
        .map(|l| {
            let mark = match l.kind {
                DiffKind::Add => '+',
                DiffKind::Del => '-',
                DiffKind::Context => ' ',
            };
            format!("{mark}{}", l.text)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let new: Vec<u32> = picked.iter().filter_map(|l| l.new_no).collect();
    let (side, nos) = if new.is_empty() { (DiffSide::Old, picked.iter().filter_map(|l| l.old_no).collect()) } else { (DiffSide::New, new) };
    let (first, last) = (nos.iter().copied().min().unwrap_or(0), nos.iter().copied().max().unwrap_or(0));
    Some(DiffQuote { text, lines: picked.len(), side, first, last })
}

fn diff_line(p: &Palette, gutter: f32, n: Option<u32>, sign: &'static str, text: &str, kind: DiffKind, selected: bool) -> gpui::Div {
    let (bg, sign_color) = match kind {
        DiffKind::Add => (Some(p.diff_add), p.success),
        DiffKind::Del => (Some(p.diff_del), p.danger),
        _ => (None, p.ink_4),
    };
    h_flex()
        .h(px(20.0))
        .w_full()
        .pr(px(12.0))
        .whitespace_nowrap()
        .overflow_hidden()
        .when_some(bg, |d, bg| d.bg(bg))
        // A selected line: the selection tint over the add / delete tint,
        // under the text (the first child paints first).
        .when(selected, |d| d.relative().child(div().absolute().inset_0().bg(p.selection)))
        .child(div().w(px(gutter)).flex_none().pr(px(8.0)).whitespace_nowrap().text_right().text_color(p.ink_4).child(n.map(|n| n.to_string()).unwrap_or_default()))
        .child(div().w(px(14.0)).flex_none().text_color(sign_color).child(sign))
        .child(div().flex_none().text_color(p.fg).child(if text.is_empty() { " ".to_string() } else { text.replace('\t', "    ") }))
}

impl RenderOnce for DiffBody {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut col = v_flex()
            .id(self.id.clone())
            .mx(px(-6.0))
            .mt(px(2.0))
            .mb(px(6.0))
            .overflow_hidden()
            .font_family(scale::FONT_MONO)
            .role_size(TextRole::Caption)
            .line_height(px(20.0));
        // One gutter, 36 wide (8 of it before the sign); wider when the
        // numbers need it (Geist Mono 12.1 runs ~8 px a digit).
        let widest = self.diff.hunks.iter().flat_map(|h| h.lines.iter()).filter_map(|l| l.new_no.max(l.old_no)).max().unwrap_or(0);
        let gutter = gutter_width(widest);
        let select = self.on_select.clone().map(|on_select| {
            let drag = window.use_keyed_state((self.id.clone(), "line-drag"), cx, |_, _| DragState::default());
            (on_select, drag)
        });
        if let Some(g) = &self.geometry {
            g.begin();
        }
        let selected = self.selected.clone();
        let mut ix = 0usize;
        for item in diff_items(&self.diff, &self.open_gaps) {
            match item {
                DiffItem::Header(i) => {
                    col = col.child(
                        div()
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .px(px(12.0))
                            .bg(crate::util::raised_lit(&p, p.surface_2))
                            .text_color(p.fg_muted)
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .child(self.diff.hunks[i].header.clone()),
                    );
                }
                DiffItem::Line(line) => {
                    let (n, sign, what) = match line.kind {
                        DiffKind::Add => (line.new_no, "+", "Added line"),
                        DiffKind::Del => (line.old_no, "−", "Removed line"),
                        _ => (line.new_no, " ", "Line"),
                    };
                    let in_sel = select.is_some() && selected.as_ref().is_some_and(|r| r.contains(&ix));
                    let row = diff_line(&p, gutter, n, sign, &line.text, line.kind, in_sel);
                    let row = match &select {
                        Some((on_select, drag)) => {
                            let row = row.id((self.id.clone(), SharedString::from(format!("line-{ix}")))).role(gpui::Role::ListItem).aria_label(match n {
                                Some(n) => SharedString::from(format!("{what} {n}")),
                                None => SharedString::from(what),
                            });
                            let row = selectable_row(row, ix, selected.clone(), drag.clone(), on_select.clone());
                            note_line(row, ix, selected.as_ref(), self.geometry.as_ref(), gutter + 14.0).into_any_element()
                        }
                        None => row.into_any_element(),
                    };
                    col = col.child(row);
                    ix += 1;
                }
                DiffItem::Gap(i, n) => {
                    let on_gap = self.on_gap.clone();
                    col = col.child(
                        h_flex()
                            .id((self.id.clone(), SharedString::from(format!("gap-{i}"))))
                            .h(px(24.0))
                            .pl(px(22.0))
                            .gap(px(6.0))
                            .items_center()
                            .cursor_pointer()
                            .role(gpui::Role::Button)
                            .aria_label(format!("Show {n} unchanged lines"))
                            .font_family(scale::FONT_UI)
                            .role_size(TextRole::Body)
                            .paint_states(StatePaint::new().tint(false).ink(p.fg_muted, p.fg), &p)
                            .child(icon(IconName::ChevronDown).size(px(11.0)))
                            .child(format!("{n} unchanged line{}", if n == 1 { "" } else { "s" }))
                            .when_some(on_gap, |d, f| d.on_click(move |_, w, cx| f(i, w, cx))),
                    );
                }
            }
        }
        col
    }
}

/// The diff gutter for numbers up to `widest`: 36, or the digits plus the 8
/// before the sign when that is wider.
pub fn gutter_width(widest: u32) -> f32 {
    let digits = widest.max(1).to_string().len() as f32;
    (digits * 8.5 + 8.0).ceil().max(36.0)
}

/// A change row's diff folding open under it on D4's fold motion (height on
/// `spring.moderate`, the body fading in 80 ms later); `None` once it has
/// folded shut.
///
/// It is its own list entry, never one element with the row: put the row in
/// the list with [`RowList::item`](crate::interact::RowList::item) and
/// this right after it with
/// [`RowList::item_static`](crate::interact::RowList::item_static), so
/// the pointer over the diff lights nothing. Owner 2026-10-21: as one item
/// (the old `change_with_diff`) the hover pill covered the row and every
/// hunk, and its tint washed out the hunk headers that divide them.
pub fn diff_fold(id: impl Into<ElementId>, open: bool, body: impl FnOnce() -> AnyElement, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let id: ElementId = id.into();
    let opacity = collapse_fluid_opacity((id.clone(), "fade"), open, window, cx);
    let child = div().opacity(opacity).child(body()).into_any_element();
    let (reveal, progress) = collapse_fluid((id, "fold"), open, child, window, cx);
    (open || progress > 0.001).then(|| div().w_full().child(reveal).into_any_element())
}

// ─────────────────────────────────────────────────────────────── commit box

/// The foot's primary button: 28, r 6, the foreground fill with frame
/// text, 600; disabled at 45 %; an optional leading glyph or a spinner.
pub fn primary_action(id: impl Into<ElementId>, label: impl Into<SharedString>, glyph: Option<IconName>, busy: bool, disabled: bool, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let label: SharedString = label.into();
    let ink = p.bg;
    h_flex()
        .id(id.clone())
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .h(px(28.0))
        .px(px(12.0))
        .gap(px(6.0))
        .flex_none()
        .items_center()
        .rounded(px(6.0))
        .paint_states(StatePaint::new().on(p.accent).ink(ink, ink).disabled(disabled || busy), &p)
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .when(disabled || busy, |d| d.opacity(0.45))
        .when(!disabled && !busy, |d| d.cursor_pointer())
        .when(busy, |d| d.child(spinner((id.clone(), "spin")).size(px(12.0)).muted(cx)))
        .when_some(glyph.filter(|_| !busy), |d, g| d.child(icon(g).size(px(14.0)).color(ink)))
        .child(label)
}

/// A quiet text control (24, 13.2 muted, hover 5 % + foreground): "Show full
/// output", "Ask Muse to fix it", "View", "Cancel".
pub fn text_action(id: impl Into<ElementId>, label: impl Into<SharedString>, glyph: Option<IconName>, height: f32, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let label: SharedString = label.into();
    h_flex()
        .id(id.into())
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .cursor_pointer()
        .h(px(height))
        .px(px(8.0))
        .gap(px(6.0))
        .flex_none()
        .items_center()
        .rounded(px(6.0))
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .whitespace_nowrap()
        .paint_states(StatePaint::new().ink(p.fg_muted, p.fg), &p)
        .when_some(glyph, |d, g| d.child(icon(g).size(px(12.0))))
        .child(label)
}

/// The branch: glyph 12 + name in mono 12.1 muted, then the faint suffix
/// (`↑2`, `↓1`, `· 2 commits not pushed`, `· up to date`).
pub fn branch_label(branch: &str, suffix: Option<SharedString>, cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    h_flex()
        .min_w(px(0.0))
        .overflow_hidden()
        .gap(px(5.0))
        .items_center()
        .whitespace_nowrap()
        .font_family(scale::FONT_MONO)
        .role_size(TextRole::Caption)
        .text_color(p.fg_muted)
        .child(icon(IconName::Branch).size(px(12.0)).color(p.fg_muted))
        .child(branch.to_string())
        .when_some(suffix, |d, s| d.child(div().text_color(p.ink_4).child(s)))
}

/// `↑2 ↓1`, faint; empty when level.
pub fn ahead_behind(ahead: u32, behind: u32) -> Option<SharedString> {
    let mut s = String::new();
    if ahead > 0 {
        s.push_str(&format!("↑{ahead}"));
    }
    if behind > 0 {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&format!("↓{behind}"));
    }
    (!s.is_empty()).then(|| s.into())
}

/// The commit box's frame: the composer's box (docs/83 §6, through
/// [`crate::composer::composer_box`]: `overlay`, 1 px `line-strong`,
/// `accent-ring` while focused, radius 10), padding 8, gap 6.
pub fn commit_frame(focused: bool, cx: &App) -> gpui::Div {
    crate::composer::composer_box(super::pane::field_border(focused, cx), cx).p(px(8.0)).gap(px(6.0))
}

/// The message as drawn at rest: its first line 600, the rest muted, 14.3 /
/// 22, padding 2 / 4, at least two lines; the placeholder "Describe the
/// change" when empty. A click puts the host's editor in its place.
pub fn commit_message_view(id: impl Into<ElementId>, message: &str, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let mut lines = message.trim_end().splitn(2, '\n');
    let first = lines.next().unwrap_or("").to_string();
    let rest = lines.next().map(|r| r.trim_start_matches('\n').to_string()).filter(|r| !r.is_empty());
    v_flex()
        .id(id.into())
        .role(gpui::Role::TextInput)
        .aria_label("Commit message")
        .cursor_text()
        .min_h(px(44.0))
        .px(px(4.0))
        .py(px(2.0))
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .line_height(px(22.0))
        .map(|d| {
            if first.is_empty() && rest.is_none() {
                d.child(div().text_color(p.fg_muted).child("Describe the change"))
            } else {
                d.child(div().font_weight(FontWeight::SEMIBOLD).text_color(p.fg).child(first))
                    .when_some(rest, |d, r| d.child(div().text_color(p.fg_muted).child(r)))
            }
        })
}

/// The pre-commit hook's note above the box (docs/43 §2.6.3): r 8 Raised
/// with the danger ring at 70 %, ✕ danger 14 + "pre-commit hook failed" 600,
/// the first lines in mono 12.1 / 18 muted, then the two text controls.
pub fn hook_failed_note(id: impl Into<ElementId>, title: impl Into<SharedString>, lines: &[String], actions: Vec<AnyElement>, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    v_flex()
        .id(id.into())
        .role(gpui::Role::Alert)
        .w_full()
        .mb(px(8.0))
        .px(px(10.0))
        .py(px(8.0))
        .gap(px(4.0))
        .rounded(px(8.0))
        .bg(p.surface_3)
        .border_1()
        .border_color(p.attention_border(p.danger))
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .child(
            h_flex()
                .gap(px(8.0))
                .items_center()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.fg)
                .child(icon(IconName::X).size(px(14.0)).color(p.danger))
                .child(title.into()),
        )
        .child(
            v_flex()
                .font_family(scale::FONT_MONO)
                .role_size(TextRole::Caption)
                .line_height(px(18.0))
                .text_color(p.fg_muted)
                .children(lines.iter().take(4).map(|l| div().overflow_hidden().whitespace_nowrap().text_ellipsis().child(l.clone()))),
        )
        .child(h_flex().mt(px(2.0)).gap(px(6.0)).children(actions))
}

/// The PR's state on its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    /// Open (success dot).
    Open,
    /// Draft (faint dot).
    Draft,
    /// Merged (muted dot).
    Merged,
    /// Closed (danger dot).
    Closed,
}

impl PrState {
    fn label(self) -> &'static str {
        match self {
            PrState::Open => "Open",
            PrState::Draft => "Draft",
            PrState::Merged => "Merged",
            PrState::Closed => "Closed",
        }
    }
    fn dot(self, p: &Palette) -> Hsla {
        match self {
            PrState::Open => p.success,
            PrState::Draft => p.ink_4,
            PrState::Merged => p.fg_muted,
            PrState::Closed => p.danger,
        }
    }
}

/// The PR card (docs/43 §2.6.6): Raised r 10, padding 10 / 12 — the PR glyph
/// in success, the title 600 (ellipsis), `#128` faint mono; under it 13.2
/// muted: the status dot · state · checks (a spinner while they run), and
/// the View text control.
pub fn pr_card(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    number: u64,
    state: PrState,
    checks: Option<(u32, u32, bool)>,
    view: impl IntoElement,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let mut meta = h_flex()
        .gap(px(8.0))
        .items_center()
        .whitespace_nowrap()
        .role_size(TextRole::Body)
        .text_color(p.fg_muted)
        .child(div().size(px(8.0)).flex_none().rounded_full().bg(state.dot(&p)))
        .child(state.label());
    if let Some((passed, total, running)) = checks {
        meta = meta
            .child("·")
            .when(running, |d| d.child(spinner((id.clone(), "checks")).size(px(12.0)).muted(cx)))
            .child(format!("{passed} of {total} checks"));
    }
    meta = meta.child(div().flex_1()).child(view);
    v_flex()
        .id(id)
        .role(gpui::Role::Group)
        .aria_label(format!("Pull request #{number}"))
        .w_full()
        .px(px(12.0))
        .py(px(10.0))
        .gap(px(6.0))
        .rounded(px(10.0))
        .bg(p.surface_3)
        .border_1()
        .border_color(p.line)
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .child(
            h_flex()
                .gap(px(8.0))
                .items_center()
                .child(icon(IconName::PullRequest).size(px(14.0)).color(p.success))
                .child(div().min_w(px(0.0)).flex_1().overflow_hidden().whitespace_nowrap().text_ellipsis().font_weight(FontWeight::SEMIBOLD).text_color(p.fg).child(title.into()))
                .child(div().flex_none().font_family(scale::FONT_MONO).role_size(TextRole::Caption).text_color(p.ink_4).child(format!("#{number}"))),
        )
        .child(meta)
}

/// The foot that holds the commit box: padding 8 on the pane's solid
/// ground, pinned to the bottom of the screen. No fade above it (docs/83 §1
/// rule 1; the composer band lost its own in task 2.6): the ground is
/// opaque from the box's padding down, so nothing shows through.
pub fn commit_foot(child: impl IntoElement, cx: &App) -> gpui::Div {
    let ground = cx.aui().colors.surface_1;
    div().absolute().left(px(0.0)).right(px(0.0)).bottom(px(0.0)).child(div().w_full().p(px(8.0)).bg(ground).child(child))
}

// ─────────────────────────────────────────────────────────────── Create PR

/// A labelled field of the Create PR form: the label 13.2 muted, 6 above
/// the control.
pub fn form_field(label: impl Into<SharedString>, control: impl IntoElement, cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    v_flex()
        .gap(px(6.0))
        .child(div().font_family(scale::FONT_UI).role_size(TextRole::Body).text_color(p.fg_muted).child(label.into()))
        .child(control)
}

/// The base-branch picker: 32, Raised-lit with a `line` border, mono 12.65, a
/// chevron; the host opens its menu on click.
pub fn base_picker(id: impl Into<ElementId>, base: impl Into<SharedString>, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let base: SharedString = base.into();
    h_flex()
        .id(id.into())
        .role(gpui::Role::ComboBox)
        .aria_label(format!("Base branch, {base}"))
        .cursor_pointer()
        .h(px(32.0))
        .px(px(10.0))
        .gap(px(8.0))
        .flex_none()
        .items_center()
        .rounded(px(6.0))
        .bg(p.surface_4)
        .border_1()
        .border_color(p.line)
        .font_family(scale::FONT_MONO)
        .role_size(TextRole::Caption)
        .text_color(p.fg)
        .child(base)
        .child(icon(IconName::ChevronDown).size(px(12.0)).color(p.fg_muted))
}

/// "Into  [main ▾] ← ⎇ right-pane".
pub fn base_and_head(picker: impl IntoElement, head: &str, cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    h_flex()
        .gap(px(8.0))
        .items_center()
        .flex_wrap()
        .child(picker)
        .child(div().text_color(p.fg_muted).child("←"))
        .child(
            h_flex()
                .gap(px(5.0))
                .items_center()
                .font_family(scale::FONT_MONO)
                .role_size(TextRole::Caption)
                .text_color(p.fg)
                .child(icon(IconName::Branch).size(px(12.0)).color(p.fg_muted))
                .child(head.to_string()),
        )
}

/// A Well input frame (32 tall at least, r 6, a [`super::pane::field_border`]),
/// padding 5 / 10, 14.3 / 22 — around a host's input.
pub fn well_input(child: impl IntoElement, min_h: f32, focused: bool, cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    div()
        .w_full()
        .min_h(px(min_h))
        .px(px(10.0))
        .py(px(5.0))
        .rounded(px(6.0))
        .bg(p.well)
        .border_1()
        .border_color(super::pane::field_border(focused, cx))
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .line_height(px(22.0))
        .text_color(p.fg)
        .child(child)
}

/// "Open as a draft": a 16 px checkbox (r 4, a 1.5 muted ring; checked = the
/// foreground fill with a check) and the label 14.3.
pub fn draft_checkbox(id: impl Into<ElementId>, checked: bool, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let boxed = div()
        .size(px(16.0))
        .flex_none()
        .rounded(px(4.0))
        .flex()
        .items_center()
        .justify_center()
        .map(|d| {
            if checked {
                d.bg(p.accent).child(icon(IconName::CheckBold).size(px(11.0)).color(p.bg))
            } else {
                d.shadow(vec![gpui::BoxShadow {
                    color: p.fg_muted,
                    offset: gpui::point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(1.5),
                    inset: true,
                }])
            }
        });
    h_flex()
        .id(id.into())
        .role(gpui::Role::CheckBox)
        .aria_label("Open as a draft")
        .aria_toggled(if checked { gpui::Toggled::True } else { gpui::Toggled::False })
        .cursor_pointer()
        .gap(px(8.0))
        .items_center()
        .font_family(scale::FONT_UI)
        .role_size(TextRole::Body)
        .text_color(p.fg)
        .child(boxed)
        .child("Open as a draft")
}

#[cfg(test)]
mod tests {
    use super::*;
    use aui_protocol::{DiffLine, Hunk};

    fn line(kind: DiffKind, old_no: Option<u32>, new_no: Option<u32>) -> DiffLine {
        DiffLine { kind, old_no, new_no, text: "x".into() }
    }

    #[test]
    fn hunk_headers_parse_both_sides() {
        assert_eq!(hunk_old_range("@@ -2820,10 +2820,12 @@ fn changes_heading"), Some((2820, 10)));
        assert_eq!(hunk_new_start("@@ -2820,10 +2820,12 @@ fn changes_heading"), Some(2820));
        assert_eq!(hunk_old_range("@@ -5 +5 @@"), Some((5, 1)));
    }

    #[test]
    fn the_gap_between_hunks_counts_new_side_lines() {
        let a = Hunk {
            header: "@@ -10,4 +10,5 @@".into(),
            lines: vec![line(DiffKind::Context, Some(10), Some(10)), line(DiffKind::Del, Some(11), None), line(DiffKind::Add, None, Some(11)), line(DiffKind::Add, None, Some(12)), line(DiffKind::Context, Some(12), Some(13)), line(DiffKind::Context, Some(13), Some(14))],
        };
        let b = Hunk { header: "@@ -40,3 +41,3 @@".into(), lines: vec![line(DiffKind::Context, Some(40), Some(41))] };
        let diff = Diff { path: "a.rs".into(), hunks: vec![a, b], added: 2, removed: 1 };
        // Hunk one covers new lines 10..=14, hunk two starts at 41.
        assert_eq!(unchanged_between(&diff, 0), 15..41);
        assert_eq!(unchanged_between(&diff, 1), 0..0);
    }

    fn two_hunks() -> Diff {
        let t = |kind, old, new, text: &str| DiffLine { kind, old_no: old, new_no: new, text: text.into() };
        let a = Hunk {
            header: "@@ -10,4 +10,4 @@".into(),
            lines: vec![
                t(DiffKind::Context, Some(10), Some(10), "a"),
                t(DiffKind::Del, Some(11), None, "old one"),
                t(DiffKind::Del, Some(12), None, "old two"),
                t(DiffKind::Add, None, Some(11), "new one"),
                t(DiffKind::Add, None, Some(12), "new two"),
                t(DiffKind::Context, Some(13), Some(13), "b"),
            ],
        };
        let b = Hunk { header: "@@ -20,1 +20,1 @@".into(), lines: vec![t(DiffKind::Context, Some(20), Some(20), "c")] };
        Diff { path: "a.rs".into(), hunks: vec![a, b], added: 2, removed: 2 }
    }

    #[test]
    fn diff_rows_skip_headers_and_folded_gaps_but_count_opened_ones() {
        let diff = two_hunks();
        // Folded: the six lines of hunk one and the one of hunk two.
        assert_eq!(diff_rows(&diff, &[]).len(), 7);
        // Opened: lines 14..20 between them, numbered on both sides.
        let gap: Arc<Vec<String>> = Arc::new((14..20).map(|n| format!("g{n}")).collect());
        let rows = diff_rows(&diff, &[(0, gap)]);
        assert_eq!(rows.len(), 13);
        assert_eq!((rows[6].old_no, rows[6].new_no, rows[6].text.as_str()), (Some(14), Some(14), "g14"));
        assert_eq!(rows[12].text, "c");
    }

    #[test]
    fn a_diff_quote_keeps_its_markers_and_counts_on_the_new_side() {
        let rows = diff_rows(&two_hunks(), &[]);
        let q = diff_quote(&rows, &(1..5)).unwrap();
        assert_eq!(q.text, "-old one\n-old two\n+new one\n+new two");
        assert_eq!((q.lines, q.side, q.first, q.last), (4, DiffSide::New, 11, 12));
        let q = diff_quote(&rows, &(0..6)).unwrap();
        assert_eq!((q.side, q.first, q.last), (DiffSide::New, 10, 13));
        assert!(q.text.starts_with(" a\n"), "context keeps its space marker");
    }

    #[test]
    fn removed_lines_alone_count_on_the_old_side() {
        let rows = diff_rows(&two_hunks(), &[]);
        let q = diff_quote(&rows, &(1..3)).unwrap();
        assert_eq!((q.lines, q.side, q.first, q.last), (2, DiffSide::Old, 11, 12));
        assert_eq!(q.text, "-old one\n-old two");
        assert_eq!(diff_quote(&rows, &(40..41)), None);
    }

    #[test]
    fn the_gutter_widens_for_long_numbers() {
        assert_eq!(gutter_width(99), 36.0);
        assert_eq!(gutter_width(2820), 42.0);
        assert_eq!(gutter_width(12_000), 51.0);
    }

    #[test]
    fn ahead_and_behind_read_as_arrows() {
        assert_eq!(ahead_behind(2, 0).as_deref(), Some("↑2"));
        assert_eq!(ahead_behind(2, 1).as_deref(), Some("↑2 ↓1"));
        assert_eq!(ahead_behind(0, 0), None);
    }

    #[test]
    fn paths_split_into_name_and_folder() {
        assert_eq!(split_path("crates/baaz/src/right.rs"), ("right.rs".into(), "crates/baaz/src".into()));
        assert_eq!(split_path("README.md"), ("README.md".into(), "".into()));
    }

    /// Owner 2026-10-21: the pointer anywhere over an open file's diff lit
    /// the whole file — row and every hunk — because the row and its diff
    /// were one list item. Composed as the docs say (row = item, diff =
    /// static), the row still lights and the diff lights nothing.
    mod hover {
        use super::super::{change_row, diff_body, diff_fold};
        use crate::interact::row_list;
        use aui_protocol::{ChangeKind, Diff, DiffKind, DiffLine, FileChange, Hunk};
        use gpui::{div, prelude::*, px, Context, Window};
        use std::cell::Cell;
        use std::rc::Rc;
        use std::sync::Arc;

        struct Host {
            lit: Rc<Cell<Option<usize>>>,
        }

        impl Render for Host {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let lines = (1..=30).map(|n| DiffLine { kind: DiffKind::Context, old_no: Some(n), new_no: Some(n), text: format!("line {n}") }).collect();
                let diff = Arc::new(Diff { path: "a.txt".into(), hunks: vec![Hunk { header: "@@ -1,30 +1,30 @@".into(), lines }], added: 0, removed: 0 });
                let change = FileChange { path: "a.txt".into(), change: ChangeKind::Modified, added: 0, removed: 0 };
                let fold = diff_fold("fold", true, move || diff_body("diff", diff).into_any_element(), window, cx).expect("an open diff shows");
                let lit = self.lit.clone();
                let list = row_list("list")
                    .gap(px(0.0))
                    .item(0, change_row("row", change, true).width(px(380.0)))
                    .item_static(1, fold)
                    .on_highlight(move |index, _, _| lit.set(index));
                div().size_full().child(list)
            }
        }

        #[gpui::test]
        fn the_pointer_over_an_open_diff_lights_nothing(cx: &mut gpui::TestAppContext) {
            cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
            let lit = Rc::new(Cell::new(None));
            let host = lit.clone();
            let (_view, vc) = cx.add_window_view(move |_, _| Host { lit: host });
            vc.run_until_parked();
            // The row is the first 32 px.
            vc.simulate_mouse_move(gpui::point(px(100.0), px(14.0)), None, Default::default());
            vc.run_until_parked();
            assert_eq!(lit.get(), Some(0), "the row lights under the pointer");
            // Down into the diff: a hunk header, then lines well inside it.
            for y in [44.0, 120.0, 300.0, 500.0] {
                vc.simulate_mouse_move(gpui::point(px(100.0), px(y)), None, Default::default());
                vc.run_until_parked();
                assert_eq!(lit.get(), None, "the pointer at y {y} over the diff lights nothing");
            }
        }
    }

    /// docs/82 7.1: a press on a diff line selects it, dragging extends,
    /// shift-press extends from the anchor; the hunk header selects nothing.
    mod select {
        use super::super::diff_body;
        use aui_protocol::{Diff, DiffKind, DiffLine, Hunk};
        use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton, Window};
        use std::cell::RefCell;
        use std::ops::Range;
        use std::rc::Rc;
        use std::sync::Arc;

        struct Host {
            got: Rc<RefCell<Option<Range<usize>>>>,
        }

        impl Render for Host {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let lines = (1..=10).map(|n| DiffLine { kind: DiffKind::Context, old_no: Some(n), new_no: Some(n), text: format!("line {n}") }).collect();
                let diff = Arc::new(Diff { path: "a.txt".into(), hunks: vec![Hunk { header: "@@ -1,10 +1,10 @@".into(), lines }], added: 0, removed: 0 });
                let got = self.got.clone();
                let held = got.borrow().clone();
                div().size_full().child(diff_body("diff", diff).selected(held).on_select_lines(move |r, _, _| *got.borrow_mut() = Some(r)))
            }
        }

        /// The centre of selectable line `k` (2 above, the 24 px header,
        /// 20 px rows).
        fn line_y(k: usize) -> gpui::Pixels {
            px(2.0 + 24.0 + 20.0 * k as f32 + 10.0)
        }

        #[gpui::test]
        fn press_drag_and_shift_press_select_lines(cx: &mut gpui::TestAppContext) {
            cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
            let got = Rc::new(RefCell::new(None));
            let host = got.clone();
            let (view, vc) = cx.add_window_view(move |_, _| Host { got: host });
            vc.run_until_parked();
            // The header is not a line.
            vc.simulate_mouse_down(gpui::point(px(100.0), px(14.0)), MouseButton::Left, Modifiers::none());
            vc.simulate_mouse_up(gpui::point(px(100.0), px(14.0)), MouseButton::Left, Modifiers::none());
            vc.run_until_parked();
            assert_eq!(*got.borrow(), None);
            // Press line 2, drag to line 5.
            vc.simulate_mouse_down(gpui::point(px(100.0), line_y(2)), MouseButton::Left, Modifiers::none());
            vc.run_until_parked();
            assert_eq!(*got.borrow(), Some(2..3));
            vc.update(|_, cx| view.update(cx, |_, cx| cx.notify()));
            for k in 3..=5 {
                vc.simulate_mouse_move(gpui::point(px(100.0), line_y(k)), Some(MouseButton::Left), Modifiers::none());
                vc.run_until_parked();
                vc.update(|_, cx| view.update(cx, |_, cx| cx.notify()));
            }
            vc.simulate_mouse_up(gpui::point(px(100.0), line_y(5)), MouseButton::Left, Modifiers::none());
            vc.run_until_parked();
            assert_eq!(*got.borrow(), Some(2..6));
            // Shift-press line 0: extends from the anchor (line 2).
            vc.update(|_, cx| view.update(cx, |_, cx| cx.notify()));
            vc.run_until_parked();
            vc.simulate_mouse_down(gpui::point(px(100.0), line_y(0)), MouseButton::Left, Modifiers::shift());
            vc.run_until_parked();
            assert_eq!(*got.borrow(), Some(0..3));
        }
    }
}
