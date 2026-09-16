//! Card 37: the code block (terminal ground, 30 px header with filename,
//! language and quiet actions, copy morphing to a check, numbered lines,
//! fold row) and the unified diff block with the per-line note affordance
//! and the note editor.

use std::time::Duration;

use std::ops::Range;

use aui_motion::{icon_morph, spring_phase, tween, IconMorph, SpringKind, Tween};
use aui_protocol::{Diff, DiffKind};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, relative, App, ElementId, Hsla, IntoElement, SharedString, StyledText, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, icon_button, pill, ButtonSize, PillVariant};
use crate::icons::{icon, IconName};
use crate::transcript::selectable::{
    SelectionEndpoint, intersect_range, selectable_text, MessageSelection, SelectionHandler,
    SelectionKey, SpanEvent, SpanHandler, TextSelection,
};
use crate::transcript::syntax::syntax_runs_in;
use crate::util::{indexed_child, interaction_flags, TrackInteraction};

/// `.cb .hd{height:30px;gap:8px;padding:0 6px 0 12px;font:500 11.5px mono}`.
const HEADER_H: f32 = 30.0;
const HEADER_GAP: f32 = 8.0;
const HEADER_PAD_L: f32 = 12.0;
const HEADER_PAD_R: f32 = 6.0;
const HEADER_TEXT: f32 = 11.5;
/// Header glyphs are 12 px; the actions rest at .6 opacity.
const HEADER_GLYPH: f32 = 12.0;
const ACTIONS_REST: f32 = 0.6;
const ACTIONS_GAP: f32 = 2.0;
/// `.cb pre{padding:10px 12px;font:12px/1.6 mono}` with a 22 px gutter.
const CODE_PAD_Y: f32 = 10.0;
const CODE_PAD_X: f32 = 12.0;
const CODE_LH: f32 = 1.6;
const GUTTER_W: f32 = 22.0;
/// `.fold{height:24px;gap:6px;font-size:11px}` with an 11 px chevron.
const FOLD_H: f32 = 24.0;
const FOLD_GAP: f32 = 6.0;
const FOLD_GLYPH: f32 = 11.0;
/// The copy check stays for 1.2 s.
const COPIED_HOLD: Duration = Duration::from_millis(1200);
/// `.df{font:11.5px/1.65 mono}`; `.hunk{padding:2px 12px;font-size:11px}`; `.ln{padding-right:8px}`;
/// `.gutter{width:36px;padding-right:8px}`; `.g2{width:20px;padding-right:8px}`.
const DIFF_TEXT: f32 = 11.5;
const DIFF_LH: f32 = 1.65;
const HUNK_PAD_Y: f32 = 2.0;
const HUNK_PAD_X: f32 = 12.0;
const LINE_PAD_R: f32 = 8.0;
const OLD_W: f32 = 36.0;
const NEW_W: f32 = 20.0;
const GUTTER_PAD: f32 = 8.0;
/// `.add-note{left:2px;top:2px;width:16px;height:16px;border-radius:4px}` with a 10 px plus.
const ADD_NOTE_INSET: f32 = 2.0;
const ADD_NOTE: f32 = 16.0;
const ADD_NOTE_GLYPH: f32 = 10.0;
/// `.note{margin:4px 12px 6px 64px;border:1px solid var(--line);padding:6px 8px;font:12px/1.4}`; caps margin-bottom 2; actions margin-top 6 gap 6.
const NOTE_MT: f32 = 4.0;
const NOTE_MR: f32 = 12.0;
const NOTE_MB: f32 = 6.0;
const NOTE_ML: f32 = 64.0;
const NOTE_PAD_Y: f32 = 6.0;
const NOTE_PAD_X: f32 = 8.0;
const NOTE_LH: f32 = 1.4;
const NOTE_CAPS_GAP: f32 = 2.0;
const NOTE_ACTIONS_TOP: f32 = 6.0;
const NOTE_ACTIONS_GAP: f32 = 6.0;
/// `.pill.success{height:16px;padding:0 6px}` in the diff header.
const COUNT_PILL_H: f32 = 16.0;

