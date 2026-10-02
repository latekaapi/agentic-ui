//! Card 37: the code block (terminal ground, 30 px header with filename,
//! language and quiet actions, copy morphing to a check, numbered lines,
//! fold row) and the unified diff block with the per-line note affordance
//! and the note editor.


use std::ops::Range;
use std::sync::Arc;

use aui_motion::{icon_morph, spring_phase, tween, IconMorph, SpringKind, Tween};
use aui_protocol::{Diff, DiffKind};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, relative, uniform_list, App, ClipboardItem, ElementId, Hsla, IntoElement, ListSizingBehavior, Pixels, ScrollStrategy, SharedString, StyledText, UniformListScrollHandle, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, icon_button, pill, record_ax_label, ButtonSize, PillVariant};
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
/// Blocks longer than this render their lines through a virtualised list
/// instead of one row per line, so a 10k-line file costs only the visible
/// rows. Short blocks render exactly as before.
pub const CODE_VIRTUALIZE_AT: usize = 400;
/// Rows rendered past the viewport edge in a virtualised block, top and
/// bottom, so fast scrolling never shows a gap.
pub const CODE_VIRTUAL_OVERDRAW: usize = 16;
/// The fixed height of a virtualised block's scrollable line list.
const VIRTUAL_H: f32 = 320.0;

/// Whether the displayed line `line_no` falls inside `range` (`start`
/// inclusive, `end` exclusive, in displayed line numbers).
pub fn highlight_contains(range: &Range<u32>, line_no: u32) -> bool {
    range.contains(&line_no)
}

/// The row index of the displayed line `line_no` in a block whose first
/// line is `start_line`. `None` when the line is above the block.
pub fn line_row_index(line_no: u32, start_line: u32) -> Option<usize> {
    line_no.checked_sub(start_line).map(|ix| ix as usize)
}

/// How many rows a virtualised block of `total_lines` lines keeps alive for
/// a viewport showing `viewport_lines` rows: the visible rows plus the
/// overdraw on each side, capped at the block length.
pub fn virtual_row_count(total_lines: usize, viewport_lines: usize) -> usize {
    total_lines.min(viewport_lines.saturating_add(2 * CODE_VIRTUAL_OVERDRAW))
}

/// Which displayed line a virtualised block scrolls to on first render: the
/// explicit `scroll_to` target, else the first highlighted line. Returns the
/// row index, clamped to the block. The render path resolves its start
/// position through this, so the helper and the painted list cannot drift.
pub fn code_scroll_target(
    scroll_to: Option<u32>,
    highlight: &Option<Range<u32>>,
    start_line: u32,
    total_lines: usize,
) -> Option<usize> {
    let line = scroll_to
        .or_else(|| highlight.as_ref().filter(|r| !r.is_empty()).map(|r| r.start))?;
    Some(line_row_index(line, start_line).unwrap_or(0).min(total_lines.saturating_sub(1)))
}

/// Whether a scroll request `(line, token)` still needs to fire given the
/// settled request: a new token re-arms even the same line, so requesting
/// the same `path#L42` link again after scrolling away scrolls again. The
/// render path settles through this, so the helper and the painted list
/// cannot drift.
pub fn scroll_request_pending(request_line: u32, request_token: u64, settled: Option<(u32, u64)>) -> bool {
    settled != Some((request_line, request_token))
}

/// How many frames a virtual block's scroll request keeps re-issuing while
/// unsettled before settling anyway, so a target that never lands (a list
/// that never lays out, a deferred scroll dropped before paint) cannot hold
/// the frame loop open. Mirrors the file tree's `FILE_TREE_SCROLL_ATTEMPTS`.
pub const CODE_SCROLL_ATTEMPTS: usize = 10;

/// Whether a scroll loop that has already waited `attempts` frames for an
/// unlanded request must settle without asking for more frames.
pub fn code_scroll_retries_exhausted(attempts: usize) -> bool {
    attempts >= CODE_SCROLL_ATTEMPTS
}

/// The cache key for a block's split lines: the text hash plus its byte
/// length, so a rebuild happens only when the text changes.
pub fn line_cache_key(code: &str) -> (u64, usize) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    (hasher.finish(), code.len())
}

/// Actions on a code block header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeBlockAction {
    /// Toggle soft wrap.
    Wrap,
    /// Open in the editor.
    Open,
    /// Copy the code.
    Copy,
    /// Show the folded lines.
    Unfold,
    /// The host-supplied header action was pressed. It carries everything
    /// the host needs to act on the block — its index, language and code —
    /// without the library knowing why.
    HostAction {
        /// The block's index in the host's list, as passed to
        /// [`CodeBlock::host_action`].
        index: usize,
        /// The fence language, when the block is tagged.
        language: Option<SharedString>,
        /// The block text.
        code: SharedString,
    },
}

