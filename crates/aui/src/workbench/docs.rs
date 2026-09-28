//! The assistant document pane of card 54 (`design/src/cards/workbench/54-files-docs.html`,
//! spec §5.5): the pane's own tab band, the formatting toolbar, the paper
//! area with the white page, the strip of artifacts the chat created and the
//! pane status row.
//!
//! The pane is chrome only: it draws a page that the application hands over as
//! a [`DocPage`] of [`DocBlock`]s, marks the spans the agent changed
//! ([`DocRun::Changed`]) and reports intents; it neither edits nor loads
//! anything. The sibling PDF and sheet panes of spec §5.5 (page stepper and
//! cited passage; formula bar and grid) are not in this card and arrive with
//! the screens phase; they will reuse [`artifact_strip`] and
//! [`pane_status_row`].

use std::ops::Range;
use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{
    div, font, prelude::*, px, relative, rgb, AnyElement, App, ElementId, FontWeight, Hsla, IntoElement, ScrollHandle, SharedString, StyledText,
    TextRun, UnderlineStyle, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, chip, icon_button, ButtonSize};
use crate::shell::TabItem;
use crate::transcript::{
    MessageSelection, SelectionEndpoint, SelectionHandler, SelectionKey, SpanHandler, TextSelection, selectable_text,
};
use crate::util::{interaction_flags, TrackInteraction};

/// `.dtabs{height:34px;gap:2px;padding:0 6px}`.
const TABS_H: f32 = 34.0;
const TABS_GAP: f32 = 2.0;
const TABS_PAD: f32 = 6.0;
/// `.dt{gap:7px;padding:0 12px;font-size:12px}`.
const TAB_GAP: f32 = 7.0;
const TAB_PAD: f32 = 12.0;
/// `.dt.on::after{left:8px;right:8px;top:0;height:2px;background:var(--accent)}` —
/// the pane's indicator is an accent bar along the *top* of the active tab,
/// unlike the shell strip's ink bar underneath, so this band is drawn here.
const INDICATOR_H: f32 = 2.0;
const INDICATOR_INSET: f32 = 8.0;
/// Glyphs in tabs and in the band's xs ghost buttons: 12 px.
const SMALL_GLYPH: f32 = 12.0;
/// The saved hint beside the tabs: `font-size:11px`.
const HINT_TEXT: f32 = scale::FS_11;
/// `.tool{height:34px;gap:4px;padding:0 10px;font-size:12px}`.
const TOOLBAR_H: f32 = 34.0;
const TOOLBAR_GAP: f32 = 4.0;
const TOOLBAR_PAD: f32 = 10.0;
/// `.tool .sel{height:24px;padding:0 8px;border-radius:5px;gap:6px}`.
const SELECT_PAD: f32 = 8.0;
const SELECT_GAP: f32 = 6.0;
const SELECT_RADIUS: f32 = 5.0;
/// The chevron inside a select: `width:10px;height:10px`.
const SELECT_CHEVRON: f32 = 10.0;
/// `.tool .sep{width:1px;height:16px;margin:0 4px}`.
const SEPARATOR_H: f32 = 16.0;
const SEPARATOR_MARGIN: f32 = 4.0;
/// The sparkle in the ask chip: `width:11px;height:11px`.
const CHIP_GLYPH: f32 = 11.0;
/// `.doc{padding:18px}`.
const DOC_PAD: f32 = 18.0;
/// `.paper{width:520px;padding:38px 46px;font-size:12.5px;line-height:1.6}`.
const PAPER_W: f32 = 520.0;
const PAPER_PAD_Y: f32 = 38.0;
const PAPER_PAD_X: f32 = 46.0;
const PAPER_TEXT: f32 = 12.5;
const PAPER_LH: f32 = 1.6;
/// `.paper h1{font-size:18px;margin:0 0 4px;font-family:var(--font-ui)}`.
const PAPER_TITLE: f32 = scale::FS_18;
/// `.paper h1{line-height:1.3}` — the title sets its own leading rather than
/// inheriting the body's 1.6, so a two-line title on a narrow page stays tight.
const PAPER_TITLE_LH: f32 = 1.3;
const PAPER_TITLE_GAP: f32 = 4.0;
/// `.paper .k{font:11px var(--font-ui);margin-bottom:18px}` — the `font`
/// shorthand drops the paper's line height back to the normal one.
const PAPER_META: f32 = scale::FS_11;
const PAPER_META_LH: f32 = 1.2;
const PAPER_META_GAP: f32 = 18.0;
/// `.paper p{margin:0 0 10px}`.
const PARAGRAPH_GAP: f32 = 10.0;
/// `.paper ol{padding-left:18px}` with the browser's default `1em` block
/// margins, which at 12.5 px is 12.5 px; CSS collapses it against the
/// paragraph's 10 px, so the larger of the two is used.
const LIST_INDENT: f32 = 18.0;
/// Chrome's decimal marker carries a trailing space: the item's text starts at
/// the 18 px indent and the marker's glyphs end 4 px before it.
const LIST_MARKER_GAP: f32 = 4.0;
const LIST_GAP: f32 = PAPER_TEXT;
/// Headings: the UI face, bold — 13 px, 12 px, 11.5 px, 15 px above, 5 px below.
const HEADING_1: f32 = 13.0;
const HEADING_2: f32 = 12.0;
const HEADING_3: f32 = 11.5;
const HEADING_GAP_TOP: f32 = 15.0;
const HEADING_GAP_BOTTOM: f32 = 5.0;
/// Tables: hairline `#DDD` borders, a `#F5F6F9` header ground carrying the UI
/// face at 11 px semibold, Georgia body cells at 11 px, 6 px vertical and 8 px
/// horizontal cell padding, 12 px above and below.
const TABLE_BORDER: u32 = 0xDDDDDD;
const TABLE_HEADER_GROUND: u32 = 0xF5F6F9;
const TABLE_HEADER_TEXT: f32 = scale::FS_11;
const TABLE_BODY_TEXT: f32 = 11.0;
const TABLE_CELL_PAD_Y: f32 = 6.0;
const TABLE_CELL_PAD_X: f32 = 8.0;
/// The narrowest a table column is ever drawn: 72 px holds "Female" plus the
/// cell's horizontal padding at the 11 px body size. A table whose columns
/// would squeeze past this keeps its natural width and scrolls sideways
/// instead.
pub const TABLE_MIN_COL_W: f32 = 72.0;
const TABLE_GAP: f32 = 12.0;
/// Images sit 12 px from their neighbours; the caller picks the logical size.
const IMAGE_GAP: f32 = 12.0;
/// A rule is a 1 px `#DDD` hairline with 16 px above and below.
const RULE_COLOUR: u32 = 0xDDDDDD;
const RULE_GAP: f32 = 16.0;
/// `.paper{font-family:Georgia,"Times New Roman",serif}` — the single place
/// the design leaves the UI face, and the tokens carry no serif family.
const PAPER_FONT: &str = "Georgia";
/// `.paper{background:#fff;color:#1a1c22}` — the page is white in both themes,
/// so these are literal and not palette colours.
const PAPER_BG: u32 = 0xFFFFFF;
const PAPER_INK: u32 = 0x1A1C22;
/// `.paper .k{color:#7b818f}`.
const PAPER_META_INK: u32 = 0x7B818F;
/// `.paper .ai{background:rgba(80,87,214,.12);border-bottom:2px solid #5057D6}` —
/// fixed against the white page rather than taken from the theme's accent.
const CHANGE_INK: u32 = 0x5057D6;
const CHANGE_GROUND_ALPHA: f32 = 0.12;
const CHANGE_UNDERLINE: f32 = 2.0;
/// `.arts{gap:6px;padding:6px 10px}`.
const ARTS_GAP: f32 = 6.0;
const ARTS_PAD_Y: f32 = 6.0;
const ARTS_PAD_X: f32 = 10.0;
/// The caps label before the artifacts: `margin-right:4px`.
const ARTS_LABEL_MARGIN: f32 = 4.0;
/// `.art{height:26px;padding:0 8px 0 6px;border-radius:6px;gap:6px;font-size:11.5px}`.
const ART_H: f32 = 26.0;
const ART_PAD_LEFT: f32 = 6.0;
const ART_PAD_RIGHT: f32 = 8.0;
const ART_GAP: f32 = 6.0;
const ART_TEXT: f32 = 11.5;
/// `.art .v{font:500 10px var(--font-mono)}`.
const ART_VERSION_TEXT: f32 = 10.0;
/// `.art.more{padding:0 8px}` — the overflow chip has no glyph, so it carries
/// the same padding on both sides.
const ART_MORE_PAD: f32 = 8.0;
/// `.art{flex:0 1 auto}`: every chip gives way at the same rate, so a strip
/// short of room shortens the long names first.
const ART_SHRINK: f32 = 1.0;
/// `.stat{height:28px;gap:10px;padding:0 12px;font:11px var(--font-ui)}`.
const STATUS_H: f32 = 28.0;
const STATUS_GAP: f32 = 10.0;
const STATUS_PAD: f32 = 12.0;
const STATUS_TEXT: f32 = scale::FS_11;

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type MoreHandler = Rc<dyn Fn(&mut Window, &mut App)>;

// ---------------------------------------------------------------------------
// Tab band
// ---------------------------------------------------------------------------

