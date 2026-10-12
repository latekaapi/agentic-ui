//! The backend contract: what the pane needs from "a page", whether that page
//! is the scripted mock ([`crate::fake`]) or a real WKWebView
//! (`crate::wry_backend`).
//!
//! The pane never talks to a browser directly. It pushes commands down
//! ([`WebBackend::navigate`], [`WebBackend::eval`], …) and pulls
//! [`WebEvent`]s back up on a timer ([`WebBackend::poll_events`]), so the
//! gpui side is the same code for the mock and for the real thing.
//!
//! # Where the element metadata lives
//!
//! [`WebEvent::Annotation`] carries only the `aui` [`Annotation`] the side
//! panel renders — the number, the selector and the note. The rest of what an
//! annotation is sent with (the bounding box, the trimmed `outerHTML`, the
//! source file) is bulky, is not part of the panel's data, and for the real
//! backend arrives on the same IPC message. So the backend keeps it in a **side
//! table** and the pane asks for it by annotation number through
//! [`WebBackend::element_info`]. That keeps [`WebEvent`] small and lets a
//! backend that knows nothing about source maps simply return `None`.

use std::cell::RefCell;

use aui::workbench::Annotation;
use gpui::SharedString;

std::thread_local! {
    /// The fallback queue behind the default [`WebBackend::eval_with_result`]
    /// and [`WebBackend::take_eval_results`].
    ///
    /// A trait cannot hold state, so a backend that supports none of this
    /// answers through one shared per-thread queue: each `eval_with_result`
    /// pushes its `Err("not supported")` here, and `take_eval_results` drains
    /// it. Backends with real answers (the mock, the real page) override both
    /// methods with their own storage and never touch this.
    static DEFAULT_EVAL_RESULTS: RefCell<Vec<(u64, Result<String, String>)>> = const { RefCell::new(Vec::new()) };
}

/// One element of the page, as the annotator sees it.
///
/// The pane uses [`ElementInfo::origin`] and [`ElementInfo::size`] to draw the
/// hover outline and to drop the pin, and the rest to label the note popover
/// and to describe the element to the agent.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementInfo {
    /// The CSS path of the element (`div.card.starter`).
    pub selector: SharedString,
    /// The tag shown in the outline's label (`p`, `h1`, `div`).
    pub label: SharedString,
    /// The element's top-left corner, in page coordinates.
    pub origin: (f32, f32),
    /// The element's box, in page pixels.
    pub size: (f32, f32),
    /// Where the element comes from when a dev server is mapped
    /// (`pricing.tsx:42`); empty when it is not.
    pub source: SharedString,
    /// The element's `outerHTML`, trimmed to [`OUTER_HTML_LIMIT`] bytes.
    pub outer_html: SharedString,
}

/// How much `outerHTML` an annotation carries. Two kilobytes is enough for the
/// element and its immediate children and small enough to cross the IPC bridge
/// on every click without a copy anyone notices.
pub const OUTER_HTML_LIMIT: usize = 2048;

impl ElementInfo {
    /// The outline's label: `p · 392 × 34`.
    pub fn outline_label(&self) -> SharedString {
        SharedString::from(format!("{} · {} × {}", self.label, self.size.0.round(), self.size.1.round()))
    }

    /// The note popover's path line: `div.card.starter · 150 × 118 · pricing.tsx:42`.
    pub fn path(&self) -> SharedString {
        let box_ = format!("{} · {} × {}", self.selector, self.size.0.round(), self.size.1.round());
        if self.source.is_empty() {
            SharedString::from(box_)
        } else {
            SharedString::from(format!("{box_} · {}", self.source))
        }
    }

    /// Whether `point` (page coordinates) is inside the element's box.
    pub fn contains(&self, point: (f32, f32)) -> bool {
        point.0 >= self.origin.0
            && point.1 >= self.origin.1
            && point.0 <= self.origin.0 + self.size.0
            && point.1 <= self.origin.1 + self.size.1
    }
}

