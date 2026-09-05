//! Card 51 · the browser pane and its annotator (spec §5.2).
//!
//! The page itself is a native webview, so this module draws everything
//! around and over it: the nav row ([`browser_nav`]), the annotate-mode
//! overlays ([`element_outline`], [`annotation_pin`], [`note_popover`]), the
//! annotations side panel ([`annotations_panel`]) and the agent-action pill
//! ([`agent_action_pill`]) shown while an agent drives the page.
//!
//! The browser is an annotator, not a design mode: hovering outlines an
//! element, clicking pins a numbered note, the side panel collects them and
//! one action sends the batch — with metadata and a screenshot — to the agent.

use std::rc::Rc;

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_motion::{shimmer_text, tween, Tween};
use aui_tokens::{light, scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, linear_color_stop, linear_gradient, prelude::*, px, relative, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, icon_button, kbd, ButtonSize, ButtonVariant};
use crate::util::{interaction_flags, TrackInteraction};

/// `.br{grid-template-rows:34px 38px 1fr}`: the nav band.
const NAV_HEIGHT: f32 = 38.0;
/// `.nav{gap:4px;padding:0 8px}`.
const NAV_GAP: f32 = 4.0;
const NAV_PAD_X: f32 = 8.0;
/// `.url{height:28px;padding:0 10px;gap:8px}` with an 11 px shield.
const URL_HEIGHT: f32 = 28.0;
const URL_PAD_X: f32 = 10.0;
const URL_GAP: f32 = 8.0;
const URL_SHIELD: f32 = 11.0;
/// `.mode{height:28px;padding:0 10px;gap:6px;font-size:12px}` with a 12 px glyph.
const MODE_HEIGHT: f32 = 28.0;
const MODE_PAD_X: f32 = 10.0;
const MODE_GAP: f32 = 6.0;
const MODE_GLYPH: f32 = 12.0;
/// The `esc` cap inside the filled toggle: `border-color:rgba(255,255,255,.3)`
/// over the ink ground, text in the toggle's own colour.
const MODE_KBD_BORDER_ALPHA: f32 = 0.3;
/// `.kbd{height:18px;padding:0 5px}` — the toggle draws its own, untinted.
const KBD_HEIGHT: f32 = 18.0;
const KBD_PAD_X: f32 = 5.0;
/// The loading hairline at the bottom of the nav row (spec §5.2): 2 px accent.
const LOADING_HEIGHT: f32 = 2.0;

/// `.hover{border-radius:4px}` with the outline at `.6` and a `.06` fill.
const OUTLINE_RADIUS: f32 = 4.0;
const OUTLINE_ALPHA: f32 = 0.6;
const OUTLINE_FILL_ALPHA: f32 = 0.06;
/// `.hover .lbl{left:-1px;top:-20px;padding:2px 6px;font:500 10px mono}`.
const OUTLINE_LABEL_LEFT: f32 = -1.0;
const OUTLINE_LABEL_TOP: f32 = -20.0;
const OUTLINE_LABEL_PAD_Y: f32 = 2.0;
const OUTLINE_LABEL_PAD_X: f32 = 6.0;
const OUTLINE_LABEL_SIZE: f32 = 10.0;
/// `.sel{border:1.5px solid var(--accent);border-radius:6px}`.
const SELECTED_BORDER: f32 = 1.5;
const SELECTED_RADIUS: f32 = 6.0;

/// `.pin{width:20px;height:20px;border-radius:50% 50% 50% 4px;font:600 11px}`.
const PIN_SIZE: f32 = 20.0;
const PIN_CORNER: f32 = 4.0;
const PIN_TEXT: f32 = 11.0;
/// `.pin{box-shadow:0 2px 6px rgba(0,0,0,.25)}` — the pin carries its own
/// drop shadow, smaller than elevation 2.
const PIN_SHADOW_Y: f32 = 2.0;
const PIN_SHADOW_BLUR: f32 = 6.0;
const PIN_SHADOW_ALPHA: f32 = 0.25;
/// `.pin.on{outline:2px solid #fff;outline-offset:1px}` — gpui has no
/// outline, so the ring is a border on a wrapper inset by offset + width.
const PIN_RING: f32 = 2.0;
const PIN_RING_OFFSET: f32 = 1.0;

