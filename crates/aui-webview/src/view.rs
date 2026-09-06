//! The gpui side: [`WebviewState`] holds a [`WebBackend`] and what the pane
//! knows about it, and [`webview_pane`] draws the nav row, the page and the
//! annotations panel over it.
//!
//! Nothing here reaches into a browser directly. The pane sends
//! [`aui::workbench::BrowserAction`]s and pointer positions down into the
//! backend and re-renders from the [`WebEvent`]s a poll timer drains, so the
//! same element tree runs over the scripted mock and over a real WKWebView.

use std::rc::Rc;
use std::time::Duration;

use aui::workbench::{annotations_panel, browser_nav, element_outline, note_popover, Annotation, AnnotatorAction, BrowserAction, NoteAction};
use aui_tokens::ActiveAui;
use gpui::{canvas, div, prelude::*, px, App, Context, ElementId, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, SharedString, Size, Task, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::backend::{ElementInfo, WebBackend, WebEvent};
use crate::page::{page_body, pin_at};

/// How often the pane drains its backend. Fast enough that a title or a pin
/// lands within a frame or two, slow enough to cost nothing while idle.
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Where the note popover opens relative to the pinned element: just clear of
/// its right edge, dropped a little so the pin stays visible.
const NOTE_GAP: f32 = 14.0;
const NOTE_DROP: f32 = 14.0;
/// The note a saved pin keeps. The pane has no text field over the page yet,
/// so Save keeps this stand-in rather than an empty row.
const SAVED_NOTE: &str = "Note for the agent.";

/// What the pane asks the app to do.
#[derive(Debug, Clone, PartialEq)]
pub enum WebviewIntent {
    /// `⌘L`: put the keyboard in the URL field.
    FocusUrl,
    /// Show the page's console.
    Console,
    /// The person asked for a screenshot.
    Screenshot,
    /// Annotate mode was turned on or off.
    Annotate(bool),
    /// Hand the collected annotations — with the last screenshot, when the
    /// backend can take one — to the agent.
    SendAnnotations {
        /// The pins, in the order they were dropped.
        annotations: Vec<Annotation>,
        /// The annotated page, when the backend produced one.
        screenshot: Option<Vec<u8>>,
        /// The page they were taken on.
        url: String,
    },
}

/// Everything the pane knows about its page.
///
/// Held as an `Entity` so the poll timer can wake the view: the timer drains
/// [`WebBackend::poll_events`] and calls `notify` only when something changed.
pub struct WebviewState {
    backend: Box<dyn WebBackend>,
    url: SharedString,
    title: SharedString,
    loading: bool,
    annotate: bool,
    /// The pins, in the order they were dropped.
    annotations: Vec<Annotation>,
    /// Each pin's element, parallel to `annotations`. `None` when the backend
    /// reported an annotation without metadata.
    elements: Vec<Option<ElementInfo>>,
    /// The element under the pointer while annotate mode is on.
    hovered: Option<ElementInfo>,
    /// The highlighted pin, as a position in `annotations`.
    selected: Option<usize>,
    /// The pin whose note popover is open.
    note: Option<usize>,
    /// The last screenshot the backend produced.
    screenshot: Option<Vec<u8>>,
    /// The page area's box, recorded during prepaint so pointer positions can
    /// be turned into page coordinates.
    page_origin: Point<Pixels>,
    page_size: Size<Pixels>,
    /// The poll timer; it stops when the state is dropped.
    _poll: Task<()>,
}

impl WebviewState {
    /// Wraps `backend` and starts polling it.
    pub fn new(backend: Box<dyn WebBackend>, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            if this.update(cx, |this, cx| this.drain(cx)).is_err() {
                return;
            }
        });
        let mut state = Self {
            backend,
            url: SharedString::default(),
            title: SharedString::default(),
            loading: false,
            annotate: false,
            annotations: Vec::new(),
            elements: Vec::new(),
            hovered: None,
            selected: None,
            note: None,
            screenshot: None,
            page_origin: gpui::point(px(0.0), px(0.0)),
            page_size: gpui::size(px(0.0), px(0.0)),
            _poll: poll,
        };
        // A backend usually has its first page to announce before the poll
        // timer has run, so the pane never draws an empty URL field.
        state.apply_pending();
        state
    }

    /// The URL the nav row shows.
    pub fn url(&self) -> SharedString {
        self.url.clone()
    }

    /// The document title.
    pub fn title(&self) -> SharedString {
        self.title.clone()
    }

    /// Whether annotate mode is on.
    pub fn annotating(&self) -> bool {
        self.annotate
    }

    /// The pins collected so far.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Turns annotate mode on or off, clearing the hover and any open note.
    pub fn set_annotate(&mut self, on: bool) {
        self.annotate = on;
        self.backend.set_annotate(on);
        if !on {
            self.hovered = None;
            self.note = None;
        }
    }

    /// Loads `url` through the backend.
    pub fn navigate(&mut self, url: &str) {
        self.url = SharedString::from(url.to_string());
        self.backend.navigate(url);
    }

    /// Pins `points` (page coordinates) as clicks would, and saves each with
    /// the note beside it. This is how a card opens on a page that already has
    /// annotations on it, without a second code path for seeded pins.
    pub fn seed_pins(&mut self, points: &[((f32, f32), &str)]) {
        for (at, note) in points {
            self.backend.point_moved(Some(*at));
            self.backend.point_clicked(*at);
            self.apply_pending();
            if let Some(last) = self.annotations.last_mut() {
                last.note = SharedString::from(note.to_string());
                last.pending = false;
            }
        }
        self.backend.point_moved(None);
        self.hovered = None;
        self.note = None;
    }

    /// Takes everything the backend has to say and folds it into the state,
    /// answering whether anything changed. Draws nothing: [`Self::drain`] is
    /// what turns a change into a re-render.
    fn apply_pending(&mut self) -> bool {
        let events = self.backend.poll_events();
        let hovered = self.backend.hovered();
        let mut changed = hovered != self.hovered;
        self.hovered = hovered;
        for event in events {
            changed = true;
            match event {
                WebEvent::Title(title) => self.title = SharedString::from(title),
                WebEvent::Url(url) => self.url = SharedString::from(url),
                WebEvent::Loading(loading) => self.loading = loading,
                WebEvent::Annotation(annotation) => self.push_annotation(annotation),
                WebEvent::Screenshot(bytes) => self.screenshot = Some(bytes),
            }
        }
        changed
    }

    /// The poll timer's tick: fold and, if anything moved, re-render.
    fn drain(&mut self, cx: &mut Context<Self>) {
        if self.apply_pending() {
            cx.notify();
        }
    }

    /// Files a new pin and opens its note.
    fn push_annotation(&mut self, annotation: Annotation) {
        let info = self.backend.element_info(annotation.index);
        self.annotations.push(annotation);
        self.elements.push(info);
        let position = self.annotations.len() - 1;
        self.selected = Some(position);
        self.note = Some(position);
    }

    /// A window position turned into page coordinates.
    fn to_page(&self, position: Point<Pixels>) -> (f32, f32) {
        (f32::from(position.x - self.page_origin.x), f32::from(position.y - self.page_origin.y))
    }

    /// The pointer moved over the page.
    fn pointer_moved(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.annotate {
            return;
        }
        self.backend.point_moved(Some(self.to_page(position)));
        let hovered = self.backend.hovered();
        if hovered != self.hovered {
            self.hovered = hovered;
            cx.notify();
        }
    }

    /// The pointer was pressed over the page.
    fn pointer_down(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.annotate {
            return;
        }
        let at = self.to_page(position);
        self.backend.point_moved(Some(at));
        self.backend.point_clicked(at);
        self.drain(cx);
    }

    /// Handles a nav-row control, answering with the intent it raises.
    fn browser_action(&mut self, action: BrowserAction, cx: &mut Context<Self>) -> Option<WebviewIntent> {
        cx.notify();
        match action {
            BrowserAction::Back => {
                self.backend.back();
                None
            }
            BrowserAction::Forward => {
                self.backend.forward();
                None
            }
            BrowserAction::Reload => {
                self.backend.reload();
                None
            }
            BrowserAction::FocusUrl => Some(WebviewIntent::FocusUrl),
            BrowserAction::ToggleAnnotate => {
                let on = !self.annotate;
                self.set_annotate(on);
                Some(WebviewIntent::Annotate(on))
            }
            BrowserAction::Screenshot => {
                self.backend.capture();
                Some(WebviewIntent::Screenshot)
            }
            BrowserAction::Console => Some(WebviewIntent::Console),
        }
    }

    /// Handles the annotations panel, answering with the intent it raises.
    fn annotator_action(&mut self, action: AnnotatorAction, cx: &mut Context<Self>) -> Option<WebviewIntent> {
        cx.notify();
        match action {
            AnnotatorAction::Close => {
                self.set_annotate(false);
                Some(WebviewIntent::Annotate(false))
            }
            AnnotatorAction::Clear => {
                self.annotations.clear();
                self.elements.clear();
                self.selected = None;
                self.note = None;
                None
            }
            AnnotatorAction::Select(position) => {
                self.selected = Some(position);
                self.note = None;
                None
            }
            AnnotatorAction::Send => Some(WebviewIntent::SendAnnotations {
                annotations: self.annotations.clone(),
                screenshot: self.screenshot.clone(),
                url: self.url.to_string(),
            }),
        }
    }

    /// Handles the note popover.
    fn note_action(&mut self, action: NoteAction, cx: &mut Context<Self>) {
        if let Some(position) = self.note.take() {
            match action {
                NoteAction::Cancel => {
                    if position < self.annotations.len() {
                        self.annotations.remove(position);
                        self.elements.remove(position);
                    }
                    self.selected = None;
                }
                NoteAction::Save => {
                    if let Some(annotation) = self.annotations.get_mut(position) {
                        annotation.pending = false;
                        annotation.note = SharedString::from(SAVED_NOTE);
                    }
                }
            }
        }
        cx.notify();
    }

    /// The pins as fractions of the page box, for the panel's preview tile.
    fn preview_pins(&self) -> Option<Vec<(f32, f32)>> {
        let (w, h) = (f32::from(self.page_size.width), f32::from(self.page_size.height));
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some(
            self.elements
                .iter()
                .flatten()
                .map(|info| ((info.origin.0 + info.size.0) / w, info.origin.1 / h))
                .collect(),
        )
    }
}

