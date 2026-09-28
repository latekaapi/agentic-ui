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

use aui::a11y_text::a11y_text_input;
use aui::data::{icon_button, kbd, record_ax_label, ButtonSize};
use aui::util::{interaction_flags, TrackInteraction};
use aui::workbench::{annotations_panel, element_outline, note_popover, Annotation, AnnotatorAction, BrowserAction, NoteAction};
use aui_icons::{icon, IconName};
use aui_motion::{tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{actions, canvas, div, img, prelude::*, px, relative, App, Bounds, Context, ElementId, Entity, FocusHandle, Global, Image, ImageFormat, IntoElement, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Role, SharedString, Size, Subscription, Task, Window};
use gpui_kit::base::input::{Escape as InputEscape, InputEvent, InputState};
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

actions!(
    aui_webview,
    [
        /// Put the keyboard in the address field, selecting the whole URL so
        /// typing replaces it. Paste, select-all and caret movement come from
        /// the real text input, not from the pane.
        ///
        /// Bound to `cmd-l` in [`WEBVIEW_CONTEXT`] by [`bind_keys`], which
        /// [`WebviewState::new`] already calls. That binding only reaches the
        /// pane while gpui holds the keyboard: with a native page focused the
        /// keystroke goes to the page, so a host that wants `⌘L` everywhere
        /// must also bind `FocusAddress` at window level (no context) and
        /// route it to [`WebviewState::begin_editing`] (or focus
        /// [`WebviewState::focus_handle`]).
        FocusAddress,
    ]
);

/// The key context the webview pane puts on its outermost element: it owns
/// `cmd-l` ([`FocusAddress`]) while the keyboard is anywhere inside the pane.
pub const WEBVIEW_CONTEXT: &str = "AuiWebview";

/// Installs the webview's default bindings (`cmd-l` → [`FocusAddress`] in
/// [`WEBVIEW_CONTEXT`]). [`WebviewState::new`] calls this once per process;
/// hosts bind [`FocusAddress`] again at window level when they want `⌘L`
/// while the page itself holds the keyboard.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-l", FocusAddress, Some(WEBVIEW_CONTEXT))]);
}