/// `.pop{width:236px;padding:10px;font-size:12px}`.
const POPOVER_WIDTH: f32 = 236.0;
const POPOVER_PAD: f32 = 10.0;
/// `.pop .el{font:11px mono;margin-bottom:6px}`.
const POPOVER_PATH_SIZE: f32 = 11.0;
const POPOVER_PATH_GAP: f32 = 6.0;
/// `.pop textarea{padding:6px 8px;font:12px/1.4 ui;rows=2}`.
const DRAFT_PAD_Y: f32 = 6.0;
const DRAFT_PAD_X: f32 = 8.0;
const DRAFT_LINE_HEIGHT: f32 = 1.4;
const DRAFT_ROWS: f32 = 2.0;
/// `.pop .acts{gap:6px;margin-top:8px}`.
const POPOVER_ACTIONS_GAP: f32 = 6.0;
const POPOVER_ACTIONS_TOP: f32 = 8.0;
/// `animation:in var(--d-enter) var(--e-out)` — `from{opacity:0;transform:translateY(4px)}`.
const POPOVER_RISE: f32 = 4.0;

/// `.br{grid-template-columns:1fr 272px}`.
const PANEL_WIDTH: f32 = 272.0;
/// `.side .hd{height:36px;padding:0 12px;gap:8px;font-size:12.5px}`.
const PANEL_HEAD_HEIGHT: f32 = 36.0;
const PANEL_PAD_X: f32 = 12.0;
const PANEL_HEAD_GAP: f32 = 8.0;
const PANEL_HEAD_SIZE: f32 = 12.5;
/// `.side .hd .btn.icon.xs svg{width:12px}`.
const PANEL_HEAD_GLYPH: f32 = 12.0;
/// `.an{gap:10px;padding:10px 12px;font-size:12px}`.
const ROW_GAP: f32 = 10.0;
const ROW_PAD_Y: f32 = 10.0;
/// `.an .n{width:18px;height:18px;font:600 10px}`.
const DISC_SIZE: f32 = 18.0;
const DISC_TEXT: f32 = 10.0;
/// `.an .el{font:10.5px mono}` and `.an p{margin-top:2px;line-height:1.4}`.
/// The `font` shorthand drops the inherited line height, so the path line is
/// a `normal` line box: the design's tight ratio.
const ROW_PATH_SIZE: f32 = 10.5;
const ROW_NOTE_TOP: f32 = 2.0;
const ROW_NOTE_SIZE: f32 = 12.0;
const ROW_NOTE_LINE_HEIGHT: f32 = 1.4;
/// `.side .shot{margin:10px 12px 0;height:64px;border-radius:6px}` with 12 px pins.
const SHOT_MARGIN: f32 = 10.0;
const SHOT_HEIGHT: f32 = 64.0;
const SHOT_RADIUS: f32 = 6.0;
const SHOT_PIN: f32 = 12.0;
const SHOT_PIN_CORNER: f32 = 2.0;
/// `linear-gradient(135deg,#fff,#eef1f5)`: a light page, whatever the theme.
const SHOT_GRADIENT_ANGLE: f32 = 135.0;
/// `.side .meta{padding:10px 12px;font-size:11px;line-height:1.6}`.
const META_PAD_Y: f32 = 10.0;
const META_SIZE: f32 = 11.0;
const META_LINE_HEIGHT: f32 = 1.6;
/// `.actions{gap:8px;padding:10px 12px}`.
const ACTIONS_GAP: f32 = 8.0;
const ACTIONS_PAD_Y: f32 = 10.0;
/// The provider mark inside the send button: `width:12px;margin:0 2px`.
const SEND_MARK: f32 = 12.0;
const SEND_MARK_MARGIN: f32 = 2.0;

/// `.agent{gap:8px;padding:5px 12px 5px 6px;font-size:12px}` with a 14 px mark.
const PILL_GAP: f32 = 8.0;
const PILL_PAD_Y: f32 = 5.0;
const PILL_PAD_LEFT: f32 = 6.0;
const PILL_PAD_RIGHT: f32 = 12.0;
const PILL_MARK: f32 = 14.0;

/// What the person is sent with each note (the design's own wording).
const METADATA_NOTE: &str = "Sent with each note: selector, bounding box, outer HTML, computed styles, source file when a dev server is mapped, page URL, and a screenshot with the pins.";

