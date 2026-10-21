//! The turns (D4, docs/41 §2.2): the person's bubble — Raised, radius 12,
//! with what they sent above it (64 px picture tiles, 30 px file chips), a
//! quotation at its top, or a Browse annotation batch in its place — and the
//! assistant's answer, prose on the sheet at the full measure. Neither shows
//! anything at rest: hovering a turn reveals its own controls (edit · copy
//! · age under the bubble; one copy and the footer under the answer) in a
//! rail whose height is always reserved, so nothing shifts.

use aui_icons::{icon, IconName};
use aui_motion::{icon_morph, tween, IconMorph, Tween};
use aui_protocol::{Attachment, AttachmentKind, TurnMeta};
use aui_tokens::{scale, ActiveAui, AuiStyled, Easing, Palette};
use gpui::{
    div, prelude::*, px, relative, App, Bounds, ElementId, IntoElement, Pixels, SharedString,
    TextRun, Window,
};
use gpui_kit::base::ElementExt;
use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use crate::transcript::{caret_top_in_line, ProseStyle, CARET_H, CARET_MARGIN_LEFT, CARET_W};
use crate::transcript::{LinkTarget, MessageSelection, SelectionHandler, SpanEvent, SpanHandler, TextSelection, last_block_runs, markdown, markdown_selected_text, markdown_selected_text_settled, message_selected_text, message_selected_text_settled, CodeBlockAction, LinkHandler};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, icon_content_button, ButtonSize};
use crate::util::{interaction_flags, TrackInteraction};

/// The bubble's column: at most 78 % of the measure, right-aligned.
const USER_MAX: f32 = 0.78;
/// Sent attachments sit 4 above the bubble, 6 apart.
const ATTACH_ABOVE: f32 = 4.0;
const ATTACH_GAP: f32 = 6.0;
/// Light keeps FF's tailed bubble until light is designed.
const BUBBLE_RADIUS: f32 = 14.0;
/// D4: Raised, radius 12, padding 9 / 13.
const BUBBLE_RADIUS_V3: f32 = 12.0;
const BUBBLE_PAD_Y: f32 = 9.0;
const BUBBLE_PAD_X: f32 = 13.0;
const BUBBLE_TEXT: f32 = BODY_TEXT;
/// The rail under a bubble: 24 px, always reserved; edit · copy as 28 px
/// icon buttons (glyph 12) then the age, 6 apart.
const RAIL_H: f32 = 24.0;
const RAIL_GAP: f32 = 6.0;
const ACTS_GLYPH: f32 = 12.0;
const ACT_SIZE: f32 = 28.0;
/// Picture tiles: 64 px, radius 6; file chips 30 px.
const TILE: f32 = 64.0;
const TILE_RADIUS: f32 = 6.0;
const PLAY_GLYPH: f32 = 16.0;
const CHIP_H: f32 = 30.0;
const CHIP_PAD_L: f32 = 8.0;
const CHIP_PAD_R: f32 = 10.0;
const CHIP_GAP: f32 = 6.0;
const CHIP_GLYPH: f32 = 13.0;
/// The quote at the top of a bubble: a 2 px rule at white 18 %, the words in
/// caption muted clamped to three lines, "from the reply above" under it.
const QUOTE_RULE: f32 = 2.0;
const QUOTE_RULE_ALPHA: f32 = 0.18;
const QUOTE_PAD_L: f32 = 12.0;
/// The annotation batch: a 120 px screenshot strip with 18 px numbered pins.
const SHOT_H: f32 = 120.0;
const PIN: f32 = 18.0;
/// The assistant answer: body at line 23 on 14.3 (1.6), paragraphs 12 apart
/// (the owner's one rhythm for every answer, 2026-10-14).
/// Body text, at the design size (the app's 1.1 text scale makes it 16 px):
/// the owner's reading scale, 16 / 26 (2026-10-14).
const BODY_TEXT: f32 = 16.0 / 1.1;
const BODY_LH: f32 = 26.0 / 16.0;
const PARAGRAPH_GAP: f32 = 16.0;
/// Under an answer, on hover: one copy button 8 below, the footer 10 below it.
const COPY_TOP: f32 = 8.0;
const FOOTER_TOP: f32 = 10.0;
/// Hover reveals: opacity 80 ms in, 60 ms out, no translation (docs/41 §3).
const REVEAL_IN: Tween = Tween::new(Duration::from_millis(80), Easing::OUT);
const REVEAL_OUT: Tween = Tween::new(Duration::from_millis(60), Easing::OUT);
/// How long the copy button's success check holds before the copy glyph
/// returns: 1.2 s, the same hold the code block header uses. The library owns
/// no timers, so the caller waits this long after the copy intent before it
/// clears the [`UserTurn::copied`] / [`AssistantTurn::copied`] flag it owns.
pub const COPY_HOLD: Duration = Duration::from_millis(1200);
/// File extensions drawn with the video treatment (a File by the wire).
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mov", "m4v", "webm", "mkv", "avi"];

/// Actions on a user turn: edit and copy (resend was removed, docs/41 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserTurnAction {
    /// Edit and resend.
    Edit,
    /// Copy the text.
    Copy,
}

impl UserTurnAction {
    /// The full set, in draw order: edit, copy.
    pub const ALL: &'static [UserTurnAction] = &[UserTurnAction::Edit, UserTurnAction::Copy];
}

/// Actions on an assistant turn: copy, the only one (docs/41 §6 — retry is
/// a typed ask, fork lives in the session menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantTurnAction {
    /// Copy the text.
    Copy,
}

/// The accessible name for one user-turn icon button, naming the action.
fn user_action_label(action: UserTurnAction) -> &'static str {
    match action {
        UserTurnAction::Edit => "Edit message",
        UserTurnAction::Copy => "Copy message",
    }
}

/// The copy ↔ success-check morph for a turn rail's copy button: the same
/// element the code block header shows (copy glyph in the rail's ink, check in
/// `success`, swapping on the swap spring), sampled under `(turn_id,
/// "copy-morph")` so neighbouring turns never share a phase.
fn copy_morph(
    turn_id: &ElementId,
    copied: bool,
    p: &Palette,
    window: &mut Window,
    cx: &mut App,
) -> IconMorph {
    let sample = icon_morph((turn_id.clone(), "copy-morph"), copied, window, cx);
    let size = px(ACTS_GLYPH);
    IconMorph::new(
        sample,
        size,
        icon(IconName::Copy).size(size),
        icon(IconName::CheckBold).size(size).color(p.success),
    )
}