/// Something the page told the pane about itself.
#[derive(Debug, Clone, PartialEq)]
pub enum WebEvent {
    /// The document title changed.
    Title(String),
    /// The page navigated; the pane shows this in the URL field.
    Url(String),
    /// Loading started (`true`) or finished (`false`).
    Loading(bool),
    /// The person pinned an element while annotate mode was on. The metadata
    /// that goes with it is fetched with [`WebBackend::element_info`].
    Annotation(Annotation),
    /// An encoded PNG of the page, answering [`WebBackend::capture`].
    Screenshot(Vec<u8>),
    /// A picture of the page for a cover's stand-in only, answering
    /// [`WebBackend::capture_stand_in`]: any format gpui decodes (JPEG for
    /// speed), never handed to anyone as "the screenshot".
    StandIn(Vec<u8>),
    /// The page's icon changed; the first icon URL, or `None` when the page
    /// has none. The toolbar shows this on the tab.
    Favicon(Option<String>),
    /// How far the main-frame load has come, 0.0–1.0. The toolbar draws this
    /// as its 2 px progress line while [`WebEvent::Loading`] is on.
    Progress(f32),
    /// What the address row's site button should say about the current page.
    Security(PageSecurity),
    /// Whether the page is currently making sound. Tabs show an audio
    /// indicator while this is `true`.
    Audible(bool),
    /// The renderer went away; the reason reads like "the page crashed".
    /// Cleared by the next successful navigation. [`WebBackend::reload`]
    /// recovers the page.
    Crashed(String),
    /// A find-in-page result for the query [`WebBackend::find`] started.
    /// `active` is the 1-based match under the cursor (0 while searching).
    Find(FindState),
    /// The page's zoom, in percent (100 is normal).
    Zoom(u16),
    /// The page opened a popup. The host makes a tab for it out of
    /// [`WebBackend::take_popup_backends`]: `foreground` asks for the tab to
    /// be selected (`window.open` into a foreground tab), otherwise it opens
    /// in the background.
    PopupOpened {
        /// Where the popup points.
        url: String,
        /// Whether the popup asked for the foreground tab.
        foreground: bool,
    },
    /// The page asked for one or more permissions (camera, microphone, …).
    /// The host shows its own UI and answers through
    /// [`WebBackend::answer_permission`].
    Permission(PermissionRequest),
    /// A permission prompt went away without an answer (navigated, closed,
    /// dismissed by the engine). Drops the matching [`WebEvent::Permission`].
    PermissionDismissed(u64),
    /// A download started or moved: the latest state for `id`. The host
    /// renders its own downloads UI and cancels through
    /// [`WebBackend::cancel_download`].
    Download(DownloadState),
}

/// One permission the page can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionKind {
    /// Camera access (video capture, pan/tilt/zoom).
    Camera,
    /// Microphone access (audio capture).
    Microphone,
    /// The page's geolocation.
    Geolocation,
    /// Showing notifications.
    Notifications,
    /// Reading or writing the clipboard.
    Clipboard,
    /// MIDI system-exclusive messages.
    Midi,
    /// Any other engine permission bit, as its raw bit value (a single
    /// `1 << n`, never a combination).
    Other(u32),
}

/// A page's permission ask, as [`WebEvent::Permission`] carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    /// What [`WebBackend::answer_permission`] answers, and what
    /// [`WebEvent::PermissionDismissed`] drops.
    pub id: u64,
    /// The origin asking (`https://example.com`).
    pub origin: String,
    /// What it asked for, in a fixed order (camera, microphone, geolocation,
    /// notifications, clipboard, MIDI, then the rest by bit).
    pub kinds: Vec<PermissionKind>,
}

/// The host's answer to a [`PermissionRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    /// Grant what was asked.
    Allow,
    /// Refuse it.
    Deny,
    /// Close the prompt without deciding.
    Dismiss,
}

/// Where a download stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    /// Bytes are still arriving.
    Downloading,
    /// The file landed whole.
    Complete,
    /// The host or the person cancelled it.
    Cancelled,
    /// It stopped some other way (interrupted, failed).
    Failed,
}