/// A control in the nav row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserAction {
    /// Back one entry in history.
    Back,
    /// Forward one entry.
    Forward,
    /// Reload the page.
    Reload,
    /// Focus the URL field (`⌘L`).
    FocusUrl,
    /// Turn annotate mode on or off (`esc` leaves it).
    ToggleAnnotate,
    /// Capture the page.
    Screenshot,
    /// Show the console.
    Console,
}

type BrowserHandler = Rc<dyn Fn(BrowserAction, &mut Window, &mut App)>;

/// The 38 px nav row. Build with [`browser_nav`].
#[derive(IntoElement)]
pub struct BrowserNav {
    id: ElementId,
    url: SharedString,
    secure: bool,
    loading: f32,
    annotating: bool,
    can_go_back: bool,
    can_go_forward: bool,
    on_action: Option<BrowserHandler>,
}

/// The nav row for `url`: history controls, the URL field, the annotate
/// toggle, screenshot and console.
pub fn browser_nav(id: impl Into<ElementId>, url: impl Into<SharedString>) -> BrowserNav {
    BrowserNav {
        id: id.into(),
        url: url.into(),
        secure: true,
        loading: 0.0,
        annotating: false,
        can_go_back: true,
        can_go_forward: false,
        on_action: None,
    }
}

impl BrowserNav {
    /// Whether the URL field shows the success shield (default true).
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// Load progress 0–1; anything above 0 draws the accent hairline.
    pub fn loading(mut self, progress: f32) -> Self {
        self.loading = progress.clamp(0.0, 1.0);
        self
    }

    /// Annotate mode: the toggle is ink-filled and shows the `esc` cap.
    pub fn annotating(mut self, annotating: bool) -> Self {
        self.annotating = annotating;
        self
    }

    /// Enables the back control (default true).
    pub fn can_go_back(mut self, can: bool) -> Self {
        self.can_go_back = can;
        self
    }

    /// Enables the forward control (default false: disabled at 45 %).
    pub fn can_go_forward(mut self, can: bool) -> Self {
        self.can_go_forward = can;
        self
    }