/// Actions on a code block header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeBlockAction {
    /// Toggle soft wrap.
    Wrap,
    /// Open in the editor.
    Open,
    /// Copy the code.
    Copy,
    /// Show the folded lines.
    Unfold,
}

type CodeHandler = std::rc::Rc<dyn Fn(CodeBlockAction, &mut Window, &mut App)>;

/// A code block. Build with [`code_block`].
#[derive(IntoElement)]
pub struct CodeBlock {
    id: ElementId,
    path: SharedString,
    language: Option<SharedString>,
    code: SharedString,
    start_line: u32,
    hidden_lines: usize,
    on_action: Option<CodeHandler>,
    selection_key: Option<SelectionKey>,
    selection: Option<Range<usize>>,
    selection_color: Option<Hsla>,
    on_selection_change: Option<SelectionHandler>,
    on_span: Option<SpanHandler>,
}

/// A block showing `code` from `path`.
pub fn code_block(id: impl Into<ElementId>, path: impl Into<SharedString>, code: impl Into<SharedString>) -> CodeBlock {
    CodeBlock { id: id.into(), path: path.into(), language: None, code: code.into(), start_line: 1, hidden_lines: 0, on_action: None, selection_key: None, selection: None, selection_color: None, on_selection_change: None, on_span: None }
}

impl CodeBlock {
    /// The language label after the filename.
    pub fn language(mut self, language: impl Into<SharedString>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// The first line number.
    pub fn start_line(mut self, line: u32) -> Self {
        self.start_line = line;
        self
    }

    /// How many more lines the fold row offers.
    pub fn hidden_lines(mut self, count: usize) -> Self {
        self.hidden_lines = count;
        self
    }

    /// Action handler.
    pub fn on_action(mut self, f: impl Fn(CodeBlockAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }

    /// The selection cell key this block's lines share. Ranges are byte
    /// offsets over the whole block text, newlines included.
    pub fn selection_key(mut self, key: SelectionKey) -> Self {
        self.selection_key = Some(key);
        self
    }

    /// The visible selection, in block-wide byte indices. Only the lines it
    /// overlaps highlight.
    pub fn selection(mut self, range: Option<Range<usize>>) -> Self {
        self.selection = range;
        self
    }

    /// The highlight colour behind selected glyphs. Defaults to the theme's
    /// `selection` token.
    pub fn selection_color(mut self, color: Hsla) -> Self {
        self.selection_color = Some(color);
        self
    }

    /// Selection intents from any line, translated to block-wide indices.
    /// Empty lines carry no bytes, so presses there clear instead.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::rc::Rc::new(f));
        self
    }

    /// Cross-cell selection events from any line, translated to block-wide
    /// indices for the view's [`SpanSession`](super::SpanSession).
    /// Empty lines anchor at their newline offset, so a span dragged across
    /// them stays continuous; word / paragraph picks remap to block-wide
    /// ranges the same way. A block in span mode wires this instead of
    /// [`on_selection_change`](Self::on_selection_change).
    pub fn on_span_event(mut self, f: impl Fn(SpanEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_span = Some(std::rc::Rc::new(f));
        self
    }
}

/// The copy ↔ check morph, keyed per block; the check holds for 1.2 s.
fn copy_button(id: &ElementId, p: &Palette, on_copy: Option<CodeHandler>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let copied = window.use_keyed_state((id.clone(), "copied"), cx, |_, _| false);
    let is_copied = *copied.read(cx);
    let sample = icon_morph((id.clone(), "copy-morph"), is_copied, window, cx);
    let glyph_size = px(HEADER_GLYPH);
    let morph = IconMorph::new(sample, glyph_size, icon(IconName::Copy).size(glyph_size), icon(IconName::CheckBold).size(glyph_size).color(p.success));
    let state = copied.clone();
    div()
        .id((id.clone(), "copy"))
        .flex_none()
        .size(cx.aui().metrics.control_xs)
        .rounded(px(scale::R_SM))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(p.ink_2)
        .hover(|s| s.bg(p.surface_2))
        .on_click(move |_, w, cx| {
            if let Some(h) = &on_copy {
                h(CodeBlockAction::Copy, w, cx);
            }
            state.update(cx, |c, cx| {
                *c = true;
                cx.notify();
            });
            let state = state.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(COPIED_HOLD).await;
                state.update(cx, |c, cx| {
                    *c = false;
                    cx.notify();
                });
            })
            .detach();
        })
        .child(morph)
}