/// The latest state of one download, as [`WebEvent::Download`] carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadState {
    /// The engine's id for the download; [`WebBackend::cancel_download`]
    /// takes it, and the pane keys its list on it.
    pub id: u32,
    /// Where the bytes come from.
    pub url: String,
    /// The file name under the download folder.
    pub file_name: String,
    /// The full path the bytes are written to.
    pub path: String,
    /// How many bytes have arrived.
    pub received: u64,
    /// How many are expected, when the server said.
    pub total: Option<u64>,
    /// Where the download stands.
    pub state: DownloadStatus,
}

/// The shown page's TLS certificate and connection, for a site-info panel
/// like Chrome's "Connection is secure › Certificate is valid".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CertificateInfo {
    /// Issued to: the subject's common name (usually the host).
    pub subject: String,
    /// The subject's organisation, when the certificate names one.
    pub subject_org: Option<String>,
    /// Issued by: the issuer's common name.
    pub issuer: String,
    /// The issuer's organisation.
    pub issuer_org: Option<String>,
    /// Valid from, as Unix seconds.
    pub valid_from: i64,
    /// Valid until, as Unix seconds.
    pub valid_to: i64,
    /// The SHA-256 fingerprint of the DER certificate, upper-case hex.
    pub sha256: String,
    /// The protocol, such as "TLS 1.3", when known.
    pub protocol: Option<String>,
}

/// What the address row's site button says about the current page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSecurity {
    /// An `https` page whose connection is secure.
    Secure,
    /// Plain `http`, or `https` whose certificate failed.
    NotSecure,
    /// A loopback or development host: `localhost`, `127.0.0.1`, `[::1]`,
    /// `*.localhost`, `*.test`.
    Local,
    /// A `file://` URL.
    File,
    /// The browser's own pages: `chrome://`, `devtools://`, `about:`.
    Internal,
    /// Anything else: unknown schemes, unparseable URLs.
    Unknown,
}

impl PageSecurity {
    /// Classifies `url` given what the transport said: `Some(true)` when the
    /// visible navigation entry reports a secure connection, `Some(false)`
    /// on a certificate error, `None` when there is no SSL status (plain
    /// `http`, files, internal pages).
    ///
    /// Classifies `url` given what the transport said: `Some(true)` when the
    /// visible navigation entry reports a secure connection, `Some(false)`
    /// on a certificate error, `None` when there is no SSL status (plain
    /// `http`, files, internal pages).
    ///
    /// A certificate error wins over everything: `https://localhost` with
    /// `Some(false)` is [`PageSecurity::NotSecure`], not
    /// [`PageSecurity::Local`]. Otherwise the host wins over the scheme:
    /// `http://localhost:5173` is [`PageSecurity::Local`], and so is an
    /// `https` loopback page with no reported error. `https` without evidence
    /// to the contrary is [`PageSecurity::Secure`].
    pub fn classify(url: &str, secure_connection: Option<bool>) -> PageSecurity {
        let url = url.trim();
        let Some((raw_scheme, rest)) = url.split_once(':') else {
            return PageSecurity::Unknown;
        };
        if !is_url_scheme(raw_scheme) {
            return PageSecurity::Unknown;
        }
        match raw_scheme.to_ascii_lowercase().as_str() {
            "chrome" | "devtools" | "about" => PageSecurity::Internal,
            "file" => PageSecurity::File,
            "http" | "ws" => {
                if is_local_host(&host_of(rest)) {
                    PageSecurity::Local
                } else {
                    PageSecurity::NotSecure
                }
            }
            "https" | "wss" => {
                // A certificate error is a broken page, whatever the host.
                if secure_connection == Some(false) {
                    PageSecurity::NotSecure
                } else if is_local_host(&host_of(rest)) {
                    PageSecurity::Local
                } else {
                    PageSecurity::Secure
                }
            }
            _ => PageSecurity::Unknown,
        }
    }
}