/// Marks [`bind_keys`] as already installed for this process.
struct WebviewKeysInstalled;
impl Global for WebviewKeysInstalled {}

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
    /// Evaluation answers drained from the backend, waiting for
    /// [`Self::take_eval_results`].
    eval_results: Vec<(u64, Result<String, String>)>,
    /// The same bytes decoded once, so the stand-in for an obscured native page
    /// is not re-hashed on every frame.
    screenshot_image: Option<Arc<Image>>,
    /// Whether the backend draws itself over the gpui scene
    /// ([`WebBackend::is_native`]); everything below is only meaningful then.
    native: bool,
    /// Whether the address field is open for typing. While true the nav row
    /// shows the real text input; otherwise it shows the page URL.
    editing: bool,
    /// The address field's real single-line text input, created on the first
    /// edit. Paste, select-all and caret movement all come from this state,
    /// which is why the pane keeps no edit buffer of its own.
    address: Option<Entity<InputState>>,
    /// Keeps the address input's commit/blur subscription alive.
    _address_sub: Option<Subscription>,
    /// The keyboard focus the pane takes.
    focus: FocusHandle,
    /// Set by the host while a gpui overlay covers the page; see
    /// [`Self::set_obscured`].
    obscured: bool,
    /// The last box pushed into a native backend, so an unchanged layout costs
    /// no `setFrame:`.
    pushed: Option<(Point<Pixels>, Size<Pixels>)>,
    /// How much of the page area survived its ancestors' clipping, or `None`
    /// when the pane has been scrolled out of sight entirely.
    visible_box: Option<Bounds<Pixels>>,
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
        if !cx.has_global::<WebviewKeysInstalled>() {
            cx.set_global(WebviewKeysInstalled);
            bind_keys(cx);
        }
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
            eval_results: Vec::new(),
            native,
            editing: false,
            address: None,
            _address_sub: None,
            focus: cx.focus_handle(),
            obscured: false,
            pushed: None,
            visible_box: None,
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
        self.editing = false;
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

    /// Runs `js` in the page; its answer arrives through
    /// [`Self::take_eval_results`] under `request_id`. See
    /// [`WebBackend::eval_with_result`](crate::backend::WebBackend::eval_with_result).
    pub fn eval_with_result(&mut self, request_id: u64, js: &str) {
        self.backend.eval_with_result(request_id, js);
    }

    /// Takes the evaluation answers the poll timer has drained from the
    /// backend since the last call, by request id.
    pub fn take_eval_results(&mut self) -> Vec<(u64, Result<String, String>)> {
        std::mem::take(&mut self.eval_results)
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
        if obscured {
            // The page is covered: a native view that kept first responder
            // would eat the keystrokes meant for whatever covers it.
            self.release_keyboard();
        }
        self.sync_visibility();
    }

    /// Whether the page currently holds the keyboard. See
    /// [`WebBackend::holds_keyboard`](crate::backend::WebBackend::holds_keyboard).
    pub fn holds_keyboard(&self) -> bool {
        self.backend.holds_keyboard()
    }

    /// Gives the keyboard back to the host view. A no-op unless the page
    /// holds it — safe to call whenever the pane stops wanting page input.
    pub fn release_keyboard(&mut self) {
        if self.backend.holds_keyboard() {
            self.backend.set_focused(false);
        }
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
        let want = !self.obscured && self.visible_box.is_some() && self.laid_out.elapsed() < STALE_LAYOUT;
        if want != self.shown {
            self.shown = want;
            self.backend.set_visible(want);
        }
    }

    /// The page area was laid out at `bounds` (window coordinates), inside an
    /// ancestor clip of `clip`. A native backend is moved to match; everything
    /// else only records the box.
    ///
    /// The two rectangles are different things. `bounds` is where the page's
    /// document is, and it is what a pointer position is turned into page
    /// coordinates against. `clip` is what gpui would have masked the pane to,
    /// and a native child view **is not subject to that mask** — it is clipped
    /// only by the window's content view. Without the intersection a pane in a
    /// scrolling container paints its page over the app's own chrome, which is
    /// exactly what the gallery's stage does when it scrolls.
    fn laid_out(&mut self, bounds: Bounds<Pixels>, clip: Bounds<Pixels>) -> bool {
        let (origin, size) = (bounds.origin, bounds.size);
        self.page_origin = origin;
        self.laid_out = Instant::now();
        let resized = self.page_size != size;
        self.page_size = size;
        let visible = bounds.intersect(&clip);
        self.visible_box = (!visible.is_empty()).then_some(visible);
        if self.native && self.pushed != Some((visible.origin, visible.size)) {
            self.pushed = Some((visible.origin, visible.size));
            self.backend.set_bounds(
                (f32::from(visible.origin.x), f32::from(visible.origin.y)),
                (f32::from(visible.size.width), f32::from(visible.size.height)),
            );
            if resized && f32::from(visible.size.width) >= 1.0 && f32::from(visible.size.height) >= 1.0 {
                // (Not at zero area: the first layout reports a size before the
                // native view has a frame, and a snapshot then comes back 0×0 —
                // seen live as a failed PNG encode on every pane open.)
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

    /// Whether the address field is open for typing.
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    /// The address field's text input, once [`Self::begin_editing`] has run.
    pub fn address_input(&self) -> Option<Entity<InputState>> {
        self.address.clone()
    }

    /// Opens the address field for typing, taking the keyboard back off a
    /// native page (which holds first responder while it has focus).
    ///
    /// Fills the real text input with the current URL and selects it all, so
    /// typing replaces it the way a browser's address bar does. Paste, `⌘A`
    /// and caret movement all come from the input itself.
    pub fn begin_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.backend.set_focused(false);
        let url = self.url.to_string();
        let input = match self.address.clone() {
            Some(input) => input,
            None => {
                let input = cx.new(|cx| InputState::new(window, cx));
                let sub = cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
                    InputEvent::PressEnter { .. } => this.commit_address(cx),
                    InputEvent::Blur => this.leave_editing(cx),
                    InputEvent::Focus | InputEvent::Change => {}
                });
                self._address_sub = Some(sub);
                self.address = Some(input.clone());
                input
            }
        };
        input.update(cx, |input, cx| {
            input.set_value(url, window, cx);
            input.select_all(window, cx);
            input.focus(window, cx);
        });
        self.editing = true;
        cx.notify();
    }

    /// Commits the address field: the typed text becomes a URL and the page
    /// navigates to it. Runs on the input's enter event.
    fn commit_address(&mut self, cx: &mut Context<Self>) {
        let typed = self.address.as_ref().map(|input| input.read(cx).value().to_string()).unwrap_or_default();
        self.editing = false;
        self.navigate(&normalize_url(&typed));
        cx.notify();
    }

    /// Leaves the address field without navigating: the URL display falls
    /// back to the page URL on the next frame. Runs on input blur and, via
    /// the pane's propagated-escape handler, on escape.
    fn leave_editing(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            self.editing = false;
            cx.notify();
        }
    }

    /// Takes everything the backend has to say and folds it into the state,
    /// answering whether anything changed. Draws nothing: [`Self::drain`] is
    /// what turns a change into a re-render.
    fn apply_pending(&mut self) -> bool {
        let events = self.backend.poll_events();
        let eval_results = self.backend.take_eval_results();
        let hovered = self.backend.hovered();
        let mut changed = hovered != self.hovered;
        let mut settled = false;
        self.hovered = hovered;
        for event in events {
            changed = true;
            match event {
                WebEvent::Title(title) => self.title = SharedString::from(title),
                WebEvent::Url(url) => {
                    // Typing in the address field survives the page announcing
                    // itself underneath it.
                    if !self.editing {
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
        if !eval_results.is_empty() {
            // Answers for the host's agent tools; they wait in the state until
            // `take_eval_results`, and their arrival is a change worth a
            // re-render like any other backend news.
            self.eval_results.extend(eval_results);
            changed = true;
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

impl Drop for WebviewState {
    /// Gives the keyboard back as the pane goes away: a dropped native view
    /// that kept first responder would leave the window's keystrokes aimed at
    /// a view that is no longer on screen.
    fn drop(&mut self) {
        self.release_keyboard();
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
        // Below ~560 px the full nav row starves the URL field.
        let compact = f32::from(view.page_size.width) > 0.0 && f32::from(view.page_size.width) < 560.0;
        let nav_handler: NavHandler = {
            let state = state.clone();
            let handler = handler.clone();
            Rc::new(move |action, window, cx| {
                let intent = state.update(cx, |state, cx| state.browser_action(action, window, cx));
                if let (Some(intent), Some(handler)) = (intent, handler.clone()) {
                    handler(intent, window, cx);
                }
            })
        };
        let nav = nav_row(
            (id.clone(), "nav").into(),
            view.url.clone(),
            view.address.clone(),
            view.editing,
            view.backend.can_go_back(),
            view.backend.can_go_forward(),
            if view.loading { 1.0 } else { 0.0 },
            annotate,
            compact,
            Some(nav_handler),
            _window,
            cx,
        );

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
                        move |bounds, window, cx| {
                            // The clip its ancestors would have masked the pane
                            // to. A native page is not masked by gpui, so the
                            // pane has to do it by hand.
                            let clip = window.content_mask().bounds;
                            state.update(cx, |state, cx| {
                                // This is also where a native page is moved:
                                // the webview follows the pane because the pane
                                // measures itself every prepaint.
                                if state.laid_out(bounds, clip) {
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
            .key_context(WEBVIEW_CONTEXT)
            .on_action({
                let state = state.clone();
                move |_: &FocusAddress, window: &mut Window, cx: &mut App| {
                    state.update(cx, |state, cx| state.begin_editing(window, cx));
                }
            })
            // The input propagates escape (it only consumes it for IME and
            // inline completions), so an escape that arrives here while the
            // address field is open cancels the edit; otherwise it travels on.
            .on_action({
                let state = state.clone();
                move |_: &InputEscape, _: &mut Window, cx: &mut App| {
                    state.update(cx, |state, cx| {
                        if state.editing {
                            state.leave_editing(cx);
                        } else {
                            cx.propagate();
                        }
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

/// The nav band: 38 px row, 4 px gap, 8 px side padding.
const NAV_HEIGHT: f32 = 38.0;
const NAV_GAP: f32 = 4.0;
const NAV_PAD_X: f32 = 8.0;
/// The URL slot: 28 px, 10 px side padding, 8 px gap, 11 px shield.
const URL_HEIGHT: f32 = 28.0;
const URL_PAD_X: f32 = 10.0;
const URL_GAP: f32 = 8.0;
const URL_SHIELD: f32 = 11.0;
/// The annotate toggle: 28 px, 10 px side padding, 6 px gap, 12 px glyph.
const MODE_HEIGHT: f32 = 28.0;
const MODE_PAD_X: f32 = 10.0;
const MODE_GAP: f32 = 6.0;
const MODE_GLYPH: f32 = 12.0;
/// The `esc` cap inside the filled toggle over the ink ground.
const MODE_KBD_BORDER_ALPHA: f32 = 0.3;
/// `.kbd{height:18px;padding:0 5px}` — the toggle draws its own, untinted.
const KBD_HEIGHT: f32 = 18.0;
const KBD_PAD_X: f32 = 5.0;
/// The loading hairline at the bottom of the nav row: 2 px accent.
const LOADING_HEIGHT: f32 = 2.0;

/// Called with the [`BrowserAction`] a nav control stands for.
type NavHandler = Rc<dyn Fn(BrowserAction, &mut Window, &mut App)>;

/// The 38 px nav row: history controls, the address slot, the annotate
/// toggle, screenshot and console. Every control names itself for the
/// accessibility tree — icon-only buttons through explicit labels, the
/// address slot through "Address".
#[allow(clippy::too_many_arguments)]
fn nav_row(
    id: ElementId,
    url: SharedString,
    address: Option<Entity<InputState>>,
    editing: bool,
    can_go_back: bool,
    can_go_forward: bool,
    loading: f32,
    annotating: bool,
    compact: bool,
    handler: Option<NavHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let p = cx.aui().colors;
    let nav_button = |key: &'static str, glyph: IconName, label: &'static str, action: BrowserAction, disabled: bool| {
        let handler = handler.clone();
        let mut b = icon_button((id.clone(), key), glyph)
            .ghost()
            .size(ButtonSize::Sm)
            .disabled(disabled)
            .accessibility_label(label);
        if let Some(handler) = handler {
            b = b.on_click(move |_, window, cx| handler(action, window, cx));
        }
        b
    };

    // The address slot. While editing it is the real single-line input
    // (role + "Address" label come from `a11y_text_input`); otherwise the
    // page URL, clickable to start editing.
    let url_id: ElementId = (id.clone(), "url").into();
    let url_field = match (editing, address) {
        (true, Some(input)) => h_flex()
            .id(url_id)
            .flex_1()
            .min_w(px(0.0))
            .h(px(URL_HEIGHT))
            .px(px(URL_PAD_X))
            .rounded(px(scale::R_SM))
            .bg(p.surface_2)
            .items_center()
            .child(a11y_text_input((id.clone(), "address"), "Address", &input, cx))
            .into_any_element(),
        _ => {
            record_ax_label("Address");
            let mut field = h_flex()
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
                .role(Role::Button)
                .aria_label("Address")
                .child(icon(IconName::Shield).size(px(URL_SHIELD)).color(p.success))
                .child(div().flex_1().min_w(px(0.0)).truncate().child(url))
                .when(!compact, |d| d.child(kbd("⌘L")));
            if let Some(handler) = handler.clone() {
                field = field.on_click(move |_, window, cx| handler(BrowserAction::FocusUrl, window, cx));
            }
            field.into_any_element()
        }
    };

    // The row wraps below ~300 px instead of clipping its trailing
    // controls; one line still measures exactly as before.
    h_flex()
        .relative()
        .w_full()
        .min_h(px(NAV_HEIGHT))
        .flex_none()
        .flex_wrap()
        .px(px(NAV_PAD_X))
        .gap(px(NAV_GAP))
        .border_b_1()
        .border_color(p.line)
        .child(nav_button("back", IconName::ArrowLeft, "Back", BrowserAction::Back, !can_go_back))
        .child(nav_button("forward", IconName::ArrowRight, "Forward", BrowserAction::Forward, !can_go_forward))
        .child(nav_button("reload", IconName::Refresh, "Reload", BrowserAction::Reload, false))
        .child(url_field)
        .child(annotate_toggle((id.clone(), "annotate"), annotating, compact, handler.clone(), window, cx))
        .child(nav_button("screenshot", IconName::Camera, "Screenshot", BrowserAction::Screenshot, false))
        .child(nav_button("console", IconName::Terminal, "Terminal", BrowserAction::Console, false))
        .when(loading > 0.0, |d| {
            d.child(div().absolute().left_0().bottom_0().w(relative(loading)).h(px(LOADING_HEIGHT)).bg(p.accent))
        })
}

/// The annotate toggle: ink-filled while annotating, the secondary control
/// look when off. Carries the Button role and the "Annotate" name.
fn annotate_toggle(id: impl Into<ElementId>, on: bool, compact: bool, handler: Option<NavHandler>, window: &mut Window, cx: &mut App) -> impl IntoElement {
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

    record_ax_label("Annotate");
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
        .role(Role::Button)
        .aria_label("Annotate")
        .child(icon(IconName::Edit).size(px(MODE_GLYPH)).color(text))
        .when(!compact, |d| d.child("Annotate"))
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

#[cfg(test)]
mod tests {
    use super::normalize_url;
    use super::{webview_pane, WebviewState};
    use crate::agent_js;
    use crate::fake::FakeWebBackend;
    use gpui::{AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext, Window};

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

    /// The state forwards the script to the backend and hands the drained
    /// answer back out; a second take sees nothing.
    #[gpui::test]
    fn eval_results_flow_through_the_state(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
        state.update(cx, |state, _| state.eval_with_result(11, &agent_js::click("h1")));
        // The poll timer has not ticked yet, so nothing has been drained.
        let early: Vec<(u64, Result<String, String>)> = state.update(cx, |state, _| state.take_eval_results());
        assert!(early.is_empty());
        state.update(cx, |state, _| {
            assert!(state.apply_pending());
        });
        let answers = state.update(cx, |state, _| state.take_eval_results());
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].0, 11);
        assert_eq!(answers[0].1, Ok(String::from("\"Simple pricing\"")));
        let drained: Vec<(u64, Result<String, String>)> = state.update(cx, |state, _| state.take_eval_results());
        assert!(drained.is_empty());
    }

    /// `release_keyboard` hands focus back when the page holds it, and is a
    /// no-op otherwise.
    #[gpui::test]
    fn release_keyboard_gives_the_keyboard_back(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
        state.update(cx, |state, _| {
            assert!(!state.holds_keyboard());
            // A no-op release records nothing on the backend.
            state.release_keyboard();
            assert!(!state.holds_keyboard());
        });
        state.update(cx, |state, _| state.backend.set_focused(true));
        state.update(cx, |state, _| {
            assert!(state.holds_keyboard());
            state.release_keyboard();
            assert!(!state.holds_keyboard());
        });
    }

    /// Covering the page hands the keyboard back, so the overlay — not the
    /// hidden page — reads the next keystroke.
    #[gpui::test]
    fn obscuring_the_page_releases_the_keyboard(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
        state.update(cx, |state, _| state.backend.set_focused(true));
        state.update(cx, |state, _| {
            assert!(state.holds_keyboard());
            state.set_obscured(true);
            assert!(!state.holds_keyboard());
        });
    }

    /// The host a pane test renders in a real window, the way an application
    /// renders the pane.
    struct PaneHost {
        state: Entity<WebviewState>,
    }

    impl Render for PaneHost {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            webview_pane("test-pane", &self.state)
        }
    }

    /// Runs `f` with a rendered pane plus its state: theme and input bindings
    /// installed, one frame drawn.
    fn with_pane(cx: &mut TestAppContext, f: impl FnOnce(Entity<WebviewState>, &mut VisualTestContext)) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        let (host, mut vcx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
            PaneHost { state }
        });
        let state = vcx.update(|_, cx| host.read(cx).state.clone());
        f(state, &mut vcx);
    }

    /// The address input's value and selected byte range.
    fn address_snapshot(state: &Entity<WebviewState>, vcx: &mut VisualTestContext) -> (String, std::ops::Range<usize>) {
        vcx.update(|_, cx| {
            let input = state.read(cx).address_input().expect("begin_editing creates the input");
            let snapshot = input.read(cx);
            (snapshot.value().to_string(), snapshot.selected_range())
        })
    }

    /// Opening the address field fills it with the page URL and selects the
    /// whole thing, so typing replaces it instead of appending to it.
    #[gpui::test]
    fn begin_editing_selects_the_whole_url(cx: &mut TestAppContext) {
        with_pane(cx, |state, vcx| {
            let home = vcx.update(|_, cx| state.read(cx).url().to_string());
            vcx.update(|window, cx| state.update(cx, |state, cx| state.begin_editing(window, cx)));
            assert!(vcx.update(|_, cx| state.read(cx).is_editing()));
            let (value, selected) = address_snapshot(&state, vcx);
            assert_eq!(value, home);
            assert_eq!(selected, 0..home.len(), "the whole URL must be selected so typing replaces it");
            // Typing over the selection replaces it — the old buffer appended
            // instead once an unhandled key had spent its fresh flag.
            vcx.update(|window, cx| {
                state
                    .read(cx)
                    .address_input()
                    .unwrap()
                    .update(cx, |input, cx| input.replace("example.com", window, cx));
            });
            let (value, _) = address_snapshot(&state, vcx);
            assert_eq!(value, "example.com");
        });
    }

    /// Enter commits the field: the typed text is normalised to a URL, the
    /// page navigates there, and editing ends.
    #[gpui::test]
    fn enter_navigates_the_normalised_url(cx: &mut TestAppContext) {
        with_pane(cx, |state, vcx| {
            vcx.update(|window, cx| state.update(cx, |state, cx| state.begin_editing(window, cx)));
            vcx.update(|window, cx| {
                state
                    .read(cx)
                    .address_input()
                    .unwrap()
                    .update(cx, |input, cx| input.replace("example.com", window, cx));
            });
            vcx.simulate_keystrokes("enter");
            let (url, editing) = vcx.update(|_, cx| (state.read(cx).url().to_string(), state.read(cx).is_editing()));
            assert_eq!(url, "https://example.com");
            assert!(!editing, "enter leaves the address field");
        });
    }

    /// Escape cancels the edit: the page URL is untouched and editing ends.
    #[gpui::test]
    fn escape_restores_the_url(cx: &mut TestAppContext) {
        with_pane(cx, |state, vcx| {
            let home = vcx.update(|_, cx| state.read(cx).url().to_string());
            vcx.update(|window, cx| state.update(cx, |state, cx| state.begin_editing(window, cx)));
            vcx.update(|window, cx| {
                state
                    .read(cx)
                    .address_input()
                    .unwrap()
                    .update(cx, |input, cx| input.replace("example.com", window, cx));
            });
            vcx.simulate_keystrokes("escape");
            let (url, editing) = vcx.update(|_, cx| (state.read(cx).url().to_string(), state.read(cx).is_editing()));
            assert_eq!(url, home);
            assert!(!editing, "escape leaves the address field");
        });
    }

    /// `⌘L` in the pane's key context opens the address field through the
    /// `FocusAddress` action.
    #[gpui::test]
    fn cmd_l_begins_editing(cx: &mut TestAppContext) {
        with_pane(cx, |state, vcx| {
            // The keystroke reaches the pane's context through the keyboard,
            // so park the keyboard in the pane first.
            vcx.update(|window, cx| window.focus(&state.read(cx).focus_handle().clone(), cx));
            vcx.simulate_keystrokes("cmd-l");
            assert!(vcx.update(|_, cx| state.read(cx).is_editing()), "⌘L must open the address field");
            let (value, selected) = address_snapshot(&state, vcx);
            assert_eq!(value, vcx.update(|_, cx| state.read(cx).url().to_string()));
            assert_eq!(selected, 0..value.len());
        });
    }

    /// Every nav control names itself for the accessibility tree: the
    /// icon-only buttons through explicit labels, the address slot and the
    /// annotate toggle through theirs. An empty recorded label is a control
    /// the tree has no name for.
    #[gpui::test]
    fn nav_controls_carry_accessibility_labels(cx: &mut TestAppContext) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        aui::data::arm_ax_probe(true);
        aui::data::take_ax_labels();
        with_pane(cx, |_state, _vcx| {});
        let labels = aui::data::take_ax_labels();
        aui::data::arm_ax_probe(false);
        for want in ["Back", "Forward", "Reload", "Address", "Annotate", "Screenshot", "Terminal"] {
            assert!(labels.iter().any(|label| label == want), "missing AX label {want:?} in {labels:?}");
        }
        assert!(!labels.iter().any(String::is_empty), "an empty label is an unnamed control in {labels:?}");
    }
}