type IntentHandler = Rc<dyn Fn(WebviewIntent, &mut Window, &mut App)>;

/// The browser pane. Build with [`webview_pane`].
#[derive(IntoElement)]
pub struct WebviewPane {
    id: ElementId,
    state: Entity<WebviewState>,
    on_intent: Option<IntentHandler>,
}

/// The pane for `state`: the nav row, the page with the annotator's overlays,
/// and — only while annotate mode is on — the annotations panel.
pub fn webview_pane(id: impl Into<ElementId>, state: &Entity<WebviewState>) -> WebviewPane {
    WebviewPane { id: id.into(), state: state.clone(), on_intent: None }
}

impl WebviewPane {
    /// Called with every [`WebviewIntent`] the pane raises.
    pub fn on_intent(mut self, f: impl Fn(WebviewIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for WebviewPane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let state = self.state.clone();
        let handler = self.on_intent.clone();
        let view = state.read(cx);
        let annotate = view.annotate;
        let annotations = view.annotations.clone();
        let elements = view.elements.clone();
        let hovered = view.hovered.clone();
        let selected = view.selected;
        let note = view.note;
        let preview = view.preview_pins();
        let nav = browser_nav((id.clone(), "nav"), view.url.clone())
            .annotating(annotate)
            .loading(if view.loading { 1.0 } else { 0.0 })
            .can_go_back(view.backend.can_go_back())
            .can_go_forward(view.backend.can_go_forward())
            .on_action({
                let state = state.clone();
                let handler = handler.clone();
                move |action, window, cx| {
                    let intent = state.update(cx, |state, cx| state.browser_action(action, cx));
                    if let (Some(intent), Some(handler)) = (intent, handler.clone()) {
                        handler(intent, window, cx);
                    }
                }
            });

        // The page area records its own box during prepaint, so a pointer
        // position can be turned into page coordinates without the state
        // having to guess the layout.
        let mut page = page_body()
            .id((id.clone(), "page"))
            .child(
                canvas(
                    {
                        let state = state.clone();
                        move |bounds, _, cx| {
                            state.update(cx, |state, cx| {
                                state.page_origin = bounds.origin;
                                if state.page_size != bounds.size {
                                    // The panel's preview tile is drawn from
                                    // this box, so a resize — the first layout
                                    // included — needs one more frame.
                                    state.page_size = bounds.size;
                                    cx.notify();
                                }
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                // Out of flow and pinned to the page's box, so it measures the
                // same rectangle the overlays are positioned against and adds
                // nothing to the document's layout.
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h_full(),
            )
            .on_mouse_move({
                let state = state.clone();
                move |event: &MouseMoveEvent, _, cx| {
                    state.update(cx, |state, cx| state.pointer_moved(event.position, cx));
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                move |event: &MouseDownEvent, _, cx| {
                    state.update(cx, |state, cx| state.pointer_down(event.position, cx));
                }
            });

        // The hovered element, unless it is already the selected one — that
        // one keeps the tighter selected outline instead.
        let selected_selector = selected.and_then(|position| elements.get(position)).and_then(|info| info.as_ref()).map(|info| info.selector.clone());
        if annotate {
            if let Some(info) = hovered.filter(|info| Some(&info.selector) != selected_selector.as_ref()) {
                page = page.child(
                    div()
                        .absolute()
                        .left(px(info.origin.0))
                        .top(px(info.origin.1))
                        .child(element_outline(info.outline_label()).size(px(info.size.0), px(info.size.1))),
                );
            }
        }

        for (position, annotation) in annotations.iter().enumerate() {
            let Some(info) = elements.get(position).and_then(|info| info.as_ref()) else { continue };
            let is_selected = selected == Some(position);
            if is_selected {
                page = page.child(
                    div()
                        .absolute()
                        .left(px(info.origin.0))
                        .top(px(info.origin.1))
                        .child(element_outline(info.selector.clone()).selected(true).size(px(info.size.0), px(info.size.1))),
                );
            }
            page = page.child(pin_at((info.origin.0 + info.size.0, info.origin.1), annotation.index, is_selected));
        }

        // The note popover sits inside the page area, clear of the pin.
        let open_note = note.and_then(|position| Some((elements.get(position)?.as_ref()?, annotations.get(position)?)));
        if let Some((info, annotation)) = open_note {
            page = page.child(
                div()
                    .absolute()
                    .left(px(info.origin.0 + info.size.0 + NOTE_GAP))
                    .top(px(info.origin.1 + NOTE_DROP))
                    .child(note_popover((id.clone(), "note"), info.path(), annotation.note.clone()).on_action({
                        let state = state.clone();
                        move |action, _, cx| {
                            state.update(cx, |state, cx| state.note_action(action, cx));
                        }
                    })),
            );
        }

        let panel = annotate.then(|| {
            annotations_panel((id.clone(), "panel"), annotations.clone())
                .selected(selected)
                .screenshot(preview)
                .on_action({
                    let state = state.clone();
                    let handler = handler.clone();
                    move |action, window, cx| {
                        let intent = state.update(cx, |state, cx| state.annotator_action(action, cx));
                        if let (Some(intent), Some(handler)) = (intent, handler.clone()) {
                            handler(intent, window, cx);
                        }
                    }
                })
        });

        v_flex()
            .size_full()
            .min_h(px(0.0))
            .bg(p.surface_1)
            .child(nav)
            .child(h_flex().flex_1().w_full().min_h(px(0.0)).items_stretch().child(page).children(panel))
    }
}