    /// Called with the [`BrowserAction`] a control stands for.
    pub fn on_action(mut self, f: impl Fn(BrowserAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for BrowserNav {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let handler = self.on_action.clone();

        let nav_button = |key: &'static str, glyph: IconName, action: BrowserAction, disabled: bool| {
            let mut b = icon_button((id.clone(), key), glyph).ghost().size(ButtonSize::Sm).disabled(disabled);
            if let Some(handler) = handler.clone() {
                b = b.on_click(move |_, window, cx| handler(action, window, cx));
            }
            b
        };

        // `.url`: surface-2, mono 12, a success shield and the ⌘L cap.
        let url_id: ElementId = (id.clone(), "url").into();
        let mut url = h_flex()
            .id(url_id)
            .flex_1()
            .min_w(px(0.0))
            .h(px(URL_HEIGHT))
            .px(px(URL_PAD_X))
            .gap(px(URL_GAP))
            .rounded(px(scale::R_SM))
            .bg(p.surface_2)
            .mono(scale::FS_12)
            .text_color(p.ink_2)
            .cursor_pointer()
            .when(self.secure, |d| d.child(icon(IconName::Shield).size(px(URL_SHIELD)).color(p.success)))
            .child(div().min_w(px(0.0)).truncate().child(self.url.clone()))
            .child(div().flex_1())
            .child(kbd("⌘L"));
        if let Some(handler) = handler.clone() {
            url = url.on_click(move |_, window, cx| handler(BrowserAction::FocusUrl, window, cx));
        }

        h_flex()
            .relative()
            .w_full()
            .h(px(NAV_HEIGHT))
            .flex_none()
            .px(px(NAV_PAD_X))
            .gap(px(NAV_GAP))
            .border_b_1()
            .border_color(p.line)
            .child(nav_button("back", IconName::ArrowLeft, BrowserAction::Back, !self.can_go_back))
            .child(nav_button("forward", IconName::ArrowRight, BrowserAction::Forward, !self.can_go_forward))
            .child(nav_button("reload", IconName::Refresh, BrowserAction::Reload, false))
            .child(url)
            .child(annotate_toggle((id.clone(), "annotate"), self.annotating, handler.clone(), window, cx))
            .child(nav_button("screenshot", IconName::Camera, BrowserAction::Screenshot, false))
            .child(nav_button("console", IconName::Terminal, BrowserAction::Console, false))
            .when(self.loading > 0.0, |d| {
                d.child(div().absolute().left_0().bottom_0().w(relative(self.loading)).h(px(LOADING_HEIGHT)).bg(p.accent))
            })
    }
}

/// `.mode`: ink-filled while annotating, the secondary control look when off.
fn annotate_toggle(id: impl Into<ElementId>, on: bool, handler: Option<BrowserHandler>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let (state, flags) = interaction_flags(id.clone(), window, cx);
    let (ground, edge, text) = match (on, flags.hovered) {
        (true, false) => (p.ink, p.ink, p.bg),
        (true, true) => (p.ink_2, p.ink_2, p.bg),
        (false, false) => (p.surface_2, p.line_strong, p.ink),
        (false, true) => (p.surface_3, p.ink_4, p.ink),
    };
    let ground = tween((id.clone(), "bg"), ground, Tween::FAST, window, cx);
    let edge = tween((id.clone(), "border"), edge, Tween::FAST, window, cx);
    let text = tween((id.clone(), "text"), text, Tween::FAST, window, cx);

    let mut toggle = h_flex()
        .id(id)
        .flex_none()
        .h(px(MODE_HEIGHT))
        .px(px(MODE_PAD_X))
        .gap(px(MODE_GAP))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(edge)
        .bg(ground)
        .text_color(text)
        .ui(scale::FS_12)
        .medium()
        .whitespace_nowrap()
        .cursor_pointer()
        .track_interaction(&state)
        .child(icon(IconName::Edit).size(px(MODE_GLYPH)).color(text))
        .child("Annotate")
        .when(on, |d| {
            d.child(
                h_flex()
                    .flex_none()
                    .h(px(KBD_HEIGHT))
                    .px(px(KBD_PAD_X))
                    .rounded(px(scale::R_XS))
                    .border_1()
                    .border_color(gpui::white().alpha(MODE_KBD_BORDER_ALPHA))
                    .font_family(scale::FONT_MONO)
                    .text_px(scale::FS_11)
                    .line_height(relative(1.0))
                    .child("esc"),
            )
        });
    if let Some(handler) = handler {
        toggle = toggle.on_click(move |_, window, cx| handler(BrowserAction::ToggleAnnotate, window, cx));
    }
    toggle
}

/// The hovered / selected element overlay. Build with [`element_outline`].
#[derive(IntoElement)]
pub struct ElementOutline {
    label: SharedString,
    width: Option<gpui::Pixels>,
    height: Option<gpui::Pixels>,
    selected: bool,
    children: Vec<AnyElement>,
}

/// An outline over a page element, labelled `label` (`p · 392 × 34`). The
/// caller positions it over the element's bounds, either by giving it a
/// [`ElementOutline::size`] or by handing it the element as children.
pub fn element_outline(label: impl Into<SharedString>) -> ElementOutline {
    ElementOutline { label: label.into(), width: None, height: None, selected: false, children: Vec::new() }
}

impl ElementOutline {
    /// The element's box.
    pub fn size(mut self, width: impl Into<gpui::Pixels>, height: impl Into<gpui::Pixels>) -> Self {
        self.width = Some(width.into());
        self.height = Some(height.into());
        self
    }