/// Everything that differs between the code block's header and the diff
/// block's, kept in one struct so [`block_header`] stays a short signature
/// alongside gpui's `window` / `cx`.
struct BlockHeaderArgs {
    /// The leading glyph: a file icon for code, the git icon for a diff.
    glyph: IconName,
    /// The path shown next to the glyph; truncates when the row is narrow.
    name: SharedString,
    /// Elements between the name and the flexible gap — the language tag on a
    /// code block, the +/- counts on a diff.
    after: Vec<gpui::AnyElement>,
    /// The right-aligned action buttons, faded until the block is hovered.
    actions: Vec<gpui::AnyElement>,
    /// Whether the enclosing block is hovered; drives the action-row fade.
    hovered: bool,
}

/// The shared 30 px header of code and diff blocks.
fn block_header(p: &Palette, id: &ElementId, args: BlockHeaderArgs, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let BlockHeaderArgs { glyph, name, after, actions, hovered } = args;
    let opacity = tween((id.clone(), "actions"), if hovered { 1.0f32 } else { ACTIONS_REST }, Tween::FAST, window, cx);
    h_flex()
        .w_full()
        .h(px(HEADER_H))
        .flex_none()
        .gap(px(HEADER_GAP))
        .pl(px(HEADER_PAD_L))
        .pr(px(HEADER_PAD_R))
        .bg(p.surface_2)
        .border_b_1()
        .border_color(p.line)
        .mono(HEADER_TEXT)
        .line_height(relative(1.0))
        .medium()
        .text_color(p.ink_2)
        .child(icon(glyph).size(px(HEADER_GLYPH)))
        .child(div().min_w(px(0.0)).truncate().child(name))
        .children(after)
        .child(div().flex_1())
        .child(h_flex().flex_none().gap(px(ACTIONS_GAP)).opacity(opacity).children(actions))
}

