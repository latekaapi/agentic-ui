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

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{
    div, font, prelude::*, px, relative, rgb, AnyElement, App, ElementId, FontWeight, Hsla, IntoElement, SharedString, StyledText, TextRun,
    UnderlineStyle, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, chip, icon_button, ButtonSize};
use crate::shell::TabItem;
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
        let mut bar = h_flex()
            .id(id.clone())
            .w_full()
            .flex_none()
            .h(px(TOOLBAR_H))
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

/// One block of the page.
#[derive(Debug, Clone, PartialEq)]
pub enum DocBlock {
    /// A paragraph (`.paper p`).
    Paragraph(Vec<DocRun>),
    /// A numbered list (`.paper ol`); items are numbered from 1.
    List(Vec<Vec<DocRun>>),
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

    /// The block's top margin, for CSS-style margin collapsing.
    fn margin_top(&self) -> f32 {
        match self {
            DocBlock::Paragraph(_) => 0.0,
            DocBlock::List(_) => LIST_GAP,
        }
    }

    /// The block's bottom margin.
    fn margin_bottom(&self) -> f32 {
        match self {
            DocBlock::Paragraph(_) => PARAGRAPH_GAP,
            DocBlock::List(_) => LIST_GAP,
        }
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

/// The paper area of the document pane (`.doc` + `.paper`). Build with
/// [`doc_pane`].
#[derive(IntoElement)]
pub struct DocPane {
    id: ElementId,
    page: DocPage,
    paper_width: f32,
    paper_pad: Option<(f32, f32)>,
}

/// The pane's page on its surface-2 ground.
pub fn doc_pane(id: impl Into<ElementId>, page: DocPage) -> DocPane {
    DocPane { id: id.into(), page, paper_width: PAPER_W, paper_pad: None }
}

impl DocPane {
    /// Overrides the 520 px page (the assistant screens use 340 with 34 / 36 padding).
    pub fn paper(mut self, width: f32, pad_y: f32, pad_x: f32) -> Self {
        self.paper_width = width;
        self.paper_pad = Some((pad_y, pad_x));
        self
    }
}

impl RenderOnce for DocPane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let ink: Hsla = rgb(PAPER_INK).into();
        let mut paper = v_flex()
            .flex_none()
            .w(px(self.paper_width))
            .px(px(self.paper_pad.map(|(_, x)| x).unwrap_or(PAPER_PAD_X)))
            .py(px(self.paper_pad.map(|(y, _)| y).unwrap_or(PAPER_PAD_Y)))
            .bg(rgb(PAPER_BG))
            .text_color(ink)
            .shadow(p.shadow(2))
            .child(
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
        let mut body = v_flex()
            .w_full()
            .font_family(PAPER_FONT)
            .text_px(PAPER_TEXT)
            .line_height(relative(PAPER_LH));
        for (index, block) in self.page.blocks.iter().enumerate() {
            // CSS collapses adjacent vertical margins; the larger of the two wins.
            let gap = if index + 1 == count { 0.0 } else { block.margin_bottom().max(self.page.blocks[index + 1].margin_top()) };
            let el = match block {
                DocBlock::Paragraph(runs) => {
                    let (text, text_runs) = page_runs(runs, ink);
                    div().w_full().child(StyledText::new(text).with_runs(text_runs)).into_any_element()
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
                                .child(div().flex_1().min_w(px(0.0)).child(StyledText::new(text).with_runs(text_runs))),
                        );
                    }
                    list.into_any_element()
                }
            };
            body = body.child(div().w_full().mb(px(gap)).child(el));
        }

        paper = paper.child(body);
        div()
            .id(self.id)
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .flex()
            .justify_center()
            .p(px(DOC_PAD))
            .bg(p.surface_2)
            .overflow_hidden()
            .child(paper)
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
    /// A PDF.
    Pdf,
    /// A plain note or markdown file.
    Note,
    /// An image.
    Image,
}

impl ArtifactKind {
    /// The glyph for this kind.
    pub fn icon(self) -> IconName {
        match self {
            ArtifactKind::Doc => IconName::Doc,
            ArtifactKind::Sheet => IconName::Sheet,
            ArtifactKind::Pdf => IconName::Pdf,
            ArtifactKind::Note => IconName::File,
            ArtifactKind::Image => IconName::Image,
        }
    }

    /// The glyph's tint; `None` inherits the chip's text colour.
    pub fn tint(self, colors: &aui_tokens::Palette) -> Option<Hsla> {
        match self {
            ArtifactKind::Doc => Some(colors.info),
            ArtifactKind::Sheet => Some(colors.success),
            ArtifactKind::Pdf | ArtifactKind::Note | ArtifactKind::Image => None,
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
}

/// The strip of artifacts the chat produced, under the page.
pub fn artifact_strip(id: impl Into<ElementId>, artifacts: Vec<Artifact>) -> ArtifactStrip {
    ArtifactStrip { id: id.into(), label: "Created in chat".into(), artifacts, max_visible: None, on_select: None }
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
        let visible = self.max_visible.unwrap_or(self.artifacts.len()).min(self.artifacts.len());
        let hidden = self.artifacts.len() - visible;
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
            strip = strip.child(
                h_flex()
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
                    .child(format!("+{hidden}")),
            );
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