/// The reveal opacity of a turn's hover controls.
fn reveal(id: &ElementId, name: &'static str, hovered: bool, window: &mut Window, cx: &mut App) -> f32 {
    tween((id.clone(), name), if hovered { 1.0f32 } else { 0.0 }, if hovered { REVEAL_IN } else { REVEAL_OUT }, window, cx)
}

/// One Browse annotation batch drawn as the person's turn (docs/41 §2.2):
/// the page's screenshot with numbered pins, the address, one line per note.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationBatch {
    /// The page screenshot on disk; `None` draws the notes without a strip.
    pub screenshot: Option<String>,
    /// The screenshot's size in px, which places the pins on the strip.
    pub size: (f32, f32),
    /// `host/path` of the annotated page.
    pub address: String,
    /// The notes, in pin order.
    pub notes: Vec<AnnotationNote>,
}

/// One annotation: its pin and what the person wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationNote {
    /// The pin's number, from 1.
    pub index: usize,
    /// Where the pin sits on the screenshot, in px of [`AnnotationBatch::size`].
    pub at: (f32, f32),
    /// The element's selector (`div.card.starter`).
    pub selector: String,
    /// The note.
    pub note: String,
}

type UserHandler = std::rc::Rc<dyn Fn(UserTurnAction, &mut Window, &mut App)>;
type AssistantHandler = std::rc::Rc<dyn Fn(AssistantTurnAction, &mut Window, &mut App)>;
type CodeHandler = std::rc::Rc<dyn Fn(CodeBlockAction, &mut Window, &mut App)>;
type IndexHandler = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>;
/// A sent quote was pressed: its source turn and its words.
type QuoteHandler = std::rc::Rc<dyn Fn(String, String, &mut Window, &mut App)>;

/// The person's turn. Build with [`user_turn`].
#[derive(IntoElement)]
pub struct UserTurn {
    id: ElementId,
    markdown: SharedString,
    attachments: Vec<Attachment>,
    annotations: Option<AnnotationBatch>,
    age: Option<SharedString>,
    max_width: f32,
    copied: bool,
    actions: Vec<UserTurnAction>,
    on_action: Option<UserHandler>,
    on_attachment: Option<IndexHandler>,
    on_quote: Option<QuoteHandler>,
    on_link: Option<LinkHandler>,
    selection: Option<TextSelection>,
    on_selection_change: Option<SelectionHandler>,
    span: Option<MessageSelection>,
    span_strength: f32,
    on_span: Option<SpanHandler>,
    on_code_action: Option<CodeHandler>,
}

/// A user turn; `markdown` may carry mentions as inline code (`` `@src/checkout` ``),
/// which render as mention chips.
pub fn user_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> UserTurn {
    UserTurn {
        id: id.into(),
        markdown: markdown.into(),
        attachments: Vec::new(),
        annotations: None,
        age: None,
        max_width: USER_MAX,
        copied: false,
        actions: UserTurnAction::ALL.to_vec(),
        on_action: None,
        on_attachment: None,
        on_quote: None,
        on_link: None,
        selection: None,
        on_selection_change: None,
        span: None,
        span_strength: 1.0,
        on_span: None,
        on_code_action: None,
    }
}

impl UserTurn {
    /// What was sent with the message: pictures and videos as 64 px tiles,
    /// files as 30 px chips above the bubble; a [`AttachmentKind::Quote`] as
    /// the quote block at the bubble's top. Pasted [`AttachmentKind::Text`]
    /// draws as a file chip.
    pub fn attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// Draws the turn as a Browse annotation batch instead of the bubble's
    /// own text.
    pub fn annotations(mut self, batch: Option<AnnotationBatch>) -> Self {
        self.annotations = batch;
        self
    }

    /// The how-long-ago caption in the rail, after edit · copy, formatted
    /// by [`format_age`] on state change. A turn with no reported timestamp
    /// never calls this and its rail shows the buttons alone.
    pub fn age(mut self, age: impl Into<SharedString>) -> Self {
        self.age = Some(age.into());
        self
    }

    /// The column's cap as a fraction of the transcript width, for a
    /// consumer whose column is narrower than the chat measure. Values
    /// outside `(0, 1]` (and NaN) fall back to the 78 % default.
    pub fn max_width_fraction(mut self, fraction: f32) -> Self {
        self.max_width = if fraction.is_finite() && fraction > 0.0 && fraction <= 1.0 { fraction } else { USER_MAX };
        self
    }

    /// Whether the copy button shows its success check; the caller sets it
    /// on the copy intent and clears it after [`COPY_HOLD`].
    pub fn copied(mut self, copied: bool) -> Self {
        self.copied = copied;
        self
    }

    /// The rail's buttons, in draw order; defaults to [`UserTurnAction::ALL`].
    pub fn actions(mut self, actions: &[UserTurnAction]) -> Self {
        self.actions = actions.to_vec();
        self
    }

    /// Rail handler.
    pub fn on_action(mut self, f: impl Fn(UserTurnAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }

    /// A sent attachment was pressed: its index in
    /// [`attachments`](Self::attachments). The host opens a picture in the
    /// right pane and reveals a file in Files.
    pub fn on_attachment(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_attachment = Some(std::rc::Rc::new(f));
        self
    }

    /// The quote at the bubble's top was pressed: the id of the turn it was
    /// quoted from, to scroll back to.
    pub fn on_quote(mut self, f: impl Fn(String, String, &mut Window, &mut App) + 'static) -> Self {
        self.on_quote = Some(std::rc::Rc::new(f));
        self
    }

    /// Link-click handler, passed through to the markdown body.
    pub fn on_link(mut self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(std::rc::Rc::new(f));
        self
    }

    /// The stored selection the markdown body highlights.
    pub fn selection(mut self, selection: Option<&TextSelection>) -> Self {
        self.selection = selection.cloned();
        self
    }

    /// Selection intents, passed straight through to the inner `markdown(...)`.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::rc::Rc::new(f));
        self
    }

    /// The stored cross-cell span the markdown body highlights.
    pub fn span_selection(mut self, selection: Option<&MessageSelection>) -> Self {
        self.span = selection.cloned();
        self
    }

    /// The span's tint at `strength` (0–1): see
    /// [`Markdown::selection_strength`](super::markdown::Markdown::selection_strength).
    pub fn span_strength(mut self, strength: f32) -> Self {
        self.span_strength = strength;
        self
    }