/// Whether `text` reads as a URL scheme: leading letter, then letters,
/// digits, `+`, `-` or `.`.
fn is_url_scheme(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => (),
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// The host of the `//authority/path` that follows a URL's scheme, lowercased
/// and without brackets, userinfo or port. A backslash ends the authority
/// like a slash does (WHATWG).
fn host_of(after_scheme: &str) -> String {
    let authority = after_scheme.strip_prefix("//").unwrap_or(after_scheme).split(['/', '\\', '?', '#']).next().unwrap_or_default();
    // `rsplit` yields the text after the last `@`, or the whole authority.
    let authority = authority.rsplit('@').next().unwrap_or_default();
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else if authority.matches(':').count() >= 2 {
        // A bare IPv6 literal (`::1`): the colons are the address, not a port.
        authority
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
            _ => authority,
        }
    };
    host.to_ascii_lowercase()
}

/// Whether `host` (already lowercased, no port) is a loopback or development
/// host. A trailing dot is a fully-qualified name (`localhost.`), still local.
fn is_local_host(host: &str) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host);
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host == "test"
        || host.ends_with(".test")
}

/// One find-in-page result, as the find strip shows it ("3 of 12").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindState {
    /// What was searched for.
    pub query: String,
    /// The 1-based match under the cursor (0 while the search runs).
    pub active: u32,
    /// How many matches the page holds so far.
    pub total: u32,
    /// Whether this is the last update for the current search.
    pub final_update: bool,
}

/// A page control a host may want to hide when its backend cannot do it.
/// Hosts ask [`WebBackend::supports`] — never a hard-coded engine name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendFeature {
    /// [`WebBackend::find`] / [`WebBackend::stop_find`].
    Find,
    /// [`WebBackend::set_zoom`].
    Zoom,
    /// [`WebBackend::show_devtools`].
    DevTools,
    /// [`WebBackend::stop`].
    Stop,
    /// [`WebEvent::Favicon`].
    Favicon,
    /// [`WebEvent::Security`].
    Security,
}

/// A page the [`crate::view::WebviewState`] can drive.
///
/// Every method is a command; nothing returns a result, because a real webview
/// answers asynchronously. Answers come back through [`Self::poll_events`],
/// which the pane drains on a timer — except evaluation answers, which come
/// back through [`Self::take_eval_results`], drained the same way.
pub trait WebBackend {
    /// Loads `url`.
    fn navigate(&mut self, url: &str);

    /// Goes back one entry in history, if there is one.
    fn back(&mut self);

    /// Goes forward one entry, if there is one.
    fn forward(&mut self);

    /// Reloads the current page.
    fn reload(&mut self);

    /// Runs `js` in the page.
    fn eval(&mut self, js: &str);

    /// Runs `js` in the page and routes its answer to
    /// [`Self::take_eval_results`] under `request_id`.
    ///
    /// `js` is a **function body**: its answer is what it `return`s (it may
    /// `await`), JSON-stringified, or the thrown exception's message. A bare
    /// expression with no `return` answers `null` — seen live, when every
    /// `agent_js` helper was an unreturned IIFE. A real page answers asynchronously, so this
    /// only queues the request; the default queues
    /// `Err("not supported")` immediately, through the fallback queue
    /// [`Self::take_eval_results`] drains.
    ///
    /// Results travel on this separate channel — not as a new [`WebEvent`]
    /// variant — so hosts matching on `WebEvent` exhaustively keep compiling.
    fn eval_with_result(&mut self, request_id: u64, js: &str) {
        let _ = js;
        DEFAULT_EVAL_RESULTS.with(|slot| slot.borrow_mut().push((request_id, Err(String::from("not supported")))));
    }

    /// Takes every evaluation answer since the last call, by request id.
    /// Backends that override [`Self::eval_with_result`] with real answers
    /// override this too; the default drains the fallback queue above.
    fn take_eval_results(&mut self) -> Vec<(u64, Result<String, String>)> {
        DEFAULT_EVAL_RESULTS.with(|slot| std::mem::take(&mut *slot.borrow_mut()))
    }