    /// The clicked element: a 1.5 px outline, no fill and no label.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl ParentElement for ElementOutline {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ElementOutline {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The outline is drawn over web content, which is a page of its own
        // and always light; the design uses the light palette's accent for it.
        let accent = light().accent;
        let mut outline = div()
            .relative()
            .when_some(self.width, |d, w| d.w(w))
            .when_some(self.height, |d, h| d.h(h))
            .children(self.children);
        if self.selected {
            outline = outline.border(px(SELECTED_BORDER)).border_color(cx.aui().colors.accent).rounded(px(SELECTED_RADIUS));
        } else {
            outline = outline
                .border_1()
                .border_color(accent.alpha(OUTLINE_ALPHA))
                .rounded(px(OUTLINE_RADIUS))
                .bg(accent.alpha(OUTLINE_FILL_ALPHA))
                .child(
                    div()
                        .absolute()
                        .left(px(OUTLINE_LABEL_LEFT))
                        .top(px(OUTLINE_LABEL_TOP))
                        .py(px(OUTLINE_LABEL_PAD_Y))
                        .px(px(OUTLINE_LABEL_PAD_X))
                        .rounded_t(px(OUTLINE_RADIUS))
                        .bg(accent)
                        .text_color(gpui::white())
                        .font_family(scale::FONT_MONO)
                        .text_px(OUTLINE_LABEL_SIZE)
                        .line_height(relative(1.0))
                        .medium()
                        .whitespace_nowrap()
                        .child(self.label),
                );
        }
        outline
    }
}

/// A numbered pin. Build with [`annotation_pin`].
#[derive(IntoElement)]
pub struct AnnotationPin {
    index: usize,
    selected: bool,
}

/// The 20 px teardrop pin carrying `index`, dropped at an element's top-right.
pub fn annotation_pin(index: usize) -> AnnotationPin {
    AnnotationPin { index, selected: false }
}

impl AnnotationPin {
    /// The pin of the selected annotation: a white ring around the teardrop.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for AnnotationPin {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        // `border-radius:50% 50% 50% 4px`: round but for the bottom-left
        // corner, which points at the element (gpui has no transform, so the
        // teardrop is a rounded square with one square corner).
        let round = PIN_SIZE / 2.0;
        let pin = div()
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .size(px(PIN_SIZE))
            .rounded_tl(px(round))
            .rounded_tr(px(round))
            .rounded_br(px(round))
            .rounded_bl(px(PIN_CORNER))
            .bg(p.accent)
            .text_color(gpui::white())
            .font_family(scale::FONT_UI)
            .text_px(PIN_TEXT)
            .line_height(relative(1.0))
            .semibold()
            .shadow(vec![gpui::BoxShadow {
                color: gpui::black().alpha(PIN_SHADOW_ALPHA),
                offset: gpui::point(px(0.0), px(PIN_SHADOW_Y)),
                blur_radius: px(PIN_SHADOW_BLUR),
                spread_radius: px(0.0),
                inset: false,
            }])
            .child(SharedString::from(self.index.to_string()));
        if !self.selected {
            return pin.into_any_element();
        }
        // `outline:2px solid #fff;outline-offset:1px` as a ring on a wrapper.
        let inset = PIN_RING + PIN_RING_OFFSET;
        div()
            .flex_none()
            .p(px(PIN_RING_OFFSET))
            .border(px(PIN_RING))
            .border_color(gpui::white())
            .rounded_tl(px(round + inset))
            .rounded_tr(px(round + inset))
            .rounded_br(px(round + inset))
            .rounded_bl(px(PIN_CORNER + inset))
            .child(pin)
            .into_any_element()
    }
}

/// What the note popover reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteAction {
    /// Discard the pending note.
    Cancel,
    /// Keep it.
    Save,
}

type NoteHandler = Rc<dyn Fn(NoteAction, &mut Window, &mut App)>;

/// The note popover. Build with [`note_popover`].
#[derive(IntoElement)]
pub struct NotePopover {
    id: ElementId,
    path: SharedString,
    draft: SharedString,
    visible: bool,
    on_action: Option<NoteHandler>,
}

/// The 236 px popover that opens on a pinned element: its path, the note
/// being typed, Cancel and Save note.
pub fn note_popover(id: impl Into<ElementId>, path: impl Into<SharedString>, draft: impl Into<SharedString>) -> NotePopover {
    NotePopover { id: id.into(), path: path.into(), draft: draft.into(), visible: true, on_action: None }
}

