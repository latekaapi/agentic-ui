//! The gpui side: [`WebviewState`] holds a [`WebBackend`] and what the pane
//! knows about it, and [`webview_pane`] draws the nav row, the page and the
//! annotations panel over it.
//!
//! Nothing here reaches into a browser directly. The pane sends
//! [`aui::workbench::BrowserAction`]s and pointer positions down into the
//! backend and re-renders from the [`WebEvent`]s a poll timer drains, so the
//! same element tree runs over the scripted mock and over a real WKWebView.

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aui::workbench::{annotations_panel, browser_nav, element_outline, note_popover, Annotation, AnnotatorAction, BrowserAction, NoteAction};
use aui_tokens::ActiveAui;
use gpui::{canvas, div, img, prelude::*, px, App, Context, ElementId, Entity, FocusHandle, Image, ImageFormat, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, SharedString, Size, Task, Window};
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
/// A native page is hidden again if it has not been laid out for this long —
/// the gallery switched card, or the window went away. Two poll ticks, so a
/// dropped frame does not flicker it.
const STALE_LAYOUT: Duration = Duration::from_millis(200);

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
    /// The same bytes decoded once, so the stand-in for an obscured native page
    /// is not re-hashed on every frame.
    screenshot_image: Option<Arc<Image>>,
    /// Whether the backend draws itself over the gpui scene
    /// ([`WebBackend::is_native`]); everything below is only meaningful then.
    native: bool,
    /// The URL field's edit buffer while `⌘L` has the keyboard.
    editing: Option<String>,
    /// Whether that buffer is still the URL the field opened on. A browser's
    /// address bar selects its contents on focus, so the first character typed
    /// replaces the whole thing rather than appending to it.
    editing_fresh: bool,
    /// The keyboard focus the URL field takes.
    focus: FocusHandle,
    /// Set by the host while a gpui overlay covers the page; see
    /// [`Self::set_obscured`].
    obscured: bool,
    /// The last box pushed into a native backend, so an unchanged layout costs
    /// no `setFrame:`.
    pushed: Option<(Point<Pixels>, Size<Pixels>)>,
    /// When the page area was last laid out, and what the native view was last
    /// told, so a card that stopped rendering takes its webview with it.
    laid_out: Instant,
    shown: bool,
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
        let native = backend.is_native();
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
            screenshot_image: None,
            native,
            editing: None,
            editing_fresh: false,
            focus: cx.focus_handle(),
            obscured: false,
            pushed: None,
            laid_out: Instant::now(),
            shown: true,
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
        self.editing = None;
        self.editing_fresh = false;
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

    /// Whether the page is a native surface over the gpui scene.
    pub fn is_native(&self) -> bool {
        self.native
    }

    /// The focus the URL field takes; the host can `focus` it to open `⌘L`.
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// Asks the backend for a screenshot. The bytes arrive on the poll as
    /// [`WebEvent::Screenshot`] and are what
    /// [`WebviewIntent::SendAnnotations`] then carries.
    pub fn capture(&mut self) {
        self.backend.capture();
    }

    /// The last screenshot the backend produced, PNG-encoded.
    pub fn screenshot(&self) -> Option<&[u8]> {
        self.screenshot.as_deref()
    }

    /// **Hides the native page so gpui can paint over its rectangle.**
    ///
    /// A `wry` child view is composited *above* the gpui scene, so a command
    /// palette, a menu or a note popover that overlaps the browser pane is
    /// simply not drawn. There is no compositing fix for that: the only way to
    /// put gpui pixels in that rectangle is to take the native view out of it.
    ///
    /// So the host calls this with `true` while such an overlay is open. The
    /// pane hides the native view and paints the last screenshot in its place
    /// (a flat surface if there is none yet), which keeps the page looking
    /// like the page while the overlay is up. `false` puts it back.
    ///
    /// Ask for a [`Self::capture`] when the pane opens and after each
    /// navigation, or the stand-in will be a flat surface.
    pub fn set_obscured(&mut self, obscured: bool) {
        if self.obscured == obscured {
            return;
        }
        self.obscured = obscured;
        self.sync_visibility();
    }

    /// Whether the host has the page covered.
    pub fn obscured(&self) -> bool {
        self.obscured
    }

    /// Shows the native view only while the pane is both on screen and not
    /// covered. A card the gallery switched away from stops laying the page
    /// area out, and its webview would otherwise stay pinned over whatever
    /// replaced it.
    fn sync_visibility(&mut self) {
        if !self.native {
            return;
        }
        let want = !self.obscured && self.laid_out.elapsed() < STALE_LAYOUT;
        if want != self.shown {
            self.shown = want;
            self.backend.set_visible(want);
        }
    }

    /// The page area was laid out at `origin`/`size` (window coordinates). A
    /// native backend is moved to match; everything else only records the box.
    fn laid_out(&mut self, origin: Point<Pixels>, size: Size<Pixels>) -> bool {
        self.page_origin = origin;
        self.laid_out = Instant::now();
        let resized = self.page_size != size;
        self.page_size = size;
        if self.native && self.pushed != Some((origin, size)) {
            self.pushed = Some((origin, size));
            self.backend.set_bounds((f32::from(origin.x), f32::from(origin.y)), (f32::from(size.width), f32::from(size.height)));
            if resized {
                // The stand-in an overlay is painted over has to be the page at
                // *this* size, so a reflow is worth a new picture. Taking it
                // here rather than in `set_obscured` also keeps it a picture of
                // a visible view: `takeSnapshot` on a hidden one comes back
                // blank.
                self.backend.capture();
            }
        }
        self.sync_visibility();
        resized
    }

    /// Opens the URL field for typing, taking the keyboard back off a native
    /// page (which holds first responder while it has focus).
    fn begin_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = Some(self.url.to_string());
        self.editing_fresh = true;
        self.backend.set_focused(false);
        window.focus(&self.focus, cx);
    }

    /// A keystroke while the URL field has the keyboard. Answers whether it
    /// was consumed.
    fn url_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let fresh = std::mem::take(&mut self.editing_fresh);
        let Some(buffer) = self.editing.as_mut() else { return false };
        let key = event.keystroke.key.as_str();
        match key {
            "enter" => {
                let url = normalize_url(&self.editing.take().unwrap_or_default());
                self.navigate(&url);
            }
            "escape" => {
                self.editing = None;
            }
            "backspace" => {
                if fresh {
                    buffer.clear();
                } else {
                    buffer.pop();
                }
            }
            _ => {
                // `key_char` is the character the layout actually produced, so
                // this types the same text a browser's URL bar would.
                match event.keystroke.key_char.as_deref() {
                    Some(text) if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform => {
                        if fresh {
                            buffer.clear();
                        }
                        buffer.push_str(text);
                    }
                    _ => return false,
                }
            }
        }
        cx.notify();
        true
    }

    /// Takes everything the backend has to say and folds it into the state,
    /// answering whether anything changed. Draws nothing: [`Self::drain`] is
    /// what turns a change into a re-render.
    fn apply_pending(&mut self) -> bool {
        let events = self.backend.poll_events();
        let hovered = self.backend.hovered();
        let mut changed = hovered != self.hovered;
        let mut settled = false;
        self.hovered = hovered;
        for event in events {
            changed = true;
            match event {
                WebEvent::Title(title) => self.title = SharedString::from(title),
                WebEvent::Url(url) => {
                    // Typing in the URL field survives the page announcing
                    // itself underneath it.
                    if self.editing.is_none() {
                        self.url = SharedString::from(url);
                    }
                }
                WebEvent::Loading(loading) => {
                    self.loading = loading;
                    settled |= !loading;
                }
                WebEvent::Annotation(annotation) => self.push_annotation(annotation),
                WebEvent::Screenshot(bytes) => {
                    self.screenshot_image = Some(Arc::new(Image::from_bytes(ImageFormat::Png, bytes.clone())));
                    self.screenshot = Some(bytes);
                }
            }
        }
        if settled {
            // The page that just finished loading is what an obscuring overlay
            // will be painted over, so take its picture now.
            self.backend.capture();
        }
        changed
    }

    /// The poll timer's tick: fold and, if anything moved, re-render.
    fn drain(&mut self, cx: &mut Context<Self>) {
        // A card the gallery navigated away from stops laying the page out;
        // the tick is where a native view notices and hides itself.
        self.sync_visibility();
        let changed = self.apply_pending();
        // A native page is asked to redraw on every tick even when nothing
        // changed. That is not decoration: `laid_out` is only refreshed by a
        // prepaint, and the prepaint is what tells the webview where the pane
        // moved to — a still window would let the view go stale under its own
        // liveness check and hide itself.
        if changed || self.native {
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
    fn browser_action(&mut self, action: BrowserAction, window: &mut Window, cx: &mut Context<Self>) -> Option<WebviewIntent> {
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
            BrowserAction::FocusUrl => {
                self.begin_editing(window, cx);
                Some(WebviewIntent::FocusUrl)
            }
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

/// What a person types in a URL field is rarely a URL. Anything with a scheme
/// is taken as it is; a bare host gets `https://`; anything with a space is a
/// search. This is the whole of the pane's address-bar cleverness.
fn normalize_url(typed: &str) -> String {
    let typed = typed.trim();
    if typed.is_empty() {
        return String::from("about:blank");
    }
    if typed.contains("://") || typed.starts_with("about:") || typed.starts_with("data:") || typed.starts_with("file:") {
        return typed.to_string();
    }
    if typed.contains(' ') || !typed.contains('.') {
        return format!("https://duckduckgo.com/?q={}", urlencode(typed));
    }
    format!("https://{typed}")
}

/// Percent-encodes a search term. Only the handful of bytes a query string
/// cannot carry — this is not a general-purpose URL encoder.
fn urlencode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
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
        let native = view.native;
        let obscured = view.obscured;
        let focus = view.focus.clone();
        let stand_in = view.screenshot_image.clone();
        // While `⌘L` has the keyboard the field shows what is being typed, not
        // where the page currently is.
        let shown_url = match &view.editing {
            Some(buffer) => SharedString::from(buffer.clone()),
            None => view.url.clone(),
        };
        let nav = browser_nav((id.clone(), "nav"), shown_url)
            .annotating(annotate)
            .loading(if view.loading { 1.0 } else { 0.0 })
            .can_go_back(view.backend.can_go_back())
            .can_go_forward(view.backend.can_go_forward())
            .on_action({
                let state = state.clone();
                let handler = handler.clone();
                move |action, window, cx| {
                    let intent = state.update(cx, |state, cx| state.browser_action(action, window, cx));
                    if let (Some(intent), Some(handler)) = (intent, handler.clone()) {
                        handler(intent, window, cx);
                    }
                }
            });

        // The page area records its own box during prepaint, so a pointer
        // position can be turned into page coordinates without the state
        // having to guess the layout — and so a native page can be moved to
        // sit exactly on it.
        //
        // A native backend draws its own document over this rectangle, so the
        // rectangle itself is left as bare ground; only the scripted page is a
        // gpui document.
        let base = if native {
            div().relative().flex_1().min_w(px(0.0)).overflow_hidden().bg(p.surface_2)
        } else {
            page_body()
        };
        let mut page = base
            .id((id.clone(), "page"))
            .child(
                canvas(
                    {
                        let state = state.clone();
                        move |bounds, _, cx| {
                            state.update(cx, |state, cx| {
                                // This is also where a native page is moved:
                                // the webview follows the pane because the pane
                                // measures itself every prepaint.
                                if state.laid_out(bounds.origin, bounds.size) {
                                    // The panel's preview tile is drawn from
                                    // this box, so a resize — the first layout
                                    // included — needs one more frame.
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

        // The native view has been hidden so gpui can paint here; its last
        // screenshot stands in for it, and a flat surface stands in for that
        // when the page has not been captured yet.
        if native && obscured {
            let cover = div().absolute().inset_0().overflow_hidden().bg(p.surface_2);
            page = page.child(match stand_in {
                Some(image) => cover.child(img(image).size_full()).into_any_element(),
                None => cover.into_any_element(),
            });
        }

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
            .track_focus(&focus)
            .on_key_down({
                let state = state.clone();
                move |event: &KeyDownEvent, _, cx| {
                    state.update(cx, |state, cx| {
                        state.url_key(event, cx);
                    });
                }
            })
            .size_full()
            .min_h(px(0.0))
            .bg(p.surface_1)
            .child(nav)
            .child(h_flex().flex_1().w_full().min_h(px(0.0)).items_stretch().child(page).children(panel))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_url;

    #[test]
    fn what_is_typed_in_the_url_field_becomes_a_url() {
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
        assert_eq!(normalize_url("  example.com/pricing "), "https://example.com/pricing");
        assert_eq!(normalize_url("localhost:3000/checkout"), "https://duckduckgo.com/?q=localhost%3A3000%2Fcheckout");
        assert_eq!(normalize_url("about:blank"), "about:blank");
        assert_eq!(normalize_url("file:///tmp/page.html"), "file:///tmp/page.html");
        assert_eq!(normalize_url("data:text/html,<b>hi</b>"), "data:text/html,<b>hi</b>");
        assert_eq!(normalize_url(""), "about:blank");
    }

    #[test]
    fn a_phrase_is_a_search_rather_than_a_host() {
        assert_eq!(normalize_url("simple pricing page"), "https://duckduckgo.com/?q=simple+pricing+page");
        assert_eq!(normalize_url("worktrees"), "https://duckduckgo.com/?q=worktrees");
    }
}