/// The document pane's tab band (`.dtabs`). Build with [`doc_tabs`].
#[derive(IntoElement)]
pub struct DocTabs {
    id: ElementId,
    tabs: Vec<TabItem>,
    active: usize,
    trailing: Vec<AnyElement>,
    on_select: Option<SelectHandler>,
}

/// The pane's tabs, one per open document, with `active` selected. The tabs
/// are [`TabItem`]s so a pane can hand the same list to the shell strip; the
/// band itself is the card's own (accent indicator on top, no close
/// affordance).
pub fn doc_tabs(id: impl Into<ElementId>, tabs: Vec<TabItem>, active: usize) -> DocTabs {
    DocTabs { id: id.into(), tabs, active, trailing: Vec::new(), on_select: None }
}

impl DocTabs {
    /// A control at the far right of the band (the saved hint, the overflow
    /// button).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing.push(el.into_any_element());
        self
    }

    /// Called with the tab id when a tab is clicked.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for DocTabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut band = h_flex()
            .id(id.clone())
            .w_full()
            .flex_none()
            .h(px(TABS_H))
            .gap(px(TABS_GAP))
            .px(px(TABS_PAD))
            .border_b_1()
            .border_color(p.line);

        for (index, tab) in self.tabs.iter().enumerate() {
            let tab_id: ElementId = (id.clone(), tab.id.clone()).into();
            let active = index == self.active;
            let (state, flags) = interaction_flags(tab_id.clone(), window, cx);
            let color = tween((tab_id.clone(), "color"), if active || flags.hovered { p.ink } else { p.ink_3 }, Tween::FAST, window, cx);
            let mut el = h_flex()
                .id(tab_id)
                .relative()
                .h_full()
                .flex_none()
                .gap(px(TAB_GAP))
                .px(px(TAB_PAD))
                .text_color(color)
                .ui(scale::FS_12)
                .whitespace_nowrap()
                .cursor_pointer()
                .track_interaction(&state);
            if let Some(glyph) = tab.icon {
                el = el.child(icon(glyph).size(px(SMALL_GLYPH)).color(color));
            }
            el = el.child(tab.label.clone());
            if active {
                el = el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left(px(INDICATOR_INSET))
                        .right(px(INDICATOR_INSET))
                        .h(px(INDICATOR_H))
                        .rounded_b(px(INDICATOR_H))
                        .bg(p.accent),
                );
            }
            if let Some(f) = self.on_select.clone() {
                let key = tab.id.clone();
                el = el.on_click(move |_, w, cx| f(&key, w, cx));
            }
            band = band.child(el);
        }
        band.child(div().flex_1().min_w(px(0.0))).children(self.trailing)
    }
}

/// The "Saved" hint that sits at the right of the tab band (`.subtle`, 11 px).
pub fn doc_saved_hint(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div().flex_none().ui(HINT_TEXT).text_color(cx.aui().colors.ink_3).child(text.into())
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

/// The character marks of the toolbar (`<b>`, `<i>`, `<u>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMark {
    /// Bold.
    Bold,
    /// Italic.
    Italic,
    /// Underline.
    Underline,
}

impl FormatMark {
    /// The letter drawn in the button.
    pub fn letter(self) -> &'static str {
        match self {
            FormatMark::Bold => "B",
            FormatMark::Italic => "I",
            FormatMark::Underline => "U",
        }
    }
}

/// What a toolbar control asks the application to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocToolbarAction {
    /// The paragraph-style select.
    Style,
    /// The font select.
    Font,
    /// A character mark.
    Mark(FormatMark),
    /// Insert a list.
    InsertList,
    /// Insert a link.
    InsertLink,
    /// Insert an image.
    InsertImage,
    /// The sparkle chip: ask the agent about the selection.
    AskAboutSelection,
    /// Export the document.
    Export,
}

type ToolbarHandler = Rc<dyn Fn(&DocToolbarAction, &mut Window, &mut App)>;

/// The document toolbar (`.tool`). Build with [`doc_toolbar`].
#[derive(IntoElement)]
pub struct DocToolbar {
    id: ElementId,
    style_label: SharedString,
    font_label: Option<SharedString>,
    ask_label: SharedString,
    export_label: Option<SharedString>,
    on_action: Option<ToolbarHandler>,
}

/// The toolbar with the paragraph style ("Body text") and font ("Georgia · 11")
/// selects; the marks, insert actions, ask chip and export button are fixed by
/// the design.
pub fn doc_toolbar(id: impl Into<ElementId>, style_label: impl Into<SharedString>, font_label: impl Into<SharedString>) -> DocToolbar {
    DocToolbar {
        id: id.into(),
        style_label: style_label.into(),
        font_label: Some(font_label.into()),
        ask_label: "Ask about selection".into(),
        export_label: Some("Export".into()),
        on_action: None,
    }
}

impl DocToolbar {
    /// Drops the font select. The toolbar's controls keep their width rather
    /// than shrinking, so in a pane as narrow as the shell's right pane
    /// (`shell::RIGHT_WIDTH`) the row cannot hold both selects and the ask
    /// chip; the screens drop the font, which the page's own face already
    /// states, and keep every control at its full size.
    pub fn without_font_select(mut self) -> Self {
        self.font_label = None;
        self
    }

    /// Overrides the sparkle chip's label.
    pub fn ask_label(mut self, label: impl Into<SharedString>) -> Self {
        self.ask_label = label.into();
        self
    }

    /// Overrides the export button's label.
    pub fn export_label(mut self, label: impl Into<SharedString>) -> Self {
        self.export_label = Some(label.into());
        self
    }

    /// Drops the export button, the way the assistant screens' narrow pane
    /// does: export lives in the tab band's overflow menu there, and the row
    /// keeps every remaining control at its designed width.
    pub fn without_export(mut self) -> Self {
        self.export_label = None;
        self
    }

    /// Called with every intent the toolbar emits.
    pub fn on_action(mut self, f: impl Fn(&DocToolbarAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for DocToolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let emit = |handler: &Option<ToolbarHandler>, action: DocToolbarAction| {
            let handler = handler.clone();
            move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(f) = &handler {
                    f(&action, w, cx)
                }
            }
        };
        // The row wraps below ~460 px instead of clipping its trailing
        // controls; one line still measures exactly as before.
        let mut bar = h_flex()
            .id(id.clone())
            .w_full()
            .flex_none()
            .min_h(px(TOOLBAR_H))
            .flex_wrap()
            .gap(px(TOOLBAR_GAP))
            .px(px(TOOLBAR_PAD))
            .border_b_1()
            .border_color(p.line)
            .child(
                toolbar_select((id.clone(), "style"), self.style_label.clone(), true, cx)
                    .on_click(emit(&self.on_action, DocToolbarAction::Style)),
            )
            .children(self.font_label.clone().map(|label| {
                toolbar_select((id.clone(), "font"), label, false, cx).on_click(emit(&self.on_action, DocToolbarAction::Font))
            }))
            .child(toolbar_separator(cx));
        for mark in [FormatMark::Bold, FormatMark::Italic, FormatMark::Underline] {
            bar = bar.child(mark_button((id.clone(), mark.letter()), mark, self.on_action.clone(), window, cx));
        }
        bar = bar.child(toolbar_separator(cx));
        for (key, glyph, action) in [
            ("list", IconName::List, DocToolbarAction::InsertList),
            ("link", IconName::Link, DocToolbarAction::InsertLink),
            ("image", IconName::Image, DocToolbarAction::InsertImage),
        ] {
            bar = bar.child(
                icon_button((id.clone(), key), glyph)
                    .ghost()
                    .size(ButtonSize::Xs)
                    .icon_size(px(SMALL_GLYPH))
                    .on_click(emit(&self.on_action, action)),
            );
        }
        bar.child(div().flex_1().min_w(px(0.0)))
            .child(
                chip((id.clone(), "ask"), self.ask_label.clone())
                    .icon(IconName::Sparkle)
                    .accent()
                    .on_click(emit(&self.on_action, DocToolbarAction::AskAboutSelection)),
            )
            .children(self.export_label.clone().map(|label| {
                button((id, "export"), label).size(ButtonSize::Xs).on_click(emit(&self.on_action, DocToolbarAction::Export))
            }))
    }
}

/// `.tool .sel`: a 24 px bordered select on surface-2, with an optional
/// chevron.
fn toolbar_select(id: impl Into<ElementId>, label: SharedString, chevron: bool, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let mut el = h_flex()
        .id(id.into())
        .flex_none()
        .h(cx.aui().metrics.control_sm)
        .px(px(SELECT_PAD))
        .gap(px(SELECT_GAP))
        .rounded(px(SELECT_RADIUS))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_2)
        .text_color(p.ink)
        .ui(scale::FS_12)
        .whitespace_nowrap()
        .cursor_pointer()
        .child(label);
    if chevron {
        el = el.child(icon(IconName::ChevronDown).size(px(SELECT_CHEVRON)).color(p.ink));
    }
    el
}

/// `.tool .sep`: the 1 × 16 px divider between toolbar groups.
fn toolbar_separator(cx: &App) -> impl IntoElement {
    div().flex_none().w(px(1.0)).h(px(SEPARATOR_H)).mx(px(SEPARATOR_MARGIN)).bg(cx.aui().colors.line)
}

