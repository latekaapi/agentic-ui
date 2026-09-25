//! The assistant's PDF pane (spec §5.5): toolbar with page stepper, zoom,
//! search, the "cited as N" pill and Insert quote; the page with the cited
//! passage highlighted.

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, font, prelude::*, px, relative, AnyElement, App, ElementId, Font, FontWeight, IntoElement, SharedString, StyledText, TextRun, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, icon_button, pill, ButtonSize, PillVariant};

/// `.tool{gap:4px;height:36px;padding:0 10px;font-size:12px}`.
const TOOL_GAP: f32 = 4.0;
const TOOL_PAD: f32 = 10.0;
/// `.tool .sel{height:26px;padding:0 8px;radius:5px;gap:6px}`.
const SEL_H: f32 = 26.0;
const SEL_PAD: f32 = 8.0;
const SEL_RADIUS: f32 = 5.0;
const SEL_GAP: f32 = 6.0;
/// `.tool .sepv{width:1px;height:16px;margin:0 4px}`.
const SEP_H: f32 = 16.0;
const SEP_MARGIN: f32 = 4.0;
/// Toolbar glyphs: 12 px; the chevron and link glyphs 10 px.
const TOOL_GLYPH: f32 = 12.0;
const SMALL_GLYPH: f32 = 10.0;
/// `.pdf{padding:20px}`; `.pg{width:340px;padding:34px 36px;font:11.5px/1.6 serif}`.
const AREA_PAD: f32 = 20.0;
const PAGE_W: f32 = 340.0;
const PAGE_PAD_Y: f32 = 34.0;
const PAGE_PAD_X: f32 = 36.0;
const PAGE_TEXT: f32 = 11.5;
const PAGE_LH: f32 = 1.6;
/// `.pg h2{font:600 12px/1.4 ui;margin-bottom:10px}`; `.pg p{margin-bottom:9px}`.
const HEADING_TEXT: f32 = 12.0;
const HEADING_LH: f32 = 1.4;
const HEADING_GAP: f32 = 10.0;
const PARAGRAPH_GAP: f32 = 9.0;
/// The footer line: 10.5 px, margin-top 22.
const FOOTER_TEXT: f32 = 10.5;
const FOOTER_TOP: f32 = 22.0;
/// `.hl{background:rgba(accent,.16)}`.
const HIGHLIGHT_ALPHA: f32 = 0.16;
/// Paper is paper in both themes: the page is white with near-black ink and
/// a muted grey footer, as the design's `.pg` rule sets it.
const PAPER_BG: u32 = 0xFFFFFF;
const PAPER_INK: u32 = 0x1A1C22;
const PAPER_MUTED: u32 = 0x7B818F;
/// The page's serif face (`Georgia, "Times New Roman", serif`).
pub const PAPER_SERIF: &str = "Georgia";

/// One run of page text.
#[derive(Debug, Clone, PartialEq)]
pub enum PdfRun {
    /// Plain.
    Text(SharedString),
    /// Bold lead (`14. Eligibility of bidders.`).
    Bold(SharedString),
    /// The cited passage.
    Highlight(SharedString),
}

/// A page: heading, paragraphs, footer.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfPage {
    /// Heading in the UI face.
    pub heading: SharedString,
    /// Paragraphs as runs.
    pub paragraphs: Vec<Vec<PdfRun>>,
    /// Footer line (`Procurement Rules 2019 · 31`).
    pub footer: SharedString,
}

/// What the toolbar asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfAction {
    /// Previous page.
    Prev,
    /// Next page.
    Next,
    /// Zoom picker.
    Zoom,
    /// Search.
    Search,
    /// Insert the highlighted quote into the draft.
    InsertQuote,
}

type ActionHandler = std::rc::Rc<dyn Fn(PdfAction, &mut Window, &mut App)>;

/// Which toolbar controls a [`PdfPane`] offers. `Default` is everything, matching the
/// toolbar before this existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfControls {
    pub pages: bool,
    pub zoom: bool,
    pub search: bool,
    pub insert_quote: bool,
}

impl Default for PdfControls {
    fn default() -> Self {
        Self { pages: true, zoom: true, search: true, insert_quote: true }
    }
}

impl PdfControls {
    /// Page stepping only: what a read-only preview can actually do.
    pub fn reading_only() -> Self {
        Self { pages: true, zoom: false, search: false, insert_quote: false }
    }
}

/// The PDF pane. Build with [`pdf_pane`].
#[derive(IntoElement)]
pub struct PdfPane {
    id: ElementId,
    page: PdfPage,
    page_no: u32,
    page_count: u32,
    zoom: SharedString,
    cited_as: Option<u8>,
    raster: Option<(gpui::ImageSource, f32, f32)>,
    on_action: Option<ActionHandler>,
    controls: PdfControls,
}

/// A pane showing `page`, page `page_no` of `page_count`.
pub fn pdf_pane(id: impl Into<ElementId>, page: PdfPage, page_no: u32, page_count: u32) -> PdfPane {
    PdfPane { id: id.into(), page, page_no, page_count, zoom: "100%".into(), cited_as: None, raster: None, on_action: None, controls: PdfControls::default() }
}

impl PdfPane {
    /// Paint a rasterised page image instead of the text page, at this exact logical size.
    pub fn raster(mut self, source: gpui::ImageSource, width: f32, height: f32) -> Self {
        self.raster = Some((source, width, height));
        self
    }

    /// The zoom label.
    pub fn zoom(mut self, zoom: impl Into<SharedString>) -> Self {
        self.zoom = zoom.into();
        self
    }

    /// Shows the `cited as N` pill.
    pub fn cited_as(mut self, n: u8) -> Self {
        self.cited_as = Some(n);
        self
    }