    /// Cross-cell selection events, passed straight through to the inner
    /// `markdown(...)`.
    pub fn on_span_event(mut self, f: impl Fn(SpanEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_span = Some(std::rc::Rc::new(f));
        self
    }

    /// Code-block intents from the bubble's markdown fences.
    pub fn on_code_action(
        mut self,
        f: impl Fn(CodeBlockAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_code_action = Some(std::rc::Rc::new(f));
        self
    }
}

/// Copies the selected text out of a turn's `markdown_source` without
/// re-rendering: the slice of the holding cell's shaped text, or `None` when
/// the key addresses no cell or the range is empty. The app puts this on the
/// clipboard on ⌘C; the keybinding stays with the app.
pub fn turn_selected_text(markdown_source: &str, selection: &TextSelection) -> Option<String> {
    markdown_selected_text(markdown_source, selection)
}

/// [`turn_selected_text`] for a settled turn: reads the same settled-aware
/// blocks the turn paints. Pass `!streaming` for assistant turns, `true` for
/// user turns (mirroring the entry above).
pub fn turn_selected_text_settled(
    markdown_source: &str,
    selection: &TextSelection,
    settled: bool,
) -> Option<String> {
    markdown_selected_text_settled(markdown_source, selection, settled)
}

/// Copies the selected text across `selection`'s cells in document order
/// without re-rendering; see [`message_selected_text`]. The app puts this on
/// the clipboard on ⌘C when it holds a span; the keybinding stays with the
/// app.
pub fn turn_span_selected_text(
    markdown_source: &str,
    selection: &MessageSelection,
) -> Option<String> {
    message_selected_text(markdown_source, selection)
}

/// [`turn_span_selected_text`] for a settled turn: reads the same
/// settled-aware blocks the turn paints. Pass `!streaming` for assistant
/// turns, `true` for user turns (mirroring the entry above).
pub fn turn_span_selected_text_settled(
    markdown_source: &str,
    selection: &MessageSelection,
    settled: bool,
) -> Option<String> {
    message_selected_text_settled(markdown_source, selection, settled)
}

/// Prose style shared by both turns: inline code on `code_bg` in the mono face.
fn prose_style(
    p: &Palette,
    size: f32,
    line_height: f32,
    code_bg: gpui::Hsla,
    code_ink: gpui::Hsla,
) -> ProseStyle {
    ProseStyle {
        ink: p.ink,
        code_ink,
        code_bg,
        size,
        line_height,
        paragraph_gap: PARAGRAPH_GAP,
    }
}

/// Whether an attachment draws with the video treatment: a File whose name
/// ends in a video extension.
pub fn is_video(attachment: &Attachment) -> bool {
    attachment.kind == AttachmentKind::File
        && attachment
            .name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| VIDEO_EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(ext)))
}

/// `JSONL · 2.4 KB`: a file chip's meta — the extension upper-cased and the
/// size — or the host's own `meta` when it set one.
pub fn file_meta(attachment: &Attachment) -> Option<String> {
    if let Some(meta) = &attachment.meta {
        return Some(meta.clone());
    }
    let ext = attachment.name.rsplit_once('.').map(|(_, ext)| ext.to_uppercase());
    let size = attachment.size_bytes.map(human_size);
    match (ext, size) {
        (Some(ext), Some(size)) => Some(format!("{ext} · {size}")),
        (Some(ext), None) => Some(ext),
        (None, Some(size)) => Some(size),
        (None, None) => None,
    }
}

/// `2.4 KB`, `1.1 MB`, `512 B`.
fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// A 64 px picture tile: the picture (cover) when its path is known, else
/// the image glyph on Raised; a video adds the play glyph over a 25 % black
/// scrim and its length (`meta`) in a dark cap.
fn picture_tile(p: &Palette, id: ElementId, attachment: &Attachment, video: bool) -> gpui::Stateful<gpui::Div> {
    let mut tile = div()
        .id(id)
        .relative()
        .flex_none()
        .size(px(TILE))
        .rounded(px(TILE_RADIUS))
        .overflow_hidden()
        .bg(p.surface_3);
    let picture = attachment.path.as_ref().filter(|_| !video).map(std::path::PathBuf::from).filter(|path| path.is_file());
    tile = match picture {
        Some(path) => tile.child(gpui::img(path).object_fit(gpui::ObjectFit::Cover).absolute().top_0().left_0().size(px(TILE))),
        None if video => tile,
        None => tile.flex().items_center().justify_center().child(icon(IconName::Image).size(px(PLAY_GLYPH)).color(p.ink_2)),
    };
    if video {
        tile = tile
            .child(div().absolute().inset_0().bg(gpui::black().alpha(0.25)).flex().items_center().justify_center().child(icon(IconName::Play).size(px(PLAY_GLYPH)).color(gpui::white())))
            .children(attachment.meta.clone().map(|length| {
                div()
                    .absolute()
                    .right(px(4.0))
                    .bottom(px(4.0))
                    .px(px(3.0))
                    .rounded(px(3.0))
                    .bg(gpui::rgba(0x121215d9))
                    .font_family(scale::FONT_MONO)
                    .text_px(10.0)
                    .line_height(relative(1.4))
                    .text_color(gpui::white())
                    .child(length)
            }));
    }
    // The edge over the picture, so it reads on any image.
    tile.child(div().absolute().inset_0().rounded(px(TILE_RADIUS)).shadow(p.edge()))
}

/// A 30 px file chip: Raised, radius 6, the edge; the file glyph, the name,
/// `TYPE · size` in caption muted.
fn file_chip(p: &Palette, id: ElementId, attachment: &Attachment) -> gpui::Stateful<gpui::Div> {
    let mut chip = h_flex()
        .id(id)
        .flex_none()
        .h(px(CHIP_H))
        .pl(px(CHIP_PAD_L))
        .pr(px(CHIP_PAD_R))
        .gap(px(CHIP_GAP))
        .rounded(px(TILE_RADIUS))
        .bg(p.surface_3)
        .shadow(p.edge())
        .ui(scale::FS_13)
        .text_color(p.ink)
        .child(icon(IconName::File).size(px(CHIP_GLYPH)).color(p.ink_2))
        .child(div().max_w(px(240.0)).truncate().child(attachment.name.clone()));
    if let Some(meta) = file_meta(attachment) {
        chip = chip.child(div().flex_none().ui(scale::FS_12).text_color(p.ink_2).child(meta));
    }
    chip
}