/// One character-mark button: an xs ghost square carrying the styled letter
/// rather than a glyph, so it cannot be an [`icon_button`].
fn mark_button(
    id: impl Into<ElementId>,
    mark: FormatMark,
    on_action: Option<ToolbarHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let (state, flags) = interaction_flags(id.clone(), window, cx);
    let bg = tint_fade((id.clone(), "bg"), flags.hovered, p.surface_2, Tween::FAST, window, cx);
    let text = tween((id.clone(), "text"), if flags.hovered { p.ink } else { p.ink_2 }, Tween::FAST, window, cx);
    let mut letter = div().ui(scale::FS_11).line_height(relative(1.0)).child(mark.letter());
    letter = match mark {
        FormatMark::Bold => letter.semibold(),
        FormatMark::Italic => letter.italic(),
        FormatMark::Underline => letter.text_decoration_1().text_decoration_color(text),
    };
    let mut el = div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(cx.aui().metrics.control_xs)
        .rounded(px(scale::R_SM))
        .bg(bg)
        .text_color(text)
        .cursor_pointer()
        .track_interaction(&state)
        .child(letter);
    if let Some(f) = on_action {
        el = el.on_click(move |_, w, cx| f(&DocToolbarAction::Mark(mark), w, cx));
    }
    el
}

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

/// One inline run of the page's body text.
#[derive(Debug, Clone, PartialEq)]
pub enum DocRun {
    /// Plain body text.
    Text(SharedString),
    /// Bold text (the numbered clause openers).
    Bold(SharedString),
    /// A span the chat changed: the accent ground and the 2 px accent
    /// underline of `.paper .ai`.
    Changed(SharedString),
}

impl DocRun {
    /// Plain text.
    pub fn text(s: impl Into<SharedString>) -> Self {
        DocRun::Text(s.into())
    }

    /// Bold text.
    pub fn bold(s: impl Into<SharedString>) -> Self {
        DocRun::Bold(s.into())
    }

    /// A span the chat changed.
    pub fn changed(s: impl Into<SharedString>) -> Self {
        DocRun::Changed(s.into())
    }

    fn str(&self) -> &str {
        match self {
            DocRun::Text(s) | DocRun::Bold(s) | DocRun::Changed(s) => s.as_ref(),
        }
    }
}

/// One cell of a [`DocTable`].
#[derive(Debug, Clone, PartialEq)]
pub struct DocCell {
    /// The cell's text.
    pub runs: Vec<DocRun>,
    /// Columns this cell spans; 1 when not merged.
    pub col_span: usize,
    /// Rows this cell spans; 1 when not merged. Carried for the caller's
    /// benefit and NOT painted by this pane — a merged-down cell draws as a
    /// single row.
    pub row_span: usize,
    /// Header styling (the `#F5F6F9` ground, UI face) rather than body styling.
    pub header: bool,
}

impl DocCell {
    /// A body cell of plain text.
    pub fn text(s: impl Into<SharedString>) -> Self {
        Self { runs: vec![DocRun::text(s)], col_span: 1, row_span: 1, header: false }
    }

    /// A header cell of plain text.
    pub fn header(s: impl Into<SharedString>) -> Self {
        Self { runs: vec![DocRun::text(s)], col_span: 1, row_span: 1, header: true }
    }
}

/// A table: proportional column widths and rows of cells.
#[derive(Debug, Clone, PartialEq)]
pub struct DocTable {
    /// Column widths as fractions summing to 1.0. If empty, columns are equal.
    pub widths: Vec<f32>,
    /// Rows of cells.
    pub rows: Vec<Vec<DocCell>>,
}

/// An image block.
#[derive(Clone)]
pub struct DocImage {
    /// Where the pixels come from.
    pub source: gpui::ImageSource,
    /// The accessible name carried as the element's accessibility label.
    pub alt: SharedString,
    /// Logical size the caller wants it painted at.
    pub width: f32,
    /// Logical size the caller wants it painted at.
    pub height: f32,
}

impl std::fmt::Debug for DocImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocImage").field("alt", &self.alt).field("width", &self.width).field("height", &self.height).finish()
    }
}

/// `gpui::ImageSource` carries loader closures and cached pixels, so it is
/// neither `PartialEq` nor a useful part of equality: two images are the same
/// block when their accessible name and logical size match.
impl PartialEq for DocImage {
    fn eq(&self, other: &Self) -> bool {
        self.alt == other.alt && self.width == other.width && self.height == other.height
    }
}

/// One block of the page.
#[derive(Debug, Clone, PartialEq)]
pub enum DocBlock {
    /// A paragraph (`.paper p`).
    Paragraph(Vec<DocRun>),
    /// A numbered list (`.paper ol`); items are numbered from 1.
    List(Vec<Vec<DocRun>>),
    /// A heading: the UI face, bold, sized by level (1–3).
    Heading {
        /// The heading level, clamped to 1–3 when painted.
        level: u8,
        /// The heading text.
        runs: Vec<DocRun>,
    },
    /// A real grid with proportional columns.
    Table(DocTable),
    /// A centred image at its logical size.
    Image(DocImage),
    /// A 1 px hairline rule.
    Rule,
}

impl DocBlock {
    /// A paragraph of plain text.
    pub fn text(s: impl Into<SharedString>) -> Self {
        DocBlock::Paragraph(vec![DocRun::text(s)])
    }

    /// A numbered list of plain items.
    pub fn list(items: impl IntoIterator<Item = &'static str>) -> Self {
        DocBlock::List(items.into_iter().map(|i| vec![DocRun::text(i)]).collect())
    }

    /// A heading of plain text at `level` (clamped to 1–3 when painted).
    pub fn heading(level: u8, text: impl Into<SharedString>) -> Self {
        DocBlock::Heading { level, runs: vec![DocRun::text(text)] }
    }

    /// The block's top margin, for CSS-style margin collapsing.
    fn margin_top(&self) -> f32 {
        match self {
            DocBlock::Paragraph(_) => 0.0,
            DocBlock::List(_) => LIST_GAP,
            DocBlock::Heading { .. } => HEADING_GAP_TOP,
            DocBlock::Table(_) => TABLE_GAP,
            DocBlock::Image(_) => IMAGE_GAP,
            DocBlock::Rule => RULE_GAP,
        }
    }

    /// The block's bottom margin.
    fn margin_bottom(&self) -> f32 {
        match self {
            DocBlock::Paragraph(_) => PARAGRAPH_GAP,
            DocBlock::List(_) => LIST_GAP,
            DocBlock::Heading { .. } => HEADING_GAP_BOTTOM,
            DocBlock::Table(_) => TABLE_GAP,
            DocBlock::Image(_) => IMAGE_GAP,
            DocBlock::Rule => RULE_GAP,
        }
    }
}

/// The collapsed vertical gap between two adjacent blocks: CSS collapses
/// adjacent margins, so the larger of the upper block's bottom margin and the
/// lower block's top margin wins.
pub fn collapsed_margin(upper: &DocBlock, lower: &DocBlock) -> f32 {
    upper.margin_bottom().max(lower.margin_top())
}

/// Clamps a heading `level` into 1–3: 0 maps to 1, anything above 3 maps to 3.
pub fn clamp_heading_level(level: u8) -> u8 {
    level.clamp(1, 3)
}

/// The UI-face size of a heading level (clamped): 13 px, 12 px, 11.5 px.
pub fn heading_text_size(level: u8) -> f32 {
    match clamp_heading_level(level) {
        1 => HEADING_1,
        2 => HEADING_2,
        _ => HEADING_3,
    }
}

/// The fraction of the row width a cell at `index` spanning `col_span`
/// columns occupies. Sums `widths[index..index + col_span]`, clamped to the
/// table so a span past the end covers what is left (at most 1.0); when
/// `widths` is empty or shorter than `col_count` the columns are equal. Never
/// divides by zero and never indexes out of bounds: a malformed table paints
/// something instead of panicking.
pub fn cell_span_width(widths: &[f32], col_count: usize, index: usize, col_span: usize) -> f32 {
    let span = col_span.max(1);
    if col_count == 0 {
        return 1.0;
    }
    if widths.len() < col_count {
        let remaining = col_count.saturating_sub(index.min(col_count)).max(1);
        (span.min(remaining) as f32) / (col_count as f32)
    } else {
        let start = index.min(widths.len());
        let end = index.saturating_add(span).min(widths.len());
        if end <= start {
            1.0 / (col_count as f32)
        } else {
            widths[start..end].iter().sum::<f32>().min(1.0)
        }
    }
}