/// The description of a [`CodeBlock::host_action`] control.
///
/// Tokens and the shared button component draw it; the host owns what a
/// press does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlockHostButton {
    /// The button label.
    pub label: SharedString,
    /// The leading glyph.
    pub icon: IconName,
}

impl CodeBlockHostButton {
    /// A header action with `label` and a leading `icon`.
    pub fn new(label: impl Into<SharedString>, icon: IconName) -> Self {
        Self { label: label.into(), icon }
    }
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
    highlight: Option<Range<u32>>,
    scroll_to_line: Option<u32>,
    scroll_token: u64,
    fill: bool,
    max_height: Option<Pixels>,
    host_action: Option<(usize, CodeBlockHostButton)>,
    on_action: Option<CodeHandler>,
    selection_key: Option<SelectionKey>,
    selection: Option<Range<usize>>,
    selection_color: Option<Hsla>,
    on_selection_change: Option<SelectionHandler>,
    on_span: Option<SpanHandler>,
}

/// A block showing `code` from `path`.
pub fn code_block(id: impl Into<ElementId>, path: impl Into<SharedString>, code: impl Into<SharedString>) -> CodeBlock {
    CodeBlock { id: id.into(), path: path.into(), language: None, code: code.into(), start_line: 1, hidden_lines: 0, highlight: None, scroll_to_line: None, scroll_token: 0, fill: false, max_height: None, host_action: None, on_action: None, selection_key: None, selection: None, selection_color: None, on_selection_change: None, on_span: None }
}

/// The command a fenced block carries, if it is runnable.
///
/// - `sh`, `bash`, `zsh`, `shell` and `terminal` run whole: the code is
///   returned unchanged.
/// - `console`, and an untagged block whose every non-blank line is a
///   `$ ` prompt, run the prompt lines only: the `$ ` prefixes are
///   stripped and the recorded output lines are dropped.
/// - Anything else is not runnable.
pub fn runnable_command(lang: &str, code: &str) -> Option<String> {
    const SHELLS: [&str; 5] = ["sh", "bash", "zsh", "shell", "terminal"];
    if SHELLS.contains(&lang) {
        return Some(code.to_string());
    }
    if lang == "console" {
        return Some(prompt_lines(code));
    }
    if lang.is_empty() {
        let mut prompts = Vec::new();
        for line in code.lines() {
            if line.trim().is_empty() {
                continue;
            }
            prompts.push(line.strip_prefix("$ ")?);
        }
        return Some(prompts.join("\n"));
    }
    None
}