/// A quote attachment's code language: `meta` = `code` or `code:<lang>`
/// marks a code quote (the quote flow, docs/43 §0); `None` is prose.
pub fn quote_code(a: &Attachment) -> Option<&str> {
    let meta = a.meta.as_deref()?;
    if meta == "code" {
        Some("")
    } else {
        meta.strip_prefix("code:")
    }
}

/// The quote block at the top of a bubble (docs/43 §0, at the shipped
/// scale): a 2 px rule at 18 %, the words 14.3 / 22 muted clamped to 3
/// lines, "from the reply above" faint 12.1 (−4 above, 8 below). A code
/// quote (`code` = its language, or "" when unknown) is mono 12.65 / 20,
/// 4 lines, "from the reply above · rust". A click scrolls to the source.
fn quote_block(p: &Palette, id: ElementId, words: &str, code: Option<&str>, source: Option<String>, on_quote: Option<QuoteHandler>) -> impl IntoElement {
    // Between FG and muted: #C9C9D0 on dark, #3F3F46 on light (docs/43 §0).
    let code_ink: gpui::Hsla = if p.is_dark() { gpui::rgb(0xC9C9D0).into() } else { gpui::rgb(0x3F3F46).into() };
    let text = match code {
        Some(_) => div().font_family(scale::FONT_MONO).text_px(12.65).line_height(px(20.0)).text_color(code_ink).whitespace_nowrap().overflow_hidden().line_clamp(4).max_h(px(80.0)),
        None => div().font_family(scale::FONT_UI).text_px(14.3).line_height(px(22.0)).text_color(p.fg_muted).line_clamp(3).max_h(px(66.0)).overflow_hidden(),
    };
    let source_line = match code.filter(|l| !l.is_empty()) {
        Some(lang) => format!("from the reply above · {lang}"),
        None => "from the reply above".to_string(),
    };
    // The canvas: the quote 1 / 0 / 1 / 12 with 8 below it; the source line
    // pulled up 4 (so 4 below the quote), 14 in, 8 above the person's words.
    let mut block = v_flex()
        .id(id)
        .w_full()
        .mb(px(8.0))
        .child(
            div()
                .w_full()
                .py(px(1.0))
                .border_l(px(QUOTE_RULE))
                .border_color(p.fg.alpha(QUOTE_RULE_ALPHA))
                .pl(px(QUOTE_PAD_L))
                .child(text.child(words.to_owned())),
        )
        .child(div().pl(px(QUOTE_PAD_L + QUOTE_RULE)).mt(px(4.0)).font_family(scale::FONT_UI).text_px(12.1).line_height(px(16.0)).text_color(p.ink_4).child(source_line));
    if let (Some(source), Some(on_quote)) = (source, on_quote) {
        let words = words.to_owned();
        crate::data::record_ax_label("Show the quoted reply");
        block = block
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label("Show the quoted reply")
            .on_click(move |_, w, cx| on_quote(source.clone(), words.clone(), w, cx));
    }
    block
}

/// The annotation batch drawn inside the bubble.
fn annotation_body(p: &Palette, id: &ElementId, batch: &AnnotationBatch, strip_w: f32) -> impl IntoElement {
    let mut col = v_flex().w_full().gap(px(8.0));
    if let Some(shot) = batch.screenshot.as_ref().map(std::path::PathBuf::from).filter(|path| path.is_file()) {
        // The strip shows the page's top: the picture at the strip's width,
        // clipped to 120 px (object-position top).
        let (w, h) = batch.size;
        let scale = if w > 0.0 { strip_w / w } else { 1.0 };
        let mut strip = div()
            .relative()
            .w_full()
            .h(px(SHOT_H))
            .rounded(px(TILE_RADIUS))
            .overflow_hidden()
            .bg(gpui::white())
            .child(div().absolute().top_0().left_0().w(px(strip_w)).h(px(h * scale)).child(gpui::img(shot).size_full().object_fit(gpui::ObjectFit::Fill)));
        for note in &batch.notes {
            let (x, y) = (note.at.0 * scale, note.at.1 * scale);
            if y > SHOT_H {
                continue;
            }
            strip = strip.child(
                div()
                    .absolute()
                    .left(px(x - PIN / 2.0))
                    .top(px(y - PIN / 2.0))
                    .size(px(PIN))
                    .rounded_full()
                    .bg(p.ink)
                    .shadow(vec![gpui::BoxShadow { color: gpui::black().alpha(0.4), offset: gpui::point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(1.0), inset: false }])
                    .flex()
                    .items_center()
                    .justify_center()
                    .ui(scale::FS_11)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(p.bg)
                    .child(note.index.to_string()),
            );
        }
        col = col.child(strip.child(div().absolute().inset_0().rounded(px(TILE_RADIUS)).shadow(p.edge())));
    }
    let count = batch.notes.len();
    col = col.child(
        div()
            .ui(scale::FS_12)
            .text_color(p.ink_2)
            .child(format!("{count} annotation{} · {}", if count == 1 { "" } else { "s" }, batch.address)),
    );
    for note in &batch.notes {
        col = col.child(
            h_flex()
                .id((id.clone(), SharedString::from(format!("note-{}", note.index))))
                .w_full()
                .items_start()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_none()
                        .mt(px(1.0))
                        .size(px(PIN))
                        .rounded_full()
                        .bg(p.selected_overlay())
                        .flex()
                        .items_center()
                        .justify_center()
                        .ui(scale::FS_11)
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(p.ink)
                        .child(note.index.to_string()),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(div().mono(scale::FS_11).text_color(p.ink_2).child(note.selector.clone()))
                        .child(div().ui(scale::FS_12).text_color(p.ink).child(note.note.clone())),
                ),
        );
    }
    col
}

impl RenderOnce for UserTurn {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let rail_opacity = reveal(&id, "rail", flags.hovered, window, cx);

        // The column is definite (`w_full` capped at `USER_MAX`) so the
        // percentage resolves; `items_end` keeps a short bubble hugging the
        // right, and `ml_auto` puts the capped column itself on the right
        // (without it a long message reads as left-aligned).
        let mut col = v_flex()
            .id(id.clone())
            .relative()
            .w_full()
            .max_w(relative(self.max_width))
            .ml_auto()
            .items_end()
            .track_interaction(&state);