/// Column pixel widths for one table, and whether the table must scroll.
/// Pure numbers in, numbers out, so tests need no window; [`paint_table`]
/// consumes the result instead of redoing the arithmetic.
///
/// Fractions come from `widths` (equal columns when `widths` is shorter than
/// `col_count`, as in [`cell_span_width`]) and the natural width is
/// `col_count * TABLE_MIN_COL_W`. When the natural width fits `usable_width`
/// the columns fill the paper exactly as before; otherwise each column keeps
/// at least `TABLE_MIN_COL_W` and the painter lays the table out at its
/// natural width inside a scrolling frame.
pub fn table_layout(widths: &[f32], col_count: usize, usable_width: f32) -> (Vec<f32>, bool) {
    if col_count == 0 {
        return (Vec::new(), false);
    }
    if !usable_width.is_finite() || usable_width <= 0.0 {
        return (vec![TABLE_MIN_COL_W; col_count], true);
    }
    let mut fracs = vec![0.0f32; col_count];
    if widths.len() >= col_count {
        let sum: f32 = widths[..col_count].iter().map(|w| w.max(0.0)).sum();
        if sum > 0.0 {
            for (frac, w) in fracs.iter_mut().zip(widths.iter()) {
                *frac = w.max(0.0) / sum;
            }
        } else {
            for frac in fracs.iter_mut() {
                *frac = 1.0 / (col_count as f32);
            }
        }
    } else {
        for frac in fracs.iter_mut() {
            *frac = 1.0 / (col_count as f32);
        }
    }
    let natural = col_count as f32 * TABLE_MIN_COL_W;
    if natural <= usable_width {
        (fracs.iter().map(|f| (f * usable_width).max(TABLE_MIN_COL_W)).collect(), false)
    } else {
        (fracs.iter().map(|f| (f * natural).max(TABLE_MIN_COL_W)).collect(), true)
    }
}

/// The page the document pane shows.
#[derive(Debug, Clone, PartialEq)]
pub struct DocPage {
    /// The page title (`.paper h1`).
    pub title: SharedString,
    /// The meta line under the title (`.paper .k`).
    pub subtitle: SharedString,
    /// The body.
    pub blocks: Vec<DocBlock>,
}

impl DocPage {
    /// A page.
    pub fn new(title: impl Into<SharedString>, subtitle: impl Into<SharedString>, blocks: Vec<DocBlock>) -> Self {
        Self { title: title.into(), subtitle: subtitle.into(), blocks }
    }
}

/// Flush-mode paper geometry for [`DocPane::flush`]: the paper spans the
/// pane's full width and stretches to at least its full height, so a short
/// page leaves no surface band below. Returns
/// `(paper_width, paper_min_height)`; a long page keeps its content height
/// and scrolls exactly as in framed mode.
pub fn flush_paper_layout(pane_width: f32, pane_height: f32, content_height: f32) -> (f32, f32) {
    (pane_width, content_height.max(pane_height))
}

/// The paper area of the document pane (`.doc` + `.paper`). Build with
/// [`doc_pane`].
#[derive(IntoElement)]
pub struct DocPane {
    id: ElementId,
    page: DocPage,
    paper_width: f32,
    paper_pad: Option<(f32, f32)>,
    flush: bool,
    scroll: Option<ScrollHandle>,
    selection: Option<TextSelection>,
    on_selection_change: Option<SelectionHandler>,
    span: Option<MessageSelection>,
    on_span: Option<SpanHandler>,
}

/// The pane's page on its surface-2 ground.
pub fn doc_pane(id: impl Into<ElementId>, page: DocPage) -> DocPane {
    DocPane {
        id: id.into(),
        page,
        paper_width: PAPER_W,
        paper_pad: None,
        flush: false,
        scroll: None,
        selection: None,
        on_selection_change: None,
        span: None,
        on_span: None,
    }
}

impl DocPane {
    /// Overrides the 520 px page (the assistant screens use 340 with 34 / 36 padding).
    pub fn paper(mut self, width: f32, pad_y: f32, pad_x: f32) -> Self {
        self.paper_width = width;
        self.paper_pad = Some((pad_y, pad_x));
        self
    }

    /// Edge-to-edge mode for hosts that pass the full pane width as the
    /// paper width: no root gutters, no card chrome on the paper (it IS the
    /// pane), and the paper's ground fills the pane height when the content
    /// is short. The inner text padding from [`paper`](Self::paper) stays.
    /// Off by default, so existing callers render exactly as before.
    pub fn flush(mut self, flush: bool) -> Self {
        self.flush = flush;
        self
    }

    /// Tracks the pane's vertical scroll position with `handle`, so a host
    /// can read or restore the offset. The pane scrolls vertically whether
    /// or not a handle is given.
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }

    /// The stored selection this render highlights: the app owns one
    /// [`TextSelection`] and passes it back here. Ignored while span mode is
    /// on (see [`span_selection`](Self::span_selection)).
    pub fn selection(mut self, selection: Option<&TextSelection>) -> Self {
        self.selection = selection.cloned();
        self
    }

    /// Selection intents: drags and word / paragraph picks arrive as `Some`,
    /// plain clicks elsewhere in a cell arrive as `None` (clearing). Not
    /// wired while span mode is on — cells report [`SpanEvent`](crate::transcript::SpanEvent)s
    /// instead, so wiring both would double-report drags.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(f));
        self
    }

    /// The stored cross-cell span this render highlights: the app owns one
    /// [`MessageSelection`] per pane (plus its [`SpanSession`](crate::transcript::SpanSession))
    /// and passes it back here. While span mode is on — a span or a span-event
    /// handler is set — cells highlight from the span and [`selection`](Self::selection)
    /// is ignored.
    pub fn span_selection(mut self, selection: Option<&MessageSelection>) -> Self {
        self.span = selection.cloned();
        self
    }

    /// Cross-cell selection events for the pane's [`SpanSession`](crate::transcript::SpanSession):
    /// presses, hovers while any button is held, releases, and word /
    /// paragraph picks. A pane in span mode wires this instead of
    /// [`on_selection_change`](Self::on_selection_change).
    pub fn on_span_event(mut self, f: impl Fn(crate::transcript::SpanEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_span = Some(Rc::new(f));
        self
    }
}

/// Selection state threaded through the page render: the app's stored
/// selection or span, the intents, and the highlight colour. Span mode is on
/// when the app holds a span or listens for span events; then cells highlight
/// from the span's document order and the legacy single-cell selection is
/// ignored, exactly like the transcript.
struct DocSel {
    selection: Option<TextSelection>,
    on_change: Option<SelectionHandler>,
    span_order: Option<Rc<Vec<SelectionKey>>>,
    span: Option<MessageSelection>,
    on_span: Option<SpanHandler>,
    color: Hsla,
}

/// One body-text cell: plain `StyledText` when no selection state and no
/// intents are wired (existing callers paint exactly as before), otherwise a
/// [`selectable_text`] cell carrying the same runs, so drags and word /
/// paragraph picks report through the pane's handlers and the theme's
/// selection colour highlights behind the glyphs.
fn doc_text_cell(pane_id: &ElementId, key: SelectionKey, text: String, runs: Vec<TextRun>, sel: &DocSel) -> AnyElement {
    if sel.selection.is_none() && sel.span.is_none() && sel.on_change.is_none() && sel.on_span.is_none() {
        return StyledText::new(text).with_runs(runs).into_any_element();
    }
    if text.is_empty() {
        return div().into_any_element();
    }
    let len = text.len();
    let el_id: ElementId = (pane_id.clone(), SharedString::from(key.as_str())).into();
    let mut el = selectable_text(el_id, key.clone(), text).runs(runs).selection_color(sel.color);
    if let Some(order) = &sel.span_order {
        if let Some(range) = sel.span.as_ref().and_then(|span| span.range_for_cell(&key, len, order)) {
            el = el.selection(Some(range));
        }
        if let Some(handler) = &sel.on_span {
            let handler = handler.clone();
            el = el.on_span_event(move |event, window, cx| handler(event, window, cx));
        }
    } else {
        if let Some(range) = sel.selection.as_ref().filter(|current| current.cell == key).map(|current| current.range.clone()) {
            el = el.selection(Some(range));
        }
        if let Some(handler) = &sel.on_change {
            let handler = handler.clone();
            el = el.on_selection_change(move |next, window, cx| handler(next, window, cx));
        }
    }
    el.into_any_element()
}