impl NotePopover {
    /// Whether the popover is open; it fades and rises 4 px on the enter timing.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Called with [`NoteAction`] when Cancel or Save note is clicked.
    pub fn on_action(mut self, f: impl Fn(NoteAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for NotePopover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // The enter is a fade and a 4 px rise on the enter timing. It is a
        // tween, not a presence: the first render adopts the state, so a
        // popover built already open draws at rest.
        let shown = tween((id.clone(), "enter"), if self.visible { 1.0f32 } else { 0.0 }, Tween::ENTER, window, cx);
        let handler = self.on_action.clone();
        let action = |key: &'static str, label: &'static str, variant: ButtonVariant, intent: NoteAction, handler: &Option<NoteHandler>| {
            let mut b = button((id.clone(), key), label).size(ButtonSize::Xs).variant(variant);
            if let Some(handler) = handler.clone() {
                b = b.on_click(move |_, window, cx| handler(intent, window, cx));
            }
            b
        };

        v_flex()
            .relative()
            .top(px(POPOVER_RISE * (1.0 - shown)))
            .opacity(shown)
            .w(px(POPOVER_WIDTH))
            .p(px(POPOVER_PAD))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(3))
            .text_color(p.ink)
            .ui(scale::FS_12)
            .child(
                div()
                    .w_full()
                    .min_w(px(0.0))
                    .truncate()
                    .mb(px(POPOVER_PATH_GAP))
                    .font_family(scale::FONT_MONO)
                    .text_px(POPOVER_PATH_SIZE)
                    .line_height(relative(scale::LH_TIGHT))
                    .text_color(p.ink_3)
                    .child(self.path),
            )
            // The textarea stand-in: the draft text on surface-2, two rows tall.
            .child(
                div()
                    .w_full()
                    .min_h(px(DRAFT_ROWS * DRAFT_LINE_HEIGHT * scale::FS_12 + 2.0 * DRAFT_PAD_Y))
                    .py(px(DRAFT_PAD_Y))
                    .px(px(DRAFT_PAD_X))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.surface_2)
                    .ui(scale::FS_12)
                    .line_height(relative(DRAFT_LINE_HEIGHT))
                    .child(self.draft),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap(px(POPOVER_ACTIONS_GAP))
                    .mt(px(POPOVER_ACTIONS_TOP))
                    .child(action("cancel", "Cancel", ButtonVariant::Ghost, NoteAction::Cancel, &handler))
                    .child(action("save", "Save note", ButtonVariant::Primary, NoteAction::Save, &handler)),
            )
    }
}

/// One annotation in the side panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    /// The number carried by its pin.
    pub index: usize,
    /// The element path (`div.card.starter`).
    pub selector: SharedString,
    /// The note text, or what is being typed.
    pub note: SharedString,
    /// Still being written: dashed disc, muted note.
    pub pending: bool,
}

impl Annotation {
    /// A saved annotation.
    pub fn new(index: usize, selector: impl Into<SharedString>, note: impl Into<SharedString>) -> Self {
        Self { index, selector: selector.into(), note: note.into(), pending: false }
    }

    /// Marks it as still being written.
    pub fn pending(mut self, pending: bool) -> Self {
        self.pending = pending;
        self
    }
}

/// What the annotations panel reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotatorAction {
    /// Close the panel (and leave annotate mode).
    Close,
    /// Drop every annotation.
    Clear,
    /// Send the batch to the agent.
    Send,
    /// Select the annotation at this position in the list.
    Select(usize),
}

type AnnotatorHandler = Rc<dyn Fn(AnnotatorAction, &mut Window, &mut App)>;

/// The annotations side panel. Build with [`annotations_panel`].
#[derive(IntoElement)]
pub struct AnnotationsPanel {
    id: ElementId,
    annotations: Vec<Annotation>,
    selected: Option<usize>,
    screenshot: Option<Vec<(f32, f32)>>,
    provider: Provider,
    agent: SharedString,
    on_action: Option<AnnotatorHandler>,
}

/// The 272 px panel collecting `annotations`, with the screenshot preview,
/// the metadata note and the send action.
pub fn annotations_panel(id: impl Into<ElementId>, annotations: Vec<Annotation>) -> AnnotationsPanel {
    AnnotationsPanel {
        id: id.into(),
        annotations,
        selected: None,
        screenshot: None,
        provider: Provider::Claude,
        agent: SharedString::from("Claude Code"),
        on_action: None,
    }
}

impl AnnotationsPanel {
    /// The screenshot preview tile: the pins' positions as fractions of the
    /// tile (`None` hides the tile).
    pub fn screenshot(mut self, pins: Option<Vec<(f32, f32)>>) -> Self {
        self.screenshot = pins;
        self
    }