        // What was sent with it, above the bubble: tiles then chips.
        type Indexed<'a> = Vec<(usize, &'a Attachment)>;
        let (quotes, sent): (Indexed, Indexed) = self.attachments.iter().enumerate().partition(|(_, a)| a.kind == AttachmentKind::Quote);
        if !sent.is_empty() {
            let mut strip = h_flex().flex_wrap().justify_end().items_end().gap(px(ATTACH_GAP)).mb(px(ATTACH_ABOVE));
            for (index, attachment) in sent {
                let child_id: ElementId = (id.clone(), SharedString::from(format!("sent-{index}"))).into();
                let video = is_video(attachment);
                let mut el = if attachment.kind == AttachmentKind::Image || video {
                    picture_tile(&p, child_id, attachment, video)
                } else {
                    file_chip(&p, child_id, attachment)
                };
                if let Some(on_attachment) = self.on_attachment.clone() {
                    let label = format!("Open {}", attachment.name);
                    crate::data::record_ax_label(&label);
                    el = el.cursor_pointer().role(gpui::Role::Button).aria_label(label).on_click(move |_, w, cx| on_attachment(index, w, cx));
                }
                strip = strip.child(el);
            }
            col = col.child(strip);
        }

        let mut bubble = v_flex()
            // Never wider than the column: `min_w(0)` lets the bubble shrink
            // past its longest unwrapped line so the markdown wraps.
            .max_w(relative(1.0))
            .min_w(px(0.0))
            .overflow_hidden()
            .py(px(BUBBLE_PAD_Y))
            .px(px(BUBBLE_PAD_X))
            .when(p.is_dark(), |d| d.rounded(px(BUBBLE_RADIUS_V3)).shadow(p.edge()))
            .when(!p.is_dark(), |d| d.rounded(px(BUBBLE_RADIUS)).rounded_br(px(scale::R_XS)))
            .bg(p.surface_3)
            .ui(BUBBLE_TEXT)
            .text_color(p.ink);
        for (index, quote) in quotes {
            bubble = bubble.child(quote_block(&p, (id.clone(), SharedString::from(format!("quote-{index}"))).into(), &quote.name, quote_code(quote), quote.quote_of.clone(), self.on_quote.clone()));
        }
        if let Some(batch) = &self.annotations {
            // An annotation batch is wide: the bubble takes the column.
            bubble = bubble.w_full().child(annotation_body(&p, &id, batch, 600.0));
        } else {
            bubble = bubble.child({
                let mut body = markdown((id.clone(), "text"), self.markdown.clone(), prose_style(&p, BUBBLE_TEXT, scale::LH_UI, p.line, p.accent_ink));
                // A user message is complete by definition: a settled
                // whole-JSON message draws its `json` code block.
                body = body.settled(true);
                if let Some(on_link) = self.on_link.clone() {
                    body = body.on_link(move |target, window, cx| on_link(target, window, cx));
                }
                if let Some(on_code) = self.on_code_action.clone() {
                    body = body.on_code_action(move |action, window, cx| on_code(action, window, cx));
                }
                body = body.selection(self.selection.as_ref());
                if let Some(on_change) = self.on_selection_change.clone() {
                    body = body.on_selection_change(move |next, window, cx| on_change(next, window, cx));
                }
                body = body.span_selection(self.span.as_ref()).selection_strength(self.span_strength);
                if let Some(on_span) = self.on_span.clone() {
                    body = body.on_span_event(move |event, window, cx| on_span(event, window, cx));
                }
                body
            });
        }
        col = col.child(bubble);

        // The rail: always 24 px tall, its contents only while hovered.
        let mut rail = h_flex().h(px(RAIL_H)).mt(px(4.0)).gap(px(RAIL_GAP)).items_center().opacity(rail_opacity);
        if rail_opacity <= 0.001 {
            rail = rail.invisible();
        }
        let mut buttons = h_flex().gap(px(0.0)).my(px(-(ACT_SIZE - RAIL_H) / 2.0));
        for action in &self.actions {
            let action = *action;
            let name = match action {
                UserTurnAction::Edit => "edit",
                UserTurnAction::Copy => "copy",
            };
            let mut b = match action {
                UserTurnAction::Copy => {
                    let morph = copy_morph(&id, self.copied, &p, window, cx);
                    icon_content_button((id.clone(), name), morph).ghost().size(ButtonSize::Md).accessibility_label(user_action_label(action))
                }
                UserTurnAction::Edit => {
                    icon_button((id.clone(), name), IconName::Edit).ghost().size(ButtonSize::Md).icon_size(px(ACTS_GLYPH)).accessibility_label(user_action_label(action))
                }
            };
            if let Some(h) = self.on_action.clone() {
                b = b.on_click(move |_, w, cx| h(action, w, cx));
            }
            buttons = buttons.child(b);
        }
        rail = rail.child(buttons);
        if let Some(age) = self.age {
            rail = rail.child(div().font_family(scale::FONT_MONO).text_px(scale::FS_11).line_height(relative(1.0)).text_color(p.ink_4).child(age));
        }
        col.child(rail)
    }
}

/// The assistant's turn. Build with [`assistant_turn`].
#[derive(IntoElement)]
pub struct AssistantTurn {
    id: ElementId,
    markdown: SharedString,
    streaming: bool,
    footer: Vec<SharedString>,
    copied: bool,
    rail: bool,
    image_root: Option<SharedString>,
    on_action: Option<AssistantHandler>,
    on_link: Option<LinkHandler>,
    selection: Option<TextSelection>,
    on_selection_change: Option<SelectionHandler>,
    span: Option<MessageSelection>,
    span_strength: f32,
    on_span: Option<SpanHandler>,
    on_code_action: Option<CodeHandler>,
}