    /// Turns annotate mode on or off. Off clears the hovered element.
    fn set_annotate(&mut self, on: bool);

    /// Takes everything the page has said since the last call.
    fn poll_events(&mut self) -> Vec<WebEvent>;

    /// Whether [`Self::back`] would go anywhere. Defaults to `false`.
    fn can_go_back(&self) -> bool {
        false
    }

    /// Whether [`Self::forward`] would go anywhere. Defaults to `false`.
    fn can_go_forward(&self) -> bool {
        false
    }

    /// Asks for a screenshot; the answer arrives as [`WebEvent::Screenshot`].
    /// Backends that cannot capture do nothing, which is the default.
    fn capture(&mut self) {}

    /// Asks for the picture a cover paints in the page's place; the answer
    /// arrives as [`WebEvent::StandIn`] (or as a [`WebEvent::Screenshot`],
    /// which serves too). It is on the critical path of every cover, so a
    /// backend should make it cheap: the CEF one asks for a speed-optimised
    /// JPEG, since Chromium encodes on the thread that also runs the UI.
    /// Defaults to [`Self::capture`].
    fn capture_stand_in(&mut self) {
        self.capture();
    }

    /// Whether the page is a **native surface composited over the gpui scene**
    /// rather than gpui elements inside it.
    ///
    /// The pane asks because a native page changes three things: gpui cannot
    /// paint over it (see
    /// [`WebviewState::set_obscured`](crate::view::WebviewState::set_obscured)),
    /// it hit-tests inside the page so the Rust-side pointer methods below are
    /// dead, and it has to be told where to sit ([`Self::set_bounds`]).
    fn is_native(&self) -> bool {
        false
    }

    /// Puts the page's native surface at `origin` with size `size`, both in
    /// logical pixels relative to the window's top-left. A page made of gpui
    /// elements is laid out by gpui and ignores this.
    fn set_bounds(&mut self, origin: (f32, f32), size: (f32, f32)) {
        let _ = (origin, size);
    }

    /// Rounds the page's native surface to `radius` logical pixels (0 =
    /// square), so a page set inside a rounded card matches it. A page made
    /// of gpui elements ignores this.
    fn set_corner_radius(&mut self, radius: f32) {
        let _ = radius;
    }

    /// Cuts rounded holes (x, y, w, h, radius — the host window's logical
    /// coordinates) through the page's native surface, so host chrome drawn
    /// under it shows through and takes the clicks there. An empty list
    /// removes them. Engines without a native surface ignore this.
    fn set_holes(&mut self, holes: Vec<[f32; 5]>) {
        let _ = holes;
    }

    /// The shown page's certificate, when it was served over TLS and the
    /// engine exposes one. Engines that do not answer `None`.
    fn certificate(&self) -> Option<CertificateInfo> {
        None
    }

    /// Shows or hides the page's native surface. Hiding it is the only way gpui
    /// gets to draw in that rectangle at all; see
    /// [`WebviewState::set_obscured`](crate::view::WebviewState::set_obscured).
    fn set_visible(&mut self, visible: bool) {
        let _ = visible;
    }

    /// Moves the keyboard focus into the page (`true`) or back to the host
    /// window (`false`). A native page holds first responder while it is
    /// focused, so the pane hands focus back before it reads a keystroke of its
    /// own — the URL field, for instance.
    fn set_focused(&mut self, focused: bool) {
        let _ = focused;
    }

    /// Whether the page currently holds the keyboard: a native page is its
    /// window's first responder (or contains it) after a click inside it, and
    /// while it does gpui text input gets nothing. The pane asks before it
    /// hides the page or reads a keystroke of its own, so hiding a focused
    /// page can hand the keyboard back first. Defaults to `false`; backends
    /// without a native surface never hold it.
    fn holds_keyboard(&self) -> bool {
        false
    }