    /// The highlighted row.
    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// The agent the batch is sent to (mark and name on the primary button).
    pub fn agent(mut self, provider: Provider, name: impl Into<SharedString>) -> Self {
        self.provider = provider;
        self.agent = name.into();
        self
    }

    /// Called with the [`AnnotatorAction`] a control stands for.
    pub fn on_action(mut self, f: impl Fn(AnnotatorAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for AnnotationsPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let handler = self.on_action.clone();

        let mut close = icon_button((id.clone(), "close"), IconName::X).ghost().size(ButtonSize::Xs).icon_size(px(PANEL_HEAD_GLYPH));
        if let Some(handler) = handler.clone() {
            close = close.on_click(move |_, window, cx| handler(AnnotatorAction::Close, window, cx));
        }
        let head = h_flex()
            .w_full()
            .flex_none()
            .h(px(PANEL_HEAD_HEIGHT))
            .px(px(PANEL_PAD_X))
            .gap(px(PANEL_HEAD_GAP))
            .border_b_1()
            .border_color(p.line)
            .ui(PANEL_HEAD_SIZE)
            .semibold()
            .text_color(p.ink)
            .child(icon(IconName::Edit).color(p.ink))
            .child("Annotations")
            .child(div().font_weight(gpui::FontWeight::NORMAL).text_color(p.ink_3).child(format!("· {}", self.annotations.len())))
            .child(div().flex_1())
            .child(close);

        let mut panel = v_flex()
            .flex_none()
            .w(px(PANEL_WIDTH))
            .h_full()
            .min_h(px(0.0))
            .border_l_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .child(head);

        for (position, annotation) in self.annotations.iter().enumerate() {
            panel = panel.child(annotation_row((id.clone(), SharedString::from(format!("row-{position}"))), annotation, self.selected == Some(position), handler.clone(), window, cx));
        }

        if let Some(pins) = self.screenshot {
            panel = panel.child(screenshot_tile(pins, p));
        }

        panel = panel.child(
            div()
                .w_full()
                .py(px(META_PAD_Y))
                .px(px(PANEL_PAD_X))
                .border_b_1()
                .border_color(p.line)
                .ui(META_SIZE)
                .line_height(relative(META_LINE_HEIGHT))
                .text_color(p.ink_3)
                .child(METADATA_NOTE),
        );

        let action = |key: &'static str, label: SharedString, variant: ButtonVariant, intent: AnnotatorAction, handler: &Option<AnnotatorHandler>| {
            let mut b = button((id.clone(), key), label).size(ButtonSize::Sm).variant(variant);
            if let Some(handler) = handler.clone() {
                b = b.on_click(move |_, window, cx| handler(intent, window, cx));
            }
            b
        };
        // "Send to ‹mark› Claude Code": the mark sits between the two words,
        // so it rides along as the button's trailing content.
        let send = action("send", SharedString::from("Send to"), ButtonVariant::Primary, AnnotatorAction::Send, &handler).trailing(
            h_flex()
                .gap(px(scale::SP_3 - 2.0))
                .child(div().mx(px(SEND_MARK_MARGIN)).child(provider_mark(self.provider).size(px(SEND_MARK))))
                .child(self.agent.clone()),
        );

        panel.child(
            h_flex()
                .w_full()
                .mt_auto()
                .flex_none()
                .gap(px(ACTIONS_GAP))
                .py(px(ACTIONS_PAD_Y))
                .px(px(PANEL_PAD_X))
                .border_t_1()
                .border_color(p.line)
                .bg(p.surface_2)
                .child(div().flex_1())
                .child(action("clear", SharedString::from("Clear"), ButtonVariant::Ghost, AnnotatorAction::Clear, &handler))
                .child(send),
        )
    }
}