/// An assistant turn rendering `markdown`.
pub fn assistant_turn(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> AssistantTurn {
    AssistantTurn {
        id: id.into(),
        markdown: markdown.into(),
        streaming: false,
        footer: Vec::new(),
        copied: false,
        rail: true,
        image_root: None,
        on_action: None,
        on_link: None,
        selection: None,
        on_selection_change: None,
        span: None,
        span_strength: 1.0,
        on_span: None,
        on_code_action: None,
    }
}

impl AssistantTurn {
    /// Draws the solid caret after the text while chunks arrive.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// The footer: model · time · tokens · reasoning.
    pub fn meta(mut self, meta: TurnMeta) -> Self {
        self.footer = footer_items(&meta);
        self
    }

    /// The how-long-ago cell at the end of the footer, formatted by
    /// [`format_age`] on state change.
    pub fn age(mut self, age: impl Into<SharedString>) -> Self {
        self.footer.push(age.into());
        self
    }

    /// Whether the copy button shows its success check; the caller sets it
    /// on the copy intent and clears it after [`COPY_HOLD`].
    pub fn copied(mut self, copied: bool) -> Self {
        self.copied = copied;
        self
    }

    /// The footer cells, already formatted, for a caller that keeps them.
    /// An empty list draws no footer.
    pub fn footer(mut self, cells: Vec<SharedString>) -> Self {
        self.footer = cells;
        self
    }

    /// Whether the answer carries its hover rail (copy + footer). A block
    /// that is not the turn's closing text draws none; default `true`.
    pub fn rail(mut self, rail: bool) -> Self {
        self.rail = rail;
        self
    }

    /// Where relative image paths in the answer resolve (the workspace).
    pub fn image_root(mut self, root: Option<SharedString>) -> Self {
        self.image_root = root;
        self
    }

    /// Copy handler.
    pub fn on_action(
        mut self,
        f: impl Fn(AssistantTurnAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }

    /// Link-click handler, passed through to the markdown body.
    pub fn on_link(mut self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(std::rc::Rc::new(f));
        self
    }

    /// The stored selection the markdown body highlights.
    pub fn selection(mut self, selection: Option<&TextSelection>) -> Self {
        self.selection = selection.cloned();
        self
    }

    /// Selection intents, passed straight through to the inner `markdown(...)`.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::rc::Rc::new(f));
        self
    }

    /// The stored cross-cell span the markdown body highlights.
    pub fn span_selection(mut self, selection: Option<&MessageSelection>) -> Self {
        self.span = selection.cloned();
        self
    }

    /// The span's tint at `strength` (0–1): see
    /// [`Markdown::selection_strength`](super::markdown::Markdown::selection_strength).
    pub fn span_strength(mut self, strength: f32) -> Self {
        self.span_strength = strength;
        self
    }

    /// Cross-cell selection events, passed straight through to the inner
    /// `markdown(...)`.
    pub fn on_span_event(mut self, f: impl Fn(SpanEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_span = Some(std::rc::Rc::new(f));
        self
    }

    /// Code-block intents from the turn's markdown fences.
    pub fn on_code_action(
        mut self,
        f: impl Fn(CodeBlockAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_code_action = Some(std::rc::Rc::new(f));
        self
    }
}

/// How many measured trailing widths [`last_line_width`] keeps. One entry per
/// streaming turn is the realistic load (only the closing block of a turn
/// that is still streaming is measured); the bound leaves room for a
/// transcript that has several in flight and for the last few shapes of each.
const CARET_MEASURE_CAP: usize = 16;

/// Measured trailing widths, keyed by what the shape depends on: the text,
/// the font size, the wrap width and the shaping-relevant parts of the runs.
/// Colour is deliberately not in the key — it does not move a glyph.
static CARET_WIDTHS: LazyLock<Mutex<Vec<CaretMeasure>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// One cached measurement: the key and the trailing width it shaped to.
type CaretMeasure = (u64, Option<Pixels>);

/// The key for one measurement. Fonts are compared by family, weight and
/// style; a run's colour, underline and strikethrough do not affect shaping.
fn caret_key(text: &str, font_size: Pixels, runs: &[TextRun], wrap_width: Pixels) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    f32::from(font_size).to_bits().hash(&mut hasher);
    f32::from(wrap_width).to_bits().hash(&mut hasher);
    for run in runs {
        run.len.hash(&mut hasher);
        run.font.family.hash(&mut hasher);
        run.font.weight.0.to_bits().hash(&mut hasher);
        matches!(run.font.style, gpui::FontStyle::Italic).hash(&mut hasher);
    }
    hasher.finish()
}

/// The width of the last *visual* line of `text` once it is wrapped at
/// `wrap_width`: the whole line when it never wrapped, otherwise the shaped
/// width from the last wrap boundary to the end.
///
/// Memoised: a streaming turn re-renders at frame rate but only grows its
/// text when a chunk lands, so without a cache this shapes the closing block
/// again for every frame in between. The cache holds
/// [`CARET_MEASURE_CAP`] entries, most recent last.
fn last_line_width(
    window: &Window,
    text: SharedString,
    font_size: Pixels,
    runs: &[TextRun],
    wrap_width: Pixels,
) -> Option<Pixels> {
    let key = caret_key(&text, font_size, runs, wrap_width);
    if let Ok(cache) = CARET_WIDTHS.lock() {
        if let Some((_, width)) = cache.iter().rev().find(|(cached, _)| *cached == key) {
            return *width;
        }
    }
    let width = shape_last_line_width(window, text, font_size, runs, wrap_width);
    if let Ok(mut cache) = CARET_WIDTHS.lock() {
        if cache.len() >= CARET_MEASURE_CAP {
            cache.remove(0);
        }
        cache.push((key, width));
    }
    width
}

/// [`last_line_width`] without the memo: the shaping pass itself.
fn shape_last_line_width(
    window: &Window,
    text: SharedString,
    font_size: Pixels,
    runs: &[TextRun],
    wrap_width: Pixels,
) -> Option<Pixels> {
    let lines = window
        .text_system()
        .shape_text(text, font_size, runs, Some(wrap_width), None)
        .ok()?;
    let line = lines.last()?;
    let layout = &line.unwrapped_layout;
    match line.wrap_boundaries.last() {
        None => Some(layout.width),
        Some(boundary) => {
            let index = layout
                .runs
                .get(boundary.run_ix)?
                .glyphs
                .get(boundary.glyph_ix)?
                .index;
            Some(layout.width - layout.x_for_index(index))
        }
    }
}

/// `now`, `5m ago`, `2h ago`, `3d ago`: the how-long-ago label for a
/// turn's [`Turn::timestamp`](aui_protocol::Turn::timestamp).
///
/// The sibling of the harness sidebar's `elapsed_at`: the same quantisation
/// against the same explicit-clock discipline — the caller reads its clock
/// once per frame and hands both instants in, so one frame formats once — and
/// the same saturation (a stamp from the future reads `now`), with the `ago`
/// the turn caption needs.
pub fn format_age(sent_ms: u64, now_ms: u64) -> SharedString {
    let seconds = now_ms.saturating_sub(sent_ms) / 1000;
    match seconds {
        s if s < 60 => "now".into(),
        s if s < 3_600 => format!("{}m ago", s / 60).into(),
        s if s < 86_400 => format!("{}h ago", s / 3_600).into(),
        s => format!("{}d ago", s / 86_400).into(),
    }
}

/// `model · 3.1 s · 2.4k tokens · 120 reasoning`: the footer cells
/// (docs/41 §2.2 — the age, when known, is appended by [`AssistantTurn::age`]).
///
/// Reasoning tokens get their own cell and only when there are any: a provider
/// can bill a reasoning budget and emit no reasoning item at all, so the number
/// is the one place a person can see that thinking happened, and a `0` on every
/// turn that did none would be noise. Zero tokens and zero time are "the wire
/// did not report it" and are dropped, never drawn as `0`.
pub fn footer_items(meta: &TurnMeta) -> Vec<SharedString> {
    let tokens = meta.tokens_in + meta.tokens_out;
    let tokens = match tokens {
        0 => String::new(),
        n if n >= 1000 => format!("{:.1}k tokens", n as f64 / 1000.0),
        n => format!("{n} tokens"),
    };
    let mut items: Vec<SharedString> = vec![
        meta.model.clone().into(),
        if meta.duration_ms == 0 { String::new() } else { format!("{:.1} s", meta.duration_ms as f64 / 1000.0) }.into(),
        tokens.into(),
    ];
    items.retain(|item| !item.is_empty());
    if meta.reasoning_tokens > 0 {
        items.push(format!("{} reasoning", meta.reasoning_tokens).into());
    }
    items
}

impl RenderOnce for AssistantTurn {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let rail_opacity = reveal(&id, "rail", flags.hovered, window, cx);

        let style = prose_style(&p, BODY_TEXT, BODY_LH, p.line, p.ink);
        // gpui text hosts no inline elements, so the caret is an absolutely
        // positioned sibling of the prose: last frame's prose bounds give the
        // wrap width, the last paragraph is re-shaped with the same runs the
        // prose paints, and the caret lands after the final visual line. It
        // is solid (docs/41 §3: nothing animates in prose).
        let bounds: Rc<RefCell<Option<Bounds<Pixels>>>> = window
            .use_keyed_state((id.clone(), "caret-bounds"), cx, |_, _| Rc::new(RefCell::new(None)))
            .read(cx)
            .clone();
        let scale_factor = cx.aui().text_scale;
        let caret = if self.streaming {
            let measured = *bounds.borrow();
            match measured {
                Some(b) if b.size.width > px(0.0) => {
                    let font_size = window.rem_size() * (BODY_TEXT / scale::FS_13);
                    let line_height = font_size * BODY_LH;
                    last_block_runs(&self.markdown, &style, p.accent)
                        .and_then(|(text, runs)| last_line_width(window, text.into(), font_size, &runs, b.size.width))
                        .map(|x| {
                            let height = px(CARET_H * scale_factor);
                            let top = b.size.height - line_height + caret_top_in_line(line_height, height, scale_factor);
                            div()
                                .absolute()
                                .left(x + px(CARET_MARGIN_LEFT * scale_factor))
                                .top(top)
                                .w(px(CARET_W * scale_factor))
                                .h(height)
                                .bg(p.ink)
                        })
                }
                _ => {
                    // Nothing measured yet: draw no caret and ask for the frame
                    // that will have the bounds.
                    window.request_animation_frame();
                    None
                }
            }
        } else {
            None
        };

        let mut turn = v_flex()
            .id(id.clone())
            .relative()
            .w_full()
            .ui(BODY_TEXT)
            .line_height(relative(BODY_LH))
            .text_color(p.ink)
            .track_interaction(&state);
        turn = turn.child(
            div()
                .relative()
                .w_full()
                .child(div().w_full().on_prepaint(move |b, _, _| *bounds.borrow_mut() = Some(b)).child({
                    let mut body = markdown((id.clone(), "text"), self.markdown.clone(), style).image_root(self.image_root.clone());
                    // Settled once `streaming` clears: only then may a
                    // whole-JSON message flip to its `json` code block, so a
                    // streaming turn never flickers mid-chunk.
                    body = body.settled(!self.streaming);
                    if let Some(on_link) = self.on_link.clone() {
                        body = body.on_link(move |target, window, cx| on_link(target, window, cx));
                    }
                    if let Some(on_code) = self.on_code_action.clone() {
                        body = body.on_code_action(move |action, window, cx| on_code(action, window, cx));
                    }
                    body = body.selection(self.selection.as_ref());
                    if let Some(on_change) = self.on_selection_change.clone() {
                        body = body.on_selection_change(move |next, window, cx| on_change(next, window, cx));
                    }
                    body = body.span_selection(self.span.as_ref()).selection_strength(self.span_strength);
                    if let Some(on_span) = self.on_span.clone() {
                        body = body.on_span_event(move |event, window, cx| on_span(event, window, cx));
                    }
                    body
                }))
                .children(caret),
        );

        // Under a settled answer: one copy button 8 below, then the footer
        // 10 below that, shown only while the turn is hovered — but always
        // laid out, so nothing shifts when they appear. A streaming answer
        // has no rail yet.
        if self.rail && !self.streaming {
            let morph = copy_morph(&id, self.copied, &p, window, cx);
            let mut copy = icon_content_button((id.clone(), "copy"), morph).ghost().size(ButtonSize::Md).accessibility_label("Copy reply");
            if let Some(h) = self.on_action.clone() {
                copy = copy.on_click(move |_, w, cx| h(AssistantTurnAction::Copy, w, cx));
            }
            let mut rail = v_flex().mt(px(COPY_TOP)).items_start().opacity(rail_opacity).child(copy);
            if !self.footer.is_empty() {
                rail = rail.child(
                    div()
                        .mt(px(FOOTER_TOP))
                        .font_family(scale::FONT_MONO)
                        .text_px(scale::FS_11)
                        .line_height(relative(1.0))
                        .text_color(p.ink_4)
                        .child(self.footer.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" · ")),
                );
            }
            if rail_opacity <= 0.001 {
                rail = rail.invisible();
            }
            turn = turn.child(rail);
        }
        turn
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assistant_turn, footer_items, format_age, turn_selected_text, turn_span_selected_text,
        user_turn, UserTurnAction, COPY_HOLD, USER_MAX,
    };
    use crate::protocol::TurnMeta;
    use crate::transcript::{MessageSelection, SelectionEndpoint, SelectionKey, TextSelection};
    use gpui::SharedString;

    #[test]
    fn user_actions_default_to_all() {
        assert_eq!(user_turn("t", "hi").actions, UserTurnAction::ALL.to_vec());
    }

    #[test]
    fn user_actions_keeps_a_reduced_set() {
        let turn = user_turn("t", "hi").actions(&[UserTurnAction::Copy]);
        assert_eq!(turn.actions, vec![UserTurnAction::Copy]);
    }

    #[test]
    fn user_width_defaults_to_the_chat_cap() {
        assert_eq!(user_turn("t", "hi").max_width, USER_MAX);
        assert_eq!(USER_MAX, 0.78);
    }

    #[test]
    fn user_width_override_is_applied() {
        assert_eq!(user_turn("t", "hi").max_width_fraction(1.0).max_width, 1.0);
        assert_eq!(user_turn("t", "hi").max_width_fraction(0.5).max_width, 0.5);
    }

    #[test]
    fn user_width_override_outside_the_unit_range_falls_back() {
        for bad in [-1.0, 1.5, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                user_turn("t", "hi").max_width_fraction(bad).max_width,
                USER_MAX,
                "fraction {bad} was trusted instead of falling back"
            );
        }
    }

    #[test]
    fn user_turns_offer_edit_and_copy_only() {
        // D4: resend was removed (docs/41 §6, decision 1).
        assert_eq!(UserTurnAction::ALL, &[UserTurnAction::Edit, UserTurnAction::Copy]);
    }

    #[test]
    fn the_footer_never_carries_a_cost() {
        let meta = TurnMeta { model: "m".into(), duration_ms: 1000, tokens_in: 1, tokens_out: 1, cost_usd: 0.31, ..TurnMeta::default() };
        let cells = footer_items(&meta);
        assert!(!cells.iter().any(|c| c.contains('$')), "{cells:?}");
    }

    #[test]
    fn video_files_take_the_video_treatment_and_file_chips_name_type_and_size() {
        use crate::protocol::{Attachment, AttachmentKind, UploadState};
        let file = |name: &str, kind: AttachmentKind| Attachment { name: name.into(), kind, size_bytes: Some(2458), meta: None, state: UploadState::Ready, path: None, quote_of: None };
        assert!(super::is_video(&file("capture.MOV", AttachmentKind::File)));
        assert!(!super::is_video(&file("capture.mov", AttachmentKind::Image)));
        assert!(!super::is_video(&file("notes.jsonl", AttachmentKind::File)));
        assert_eq!(super::file_meta(&file("synthetic-markdown.jsonl", AttachmentKind::File)).as_deref(), Some("JSONL · 2.4 KB"));
    }

    #[test]
    fn turn_selected_text_reads_a_paragraph_slice() {
        // Code spans read as plain words in the shaped text.
        let selection = TextSelection {
            cell: SelectionKey::paragraph("", 0),
            range: 0..7,
        };
        assert_eq!(
            turn_selected_text("Tighten `validateAddress` now", &selection).as_deref(),
            Some("Tighten")
        );
    }

    #[test]
    fn turn_span_selected_text_joins_cells_in_order() {
        let span = MessageSelection {
            anchor: SelectionEndpoint {
                cell: SelectionKey::paragraph("", 0),
                offset: 6,
            },
            focus: SelectionEndpoint {
                cell: SelectionKey::list_item("", 1, false, 0),
                offset: 5,
            },
        };
        assert_eq!(
            turn_span_selected_text("First paragraph here.\n\n- alpha\n", &span).as_deref(),
            Some("paragraph here.\n\n- alpha")
        );
        let unknown = MessageSelection {
            anchor: SelectionEndpoint {
                cell: SelectionKey::paragraph("", 9),
                offset: 0,
            },
            focus: SelectionEndpoint {
                cell: SelectionKey::paragraph("", 9),
                offset: 3,
            },
        };
        assert_eq!(turn_span_selected_text("Hello", &unknown), None);
    }

    #[test]
    fn turn_selected_text_rejects_unknown_cells() {
        let selection = TextSelection {
            cell: SelectionKey::paragraph("", 9),
            range: 0..7,
        };
        assert_eq!(turn_selected_text("Hello", &selection), None);
    }

    #[test]
    fn age_labels_are_quantised_against_the_given_clock() {
        let now = 1_786_320_000_000;
        assert_eq!(format_age(now, now), "now");
        // A stamp from the future saturates rather than going negative.
        assert_eq!(format_age(now + 60_000, now), "now");
        assert_eq!(format_age(now - 90_000, now), "1m ago");
        assert_eq!(format_age(now - 2 * 3_600_000, now), "2h ago");
        assert_eq!(format_age(now - 3 * 86_400_000, now), "3d ago");
    }

    #[test]
    fn copy_buttons_rest_on_the_copy_glyph_until_the_caller_confirms() {
        assert!(!user_turn("t", "hi").copied);
        assert!(!assistant_turn("t", "hi").copied);
        assert!(user_turn("t", "hi").copied(true).copied);
        assert!(assistant_turn("t", "hi").copied(true).copied);
        // One hold for both rails and the code block header.
        assert_eq!(COPY_HOLD, std::time::Duration::from_millis(1200));
    }

    #[test]
    fn turns_draw_no_age_until_the_caller_passes_one() {
        assert!(user_turn("t", "hi").age.is_none());
        assert!(assistant_turn("t", "hi").footer.is_empty());
        assert_eq!(
            assistant_turn("t", "hi")
                .age("2m ago")
                .footer
                .last()
                .map(SharedString::as_ref),
            Some("2m ago")
        );
    }

    /// A turn whose usage the wire never reported draws no token cell at all,
    /// the way a catalog with no price draws no cost cell — `0 tokens` under
    /// a full reply is a number the app cannot stand behind.
    #[test]
    fn a_turn_with_no_reported_usage_draws_no_token_cell() {
        let meta = |tokens_in: u64, tokens_out: u64| TurnMeta {
            model: "muse-spark-1.3".into(),
            duration_ms: 2300,
            tokens_in,
            tokens_out,
            ..TurnMeta::default()
        };
        let cells = footer_items(&meta(0, 0));
        assert!(
            !cells.iter().any(|c| c.contains("token")),
            "an unreported count was drawn: {cells:?}"
        );
        assert_eq!(
            cells,
            vec![
                SharedString::from("muse-spark-1.3"),
                SharedString::from("2.3 s")
            ]
        );

        // A reported count still draws, in both spellings.
        assert!(footer_items(&meta(400, 19))
            .iter()
            .any(|c| c.as_ref() == "419 tokens"));
        assert!(footer_items(&meta(59_000, 200))
            .iter()
            .any(|c| c.as_ref() == "59.2k tokens"));
    }
}