impl RenderOnce for CodeBlock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let emit = |action: CodeBlockAction| {
            let h = self.on_action.clone();
            move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(h) = &h {
                    h(action, w, cx)
                }
            }
        };
        let ghost = |name: &'static str, glyph: IconName| icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(HEADER_GLYPH));
        let actions: Vec<gpui::AnyElement> = vec![
            ghost("wrap", IconName::List).on_click(emit(CodeBlockAction::Wrap)).into_any_element(),
            ghost("open", IconName::Edit).on_click(emit(CodeBlockAction::Open)).into_any_element(),
            copy_button(&id, &p, self.on_action.clone(), window, cx).into_any_element(),
        ];
        let after: Vec<gpui::AnyElement> = self.language.iter().map(|l| div().text_color(p.ink_3).child(l.clone()).into_any_element()).collect();
        let header = block_header(&p, &id, BlockHeaderArgs { glyph: IconName::File, name: self.path.clone(), after, actions, hovered: flags.hovered }, window, cx);

        let language = self.language.clone();
        // Selection shares one cell key across lines: `base` is the line's
        // byte offset over the whole block text (`lines()` strips the
        // newline, hence `+ 1`). Without a key the lines keep their plain
        // `StyledText`, pixel-identical to before.
        let sel_key = self.selection_key.clone();
        let sel_color = self.selection_color.unwrap_or(p.selection);
        let sel_range = self.selection.clone();
        let sel_emit = self.on_selection_change.clone();
        let sel_span = self.on_span.clone();
        let mut body = v_flex().w_full().py(px(CODE_PAD_Y)).px(px(CODE_PAD_X)).mono(scale::FS_12).line_height(relative(CODE_LH)).text_color(p.term_fg).whitespace_nowrap();
        let mut base = 0usize;
        for (i, line) in self.code.lines().enumerate() {
            let line_start = base;
            base += line.len() + 1;
            let runs = syntax_runs_in(line, language.as_deref(), &p, scale::FONT_MONO);
            let text = if line.is_empty() { " ".to_string() } else { line.to_string() };
            let runs = if line.is_empty() { Vec::new() } else { runs };
            let line_body: gpui::AnyElement = match &sel_key {
                Some(key) => {
                    let line_id: ElementId = indexed_child(&id, "sel-", i);
                    let local = sel_range
                        .as_ref()
                        .and_then(|range| intersect_range(range, line_start, line.len()));
                    let mut element = selectable_text(line_id, key.clone(), text)
                        .runs(runs)
                        .selection_color(sel_color)
                        .selection(local);
                    if let Some(emit) = &sel_emit {
                        let emit = emit.clone();
                        let key = key.clone();
                        let empty = line.is_empty();
                        element = element.on_selection_change(move |next, window, cx| {
                            let next = next.and_then(|local| {
                                if empty {
                                    None
                                } else {
                                    Some(TextSelection {
                                        cell: key.clone(),
                                        range: local.range.start + line_start
                                            ..local.range.end + line_start,
                                    })
                                }
                            });
                            emit(next, window, cx);
                        });
                    }
                    if let Some(emit) = &sel_span {
                        let emit = emit.clone();
                        let key = key.clone();
                        let empty = line.is_empty();
                        element = element.on_span_event(move |event, window, cx| {
                            // Empty lines render a phantom space with no
                            // bytes of their own; anchor those events at the
                            // newline offset so spans stay continuous.
                            let at = |offset: usize| {
                                if empty {
                                    line_start
                                } else {
                                    line_start + offset
                                }
                            };
                            let mapped = match event {
                                SpanEvent::Press { offset, .. } => SpanEvent::Press {
                                    cell: key.clone(),
                                    offset: at(offset),
                                },
                                SpanEvent::Hover { offset, .. } => SpanEvent::Hover {
                                    cell: key.clone(),
                                    offset: at(offset),
                                },
                                SpanEvent::Release {
                                    hovered, link, ..
                                } => SpanEvent::Release {
                                    cell: key.clone(),
                                    hovered,
                                    link,
                                },
                                SpanEvent::Pick { selection } => SpanEvent::Pick {
                                    selection: selection.map(|single| {
                                        MessageSelection {
                                            anchor: SelectionEndpoint {
                                                cell: key.clone(),
                                                offset: at(single.anchor.offset),
                                            },
                                            focus: SelectionEndpoint {
                                                cell: key.clone(),
                                                offset: at(single.focus.offset),
                                            },
                                        }
                                    }),
                                },
                            };
                            emit(mapped, window, cx);
                        });
                    }
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .child(element)
                        .into_any_element()
                }
                None => {
                    let styled = if runs.is_empty() {
                        StyledText::new(text)
                    } else {
                        StyledText::new(text).with_runs(runs)
                    };
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .child(styled)
                        .into_any_element()
                }
            };
            body = body.child(
                h_flex()
                    .w_full()
                    .items_start()
                    .child(div().flex_none().w(px(GUTTER_W)).text_color(p.term_dim).child((self.start_line + i as u32).to_string()))
                    .child(line_body),
            );
        }

        let mut block = v_flex()
            .id(id.clone())
            .w_full()
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.term_bg)
            .overflow_hidden()
            .track_interaction(&state)
            .child(header)
            .child(body);
        if self.hidden_lines > 0 {
            block = block.child(
                h_flex()
                    .id((id.clone(), "fold"))
                    .w_full()
                    .h(px(FOLD_H))
                    .justify_center()
                    .gap(px(FOLD_GAP))
                    .bg(p.surface_2)
                    .border_t_1()
                    .border_color(p.line)
                    .ui(scale::FS_11)
                    .text_color(p.ink_3)
                    .cursor_pointer()
                    .on_click(emit(CodeBlockAction::Unfold))
                    .child(icon(IconName::ChevronDown).size(px(FOLD_GLYPH)))
                    .child(format!("Show {} more lines", self.hidden_lines)),
            );
        }
        block
    }
}

/// A note attached to a diff line.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffNote {
    /// The new-side line number the note is on.
    pub line: u32,
    /// The note text.
    pub text: SharedString,
    /// Still being written: shows Cancel / Add note.
    pub pending: bool,
}