/// `.an`: the numbered disc, the element path and the note.
fn annotation_row(
    id: impl Into<ElementId>,
    annotation: &Annotation,
    selected: bool,
    handler: Option<AnnotatorHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let (state, flags) = interaction_flags(id.clone(), window, cx);
    let ground = tween(
        (id.clone(), "bg"),
        if selected || flags.hovered { p.surface_2 } else { gpui::transparent_black() },
        Tween::FAST,
        window,
        cx,
    );

    let mut disc = div()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .size(px(DISC_SIZE))
        .rounded_full()
        .font_family(scale::FONT_UI)
        .text_px(DISC_TEXT)
        .line_height(relative(1.0))
        .semibold()
        .child(SharedString::from(annotation.index.to_string()));
    disc = if annotation.pending {
        disc.bg(p.surface_3).text_color(p.ink_3).border_1().border_dashed().border_color(p.ink_4)
    } else {
        disc.bg(p.accent).text_color(gpui::white())
    };

    let mut row = h_flex()
        .id(id)
        .w_full()
        .items_start()
        .gap(px(ROW_GAP))
        .py(px(ROW_PAD_Y))
        .px(px(PANEL_PAD_X))
        .border_b_1()
        .border_color(p.line)
        .bg(ground)
        .cursor_pointer()
        .track_interaction(&state)
        .child(disc)
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .child(div().min_w(px(0.0)).truncate().text_role(TextRole::Tag).text_px(ROW_PATH_SIZE).line_height(relative(scale::LH_TIGHT)).text_color(p.ink_3).child(annotation.selector.clone()))
                .child(
                    div()
                        .mt(px(ROW_NOTE_TOP))
                        .ui(ROW_NOTE_SIZE)
                        .line_height(relative(ROW_NOTE_LINE_HEIGHT))
                        .text_color(if annotation.pending { p.ink_3 } else { p.ink })
                        .child(annotation.note.clone()),
                ),
        );
    if let Some(handler) = handler {
        let position = annotation.index.saturating_sub(1);
        row = row.on_click(move |_, window, cx| handler(AnnotatorAction::Select(position), window, cx));
    }
    row
}

/// `.side .shot`: the captured page with its pins, over a light gradient.
fn screenshot_tile(pins: Vec<(f32, f32)>, p: Palette) -> impl IntoElement {
    // The tile shows a web page, which is light whatever the app's theme is.
    let page = light();
    let mut tile = div()
        .relative()
        .flex_none()
        .mx(px(PANEL_PAD_X))
        .mt(px(SHOT_MARGIN))
        .h(px(SHOT_HEIGHT))
        .overflow_hidden()
        .rounded(px(SHOT_RADIUS))
        .border_1()
        .border_color(p.line)
        .bg(linear_gradient(
            SHOT_GRADIENT_ANGLE,
            linear_color_stop(page.surface_1, 0.0),
            linear_color_stop(page.surface_2, 1.0),
        ));
    let round = SHOT_PIN / 2.0;
    for (x, y) in pins {
        tile = tile.child(
            div()
                .absolute()
                .left(relative(x))
                .top(relative(y))
                .size(px(SHOT_PIN))
                .rounded_tl(px(round))
                .rounded_tr(px(round))
                .rounded_br(px(round))
                .rounded_bl(px(SHOT_PIN_CORNER))
                .bg(p.accent),
        );
    }
    tile
}

/// The agent-action pill. Build with [`agent_action_pill`].
#[derive(IntoElement)]
pub struct AgentActionPill {
    id: ElementId,
    text: SharedString,
    detail: Option<SharedString>,
    provider: Provider,
}

/// The pill shown over the page while an agent drives it; `text` shimmers.
pub fn agent_action_pill(id: impl Into<ElementId>, text: impl Into<SharedString>) -> AgentActionPill {
    AgentActionPill { id: id.into(), text: text.into(), detail: None, provider: Provider::Claude }
}

impl AgentActionPill {
    /// The quiet trailing clause (`· verifying signup flow`).
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Which agent is driving (default Claude).
    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = provider;
        self
    }
}

impl RenderOnce for AgentActionPill {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        h_flex()
            .flex_none()
            .gap(px(PILL_GAP))
            .py(px(PILL_PAD_Y))
            .pl(px(PILL_PAD_LEFT))
            .pr(px(PILL_PAD_RIGHT))
            .rounded(px(scale::R_FULL))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(2))
            .ui(scale::FS_12)
            .text_color(p.ink)
            .whitespace_nowrap()
            .child(provider_mark(self.provider).size(px(PILL_MARK)))
            .child(div().medium().child(shimmer_text((self.id, "shimmer"), self.text, cx)))
            .when_some(self.detail, |d, detail| d.child(div().text_color(p.ink_3).child(detail)))
    }
}