    /// Action handler.
    pub fn on_action(mut self, f: impl Fn(PdfAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }

    /// Show only the controls the host can honour. Defaults to all of them, so an
    /// existing caller keeps today's toolbar; a read-only preview calls this to drop
    /// the ones it cannot implement.
    pub fn controls(mut self, controls: PdfControls) -> Self {
        self.controls = controls;
        self
    }
}

fn paragraph(runs: &[PdfRun], ink: gpui::Hsla, highlight: gpui::Hsla) -> impl IntoElement {
    let serif: Font = font(PAPER_SERIF);
    let mut text = String::new();
    let mut out = Vec::new();
    for run in runs {
        let (s, f, bg) = match run {
            PdfRun::Text(s) => (s, serif.clone(), None),
            PdfRun::Bold(s) => {
                let mut f = serif.clone();
                f.weight = FontWeight::BOLD;
                (s, f, None)
            }
            PdfRun::Highlight(s) => (s, serif.clone(), Some(highlight)),
        };
        text.push_str(s);
        out.push(TextRun { len: s.len(), font: f, color: ink, background_color: bg, underline: None, strikethrough: None });
    }
    div().w_full().child(StyledText::new(text).with_runs(out))
}

impl RenderOnce for PdfPane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let emit = |action: PdfAction| {
            let h = self.on_action.clone();
            move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(h) = &h {
                    h(action, w, cx)
                }
            }
        };
        let sel = |el: gpui::Div| {
            el.h(px(SEL_H)).px(px(SEL_PAD)).gap(px(SEL_GAP)).rounded(px(SEL_RADIUS)).border_1().border_color(p.line).bg(p.surface_2).flex().items_center().text_color(p.ink)
        };
        let ghost = |name: &'static str, glyph: IconName| icon_button((id.clone(), name), glyph).ghost().size(ButtonSize::Xs).icon_size(px(TOOL_GLYPH));
        let controls = self.controls;
        let mut toolbar = h_flex()
            .w_full()
            .h(cx.aui().metrics.panel_header)
            .flex_none()
            .gap(px(TOOL_GAP))
            .px(px(TOOL_PAD))
            .border_b_1()
            .border_color(p.line)
            .ui(scale::FS_12);
        if controls.pages {
            toolbar = toolbar
                .child(ghost("prev", IconName::ArrowLeft).on_click(emit(PdfAction::Prev)))
                .child(sel(div()).child(div().mono(scale::FS_12).child(self.page_no.to_string())).child(div().text_color(p.ink_3).child(format!("/ {}", self.page_count))))
                .child(ghost("next", IconName::ArrowRight).on_click(emit(PdfAction::Next)));
        }
        if controls.pages && (controls.zoom || controls.search) {
            toolbar = toolbar.child(div().flex_none().w(px(1.0)).h(px(SEP_H)).mx(px(SEP_MARGIN)).bg(p.line));
        }
        if controls.zoom {
            toolbar = toolbar.child(
                div()
                    .id((id.clone(), "zoom"))
                    .cursor_pointer()
                    .on_click(emit(PdfAction::Zoom))
                    .child(sel(div()).child(self.zoom.clone()).child(icon(IconName::ChevronDown).size(px(SMALL_GLYPH)))),
            );
        }
        if controls.search {
            toolbar = toolbar.child(ghost("search", IconName::Search).on_click(emit(PdfAction::Search)));
        }
        toolbar = toolbar.child(div().flex_1());
        if let Some(n) = self.cited_as {
            toolbar = toolbar.child(pill(format!("cited as {n}")).variant(PillVariant::Line).leading(icon(IconName::Link).size(px(SMALL_GLYPH))));
        }
        if controls.insert_quote {
            toolbar = toolbar.child(button((id.clone(), "insert"), "Insert quote").xs().on_click(emit(PdfAction::InsertQuote)));
        }

        let paper_ink = gpui::rgb(PAPER_INK).into();
        let highlight = p.accent.alpha(HIGHLIGHT_ALPHA);
        let page: AnyElement = match self.raster.clone() {
            Some((source, width, height)) => v_flex()
                .flex_none()
                .bg(gpui::rgb(PAPER_BG))
                .shadow(p.shadow(2))
                .child(gpui::img(source).w(px(width)).h(px(height)))
                .into_any_element(),
            None => {
                let mut text = v_flex()
                    .w(px(PAGE_W))
                    .flex_none()
                    .py(px(PAGE_PAD_Y))
                    .px(px(PAGE_PAD_X))
                    .bg(gpui::rgb(PAPER_BG))
                    .shadow(p.shadow(2))
                    .font_family(PAPER_SERIF)
                    .text_px(PAGE_TEXT)
                    .line_height(relative(PAGE_LH))
                    .text_color(paper_ink)
                    .child(div().mb(px(HEADING_GAP)).ui(HEADING_TEXT).line_height(relative(HEADING_LH)).semibold().text_color(paper_ink).child(self.page.heading.clone()));
                let count = self.page.paragraphs.len();
                for (i, para) in self.page.paragraphs.iter().enumerate() {
                    text = text.child(div().w_full().when(i + 1 < count, |d| d.mb(px(PARAGRAPH_GAP))).child(paragraph(para, paper_ink, highlight)));
                }
                text.child(div().mt(px(FOOTER_TOP)).text_px(FOOTER_TEXT).text_color(gpui::rgb(PAPER_MUTED)).child(self.page.footer.clone())).into_any_element()
            }
        };

        v_flex()
            .id(id)
            .size_full()
            .bg(p.surface_1)
            .child(toolbar)
            .child(div().flex_1().min_h(px(0.0)).w_full().bg(p.surface_2).p(px(AREA_PAD)).flex().justify_center().items_start().overflow_hidden().child(page))
    }
}