/// Actions on a diff block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffBlockAction {
    /// Switch to the unified view.
    Unified,
    /// Switch to the split view.
    Split,
    /// Open in the diff review pane.
    OpenInDiff,
    /// Start a note on a line.
    AddNote(u32),
    /// Save the pending note on a line.
    SaveNote(u32),
    /// Discard the pending note on a line.
    CancelNote(u32),
}

type DiffHandler = std::rc::Rc<dyn Fn(DiffBlockAction, &mut Window, &mut App)>;

/// A diff block. Build with [`diff_block`].
#[derive(IntoElement)]
pub struct DiffBlock {
    id: ElementId,
    diff: Diff,
    notes: Vec<DiffNote>,
    on_action: Option<DiffHandler>,
}

/// A unified diff block for `diff`.
pub fn diff_block(id: impl Into<ElementId>, diff: Diff) -> DiffBlock {
    DiffBlock { id: id.into(), diff, notes: Vec::new(), on_action: None }
}

impl DiffBlock {
    /// Notes shown under their lines.
    pub fn notes(mut self, notes: Vec<DiffNote>) -> Self {
        self.notes = notes;
        self
    }

    /// Action handler.
    pub fn on_action(mut self, f: impl Fn(DiffBlockAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }
}

/// Geometry a host can override on [`diff_note_inset`]. The defaults are the
/// transcript diff block's own values (card 37).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteInsets {
    /// Margins around the card: top, right, bottom, left.
    pub margin: (f32, f32, f32, f32),
    /// Body line height, relative to the font size.
    pub line_height: f32,
    /// Gap under the caps line.
    pub caps_gap: f32,
}

impl Default for NoteInsets {
    fn default() -> Self {
        Self { margin: (NOTE_MT, NOTE_MR, NOTE_MB, NOTE_ML), line_height: NOTE_LH, caps_gap: NOTE_CAPS_GAP }
    }
}

/// `.note`: the note under a diff line, saved or pending.
pub fn diff_note(p: &Palette, id: ElementId, note: &DiffNote, on_action: Option<DiffHandler>) -> impl IntoElement {
    diff_note_inset(p, id, note, NoteInsets::default(), on_action)
}

/// [`diff_note`] with host-supplied margins and body metrics, so a pane that
/// insets its notes differently still draws the one note card.
///
/// Every note card carries the same frame: a hairline `line` border all the way
/// round on `surface-2` at `--r-sm`. Pending and saved differ only in content.
pub fn diff_note_inset(p: &Palette, id: ElementId, note: &DiffNote, insets: NoteInsets, on_action: Option<DiffHandler>) -> impl IntoElement {
    let (mt, mr, mb, ml) = insets.margin;
    let mut el = v_flex()
        .id(id.clone())
        .mt(px(mt))
        .mr(px(mr))
        .mb(px(mb))
        .ml(px(ml))
        .py(px(NOTE_PAD_Y))
        .px(px(NOTE_PAD_X))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_2)
        .ui(scale::FS_12)
        .line_height(relative(insets.line_height))
        .text_color(p.ink)
        .child(div().mb(px(insets.caps_gap)).text_role(TextRole::Caps).line_height(relative(insets.line_height)).text_color(p.accent_ink).child(format!("NOTE · LINE {}", note.line)))
        .child(note.text.clone());
    if note.pending {
        let line = note.line;
        let cancel = on_action.clone();
        let save = on_action;
        el = el.child(
            h_flex()
                .w_full()
                .mt(px(NOTE_ACTIONS_TOP))
                .gap(px(NOTE_ACTIONS_GAP))
                .justify_end()
                .child(button((id.clone(), "cancel"), "Cancel").xs().ghost().on_click(move |_, w, cx| {
                    if let Some(h) = &cancel {
                        h(DiffBlockAction::CancelNote(line), w, cx)
                    }
                }))
                .child(button((id, "save"), "Add note").xs().primary().on_click(move |_, w, cx| {
                    if let Some(h) = &save {
                        h(DiffBlockAction::SaveNote(line), w, cx)
                    }
                })),
        );
    }
    el
}

