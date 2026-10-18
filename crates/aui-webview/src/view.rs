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
use std::time::Duration;

use aui::a11y_text::a11y_text_input;
use aui::data::record_ax_label;
use aui::workbench::{note_popover, Annotation, AnnotatorAction, BrowserAction, NoteAction};
use aui_tokens::ActiveAui;
use gpui::{actions, canvas, div, img, prelude::*, px, App, Bounds, Context, ElementId, Entity, FocusHandle, Focusable as _, Global, Image, ImageFormat, IntoElement, KeyBinding, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, SharedString, Size, Subscription, Task, Window};
use gpui_kit::base::input::{Escape as InputEscape, InputEvent, InputState};
use gpui_kit::base::{h_flex, v_flex};

use crate::backend::{BackendFeature, DownloadState, ElementInfo, FindState, PageSecurity, PermissionDecision, PermissionRequest, WebBackend, WebEvent};
use crate::page::page_body;

/// How often the pane drains its backend. Fast enough that a title or a pin
/// lands within a frame or two, slow enough to cost nothing while idle.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How long a cover waits for its fresh picture before giving up on it
/// ([`WebviewState::set_obscured`]): a CDP screenshot answers in tens of ms.
const COVER_CAPTURE_WAIT: Duration = Duration::from_millis(300);

/// After the fresh picture lands, the native view stays up this much
/// longer, so the frame that paints the picture is on screen before the
/// live page goes (no bare frame between them).
const COVER_HANDOFF: Duration = Duration::from_millis(40);

/// How often a cover that waits for its picture drains the backend: about a
/// frame, so the picture stands in the frame after it lands.
const COVER_POLL_INTERVAL: Duration = Duration::from_millis(8);
/// Where the note popover opens relative to the pinned element: just clear of
/// its right edge, dropped a little so the pin stays visible.
const NOTE_GAP: f32 = 14.0;
const NOTE_DROP: f32 = 14.0;
/// The note a saved pin keeps. The pane has no text field over the page yet,
/// so Save keeps this stand-in rather than an empty row.
const SAVED_NOTE: &str = "Note for the agent.";
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
    /// Open the page in Browse (D6's nav row).
    OpenInBrowse,
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
        /// Where each pin sits on the page (its element's top-right), in
        /// the page's own points, in annotation order; `None` for an
        /// element the page could not locate.
        pins: Vec<Option<(f32, f32)>>,
        /// The page's size in the same points, which places the pins on the
        /// screenshot.
        page_size: (f32, f32),
    },
}

/// How many addresses [`WebviewState::recent_pages`] keeps.
pub const RECENT_PAGES: usize = 8;

/// One address the page has shown, and the title it showed there.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentPage {
    /// The main-frame address.
    pub url: SharedString,
    /// The title it showed there (empty until one arrived).
    pub title: SharedString,
}