    /// The pointer moved to `at` in page coordinates, or left the page
    /// (`None`). Only backends that hit-test in Rust — the mock — use this; a
    /// real webview does its own hit-testing inside the page, so the default
    /// is to ignore it.
    fn point_moved(&mut self, at: Option<(f32, f32)>) {
        let _ = at;
    }

    /// The pointer was pressed at `at` in page coordinates. In annotate mode a
    /// backend that hit-tests in Rust answers with a [`WebEvent::Annotation`].
    fn point_clicked(&mut self, at: (f32, f32)) {
        let _ = at;
    }

    /// The element under the pointer, when the backend knows it.
    fn hovered(&self) -> Option<ElementInfo> {
        None
    }

    /// The metadata collected for the annotation numbered `index`.
    fn element_info(&self, index: usize) -> Option<ElementInfo> {
        let _ = index;
        None
    }

    /// Stops the current load, if any. Backends that cannot do it ignore
    /// this, which is the default.
    fn stop(&mut self) {}

    /// Sets the page zoom to `percent` (100 is normal). Backends snap to
    /// their engine's steps; backends that cannot zoom ignore this, which is
    /// the default.
    fn set_zoom(&mut self, percent: u16) {
        let _ = percent;
    }

    /// Finds `query` in the page. Calling it again with the same query moves
    /// to the next match (`forward`) or the previous one; a different query
    /// starts a new search. Results arrive as [`WebEvent::Find`]. Backends
    /// that cannot search ignore this, which is the default.
    fn find(&mut self, query: &str, forward: bool, match_case: bool) {
        let _ = (query, forward, match_case);
    }

    /// Ends the find session and clears the highlight. Does nothing by
    /// default.
    fn stop_find(&mut self) {}

    /// Opens the developer tools. Does nothing by default.
    fn show_devtools(&mut self) {}

    /// Whether this backend implements `feature`, so a host can hide the
    /// controls it cannot back. `false` by default: a backend opts each
    /// control in.
    fn supports(&self, feature: BackendFeature) -> bool {
        let _ = feature;
        false
    }

    /// Takes the popup pages queued since the last call, oldest first: each
    /// is `(url, foreground, page)`, a backend the host wraps in a new tab
    /// next to the opener. Announced by [`WebEvent::PopupOpened`]; empty by
    /// default.
    fn take_popup_backends(&mut self) -> Vec<(String, bool, Box<dyn WebBackend>)> {
        Vec::new()
    }

    /// Takes the "open this URL" requests queued since the last call, oldest
    /// first: each is `(url, foreground)`. Context-menu "open in new tab"
    /// lands here (the page cannot open it), and the host makes a tab for it
    /// itself. Empty by default.
    fn take_open_requests(&mut self) -> Vec<(String, bool)> {
        Vec::new()
    }

    /// Answers the permission request `id` (see [`WebEvent::Permission`]).
    /// Unknown ids are ignored. Does nothing by default.
    fn answer_permission(&mut self, id: u64, decision: PermissionDecision) {
        let _ = (id, decision);
    }

    /// Cancels the download `id` (see [`WebEvent::Download`]). Unknown ids
    /// are ignored. Does nothing by default.
    fn cancel_download(&mut self, id: u32) {
        let _ = id;
    }
}

/// How often a download may report progress: at most one [`DownloadState`]
/// per id per interval, except state changes, which always land.
pub const DOWNLOAD_EMIT_INTERVAL_MS: u64 = 250;

/// Whether a download update is worth emitting: always the first
/// (`elapsed_ms` is `None`), always a state change, otherwise only once per
/// [`DOWNLOAD_EMIT_INTERVAL_MS`]. Pure, so the quiet rule is unit-testable
/// without a browser.
pub fn download_emit_due(elapsed_ms: Option<u64>, state_changed: bool) -> bool {
    if state_changed {
        return true;
    }
    match elapsed_ms {
        None => true,
        Some(elapsed) => elapsed >= DOWNLOAD_EMIT_INTERVAL_MS,
    }
}