impl RenderOnce for DocPane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let ink: Hsla = rgb(PAPER_INK).into();
        let mut paper = v_flex()
            .px(px(self.paper_pad.map(|(_, x)| x).unwrap_or(PAPER_PAD_X)))
            .py(px(self.paper_pad.map(|(y, _)| y).unwrap_or(PAPER_PAD_Y)))
            .bg(rgb(PAPER_BG))
            .text_color(ink);
        if self.flush {
            // The paper IS the pane: full width, at least the pane height
            // when short, with no card chrome. The text padding above stays.
            paper = paper.flex_1().w_full().min_h(relative(1.0));
        } else {
            // The paper caps at the pane width so a 520 px sheet in a 280 px
            // pane wraps its text instead of clipping mid-glyph on both sides.
            paper = paper.flex_none().w(px(self.paper_width)).max_w(relative(1.0)).shadow(p.shadow(2));
        }
        paper = paper.child(
                div()
                    .mb(px(PAPER_TITLE_GAP))
                    .ui(PAPER_TITLE)
                    .line_height(relative(PAPER_TITLE_LH))
                    .font_weight(FontWeight::BOLD)
                    .child(self.page.title.clone()),
            )
            .child(
                div()
                    .mb(px(PAPER_META_GAP))
                    .ui(PAPER_META)
                    .line_height(relative(PAPER_META_LH))
                    .text_color(rgb(PAPER_META_INK))
                    .child(self.page.subtitle.clone()),
            );

        let count = self.page.blocks.len();
        let paper_pad_x = self.paper_pad.map(|(_, x)| x).unwrap_or(PAPER_PAD_X);
        let usable_width = self.paper_width - paper_pad_x * 2.0;
        // Span mode is on when the app holds a span or listens for span
        // events; the order walk is keys only (no shaping), so this adds no
        // layout work beyond what the legacy path already does per frame.
        let span_mode = self.span.is_some() || self.on_span.is_some();
        let sel = DocSel {
            selection: self.selection.clone(),
            on_change: self.on_selection_change.clone(),
            span_order: span_mode.then(|| Rc::new(doc_order(&self.page))),
            span: self.span.clone(),
            on_span: self.on_span.clone(),
            color: p.selection,
        };
        let mut body = v_flex()
            .w_full()
            .font_family(PAPER_FONT)
            .text_px(PAPER_TEXT)
            .line_height(relative(PAPER_LH));
        for (index, block) in self.page.blocks.iter().enumerate() {
            // CSS collapses adjacent vertical margins; the larger of the two wins.
            let gap = if index + 1 == count { 0.0 } else { collapsed_margin(block, &self.page.blocks[index + 1]) };
            let el = match block {
                DocBlock::Paragraph(runs) => {
                    let (text, text_runs) = page_runs(runs, ink);
                    div().w_full().child(doc_text_cell(&self.id, SelectionKey::paragraph("", index), text, text_runs, &sel)).into_any_element()
                }
                DocBlock::List(items) => {
                    let mut list = v_flex().w_full();
                    for (n, item) in items.iter().enumerate() {
                        let (text, text_runs) = page_runs(item, ink);
                        list = list.child(
                            h_flex()
                                .w_full()
                                .items_start()
                                // The marker sits in the list's 18 px indent,
                                // right-aligned against the content edge.
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(LIST_INDENT - LIST_MARKER_GAP))
                                        .mr(px(LIST_MARKER_GAP))
                                        .flex()
                                        .justify_end()
                                        .child(format!("{}.", n + 1)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .child(doc_text_cell(&self.id, SelectionKey::list_item("", index, true, n), text, text_runs, &sel)),
                                ),
                        );
                    }
                    list.into_any_element()
                }
                DocBlock::Heading { level, runs } => {
                    let (text, mut text_runs) = page_runs(runs, ink);
                    // The runs carry the serif face; a heading speaks in the UI
                    // face instead, bold at its level's size.
                    let ui = font(scale::FONT_UI);
                    for run in &mut text_runs {
                        run.font.family = ui.family.clone();
                        run.font.weight = FontWeight::BOLD;
                    }
                    div()
                        .w_full()
                        .ui(heading_text_size(*level))
                        .font_weight(FontWeight::BOLD)
                        .child(doc_text_cell(&self.id, SelectionKey::heading("", index), text, text_runs, &sel))
                        .into_any_element()
                }
                DocBlock::Table(table) => {
                    let scroll_id: ElementId = (self.id.clone(), SharedString::from(format!("table-{index}-scroll"))).into();
                    paint_table(table, ink, usable_width, scroll_id, &self.id, index, &sel)
                }
                DocBlock::Image(image) => div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(
                        gpui::img(image.source.clone())
                            .w(px(image.width))
                            .h(px(image.height))
                            .aria_label(image.alt.clone()),
                    )
                    .into_any_element(),
                DocBlock::Rule => div().w_full().h(px(1.0)).bg(rgb(RULE_COLOUR)).into_any_element(),
            };
            body = body.child(div().w_full().mb(px(gap)).child(el));
        }

        paper = paper.child(body);
        let mut root = div()
            .id(self.id)
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .flex()
            .justify_center()
            .overflow_hidden()
            .overflow_y_scroll();
        if self.flush {
            // Edge to edge: no gutters, on the paper's own ground so no
            // surface band shows below a short page. Top-aligned exactly
            // like the framed pane, so long content still scrolls; the
            // paper's own minimum height fills short pages.
            root = root.items_start().p(px(0.0)).bg(rgb(PAPER_BG));
        } else {
            // Top-align: the row's default cross-axis stretch would size the
            // paper to the pane and leave nothing to scroll.
            root = root.items_start().p(px(DOC_PAD)).bg(p.surface_2);
        }
        if let Some(handle) = &self.scroll {
            root = root.track_scroll(handle);
        }
        root.child(paper)
    }
}

/// Paints a [`DocTable`] as a real grid: rows of cells sized in the caller's
/// proportions, hairline `#DDD` borders with no doubled outer edge, header
/// cells on the `#F5F6F9` ground in the UI face, body cells in Georgia. Cell
/// text goes through [`page_runs`] so Bold and Changed runs work exactly as
/// they do in a paragraph; cells wrap, never truncate. A malformed table (no
/// rows, ragged spans) paints what it can instead of panicking.
///
/// Column widths come from [`table_layout`] ONCE for the whole table, so every
/// row shares the same boundaries. When the columns fit `usable_width` the
/// cells size by `relative` fraction exactly as before; when they do not, the
/// table lays out at its natural width inside a horizontally scrolling frame,
/// like the transcript's markdown tables. Padding is counted INSIDE the
/// computed width: `TABLE_MIN_COL_W` already holds the 8 px pads on each side
/// plus the word "Female", so flooring the outer width at the minimum keeps
/// the usable text readable instead of letting the pads eat the column.
fn paint_table(
    table: &DocTable,
    ink: Hsla,
    usable_width: f32,
    scroll_id: ElementId,
    pane_id: &ElementId,
    block: usize,
    sel: &DocSel,
) -> AnyElement {
    if table.rows.is_empty() {
        return div().w_full().into_any_element();
    }
    let edge: Hsla = rgb(TABLE_BORDER).into();
    let col_count = table.rows.iter().map(|row| row.iter().map(|cell| cell.col_span.max(1)).sum::<usize>()).max().unwrap_or(0);
    if col_count == 0 {
        return div().w_full().into_any_element();
    }
    let (col_widths, scroll) = table_layout(&table.widths, col_count, usable_width);
    let total: f32 = col_widths.iter().sum();
    let mut grid = if scroll { v_flex().w(px(total)).flex_none() } else { v_flex().w_full() };
    grid = grid.border_1().border_color(edge);
    let last_row = table.rows.len() - 1;
    for (ri, row) in table.rows.iter().enumerate() {
        let mut line = if scroll { h_flex().w(px(total)).flex_none() } else { h_flex().w_full() };
        let mut index = 0usize;
        let last_cell = row.len().saturating_sub(1);
        for (ci, cell) in row.iter().enumerate() {
            let start = index.min(col_count);
            let end = index.saturating_add(cell.col_span.max(1)).min(col_count);
            let cell_px: f32 = if end <= start { TABLE_MIN_COL_W } else { col_widths[start..end].iter().sum() };
            index += cell.col_span.max(1);
            let (text, mut text_runs) = page_runs(&cell.runs, ink);
            let mut el = if scroll {
                div().w(px(cell_px)).flex_none()
            } else {
                let frac = if total > 0.0 { cell_px / total } else { 1.0 / (col_count as f32) };
                div().w(relative(frac)).min_w(px(0.0))
            };
            el = el.px(px(TABLE_CELL_PAD_X)).py(px(TABLE_CELL_PAD_Y)).border_color(edge);
            if cell.header {
                // The runs carry the serif face; a header speaks in the UI
                // face instead, semibold at 11 px.
                let ui = font(scale::FONT_UI);
                for run in &mut text_runs {
                    run.font.family = ui.family.clone();
                    run.font.weight = FontWeight::SEMIBOLD;
                }
                el = el.bg(rgb(TABLE_HEADER_GROUND)).ui(TABLE_HEADER_TEXT).font_weight(FontWeight::SEMIBOLD);
            } else {
                el = el.font_family(PAPER_FONT).text_px(TABLE_BODY_TEXT);
            }
            // The table's own border draws the outer edge; cells only rule
            // their inner sides.
            if ci != last_cell {
                el = el.border_r_1();
            }
            if ri != last_row {
                el = el.border_b_1();
            }
            let key = SelectionKey::table_cell("", block, Some(ri), ci);
            line = line.child(el.child(doc_text_cell(pane_id, key, text, text_runs, sel)));
        }
        grid = grid.child(line);
    }
    if scroll {
        // Like the transcript's markdown tables, a wide table gets its own
        // horizontally scrolling frame instead of squeezing its columns; the
        // id keeps the scroll position across re-renders.
        div().w_full().id(scroll_id).overflow_x_scroll().child(grid).into_any_element()
    } else {
        grid.into_any_element()
    }
}

/// Turns the page runs into styled text: bold clause openers keep the serif
/// face, changed spans carry the accent ground and underline.
fn page_runs(runs: &[DocRun], ink: Hsla) -> (String, Vec<TextRun>) {
    let serif = font(PAPER_FONT);
    let mut text = String::new();
    let mut out = Vec::new();
    for run in runs {
        let s = run.str();
        let mut style = TextRun { len: s.len(), font: serif.clone(), color: ink, background_color: None, underline: None, strikethrough: None };
        match run {
            DocRun::Text(_) => {}
            DocRun::Bold(_) => style.font.weight = FontWeight::BOLD,
            DocRun::Changed(_) => {
                let accent: Hsla = rgb(CHANGE_INK).into();
                style.background_color = Some(accent.alpha(CHANGE_GROUND_ALPHA));
                style.underline = Some(UnderlineStyle { thickness: px(CHANGE_UNDERLINE), color: Some(accent), wavy: false });
            }
        }
        text.push_str(s);
        out.push(style);
    }
    (text, out)
}