/// Move `url` to the front of `recent` (a blank page never counts).
fn note_recent(recent: &mut Vec<RecentPage>, url: &str) {
    if matches!(url.trim(), "" | "about:blank") {
        return;
    }
    let title = recent.iter().position(|p| p.url.as_ref() == url).map(|i| recent.remove(i).title).unwrap_or_default();
    recent.insert(0, RecentPage { url: url.to_string().into(), title });
    recent.truncate(RECENT_PAGES);
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
    /// The page's icon URL, if it has one ([`WebEvent::Favicon`]).
    favicon: Option<SharedString>,
    /// How far the load has come, 0.0–1.0 ([`WebEvent::Progress`]).
    progress: f32,
    /// What the site button says ([`WebEvent::Security`]).
    security: PageSecurity,
    /// Whether the page is making sound ([`WebEvent::Audible`]).
    audible: bool,
    /// Why the renderer went away, if it did ([`WebEvent::Crashed`]).
    /// Cleared by the next successful navigation.
    crashed: Option<SharedString>,
    /// The main frame's failed load, if the last one failed ([`WebEvent::LoadFailed`]).
    load_failed: Option<(SharedString, SharedString)>,
    /// The page's main-frame addresses, newest first, each once, with the
    /// title it last showed ([`Self::recent_pages`]).
    recent: Vec<RecentPage>,
    /// A navigation the backend has been asked for but not reported yet
    /// ([`Self::navigate`], back/forward/reload). The load end that follows
    /// one clears [`Self::crashed`]; a bare `Loading(false)` — the one the
    /// crash report itself carries — never does.
    navigation_pending: bool,
    /// The latest find-in-page result ([`WebEvent::Find`]).
    find: Option<FindState>,
    /// Set by [`Self::stop_find`]: late [`WebEvent::Find`] results from the
    /// stopped session are dropped until the next [`Self::find`] call.
    find_stopped: bool,
    /// The page zoom in percent; 100 is normal ([`WebEvent::Zoom`]).
    zoom: u16,
    /// The permission asks waiting for an answer ([`WebEvent::Permission`]),
    /// oldest first. Answered or dismissed ones are dropped.
    permissions: Vec<PermissionRequest>,
    /// Every download's latest state ([`WebEvent::Download`]), by id.
    downloads: Vec<DownloadState>,
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
    /// Keeps the escape interceptor alive; see [`Self::new`].
    _intercept: Subscription,
    /// The keyboard focus the pane takes.
    focus: FocusHandle,
    /// Set by the host while a gpui overlay covers the page; see
    /// [`Self::set_obscured`].
    obscured: bool,
    /// The native page's corner radius ([`Self::set_corner_radius`]): the
    /// snapshot stand-in rounds to it too, so covering the page never
    /// squares a rounded card's corners (owner 2026-10-11).
    corner_radius: f32,
    /// While set, the page is covered but its native view stays up until a
    /// picture taken NOW lands ([`Self::set_obscured`]), or until the
    /// deadline passes.
    cover_capture_until: Option<std::time::Instant>,
    /// The stand-in was taken at the moment of this cover: only then is it
    /// the page as the person last saw it (same scroll, same content).
    stand_in_fresh: bool,
    /// The last box pushed into a native backend, so an unchanged layout costs
    /// no `setFrame:`.
    pushed: Option<(Point<Pixels>, Size<Pixels>)>,
    /// How much of the page area survived its ancestors' clipping, or `None`
    /// when the pane has been scrolled out of sight entirely.
    visible_box: Option<Bounds<Pixels>>,
    /// Whether the host currently drives the pane's box through an animation
    /// (a spring, a slide); see [`Self::set_animating`].
    animating: bool,
    /// The latest frame laid out while [`Self::animating`] froze the native
    /// view, pushed exactly once when the animation ends.
    pending_frame: Option<(Point<Pixels>, Size<Pixels>)>,
    /// Whether any frozen layout resized the page, so the end-of-animation
    /// push refreshes the snapshot stand-in like a live push would.
    pending_resized: bool,
    /// The deferred frame seen at the last drain, with how many consecutive
    /// drains have seen it unchanged. Two identical drains (~120 ms of poll
    /// ticks) mean the spring ended without the host saying so; see
    /// [`Self::maybe_auto_settle`].
    settle_frame: Option<(Point<Pixels>, Size<Pixels>)>,
    settle_ticks: u32,
    /// Whether the page was hidden at the last drain. A hidden→visible edge
    /// notifies on the next drain, so an un-obscure without a `cx` still
    /// repaints; starts visible so a pane that begins hidden wakes nothing.
    was_hidden: bool,
    /// What the native view was last told about visibility; see
    /// [`Self::sync_visibility`].
    shown: bool,
    /// The page area's box, recorded during prepaint so pointer positions can
    /// be turned into page coordinates.
    page_origin: Point<Pixels>,
    page_size: Size<Pixels>,
    /// The poll timer; it stops when the state is dropped.
    _poll: Task<()>,
    /// The fast drain while a cover waits for its picture
    /// ([`Self::set_obscured`]); it ends itself once the cover settles.
    _cover_poll: Option<Task<()>>,
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
        // Escape has to reach the pane even when the host binds it at the
        // root context: a contextless binding matches at full depth and
        // outranks the input's own `escape → Escape` binding, so the host
        // action would swallow the keystroke and editing would stick. The
        // interceptor runs before any binding resolves: while the address
        // input holds the keyboard, a bare escape ends the edit and never
        // reaches the host. Anything else propagates untouched.
        let weak = cx.weak_entity();
        let intercept = cx.intercept_keystrokes(move |event, window, cx| {
            if event.keystroke.key == "escape" && event.keystroke.modifiers == Modifiers::none() {
                let ours = weak
                    .update(cx, |state: &mut Self, cx| {
                        let mine = state.editing
                            && state
                                .address
                                .as_ref()
                                .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window));
                        if mine {
                            state.leave_editing(cx);
                            window.focus(&state.focus, cx);
                        }
                        mine
                    })
                    .unwrap_or(false);
                if ours {
                    cx.stop_propagation();
                }
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
            favicon: None,
            progress: 0.0,
            security: PageSecurity::Unknown,
            audible: false,
            crashed: None,
            load_failed: None,
            recent: Vec::new(),
            navigation_pending: false,
            find: None,
            find_stopped: false,
            zoom: 100,
            permissions: Vec::new(),
            downloads: Vec::new(),
            screenshot_image: None,
            eval_results: Vec::new(),
            native,
            editing: false,
            address: None,
            _address_sub: None,
            _intercept: intercept,
            focus: cx.focus_handle(),
            obscured: false,
            corner_radius: 0.0,
            cover_capture_until: None,
            stand_in_fresh: false,
            pushed: None,
            visible_box: None,
            animating: false,
            pending_frame: None,
            pending_resized: false,
            settle_frame: None,
            settle_ticks: 0,
            was_hidden: false,
            shown: true,
            page_origin: gpui::point(px(0.0), px(0.0)),
            page_size: gpui::size(px(0.0), px(0.0)),
            _poll: poll,
            _cover_poll: None,
        };
        // A backend usually has its first page to announce before the poll
        // timer has run, so the pane never draws an empty URL field.
        let _ = state.apply_pending();
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

    /// Whether the page is loading (between the engine's load start and
    /// load end) — what a host's Stop/Reload button follows.
    pub fn loading(&self) -> bool {
        self.loading
    }

    /// The addresses this page has shown, newest first, each once, at most
    /// [`RECENT_PAGES`] (D6's "Recent pages" menu, docs/43 §2.7).
    pub fn recent_pages(&self) -> &[RecentPage] {
        &self.recent
    }

    /// The main frame's failed load — (address, engine reason) — until the
    /// next load starts.
    pub fn load_failed(&self) -> Option<(SharedString, SharedString)> {
        self.load_failed.clone()
    }

    /// The page's visible rectangle in window coordinates (its last
    /// layout), for a host that seats an overlay window over the page.
    pub fn page_rect(&self) -> Option<Bounds<Pixels>> {
        self.visible_box
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
    ///
    /// Turning it on takes the picture [`WebviewIntent::SendAnnotations`]
    /// carries: nothing else captures the page any more.
    pub fn set_annotate(&mut self, on: bool) {
        if on && !self.annotate {
            self.capture();
        }
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
        self.navigation_pending = true;
        self.backend.navigate(url);
    }

    /// Pins `points` (page coordinates) as clicks would, and saves each with
    /// the note beside it. This is how a card opens on a page that already has
    /// annotations on it, without a second code path for seeded pins.
    pub fn seed_pins(&mut self, points: &[((f32, f32), &str)]) {
        for (at, note) in points {
            self.backend.point_moved(Some(*at));
            self.backend.point_clicked(*at);
            let _ = self.apply_pending();
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
    ///
    /// Forgets the previous picture first, so a reader waiting on
    /// [`Self::screenshot`] gets this capture, never an older one.
    pub fn capture(&mut self) {
        self.screenshot = None;
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

    /// The page's icon URL, if the backend has reported one.
    pub fn favicon(&self) -> Option<SharedString> {
        self.favicon.clone()
    }

    /// How far the current load has come, 0.0–1.0.
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// What the site button says about the current page.
    pub fn security(&self) -> PageSecurity {
        self.security
    }

    /// Whether the page is currently making sound.
    pub fn audible(&self) -> bool {
        self.audible
    }

    /// Why the renderer went away, if it did. Cleared by the next
    /// successful navigation.
    pub fn crashed(&self) -> Option<SharedString> {
        self.crashed.clone()
    }

    /// The latest find-in-page result, if a search is running.
    pub fn find_state(&self) -> Option<FindState> {
        self.find.clone()
    }

    /// The page zoom in percent; 100 is normal.
    pub fn zoom(&self) -> u16 {
        self.zoom
    }

    /// Stops the current load, if the backend can.
    pub fn stop(&mut self) {
        self.backend.stop();
    }

    /// Sets the page zoom to `percent` (100 is normal).
    pub fn set_zoom(&mut self, percent: u16) {
        self.backend.set_zoom(percent);
    }

    /// Finds `query` in the page; the same query again moves to the next
    /// match (`forward`) or the previous one. Results arrive as
    /// [`WebEvent::Find`]. Re-opens the session [`Self::stop_find`] closed,
    /// so late results from the old session no longer land after this.
    pub fn find(&mut self, query: &str, forward: bool, match_case: bool) {
        self.find_stopped = false;
        self.backend.find(query, forward, match_case);
    }

    /// Ends the find session and clears the highlight. Late
    /// [`WebEvent::Find`] results still in flight are dropped until the next
    /// [`Self::find`] call.
    pub fn stop_find(&mut self) {
        self.find = None;
        self.find_stopped = true;
        self.backend.stop_find();
    }

    /// Opens the developer tools, if the backend has any.
    pub fn show_devtools(&mut self) {
        self.backend.show_devtools();
    }

    /// Whether the backend implements `feature`, so the host can hide the
    /// controls it cannot back.
    pub fn supports(&self, feature: BackendFeature) -> bool {
        self.backend.supports(feature)
    }

    /// The permission asks waiting for an answer, oldest first.
    pub fn permissions(&self) -> &[PermissionRequest] {
        &self.permissions
    }

    /// Every download's latest state, oldest first.
    pub fn downloads(&self) -> &[DownloadState] {
        &self.downloads
    }

    /// Answers the permission request `id` and drops it from
    /// [`Self::permissions`]. Unknown ids still reach the backend.
    pub fn answer_permission(&mut self, id: u64, decision: PermissionDecision, cx: &mut Context<Self>) {
        self.backend.answer_permission(id, decision);
        let before = self.permissions.len();
        self.permissions.retain(|request| request.id != id);
        if self.permissions.len() != before {
            cx.notify();
        }
    }

    /// Asks the backend to cancel the download `id`. Its end arrives as a
    /// [`WebEvent::Download`].
    pub fn cancel_download(&mut self, id: u32) {
        self.backend.cancel_download(id);
    }

    /// Takes the popup pages the backend queued, oldest first: each is
    /// `(url, foreground, page)`, for the host to wrap in a new tab.
    pub fn take_popup_backends(&mut self) -> Vec<(String, bool, Box<dyn WebBackend>)> {
        self.backend.take_popup_backends()
    }

    /// Takes the "open this URL" requests the backend queued, oldest first.
    pub fn take_open_requests(&mut self) -> Vec<(String, bool)> {
        self.backend.take_open_requests()
    }

    /// **Hides the native page so gpui can paint over its rectangle.**
    ///
    /// A `wry` child view is composited *above* the gpui scene, so a command
    /// palette, a menu or a note popover that overlaps the browser pane is
    /// simply not drawn. There is no compositing fix for that: the only way to
    /// put gpui pixels in that rectangle is to take the native view out of it.
    ///
    /// So the host calls this with `true` while such an overlay is open. The
    /// pane hides the native view and paints a picture taken for this cover
    /// in its place (a flat surface if none comes), which keeps the page
    /// looking like the page while the overlay is up. `false` puts it back.
    ///
    /// Covering asks for a fresh picture ([`WebBackend::capture_stand_in`])
    /// and keeps the live page up until it lands; [`Self::cover_settling`]
    /// says when that is over. While it waits, the backend is drained every
    /// [`COVER_POLL_INTERVAL`] instead of every [`POLL_INTERVAL`]: the slow
    /// poll alone added up to 100 ms to every cover.
    pub fn set_obscured(&mut self, obscured: bool, cx: &mut Context<Self>) {
        if self.obscured == obscured {
            return;
        }
        self.obscured = obscured;
        self.stand_in_fresh = false;
        self.cover_capture_until = None;
        if obscured {
            // The page is covered: a native view that kept first responder
            // would eat the keystrokes meant for whatever covers it.
            self.release_keyboard();
            // The stand-in must be the page as it is NOW — a picture from
            // load time showed the wrong scroll position and stale content
            // under every cover (owner recordings 2026-10-11). Take one, and
            // keep the native view up until it lands, so the swap is
            // invisible; a capture that never answers falls back to the flat
            // surface at the deadline, never to an old picture.
            if self.native && self.shown && !is_blank_url(&self.url) {
                self.backend.capture_stand_in();
                self.cover_capture_until = Some(std::time::Instant::now() + COVER_CAPTURE_WAIT);
                self._cover_poll = Some(cx.spawn(async move |this, cx| loop {
                    cx.background_executor().timer(COVER_POLL_INTERVAL).await;
                    let waiting = this
                        .update(cx, |this, cx| {
                            this.drain(cx);
                            this.cover_capture_until.is_some()
                        })
                        .unwrap_or(false);
                    if !waiting {
                        return;
                    }
                }));
            }
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

    /// Marks the pane as animating (`true`) or back at rest (`false`).
    ///
    /// # Contract
    ///
    /// While the pane's box is driven by an animation (the right column's
    /// spring, a slide), every frame reports a new box through layout.
    /// Pushing each intermediate width into a native webview re-lays the
    /// page out per spring frame, so the page lags the pane and burns a
    /// layout per tick. While this is set, the native frame stays frozen at
    /// its rest size: layouts keep recording the box — pointer mapping,
    /// clipping and [`Self::sync_visibility`] stay current — but the
    /// `set_bounds` push is deferred, and `set_animating(false)` applies
    /// the latest box exactly once.
    ///
    /// The host should call this with `false` when the animation settles,
    /// even if the box ended where it started; until then the page keeps
    /// its frozen size. That call is the fast path, not the only one: if the
    /// laid-out frame stops changing for ~120 ms of poll ticks the pane
    /// treats the spring as ended and applies the size itself, so a host
    /// that never calls back still converges. Combine with
    /// [`Self::set_obscured`] when the frozen frame itself would distract:
    /// the last screenshot stands in for the page while covered.
    pub fn set_animating(&mut self, animating: bool) {
        if self.animating == animating {
            return;
        }
        self.animating = animating;
        if animating {
            self.settle_frame = None;
            self.settle_ticks = 0;
        } else {
            let frame = self.pending_frame.take();
            let resized = std::mem::replace(&mut self.pending_resized, false);
            self.settle_frame = None;
            self.settle_ticks = 0;
            if self.native {
                if let Some(frame) = frame {
                    if self.pushed != Some(frame) {
                        self.push_frame(frame, resized);
                    }
                }
            }
            self.sync_visibility();
        }
    }

    /// Whether the host currently marks the pane as animating.
    pub fn animating(&self) -> bool {
        self.animating
    }

    /// Shows the native view only while the pane is both laid out on screen
    /// and not covered.
    ///
    /// There is deliberately no "not laid out recently" clause: a pane at
    /// rest draws no frames, so staleness cannot tell a settled pane from a
    /// discarded one, and the poll tick must not hide a visible page on that
    /// basis. Hosts hide the page explicitly — [`Self::set_obscured`] while
    /// an overlay covers it, dropping the state when the pane goes away
    /// (dropping parks the native view; see the `Drop` impl).
    fn sync_visibility(&mut self) {
        if !self.native {
            return;
        }
        // The first layout while animating defers its (tiny) frame instead
        // of pushing it, so the native view stays parked until the rest
        // frame lands: hidden is the snapshot stand-in, or nothing.
        let holding = self.cover_capture_until.is_some();
        let want = (!self.obscured || holding) && self.visible_box.is_some() && !(self.animating && self.pushed.is_none());
        if want != self.shown {
            self.shown = want;
            self.backend.set_visible(want);
        }
    }

    /// The page area was laid out at `bounds` (window coordinates), inside an
    /// ancestor clip of `clip`. A native backend is moved to match (unless
    /// [`Self::set_animating`] froze it); everything else only records the box.
    ///
    /// The two rectangles are different things. `bounds` is where the page's
    /// document is, and it is what a pointer position is turned into page
    /// coordinates against. `clip` is what gpui would have masked the pane to,
    /// and a native child view **is not subject to that mask** — it is clipped
    /// only by the window's content view. Without the intersection a pane in a
    /// scrolling container paints its page over the app's own chrome, which is
    /// exactly what the gallery's stage does when it scrolls.
    ///
    /// Recording the box is unconditional, so pointer mapping, clipping and
    /// visibility stay current on every frame — including frames laid out
    /// while [`Self::set_animating`] freezes the native view, when the push
    /// below is deferred and applied exactly once at the end.
    fn laid_out(&mut self, bounds: Bounds<Pixels>, clip: Bounds<Pixels>) -> bool {
        let (origin, size) = (bounds.origin, bounds.size);
        self.page_origin = origin;
        let resized = self.page_size != size;
        self.page_size = size;
        let visible = bounds.intersect(&clip);
        self.visible_box = (!visible.is_empty()).then_some(visible);
        let frame = (visible.origin, visible.size);
        if self.native {
            if self.pushed == Some(frame) {
                // At rest — including an animation that came back to the
                // already-pushed frame. Drop any stale deferral instead of
                // pushing it later over an up-to-date view.
                self.pending_frame = None;
                self.pending_resized = false;
                self.settle_frame = None;
                self.settle_ticks = 0;
            } else if self.animating {
                // Frozen mid-animation: the origin follows every frame so a
                // sliding pane tracks, while the size waits for rest. With
                // nothing pushed yet (the first layout mid-animation) the
                // tiny frame is only remembered; `sync_visibility` below
                // keeps the native view parked until rest.
                if let Some((pushed_origin, pushed_size)) = self.pushed {
                    if visible.origin != pushed_origin {
                        let (x, y) = (f32::from(visible.origin.x), f32::from(visible.origin.y));
                        let (w, h) = (f32::from(pushed_size.width), f32::from(pushed_size.height));
                        self.pushed = Some((visible.origin, pushed_size));
                        self.backend.set_bounds((x, y), (w, h));
                    }
                }
                self.pending_frame = Some(frame);
                self.pending_resized |= resized;
            } else {
                self.pending_frame = None;
                self.pending_resized = false;
                self.settle_frame = None;
                self.settle_ticks = 0;
                self.push_frame(frame, resized);
            }
        }
        self.sync_visibility();
        resized
    }

    /// The shown page's certificate, asked of the engine now (see
    /// [`WebBackend::certificate`](crate::backend::WebBackend::certificate)).
    pub fn certificate(&self) -> Option<crate::backend::CertificateInfo> {
        self.backend.certificate()
    }

    /// Rounds the native page's corners to `radius` (0 = square): a host that
    /// sets the page inside a rounded card passes the card's radius.
    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner_radius = radius;
        self.backend.set_corner_radius(radius);
    }

    /// Cuts rounded holes through the native page (window coordinates,
    /// logical pixels: x, y, w, h, radius) so host chrome drawn under the
    /// page — the closed shell's pills, the peek card — shows through and
    /// takes the clicks there. An empty list removes them.
    pub fn set_holes(&mut self, holes: Vec<[f32; 5]>) {
        self.backend.set_holes(holes);
    }

    /// Regions (window coordinates) where a pointer move over the page is
    /// also handed to the host window, so a hover-armed control under the
    /// page can arm; see [`WebBackend::set_hover_zones`].
    pub fn set_hover_zones(&mut self, zones: Vec<[f32; 4]>) {
        self.backend.set_hover_zones(zones);
    }

    /// Pushes `frame` into the native backend and remembers it, refreshing
    /// the snapshot stand-in when the page area itself resized.
    ///
    /// No picture is taken here. A capture per resize was the stand-in's old
    /// source: a sidebar drag asked Chromium for a full-window PNG every frame,
    /// encoded on the UI thread, and the window fell to ~11 fps (sampled
    /// 2026-10-11: 84 % of the main thread in the PNG writer). Covers take
    /// their own fresh picture ([`Self::set_obscured`]).
    fn push_frame(&mut self, frame: (Point<Pixels>, Size<Pixels>), _resized: bool) {
        self.pushed = Some(frame);
        self.backend.set_bounds(
            (f32::from(frame.0.x), f32::from(frame.0.y)),
            (f32::from(frame.1.width), f32::from(frame.1.height)),
        );
    }

    /// A picture landed while a cover waits for one: it stands in from now,
    /// and the native view goes after [`COVER_HANDOFF`]. A picture with no
    /// cover waiting is only kept (a later cover asks for its own).
    fn land_stand_in(&mut self, image: Image) {
        self.screenshot_image = Some(Arc::new(image));
        if self.cover_capture_until.is_some() && !self.stand_in_fresh {
            self.stand_in_fresh = true;
            self.cover_capture_until = Some(std::time::Instant::now() + COVER_HANDOFF);
        }
    }

    /// Whether a cover was asked for but the live page is still up, waiting
    /// on its picture (or the handoff after it). A host draws its covering
    /// UI only once this is false: drawn sooner, the part over the gpui
    /// ground shows while the page is still live above the rest — the ⌘K
    /// scrim dimmed the sidebar ~165 ms before the page (owner recording
    /// 2026-10-11).
    pub fn cover_settling(&self) -> bool {
        self.native && self.obscured && self.shown && self.cover_capture_until.is_some()
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
                // `Blur` ends the edit without navigating: it fires whenever
                // the input loses the keyboard to another focusable element.
                // Focus events dispatch when a frame renders, so a live
                // window that keeps painting always notices; a click on a
                // non-focusable surface moves no focus and ends nothing.
                let sub = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => this.commit_address(window, cx),
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
        // Again once the current event has finished dispatching: a click on
        // the URL display runs this from inside the press, and an ancestor's
        // own click handling (a host pane that takes focus when pressed)
        // lands after it and would leave the input without the keyboard —
        // the field showed the URL, but typing went nowhere (L5).
        let deferred = input.downgrade();
        let pane = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            // Only while still editing: an edit that ended in the same event
            // (Enter, Escape, a press elsewhere) must not pull the keyboard
            // back into a field that is about to unmount.
            let editing = pane.upgrade().is_some_and(|pane| pane.read(cx).editing);
            if let (true, Some(input)) = (editing, deferred.upgrade()) {
                input.update(cx, |input, cx| input.focus(window, cx));
            }
        });
        self.editing = true;
        cx.notify();
    }

    /// Commits the address field: the typed text becomes a URL and the page
    /// navigates to it. Runs on the input's enter event.
    fn commit_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.address.as_ref().map(|input| input.read(cx).value().to_string()).unwrap_or_default();
        self.editing = false;
        // The field unmounts next frame: park the keyboard on the pane so
        // ⌘L and Escape keep reaching it, not an element that is gone.
        window.focus(&self.focus, cx);
        self.navigate(&normalize_url(&typed));
        cx.notify();
    }

    /// Leaves the address field without navigating: the URL display falls
    /// back to the page URL on the next frame. Runs on input blur and on
    /// escape — through the escape interceptor in [`Self::new`], with the
    /// pane's propagated-escape handler as the fallback when no host
    /// binding competes for the keystroke.
    fn leave_editing(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            self.editing = false;
            cx.notify();
        }
    }

    /// Takes everything the backend has to say and folds it into the state,
    /// answering whether anything changed and whether a screenshot arrived.
    /// Draws nothing: [`Self::drain`] is what turns a change into a
    /// re-render.
    fn apply_pending(&mut self) -> (bool, bool, bool) {
        let url_before = self.url.clone();
        let events = self.backend.poll_events();
        let eval_results = self.backend.take_eval_results();
        let hovered = self.backend.hovered();
        let mut changed = hovered != self.hovered;
        let mut screenshot = false;
        self.hovered = hovered;
        for event in events {
            changed = true;
            match event {
                WebEvent::Title(title) => {
                    self.title = SharedString::from(title);
                    let (url, title) = (self.url.clone(), self.title.clone());
                    if let Some(page) = self.recent.iter_mut().find(|p| p.url == url) {
                        page.title = title;
                    }
                }
                WebEvent::Url(url) => {
                    // A main-frame address is a navigation, and a navigation
                    // is what clears the crashed tab — including a reload to
                    // the same URL, which still announces itself.
                    if url.as_str() != &*self.url || self.navigation_pending {
                        self.crashed = None;
                    }
                    self.navigation_pending = false;
                    // Typing in the address field survives the page announcing
                    // itself underneath it.
                    if !self.editing {
                        self.url = SharedString::from(url.clone());
                    }
                    note_recent(&mut self.recent, &url);
                }
                WebEvent::Loading(loading) => {
                    self.loading = loading;
                    if loading {
                        self.load_failed = None;
                    }
                    // Only the load end of a navigation we asked for clears
                    // the crash. The crash report itself arrives with a
                    // `Loading(false)` (a dead renderer never finishes), and
                    // that bare edge must leave the crashed tab up.
                    if !loading && self.navigation_pending {
                        self.crashed = None;
                        self.navigation_pending = false;
                    }
                }
                WebEvent::Annotation(annotation) => self.push_annotation(annotation),
                WebEvent::Screenshot(bytes) => {
                    self.land_stand_in(Image::from_bytes(ImageFormat::Png, bytes.clone()));
                    self.screenshot = Some(bytes);
                    screenshot = true;
                }
                WebEvent::StandIn(bytes) => {
                    let format = if bytes.starts_with(&[0xFF, 0xD8]) { ImageFormat::Jpeg } else { ImageFormat::Png };
                    self.land_stand_in(Image::from_bytes(format, bytes));
                    screenshot = true;
                }
                WebEvent::Favicon(url) => self.favicon = url.map(SharedString::from),
                WebEvent::Progress(progress) => self.progress = progress.clamp(0.0, 1.0),
                WebEvent::Security(security) => self.security = security,
                WebEvent::Audible(audible) => self.audible = audible,
                WebEvent::Crashed(reason) => self.crashed = Some(SharedString::from(reason)),
                WebEvent::LoadFailed { url, reason } => self.load_failed = Some((url.into(), reason.into())),
                WebEvent::Find(found) => {
                    // Late results from a stopped session never repopulate
                    // the strip; the next `find` call re-opens it.
                    if !self.find_stopped {
                        self.find = Some(found);
                    }
                }
                WebEvent::Zoom(percent) => self.zoom = percent,
                WebEvent::PopupOpened { .. } => {
                    // The host drains the page itself through
                    // `take_popup_backends`; the event only wakes the view.
                }
                WebEvent::Permission(request) => {
                    // A re-ask for the same prompt replaces the pending one.
                    if let Some(slot) = self.permissions.iter_mut().find(|pending| pending.id == request.id) {
                        *slot = request;
                    } else {
                        self.permissions.push(request);
                    }
                }
                WebEvent::PermissionDismissed(id) => {
                    self.permissions.retain(|pending| pending.id != id);
                }
                WebEvent::Download(state) => {
                    // One row per download: the latest state wins.
                    if let Some(slot) = self.downloads.iter_mut().find(|known| known.id == state.id) {
                        *slot = state;
                    } else {
                        self.downloads.push(state);
                    }
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
        (changed, screenshot, self.url != url_before)
    }

    /// The poll timer's tick: fold and, if anything moved, re-render.
    ///
    /// A hidden page — covered ([`Self::set_obscured`]) or laid out with no
    /// surviving box — notifies only for a fresh screenshot (which is what
    /// the snapshot stand-in paints) or for the hidden→visible edge itself,
    /// so an un-obscure without a `cx` still repaints on the next tick.
    /// Waking the window every 50 ms is what kept it re-rendering forever
    /// once any webview existed. A visible page notifies only when the fold
    /// changed something, never merely because the backend is native.
    fn drain(&mut self, cx: &mut Context<Self>) {
        let (changed, screenshot, url_moved) = self.apply_pending();
        // A cover's picture that never came: give up on it at the deadline.
        if self.cover_capture_until.is_some_and(|until| std::time::Instant::now() >= until) {
            self.cover_capture_until = None;
        }
        self.sync_visibility();
        self.maybe_auto_settle();
        let hidden = self.is_hidden();
        let became_visible = !hidden && self.was_hidden;
        self.was_hidden = hidden;
        // A hidden page still tells its host when its address moved: the
        // host decides whether to cover the page from the address (a blank
        // tab stays covered), so a page that committed its first navigation
        // while covered stayed dark until some unrelated event re-rendered
        // the host (seen live 2026-10-13: Browse dark at launch until the
        // pointer moved).
        if screenshot || became_visible || url_moved || (changed && !hidden) {
            cx.notify();
        }
    }

    /// Applies the deferred frame once the spring has plausibly ended: while
    /// animating, two consecutive drains that see the same deferred frame
    /// (~120 ms of poll ticks with no movement) push it like
    /// `set_animating(false)` would. A host that never ends its animation
    /// still converges; one that does takes the fast path there instead.
    fn maybe_auto_settle(&mut self) {
        if !self.animating || !self.native {
            self.settle_frame = None;
            self.settle_ticks = 0;
            return;
        }
        let Some(pending) = self.pending_frame else {
            self.settle_frame = None;
            self.settle_ticks = 0;
            return;
        };
        if self.settle_frame == Some(pending) {
            self.settle_ticks += 1;
        } else {
            self.settle_frame = Some(pending);
            self.settle_ticks = 1;
        }
        if self.settle_ticks >= 2 {
            self.settle_frame = None;
            self.settle_ticks = 0;
            let resized = std::mem::replace(&mut self.pending_resized, false);
            self.pending_frame = None;
            if self.pushed != Some(pending) {
                self.push_frame(pending, resized);
            }
            self.sync_visibility();
        }
    }

    /// Whether the page has nothing on screen worth a re-render: covered by a
    /// host overlay, or with no surviving laid-out box (scrolled away, or
    /// never laid out yet).
    fn is_hidden(&self) -> bool {
        self.obscured || self.visible_box.is_none()
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
                self.navigation_pending = true;
                self.backend.back();
                None
            }
            BrowserAction::Forward => {
                self.navigation_pending = true;
                self.backend.forward();
                None
            }
            BrowserAction::Reload => {
                self.navigation_pending = true;
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
    pub fn annotator_action(&mut self, action: AnnotatorAction, cx: &mut Context<Self>) -> Option<WebviewIntent> {
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
                // The page's own pins go too (the annotator drops them
                // when annotate mode cycles).
                if self.annotate {
                    self.backend.set_annotate(false);
                    self.backend.set_annotate(true);
                }
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
                pins: self.elements.iter().map(|info| info.as_ref().map(|info| (info.origin.0 + info.size.0, info.origin.1))).collect(),
                page_size: (f32::from(self.page_size.width), f32::from(self.page_size.height)),
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
        // The poll tick no longer hides abandoned panes (see
        // `sync_visibility`), so dropping the state parks the native view
        // itself. A no-op for element-backed pages.
        self.backend.set_visible(false);
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
    nav: bool,
    agent: SharedString,
}

/// The pane for `state`: the nav row, the page with the annotator's overlays,
/// and — only while annotate mode is on — the annotations panel.
pub fn webview_pane(id: impl Into<ElementId>, state: &Entity<WebviewState>) -> WebviewPane {
    WebviewPane { id: id.into(), state: state.clone(), on_intent: None, nav: true, agent: "the agent".into() }
}

impl WebviewPane {
    /// Leaves the nav row out: for hosts that draw their own toolbar for
    /// the page (Baaz's Browse mode). The page fills the pane.
    pub fn without_nav(mut self) -> Self {
        self.nav = false;
        self
    }

    /// The agent the notes go to ("Send 2 to Muse").
    pub fn agent(mut self, name: impl Into<SharedString>) -> Self {
        self.agent = name.into();
        self
    }

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
        let show_nav = self.nav;
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
        let corner_radius = px(view.corner_radius);
        let focus = view.focus.clone();
        // A blank page has nothing worth standing in for: its picture is
        // WebKit's white ground, which reads as a white slab over a dark pane.
        // Only a picture taken for this cover stands in; an older one would
        // show the page at another scroll position or with stale content.
        let stand_in = if is_blank_url(&view.url) || !view.stand_in_fresh { None } else { view.screenshot_image.clone() };
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
        // A press outside the address input ends the edit without
        // navigating: the slot's own out-handler covers presses inside the
        // pane, and the pane root's out-handler below covers presses out in
        // the host (a transcript click moves no focus when its target is
        // not focusable, so blur alone would miss it).
        let dismiss: Rc<dyn Fn(&mut App)> = {
            let state = state.clone();
            Rc::new(move |cx: &mut App| {
                state.update(cx, |state, cx| state.leave_editing(cx));
            })
        };
        // D6's 36 px nav row (docs/43 §2.7): the address at rest or the
        // editing input in the same Well frame.
        let address: gpui::AnyElement = match (view.editing, view.address.clone()) {
            (true, Some(input)) => aui::workbench::address_frame((id.clone(), "url"), cx)
                .debug_selector(|| "address-box".into())
                .on_mouse_down_out({
                    let dismiss = dismiss.clone();
                    move |_, _, cx| dismiss(cx)
                })
                .child(a11y_text_input((id.clone(), "address"), "Address", &input, cx).flex_1().min_w(px(0.0)).debug_selector(|| "address-slot".into()))
                .into_any_element(),
            _ => {
                record_ax_label("Address");
                let nav_handler = nav_handler.clone();
                aui::workbench::page_address((id.clone(), "url"), &view.url, cx)
                    .debug_selector(|| "address-display".into())
                    .on_click(move |_, window, cx| nav_handler(BrowserAction::FocusUrl, window, cx))
                    .into_any_element()
            }
        };
        let nav = {
            let nav_handler = nav_handler.clone();
            let handler = handler.clone();
            aui::workbench::page_nav_row(
                (id.clone(), "nav"),
                address,
                view.backend.can_go_back(),
                view.backend.can_go_forward(),
                annotate,
                f32::from(view.page_size.width),
                view.loading.then_some(0.6),
                move |action, window, cx| {
                    use aui::workbench::PageNavAction as A;
                    let mapped = match action {
                        A::Back => Some(BrowserAction::Back),
                        A::Forward => Some(BrowserAction::Forward),
                        A::Reload => Some(BrowserAction::Reload),
                        A::FocusAddress => Some(BrowserAction::FocusUrl),
                        A::Annotate => Some(BrowserAction::ToggleAnnotate),
                        A::Screenshot => Some(BrowserAction::Screenshot),
                        A::OpenInBrowse => None,
                    };
                    match mapped {
                        Some(a) => nav_handler(a, window, cx),
                        None => {
                            if let Some(h) = handler.clone() {
                                h(WebviewIntent::OpenInBrowse, window, cx);
                            }
                        }
                    }
                },
                cx,
            )
        };

        // The page area records its own box during prepaint, so a pointer
        // position can be turned into page coordinates without the state
        // having to guess the layout — and so a native page can be moved to
        // sit exactly on it.
        //
        // A native backend draws its own document over this rectangle, so the
        // rectangle itself is left as bare ground; only the scripted page is a
        // gpui document.
        // Rounded to the page's own radius: gpui's `overflow_hidden` clips to
        // a rectangle, so a square ground showed in the four corners around a
        // rounded page (owner 2026-10-13: "a slight weird border at all the 4
        // rounded edges").
        let base = if native {
            div().relative().flex_1().min_w(px(0.0)).overflow_hidden().rounded(corner_radius).bg(p.surface_2)
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
            let cover = div().absolute().inset_0().overflow_hidden().rounded(corner_radius).bg(p.surface_2);
            page = page.child(match stand_in {
                // Never stretched: the page's rect can change while it is
                // covered (the window resized, the sidebar toggled), and a
                // picture filled to the new box read as the page itself
                // squashed sideways (owner 2026-10-13). Cover keeps the
                // proportions; the live page replaces it once uncovered.
                Some(image) => cover.child(img(image).size_full().object_fit(gpui::ObjectFit::Cover).rounded(corner_radius)).into_any_element(),
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
                        .child(aui::workbench::annotate_outline(info.size.0, info.size.1)),
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
                        .child(aui::workbench::annotate_outline(info.size.0, info.size.1)),
                );
            }
            // D6: the pin's centre sits on the element's top-right corner.
            let _ = is_selected;
            page = page.child(div().absolute().left(px(info.origin.0 + info.size.0 - 12.0)).top(px(info.origin.1 - 12.0)).child(aui::workbench::annotate_pin(annotation.index)));
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

        // D6: the notes collect in a panel 8 in from the page's foot
        // (docs/43 §2.7), over the page.
        // Over a native page the host draws the panel in an overlay window
        // (the page window sits above everything gpui paints).
        if annotate && !native {
            let notes: Vec<aui::workbench::PageNote> = annotations
                .iter()
                .map(|a| aui::workbench::PageNote { index: a.index, selector: a.selector.clone(), note: a.note.clone() })
                .collect();
            let act = |action: AnnotatorAction| -> NotesClick {
                let state = state.clone();
                let handler = handler.clone();
                std::rc::Rc::new(move |_, window, cx| {
                    let intent = state.update(cx, |state, cx| state.annotator_action(action, cx));
                    if let (Some(intent), Some(handler)) = (intent, handler.clone()) {
                        handler(intent, window, cx);
                    }
                })
            };
            page = page.child(
                div().absolute().left(px(8.0)).right(px(8.0)).bottom(px(8.0)).child(aui::workbench::notes_panel(
                    (id.clone(), "notes"),
                    &notes,
                    &self.agent,
                    Some(act(AnnotatorAction::Clear)),
                    Some(act(AnnotatorAction::Send)),
                    cx,
                )),
            );
        }
        let _ = preview;

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
                move |_: &InputEscape, window: &mut Window, cx: &mut App| {
                    state.update(cx, |state, cx| {
                        if state.editing {
                            state.leave_editing(cx);
                            // As on Enter: the keyboard returns to the pane.
                            window.focus(&state.focus, cx);
                        } else {
                            cx.propagate();
                        }
                    });
                }
            })
            // A press outside the pane (a transcript click, a palette)
            // ends an open address edit without navigating; presses inside
            // the pane reach the slot's own out-handler instead.
            .on_mouse_down_out(move |_, _, cx| dismiss(cx))
            .size_full()
            .min_h(px(0.0))
            .bg(p.surface_1)
            .when(show_nav, |pane| pane.child(nav))
            .child(h_flex().flex_1().w_full().min_h(px(0.0)).items_stretch().child(page))
    }
}

/// A click on the notes panel's Clear or Send.
type NotesClick = std::rc::Rc<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)>;

/// Called with the [`BrowserAction`] a nav control stands for.
type NavHandler = Rc<dyn Fn(BrowserAction, &mut Window, &mut App)>;

/// Whether `url` is a page with no content of its own: before the first
/// load (empty) or `about:blank`.
fn is_blank_url(url: &str) -> bool {
    matches!(url.trim(), "" | "about:blank")
}

#[cfg(test)]
mod tests {
    /// Every `set_bounds` call a fake backend saw: (origin, size).
    type BoundsLog = Rc<RefCell<Vec<((f32, f32), (f32, f32))>>>;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::{is_blank_url, normalize_url};
    use super::{webview_pane, WebviewState};
    use crate::agent_js;
    use crate::backend::{BackendFeature, ElementInfo, FindState, PageSecurity, WebBackend, WebEvent};
    use crate::fake::FakeWebBackend;
    use gpui::{
        AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _,
        Render, Styled as _, TestAppContext, VisualTestContext, Window,
    };

    #[test]
    fn what_is_typed_in_the_url_field_becomes_a_url() {
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
        assert_eq!(normalize_url("  example.com/pricing "), "https://example.com/pricing");
        assert_eq!(normalize_url("localhost:3000/checkout"), "https://duckduckgo.com/?q=localhost%3A3000%2Fcheckout");
        assert_eq!(normalize_url("about:blank"), "about:blank");
        assert_eq!(normalize_url("file:///tmp/page.html"), "file:///tmp/page.html");
        assert_eq!(normalize_url("data:text/html,<b>hi</b>"), "data:text/html,<b>hi</b>");
        assert_eq!(normalize_url(""), "about:blank");
        assert!(is_blank_url("") && is_blank_url("about:blank") && !is_blank_url("https://example.com"));
    }

    #[test]
    fn a_phrase_is_a_search_rather_than_a_host() {
        assert_eq!(normalize_url("simple pricing page"), "https://duckduckgo.com/?q=simple+pricing+page");
        assert_eq!(normalize_url("worktrees"), "https://duckduckgo.com/?q=worktrees");
    }

    /// A page's address moving is reported even when nothing else is: the
    /// host decides whether to cover the page from the address, so a hidden
    /// page that commits its first navigation must still wake the host.
    #[gpui::test]
    fn a_moved_address_is_reported_to_the_host(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
            state.backend.navigate("https://news.ycombinator.com/");
            let (_, _, url_moved) = state.apply_pending();
            assert!(url_moved, "a new address is reported");
            let (_, _, url_moved) = state.apply_pending();
            assert!(!url_moved, "nothing new, nothing reported");
        });
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
            assert!(state.apply_pending().0);
        });
        let answers = state.update(cx, |state, _| state.take_eval_results());
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].0, 11);
        assert_eq!(answers[0].1, Ok(String::from("\"Simple pricing\"")));
        let drained: Vec<(u64, Result<String, String>)> = state.update(cx, |state, _| state.take_eval_results());
        assert!(drained.is_empty());
    }

    /// A [`WebBackend`] over a shared [`FakeWebBackend`], so the test can
    /// script events after the state has taken ownership of the backend.
    struct SharedFake(Rc<RefCell<FakeWebBackend>>);

    impl WebBackend for SharedFake {
        fn navigate(&mut self, url: &str) {
            self.0.borrow_mut().navigate(url);
        }
        fn back(&mut self) {
            self.0.borrow_mut().back();
        }
        fn forward(&mut self) {
            self.0.borrow_mut().forward();
        }
        fn reload(&mut self) {
            self.0.borrow_mut().reload();
        }
        fn eval(&mut self, js: &str) {
            self.0.borrow_mut().eval(js);
        }
        fn set_annotate(&mut self, on: bool) {
            self.0.borrow_mut().set_annotate(on);
        }
        fn poll_events(&mut self) -> Vec<WebEvent> {
            self.0.borrow_mut().poll_events()
        }
        fn take_eval_results(&mut self) -> Vec<(u64, Result<String, String>)> {
            self.0.borrow_mut().take_eval_results()
        }
        fn stop(&mut self) {
            self.0.borrow_mut().stop();
        }
        fn set_zoom(&mut self, percent: u16) {
            self.0.borrow_mut().set_zoom(percent);
        }
        fn find(&mut self, query: &str, forward: bool, match_case: bool) {
            self.0.borrow_mut().find(query, forward, match_case);
        }
        fn stop_find(&mut self) {
            self.0.borrow_mut().stop_find();
        }
        fn show_devtools(&mut self) {
            self.0.borrow_mut().show_devtools();
        }
        fn take_popup_backends(&mut self) -> Vec<(String, bool, Box<dyn WebBackend>)> {
            self.0.borrow_mut().take_popup_backends()
        }
        fn take_open_requests(&mut self) -> Vec<(String, bool)> {
            self.0.borrow_mut().take_open_requests()
        }
        fn answer_permission(&mut self, id: u64, decision: crate::backend::PermissionDecision) {
            self.0.borrow_mut().answer_permission(id, decision);
        }
        fn cancel_download(&mut self, id: u32) {
            self.0.borrow_mut().cancel_download(id);
        }
        fn supports(&self, feature: BackendFeature) -> bool {
            self.0.borrow().supports(feature)
        }
    }

    /// The state keeps the latest of each page-state event, forwards its
    /// commands to the backend, and clears the crashed tab on the next
    /// successful navigation.
    #[gpui::test]
    fn page_state_tracks_events_and_crash_clears_on_navigation(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        use gpui::SharedString;
        let fake = Rc::new(RefCell::new(FakeWebBackend::new()));
        let state = cx.new(|cx| WebviewState::new(Box::new(SharedFake(fake.clone())), cx));
        // Defaults before any page-state event arrives.
        state.update(cx, |state, _| {
            assert_eq!(state.favicon(), None);
            assert_eq!(state.progress(), 0.0);
            assert_eq!(state.security(), PageSecurity::Unknown);
            assert!(!state.audible());
            assert_eq!(state.crashed(), None);
            assert_eq!(state.find_state(), None);
            assert_eq!(state.zoom(), 100);
            assert!(state.supports(BackendFeature::Find));
        });
        fake.borrow_mut().script_favicon(Some("https://example.com/icon.png"));
        fake.borrow_mut().script_progress(0.5);
        fake.borrow_mut().script_security(PageSecurity::Secure);
        fake.borrow_mut().script_audible(true);
        fake.borrow_mut().script_crashed("the page crashed");
        fake.borrow_mut().script_find("price", 3, 12, true);
        fake.borrow_mut().script_zoom(125);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.favicon(), Some(SharedString::from("https://example.com/icon.png")));
            assert_eq!(state.progress(), 0.5);
            assert_eq!(state.security(), PageSecurity::Secure);
            assert!(state.audible());
            assert_eq!(state.crashed(), Some(SharedString::from("the page crashed")));
            assert_eq!(
                state.find_state(),
                Some(FindState { query: String::from("price"), active: 3, total: 12, final_update: true })
            );
            assert_eq!(state.zoom(), 125);
        });
        // Commands forward to the backend, where the fake records them.
        state.update(cx, |state, _| {
            state.stop();
            state.set_zoom(150);
            state.find("price", true, false);
            state.stop_find();
            state.show_devtools();
        });
        assert_eq!(fake.borrow().stop_calls(), 1);
        assert_eq!(fake.borrow().zoom_calls(), &[150]);
        assert_eq!(fake.borrow().find_calls(), &[(String::from("price"), true, false)]);
        assert_eq!(fake.borrow().stop_find_calls(), 1);
        assert_eq!(fake.borrow().devtools_calls(), 1);
        // The next successful navigation clears the crash but keeps the rest.
        state.update(cx, |state, _| state.navigate("https://example.com/"));
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.crashed(), None);
            assert_eq!(state.security(), PageSecurity::Secure);
            assert_eq!(state.zoom(), 125);
        });
    }

    /// The crash batch (`Crashed` + the dead renderer's `Loading(false)`)
    /// leaves the crashed tab up; only the next navigation clears it.
    #[gpui::test]
    fn crash_survives_its_own_load_edge_and_clears_on_navigation(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        use gpui::SharedString;
        let fake = Rc::new(RefCell::new(FakeWebBackend::new()));
        let state = cx.new(|cx| WebviewState::new(Box::new(SharedFake(fake.clone())), cx));
        fake.borrow_mut().script_crashed("the page crashed");
        fake.borrow_mut().script_loading(false);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.crashed(), Some(SharedString::from("the page crashed")));
        });
        // A following navigation clears it.
        state.update(cx, |state, _| state.navigate("https://example.com/"));
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.crashed(), None);
        });
    }

    /// `stop_find` clears the strip at once and swallows the stopped
    /// session's late results; the next `find` re-opens it.
    #[gpui::test]
    fn stop_find_clears_and_swallows_late_results(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let fake = Rc::new(RefCell::new(FakeWebBackend::new()));
        let state = cx.new(|cx| WebviewState::new(Box::new(SharedFake(fake.clone())), cx));
        state.update(cx, |state, _| state.find("price", true, false));
        fake.borrow_mut().script_find("price", 3, 12, true);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(
                state.find_state(),
                Some(FindState { query: String::from("price"), active: 3, total: 12, final_update: true })
            );
        });
        state.update(cx, |state, _| state.stop_find());
        state.update(cx, |state, _| {
            assert_eq!(state.find_state(), None);
        });
        // A late result from the stopped session lands on nothing.
        fake.borrow_mut().script_find("price", 4, 12, true);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.find_state(), None);
        });
        // The next search re-opens the strip.
        state.update(cx, |state, _| state.find("price", true, false));
        fake.borrow_mut().script_find("price", 5, 12, true);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(
                state.find_state(),
                Some(FindState { query: String::from("price"), active: 5, total: 12, final_update: true })
            );
        });
    }

    /// Zoom is per-origin: a `Zoom` arriving after a navigation is the new
    /// origin's level, and the state takes it.
    #[gpui::test]
    fn zoom_after_navigation_updates_the_state(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let fake = Rc::new(RefCell::new(FakeWebBackend::new()));
        let state = cx.new(|cx| WebviewState::new(Box::new(SharedFake(fake.clone())), cx));
        state.update(cx, |state, _| state.navigate("https://a.example/"));
        fake.borrow_mut().script_zoom(150);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.zoom(), 150);
        });
        // Another origin with its own level replaces it.
        state.update(cx, |state, _| state.navigate("https://b.example/"));
        fake.borrow_mut().script_zoom(100);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.zoom(), 100);
        });
    }

    /// Permissions queue until answered or dismissed, downloads keep their
    /// latest state by id, and popups drain through the passthroughs — with
    /// answers and cancels forwarded to the backend.
    #[gpui::test]
    fn permission_download_and_popup_bookkeeping(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;

        use crate::backend::{DownloadState, DownloadStatus, PermissionDecision, PermissionKind};
        let fake = Rc::new(RefCell::new(FakeWebBackend::new()));
        let state = cx.new(|cx| WebviewState::new(Box::new(SharedFake(fake.clone())), cx));
        state.update(cx, |state, _| {
            assert!(state.permissions().is_empty());
            assert!(state.downloads().is_empty());
        });
        fake.borrow_mut().script_permission(7, "https://example.com", vec![PermissionKind::Camera, PermissionKind::Microphone]);
        fake.borrow_mut().script_permission(8, "https://example.com", vec![PermissionKind::Geolocation]);
        downloading(&fake, 3, 10, DownloadStatus::Downloading);
        downloading(&fake, 4, 0, DownloadStatus::Downloading);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.permissions().len(), 2);
            assert_eq!(state.permissions()[0].id, 7);
            assert_eq!(state.permissions()[0].kinds, vec![PermissionKind::Camera, PermissionKind::Microphone]);
            assert_eq!(state.downloads().len(), 2);
            assert_eq!(state.downloads()[0].received, 10);
        });
        // A re-ask for the same prompt replaces it; progress replaces the row.
        fake.borrow_mut().script_permission(7, "https://example.com", vec![PermissionKind::Camera]);
        downloading(&fake, 3, 90, DownloadStatus::Downloading);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.permissions().len(), 2);
            assert_eq!(state.permissions()[0].kinds, vec![PermissionKind::Camera]);
            assert_eq!(state.downloads().len(), 2);
            assert_eq!(state.downloads()[0].received, 90);
        });
        // Answering drops the pending ask and reaches the backend …
        state.update(cx, |state, cx| state.answer_permission(7, PermissionDecision::Allow, cx));
        // … as does a dismissal from the page.
        fake.borrow_mut().script_permission_dismissed(8);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert!(state.permissions().is_empty());
        });
        assert_eq!(fake.borrow().answers(), &[(7, PermissionDecision::Allow)]);
        // Cancels forward; the end arrives as an event.
        state.update(cx, |state, _| state.cancel_download(3));
        assert_eq!(fake.borrow().cancelled_downloads(), &[3]);
        downloading(&fake, 3, 90, DownloadStatus::Cancelled);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        state.update(cx, |state, _| {
            assert_eq!(state.downloads().len(), 2);
            assert_eq!(state.downloads()[0].state, DownloadStatus::Cancelled);
        });
        // Popups and open requests drain through the state to the host.
        fake.borrow_mut().script_popup("https://example.com/pop", true);
        fake.borrow_mut().script_open_request("https://example.com/menu", false);
        state.update(cx, |state, _| {
            let _ = state.apply_pending();
        });
        let popups = state.update(cx, |state, _| state.take_popup_backends());
        assert_eq!(popups.len(), 1);
        assert_eq!((popups[0].0.as_str(), popups[0].1), ("https://example.com/pop", true));
        let requests = state.update(cx, |state, _| state.take_open_requests());
        assert_eq!(requests, vec![(String::from("https://example.com/menu"), false)]);
        assert!(state.update(cx, |state, _| state.take_popup_backends()).is_empty());

        fn downloading(fake: &Rc<RefCell<FakeWebBackend>>, id: u32, received: u64, status: DownloadStatus) {
            fake.borrow_mut().script_download(DownloadState {
                id,
                url: String::from("https://example.com/f.zip"),
                file_name: String::from("f.zip"),
                path: String::from("/tmp/f.zip"),
                received,
                total: Some(100),
                state: status,
            });
        }
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
        state.update(cx, |state, cx| {
            assert!(state.holds_keyboard());
            state.set_obscured(true, cx);
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
        let (host, vcx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
            PaneHost { state }
        });
        let state = vcx.update(|_, cx| host.read(cx).state.clone());
        f(state, vcx);
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
            let pane_focused = vcx.update(|window, cx| state.read(cx).focus_handle().is_focused(window));
            assert!(pane_focused, "the keyboard returns to the pane, not the unmounted field");
            vcx.simulate_keystrokes("cmd-l");
            assert!(vcx.update(|_, cx| state.read(cx).is_editing()), "⌘L opens the field again after an escape");
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

    // L4: a host action standing in for Baaz's root-context `escape` binding.
    gpui::actions!(l4_test_host, [HostCancel]);

    /// L4: a host that notices when a root `escape → HostCancel` binding
    /// dispatches while the pane is open.
    struct CancelHost {
        state: Entity<WebviewState>,
        cancelled: Rc<std::cell::Cell<bool>>,
    }

    impl Render for CancelHost {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let flag = self.cancelled.clone();
            gpui::div()
                .id("l4-cancel-host")
                .on_action(move |_: &HostCancel, _, _| flag.set(true))
                .child(webview_pane("test-pane", &self.state))
        }
    }

    /// L4: a host with a second focusable element beside the pane: the pane
    /// gets a fixed height so the element below it is laid out and hittable.
    struct BlurHost {
        state: Entity<WebviewState>,
        other: FocusHandle,
    }

    impl Render for BlurHost {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            gpui_kit::base::v_flex()
                .child(gpui::div().h(gpui::px(400.)).child(webview_pane("test-pane", &self.state)))
                .child(
                    gpui::div()
                        .id("l4-other")
                        .h(gpui::px(40.))
                        .track_focus(&self.other)
                        .debug_selector(|| "l4-other".into())
                        .child("other"),
                )
        }
    }

    /// L4: while editing, the input fills the address slot — its laid-out
    /// text bounds span the slot's content width at a 360 px pane — and it
    /// holds the page URL.
    #[gpui::test]
    fn editing_input_fills_the_address_slot(cx: &mut TestAppContext) {
        with_pane(cx, |state, vcx| {
            vcx.simulate_resize(gpui::size(gpui::px(360.), gpui::px(600.)));
            let home = vcx.update(|_, cx| state.read(cx).url().to_string());
            vcx.update(|window, cx| state.update(cx, |state, cx| state.begin_editing(window, cx)));
            vcx.update(|window, cx| {
                let _ = window.draw(cx);
            });
            let slot_width = vcx.debug_bounds("address-slot").map(|bounds| f32::from(bounds.size.width)).unwrap_or(-1.0);
            let (value, text_width) = vcx.update(|_, cx| {
                let input = state.read(cx).address_input().expect("begin_editing creates the input");
                let snapshot = input.read(cx);
                let width = snapshot.text_bounds().map(|bounds| f32::from(bounds.size.width)).unwrap_or(-1.0);
                (snapshot.value().to_string(), width)
            });
            assert_eq!(value, home, "the field shows the page URL while editing");
            assert!(
                slot_width > 0.0 && text_width >= slot_width - 22.0 && text_width <= slot_width + 1.0,
                "the input fills the slot: text width {text_width} vs slot width {slot_width}"
            );
        });
    }

    /// L4: escape while editing leaves editing even when the host binds
    /// `escape` at the root context (no predicate outranks the input's own
    /// Escape binding), and the host action does not fire for that keystroke.
    #[gpui::test]
    fn escape_leaves_editing_despite_a_host_root_binding(cx: &mut TestAppContext) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        let (host, vcx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
            CancelHost { state, cancelled: Rc::new(std::cell::Cell::new(false)) }
        });
        let state = vcx.update(|_, cx| host.read(cx).state.clone());
        let home = vcx.update(|_, cx| state.read(cx).url().to_string());
        vcx.update(|_, cx| cx.bind_keys([gpui::KeyBinding::new("escape", HostCancel, None)]));
        vcx.update(|window, cx| state.update(cx, |state, cx| state.begin_editing(window, cx)));
        vcx.simulate_keystrokes("escape");
        let (url, editing) = vcx.update(|_, cx| (state.read(cx).url().to_string(), state.read(cx).is_editing()));
        let cancelled = vcx.update(|_, cx| host.read(cx).cancelled.get());
        assert_eq!(url, home, "escape restores the URL instead of navigating");
        assert!(!editing, "escape leaves editing even with a host root escape binding");
        assert!(!cancelled, "the host Cancel action must not fire for the editing escape");
    }

    /// L4: pressing on another focusable element in the same window — which
    /// moves the keyboard there — leaves editing without navigating. (A bare
    /// focus move between two painted elements changes no focus path, so no
    /// focus event or input blur ever fires for it; the press itself is what
    /// the pane listens for.)
    #[gpui::test]
    fn focusing_elsewhere_leaves_editing_without_navigating(cx: &mut TestAppContext) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        let (host, vcx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
            let other = cx.focus_handle();
            BlurHost { state, other }
        });
        let state = vcx.update(|_, cx| host.read(cx).state.clone());
        let other = vcx.update(|_, cx| host.read(cx).other.clone());
        let home = vcx.update(|_, cx| state.read(cx).url().to_string());
        vcx.update(|window, cx| state.update(cx, |state, cx| state.begin_editing(window, cx)));
        vcx.update(|window, cx| {
            state
                .read(cx)
                .address_input()
                .unwrap()
                .update(cx, |input, cx| input.replace("example.com", window, cx));
        });
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let center = vcx.debug_bounds("l4-other").expect("the other element is painted").center();
        vcx.simulate_click(center, gpui::Modifiers::none());
        vcx.run_until_parked();
        let (url, editing, moved) = vcx.update(|window, cx| {
            (state.read(cx).url().to_string(), state.read(cx).is_editing(), other.is_focused(window))
        });
        assert!(moved, "the press moves the keyboard to the other element");
        assert_eq!(url, home, "leaving the field must not navigate");
        assert!(!editing, "pressing outside the input leaves editing");
    }

    /// L4: a press in the address box's own padding — outside the input
    /// element but inside the rounded field — keeps editing.
    #[gpui::test]
    fn a_press_in_the_field_padding_keeps_editing(cx: &mut TestAppContext) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        let (host, vcx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
            let other = cx.focus_handle();
            BlurHost { state, other }
        });
        let state = vcx.update(|_, cx| host.read(cx).state.clone());
        vcx.update(|window, cx| state.update(cx, |state, cx| state.begin_editing(window, cx)));
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let slot = vcx.debug_bounds("address-box").expect("the address box is painted");
        let padding = gpui::point(slot.origin.x + gpui::px(2.0), slot.center().y);
        vcx.simulate_click(padding, gpui::Modifiers::none());
        vcx.run_until_parked();
        assert!(vcx.update(|_, cx| state.read(cx).is_editing()), "a press inside the field box keeps editing");
    }

    /// L5: the host a real-input test renders in, shaped like Baaz: a root
    /// `escape → HostCancel` binding with no context, and the pane inside a
    /// focusable ancestor that activates (focuses) itself on click, the way
    /// a pane container takes selection when anything inside it is pressed.
    use gpui::{Focusable as _, StatefulInteractiveElement as _};

    struct L5Host {
        state: Entity<WebviewState>,
        wrap: FocusHandle,
        cancelled: Rc<std::cell::Cell<bool>>,
    }

    impl Render for L5Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let flag = self.cancelled.clone();
            let wrap = self.wrap.clone();
            gpui::div()
                .id("l5-host")
                .on_action(move |_: &HostCancel, _, _| flag.set(true))
                .child(
                    gpui::div()
                        .id("l5-wrap")
                        .track_focus(&self.wrap)
                        .on_click(move |_, window, cx| window.focus(&wrap, cx))
                        .child(webview_pane("test-pane", &self.state)),
                )
        }
    }

    /// L5: runs `f` with a pane in the Baaz-shaped host: theme and input
    /// bindings installed, the root escape binding bound, one frame drawn.
    fn with_l5_host(
        cx: &mut TestAppContext,
        f: impl FnOnce(Entity<WebviewState>, Rc<std::cell::Cell<bool>>, &mut VisualTestContext),
    ) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        let (host, vcx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
            let wrap = cx.focus_handle();
            L5Host { state, wrap, cancelled: Rc::new(std::cell::Cell::new(false)) }
        });
        vcx.update(|_, cx| cx.bind_keys([gpui::KeyBinding::new("escape", HostCancel, None)]));
        let (state, cancelled) = vcx.update(|_, cx| (host.read(cx).state.clone(), host.read(cx).cancelled.clone()));
        f(state, cancelled, vcx);
    }

    /// L5: clicks the painted URL display the way a mouse does — through the
    /// real mouse path, never through `begin_editing` directly.
    fn click_address_display(vcx: &mut VisualTestContext) {
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let center = vcx.debug_bounds("address-display").expect("the URL display is painted").center();
        vcx.simulate_click(center, gpui::Modifiers::none());
        vcx.run_until_parked();
    }

    /// L5 (E2): a mouse click on the URL display enters editing AND leaves
    /// the keyboard in the field, the way `⌘L` does — typed text replaces
    /// the selected URL instead of going nowhere.
    #[gpui::test]
    fn clicking_the_url_display_leaves_the_keyboard_in_the_field(cx: &mut TestAppContext) {
        with_l5_host(cx, |state, _cancelled, vcx| {
            click_address_display(vcx);
            assert!(vcx.update(|_, cx| state.read(cx).is_editing()), "clicking the URL display enters editing");
            let input_focused = vcx.update(|window, cx| {
                state
                    .read(cx)
                    .address_input()
                    .expect("the click creates the input")
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            });
            assert!(input_focused, "the click path ends with the input focused, the way ⌘L does");
            vcx.simulate_keystrokes("a b c");
            let typed = vcx.update(|_, cx| state.read(cx).address_input().unwrap().read(cx).value().to_string());
            assert_eq!(typed, "abc", "typed text replaces the selected URL instead of going nowhere");
        });
    }

    /// L5 (E1): escape after a click-driven edit ends the edit, brings the
    /// URL display back, and never reaches the host's root binding.
    #[gpui::test]
    fn escape_after_a_click_leaves_editing_without_firing_the_host_action(cx: &mut TestAppContext) {
        with_l5_host(cx, |state, cancelled, vcx| {
            let home = vcx.update(|_, cx| state.read(cx).url().to_string());
            click_address_display(vcx);
            assert!(vcx.update(|_, cx| state.read(cx).is_editing()), "clicking the URL display enters editing");
            vcx.simulate_keystrokes("a b c");
            vcx.simulate_keystrokes("escape");
            let (url, editing) = vcx.update(|_, cx| (state.read(cx).url().to_string(), state.read(cx).is_editing()));
            let fired = vcx.update(|_, _| cancelled.get());
            assert!(!editing, "escape leaves editing even with a host root escape binding");
            assert_eq!(url, home, "escape restores the URL instead of navigating");
            assert!(!fired, "the host action must not fire for the editing escape");
            vcx.update(|window, cx| {
                let _ = window.draw(cx);
            });
            assert!(
                vcx.debug_bounds("address-display").is_some(),
                "the URL display is back after escape leaves editing"
            );
        });
    }

    /// L4: a settled 360 px pane drops the `⌘L` keycap so the host/path keeps
    /// room; a settled 500 px pane keeps it (~24 URL characters still fit).
    #[gpui::test]
    fn narrow_pane_drops_back_and_forward(cx: &mut TestAppContext) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        aui::data::arm_ax_probe(true);
        aui::data::take_ax_labels();
        let (_host, vcx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| WebviewState::new(Box::new(FakeWebBackend::new()), cx));
            PaneHost { state }
        });
        // Two draws per width: the first frame's prepaint measures the page
        // width the resize produced, and only the second frame renders with
        // it. Labels are drained between the draws so each assertion sees a
        // single settled frame: the probe accumulates across renders, and
        // the first frame still carries the previous width.
        vcx.simulate_resize(gpui::size(gpui::px(360.), gpui::px(600.)));
        aui::data::take_ax_labels();
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        aui::data::take_ax_labels();
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let narrow_first = aui::data::take_ax_labels();
        assert!(narrow_first.iter().any(|label| label == "Back"), "back stays at 360 px, got {narrow_first:?}");
        vcx.simulate_resize(gpui::size(gpui::px(500.), gpui::px(600.)));
        aui::data::take_ax_labels();
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        aui::data::take_ax_labels();
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let medium = aui::data::take_ax_labels();
        assert!(medium.iter().any(|label| label == "Forward"), "forward stays at 500 px, got {medium:?}");
        vcx.simulate_resize(gpui::size(gpui::px(280.), gpui::px(600.)));
        aui::data::take_ax_labels();
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        aui::data::take_ax_labels();
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let narrow = aui::data::take_ax_labels();
        assert!(!narrow.iter().any(|label| label == "Back" || label == "Forward"), "back / forward leave under 300 px (docs/43 §2.7), got {narrow:?}");
        aui::data::arm_ax_probe(false);
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
        for want in ["Back", "Forward", "Reload", "Address", "Annotate", "Screenshot to the composer", "Open in Browse"] {
            assert!(labels.iter().any(|label| label == want), "missing AX label {want:?} in {labels:?}");
        }
        assert!(!labels.iter().any(String::is_empty), "an empty label is an unnamed control in {labels:?}");
    }

    /// A [`FakeWebBackend`] that reports itself as native, records every
    /// `set_bounds` and `set_visible`, and drains caller-injected events
    /// ahead of the scripted ones — so the pane tests can assert
    /// follow-the-pane behaviour without a real webview.
    struct NativeFake {
        inner: FakeWebBackend,
        set_bounds_calls: BoundsLog,
        set_visible_calls: Rc<RefCell<Vec<bool>>>,
        extra_events: Rc<RefCell<Vec<WebEvent>>>,
        captures: Rc<Cell<u32>>,
        stand_ins: Rc<Cell<u32>>,
    }

    impl NativeFake {
        fn new(
            set_bounds_calls: BoundsLog,
            set_visible_calls: Rc<RefCell<Vec<bool>>>,
            extra_events: Rc<RefCell<Vec<WebEvent>>>,
        ) -> Self {
            Self {
                inner: FakeWebBackend::new(),
                set_bounds_calls,
                set_visible_calls,
                extra_events,
                captures: Rc::new(Cell::new(0)),
                stand_ins: Rc::new(Cell::new(0)),
            }
        }
    }

    impl WebBackend for NativeFake {
        fn navigate(&mut self, url: &str) {
            self.inner.navigate(url);
        }
        fn back(&mut self) {
            self.inner.back();
        }
        fn forward(&mut self) {
            self.inner.forward();
        }
        fn reload(&mut self) {
            self.inner.reload();
        }
        fn eval(&mut self, js: &str) {
            self.inner.eval(js);
        }
        fn eval_with_result(&mut self, request_id: u64, js: &str) {
            self.inner.eval_with_result(request_id, js);
        }
        fn take_eval_results(&mut self) -> Vec<(u64, Result<String, String>)> {
            self.inner.take_eval_results()
        }
        fn set_annotate(&mut self, on: bool) {
            self.inner.set_annotate(on);
        }
        fn capture(&mut self) {
            self.captures.set(self.captures.get() + 1);
        }
        fn capture_stand_in(&mut self) {
            self.stand_ins.set(self.stand_ins.get() + 1);
        }
        fn poll_events(&mut self) -> Vec<WebEvent> {
            let mut events = self.extra_events.borrow_mut().drain(..).collect::<Vec<_>>();
            events.extend(self.inner.poll_events());
            events
        }
        fn can_go_back(&self) -> bool {
            self.inner.can_go_back()
        }
        fn can_go_forward(&self) -> bool {
            self.inner.can_go_forward()
        }
        fn is_native(&self) -> bool {
            true
        }
        fn set_bounds(&mut self, origin: (f32, f32), size: (f32, f32)) {
            self.set_bounds_calls.borrow_mut().push((origin, size));
        }
        fn set_visible(&mut self, visible: bool) {
            self.set_visible_calls.borrow_mut().push(visible);
            self.inner.set_visible(visible);
        }
        fn set_focused(&mut self, focused: bool) {
            self.inner.set_focused(focused);
        }
        fn holds_keyboard(&self) -> bool {
            self.inner.holds_keyboard()
        }
        fn point_moved(&mut self, at: Option<(f32, f32)>) {
            self.inner.point_moved(at);
        }
        fn point_clicked(&mut self, at: (f32, f32)) {
            self.inner.point_clicked(at);
        }
        fn hovered(&self) -> Option<ElementInfo> {
            self.inner.hovered()
        }
        fn element_info(&self, index: usize) -> Option<ElementInfo> {
            self.inner.element_info(index)
        }
    }

    /// A hidden native webview never poll-notifies: ten idle ticks wake the
    /// window zero times, even though the backend is native (the old
    /// `changed || native` woke it on every tick).
    #[gpui::test]
    fn a_hidden_webview_does_not_poll_notify_while_idle(cx: &mut TestAppContext) {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let state = cx.new(|cx| {
            WebviewState::new(
                Box::new(NativeFake::new(calls, Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())))),
                cx,
            )
        });
        // `new` already folded the opening announcements; covering the page
        // is what hides it — no layout has run, so there is no box either.
        state.update(cx, |state, cx| state.set_obscured(true, cx));
        let count = Rc::new(Cell::new(0usize));
        // Kept alive to the end of the test: dropping it stops the count.
        let _keep = cx.update(|cx| {
            let count = count.clone();
            cx.observe(&state, move |_, _| count.set(count.get() + 1))
        });
        for _ in 0..10 {
            state.update(cx, |state, cx| state.drain(cx));
        }
        assert_eq!(count.get(), 0, "a hidden webview with no changes must not wake the window");
    }

    /// A visible native webview at rest does not notify either: nothing
    /// changed, so no re-render, even though the backend is native.
    #[gpui::test]
    fn a_visible_native_webview_does_not_notify_without_changes(cx: &mut TestAppContext) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        let calls = Rc::new(RefCell::new(Vec::new()));
        let (host, vcx) = cx.add_window_view({
            let calls = calls.clone();
            move |_window, cx| {
                let state = cx.new(|cx| {
                    WebviewState::new(
                        Box::new(NativeFake::new(
                            calls,
                            Rc::new(RefCell::new(Vec::new())),
                            Rc::new(RefCell::new(Vec::new())),
                        )),
                        cx,
                    )
                });
                PaneHost { state }
            }
        });
        let state = vcx.update(|_, cx| host.read(cx).state.clone());
        // One draw lays the page out, so the view counts as visible.
        vcx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        assert!(vcx.update(|_, cx| !state.read(cx).obscured()));
        let count = Rc::new(Cell::new(0usize));
        let _keep = vcx.update(|_, cx| {
            let count = count.clone();
            cx.observe(&state, move |_, _| count.set(count.get() + 1))
        });
        for _ in 0..10 {
            vcx.update(|_, cx| state.update(cx, |state, cx| state.drain(cx)));
        }
        assert_eq!(count.get(), 0, "an unchanged visible webview must not wake the window merely for being native");
    }

    /// While the host marks the view as animating, intermediate spring
    /// frames never reach the native view; the rest frame lands exactly
    /// once when the animation ends.
    #[gpui::test]
    fn animating_defers_bounds_until_the_end(cx: &mut TestAppContext) {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let state = cx.new(|cx| {
            WebviewState::new(
                Box::new(NativeFake::new(
                    calls.clone(),
                    Rc::new(RefCell::new(Vec::new())),
                    Rc::new(RefCell::new(Vec::new())),
                )),
                cx,
            )
        });
        let frame = |width: f32| gpui::Bounds {
            origin: gpui::point(gpui::px(0.), gpui::px(0.)),
            size: gpui::size(gpui::px(width), gpui::px(600.)),
        };
        let wide = || frame(1000.);
        state.update(cx, |state, _| {
            state.laid_out(frame(400.), wide());
        });
        assert_eq!(calls.borrow().len(), 1, "the rest frame is pushed");
        state.update(cx, |state, _| state.set_animating(true));
        assert!(state.update(cx, |state, _| state.animating()));
        for width in [380., 350., 320., 305., 300.] {
            state.update(cx, |state, _| {
                state.laid_out(frame(width), wide());
            });
        }
        assert_eq!(calls.borrow().len(), 1, "intermediate spring frames must not reach the native view");
        state.update(cx, |state, _| state.set_animating(false));
        assert_eq!(calls.borrow().len(), 2, "the rest frame lands exactly once");
        assert_eq!(calls.borrow().last(), Some(&((0., 0.), (300., 600.))));
        // Settling again with no movement pushes nothing further.
        state.update(cx, |state, _| state.set_animating(true));
        state.update(cx, |state, _| {
            state.laid_out(frame(300.), wide());
        });
        state.update(cx, |state, _| state.set_animating(false));
        assert_eq!(calls.borrow().len(), 2, "an animation that moved nothing pushes nothing");
    }

    /// L5fix: a native-backed state plus its `set_bounds` / `set_visible`
    /// records and its injectable event queue.
    #[allow(clippy::type_complexity)]
    fn native_state(
        cx: &mut TestAppContext,
    ) -> (
        Entity<WebviewState>,
        BoundsLog,
        Rc<RefCell<Vec<bool>>>,
        Rc<RefCell<Vec<WebEvent>>>,
    ) {
        let bounds = Rc::new(RefCell::new(Vec::new()));
        let visible = Rc::new(RefCell::new(Vec::new()));
        let events: Rc<RefCell<Vec<WebEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let state = cx.new(|cx| {
            WebviewState::new(Box::new(NativeFake::new(bounds.clone(), visible.clone(), events.clone())), cx)
        });
        (state, bounds, visible, events)
    }

    /// A native-backed state with counters for `capture` and
    /// `capture_stand_in` calls, plus its `set_visible` log and event queue.
    #[allow(clippy::type_complexity)]
    fn counting_state(
        cx: &mut TestAppContext,
    ) -> (Entity<WebviewState>, Rc<Cell<u32>>, Rc<Cell<u32>>, Rc<RefCell<Vec<bool>>>, Rc<RefCell<Vec<WebEvent>>>) {
        let visible = Rc::new(RefCell::new(Vec::new()));
        let events: Rc<RefCell<Vec<WebEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let fake = NativeFake::new(Rc::new(RefCell::new(Vec::new())), visible.clone(), events.clone());
        let (captures, stand_ins) = (fake.captures.clone(), fake.stand_ins.clone());
        let state = cx.new(|cx| WebviewState::new(Box::new(fake), cx));
        (state, captures, stand_ins, visible, events)
    }

    /// L5fix: the laid-out page box at `x` with `width` (600 px tall).
    fn frame_at(x: f32, width: f32) -> gpui::Bounds<gpui::Pixels> {
        gpui::Bounds {
            origin: gpui::point(gpui::px(x), gpui::px(0.)),
            size: gpui::size(gpui::px(width), gpui::px(600.)),
        }
    }

    /// L5fix: an ancestor clip that contains every test frame.
    fn wide_clip() -> gpui::Bounds<gpui::Pixels> {
        gpui::Bounds {
            origin: gpui::point(gpui::px(0.), gpui::px(0.)),
            size: gpui::size(gpui::px(1000.), gpui::px(600.)),
        }
    }

    /// L5fix: lays the page out at `bounds` inside a wide clip, without a window.
    fn lay_out(state: &Entity<WebviewState>, cx: &mut TestAppContext, bounds: gpui::Bounds<gpui::Pixels>) {
        state.update(cx, |state, _| {
            state.laid_out(bounds, wide_clip());
        });
    }

    /// L5fix: runs one poll tick, without a window.
    fn tick(state: &Entity<WebviewState>, cx: &mut TestAppContext) {
        state.update(cx, |state, cx| state.drain(cx));
    }

    /// L5fix: counts the state's notifications; the guard must stay alive.
    fn notify_count(state: &Entity<WebviewState>, cx: &mut TestAppContext) -> (Rc<Cell<usize>>, gpui::Subscription) {
        let count = Rc::new(Cell::new(0usize));
        let keep = cx.update(|cx| {
            let count = count.clone();
            cx.observe(state, move |_, _| count.set(count.get() + 1))
        });
        (count, keep)
    }

    /// L5fix: an animation that returns to the already-pushed rest frame (a
    /// reversed spring, a quick toggle) drops its deferral: ending the
    /// animation pushes nothing stale over the up-to-date view.
    #[gpui::test]
    fn reversal_to_the_pushed_frame_pushes_nothing_stale(cx: &mut TestAppContext) {
        let (state, bounds, _, _) = native_state(cx);
        lay_out(&state, cx, frame_at(0., 400.));
        assert_eq!(bounds.borrow().len(), 1, "the rest frame is pushed");
        state.update(cx, |state, _| state.set_animating(true));
        for width in [350., 320.] {
            lay_out(&state, cx, frame_at(0., width));
        }
        lay_out(&state, cx, frame_at(0., 400.));
        state.update(cx, |state, _| state.set_animating(false));
        assert_eq!(
            bounds.borrow().len(),
            1,
            "reversing to the pushed frame must not push a stale intermediate frame"
        );
    }

    /// L5fix: while animating, the native origin tracks every frame while the
    /// frozen size is pushed exactly once, at rest.
    #[gpui::test]
    fn origin_follows_while_size_waits_for_rest(cx: &mut TestAppContext) {
        let (state, bounds, _, _) = native_state(cx);
        lay_out(&state, cx, frame_at(0., 400.));
        state.update(cx, |state, _| state.set_animating(true));
        lay_out(&state, cx, frame_at(20., 350.));
        lay_out(&state, cx, frame_at(40., 300.));
        assert_eq!(
            bounds.borrow().as_slice(),
            &[((0., 0.), (400., 600.)), ((20., 0.), (400., 600.)), ((40., 0.), (400., 600.))]
        );
        state.update(cx, |state, _| state.set_animating(false));
        assert_eq!(bounds.borrow().len(), 4, "the rest size lands exactly once");
        assert_eq!(bounds.borrow().last(), Some(&((40., 0.), (300., 600.))));
    }

    /// D6 recent pages: newest first, each address once with its last
    /// title, blank pages left out, capped.
    #[gpui::test]
    fn recent_pages_keep_each_address_once_newest_first(cx: &mut TestAppContext) {
        let (state, _, _, events) = native_state(cx);
        for (url, title) in [("https://a.example/", "A"), ("about:blank", ""), ("https://b.example/", "B"), ("https://a.example/", "A again")] {
            events.borrow_mut().push(WebEvent::Url(url.to_owned()));
            events.borrow_mut().push(WebEvent::Title(title.to_owned()));
            tick(&state, cx);
        }
        let recent: Vec<(String, String)> = state.update(cx, |s, _| s.recent_pages().iter().map(|p| (p.url.to_string(), p.title.to_string())).collect());
        // The fixture's own first page sits after these two.
        assert_eq!(recent[..2], [("https://a.example/".into(), "A again".into()), ("https://b.example/".into(), "B".into())]);
        assert!(!recent.iter().any(|(u, _)| u == "about:blank"));
        for n in 0..12 {
            events.borrow_mut().push(WebEvent::Url(format!("https://p{n}.example/")));
        }
        tick(&state, cx);
        assert_eq!(state.update(cx, |s, _| s.recent_pages().len()), super::RECENT_PAGES);
    }

    /// L5fix: a screenshot that arrives while covered is exactly what the
    /// snapshot stand-in paints, so it notifies even though hidden.
    #[gpui::test]
    fn screenshot_while_obscured_still_notifies(cx: &mut TestAppContext) {
        let (state, _, _, events) = native_state(cx);
        lay_out(&state, cx, frame_at(0., 400.));
        state.update(cx, |state, cx| state.set_obscured(true, cx));
        let (count, _keep) = notify_count(&state, cx);
        tick(&state, cx);
        assert_eq!(count.get(), 0, "a hidden page with no news stays quiet");
        events.borrow_mut().push(WebEvent::Screenshot(vec![1, 2, 3, 4]));
        tick(&state, cx);
        assert_eq!(count.get(), 1, "a screenshot must repaint even while obscured");
        assert!(state.update(cx, |state, _| state.screenshot().is_some()));
    }

    /// Owner recordings 2026-10-11: a cover showed a picture from load time
    /// (wrong scroll, stale content). Covering now asks for a picture of the
    /// page as it is and keeps the live page up until it lands — then one
    /// handoff tick more — so the swap is invisible.
    #[gpui::test]
    fn a_cover_keeps_the_live_page_until_its_fresh_picture_lands(cx: &mut TestAppContext) {
        let (state, _, visible, events) = native_state(cx);
        events.borrow_mut().push(WebEvent::Url("https://a.example/".to_owned()));
        tick(&state, cx);
        lay_out(&state, cx, frame_at(0., 400.));
        visible.borrow_mut().clear();
        state.update(cx, |state, cx| state.set_obscured(true, cx));
        tick(&state, cx);
        assert!(!visible.borrow().contains(&false), "the live page stays up while its picture is taken");
        assert!(state.update(cx, |state, _| !state.stand_in_fresh), "no fresh picture yet");
        events.borrow_mut().push(WebEvent::Screenshot(vec![1, 2, 3, 4]));
        tick(&state, cx);
        assert!(state.update(cx, |state, _| state.stand_in_fresh), "the cover's picture landed");
        assert!(!visible.borrow().contains(&false), "one handoff tick so the picture paints first");
        std::thread::sleep(super::COVER_HANDOFF + std::time::Duration::from_millis(10));
        tick(&state, cx);
        assert_eq!(visible.borrow().last(), Some(&false), "then the native view goes");
    }

    /// A picture that never comes falls back to the flat surface at the
    /// deadline — never to an older picture.
    #[gpui::test]
    fn a_cover_without_its_picture_gives_up_at_the_deadline(cx: &mut TestAppContext) {
        let (state, _, visible, events) = native_state(cx);
        events.borrow_mut().push(WebEvent::Screenshot(vec![9, 9]));
        events.borrow_mut().push(WebEvent::Url("https://a.example/".to_owned()));
        tick(&state, cx);
        lay_out(&state, cx, frame_at(0., 400.));
        state.update(cx, |state, cx| state.set_obscured(true, cx));
        std::thread::sleep(super::COVER_CAPTURE_WAIT + std::time::Duration::from_millis(10));
        tick(&state, cx);
        assert_eq!(visible.borrow().last(), Some(&false), "hidden at the deadline");
        assert!(state.update(cx, |state, _| !state.stand_in_fresh), "the load-time picture never stands in");
    }

    /// Owner recording 2026-10-11 (sidebar resize lag): resizing the page asks
    /// the backend for no picture at all.
    #[gpui::test]
    fn a_resize_asks_for_no_capture(cx: &mut TestAppContext) {
        let (state, captures, stand_ins, _, _) = counting_state(cx);
        lay_out(&state, cx, frame_at(0., 400.));
        lay_out(&state, cx, frame_at(0., 300.));
        tick(&state, cx);
        assert_eq!((captures.get(), stand_ins.get()), (0, 0));
    }

    /// Owner recording 2026-10-11 (PNG writer on the main thread): a load end
    /// asks the backend for no picture.
    #[gpui::test]
    fn a_load_end_asks_for_no_capture(cx: &mut TestAppContext) {
        let (state, captures, stand_ins, _, events) = counting_state(cx);
        lay_out(&state, cx, frame_at(0., 400.));
        events.borrow_mut().push(WebEvent::Loading(true));
        tick(&state, cx);
        events.borrow_mut().push(WebEvent::Loading(false));
        tick(&state, cx);
        assert_eq!((captures.get(), stand_ins.get()), (0, 0));
    }

    /// Owner recording 2026-10-11 (slow covers): covering a shown page takes
    /// exactly one cover-only picture, not a full capture.
    #[gpui::test]
    fn covering_takes_one_stand_in_and_no_capture(cx: &mut TestAppContext) {
        let (state, captures, stand_ins, _, events) = counting_state(cx);
        events.borrow_mut().push(WebEvent::Url("https://a.example/".to_owned()));
        tick(&state, cx);
        lay_out(&state, cx, frame_at(0., 400.));
        state.update(cx, |state, cx| state.set_obscured(true, cx));
        tick(&state, cx);
        assert_eq!((captures.get(), stand_ins.get()), (0, 1));
    }

    /// Owner recording 2026-10-11 (cover picture): a stand-in landing hands the
    /// page off to the cover without becoming the readable screenshot.
    #[gpui::test]
    fn a_stand_in_covers_without_becoming_the_screenshot(cx: &mut TestAppContext) {
        let (state, _, _, visible, events) = counting_state(cx);
        events.borrow_mut().push(WebEvent::Url("https://a.example/".to_owned()));
        tick(&state, cx);
        lay_out(&state, cx, frame_at(0., 400.));
        visible.borrow_mut().clear();
        state.update(cx, |state, cx| state.set_obscured(true, cx));
        events.borrow_mut().push(WebEvent::StandIn(vec![1, 2, 3, 4]));
        tick(&state, cx);
        assert!(state.update(cx, |state, _| state.stand_in_fresh), "the cover's picture landed");
        assert!(state.update(cx, |state, _| state.screenshot().is_none()), "not the annotate screenshot");
        assert!(!visible.borrow().contains(&false), "one handoff tick first");
        std::thread::sleep(super::COVER_HANDOFF + std::time::Duration::from_millis(10));
        tick(&state, cx);
        assert_eq!(visible.borrow().last(), Some(&false), "then the native view goes");
    }

    /// Owner recording 2026-10-11 (scrim dimmed the sidebar before the page):
    /// `cover_settling` holds through the picture wait and the handoff only.
    #[gpui::test]
    fn cover_settling_spans_the_wait_and_the_handoff(cx: &mut TestAppContext) {
        let (state, _, _, events) = native_state(cx);
        events.borrow_mut().push(WebEvent::Url("https://a.example/".to_owned()));
        tick(&state, cx);
        lay_out(&state, cx, frame_at(0., 400.));
        assert!(!state.update(cx, |state, _| state.cover_settling()), "not covered");
        state.update(cx, |state, cx| state.set_obscured(true, cx));
        assert!(state.update(cx, |state, _| state.cover_settling()), "waiting on the picture");
        events.borrow_mut().push(WebEvent::StandIn(vec![1, 2, 3, 4]));
        tick(&state, cx);
        assert!(state.update(cx, |state, _| state.cover_settling()), "during the handoff");
        std::thread::sleep(super::COVER_HANDOFF + std::time::Duration::from_millis(10));
        tick(&state, cx);
        assert!(!state.update(cx, |state, _| state.cover_settling()), "settled");
    }

    /// Owner recording 2026-10-11 (annotate sent an older picture): `capture`
    /// forgets the previous screenshot before asking for the next.
    #[gpui::test]
    fn capture_forgets_an_older_screenshot(cx: &mut TestAppContext) {
        let (state, _, _, events) = native_state(cx);
        events.borrow_mut().push(WebEvent::Screenshot(vec![1]));
        tick(&state, cx);
        assert!(state.update(cx, |state, _| state.screenshot().is_some()));
        state.update(cx, |state, _| state.capture());
        assert!(state.update(cx, |state, _| state.screenshot().is_none()));
        events.borrow_mut().push(WebEvent::Screenshot(vec![2]));
        tick(&state, cx);
        assert_eq!(state.update(cx, |state, _| state.screenshot().map(<[u8]>::to_vec)), Some(vec![2]));
    }

    /// Owner recording 2026-10-11 (PNG writer on the main thread): turning
    /// annotate on captures once; turning it on again while on does not.
    #[gpui::test]
    fn annotate_captures_only_on_the_off_to_on_edge(cx: &mut TestAppContext) {
        let (state, captures, _, _, _) = counting_state(cx);
        state.update(cx, |state, _| state.set_annotate(true));
        assert_eq!(captures.get(), 1);
        state.update(cx, |state, _| state.set_annotate(true));
        assert_eq!(captures.get(), 1, "already on: no second capture");
    }

    /// L5fix: un-obscuring has no `cx`, so the next drain repaints — once.
    #[gpui::test]
    fn hidden_to_visible_notifies_once(cx: &mut TestAppContext) {
        let (state, _, _, _) = native_state(cx);
        lay_out(&state, cx, frame_at(0., 400.));
        state.update(cx, |state, cx| state.set_obscured(true, cx));
        let (count, _keep) = notify_count(&state, cx);
        tick(&state, cx);
        assert_eq!(count.get(), 0, "covering the page notifies nothing by itself");
        state.update(cx, |state, cx| state.set_obscured(false, cx));
        tick(&state, cx);
        assert_eq!(count.get(), 1, "the un-obscure repaints on the next drain");
        tick(&state, cx);
        assert_eq!(count.get(), 1, "the transition notifies exactly once");
    }

    /// L5fix: a visible title, URL or loading change repaints.
    #[gpui::test]
    fn visible_title_url_and_loading_notify(cx: &mut TestAppContext) {
        let (state, _, _, events) = native_state(cx);
        lay_out(&state, cx, frame_at(0., 400.));
        let (count, _keep) = notify_count(&state, cx);
        tick(&state, cx);
        assert_eq!(count.get(), 0, "the baseline tick is quiet");
        events.borrow_mut().push(WebEvent::Title(String::from("New title")));
        tick(&state, cx);
        assert_eq!(count.get(), 1, "a visible title change notifies");
        events.borrow_mut().push(WebEvent::Url(String::from("https://example.com/next")));
        tick(&state, cx);
        assert_eq!(count.get(), 2, "a visible URL change notifies");
        events.borrow_mut().push(WebEvent::Loading(true));
        tick(&state, cx);
        assert_eq!(count.get(), 3, "a visible loading change notifies");
    }

    /// L5fix: when the laid-out frame stops changing mid-animation, two poll
    /// ticks apply the deferred size even though the host never ends it.
    #[gpui::test]
    fn auto_settle_applies_size_without_set_animating_false(cx: &mut TestAppContext) {
        let (state, bounds, _, _) = native_state(cx);
        lay_out(&state, cx, frame_at(0., 400.));
        assert_eq!(bounds.borrow().len(), 1, "the rest frame is pushed");
        state.update(cx, |state, _| state.set_animating(true));
        for width in [350., 320., 300.] {
            lay_out(&state, cx, frame_at(0., width));
        }
        assert_eq!(bounds.borrow().len(), 1, "intermediate frames stay deferred");
        tick(&state, cx);
        assert_eq!(bounds.borrow().len(), 1, "one tick is not yet settled");
        tick(&state, cx);
        assert_eq!(bounds.borrow().len(), 2, "two still ticks apply the size");
        assert_eq!(bounds.borrow().last(), Some(&((0., 0.), (300., 600.))));
        assert!(state.update(cx, |state, _| state.animating()), "the host never ended the animation");
        tick(&state, cx);
        tick(&state, cx);
        assert_eq!(bounds.borrow().len(), 2, "settling pushes exactly once");
    }

    /// L5fix: the first layout while animating defers its tiny frame instead
    /// of pushing it; the native view stays parked until rest.
    #[gpui::test]
    fn first_layout_while_animating_pushes_nothing(cx: &mut TestAppContext) {
        let (state, bounds, visible, _) = native_state(cx);
        state.update(cx, |state, _| state.set_animating(true));
        lay_out(&state, cx, frame_at(0., 50.));
        assert!(bounds.borrow().is_empty(), "the tiny first frame must not be pushed");
        assert_eq!(visible.borrow().as_slice(), &[false], "the native view stays parked until rest");
        state.update(cx, |state, _| state.set_animating(false));
        assert_eq!(bounds.borrow().as_slice(), &[((0., 0.), (50., 600.))], "rest applies the frame once");
        assert_eq!(visible.borrow().as_slice(), &[false, true], "rest shows the native view again");
    }
}