/// The file name a download suggestion lands under: the last path component
/// only (either separator), with NUL and control characters stripped. `.`,
/// `..` and the empty string read as `download`, so `dir.join` never sees an
/// absolute path or a parent escape. Pure, so the scrubbing is unit-testable
/// without a filesystem.
pub fn sanitize_download_name(suggested: &str) -> String {
    let last = suggested.split(['/', '\\']).rfind(|part| !part.is_empty()).unwrap_or("");
    let scrubbed: String = last.chars().filter(|c| !c.is_control()).collect();
    match scrubbed.as_str() {
        "" | "." | ".." => String::from("download"),
        _ => scrubbed,
    }
}

/// A file name in the download folder that nothing has taken yet: `name`
/// itself when free, else `stem (1).ext`, `stem (2).ext`, … `taken` is the
/// folder's membership test (usually "this path exists"). An empty `name`
/// reads as `download`; a leading dot stays with the stem (`.zshrc` never
/// grows an "extension"). Pure, so the numbering is unit-testable without a
/// filesystem.
pub fn unique_download_name(taken: &dyn Fn(&str) -> bool, name: &str) -> String {
    let name = if name.is_empty() { "download" } else { name };
    if !taken(name) {
        return name.to_owned();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    let mut n = 1u32;
    loop {
        let candidate = format!("{stem} ({n}){ext}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backend implementing only the required methods.
    struct Minimal;

    impl WebBackend for Minimal {
        fn navigate(&mut self, url: &str) {
            let _ = url;
        }
        fn back(&mut self) {}
        fn forward(&mut self) {}
        fn reload(&mut self) {}
        fn eval(&mut self, _js: &str) {}
        fn set_annotate(&mut self, _on: bool) {}
        fn poll_events(&mut self) -> Vec<WebEvent> {
            Vec::new()
        }
    }

    #[test]
    fn the_default_eval_impl_answers_not_supported() {
        let mut backend = Minimal;
        backend.eval_with_result(7, "document.title");
        assert_eq!(backend.take_eval_results(), vec![(7, Err(String::from("not supported")))]);
        // Drained: a second take sees nothing.
        assert!(backend.take_eval_results().is_empty());
    }

    #[test]
    fn new_controls_default_to_unsupported_no_ops() {
        let mut backend = Minimal;
        // They run without doing anything …
        backend.stop();
        backend.set_zoom(125);
        backend.find("price", true, false);
        backend.stop_find();
        backend.show_devtools();
        // … and report every feature as unsupported.
        for feature in [
            BackendFeature::Find,
            BackendFeature::Zoom,
            BackendFeature::DevTools,
            BackendFeature::Stop,
            BackendFeature::Favicon,
            BackendFeature::Security,
        ] {
            assert!(!backend.supports(feature), "{feature:?} must default to false");
        }
    }

    #[test]
    fn security_classification_table() {
        use PageSecurity::{File, Internal, Local, NotSecure, Secure, Unknown};
        let cases = [
            ("https://example.com/", Some(true), Secure),
            ("https://example.com/", None, Secure),
            ("https://example.com/", Some(false), NotSecure),
            ("http://example.com/", None, NotSecure),
            // An `http` page is never secure, whatever the transport claims.
            ("http://example.com/", Some(true), NotSecure),
            ("http://localhost:5173/", None, Local),
            ("http://localhost/", None, Local),
            ("http://127.0.0.1:3000/x", None, Local),
            ("http://[::1]:8080/", None, Local),
            ("http://shop.localhost/", None, Local),
            ("http://api.test/", None, Local),
            // The loopback host wins over a secure scheme too.
            ("https://app.localhost/", Some(true), Local),
            // … but a certificate error wins over the loopback host.
            ("https://localhost:8443/", Some(false), NotSecure),
            ("https://app.localhost/", Some(false), NotSecure),
            ("https://127.0.0.1/", Some(false), NotSecure),
            ("wss://localhost/socket", Some(false), NotSecure),
            // A backslash ends the authority like a slash (WHATWG).
            ("https://example.com\\@evil.com/", None, Secure),
            ("http://localhost\\path", None, Local),
            // A trailing dot is a fully-qualified name, still local.
            ("http://localhost./", None, Local),
            ("http://shop.localhost./x", None, Local),
            ("http://127.0.0.1./", None, Local),
            ("https://example.com./", Some(true), Secure),
            ("HTTPS://EXAMPLE.COM/", Some(true), Secure),
            ("file:///etc/hosts", None, File),
            ("chrome://settings/", None, Internal),
            ("about:blank", None, Internal),
            ("devtools://devtools/bundled/inspector.html", None, Internal),
            ("data:text/html,hi", None, Unknown),
            ("madeup:thing", None, Unknown),
            ("", None, Unknown),
            ("no scheme at all", None, Unknown),
            // The fake page's schemeless home URL is not a page state.
            ("localhost:3000/pricing", None, Unknown),
        ];
        for (url, secure, expected) in cases {
            assert_eq!(PageSecurity::classify(url, secure), expected, "url: {url}");
        }
    }

    #[test]
    fn new_popup_permission_and_download_controls_default_to_empty_no_ops() {
        let mut backend = Minimal;
        // They run without doing anything …
        backend.answer_permission(1, PermissionDecision::Allow);
        backend.cancel_download(9);
        // … and the queues drain empty.
        assert!(backend.take_popup_backends().is_empty());
        assert!(backend.take_open_requests().is_empty());
    }

    #[test]
    fn free_download_names_pass_through_and_taken_ones_number_up() {
        let taken = |name: &str| matches!(name, "a.txt" | "a (1).txt" | "noext" | ".zshrc");
        assert_eq!(unique_download_name(&|_: &str| false, "a.txt"), "a.txt");
        assert_eq!(unique_download_name(&taken, "a.txt"), "a (2).txt");
        assert_eq!(unique_download_name(&taken, "b.txt"), "b.txt");
        assert_eq!(unique_download_name(&taken, "noext"), "noext (1)");
        // A leading dot is not an extension.
        assert_eq!(unique_download_name(&taken, ".zshrc"), ".zshrc (1)");
        // An empty suggestion still lands somewhere.
        assert_eq!(unique_download_name(&|_: &str| false, ""), "download");
        assert_eq!(unique_download_name(&|name: &str| name == "download", ""), "download (1)");
    }

    #[test]
    fn suggested_names_scrub_to_a_safe_last_component() {
        // Absolute paths never reach `dir.join`: only the last part survives.
        assert_eq!(sanitize_download_name("/abs/x"), "x");
        assert_eq!(sanitize_download_name("../x"), "x");
        assert_eq!(sanitize_download_name("a/b"), "b");
        assert_eq!(sanitize_download_name("a\\b"), "b");
        // `.`, `..` and the empty string all land somewhere harmless.
        assert_eq!(sanitize_download_name(""), "download");
        assert_eq!(sanitize_download_name("."), "download");
        assert_eq!(sanitize_download_name(".."), "download");
        // NUL and control characters go away, the rest stays.
        assert_eq!(sanitize_download_name("a\0b/c\0"), "c");
        assert_eq!(sanitize_download_name("a/b\x07c\x1bd"), "bcd");
        // Ordinary names pass through untouched.
        assert_eq!(sanitize_download_name("report (1).pdf"), "report (1).pdf");
        assert_eq!(sanitize_download_name(".zshrc"), ".zshrc");
    }

    #[test]
    fn download_progress_reports_coalesce_but_state_changes_land() {
        // The first report always lands.
        assert!(download_emit_due(None, false));
        assert!(download_emit_due(None, true));
        // A state change always lands, however soon.
        assert!(download_emit_due(Some(0), true));
        // Progress alone waits out the interval …
        assert!(!download_emit_due(Some(0), false));
        assert!(!download_emit_due(Some(249), false));
        // … then lands.
        assert!(download_emit_due(Some(250), false));
        assert!(download_emit_due(Some(10_000), false));
    }
}