// ---------------------------------------------------------------------------
// Selection keys and copy text
// ---------------------------------------------------------------------------

/// The plain shaped text of `runs`: what [`page_runs`] draws, minus styling.
fn doc_runs_text(runs: &[DocRun]) -> String {
    let mut text = String::new();
    for run in runs {
        text.push_str(run.str());
    }
    text
}

/// The selectable cells of a page in render order: one key per paragraph,
/// heading and list item, one per table cell row-major; images and rules are
/// not text. The render builds this once per frame when span mode is on, so
/// the keys cells paint with are the keys a span resolves against.
fn doc_order(page: &DocPage) -> Vec<SelectionKey> {
    let mut order = Vec::new();
    for (index, block) in page.blocks.iter().enumerate() {
        match block {
            DocBlock::Paragraph(_) => order.push(SelectionKey::paragraph("", index)),
            DocBlock::Heading { .. } => order.push(SelectionKey::heading("", index)),
            DocBlock::List(items) => {
                for (n, _) in items.iter().enumerate() {
                    order.push(SelectionKey::list_item("", index, true, n));
                }
            }
            DocBlock::Table(table) => {
                for (r, row) in table.rows.iter().enumerate() {
                    for (c, _) in row.iter().enumerate() {
                        order.push(SelectionKey::table_cell("", index, Some(r), c));
                    }
                }
            }
            DocBlock::Image(_) | DocBlock::Rule => {}
        }
    }
    order
}