impl RenderOnce for DiffBlock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let emit = |action: DiffBlockAction| {
            let h = self.on_action.clone();
            move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(h) = &h {
                    h(action.clone(), w, cx)
                }
            }
        };
        let text_button = |name: &'static str, label: &'static str, muted: bool| {
            let mut b = button((id.clone(), name), label).xs().ghost();
            if muted {
                b = b.muted();
            }
            b
        };
        let actions: Vec<gpui::AnyElement> = vec![
            text_button("unified", "Unified", false).on_click(emit(DiffBlockAction::Unified)).into_any_element(),
            text_button("split", "Split", true).on_click(emit(DiffBlockAction::Split)).into_any_element(),
            text_button("open", "Open in Diff", false).on_click(emit(DiffBlockAction::OpenInDiff)).into_any_element(),
        ];
        let after: Vec<gpui::AnyElement> = vec![pill(format!("+{} −{}", self.diff.added, self.diff.removed)).variant(PillVariant::Success).height(COUNT_PILL_H).into_any_element()];
        let header = block_header(&p, &id, BlockHeaderArgs { glyph: IconName::Git, name: self.diff.path.clone().into(), after, actions, hovered: flags.hovered }, window, cx);

        let mut body = v_flex().w_full().mono(DIFF_TEXT).line_height(relative(DIFF_LH)).text_color(p.ink);
        for (h, hunk) in self.diff.hunks.iter().enumerate() {
            body = body.child(div().w_full().py(px(HUNK_PAD_Y)).px(px(HUNK_PAD_X)).bg(p.surface_2).ui(scale::FS_11).font_family(scale::FONT_MONO).text_color(p.ink_3).child(hunk.header.clone()));
            for (i, line) in hunk.lines.iter().enumerate() {
                let line_id: ElementId = (id.clone(), SharedString::from(format!("h{h}-l{i}"))).into();
                let (line_state, line_flags) = interaction_flags(line_id.clone(), window, cx);
                let (bg, sign) = match line.kind {
                    DiffKind::Add => (Some(p.diff_add), "+"),
                    DiffKind::Del => (Some(p.diff_del), "-"),
                    DiffKind::Context => (None, " "),
                };
                let show_add = spring_phase((line_id.clone(), "add-note"), line_flags.hovered, SpringKind::Swap, window, cx).clamp(0.0, 1.0);
                let target_line = line.new_no.or(line.old_no).unwrap_or(0);
                let mut row = h_flex().id(line_id.clone()).relative().w_full().items_start().pr(px(LINE_PAD_R)).track_interaction(&line_state);
                if let Some(bg) = bg {
                    row = row.bg(bg);
                }
                row = row
                    .child(
                        div()
                            .absolute()
                            .left(px(ADD_NOTE_INSET))
                            .top(px(ADD_NOTE_INSET))
                            .size(px(ADD_NOTE))
                            .rounded(px(scale::R_XS))
                            .bg(p.accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(show_add)
                            .cursor_pointer()
                            .child(icon(IconName::Plus).size(px(ADD_NOTE_GLYPH)).color(gpui::white())),
                    )
                    .child(div().flex_none().w(px(OLD_W)).pr(px(GUTTER_PAD)).flex().justify_end().text_color(if line.kind == DiffKind::Del { p.danger } else { p.ink_4 }).child(line.old_no.map(|n| n.to_string()).unwrap_or_default()))
                    .child(div().flex_none().w(px(NEW_W)).pr(px(GUTTER_PAD)).flex().justify_end().text_color(if line.kind == DiffKind::Add { p.success } else { p.ink_4 }).child(line.new_no.map(|n| n.to_string()).unwrap_or_default()))
                    .child(div().flex_1().min_w(px(0.0)).child(format!("{sign} {}", line.text)));
                if line_flags.hovered {
                    let add = emit(DiffBlockAction::AddNote(target_line));
                    row = row.on_click(move |e, w, cx| add(e, w, cx));
                }
                body = body.child(row);
                for (n, note) in self.notes.iter().enumerate() {
                    if Some(note.line) == line.new_no {
                        body = body.child(diff_note(&p, (id.clone(), SharedString::from(format!("note-{n}"))).into(), note, self.on_action.clone()));
                    }
                }
            }
        }

        v_flex()
            .id(id.clone())
            .w_full()
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .track_interaction(&state)
            .child(header)
            .child(body)
    }
}