/// The `$ ` prompt lines of `code` with their prefixes stripped. Blank
/// lines and recorded output lines are dropped.
fn prompt_lines(code: &str) -> String {
    code.lines().filter_map(|line| line.strip_prefix("$ ")).collect::<Vec<_>>().join("\n")
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

    /// Paints a calm band behind the displayed lines in `range` (`start`
    /// inclusive, `end` exclusive — `40..45` bands lines 40 through 44).
    /// Unset by default; without it every line renders exactly as before.
    pub fn highlight_lines(mut self, range: Range<u32>) -> Self {
        self.highlight = Some(range);
        self
    }

    /// Scrolls this displayed line into view on first render (a virtualised
    /// block's list starts there; a short block has no inner scroll, so the
    /// line only gains the highlight when [`highlight_lines`](Self::highlight_lines)
    /// covers it). Unset by default. When unset but a highlight is set, the
    /// list starts at the highlight's first line instead.
    pub fn scroll_to_line(mut self, line: u32) -> Self {
        self.scroll_to_line = Some(line);
        self
    }

    /// The request token for [`scroll_to_line`](Self::scroll_to_line): the
    /// list settles per `(line, token)`, so repeating the same line with a
    /// new token scrolls again after the reader scrolled away. `0` by
    /// default; without it a repeated request for the settled line is a
    /// no-op.
    pub fn scroll_token(mut self, token: u64) -> Self {
        self.scroll_token = token;
        self
    }

    /// Fills the parent's height: the virtual list drops its fixed 320 px
    /// box for `flex-1` / `min-h-0` and becomes the pane's only scroller.
    /// Off by default, which keeps the transcript's fixed strip exactly as
    /// before. Only affects blocks past [`CODE_VIRTUALIZE_AT`] lines.
    pub fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// Caps the virtual list's height. `None` (the default) keeps today's
    /// behaviour: the fixed 320 px box, or the parent's height when
    /// [`fill`](Self::fill) is set.
    pub fn max_height(mut self, max: Option<Pixels>) -> Self {
        self.max_height = max;
        self
    }

    /// An optional host-supplied action at the end of the header row, after
    /// the Copy control. `index` is the block's index in the host's list;
    /// pressing the control emits [`CodeBlockAction::HostAction`] with that
    /// index and the block's language and code, so the host can act on the
    /// block without the library knowing why. Unset by default; without it
    /// the header renders exactly as before.
    pub fn host_action(mut self, index: usize, button: CodeBlockHostButton) -> Self {
        self.host_action = Some((index, button));
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

/// Copies `code` to the clipboard and reports [`CodeBlockAction::Copy`];
/// the clipboard write is the default, so a block with no host handler
/// still copies, and a host handler only adds behaviour.
fn emit_copy(code: &SharedString, on_copy: &Option<CodeHandler>, window: &mut Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
    if let Some(h) = on_copy {
        h(CodeBlockAction::Copy, window, cx);
    }
}

/// The copy ↔ check morph, keyed per block; the check holds for [`COPY_HOLD`](super::turns::COPY_HOLD), the same hold the turn rails use.
fn copy_button(id: &ElementId, p: &Palette, code: SharedString, on_copy: Option<CodeHandler>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let copied = window.use_keyed_state((id.clone(), "copied"), cx, |_, _| false);
    let is_copied = *copied.read(cx);
    let sample = icon_morph((id.clone(), "copy-morph"), is_copied, window, cx);
    let glyph_size = px(HEADER_GLYPH);
    let morph = IconMorph::new(sample, glyph_size, icon(IconName::Copy).size(glyph_size), icon(IconName::CheckBold).size(glyph_size).color(p.success));
    let state = copied.clone();
    // The copy target names itself, like the turn rails' copy buttons do.
    record_ax_label("Copy code");
    div()
        .id((id.clone(), "copy"))
        .flex_none()
        .size(cx.aui().metrics.control_xs)
        .rounded(px(scale::R_SM))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .role(gpui::Role::Button)
        .aria_label("Copy code")
        .text_color(p.ink_2)
        .hover(|s| s.bg(p.surface_2))
        .on_click(move |_, w, cx| {
            emit_copy(&code, &on_copy, w, cx);
            state.update(cx, |c, cx| {
                *c = true;
                cx.notify();
            });
            let state = state.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(super::turns::COPY_HOLD).await;
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

/// The per-line selection wiring a [`CodeBlock`] shares across its rows:
/// one cell key over the whole block text, so a drag can span lines.
#[derive(Clone)]
struct LineSelect {
    key: SelectionKey,
    color: Hsla,
    range: Option<Range<usize>>,
    on_change: Option<SelectionHandler>,
    on_span: Option<SpanHandler>,
}

/// One numbered code row: the gutter plus the syntax-coloured line, with the
/// calm highlight band behind it when `highlighted`.
///
/// Selection shares one cell key across lines: `line_start` is the line's
/// byte offset over the whole block text (`lines()` strips the newline,
/// hence `+ 1`). Without a [`LineSelect`] the line keeps its plain
/// `StyledText`, pixel-identical to before.
#[allow(clippy::too_many_arguments)]
fn code_line_row(
    line_id: ElementId,
    line_no: u32,
    line: &str,
    line_start: usize,
    language: Option<&str>,
    p: &Palette,
    select: Option<&LineSelect>,
    highlighted: bool,
) -> impl IntoElement {
    let runs = syntax_runs_in(line, language, p, scale::FONT_MONO);
    let text = if line.is_empty() { " ".to_string() } else { line.to_string() };
    let runs = if line.is_empty() { Vec::new() } else { runs };
    let line_body: gpui::AnyElement = match select {
        Some(sel) => {
            let local =
                sel.range.as_ref().and_then(|range| intersect_range(range, line_start, line.len()));
            let mut element =
                selectable_text(line_id.clone(), sel.key.clone(), text).runs(runs).selection_color(sel.color).selection(local);
            if let Some(emit) = &sel.on_change {
                let emit = emit.clone();
                let key = sel.key.clone();
                let empty = line.is_empty();
                element = element.on_selection_change(move |next, window, cx| {
                    let next = next.and_then(|local| {
                        if empty {
                            None
                        } else {
                            Some(TextSelection {
                                cell: key.clone(),
                                range: local.range.start + line_start..local.range.end + line_start,
                            })
                        }
                    });
                    emit(next, window, cx);
                });
            }
            if let Some(emit) = &sel.on_span {
                let emit = emit.clone();
                let key = sel.key.clone();
                let empty = line.is_empty();
                element = element.on_span_event(move |event, window, cx| {
                    // Empty lines render a phantom space with no bytes of
                    // their own; anchor those events at the newline offset
                    // so spans stay continuous.
                    let at = |offset: usize| {
                        if empty {
                            line_start
                        } else {
                            line_start + offset
                        }
                    };
                    let mapped = match event {
                        SpanEvent::Press { offset, .. } => SpanEvent::Press { cell: key.clone(), offset: at(offset) },
                        SpanEvent::Hover { offset, .. } => SpanEvent::Hover { cell: key.clone(), offset: at(offset) },
                        SpanEvent::Release { hovered, link, .. } => SpanEvent::Release { cell: key.clone(), hovered, link },
                        SpanEvent::Pick { selection } => SpanEvent::Pick {
                            selection: selection.map(|single| MessageSelection {
                                anchor: SelectionEndpoint { cell: key.clone(), offset: at(single.anchor.offset) },
                                focus: SelectionEndpoint { cell: key.clone(), offset: at(single.focus.offset) },
                            }),
                        },
                    };
                    emit(mapped, window, cx);
                });
            }
            div().flex_1().min_w(px(0.0)).overflow_hidden().child(element).into_any_element()
        }
        None => {
            let _ = &line_id;
            let styled =
                if runs.is_empty() { StyledText::new(text) } else { StyledText::new(text).with_runs(runs) };
            div().flex_1().min_w(px(0.0)).overflow_hidden().child(styled).into_any_element()
        }
    };
    let mut row = h_flex()
        .id((line_id.clone(), "row"))
        .w_full()
        .items_start()
        .role(gpui::Role::ListItem)
        .aria_label(format!("line {line_no}"))
        .child(div().flex_none().w(px(GUTTER_W)).text_color(p.term_dim).child(line_no.to_string()))
        .child(line_body);
    if highlighted {
        row = row.bg(p.accent_soft);
    }
    row
}

/// What a virtual block remembers about its in-flight scroll: which request
/// settled, and how many frames the current one has waited to land.
#[derive(Clone, Default)]
struct CodeScrollState {
    /// The settled `(line, token)` request.
    settled: Option<(u32, u64)>,
    /// The request currently waiting to land.
    waiting: Option<(u32, u64)>,
    /// Frames waited for `waiting` so far.
    attempts: usize,
}

/// The split lines and their block-wide byte offsets, cached per block so a
/// 10k-line file is re-split only when its text changes.
#[derive(Clone)]
struct LineCache {
    /// [`line_cache_key`] of the cached text.
    key: (u64, usize),
    /// One entry per line, newlines stripped.
    lines: Arc<Vec<String>>,
    /// The byte offset of each line over the whole block text.
    starts: Arc<Vec<usize>>,
}

/// The scrollable, virtualised line list for blocks over
/// [`CODE_VIRTUALIZE_AT`] lines: only the visible rows are built each frame,
/// and the list starts at `scroll_to` on first render.
#[allow(clippy::too_many_arguments)]
fn virtual_code_body(
    id: ElementId,
    code: &str,
    start_line: u32,
    language: Option<SharedString>,
    select: Option<LineSelect>,
    highlight: Option<Range<u32>>,
    scroll_to: Option<u32>,
    scroll_token: u64,
    fill: bool,
    max_height: Option<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    // The split lines live in keyed state, rebuilt only when the text's
    // hash/length changes instead of on every render.
    let cache_slot = window.use_keyed_state((id.clone(), "lines-cache"), cx, |_, _| None::<LineCache>);
    let key = line_cache_key(code);
    let cached = cache_slot.read(cx).clone();
    let cache = match cached {
        Some(cache) if cache.key == key => cache,
        _ => {
            let lines: Arc<Vec<String>> = Arc::new(code.lines().map(str::to_string).collect());
            let mut starts = Vec::with_capacity(lines.len());
            let mut base = 0usize;
            for line in lines.iter() {
                starts.push(base);
                base += line.len() + 1;
            }
            let fresh = LineCache { key, lines, starts: Arc::new(starts) };
            cache_slot.update(cx, |slot, cx| {
                *slot = Some(fresh.clone());
                cx.notify();
            });
            fresh
        }
    };
    let lines = cache.lines;
    let starts = cache.starts;
    let total = lines.len();

    let handle: UniformListScrollHandle =
        window.use_keyed_state((id.clone(), "lines-scroll"), cx, |_, _| UniformListScrollHandle::default()).read(cx).clone();
    // The list starts at the requested line on first render. The request
    // settles per `(line, token)`, so repeating the same line with a new
    // token scrolls again; otherwise a reader who scrolls away afterwards is
    // never snapped back. An unsettled request re-arms for at most
    // `CODE_SCROLL_ATTEMPTS` frames and then settles anyway, so a target
    // that never lands cannot ask for frames forever.
    if let Some(line) = scroll_to {
        if let Some(target) = code_scroll_target(Some(line), &highlight, start_line, total) {
            let wanted = (line, scroll_token);
            let state = window.use_keyed_state((id.clone(), "lines-scrolled"), cx, |_, _| CodeScrollState::default());
            let current = state.read(cx).clone();
            if scroll_request_pending(line, scroll_token, current.settled.clone()) {
                let attempts = if current.waiting.as_ref() == Some(&wanted) { current.attempts } else { 0 };
                if code_scroll_retries_exhausted(attempts) {
                    // Done waiting: settle without asking for another frame.
                    state.update(cx, |s, cx| {
                        s.settled = Some(wanted);
                        s.waiting = None;
                        s.attempts = 0;
                        cx.notify();
                    });
                } else {
                    handle.scroll_to_item_strict(target, ScrollStrategy::Top);
                    state.update(cx, |s, cx| {
                        s.waiting = Some(wanted);
                        s.attempts = attempts + 1;
                        cx.notify();
                    });
                    // One more frame so the deferred scroll paints; the
                    // attempts bound above is the only re-arm path, so this
                    // cannot chain beyond `CODE_SCROLL_ATTEMPTS` frames.
                    window.request_animation_frame();
                }
            }
        }
    }

    let list_id = id.clone();
    let row_id = id.clone();
    let list = uniform_list((list_id, "lines"), total, move |range, _window, cx| {
        let p = cx.aui().colors;
        range
            .map(|i| {
                let line_no = start_line + i as u32;
                let highlighted = highlight.as_ref().is_some_and(|r| highlight_contains(r, line_no));
                // The row's accessible name doubles as the render-test
                // probe: only built (visible) rows record, so a draw of a
                // 10k-line file records a bounded set.
                record_ax_label(&format!("line {line_no}"));
                let line_id: ElementId = indexed_child(&row_id, "sel-", i);
                code_line_row(line_id, line_no, &lines[i], starts[i], language.as_deref(), &p, select.as_ref(), highlighted)
            })
            .collect::<Vec<_>>()
    })
    // A filling list takes its parent's height and virtualises against it;
    // `Infer` sizes the list to all of its rows, which mounts every row.
    .with_sizing_behavior(if fill { ListSizingBehavior::Auto } else { ListSizingBehavior::Infer })
    .when(fill, |list| list.h_full())
    .track_scroll(&handle);
    let mut wrap = div()
        .id((id, "lines-wrap"))
        .w_full()
        .overflow_hidden()
        .py(px(CODE_PAD_Y))
        .px(px(CODE_PAD_X))
        .mono(scale::FS_12)
        .line_height(relative(CODE_LH))
        .text_color(cx.aui().colors.term_fg)
        .whitespace_nowrap()
        .role(gpui::Role::List)
        .aria_label("Code lines");
    if fill {
        // The pane's only scroller: no fixed height, no nested wheel trap.
        wrap = wrap.flex_1().min_h(px(0.0)).flex().flex_col();
    } else {
        wrap = wrap.h(px(VIRTUAL_H));
    }
    if let Some(max) = max_height {
        wrap = wrap.max_h(max);
    }
    wrap.child(list).into_any_element()
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
                    h(action.clone(), w, cx)
                }
            }
        };
        let ghost = |name: &'static str, glyph: IconName, label: &'static str| {
            icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(HEADER_GLYPH)).accessibility_label(label)
        };
        let mut actions: Vec<gpui::AnyElement> = vec![
            ghost("wrap", IconName::List, "Toggle soft wrap").on_click(emit(CodeBlockAction::Wrap)).into_any_element(),
            ghost("open", IconName::Edit, "Open in editor").on_click(emit(CodeBlockAction::Open)).into_any_element(),
            copy_button(&id, &p, self.code.clone(), self.on_action.clone(), window, cx).into_any_element(),
        ];
        // The host action trails Copy. With none set the header builds
        // exactly as before.
        if let Some((index, host)) = &self.host_action {
            let press = CodeBlockAction::HostAction { index: *index, language: self.language.clone(), code: self.code.clone() };
            actions.push(button((id.clone(), "host-action"), host.label.clone()).xs().ghost().icon(host.icon).on_click(emit(press)).into_any_element());
        }
        let after: Vec<gpui::AnyElement> = self.language.iter().map(|l| div().text_color(p.ink_3).child(l.clone()).into_any_element()).collect();
        let header = block_header(&p, &id, BlockHeaderArgs { glyph: IconName::File, name: self.path.clone(), after, actions, hovered: flags.hovered }, window, cx);

        let language = self.language.clone();
        let select = self.selection_key.clone().map(|key| LineSelect {
            key,
            color: self.selection_color.unwrap_or(p.selection),
            range: self.selection.clone(),
            on_change: self.on_selection_change.clone(),
            on_span: self.on_span.clone(),
        });
        let highlight = self.highlight.clone();
        if let Some(range) = highlight.as_ref().filter(|r| !r.is_empty()) {
            record_ax_label(&format!("Highlighted lines {} to {}", range.start, range.end - 1));
        }
        // Long blocks virtualise: only the visible rows are built each
        // frame. Short blocks keep the one-row-per-line body, plus the
        // highlight band where set.
        let body: gpui::AnyElement = if self.code.lines().count() > CODE_VIRTUALIZE_AT {
            let scroll_to = self
                .scroll_to_line
                .or_else(|| highlight.as_ref().filter(|r| !r.is_empty()).map(|r| r.start));
            virtual_code_body(
                id.clone(),
                &self.code,
                self.start_line,
                language.clone(),
                select.clone(),
                highlight.clone(),
                scroll_to,
                self.scroll_token,
                self.fill,
                self.max_height,
                window,
                cx,
            )
        } else {
            let mut body = v_flex().w_full().py(px(CODE_PAD_Y)).px(px(CODE_PAD_X)).mono(scale::FS_12).line_height(relative(CODE_LH)).text_color(p.term_fg).whitespace_nowrap();
            let mut base = 0usize;
            for (i, line) in self.code.lines().enumerate() {
                let line_start = base;
                base += line.len() + 1;
                let line_no = self.start_line + i as u32;
                let highlighted = highlight.as_ref().is_some_and(|r| highlight_contains(r, line_no));
                let line_id: ElementId = indexed_child(&id, "sel-", i);
                body = body.child(code_line_row(
                    line_id,
                    line_no,
                    line,
                    line_start,
                    language.as_deref(),
                    &p,
                    select.as_ref(),
                    highlighted,
                ));
            }
            body.into_any_element()
        };

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
        if self.fill {
            // A filling block stretches with its pane so the virtual list
            // inside is the pane's only scroller.
            block = block.flex_1().min_h(px(0.0));
        }
        if self.hidden_lines > 0 {
            let fold_label = format!("Show {} more lines", self.hidden_lines);
            record_ax_label(&fold_label);
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
                    .role(gpui::Role::Button)
                    .aria_label(fold_label.clone())
                    .on_click(emit(CodeBlockAction::Unfold))
                    .child(icon(IconName::ChevronDown).size(px(FOLD_GLYPH)))
                    .child(fold_label),
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
                    let note_label = format!("Add note on line {target_line}");
                    record_ax_label(&note_label);
                    row = row.role(gpui::Role::Button).aria_label(note_label).on_click(move |e, w, cx| add(e, w, cx));
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

#[cfg(test)]
mod tests {
    use super::{
        code_block, code_scroll_retries_exhausted, code_scroll_target, highlight_contains, line_cache_key,
        line_row_index, runnable_command, scroll_request_pending, virtual_row_count, CODE_SCROLL_ATTEMPTS,
        CODE_VIRTUALIZE_AT,
    };
    use gpui::prelude::*;

    #[test]
    fn shell_languages_run_whole_and_unchanged() {
        for lang in ["sh", "bash", "zsh", "shell", "terminal"] {
            assert_eq!(runnable_command(lang, "npm test -- --watch"), Some("npm test -- --watch".to_string()), "lang `{lang}`");
        }
    }

    #[test]
    fn shell_blocks_keep_surrounding_blank_lines() {
        assert_eq!(runnable_command("bash", "\n  git status\n\n"), Some("\n  git status\n\n".to_string()));
    }

    #[test]
    fn untagged_all_prompt_blocks_strip_the_prefix() {
        assert_eq!(runnable_command("", "$ git status\n$ git diff --stat"), Some("git status\ngit diff --stat".to_string()));
    }

    #[test]
    fn untagged_blocks_ignore_blank_lines_around_prompts() {
        assert_eq!(runnable_command("", "\n\n$ echo hi\n   \n$ exit\n\n"), Some("echo hi\nexit".to_string()));
    }

    #[test]
    fn untagged_blocks_with_recorded_output_are_not_runnable() {
        assert_eq!(runnable_command("", "$ git status\nOn branch main\n$ git diff"), None);
    }

    #[test]
    fn untagged_plain_code_is_not_runnable() {
        assert_eq!(runnable_command("", "let x = 1;\nprintln!(\"{x}\");"), None);
    }

    #[test]
    fn console_blocks_keep_only_the_prompt_lines() {
        assert_eq!(runnable_command("console", "$ npm test\n42 passing\n$ npm run lint\nclean"), Some("npm test\nnpm run lint".to_string()));
    }

    #[test]
    fn console_blocks_ignore_blank_lines() {
        assert_eq!(runnable_command("console", "\n$ echo hi\n\n"), Some("echo hi".to_string()));
    }

    #[test]
    fn other_languages_are_not_runnable() {
        for lang in ["python", "rust", "typescript", "json", "diff", "text", "Sh", "SHELL"] {
            assert_eq!(runnable_command(lang, "$ echo hi"), None, "lang `{lang}`");
        }
    }

    #[test]
    fn empty_code_follows_the_language_rule() {
        assert_eq!(runnable_command("sh", ""), Some(String::new()));
        assert_eq!(runnable_command("console", ""), Some(String::new()));
        assert_eq!(runnable_command("", ""), Some(String::new()));
        assert_eq!(runnable_command("python", ""), None);
    }

    #[test]
    fn highlight_range_covers_its_lines_and_nothing_else() {
        let range = 40..45u32;
        for line in 40..45 {
            assert!(highlight_contains(&range, line), "line {line}");
        }
        for line in [1, 39, 45, 46, 400] {
            assert!(!highlight_contains(&range, line), "line {line}");
        }
        assert!(!highlight_contains(&(45..45u32), 45), "empty range highlights nothing");
    }

    #[test]
    fn displayed_lines_map_to_row_indices() {
        assert_eq!(line_row_index(40, 1), Some(39));
        assert_eq!(line_row_index(1, 1), Some(0));
        assert_eq!(line_row_index(44, 44), Some(0));
        assert_eq!(line_row_index(43, 44), None);
    }

    #[test]
    fn virtualised_row_count_stays_bounded_for_long_files() {
        // A 10k-line file keeps only the viewport plus overdraw alive.
        assert!(virtual_row_count(10_000, 20) <= 20 + 2 * super::CODE_VIRTUAL_OVERDRAW);
        assert_eq!(virtual_row_count(10_000, 20), 20 + 2 * super::CODE_VIRTUAL_OVERDRAW);
        // A block shorter than the window keeps every row.
        assert_eq!(virtual_row_count(CODE_VIRTUALIZE_AT, 10_000), CODE_VIRTUALIZE_AT);
        assert_eq!(virtual_row_count(7, 20), 7);
    }

    #[test]
    fn scroll_target_prefers_the_explicit_line_then_the_highlight() {
        assert_eq!(code_scroll_target(Some(5000), &Some(40..45), 1, 10_000), Some(4999));
        assert_eq!(code_scroll_target(None, &Some(40..45), 1, 10_000), Some(39));
        assert_eq!(code_scroll_target(None, &None, 1, 10_000), None);
        assert_eq!(code_scroll_target(None, &Some(45..45), 1, 10_000), None);
        // Clamped to the block, and lines above the block start at row 0.
        assert_eq!(code_scroll_target(Some(99_999), &None, 1, 10_000), Some(9999));
        assert_eq!(code_scroll_target(Some(1), &None, 40, 10_000), Some(0));
    }

    #[test]
    fn scroll_token_rearms_the_same_line() {
        // Unseen: fires. Settled with the same token: quiet. Same line with
        // a new token: fires again.
        assert!(scroll_request_pending(42, 0, None));
        assert!(!scroll_request_pending(42, 0, Some((42, 0))));
        assert!(scroll_request_pending(42, 1, Some((42, 0))));
        assert!(scroll_request_pending(43, 0, Some((42, 0))));
    }

    #[test]
    fn line_cache_key_only_changes_with_the_text() {
        let code = "fn a() {}\nfn b() {}\n";
        assert_eq!(line_cache_key(code), line_cache_key(code));
        assert_eq!(line_cache_key(code).1, code.len());
        assert_ne!(line_cache_key(code), line_cache_key("fn a() {}\nfn c() {}\n"));
        assert_ne!(line_cache_key(code), line_cache_key("fn a() {}\n"));
    }

    /// Ten thousand short lines, past the virtualisation threshold.
    fn ten_k_lines() -> gpui::SharedString {
        let mut code = String::with_capacity(10_000 * 24);
        for i in 0..10_000usize {
            use std::fmt::Write;
            let _ = writeln!(code, "const v{i:05} = {i};");
        }
        code.into()
    }

    /// Drawn line numbers recorded by the virtual row builder, deduped:
    /// only built (visible) rows record, so this is the mounted row set.
    fn drawn_lines() -> Vec<u32> {
        let mut lines: Vec<u32> = crate::data::take_ax_labels()
            .iter()
            .filter_map(|label| label.strip_prefix("line ").and_then(|n| n.parse().ok()))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    struct FillHost {
        code: gpui::SharedString,
        line: u32,
        token: u64,
        seen: std::rc::Rc<std::cell::RefCell<Vec<gpui::Bounds<gpui::Pixels>>>>,
    }

    impl gpui::Render for FillHost {
        fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
            let seen = self.seen.clone();
            gpui_kit::base::v_flex()
                .size_full()
                .on_children_prepainted(move |bounds, _, _| {
                    *seen.borrow_mut() = bounds;
                })
                .child(
                    code_block("l6fix-fill", "src/checkout/big.ts", self.code.clone())
                        .language("typescript")
                        .fill(true)
                        .scroll_to_line(self.line)
                        .scroll_token(self.token),
                )
        }
    }

    #[test]
    fn code_scroll_retries_stop_after_ten_unseen_frames() {
        assert!(!code_scroll_retries_exhausted(0));
        assert!(!code_scroll_retries_exhausted(CODE_SCROLL_ATTEMPTS - 1));
        assert!(code_scroll_retries_exhausted(CODE_SCROLL_ATTEMPTS));
        assert!(code_scroll_retries_exhausted(CODE_SCROLL_ATTEMPTS + 40));
    }

    /// Draws pumped per scroll request: the retry bound plus two spare
    /// frames, so the request always settles inside the loop. Fixed — the
    /// loop below always ends, and with it the test.
    const SCROLL_SETTLE_DRAWS: usize = CODE_SCROLL_ATTEMPTS + 2;

    /// One settled frame without ever parking: draw, then deliver the
    /// next-frame callback the scroll request registered. The caller bounds
    /// the loop, so a render that kept asking for frames would fail loudly
    /// instead of hanging the suite.
    fn pump_frame(vcx: &mut gpui::VisualTestContext) {
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        vcx.update(|window, cx| {
            let _ = window.simulate_next_frame(cx);
        });
    }

    /// Requests (`line`, `token`) on the fill host, pumps a bounded number
    /// of frames so the scroll request settles, and returns the mounted
    /// (built) line numbers.
    fn draw_fill(
        vcx: &mut gpui::VisualTestContext,
        host: &gpui::Entity<FillHost>,
        line: u32,
        token: u64,
    ) -> Vec<u32> {
        host.update(vcx, |host, cx| {
            host.line = line;
            host.token = token;
            cx.notify();
        });
        crate::data::take_ax_labels();
        for _ in 0..SCROLL_SETTLE_DRAWS {
            pump_frame(vcx);
        }
        drawn_lines()
    }

    /// A 10k-line filled block mounts a bounded row set, starts at the
    /// requested line, and fills its parent instead of the 320 px strip.
    #[gpui::test]
    fn filled_10k_block_mounts_bounded_rows_at_line_5000(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let code = ten_k_lines();
        let (host, vcx) = cx.add_window_view(|_, _| FillHost { code, line: 5000, token: 0, seen: seen.clone() });
        vcx.simulate_resize(gpui::size(gpui::px(800.0), gpui::px(600.0)));
        crate::data::arm_ax_probe(true);
        let lines = draw_fill(vcx, &host, 5000, 0);
        crate::data::arm_ax_probe(false);
        assert!(!lines.is_empty(), "the virtual list must build rows");
        assert!(
            lines.len() <= virtual_row_count(10_000, 60) * 4,
            "a 10k-line file mounted {} rows, past the bounded budget",
            lines.len()
        );
        assert!(lines.contains(&5000), "scroll_to_line(5000) must make row 5000 visible: {lines:?}");

        // The filled block takes the window height (600 px), not the fixed
        // 320 px strip: header (30 px) plus the filling list.
        let painted = seen.borrow();
        assert!(!painted.is_empty(), "the filled block must paint");
        let height = painted.iter().map(|b| f32::from(b.size.height)).fold(0.0, f32::max);
        assert!(
            height > 400.0 && (height - 600.0).abs() < 24.0,
            "the filled block painted {height}px tall, not its parent's 600 px"
        );
    }

    /// Repeating the same line with a new token re-scrolls: after moving the
    /// request token on, row 5000 is visible again.
    #[gpui::test]
    fn repeated_scroll_request_with_a_new_token_rescrolls(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let code = ten_k_lines();
        let (host, vcx) = cx.add_window_view(|_, _| FillHost { code, line: 5000, token: 0, seen: seen.clone() });
        vcx.simulate_resize(gpui::size(gpui::px(800.0), gpui::px(600.0)));
        crate::data::arm_ax_probe(true);
        let first = draw_fill(vcx, &host, 5000, 0);
        assert!(first.contains(&5000), "first request must reveal row 5000");

        // Move away with a new request: row 100 shows, row 5000 leaves.
        let away = draw_fill(vcx, &host, 100, 1);
        assert!(away.contains(&100), "the move-away request must reveal row 100");
        assert!(!away.contains(&5000), "row 5000 must leave after scrolling to row 100");

        // Same line as before, new token: re-scrolls to row 5000.
        let lines = draw_fill(vcx, &host, 5000, 2);
        crate::data::arm_ax_probe(false);
        assert!(lines.contains(&5000), "repeated request with a new token must re-scroll to row 5000");
        assert!(!lines.contains(&100), "row 100 must leave after re-scrolling to row 5000");
    }
}