/// The shaped text of the cell `key` addresses, or `None` for keys the page
/// no longer renders — stale selections select nothing.
fn doc_cell_text(page: &DocPage, key: &SelectionKey) -> Option<String> {
    for (index, block) in page.blocks.iter().enumerate() {
        match block {
            DocBlock::Paragraph(runs) if *key == SelectionKey::paragraph("", index) => return Some(doc_runs_text(runs)),
            DocBlock::Heading { runs, .. } if *key == SelectionKey::heading("", index) => return Some(doc_runs_text(runs)),
            DocBlock::List(items) => {
                for (n, item) in items.iter().enumerate() {
                    if *key == SelectionKey::list_item("", index, true, n) {
                        return Some(doc_runs_text(item));
                    }
                }
            }
            DocBlock::Table(table) => {
                for (r, row) in table.rows.iter().enumerate() {
                    for (c, cell) in row.iter().enumerate() {
                        if *key == SelectionKey::table_cell("", index, Some(r), c) {
                            return Some(doc_runs_text(&cell.runs));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Clamps a range to `text`, snapping both ends down to char boundaries;
/// `None` when nothing remains. Every slice into cell text goes through here,
/// so event indices can never panic a copy.
fn clamp_bytes(range: Range<usize>, text: &str) -> Option<Range<usize>> {
    if text.is_empty() {
        return None;
    }
    let len = text.len();
    let mut start = range.start.min(len);
    let mut end = range.end.min(len);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    if start >= end { None } else { Some(start..end) }
}

/// Copies the selected text out of `selection`: the slice of the holding
/// cell's shaped text, or `None` when the key addresses no cell or the range
/// is empty. The app puts this on the clipboard on ⌘C; the keybinding stays
/// with the app.
pub fn doc_selected_text(page: &DocPage, selection: &TextSelection) -> Option<String> {
    let cell = doc_cell_text(page, &selection.cell)?;
    clamp_bytes(selection.range.clone(), &cell).map(|range| cell[range].to_string())
}

/// One selectable cell's copy text with its structure: list items carry their
/// `N. ` marker so a copied span pastes back as the same list, and table
/// cells carry their `(block, row)` so a span joins them as TSV.
struct DocSlice {
    /// The key the cell paints with.
    key: SelectionKey,
    /// The shaped text: what the cell draws, minus styling.
    text: String,
    /// The list block this item belongs to, for tight joining; `None` for
    /// every other cell.
    list: Option<usize>,
    /// The structural marker prepended on copy, if any.
    marker: Option<String>,
    /// The `(block, row)` of a table cell, for TSV joining; `None` for every
    /// other cell.
    table: Option<(usize, usize)>,
}

/// Walks `page` in render order, pushing one [`DocSlice`] per selectable
/// cell. The traversal mirrors [`doc_order`], so the keys found here are the
/// keys cells paint with.
fn doc_slices(page: &DocPage) -> Vec<DocSlice> {
    let mut out = Vec::new();
    for (index, block) in page.blocks.iter().enumerate() {
        match block {
            DocBlock::Paragraph(runs) => out.push(DocSlice {
                key: SelectionKey::paragraph("", index),
                text: doc_runs_text(runs),
                list: None,
                marker: None,
                table: None,
            }),
            DocBlock::Heading { runs, .. } => out.push(DocSlice {
                key: SelectionKey::heading("", index),
                text: doc_runs_text(runs),
                list: None,
                marker: None,
                table: None,
            }),
            DocBlock::List(items) => {
                for (n, item) in items.iter().enumerate() {
                    out.push(DocSlice {
                        key: SelectionKey::list_item("", index, true, n),
                        text: doc_runs_text(item),
                        list: Some(index),
                        marker: Some(format!("{}. ", n + 1)),
                        table: None,
                    });
                }
            }
            DocBlock::Table(table) => {
                for (r, row) in table.rows.iter().enumerate() {
                    for (c, cell) in row.iter().enumerate() {
                        out.push(DocSlice {
                            key: SelectionKey::table_cell("", index, Some(r), c),
                            text: doc_runs_text(&cell.runs),
                            list: None,
                            marker: None,
                            table: Some((index, r)),
                        });
                    }
                }
            }
            DocBlock::Image(_) | DocBlock::Rule => {}
        }
    }
    out
}

/// Copies the selected text across `selection`'s cells in document order. The
/// app puts this on the clipboard on ⌘C when it holds a span; the keybinding
/// stays with the app.
///
/// Cells between the ends copy whole; the ends copy their overlapped slice.
/// Fragments join with a blank line between blocks — the copy reads the way
/// the pane renders — except consecutive items of one list, which join tight
/// so the list pastes back as a list, and table cells, which join as TSV
/// (tabs within a row, newlines across rows). Wholly selected list items keep
/// their `N. ` markers, while partial slices read as plain words exactly like
/// the legacy single-cell copy. Empty cells contribute nothing, so they never
/// pile up blank lines. `None` when an endpoint key addresses no cell or
/// nothing remains.
pub fn doc_span_selected_text(page: &DocPage, selection: &MessageSelection) -> Option<String> {
    let cells = doc_slices(page);
    let position = |key: &SelectionKey| cells.iter().position(|cell| cell.key == *key);
    let anchor_ix = position(&selection.anchor.cell)?;
    let focus_ix = position(&selection.focus.cell)?;
    let (lo_ix, lo_off, hi_ix, hi_off) = if anchor_ix <= focus_ix {
        (anchor_ix, selection.anchor.offset, focus_ix, selection.focus.offset)
    } else {
        (focus_ix, selection.focus.offset, anchor_ix, selection.anchor.offset)
    };
    let mut out = String::new();
    let mut prev_list: Option<usize> = None;
    let mut prev_table: Option<(usize, usize)> = None;
    for (ix, cell) in cells.iter().enumerate().skip(lo_ix).take(hi_ix - lo_ix + 1) {
        let full = cell.text.as_str();
        let bounds = if ix == lo_ix && ix == hi_ix {
            let (start, end) = if lo_off <= hi_off { (lo_off, hi_off) } else { (hi_off, lo_off) };
            start..end
        } else if ix == lo_ix {
            lo_off..full.len()
        } else if ix == hi_ix {
            0..hi_off
        } else {
            0..full.len()
        };
        let Some(slice) = clamp_bytes(bounds, full) else {
            continue;
        };
        // Markers ride only on wholly selected cells: a partial slice reads
        // exactly like the legacy single-cell copy, while fully covered
        // items keep their structure so the span pastes back as a list.
        let whole = slice.start == 0 && slice.end == full.len();
        let fragment = if whole {
            if let Some(marker) = &cell.marker {
                marker.clone() + &full[slice]
            } else {
                full[slice].to_string()
            }
        } else {
            full[slice].to_string()
        };
        // A table cell's own tabs and line breaks would split it into extra
        // columns or rows on paste, so they flatten to spaces inside a table.
        let fragment = if cell.table.is_some() {
            fragment.replace(['\t', '\r', '\n'], " ")
        } else {
            fragment
        };
        if !out.is_empty() {
            // Cells of one table row stay one TSV row; consecutive items of
            // one list stay one list; rows of one table stay line-separated;
            // every other boundary is a block boundary, hence a blank line.
            let same_row = matches!((prev_table, cell.table), (Some(a), Some(b)) if a == b);
            let tight_list = cell.list.is_some() && prev_list == cell.list;
            let same_table = matches!((prev_table, cell.table), (Some(a), Some(b)) if a.0 == b.0);
            out.push_str(if same_row {
                "\t"
            } else if tight_list || same_table {
                "\n"
            } else {
                "\n\n"
            });
        }
        out.push_str(&fragment);
        prev_list = cell.list;
        prev_table = cell.table;
    }
    if out.is_empty() { None } else { Some(out) }
}

/// The span covering a whole page: the first non-empty cell's start to the
/// last non-empty cell's end. `None` for pages with no selectable text. The
/// app drives select-all through this; the keybinding stays with the app.
pub fn doc_select_all(page: &DocPage) -> Option<MessageSelection> {
    let cells = doc_slices(page);
    let first = cells.iter().find(|cell| !cell.text.is_empty())?;
    let last = cells.iter().rfind(|cell| !cell.text.is_empty())?;
    MessageSelection::new(
        SelectionEndpoint { cell: first.key.clone(), offset: 0 },
        SelectionEndpoint { cell: last.key.clone(), offset: last.text.len() },
    )
}

// ---------------------------------------------------------------------------
// Artifacts created in chat
// ---------------------------------------------------------------------------

/// The kind of an artifact chip: its glyph and, for the two office kinds, the
/// status hue the design tints the glyph with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    /// A text document (`doc`, tinted info).
    Doc,
    /// A spreadsheet (`sheet`, tinted success).
    Sheet,
    /// A slide deck.
    Slides,
    /// A PDF.
    Pdf,
    /// A source file.
    Code,
    /// A plain note or markdown file.
    Note,
    /// An image.
    Image,
    /// Anything else.
    Other,
}

impl ArtifactKind {
    /// The glyph for this kind.
    pub fn icon(self) -> IconName {
        match self {
            ArtifactKind::Doc => IconName::Doc,
            ArtifactKind::Sheet => IconName::Sheet,
            ArtifactKind::Slides => IconName::Layout,
            ArtifactKind::Pdf => IconName::Pdf,
            ArtifactKind::Code => IconName::Terminal,
            ArtifactKind::Note => IconName::File,
            ArtifactKind::Image => IconName::Image,
            ArtifactKind::Other => IconName::File,
        }
    }

    /// The glyph's tint; `None` inherits the chip's text colour.
    pub fn tint(self, colors: &aui_tokens::Palette) -> Option<Hsla> {
        match self {
            ArtifactKind::Doc => Some(colors.info),
            ArtifactKind::Sheet => Some(colors.success),
            ArtifactKind::Slides | ArtifactKind::Pdf | ArtifactKind::Code | ArtifactKind::Note | ArtifactKind::Image | ArtifactKind::Other => None,
        }
    }
}

/// One artifact the chat created.
#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    /// File name, shown on the chip.
    pub name: SharedString,
    /// The kind, which picks the glyph and its tint.
    pub kind: ArtifactKind,
    /// The version tag after the name (`v3`), if the artifact carries one.
    pub version: Option<SharedString>,
    /// The artifact open in the pane.
    pub active: bool,
}

impl Artifact {
    /// An artifact chip.
    pub fn new(name: impl Into<SharedString>, kind: ArtifactKind) -> Self {
        Self { name: name.into(), kind, version: None, active: false }
    }

    /// Sets the version tag.
    pub fn version(mut self, version: impl Into<SharedString>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Marks the artifact as the one open in the pane.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// The "Created in chat" strip (`.arts`). Build with [`artifact_strip`].
#[derive(IntoElement)]
pub struct ArtifactStrip {
    id: ElementId,
    label: SharedString,
    artifacts: Vec<Artifact>,
    max_visible: Option<usize>,
    on_select: Option<SelectHandler>,
    on_more: Option<MoreHandler>,
}

/// The strip of artifacts the chat produced, under the page.
pub fn artifact_strip(id: impl Into<ElementId>, artifacts: Vec<Artifact>) -> ArtifactStrip {
    ArtifactStrip { id: id.into(), label: "Created in chat".into(), artifacts, max_visible: None, on_select: None, on_more: None }
}

/// Splits `total` artifacts into the visible chips and the hidden count
/// behind the `+N` chip under the `max_visible` cap.
fn overflow_counts(total: usize, max_visible: Option<usize>) -> (usize, usize) {
    let visible = max_visible.unwrap_or(total).min(total);
    (visible, total - visible)
}

impl ArtifactStrip {
    /// Overrides the caps label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Shows at most `n` chips and gathers the rest behind a `+N` chip
    /// (`.art.more`). Chip names always truncate, so nothing is ever sliced at
    /// the strip's edge; the cap is what keeps a narrow pane's names readable
    /// instead of shrinking every chip to two letters.
    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = Some(n);
        self
    }

    /// Called with the artifact's name when a chip is clicked.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// Called when the `+N` overflow chip is clicked; when set, the chip is
    /// a button, and when unset it renders exactly as before.
    pub fn on_more(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_more = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ArtifactStrip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut strip = h_flex()
            .id(id.clone())
            .w_full()
            .flex_none()
            .gap(px(ARTS_GAP))
            .px(px(ARTS_PAD_X))
            .py(px(ARTS_PAD_Y))
            .border_t_1()
            .border_color(p.line)
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .mr(px(ARTS_LABEL_MARGIN))
                    .text_role(TextRole::Caps)
                    .text_color(p.ink_3)
                    .child(self.label.to_uppercase()),
            );
        let (visible, hidden) = overflow_counts(self.artifacts.len(), self.max_visible);
        for artifact in self.artifacts.into_iter().take(visible) {
            let chip_id: ElementId = (id.clone(), artifact.name.clone()).into();
            let (border, bg, text) = if artifact.active {
                (p.accent_ring, p.accent_soft, p.ink)
            } else {
                (p.line, gpui::transparent_black(), p.ink_2)
            };
            let mut el = h_flex()
                .id(chip_id)
                .flex_shrink(ART_SHRINK)
                .min_w(px(0.0))
                .overflow_hidden()
                .h(px(ART_H))
                .pl(px(ART_PAD_LEFT))
                .pr(px(ART_PAD_RIGHT))
                .gap(px(ART_GAP))
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(border)
                .bg(bg)
                .text_color(text)
                .ui(ART_TEXT)
                .whitespace_nowrap()
                .cursor_pointer()
                .child(icon(artifact.kind.icon()).size(px(CHIP_GLYPH)).color(artifact.kind.tint(&p).unwrap_or(text)))
                // `.art .nm{overflow:hidden;text-overflow:ellipsis}`: the name is
                // the only part of a chip that gives way, so a strip short of
                // room truncates names instead of slicing the last chip.
                .child(div().min_w(px(0.0)).truncate().child(artifact.name.clone()));
            if let Some(version) = artifact.version.clone() {
                el = el.child(
                    div()
                        .flex_none()
                        .text_role(TextRole::MonoSmall)
                        .text_px(ART_VERSION_TEXT)
                        .medium()
                        .text_color(p.ink_3)
                        .child(version),
                );
            }
            if let Some(f) = self.on_select.clone() {
                let name = artifact.name.clone();
                el = el.on_click(move |_, w, cx| f(&name, w, cx));
            }
            strip = strip.child(el);
        }
        if hidden > 0 {
            let mut more = h_flex()
                .id((id, "more"))
                .flex_none()
                .h(px(ART_H))
                .px(px(ART_MORE_PAD))
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(p.line)
                .text_color(p.ink_3)
                .ui(ART_TEXT)
                .whitespace_nowrap()
                .cursor_pointer()
                .child(format!("+{hidden}"));
            if let Some(f) = self.on_more.clone() {
                more = more.on_click(move |_, w, cx| f(w, cx));
            }
            strip = strip.child(more);
        }
        strip
    }
}

// ---------------------------------------------------------------------------
// Status row
// ---------------------------------------------------------------------------

/// One item of a pane status row.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneStatusItem {
    /// The text.
    pub text: SharedString,
    /// Accent-ink instead of ink-3 (the "1 change from chat highlighted" note).
    pub accent: bool,
}

/// A plain status item.
pub fn pane_status(text: impl Into<SharedString>) -> PaneStatusItem {
    PaneStatusItem { text: text.into(), accent: false }
}

impl PaneStatusItem {
    /// Colours the item accent-ink.
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }
}

/// The pane status row (`.stat`). Build with [`pane_status_row`].
#[derive(IntoElement)]
pub struct PaneStatusRow {
    id: ElementId,
    left: Vec<PaneStatusItem>,
    right: Vec<PaneStatusItem>,
}

/// A 28 px status row with items on the left and on the right of a spacer.
pub fn pane_status_row(id: impl Into<ElementId>, left: Vec<PaneStatusItem>, right: Vec<PaneStatusItem>) -> PaneStatusRow {
    PaneStatusRow { id: id.into(), left, right }
}

impl RenderOnce for PaneStatusRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let item = move |i: PaneStatusItem| {
            div()
                .flex_none()
                .text_color(if i.accent { p.accent_ink } else { p.ink_3 })
                .whitespace_nowrap()
                .child(i.text)
        };
        h_flex()
            .id(self.id)
            .w_full()
            .flex_none()
            .h(px(STATUS_H))
            .gap(px(STATUS_GAP))
            .px(px(STATUS_PAD))
            .border_t_1()
            .border_color(p.line)
            .ui(STATUS_TEXT)
            .text_color(p.ink_3)
            .children(self.left.into_iter().map(item))
            .child(div().flex_1().min_w(px(0.0)))
            .children(self.right.into_iter().map(item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifacts(n: usize) -> Vec<Artifact> {
        (0..n).map(|i| Artifact::new(format!("file-{i}.md"), ArtifactKind::Note)).collect()
    }

    /// The `+N` chip only exists past the cap: everything fits without one.
    #[test]
    fn no_overflow_chip_at_or_below_the_cap() {
        assert_eq!(overflow_counts(2, Some(2)), (2, 0), "two artifacts fit two slots with nothing hidden");
        assert_eq!(overflow_counts(1, Some(2)), (1, 0), "one artifact fits two slots with nothing hidden");
        assert_eq!(overflow_counts(2, None), (2, 0), "with no cap nothing is hidden");
    }

    /// Past the cap the rest gather behind the `+N` chip.
    #[test]
    fn overflow_chip_counts_what_the_cap_hides() {
        assert_eq!(overflow_counts(3, Some(2)), (2, 1), "three artifacts over two slots hide one");
        assert_eq!(overflow_counts(5, Some(2)), (2, 3), "five artifacts over two slots hide three");
    }

    /// `on_more` presence toggles the `+N` chip being a button.
    #[test]
    fn on_more_presence_toggles_the_overflow_button() {
        let plain = artifact_strip("strip", artifacts(3)).max_visible(2);
        assert!(plain.on_more.is_none(), "with no handler the chip renders as today");
        let wired = artifact_strip("strip", artifacts(3)).max_visible(2).on_more(|_, _| {});
        assert!(wired.on_more.is_some(), "with a handler the chip is a button");
    }

    /// The scroll handle is stored when set, so the pane can report it.
    #[test]
    fn track_scroll_stores_the_handle() {
        let handle = ScrollHandle::new();
        let plain = doc_pane("pane", DocPage::new("Title", "Subtitle", Vec::new()));
        assert!(plain.scroll.is_none(), "no handle is tracked by default");
        let tracked = doc_pane("pane", DocPage::new("Title", "Subtitle", Vec::new())).track_scroll(&handle);
        assert!(tracked.scroll.is_some(), "the handle is stored when set");
    }

    /// Flush mode is opt-in: every existing caller renders exactly as before.
    #[test]
    fn flush_is_off_by_default() {
        let plain = doc_pane("pane", DocPage::new("Title", "Subtitle", Vec::new()));
        assert!(!plain.flush, "no pane is flush unless asked");
        let framed = doc_pane("pane", DocPage::new("Title", "Subtitle", Vec::new())).flush(false);
        assert!(!framed.flush, "flush(false) keeps the framed pane");
        let flush = doc_pane("pane", DocPage::new("Title", "Subtitle", Vec::new())).flush(true);
        assert!(flush.flush, "flush(true) opts into the edge-to-edge pane");
    }

    /// Flush geometry: the paper spans the pane's full width.
    #[test]
    fn flush_paper_spans_the_pane_width() {
        let (width, _) = flush_paper_layout(640.0, 500.0, 120.0);
        assert_eq!(width, 640.0, "a flush paper is as wide as the pane, got {width}");
        let (narrow, _) = flush_paper_layout(280.0, 500.0, 120.0);
        assert_eq!(narrow, 280.0, "a flush paper in a narrow pane is as wide as the pane, got {narrow}");
    }

    /// Flush geometry: one short paragraph still fills the pane height,
    /// while a long page keeps its content height and scrolls.
    #[test]
    fn flush_paper_fills_the_pane_height_when_short() {
        let (_, short) = flush_paper_layout(640.0, 500.0, 120.0);
        assert_eq!(short, 500.0, "a short page fills the pane height, got {short}");
        let (_, long) = flush_paper_layout(640.0, 500.0, 1400.0);
        assert_eq!(long, 1400.0, "a long page keeps its height and scrolls, got {long}");
    }

    fn select_page() -> DocPage {
        DocPage::new(
            "Title",
            "Subtitle",
            vec![
                DocBlock::Paragraph(vec![DocRun::text("First paragraph.")]),
                DocBlock::heading(1, "Section"),
                DocBlock::list(["alpha", "beta"]),
                DocBlock::Table(DocTable {
                    widths: vec![],
                    rows: vec![
                        vec![DocCell::text("a1"), DocCell::text("b1")],
                        vec![DocCell::text("a2"), DocCell::text("b2")],
                    ],
                }),
                DocBlock::Paragraph(vec![DocRun::text("Last paragraph.")]),
            ],
        )
    }

    #[test]
    fn a_cells_own_tab_or_newline_never_splits_the_tsv() {
        let page = DocPage::new(
            "T",
            "",
            vec![DocBlock::Table(DocTable {
                widths: vec![],
                rows: vec![vec![DocCell::text("a\tb"), DocCell::text("line\nbreak")]],
            })],
        );
        let span = doc_select_all(&page).expect("the table has text");
        assert_eq!(
            doc_span_selected_text(&page, &span).as_deref(),
            Some("a b\tline break")
        );
    }

    fn endpoint(cell: &SelectionKey, offset: usize) -> SelectionEndpoint {
        SelectionEndpoint { cell: cell.clone(), offset }
    }

    /// A fresh pane holds no selection state, so existing callers render
    /// exactly as before.
    #[test]
    fn a_plain_pane_holds_no_selection() {
        let pane = doc_pane("pane", select_page());
        assert!(pane.selection.is_none(), "no selection is held by default");
        assert!(pane.on_selection_change.is_none(), "no selection handler by default");
        assert!(pane.span.is_none(), "no span is held by default");
        assert!(pane.on_span.is_none(), "no span handler by default");
    }

    /// A span across the whole page copies cells in document order: blank
    /// lines between blocks, markers on wholly selected list items, TSV for
    /// table cells.
    #[test]
    fn a_span_across_blocks_copies_in_document_order() {
        let page = select_page();
        let span = doc_select_all(&page).expect("the page has selectable text");
        assert_eq!(
            doc_span_selected_text(&page, &span).as_deref(),
            Some("First paragraph.\n\nSection\n\n1. alpha\n2. beta\n\na1\tb1\na2\tb2\n\nLast paragraph."),
            "blocks join with blank lines, lists keep markers, tables read as TSV"
        );
    }

    /// Endpoint cells copy their overlapped slice without markers; middle
    /// cells copy whole with markers; table cells join their row with tabs.
    #[test]
    fn a_partial_span_slices_its_ends_and_keeps_tsv_rows() {
        use crate::transcript::SelectionKey as Key;
        let page = select_page();
        let span = MessageSelection {
            anchor: endpoint(&Key::list_item("", 2, true, 0), 2),
            focus: endpoint(&Key::table_cell("", 3, Some(0), 1), 1),
        };
        assert_eq!(
            doc_span_selected_text(&page, &span).as_deref(),
            Some("pha\n2. beta\n\na1\tb"),
            "partial ends read as plain words, whole items keep markers, row cells tab-join"
        );
        // A reversed drag (focus before anchor) reads exactly the same.
        let reversed = MessageSelection {
            anchor: endpoint(&Key::table_cell("", 3, Some(0), 1), 1),
            focus: endpoint(&Key::list_item("", 2, true, 0), 2),
        };
        assert_eq!(
            doc_span_selected_text(&page, &reversed).as_deref(),
            Some("pha\n2. beta\n\na1\tb"),
            "reversed drags copy in document order"
        );
    }

    /// A table-only span copies rows newline-separated: the TSV shape.
    #[test]
    fn a_table_span_copies_as_tsv() {
        use crate::transcript::SelectionKey as Key;
        let page = select_page();
        let span = MessageSelection {
            anchor: endpoint(&Key::table_cell("", 3, Some(0), 0), 0),
            focus: endpoint(&Key::table_cell("", 3, Some(1), 1), 2),
        };
        assert_eq!(
            doc_span_selected_text(&page, &span).as_deref(),
            Some("a1\tb1\na2\tb2"),
            "cells tab-join within a row, rows newline-join"
        );
    }

    /// The legacy single-cell copy slices the holding cell and rejects stale
    /// keys and empty ranges.
    #[test]
    fn a_single_cell_selection_slices_its_cell() {
        use crate::transcript::SelectionKey as Key;
        let page = select_page();
        let word = TextSelection { cell: Key::paragraph("", 0), range: 0..5 };
        assert_eq!(doc_selected_text(&page, &word).as_deref(), Some("First"), "the holding cell's slice");
        let stale = TextSelection { cell: Key::paragraph("", 9), range: 0..5 };
        assert_eq!(doc_selected_text(&page, &stale), None, "a stale key selects nothing");
        let caret = TextSelection { cell: Key::paragraph("", 0), range: 3..3 };
        assert_eq!(doc_selected_text(&page, &caret), None, "an empty range is never stored");
    }

    /// Spans over unknown keys or empty pages copy nothing.
    #[test]
    fn spans_over_unknown_keys_copy_nothing() {
        use crate::transcript::SelectionKey as Key;
        let page = select_page();
        let span = MessageSelection {
            anchor: endpoint(&Key::paragraph("", 9), 0),
            focus: endpoint(&Key::paragraph("", 0), 5),
        };
        assert_eq!(doc_span_selected_text(&page, &span), None, "a stale endpoint selects nothing");
        let empty = DocPage::new("Title", "Subtitle", vec![DocBlock::Rule]);
        assert_eq!(doc_select_all(&empty), None, "a page with no text has no span");
    }
}
