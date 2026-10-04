//! [`CefBackend`]: a Chrome-style CEF page behind [`aui_webview::WebBackend`].
//!
//! Each backend is one frameless Views `CefWindow` with no Chrome toolbar,
//! adopted as a **child window** of the host's window and kept over the rect
//! the pane lays out (spike S1). It is a native overlay exactly like the wry
//! backend's WKWebView: gpui can never paint over it, so the host's
//! native-overlay rule (`set_visible(false)` while anything covers the pane)
//! applies unchanged.
//!
//! Automation goes over CEF's **in-process DevTools channel**
//! (`send_dev_tools_message` + an observer) — no remote-debugging port:
//!
//! - `eval_with_result` is `Runtime.evaluate` with `awaitPromise`, so a host
//!   script may `await` and return a value directly (the IPC detour WebKit
//!   needed is gone);
//! - `capture` is `Page.captureScreenshot`;
//! - the annotator ([`aui_webview::ANNOTATOR_JS`]) is registered with
//!   `Page.addScriptToEvaluateOnNewDocument`, and its `window.ipc.postMessage`
//!   is backed by a `Runtime.addBinding` channel.
//!
//! A page starts on `about:blank`; the first real URL loads only after those
//! scripts are registered, so the first document is already annotatable.
//!
//! # The lock rule
//!
//! [`Shared`] sits behind a `Mutex`, and **nothing calls into AppKit or CEF
//! while holding it**. Both call straight back on the same thread: placing the
//! child window makes gpui redraw synchronously (which lays the pane out and
//! calls `set_bounds`), and CEF runs handlers inside the calls that trigger
//! them. Holding the lock across such a call deadlocks the main thread on
//! itself (B1a, live: `__psynch_mutexwait` under `-[NSWindow
//! makeKeyAndOrderFront:]` → gpui `window_did_change_key` → draw). So every
//! path copies what it needs under the lock, drops it, then acts.
//!
//! # Host shortcuts, command filtering, never-main (B1b-L)
//!
//! The page is key while the user types in it, so two CEF handlers keep it
//! from hijacking the host: the keyboard handler consumes keystrokes the
//! host's router ([`crate::keys::set_key_router`]) claims (the host's
//! `claims` runs synchronously inside `on_pre_key_event`, and `deliver` on a
//! later main-queue turn), and the command handler blocks Chromium's own
//! accelerators (⌘N, ⌘T, …) via [`crate::keys::chrome_command_allowed`].
//! The page window is additionally flagged at creation
//! (`aui_cef_never_main`, a class-level `canBecomeMainWindow` swizzle guarded
//! by a per-instance flag) so it can become key but never main, keeping the
//! host window's traffic lights coloured.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use aui::workbench::Annotation;
use aui_webview::backend::OUTER_HTML_LIMIT;
use aui_webview::wry_backend::navigation_allowed;
use aui_webview::{
    BackendFeature, DownloadState, DownloadStatus, ElementInfo, FindState, PageSecurity, PermissionDecision, PermissionKind, PermissionRequest, WebBackend, WebEvent,
    ANNOTATOR_JS, download_emit_due, sanitize_download_name, unique_download_name,
};
use base64::Engine as _;
use cef::*;
use gpui::SharedString;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::Deserialize;
use serde_json::{json, Value};

unsafe extern "C" {
    fn aui_cef_place(parent: *mut c_void, child: *mut c_void, x: f64, y: f64, w: f64, h: f64) -> i32;
    fn aui_cef_set_hidden(child: *mut c_void, hidden: i32);
    fn aui_cef_is_key(child: *mut c_void) -> i32;
    fn aui_cef_focus_parent(parent: *mut c_void);
    fn aui_cef_focus_child(child: *mut c_void);
    fn aui_cef_detach(child: *mut c_void);
    fn aui_cef_defer(f: extern "C" fn(*mut c_void), ctx: *mut c_void);
    fn aui_cef_never_main(child: *mut c_void);
}

/// The binding the annotator's `window.ipc.postMessage` posts through.
const BINDING: &str = "__auiPost";

/// Runs before every document, ahead of the annotator: the `window.ipc`
/// shape the annotator was written against (wry's), over the CDP binding.
const IPC_PRELUDE: &str = "window.ipc = window.ipc || { postMessage: function (m) { window.__auiPost(String(m)); } };";

/// What a DevTools method call is waiting to do with its answer.
#[derive(Debug, Clone, Copy)]
enum Pending {
    /// A host `eval_with_result`, by the host's request id.
    Eval(u64),
    Screenshot,
    /// Setup calls; the last one releases the first real navigation.
    Setup { last: bool },
}

#[derive(Default)]
struct Shared {
    events: Vec<WebEvent>,
    elements: HashMap<usize, ElementInfo>,
    eval_results: Vec<(u64, Result<String, String>)>,
    pending: HashMap<i32, Pending>,
    next_id: i32,
    browser: Option<Browser>,
    window: Option<cef::Window>,
    registration: Option<Registration>,
    /// The page window's root `NSView`, as an address (raw pointers are not `Send`).
    child_view: usize,
    /// The host window's content `NSView`.
    parent_view: usize,
    bounds: ((f32, f32), (f32, f32)),
    visible: bool,
    /// Set once the setup scripts are registered.
    ready: bool,
    /// The URL to load once ready (the latest `navigate` wins).
    pending_url: Option<String>,
    can_back: bool,
    can_forward: bool,
    closed: bool,
    /// The `(query, match_case)` the last [`WebBackend::find`] started, or
    /// `None` after [`WebBackend::stop_find`]. Find results carry no query of
    /// their own, so this tags them on the way up — and decides `find_next`.
    find_session: Option<(String, bool)>,
    /// How many audio streams are currently open. The tab reads as audible
    /// while this is non-zero.
    audio_streams: u32,
    /// Popups the host has not taken yet ([`CefBackend::take_popups`]),
    /// oldest first.
    popups: Vec<QueuedPopup>,
    /// Popups `on_before_popup` announced but
    /// `on_popup_browser_view_created` has not hosted yet, as
    /// `(popup_id, popup)`.
    pending_popups: Vec<(i32, QueuedPopup)>,
    /// "Open this URL" requests from the context menu
    /// ([`WebBackend::take_open_requests`]), oldest first.
    open_requests: Vec<(String, bool)>,
    /// The link (else media source) URL the open context menu points at, as
    /// `on_before_context_menu` last saw it.
    menu_link: Option<String>,
    /// Permission callbacks waiting for [`WebBackend::answer_permission`],
    /// by prompt id.
    permissions: HashMap<u64, StoredPermission>,
    /// The next host-minted permission id (media access has no engine id).
    next_permission_id: u64,
    /// Download cancel callbacks, by engine download id.
    dl_callbacks: HashMap<u32, DownloadItemCallback>,
    /// When each download last reported, and what state it reported.
    dl_emit: HashMap<u32, (Instant, DownloadStatus)>,
    /// Where each download writes, by engine download id.
    dl_meta: HashMap<u32, DlMeta>,
    /// The last progress pushed, so repeats under 0.02 stay quiet.
    last_progress: Option<f32>,
    /// The last favicon and security pushed, so unchanged repeats stay quiet.
    last_favicon: Option<Option<String>>,
    last_security: Option<PageSecurity>,
}

fn locked<T>(slot: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

type Page = Arc<Mutex<Shared>>;

/// A popup page the opener queued for the host to tab.
pub struct PopupTab {
    /// The popup as its own backend: a frameless child-window host over the
    /// opener's parent, starting hidden at zero size. The host tabs it by
    /// pushing bounds and visibility, like any page.
    pub backend: CefBackend,
    /// Where the popup points.
    pub url: String,
    /// Where the opener pointed when the popup was made.
    pub opener_url: String,
    /// Whether the popup asked for the foreground tab.
    pub foreground: bool,
}

/// A popup page waiting on its opener: the page plus what
/// [`CefBackend::take_popups`] reports for it.
struct QueuedPopup {
    page: Page,
    url: String,
    opener_url: String,
    foreground: bool,
}

impl Drop for QueuedPopup {
    /// An orphaned popup — aborted, failed to host, opener gone — closes its
    /// page on the way out, so no hidden browser lingers. A queued popup the
    /// host takes goes through [`QueuedPopup::into_taken`], never dropped.
    fn drop(&mut self) {
        close(&self.page);
    }
}

impl QueuedPopup {
    /// Moves the parts out for the host tab without closing the page: the
    /// tab owns it now.
    fn into_taken(self) -> (Page, String, String, bool) {
        let taken = std::mem::ManuallyDrop::new(self);
        // Safety: each field is read out exactly once, and `taken` — which
        // would run no `Drop` anyway — is never used again.
        unsafe {
            let page = std::ptr::read(&taken.page);
            let url = std::ptr::read(&taken.url);
            let opener_url = std::ptr::read(&taken.opener_url);
            (page, url, opener_url, taken.foreground)
        }
    }
}

/// Takes the staged popup with `popup_id`, wherever it sits in the queue —
/// identity, not position, so concurrent popups claim their own entry however
/// CEF interleaves them. Pure, so the matching is unit-testable without CEF.
fn take_pending_popup_by_id(pending: &mut Vec<(i32, QueuedPopup)>, popup_id: i32) -> Option<QueuedPopup> {
    pending.iter().position(|(id, _)| *id == popup_id).map(|at| pending.remove(at).1)
}

/// A popup page's [`Shared`]: hidden at zero size until the host tabs it, and
/// with no pending URL. CEF is already navigating the popup browser, so a
/// pending URL would navigate it twice — breaking POST/form-target popups,
/// re-requesting OAuth URLs, and wiping `about:blank` popups the opener
/// writes into. The setup scripts still register; [`release_first_navigation`]
/// then only marks the page ready. Pure, so the no-reload rule is
/// unit-testable without CEF.
fn new_popup_shared(parent_view: usize) -> Shared {
    Shared { parent_view, bounds: ((0.0, 0.0), (0.0, 0.0)), visible: false, ..Shared::default() }
}

/// A permission callback waiting for the host's answer.
#[derive(Clone)]
enum StoredPermission {
    /// A prompt callback: answered with accept/deny/dismiss.
    Prompt(PermissionPromptCallback),
    /// A media-access callback: continued with `allowed` on allow,
    /// cancelled on deny or dismiss.
    Media { callback: MediaAccessCallback, allowed: u32 },
}

/// Where one download writes.
#[derive(Clone)]
struct DlMeta {
    url: String,
    file_name: String,
    path: String,
}

/// The folder downloads land in, runtime-wide (every page).
static DOWNLOAD_DIR: std::sync::OnceLock<Mutex<PathBuf>> = std::sync::OnceLock::new();

fn download_dir_slot() -> &'static Mutex<PathBuf> {
    DOWNLOAD_DIR.get_or_init(|| Mutex::new(default_download_dir()))
}

fn default_download_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp")).join("Downloads")
}

fn current_download_dir() -> PathBuf {
    locked(download_dir_slot()).clone()
}

// ------------------------------------------------------------------ actions (lock released before acting)

/// Puts the page window over the host's rect, shown or hidden.
fn place(page: &Page) {
    let (parent, child, ((x, y), (w, h)), visible) = {
        let s = locked(page);
        (s.parent_view, s.child_view, s.bounds, s.visible)
    };
    if parent == 0 || child == 0 {
        return;
    }
    // Safety: both addresses are live NSViews owned by their windows; main thread.
    unsafe {
        aui_cef_place(parent as *mut c_void, child as *mut c_void, x as f64, y as f64, w as f64, h as f64);
        aui_cef_set_hidden(child as *mut c_void, if visible { 0 } else { 1 });
    }
}

fn browser_of(page: &Page) -> Option<Browser> {
    locked(page).browser.clone()
}

/// Sends one DevTools method call; `None` when there is no channel yet.
fn send(page: &Page, method: &str, params: Value, purpose: Pending) -> Option<i32> {
    let host = browser_of(page)?.host()?;
    let id = {
        let mut s = locked(page);
        s.next_id += 1;
        let id = 10_000 + s.next_id;
        s.pending.insert(id, purpose);
        id
    };
    let message = json!({ "id": id, "method": method, "params": params }).to_string();
    if host.send_dev_tools_message(Some(message.as_bytes())) == 0 {
        locked(page).pending.remove(&id);
        return None;
    }
    Some(id)
}

fn load(page: &Page, url: &str) {
    if let Some(frame) = browser_of(page).and_then(|b| b.main_frame()) {
        frame.load_url(Some(&CefString::from(url)));
    }
}

/// Marks the page ready and loads whatever URL was waiting for it.
fn release_first_navigation(page: &Page) {
    let url = {
        let mut s = locked(page);
        s.ready = true;
        s.pending_url.take()
    };
    if let Some(url) = url {
        load(page, &url);
    }
}

thread_local! {
    /// Every page this process opened, for [`close_all`] at quit.
    static OPEN: RefCell<Vec<Weak<Mutex<Shared>>>> = const { RefCell::new(Vec::new()) };
}

/// Asks every open page to close (quit path).
pub(crate) fn close_all() {
    let pages: Vec<Page> = OPEN.with(|open| open.borrow().iter().filter_map(Weak::upgrade).collect());
    for page in pages {
        close(&page);
    }
}

/// Pages not yet reported closed.
pub(crate) fn open_count() -> usize {
    OPEN.with(|open| open.borrow().iter().filter_map(Weak::upgrade).filter(|s| !locked(s).closed).count())
}

fn close(page: &Page) {
    // After `shutdown` (or before init) there is no CEF to call into; a page
    // dropped late at quit has nothing left to close.
    if crate::runtime::state() != crate::runtime::State::Ready {
        return;
    }
    let (child, window, browser) = {
        let mut s = locked(page);
        // A page whose window was never created must not create it later.
        s.pending_url = None;
        (s.child_view, s.window.clone(), s.browser.clone())
    };
    if child != 0 {
        // Safety: a live NSView address, main thread.
        unsafe { aui_cef_detach(child as *mut c_void) };
    }
    if let Some(window) = window {
        window.close();
    } else if let Some(host) = browser.and_then(|b| b.host()) {
        host.close_browser(1);
    } else {
        locked(page).closed = true;
    }
}

// ------------------------------------------------------------------ deferral

extern "C" fn run_deferred(ctx: *mut c_void) {
    // Safety: `ctx` is the box `defer` leaked, run exactly once.
    let f = unsafe { Box::from_raw(ctx as *mut Box<dyn FnOnce()>) };
    f();
}

/// Runs `f` on the main queue's next turn, outside the caller's host update.
pub(crate) fn defer(f: impl FnOnce() + 'static) {
    let boxed: Box<Box<dyn FnOnce()>> = Box::new(Box::new(f));
    // Safety: `run_deferred` takes the box back and frees it.
    unsafe { aui_cef_defer(run_deferred, Box::into_raw(boxed) as *mut c_void) };
}

/// A closure a CEF task runs once.
type Once = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

wrap_task! {
    struct OnceTask {
        f: Once,
    }

    impl Task {
        fn execute(&self) {
            let f = self.f.borrow_mut().take();
            if let Some(f) = f {
                f();
            }
        }
    }
}

/// Runs `f` as a CEF UI-thread task: inside the pump's `CefDoMessageLoopWork`,
/// which is where CEF expects Views windows to be created.
fn on_cef_ui(f: impl FnOnce() + 'static) {
    let mut task = OnceTask::new(Rc::new(RefCell::new(Some(Box::new(f)))));
    if post_task(ThreadId::UI, Some(&mut task)) == 1 {
        crate::runtime::kick_pump();
    } else {
        eprintln!("aui-cef: could not post a UI task");
    }
}

// ------------------------------------------------------------------ the annotator's messages

#[derive(Debug, Deserialize)]
struct Hit {
    selector: String,
    label: String,
    rect: HitRect,
    #[serde(rename = "outerHTML")]
    outer_html: String,
}

#[derive(Debug, Deserialize)]
struct HitRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// One annotator click, as `ANNOTATOR_JS` posts it (same handling as the wry
/// backend's `on_ipc`, minus eval answers, which CDP returns directly).
fn on_annotator_message(shared: &mut Shared, body: &str) {
    let Ok(hit) = serde_json::from_str::<Hit>(body) else { return };
    let index = shared.elements.len() + 1;
    let mut outer_html = hit.outer_html;
    outer_html.truncate(OUTER_HTML_LIMIT.min(outer_html.len()));
    shared.elements.insert(
        index,
        ElementInfo {
            selector: SharedString::from(hit.selector.clone()),
            label: SharedString::from(hit.label),
            origin: (hit.rect.x, hit.rect.y),
            size: (hit.rect.w, hit.rect.h),
            source: SharedString::default(),
            outer_html: SharedString::from(outer_html),
        },
    );
    shared.events.push(WebEvent::Annotation(Annotation::new(index, hit.selector, "").pending(true)));
}

/// One DevTools message: a method result (by id) or an event.
fn on_devtools(page: &Page, message: &[u8]) {
    let Ok(value) = serde_json::from_slice::<Value>(message) else { return };
    if let Some(id) = value.get("id").and_then(Value::as_i64) {
        let Some(purpose) = locked(page).pending.remove(&(id as i32)) else { return };
        let error = value.get("error").map(|e| e.get("message").and_then(Value::as_str).unwrap_or("DevTools error").to_owned());
        match purpose {
            Pending::Eval(request) => {
                let result = match error {
                    Some(e) => Err(e),
                    None => eval_answer(value.get("result").unwrap_or(&Value::Null)),
                };
                locked(page).eval_results.push((request, result));
            }
            Pending::Screenshot => {
                let data = value.pointer("/result/data").and_then(Value::as_str).unwrap_or_default();
                match base64::engine::general_purpose::STANDARD.decode(data) {
                    Ok(bytes) if !bytes.is_empty() => locked(page).events.push(WebEvent::Screenshot(bytes)),
                    _ => eprintln!("aui-cef: captureScreenshot returned no image ({})", error.unwrap_or_default()),
                }
            }
            Pending::Setup { last } => {
                if let Some(e) = error {
                    eprintln!("aui-cef: page setup call failed: {e}");
                }
                if last {
                    release_first_navigation(page);
                }
            }
        }
        return;
    }
    if value.get("method").and_then(Value::as_str) == Some("Runtime.bindingCalled")
        && value.pointer("/params/name").and_then(Value::as_str) == Some(BINDING)
    {
        let payload = value.pointer("/params/payload").and_then(Value::as_str).unwrap_or_default().to_owned();
        on_annotator_message(&mut locked(page), &payload);
    }
}

/// A `Runtime.evaluate` result as the host's `eval_with_result` contract
/// wants it: the value's JSON text (`null` for undefined), or the exception.
fn eval_answer(result: &Value) -> Result<String, String> {
    if let Some(details) = result.get("exceptionDetails") {
        let message = details
            .pointer("/exception/description")
            .or_else(|| details.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("the script failed");
        // The first line of a JS error description is its message.
        let message = message.lines().next().unwrap_or(message);
        let message = message.strip_prefix("Error: ").unwrap_or(message);
        return Err(message.to_owned());
    }
    let value = result.pointer("/result/value").cloned().unwrap_or(Value::Null);
    Ok(value.to_string())
}

// ------------------------------------------------------------------ page state (pure)

/// Chrome's zoom menu steps, in percent.
pub const ZOOM_STEPS: [u16; 17] = [25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300, 400, 500];

/// Snaps `percent` to the nearest Chrome zoom step (ties go to the smaller step).
pub fn snap_zoom_percent(percent: u16) -> u16 {
    let mut best = ZOOM_STEPS[0];
    for step in ZOOM_STEPS {
        if step.abs_diff(percent) < best.abs_diff(percent) {
            best = step;
        }
    }
    best
}

/// CEF zoom levels are log base 1.2 of the scale factor: this maps a percent
/// (snapped to [`ZOOM_STEPS`]) to the level [`WebBackend::set_zoom`] pushes.
pub fn zoom_percent_to_level(percent: u16) -> f64 {
    (f64::from(snap_zoom_percent(percent)) / 100.0).ln() / 1.2_f64.ln()
}

/// Maps a CEF zoom level back to a Chrome step percent (100 is normal).
/// Non-finite levels read as 100.
pub fn zoom_level_to_percent(level: f64) -> u16 {
    if !level.is_finite() {
        return 100;
    }
    snap_zoom_percent((1.2_f64.powf(level) * 100.0).round().max(0.0) as u16)
}

/// A human reason for a dead renderer, as [`WebEvent::Crashed`] carries it.
fn crash_reason(status: TerminationStatus, error_code: i32, detail: &str) -> String {
    let base = if status == TerminationStatus::PROCESS_CRASHED {
        "the page crashed"
    } else if status == TerminationStatus::PROCESS_OOM {
        "the page ran out of memory"
    } else if status == TerminationStatus::PROCESS_WAS_KILLED {
        "the page was killed"
    } else if status == TerminationStatus::ABNORMAL_TERMINATION {
        "the page terminated abnormally"
    } else {
        "the page terminated unexpectedly"
    };
    let detail = detail.trim();
    if detail.is_empty() {
        format!("{base} ({error_code})")
    } else {
        format!("{base}: {detail} ({error_code})")
    }
}

/// Moves the audio-stream count by `delta` (+1 per started stream, −1 per
/// stopped or errored one, floored at 0), answering the new count and the
/// [`WebEvent::Audible`] to push, if the tab's audibility changed. Pure, so
/// the counting rule is unit-testable without a browser.
fn audio_update(count: u32, delta: i32) -> (u32, Option<bool>) {
    let next = (count as i64 + delta as i64).max(0) as u32;
    let event = match (count > 0, next > 0) {
        (false, true) => Some(true),
        (true, false) => Some(false),
        _ => None,
    };
    (next, event)
}

/// Whether `find` with `query`/`match_case` continues the `active` session
/// (`find_next`) or starts a new one. Only the same query *and* the same
/// case sensitivity keep the session: after [`WebBackend::stop_find`] there
/// is no active session, and toggling the case with the same query
/// re-searches. Pure, so both misfires are unit-testable without a browser.
fn find_is_next(active: Option<(&str, bool)>, query: &str, match_case: bool) -> bool {
    matches!(active, Some((active_query, active_case)) if active_query == query && active_case == match_case)
}

/// Whether a `progress` report is worth pushing given the last pushed one:
/// always the first, always the arrival at 1.0, otherwise only a move of at
/// least 0.02. Pure, so the quiet rule is unit-testable without a browser.
fn progress_should_push(last: Option<f32>, next: f32) -> bool {
    match last {
        None => true,
        Some(last) => (next >= 1.0 && last < 1.0) || (next - last).abs() >= 0.02,
    }
}

/// The [`PageSecurity`] of `url` from the visible navigation entry's SSL
/// state. Makes no AppKit calls and holds no lock; callers lock only to push
/// the resulting event.
fn security_for(url: &str, browser: Option<&Browser>) -> PageSecurity {
    let secure = browser
        .and_then(|b| b.host())
        .and_then(|host| host.visible_navigation_entry())
        .and_then(|entry| entry.sslstatus())
        .map(|status| status.is_secure_connection() == 1);
    PageSecurity::classify(url, secure)
}

/// The current host zoom level as a Chrome-step percent, read off the
/// browser host (a CEF call: never under the lock). Zoom is per-origin in
/// Chromium, so address changes and load ends re-read it — `set_zoom` alone
/// would go stale on navigation.
fn zoom_for(browser: Option<&Browser>) -> u16 {
    let level = browser.and_then(|b| b.host()).map(|host| host.zoom_level()).unwrap_or(0.0);
    zoom_level_to_percent(level)
}

/// Pushes `security` unless it repeats the last pushed value.
fn push_security_deduped(shared: &mut Shared, security: PageSecurity) {
    if shared.last_security == Some(security) {
        return;
    }
    shared.last_security = Some(security);
    shared.events.push(WebEvent::Security(security));
}

/// Maps engine permission bits to [`PermissionKind`]s, in the
/// [`PermissionRequest`] order (camera, microphone, geolocation,
/// notifications, clipboard, MIDI, then the rest by bit, one [`PermissionKind::Other`]
/// per unknown set bit). Pure, so the mapping is unit-testable without a
/// browser.
pub fn permission_kinds_from_bits(bits: u32) -> Vec<PermissionKind> {
    let camera = PermissionRequestTypes::CAMERA_STREAM.get_raw() | PermissionRequestTypes::CAMERA_PAN_TILT_ZOOM.get_raw();
    let microphone = PermissionRequestTypes::MIC_STREAM.get_raw();
    let geolocation = PermissionRequestTypes::GEOLOCATION.get_raw();
    let notifications = PermissionRequestTypes::NOTIFICATIONS.get_raw();
    let clipboard = PermissionRequestTypes::CLIPBOARD.get_raw();
    let midi = PermissionRequestTypes::MIDI_SYSEX.get_raw();
    let mut kinds = Vec::new();
    if bits & camera != 0 {
        kinds.push(PermissionKind::Camera);
    }
    if bits & microphone != 0 {
        kinds.push(PermissionKind::Microphone);
    }
    if bits & geolocation != 0 {
        kinds.push(PermissionKind::Geolocation);
    }
    if bits & notifications != 0 {
        kinds.push(PermissionKind::Notifications);
    }
    if bits & clipboard != 0 {
        kinds.push(PermissionKind::Clipboard);
    }
    if bits & midi != 0 {
        kinds.push(PermissionKind::Midi);
    }
    let rest = bits & !(camera | microphone | geolocation | notifications | clipboard | midi);
    for i in 0..32 {
        if rest & (1u32 << i) != 0 {
            kinds.push(PermissionKind::Other(1u32 << i));
        }
    }
    kinds
}

/// Maps media-access permission bits to [`PermissionKind`]s: device and
/// desktop video capture read as camera, device and desktop audio capture as
/// microphone, anything else as [`PermissionKind::Other`]. Pure, so the
/// mapping is unit-testable without a browser.
pub fn media_kinds_from_bits(bits: u32) -> Vec<PermissionKind> {
    let audio =
        MediaAccessPermissionTypes::DEVICE_AUDIO_CAPTURE.get_raw() | MediaAccessPermissionTypes::DESKTOP_AUDIO_CAPTURE.get_raw();
    let video =
        MediaAccessPermissionTypes::DEVICE_VIDEO_CAPTURE.get_raw() | MediaAccessPermissionTypes::DESKTOP_VIDEO_CAPTURE.get_raw();
    let mut kinds = Vec::new();
    if bits & video != 0 {
        kinds.push(PermissionKind::Camera);
    }
    if bits & audio != 0 {
        kinds.push(PermissionKind::Microphone);
    }
    let rest = bits & !(audio | video);
    for i in 0..32 {
        if rest & (1u32 << i) != 0 {
            kinds.push(PermissionKind::Other(1u32 << i));
        }
    }
    kinds
}

/// Whether a popup disposition asks for the foreground tab: a new foreground
/// tab — and a new window, which would have shown itself — does; background
/// tabs, plain popups and everything else open behind. Pure, so the rule is
/// unit-testable without a browser.
fn popup_foreground(disposition: WindowOpenDisposition) -> bool {
    disposition == WindowOpenDisposition::NEW_FOREGROUND_TAB || disposition == WindowOpenDisposition::NEW_WINDOW
}

/// Forgets every pending permission prompt (both kinds), answering the ids
/// the host must drop. Sorted, so the dismissals report deterministically.
/// Dropping a stored callback un-runs it: the engine already moved on
/// (navigated, terminated, closed), matching `on_dismiss_permission_prompt`.
/// Holds no lock itself; pure over the page state, so the cleanup is
/// unit-testable without CEF.
fn clear_permissions(shared: &mut Shared) {
    if shared.permissions.is_empty() {
        return;
    }
    let mut ids: Vec<u64> = shared.permissions.keys().copied().collect();
    ids.sort_unstable();
    shared.permissions.clear();
    for id in ids {
        shared.events.push(WebEvent::PermissionDismissed(id));
    }
}

/// The [`DownloadState`] for an update that arrived before
/// `on_before_download` staged any metadata: what the item reported, with
/// blanks where only metadata knows (URL, file name, path). Pure, so the
/// early-terminal rule is unit-testable without CEF.
fn early_download_state(id: u32, received: u64, total: Option<u64>, state: DownloadStatus) -> DownloadState {
    DownloadState { id, url: String::new(), file_name: String::new(), path: String::new(), received, total, state }
}

/// Applies an update for `id` that arrived before `on_before_download`
/// staged metadata. A terminal update (complete, cancelled, failed) reports
/// what is known and keeps no callback; anything still downloading only parks
/// its callback for the metadata to meet. Pure over the page state (the
/// callback travels as `Option`, so tests exercise the decision with `None`).
fn apply_pre_meta_update(
    shared: &mut Shared,
    id: u32,
    received: u64,
    total: Option<u64>,
    state: DownloadStatus,
    callback: Option<DownloadItemCallback>,
) {
    match state {
        DownloadStatus::Downloading => {
            if let Some(callback) = callback {
                shared.dl_callbacks.insert(id, callback);
            }
        }
        _ => {
            shared.dl_callbacks.remove(&id);
            shared.events.push(WebEvent::Download(early_download_state(id, received, total, state)));
        }
    }
}

/// Picks a free name for `base` under `dir` and reserves it with an empty
/// `create_new` file before CEF is told to write there, so concurrent
/// same-name downloads cannot clobber each other. A lost reservation race
/// numbers up and retries; any other filesystem error keeps the last name
/// and lets the download itself report the failure. Not pure (filesystem),
/// so tests run it against a scratch directory.
fn reserve_download_path(dir: &std::path::Path, base: &str, taken: &dyn Fn(&str) -> bool) -> (String, std::path::PathBuf) {
    let mut file_name = unique_download_name(taken, base);
    loop {
        let path = dir.join(&file_name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return (file_name, path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let previous = file_name.clone();
                file_name = unique_download_name(&|name| name == previous.as_str() || taken(name), base);
            }
            Err(e) => {
                eprintln!("aui-cef: could not reserve {}: {e}", path.display());
                return (file_name, path);
            }
        }
    }
}

/// Whether a terminal update must remove the reserved placeholder: cancelled
/// before any data arrived, the empty `create_new` file is still ours. A
/// completed or failed download owns its bytes, and any arrivals mean CEF
/// wrote. Pure, so the rule is unit-testable without CEF.
fn download_placeholder_cleanup(state: DownloadStatus, received: u64) -> bool {
    state == DownloadStatus::Cancelled && received == 0
}

// ------------------------------------------------------------------ CEF handlers

wrap_client! {
    struct AuiClient {
        shared: Page,
    }

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(AuiDisplay::new(self.shared.clone()))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(AuiLoad::new(self.shared.clone()))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(AuiLifeSpan::new(self.shared.clone()))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(AuiRequest::new(self.shared.clone()))
        }

        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(AuiPermissions::new(self.shared.clone()))
        }

        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(AuiDownloads::new(self.shared.clone()))
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(AuiContextMenu::new(self.shared.clone()))
        }

        fn audio_handler(&self) -> Option<AudioHandler> {
            Some(AuiAudio::new(self.shared.clone()))
        }

        fn find_handler(&self) -> Option<FindHandler> {
            Some(AuiFind::new(self.shared.clone()))
        }

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(AuiKeyboard::new(()))
        }

        fn command_handler(&self) -> Option<CommandHandler> {
            Some(AuiCommands::new(()))
        }
    }
}

wrap_display_handler! {
    struct AuiDisplay {
        shared: Page,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            let title = title.map(|t| t.to_string()).unwrap_or_default();
            locked(&self.shared).events.push(WebEvent::Title(title));
        }

        fn on_address_change(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if frame.map(|f| f.is_main() == 1).unwrap_or(true) {
                let url = url.map(|u| u.to_string()).unwrap_or_default();
                // The entry's SSL state and the host zoom level are CEF
                // calls: read both before locking.
                let security = security_for(&url, browser.as_deref());
                let zoom = zoom_for(browser.as_deref());
                let mut s = locked(&self.shared);
                s.events.push(WebEvent::Url(url));
                push_security_deduped(&mut s, security);
                s.events.push(WebEvent::Zoom(zoom));
            }
        }

        fn on_favicon_urlchange(&self, _browser: Option<&mut Browser>, icon_urls: Option<&mut CefStringList>) {
            let first = icon_urls.and_then(|list| {
                if cef::string_list_size(Some(&mut *list)) == 0 {
                    return None;
                }
                let mut value = CefString::from("");
                if cef::string_list_value(Some(&mut *list), 0, Some(&mut value)) == 1 {
                    Some(value.to_string())
                } else {
                    None
                }
            });
            // Repeats of the icon the pane already shows stay quiet.
            let mut s = locked(&self.shared);
            if s.last_favicon == Some(first.clone()) {
                return;
            }
            s.last_favicon = Some(first.clone());
            s.events.push(WebEvent::Favicon(first));
        }

        fn on_loading_progress_change(&self, _browser: Option<&mut Browser>, progress: f64) {
            let progress = progress.clamp(0.0, 1.0) as f32;
            // Sub-step repeats stay quiet; the arrival at 1.0 always lands.
            let mut s = locked(&self.shared);
            if !progress_should_push(s.last_progress, progress) {
                return;
            }
            s.last_progress = Some(progress);
            s.events.push(WebEvent::Progress(progress));
        }
    }
}

wrap_load_handler! {
    struct AuiLoad {
        shared: Page,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: ::std::os::raw::c_int,
            can_go_back: ::std::os::raw::c_int,
            can_go_forward: ::std::os::raw::c_int,
        ) {
            let mut s = locked(&self.shared);
            s.can_back = can_go_back == 1;
            s.can_forward = can_go_forward == 1;
            s.events.push(WebEvent::Loading(is_loading == 1));
        }

        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _http_status_code: ::std::os::raw::c_int,
        ) {
            if frame.map(|f| f.is_main() == 1).unwrap_or(true) {
                // The entry read and the host zoom level are CEF calls:
                // finish both before locking.
                let security = match browser.as_deref().and_then(|b| b.host()) {
                    Some(host) => {
                        let entry = host.visible_navigation_entry();
                        let url =
                            entry.as_ref().map(|e| CefString::from(&e.url()).to_string()).unwrap_or_default();
                        let secure = entry
                            .as_ref()
                            .and_then(|e| e.sslstatus())
                            .map(|status| status.is_secure_connection() == 1);
                        PageSecurity::classify(&url, secure)
                    }
                    None => PageSecurity::Unknown,
                };
                let zoom = zoom_for(browser.as_deref());
                let mut s = locked(&self.shared);
                push_security_deduped(&mut s, security);
                // Zoom is per-origin: the load may have landed on an origin
                // with its own level, so re-read it here, not just on
                // address changes.
                s.events.push(WebEvent::Zoom(zoom));
            }
        }
    }
}

wrap_life_span_handler! {
    struct AuiLifeSpan {
        shared: Page,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else { return };
            let mut observer = AuiDevTools::new(self.shared.clone());
            let registration = browser.host().and_then(|host| host.add_dev_tools_message_observer(Some(&mut observer)));
            {
                let mut s = locked(&self.shared);
                s.browser = Some(browser.clone());
                s.registration = registration;
            }
            send(&self.shared, "Runtime.enable", json!({}), Pending::Setup { last: false });
            send(&self.shared, "Runtime.addBinding", json!({ "name": BINDING }), Pending::Setup { last: false });
            let source = format!("{IPC_PRELUDE}\n{ANNOTATOR_JS}");
            if send(&self.shared, "Page.addScriptToEvaluateOnNewDocument", json!({ "source": source }), Pending::Setup { last: true }).is_none() {
                // No DevTools channel: load anyway, unannotated.
                release_first_navigation(&self.shared);
            }
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            let mut s = locked(&self.shared);
            s.closed = true;
            s.registration = None;
            s.browser = None;
            // A closed page answers nothing: forget every pending prompt.
            clear_permissions(&mut s);
        }

        // Announces a popup without cancelling it: the page is created here
        // (its own hidden child-window host, bound to a fresh client so the
        // popup browser never reports to the opener) and hosted as a tab in
        // `on_popup_browser_view_created`. Holds no lock across CEF calls.
        #[allow(clippy::too_many_arguments)]
        fn on_before_popup(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            // CEF reads first: the popup URL and the opener URL are engine
            // calls, never made under the page Mutex.
            let url = target_url.map(|u| u.to_string()).unwrap_or_default();
            let opener_url = browser
                .as_deref()
                .and_then(|b| b.main_frame())
                .map(|f| CefString::from(&f.url()).to_string())
                .unwrap_or_default();
            let foreground = popup_foreground(target_disposition);
            let parent_view = locked(&self.shared).parent_view;
            // The popup's own page: hidden at zero size until the host tabs
            // it, with no pending URL (CEF is already navigating it — see
            // `new_popup_shared`). Its client is swapped in below so the
            // popup browser binds here — never to the opener — while
            // `window.opener` still points at the opener.
            let page: Page = Arc::new(Mutex::new(new_popup_shared(parent_view)));
            OPEN.with(|open| open.borrow_mut().push(Arc::downgrade(&page)));
            if let Some(client) = client {
                *client = Some(AuiClient::new(page.clone()));
            }
            locked(&self.shared).pending_popups.push((popup_id, QueuedPopup { page, url, opener_url, foreground }));
            // Never cancel: cancelling would break `window.opener`. The popup
            // becomes a host tab in `on_popup_browser_view_created`, so no
            // top-level window ever appears either.
            0
        }

        fn on_before_popup_aborted(&self, _browser: Option<&mut Browser>, popup_id: ::std::os::raw::c_int) {
            locked(&self.shared).pending_popups.retain(|(id, _)| *id != popup_id);
        }
    }
}

wrap_request_handler! {
    struct AuiRequest {
        shared: Page,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // A main-frame navigation leaves every pending prompt behind.
            let main = frame.map(|f| f.is_main() == 1).unwrap_or(true);
            if main {
                clear_permissions(&mut locked(&self.shared));
            }
            let url = request.map(|r| CefString::from(&r.url()).to_string()).unwrap_or_default();
            // Same scheme rule as the wry backend: a page (or an agent clicking
            // in it) never reaches `javascript:`, `data:`, app schemes.
            // chrome:// stays reachable for the browser's own pages.
            if navigation_allowed(&url) || url.starts_with("chrome://") || url.starts_with("devtools://") {
                0
            } else {
                1
            }
        }

        fn on_render_process_terminated(
            &self,
            _browser: Option<&mut Browser>,
            status: TerminationStatus,
            error_code: ::std::os::raw::c_int,
            error_string: Option<&CefString>,
        ) {
            let detail = error_string.map(|e| e.to_string()).unwrap_or_default();
            let reason = crash_reason(status, error_code, &detail);
            let mut s = locked(&self.shared);
            // A dead renderer answers nothing: forget every pending prompt.
            clear_permissions(&mut s);
            // A dead renderer never finishes its load; end it now so the
            // toolbar leaves its loading state. `reload` recovers the page.
            // The load edge goes first: the crash is the batch's last word,
            // so a fold that clears crash state on load edges still shows it.
            s.events.push(WebEvent::Loading(false));
            s.events.push(WebEvent::Crashed(reason));
        }
    }
}

// Turns engine permission asks into the host's own UI. Both entry points
// return 1 (handled) so Chromium shows no bubble of its own; the host
// answers later through [`WebBackend::answer_permission`], which runs the
// stored callback as a CEF UI task — never under the page Mutex.
wrap_permission_handler! {
    struct AuiPermissions {
        shared: Page,
    }

    impl PermissionHandler {
        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            prompt_id: u64,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> ::std::os::raw::c_int {
            // Plain copies first; the lock only stores and queues.
            let origin = requesting_origin.map(|o| o.to_string()).unwrap_or_default();
            let kinds = permission_kinds_from_bits(requested_permissions);
            let mut s = locked(&self.shared);
            if let Some(callback) = callback {
                s.permissions.insert(prompt_id, StoredPermission::Prompt(callback.clone()));
            }
            s.events.push(WebEvent::Permission(PermissionRequest { id: prompt_id, origin, kinds }));
            1
        }

        fn on_dismiss_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            prompt_id: u64,
            _result: PermissionRequestResult,
        ) {
            let mut s = locked(&self.shared);
            s.permissions.remove(&prompt_id);
            s.events.push(WebEvent::PermissionDismissed(prompt_id));
        }

        fn on_request_media_access_permission(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut MediaAccessCallback>,
        ) -> ::std::os::raw::c_int {
            let origin = requesting_origin.map(|o| o.to_string()).unwrap_or_default();
            let kinds = media_kinds_from_bits(requested_permissions);
            let mut s = locked(&self.shared);
            // Media access has no engine id: mint one far above the prompt
            // range instead of risking a clash with `prompt_id`s.
            if s.next_permission_id < 1u64 << 48 {
                s.next_permission_id = 1u64 << 48;
            }
            let id = s.next_permission_id;
            s.next_permission_id += 1;
            if let Some(callback) = callback {
                s.permissions.insert(id, StoredPermission::Media { callback: callback.clone(), allowed: requested_permissions });
            }
            s.events.push(WebEvent::Permission(PermissionRequest { id, origin, kinds }));
            1
        }
    }
}

// Saves downloads to the host-chosen folder with no dialog, reporting them
// as [`WebEvent::Download`]. `can_download` always proceeds; the path is
// picked (and the callback run) synchronously in `on_before_download`;
// `on_download_updated` coalesces progress to one report per id per
// 250 ms except state changes. Holds no lock across CEF or filesystem calls.
wrap_download_handler! {
    struct AuiDownloads {
        shared: Page,
    }

    impl DownloadHandler {
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            _url: Option<&CefString>,
            _request_method: Option<&CefString>,
        ) -> ::std::os::raw::c_int {
            1
        }

        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> ::std::os::raw::c_int {
            let (Some(item), Some(callback)) = (download_item, callback) else { return 0 };
            // CEF reads first — all outside the lock.
            let id = item.id();
            let url = CefString::from(&item.url()).to_string();
            let suggested = suggested_name.map(|name| name.to_string()).unwrap_or_default();
            let dir = current_download_dir();
            // Names held by in-flight downloads count as taken, and the
            // suggestion is sanitised before it ever meets `dir.join`.
            let inflight: Vec<String> = locked(&self.shared).dl_meta.values().map(|meta| meta.file_name.clone()).collect();
            let base = sanitize_download_name(&suggested);
            let taken = |name: &str| dir.join(name).exists() || inflight.iter().any(|held| held == name);
            // Reserve the chosen path before `cont`, so a concurrent
            // same-name download numbers up instead of clobbering. CEF
            // writes the download into the `cont` path, truncating the empty
            // reservation file — unverifiable without a live browser, so if
            // relay ever shows CEF refusing an existing path instead, switch
            // this reservation to a `<name>.baazpart` sentinel (reserved but
            // never handed to CEF) and remove it on terminal updates.
            let (file_name, reserved) = reserve_download_path(&dir, &base, &taken);
            let path = reserved.to_string_lossy().into_owned();
            // No dialog (`show_dialog` 0): the host chose the folder.
            callback.cont(Some(&CefString::from(path.as_str())), 0);
            let mut s = locked(&self.shared);
            s.dl_meta.insert(id, DlMeta { url: url.clone(), file_name: file_name.clone(), path: path.clone() });
            s.dl_emit.insert(id, (Instant::now(), DownloadStatus::Downloading));
            s.events.push(WebEvent::Download(DownloadState {
                id,
                url,
                file_name,
                path,
                received: 0,
                total: None,
                state: DownloadStatus::Downloading,
            }));
            1
        }

        fn on_download_updated(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            callback: Option<&mut DownloadItemCallback>,
        ) {
            let (Some(item), Some(callback)) = (download_item, callback) else { return };
            // CEF reads first; nothing below runs under the page Mutex.
            let id = item.id();
            let received = item.received_bytes().max(0) as u64;
            let total = match item.total_bytes() {
                n if n > 0 => Some(n as u64),
                _ => None,
            };
            let state = if item.is_complete() == 1 {
                DownloadStatus::Complete
            } else if item.is_canceled() == 1 {
                DownloadStatus::Cancelled
            } else if item.is_interrupted() == 1 {
                DownloadStatus::Failed
            } else {
                DownloadStatus::Downloading
            };
            let callback = callback.clone();
            let mut s = locked(&self.shared);
            // Updates before `on_before_download` chose a path: a terminal
            // one reports what is known and keeps no callback; anything
            // still downloading only parks its callback.
            if !s.dl_meta.contains_key(&id) {
                apply_pre_meta_update(&mut s, id, received, total, state, Some(callback));
                return;
            }
            let (elapsed, changed) = match s.dl_emit.get(&id) {
                None => (None, true),
                Some((at, known)) => {
                    let elapsed = at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                    (Some(elapsed), *known != state)
                }
            };
            s.dl_callbacks.insert(id, callback);
            if download_emit_due(elapsed, changed) {
                s.dl_emit.insert(id, (Instant::now(), state));
                if let Some(meta) = s.dl_meta.get(&id).cloned() {
                    s.events.push(WebEvent::Download(DownloadState {
                        id,
                        url: meta.url,
                        file_name: meta.file_name,
                        path: meta.path,
                        received,
                        total,
                        state,
                    }));
                }
            }
            if !matches!(state, DownloadStatus::Downloading) {
                // Terminal: the host's list keeps the last event; the live
                // callbacks go away with the download.
                s.dl_callbacks.remove(&id);
                s.dl_emit.remove(&id);
                let meta = s.dl_meta.remove(&id);
                // Cancelled before any data arrived: the reservation file is
                // still ours — remove it outside the lock.
                let stale = match (&meta, download_placeholder_cleanup(state, received)) {
                    (Some(meta), true) => Some(meta.path.clone()),
                    _ => None,
                };
                drop(s);
                if let Some(stale) = stale {
                    let _ = std::fs::remove_file(&stale);
                }
            }
        }
    }
}

// Turns the context menu's "open in new …" commands into host tabs. The link
// (else media source) URL is copied out in `on_before_context_menu` — params
// die with that call — and the command handler queues an open request plus a
// [`WebEvent::PopupOpened`], returning handled so Chromium opens nothing of
// its own. Every other menu command stays with Chromium. Holds no lock
// across CEF calls.
wrap_context_menu_handler! {
    struct AuiContextMenu {
        shared: Page,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            _model: Option<&mut MenuModel>,
        ) {
            let url = params.map(|p| {
                let link = CefString::from(&p.link_url()).to_string();
                if link.is_empty() { CefString::from(&p.source_url()).to_string() } else { link }
            });
            locked(&self.shared).menu_link = url.filter(|u| !u.is_empty());
        }

        fn on_context_menu_command(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            command_id: ::std::os::raw::c_int,
            _event_flags: EventFlags,
        ) -> ::std::os::raw::c_int {
            let Some(foreground) = crate::keys::context_menu_open_foreground(command_id) else { return 0 };
            // Brief lock, no CEF under it.
            let mut s = locked(&self.shared);
            let Some(url) = s.menu_link.clone() else { return 1 };
            s.open_requests.push((url.clone(), foreground));
            s.events.push(WebEvent::PopupOpened { url, foreground });
            // Handled: Chromium opens nothing of its own.
            1
        }
    }
}

// Hands host-claimed keystrokes to the router before Chromium sees them.
//
// A claimed keystroke is consumed (return 1, with `*is_keyboard_shortcut`
// set where CEF provides it) and delivered on a later main-queue turn via
// `defer` — never inside the CEF callback. The KEYUP (and any CHAR) of a
// consumed key is swallowed too, so the page never sees an orphan keyup.
// Unclaimed keystrokes and the no-router case pass through untouched. Holds
// no lock.
wrap_keyboard_handler! {
    struct AuiKeyboard {
        _unit: (),
    }

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            _browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: *mut u8,
            is_keyboard_shortcut: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let Some(event) = event else { return 0 };
            if event.type_ == KeyEventType::KEYUP || event.type_ == KeyEventType::CHAR {
                let is_up = event.type_ == KeyEventType::KEYUP;
                if !crate::keys::swallow_trailer(event.native_key_code, is_up) {
                    return 0;
                }
                if let Some(flag) = is_keyboard_shortcut {
                    *flag = 1;
                }
                return 1;
            }
            let Some(keystroke) = crate::keys::keystroke_of(event) else { return 0 };
            if !crate::keys::route_pre_key_event(keystroke, event.native_key_code) {
                return 0;
            }
            if let Some(flag) = is_keyboard_shortcut {
                *flag = 1;
            }
            1
        }
    }
}

// Blocks Chromium's own accelerators (⌘N, ⌘T, …) so no Chrome window or tab
// Baaz does not own can open. Each blocked id is recorded for
// `crate::keys::take_blocked_commands`. Holds no lock.
wrap_command_handler! {
    struct AuiCommands {
        _unit: (),
    }

    impl CommandHandler {
        fn on_chrome_command(
            &self,
            _browser: Option<&mut Browser>,
            command_id: ::std::os::raw::c_int,
            _disposition: WindowOpenDisposition,
        ) -> ::std::os::raw::c_int {
            if crate::keys::chrome_command_allowed(command_id) {
                return 0;
            }
            crate::keys::record_blocked_command(command_id);
            1
        }
    }
}

// Reports page audio on the tab. CEF 154 has no `on_audio_state_change` —
// the audio handler's stream callbacks are the only audio signal, so open
// streams are counted (started +1, stopped or errored −1) and the tab reads
// as audible while any is open. Returning 1 from `audio_parameters` leaves
// the parameters as given and keeps capture running — the default 0 would
// cancel it and these callbacks would never fire. Holds no lock.
wrap_audio_handler! {
    struct AuiAudio {
        shared: Page,
    }

    impl AudioHandler {
        fn audio_parameters(
            &self,
            _browser: Option<&mut Browser>,
            _params: Option<&mut AudioParameters>,
        ) -> ::std::os::raw::c_int {
            1
        }

        fn on_audio_stream_started(
            &self,
            _browser: Option<&mut Browser>,
            _params: Option<&AudioParameters>,
            _channels: ::std::os::raw::c_int,
        ) {
            let mut s = locked(&self.shared);
            let (next, event) = audio_update(s.audio_streams, 1);
            s.audio_streams = next;
            if let Some(audible) = event {
                s.events.push(WebEvent::Audible(audible));
            }
        }

        fn on_audio_stream_stopped(&self, _browser: Option<&mut Browser>) {
            let mut s = locked(&self.shared);
            let (next, event) = audio_update(s.audio_streams, -1);
            s.audio_streams = next;
            if let Some(audible) = event {
                s.events.push(WebEvent::Audible(audible));
            }
        }

        fn on_audio_stream_error(&self, _browser: Option<&mut Browser>, _message: Option<&CefString>) {
            // An errored stream is gone like a stopped one.
            let mut s = locked(&self.shared);
            let (next, event) = audio_update(s.audio_streams, -1);
            s.audio_streams = next;
            if let Some(audible) = event {
                s.events.push(WebEvent::Audible(audible));
            }
        }
    }
}

// Turns find-in-page results into the find strip's state ("3 of 12").
// Results carry no query, so they are tagged with the latest `find`
// command's. Holds no lock.
wrap_find_handler! {
    struct AuiFind {
        shared: Page,
    }

    impl FindHandler {
        fn on_find_result(
            &self,
            _browser: Option<&mut Browser>,
            _identifier: ::std::os::raw::c_int,
            count: ::std::os::raw::c_int,
            _selection_rect: Option<&Rect>,
            active_match_ordinal: ::std::os::raw::c_int,
            final_update: ::std::os::raw::c_int,
        ) {
            let found = {
                let s = locked(&self.shared);
                FindState {
                    query: s.find_session.clone().map(|(query, _)| query).unwrap_or_default(),
                    active: active_match_ordinal.max(0) as u32,
                    total: count.max(0) as u32,
                    final_update: final_update == 1,
                }
            };
            locked(&self.shared).events.push(WebEvent::Find(found));
        }
    }
}

wrap_dev_tools_message_observer! {
    struct AuiDevTools {
        shared: Page,
    }

    impl DevToolsMessageObserver {
        fn on_dev_tools_message(&self, _browser: Option<&mut Browser>, message: Option<&[u8]>) -> ::std::os::raw::c_int {
            if let Some(message) = message {
                on_devtools(&self.shared, message);
            }
            1
        }
    }
}

/// Peeks at the oldest popup `page` staged on, as `(popup_id, popup page)`.
/// The entry stays staged: `on_popup_browser_view_created` claims it by id,
/// so an abort in between still finds and closes it. Brief lock, no CEF.
fn peek_oldest_staged(page: &Page) -> Option<(i32, Page)> {
    locked(page).pending_popups.first().map(|(id, queued)| (*id, queued.page.clone()))
}

/// The oldest staged popup on any open page, as `(owner, popup_id, popup
/// page)`: the fallback when the parent view's delegate is bound to a page
/// with nothing staged of its own. Brief locks, no CEF.
fn staged_elsewhere() -> Option<(Page, i32, Page)> {
    let pages: Vec<Page> = OPEN.with(|open| open.borrow().iter().filter_map(Weak::upgrade).collect());
    pages.into_iter().filter_map(|page| peek_oldest_staged(&page).map(|(id, popup)| (page, id, popup))).next()
}

wrap_browser_view_delegate! {
    struct AuiViewDelegate {
        shared: Page,
        // The page that staged this popup's entry (the opener, or the popup
        // itself for a popup of a popup); the page's own delegate carries
        // itself here.
        owner: Page,
        // The `on_before_popup` id this delegate was made for, carried to
        // `on_popup_browser_view_created` with the popup view. `None` for a
        // page's own delegate and for fallbacks with nothing staged.
        popup_id: Option<i32>,
    }

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn chrome_toolbar_type(&self, _browser_view: Option<&mut BrowserView>) -> ChromeToolbarType {
            ChromeToolbarType::NONE
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
        }

        fn delegate_for_popup_browser_view(
            &self,
            _browser_view: Option<&mut BrowserView>,
            _settings: Option<&BrowserSettings>,
            _client: Option<&mut Client>,
            is_devtools: ::std::os::raw::c_int,
        ) -> Option<BrowserViewDelegate> {
            if is_devtools == 1 {
                return None;
            }
            // The page that owns the staged entry — this page first (a popup
            // of a popup stages on its own page, whose delegate is already
            // popup-bound), else any open page still staging one.
            let staged = peek_oldest_staged(&self.shared)
                .map(|(id, popup)| (self.shared.clone(), id, popup))
                .or_else(staged_elsewhere);
            let Some((owner, popup_id, popup)) = staged else {
                // Nothing staged anywhere: stay opener-bound without an id,
                // so the created callback can only decline.
                return Some(AuiViewDelegate::new(self.shared.clone(), self.shared.clone(), None));
            };
            // Bind to the popup's page — never the opener's — so a nested
            // popup stages on (and is claimed from) its own page instead of
            // escaping as a stray top-level window.
            Some(AuiViewDelegate::new(popup, owner, Some(popup_id)))
        }

        // Hosts the popup view in our own frameless child window — hidden at
        // zero size until the host tabs it — and queues it on the page that
        // staged it, returning handled so CEF creates no top-level window.
        // Holds no lock across CEF calls.
        fn on_popup_browser_view_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            popup_browser_view: Option<&mut BrowserView>,
            is_devtools: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            if is_devtools == 1 {
                return 0;
            }
            let Some(popup_browser_view) = popup_browser_view else { return 0 };
            // Claim this popup's staged entry by identity — never "oldest
            // pending" — so concurrent popups cannot take each other's page.
            // A miss means the popup was aborted or already claimed: decline,
            // and there is no attributable entry to clear.
            let queued = {
                let mut owner = locked(&self.owner);
                match self.popup_id.and_then(|id| take_pending_popup_by_id(&mut owner.pending_popups, id)) {
                    Some(queued) => queued,
                    None => return 0,
                }
            };
            // Our own window around the popup view: frameless, never-main,
            // adopted as a child of the host — the same host
            // `AuiWindowDelegate` gives every page. CEF calls, lock released.
            let mut window_delegate = AuiWindowDelegate::new(queued.page.clone(), RefCell::new(Some(popup_browser_view.clone())));
            if window_create_top_level(Some(&mut window_delegate)).is_none() {
                eprintln!("aui-cef: popup window_create_top_level failed");
                // Closed, never queued: `QueuedPopup`'s `Drop` closes the page.
                drop(queued);
                return 0;
            }
            let mut owner = locked(&self.owner);
            owner.events.push(WebEvent::PopupOpened { url: queued.url.clone(), foreground: queued.foreground });
            owner.popups.push(queued);
            1
        }
    }
}

wrap_window_delegate! {
    struct AuiWindowDelegate {
        shared: Page,
        browser_view: RefCell<Option<BrowserView>>,
    }

    impl ViewDelegate {}

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut cef::Window>) {
            let view = self.browser_view.borrow().clone();
            let (Some(window), Some(view)) = (window, view) else { return };
            let mut child = View::from(&view);
            window.add_child_view(Some(&mut child));
            let (parent, child) = {
                let mut s = locked(&self.shared);
                s.window = Some(window.clone());
                s.child_view = window.window_handle() as usize;
                (s.parent_view, s.child_view)
            };
            if child != 0 {
                // Safety: the page's live root view, main thread, no lock held.
                // Flag before `show` so the window can never become main.
                unsafe { aui_cef_never_main(child as *mut c_void) };
            }
            place(&self.shared);
            window.show();
            // `show` orders the window front and makes it key: re-assert the
            // host's rect and visibility, and hand the keyboard back — opening
            // a page never takes focus from the host.
            place(&self.shared);
            if parent != 0 {
                // Safety: the host's live content view, main thread.
                unsafe { aui_cef_focus_parent(parent as *mut c_void) };
            }
        }

        fn on_window_destroyed(&self, _window: Option<&mut cef::Window>) {
            *self.browser_view.borrow_mut() = None;
            let mut s = locked(&self.shared);
            s.window = None;
            s.child_view = 0;
            s.closed = true;
        }

        fn can_close(&self, _window: Option<&mut cef::Window>) -> ::std::os::raw::c_int {
            let view = self.browser_view.borrow().clone();
            match view.and_then(|v| v.browser()).and_then(|b| b.host()) {
                Some(host) => host.try_close_browser(),
                None => 1,
            }
        }

        fn is_frameless(&self, _window: Option<&mut cef::Window>) -> ::std::os::raw::c_int {
            1
        }

        fn with_standard_window_buttons(&self, _window: Option<&mut cef::Window>) -> ::std::os::raw::c_int {
            0
        }

        fn initial_bounds(&self, _window: Option<&mut cef::Window>) -> cef::Rect {
            cef::Rect { x: 0, y: 0, width: 800, height: 600 }
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
        }
    }
}

// ------------------------------------------------------------------ the backend

/// A real Chromium page, hosted as a child window over the pane's rect.
pub struct CefBackend {
    page: Page,
}

impl CefBackend {
    /// Opens a page showing `url` over `origin`/`size` (logical pixels,
    /// top-left origin, relative to `parent`'s content) on a ground of `rgba`.
    /// Initialises CEF first if it is not running yet; the page window itself
    /// is created shortly after, in a CEF task.
    pub fn new_at<W: HasWindowHandle>(
        parent: &W,
        url: &str,
        origin: (f32, f32),
        size: (f32, f32),
        rgba: (u8, u8, u8, u8),
    ) -> Result<Self, String> {
        let parent_view = match parent.window_handle().map_err(|e| e.to_string())?.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as usize,
            _ => return Err(String::from("aui-cef: not an AppKit window")),
        };
        crate::runtime::ensure_initialized()?;

        let page: Page = Arc::new(Mutex::new(Shared {
            parent_view,
            bounds: (origin, size),
            visible: true,
            pending_url: Some(url.to_owned()),
            ..Shared::default()
        }));
        OPEN.with(|open| open.borrow_mut().push(Arc::downgrade(&page)));

        // The page window is created in a CEF UI-thread task, run by the pump
        // (inside `CefDoMessageLoopWork`), never in the caller's context: CEF
        // expects Views windows to be created from its own message loop, and
        // the caller is usually inside a gpui update.
        let pending = page.clone();
        on_cef_ui(move || {
            // Dropped before its window existed: nothing to create.
            let closed = locked(&pending).closed;
            if closed {
                return;
            }
            let (r, g, b, a) = rgba;
            let settings = BrowserSettings {
                background_color: (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32,
                ..Default::default()
            };
            let mut client = AuiClient::new(pending.clone());
            // The page's own delegate: owner and id both itself/empty.
            let mut view_delegate = AuiViewDelegate::new(pending.clone(), pending.clone(), None);
            let Some(browser_view) = browser_view_create(
                Some(&mut client),
                Some(&CefString::from("about:blank")),
                Some(&settings),
                None,
                None,
                Some(&mut view_delegate),
            ) else {
                eprintln!("aui-cef: browser_view_create failed");
                return;
            };
            let mut window_delegate = AuiWindowDelegate::new(pending.clone(), RefCell::new(Some(browser_view)));
            if window_create_top_level(Some(&mut window_delegate)).is_none() {
                eprintln!("aui-cef: window_create_top_level failed");
            }
        });
        Ok(Self { page })
    }

    /// Takes the popup pages queued on this page since the last call, oldest
    /// first. The host wraps each in a tab next to the opener.
    pub fn take_popups(&mut self) -> Vec<PopupTab> {
        std::mem::take(&mut locked(&self.page).popups)
            .into_iter()
            .map(|queued| {
                let (page, url, opener_url, foreground) = queued.into_taken();
                PopupTab { backend: CefBackend { page }, url, opener_url, foreground }
            })
            .collect()
    }

    /// Points download saves at `dir` (runtime-wide, for every page),
    /// creating it first when it is missing. Defaults to `~/Downloads`.
    pub fn set_download_dir(dir: PathBuf) {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("aui-cef: could not create download dir {}: {e}", dir.display());
        }
        *locked(download_dir_slot()) = dir;
    }
}

impl Drop for CefBackend {
    fn drop(&mut self) {
        // Queued and still-staging popups orphan with their opener: dropping
        // closes every one of their pages (`QueuedPopup`'s `Drop`).
        let staged = {
            let mut s = locked(&self.page);
            let queued = std::mem::take(&mut s.popups);
            let pending = std::mem::take(&mut s.pending_popups);
            (queued, pending)
        };
        drop(staged);
        close(&self.page);
    }
}

impl WebBackend for CefBackend {
    fn navigate(&mut self, url: &str) {
        let ready = {
            let mut s = locked(&self.page);
            if !s.ready {
                s.pending_url = Some(url.to_owned());
            }
            s.ready
        };
        if ready {
            load(&self.page, url);
        }
    }

    fn back(&mut self) {
        if let Some(b) = browser_of(&self.page) {
            b.go_back();
        }
    }

    fn forward(&mut self) {
        if let Some(b) = browser_of(&self.page) {
            b.go_forward();
        }
    }

    fn reload(&mut self) {
        if let Some(b) = browser_of(&self.page) {
            b.reload();
        }
    }

    fn eval(&mut self, js: &str) {
        if let Some(frame) = browser_of(&self.page).and_then(|b| b.main_frame()) {
            frame.execute_java_script(Some(&CefString::from(js)), Some(&CefString::from("")), 0);
        }
    }

    fn eval_with_result(&mut self, request_id: u64, js: &str) {
        let expression = format!("(async () => {{\n{js}\n}})()");
        let params = json!({ "expression": expression, "awaitPromise": true, "returnByValue": true, "userGesture": true });
        if send(&self.page, "Runtime.evaluate", params, Pending::Eval(request_id)).is_none() {
            locked(&self.page).eval_results.push((request_id, Err(String::from("the page is not ready"))));
        }
    }

    fn take_eval_results(&mut self) -> Vec<(u64, Result<String, String>)> {
        std::mem::take(&mut locked(&self.page).eval_results)
    }

    fn set_annotate(&mut self, on: bool) {
        self.eval(if on { "window.__aui && window.__aui.setAnnotate(true)" } else { "window.__aui && window.__aui.setAnnotate(false)" });
    }

    fn poll_events(&mut self) -> Vec<WebEvent> {
        std::mem::take(&mut locked(&self.page).events)
    }

    fn can_go_back(&self) -> bool {
        locked(&self.page).can_back
    }

    fn can_go_forward(&self) -> bool {
        locked(&self.page).can_forward
    }

    fn element_info(&self, index: usize) -> Option<ElementInfo> {
        locked(&self.page).elements.get(&index).cloned()
    }

    fn is_native(&self) -> bool {
        true
    }

    fn set_bounds(&mut self, origin: (f32, f32), size: (f32, f32)) {
        let changed = {
            let mut s = locked(&self.page);
            let changed = s.bounds != (origin, size);
            s.bounds = (origin, size);
            changed
        };
        if changed {
            place(&self.page);
        }
    }

    fn set_visible(&mut self, visible: bool) {
        if visible == locked(&self.page).visible {
            return;
        }
        if !visible && self.holds_keyboard() {
            // A hidden page that is still key keeps eating keystrokes.
            self.set_focused(false);
        }
        locked(&self.page).visible = visible;
        place(&self.page);
    }

    fn set_focused(&mut self, focused: bool) {
        let (host, parent, child) = {
            let s = locked(&self.page);
            (s.browser.as_ref().and_then(|b| b.host()), s.parent_view, s.child_view)
        };
        if focused && child != 0 {
            // Safety: the page window's live root view, main thread; lock released.
            unsafe { aui_cef_focus_child(child as *mut c_void) };
        }
        if let Some(host) = host {
            host.set_focus(if focused { 1 } else { 0 });
        }
        if !focused && parent != 0 {
            // Safety: the host's live content view, main thread.
            unsafe { aui_cef_focus_parent(parent as *mut c_void) };
        }
    }

    fn holds_keyboard(&self) -> bool {
        let child = locked(&self.page).child_view;
        // Safety: a live NSView address or 0, main thread.
        child != 0 && unsafe { aui_cef_is_key(child as *mut c_void) } == 1
    }

    fn capture(&mut self) {
        send(&self.page, "Page.captureScreenshot", json!({ "format": "png" }), Pending::Screenshot);
    }

    fn stop(&mut self) {
        if let Some(browser) = browser_of(&self.page) {
            browser.stop_load();
        }
    }

    fn set_zoom(&mut self, percent: u16) {
        let level = zoom_percent_to_level(percent);
        if let Some(host) = browser_of(&self.page).and_then(|b| b.host()) {
            host.set_zoom_level(level);
        }
        // CEF emits no zoom-change event; the level just set is the state.
        locked(&self.page).events.push(WebEvent::Zoom(zoom_level_to_percent(level)));
    }

    fn find(&mut self, query: &str, forward: bool, match_case: bool) {
        let find_next = {
            let mut s = locked(&self.page);
            // Only the same query *and* case sensitivity keep the session;
            // anything else — and any search after a stop — starts a new one.
            let next = find_is_next(s.find_session.as_ref().map(|(q, c)| (q.as_str(), *c)), query, match_case);
            s.find_session = Some((query.to_owned(), match_case));
            next
        };
        if let Some(host) = browser_of(&self.page).and_then(|b| b.host()) {
            // The same query again keeps the session and moves (`find_next`);
            // a new query starts a session.
            host.find(
                Some(&CefString::from(query)),
                i32::from(forward),
                i32::from(match_case),
                i32::from(find_next),
            );
        }
    }

    fn stop_find(&mut self) {
        // The session is over: the next `find` starts a new one even with
        // the same query.
        locked(&self.page).find_session = None;
        if let Some(host) = browser_of(&self.page).and_then(|b| b.host()) {
            host.stop_finding(1);
        }
    }

    fn show_devtools(&mut self) {
        // No window info: CEF opens DevTools in its own titled window — never
        // adopted as a child of the page window, never flagged never-main.
        if let Some(host) = browser_of(&self.page).and_then(|b| b.host()) {
            host.show_dev_tools(None, None, None, None);
        }
    }

    fn supports(&self, _feature: BackendFeature) -> bool {
        true
    }

    fn take_popup_backends(&mut self) -> Vec<(String, bool, Box<dyn WebBackend>)> {
        self.take_popups()
            .into_iter()
            .map(|popup| (popup.url, popup.foreground, Box::new(popup.backend) as Box<dyn WebBackend>))
            .collect()
    }

    fn take_open_requests(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut locked(&self.page).open_requests)
    }

    fn answer_permission(&mut self, id: u64, decision: PermissionDecision) {
        let stored = locked(&self.page).permissions.remove(&id);
        let Some(stored) = stored else { return };
        // The callback runs as a CEF UI task: never under the page Mutex.
        on_cef_ui(move || match stored {
            StoredPermission::Prompt(callback) => {
                let result = match decision {
                    PermissionDecision::Allow => PermissionRequestResult::ACCEPT,
                    PermissionDecision::Deny => PermissionRequestResult::DENY,
                    PermissionDecision::Dismiss => PermissionRequestResult::DISMISS,
                };
                callback.cont(result);
            }
            StoredPermission::Media { callback, allowed } => match decision {
                PermissionDecision::Allow => callback.cont(allowed),
                PermissionDecision::Deny | PermissionDecision::Dismiss => callback.cancel(),
            },
        });
    }

    fn cancel_download(&mut self, id: u32) {
        let callback = locked(&self.page).dl_callbacks.remove(&id);
        let Some(callback) = callback else { return };
        // Never under the page Mutex.
        on_cef_ui(move || callback.cancel());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_answers_carry_json_text_or_the_exception_message() {
        assert_eq!(eval_answer(&json!({ "result": { "type": "string", "value": "hi" } })), Ok(String::from("\"hi\"")));
        assert_eq!(eval_answer(&json!({ "result": { "type": "object", "value": { "a": 1 } } })), Ok(String::from("{\"a\":1}")));
        // undefined has no value: the contract says null.
        assert_eq!(eval_answer(&json!({ "result": { "type": "undefined" } })), Ok(String::from("null")));
        let thrown = json!({ "result": { "type": "object" }, "exceptionDetails": { "text": "Uncaught", "exception": { "description": "Error: element not found: x\n    at <anonymous>:3:9" } } });
        assert_eq!(eval_answer(&thrown), Err(String::from("element not found: x")));
    }

    #[test]
    fn annotator_clicks_become_numbered_pending_annotations() {
        let mut s = Shared::default();
        on_annotator_message(&mut s, r#"{"selector":"div.card","label":"div","rect":{"x":1,"y":2,"w":3,"h":4},"outerHTML":"<div>x</div>"}"#);
        on_annotator_message(&mut s, "not json");
        assert_eq!(s.elements.len(), 1);
        assert_eq!(s.elements[&1].selector, "div.card");
        let WebEvent::Annotation(a) = &s.events[0] else { panic!("expected an annotation") };
        assert_eq!(a.index, 1);
        assert!(a.pending);
    }

    #[test]
    fn devtools_results_route_by_id_and_unknown_ids_are_ignored() {
        let page: Page = Arc::new(Mutex::new(Shared::default()));
        locked(&page).pending.insert(10_001, Pending::Eval(7));
        on_devtools(&page, br#"{"id":99,"result":{}}"#);
        on_devtools(&page, br#"{"id":10001,"result":{"result":{"type":"number","value":3}}}"#);
        on_devtools(&page, br#"{"id":10001,"result":{"result":{"type":"number","value":4}}}"#);
        assert_eq!(locked(&page).eval_results, vec![(7, Ok(String::from("3")))]);
    }

    #[test]
    fn a_binding_call_from_the_annotator_lands_as_an_annotation() {
        let page: Page = Arc::new(Mutex::new(Shared::default()));
        let payload = r#"{"selector":"h1","label":"h1","rect":{"x":0,"y":0,"w":10,"h":10},"outerHTML":"<h1/>"}"#;
        let message = json!({ "method": "Runtime.bindingCalled", "params": { "name": BINDING, "payload": payload } }).to_string();
        on_devtools(&page, message.as_bytes());
        // Another page's binding name is not ours.
        let other = json!({ "method": "Runtime.bindingCalled", "params": { "name": "evil", "payload": payload } }).to_string();
        on_devtools(&page, other.as_bytes());
        assert_eq!(locked(&page).elements.len(), 1);
    }

    #[test]
    fn zoom_maps_round_trip_through_every_chrome_step() {
        for step in ZOOM_STEPS {
            let level = zoom_percent_to_level(step);
            assert_eq!(zoom_level_to_percent(level), step, "step {step} must survive the round trip (level {level})");
        }
        assert_eq!(zoom_level_to_percent(0.0), 100);
        assert_eq!(zoom_level_to_percent(f64::NAN), 100);
    }

    #[test]
    fn zoom_snaps_to_chrome_steps() {
        assert_eq!(snap_zoom_percent(24), 25);
        assert_eq!(snap_zoom_percent(120), 125);
        assert_eq!(snap_zoom_percent(149), 150);
        assert_eq!(snap_zoom_percent(1000), 500);
        // `set_zoom` snaps before converting, so near-misses share a level.
        assert_eq!(zoom_percent_to_level(120), zoom_percent_to_level(125));
        assert_eq!(zoom_level_to_percent(zoom_percent_to_level(67)), 67);
    }

    #[test]
    fn crash_reasons_name_the_failure() {
        assert_eq!(crash_reason(TerminationStatus::PROCESS_CRASHED, -1, ""), "the page crashed (-1)");
        assert_eq!(
            crash_reason(TerminationStatus::PROCESS_OOM, -2, "  out of memory  "),
            "the page ran out of memory: out of memory (-2)"
        );
        assert_eq!(crash_reason(TerminationStatus::PROCESS_WAS_KILLED, 0, ""), "the page was killed (0)");
        assert_eq!(
            crash_reason(TerminationStatus::ABNORMAL_TERMINATION, 3, "oops"),
            "the page terminated abnormally: oops (3)"
        );
    }

    #[test]
    fn audible_follows_the_stream_count_not_any_single_stream() {
        // Silence to sound on the first stream.
        assert_eq!(audio_update(0, 1), (1, Some(true)));
        // A second stream changes nothing audible.
        assert_eq!(audio_update(1, 1), (2, None));
        // One of two stopping leaves the tab audible.
        assert_eq!(audio_update(2, -1), (1, None));
        // The last one going quiet is the only other transition.
        assert_eq!(audio_update(1, -1), (0, Some(false)));
        // An error counts like a stop.
        assert_eq!(audio_update(1, -1), (0, Some(false)));
        // The floor holds: stray stops never underflow, never re-announce.
        assert_eq!(audio_update(0, -1), (0, None));
        assert_eq!(audio_update(0, -5), (0, None));
    }

    #[test]
    fn find_next_only_continues_the_live_session() {
        // No session (notably after a stop): a new search, even repeating.
        assert!(!find_is_next(None, "a", false));
        // The same query again keeps the session and moves.
        assert!(find_is_next(Some(("a", false)), "a", false));
        // A different query starts a new search.
        assert!(!find_is_next(Some(("a", false)), "b", false));
        // Toggling the case with the same query re-searches.
        assert!(!find_is_next(Some(("a", false)), "a", true));
        assert!(!find_is_next(Some(("a", true)), "a", false));
        assert!(find_is_next(Some(("a", true)), "a", true));
    }

    #[test]
    fn progress_pushes_moves_and_arrivals_only() {
        assert!(progress_should_push(None, 0.0));
        assert!(!progress_should_push(Some(0.5), 0.51));
        assert!(progress_should_push(Some(0.5), 0.53));
        assert!(!progress_should_push(Some(0.5), 0.49));
        assert!(progress_should_push(Some(0.5), 0.47));
        // The arrival at 1.0 always lands, however small the last step.
        assert!(progress_should_push(Some(0.99), 1.0));
        // … but 1.0 reported twice is not news twice.
        assert!(!progress_should_push(Some(1.0), 1.0));
    }

    #[test]
    fn the_last_setup_answer_releases_the_waiting_url_even_without_a_browser() {
        let page: Page = Arc::new(Mutex::new(Shared { pending_url: Some(String::from("https://a.example/")), ..Shared::default() }));
        locked(&page).pending.insert(10_003, Pending::Setup { last: true });
        on_devtools(&page, br#"{"id":10003,"result":{}}"#);
        let s = locked(&page);
        assert!(s.ready);
        assert!(s.pending_url.is_none(), "taken for loading");
    }

    #[test]
    fn permission_bits_map_to_kinds_in_request_order() {
        use aui_webview::PermissionKind;
        let camera = PermissionRequestTypes::CAMERA_STREAM.get_raw();
        let mic = PermissionRequestTypes::MIC_STREAM.get_raw();
        let geo = PermissionRequestTypes::GEOLOCATION.get_raw();
        let notes = PermissionRequestTypes::NOTIFICATIONS.get_raw();
        let clip = PermissionRequestTypes::CLIPBOARD.get_raw();
        let midi = PermissionRequestTypes::MIDI_SYSEX.get_raw();
        let pan_tilt = PermissionRequestTypes::CAMERA_PAN_TILT_ZOOM.get_raw();
        assert!(permission_kinds_from_bits(0).is_empty());
        assert_eq!(permission_kinds_from_bits(camera), vec![PermissionKind::Camera]);
        // Pan/tilt/zoom is still the camera.
        assert_eq!(permission_kinds_from_bits(pan_tilt), vec![PermissionKind::Camera]);
        assert_eq!(permission_kinds_from_bits(mic), vec![PermissionKind::Microphone]);
        assert_eq!(permission_kinds_from_bits(geo), vec![PermissionKind::Geolocation]);
        assert_eq!(permission_kinds_from_bits(notes), vec![PermissionKind::Notifications]);
        assert_eq!(permission_kinds_from_bits(clip), vec![PermissionKind::Clipboard]);
        assert_eq!(permission_kinds_from_bits(midi), vec![PermissionKind::Midi]);
        // Combined bits keep the request order whatever the input order.
        assert_eq!(
            permission_kinds_from_bits(midi | camera | geo),
            vec![PermissionKind::Camera, PermissionKind::Geolocation, PermissionKind::Midi]
        );
        // Unknown bits survive one by one, after the known kinds.
        let exotic = PermissionRequestTypes::SENSORS.get_raw();
        assert_eq!(permission_kinds_from_bits(exotic), vec![PermissionKind::Other(exotic)]);
        assert_eq!(
            permission_kinds_from_bits(mic | exotic | camera),
            vec![PermissionKind::Camera, PermissionKind::Microphone, PermissionKind::Other(exotic)]
        );
    }

    #[test]
    fn media_bits_map_to_camera_and_microphone() {
        use aui_webview::PermissionKind;
        let device_audio = MediaAccessPermissionTypes::DEVICE_AUDIO_CAPTURE.get_raw();
        let device_video = MediaAccessPermissionTypes::DEVICE_VIDEO_CAPTURE.get_raw();
        let desktop_audio = MediaAccessPermissionTypes::DESKTOP_AUDIO_CAPTURE.get_raw();
        let desktop_video = MediaAccessPermissionTypes::DESKTOP_VIDEO_CAPTURE.get_raw();
        assert!(media_kinds_from_bits(0).is_empty());
        assert_eq!(media_kinds_from_bits(device_audio), vec![PermissionKind::Microphone]);
        assert_eq!(media_kinds_from_bits(device_video), vec![PermissionKind::Camera]);
        assert_eq!(media_kinds_from_bits(desktop_audio), vec![PermissionKind::Microphone]);
        assert_eq!(media_kinds_from_bits(desktop_video), vec![PermissionKind::Camera]);
        assert_eq!(
            media_kinds_from_bits(device_audio | device_video),
            vec![PermissionKind::Camera, PermissionKind::Microphone]
        );
        assert_eq!(media_kinds_from_bits(1u32 << 31), vec![PermissionKind::Other(1u32 << 31)]);
    }

    #[test]
    fn popup_pages_start_with_no_pending_url() {
        // CEF is already navigating the popup: a pending URL would navigate
        // it a second time. Setup still runs; the release only marks ready.
        let shared = new_popup_shared(123);
        assert!(shared.pending_url.is_none());
        assert!(!shared.ready);
        assert!(!shared.visible);
        assert_eq!(shared.parent_view, 123);
    }

    fn staged(id: i32) -> (i32, QueuedPopup) {
        (
            id,
            QueuedPopup {
                page: Arc::new(Mutex::new(Shared::default())),
                url: format!("https://example.com/{id}"),
                opener_url: String::from("https://opener.example/"),
                foreground: false,
            },
        )
    }

    #[test]
    fn staged_popups_claim_by_identity_not_position() {
        let mut pending = vec![staged(3), staged(1), staged(2)];
        // The middle entry claims by id, leaving the rest in order …
        let claimed = take_pending_popup_by_id(&mut pending, 1).expect("staged id 1");
        assert_eq!(claimed.url, "https://example.com/1");
        assert_eq!(pending.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![3, 2]);
        // … an unknown id claims nothing …
        assert!(take_pending_popup_by_id(&mut pending, 9).is_none());
        assert_eq!(pending.len(), 2);
        // … and the oldest goes last when asked for by id.
        let claimed = take_pending_popup_by_id(&mut pending, 3).expect("staged id 3");
        assert_eq!(claimed.url, "https://example.com/3");
    }

    #[test]
    fn clearing_permissions_with_none_pending_stays_quiet() {
        let mut shared = Shared::default();
        clear_permissions(&mut shared);
        assert!(shared.events.is_empty());
    }

    #[test]
    fn answering_an_unknown_permission_is_a_no_op() {
        let page: Page = Arc::new(Mutex::new(Shared::default()));
        let mut backend = CefBackend { page: page.clone() };
        // Unknown ids never reach a CEF task: no callback, no panic, no events.
        backend.answer_permission(999, PermissionDecision::Allow);
        backend.answer_permission(999, PermissionDecision::Dismiss);
        assert!(locked(&page).events.is_empty());
        assert!(locked(&page).permissions.is_empty());
    }

    #[test]
    fn renderer_termination_keeps_its_crash_report_with_no_prompts_pending() {
        let page: Page = Arc::new(Mutex::new(Shared::default()));
        let handler = AuiRequest::new(page.clone());
        handler.on_render_process_terminated(None, TerminationStatus::PROCESS_CRASHED, -1, None);
        let events = std::mem::take(&mut locked(&page).events);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], WebEvent::Loading(false));
        assert!(matches!(&events[1], WebEvent::Crashed(_)));
    }

    #[test]
    fn main_frame_browse_clears_nothing_when_nothing_pends() {
        let page: Page = Arc::new(Mutex::new(Shared::default()));
        let handler = AuiRequest::new(page.clone());
        // No frame reads as main; the empty URL is still blocked as before.
        let decision = handler.on_before_browse(None, None, None, 0, 0);
        assert_eq!(decision, 1);
        assert!(locked(&page).events.is_empty());
    }

    #[test]
    fn early_terminal_download_updates_report_what_is_known_and_keep_no_callback() {
        for state in [DownloadStatus::Complete, DownloadStatus::Cancelled, DownloadStatus::Failed] {
            let mut shared = Shared::default();
            apply_pre_meta_update(&mut shared, 5, 1234, Some(9999), state, None);
            assert!(shared.dl_callbacks.is_empty(), "{state:?} must keep no callback");
            let [WebEvent::Download(reported)] = shared.events.as_slice() else {
                panic!("{state:?} must emit one Download event, got {:?}", shared.events);
            };
            assert_eq!(reported.id, 5);
            assert_eq!(reported.received, 1234);
            assert_eq!(reported.total, Some(9999));
            assert_eq!(reported.state, state);
            // Blanks where only metadata knows.
            assert!(reported.url.is_empty() && reported.file_name.is_empty() && reported.path.is_empty());
        }
    }

    #[test]
    fn early_downloading_updates_stay_quiet_without_a_callback() {
        let mut shared = Shared::default();
        apply_pre_meta_update(&mut shared, 5, 10, None, DownloadStatus::Downloading, None);
        assert!(shared.events.is_empty());
        assert!(shared.dl_callbacks.is_empty());
    }

    #[test]
    fn inflight_download_names_count_as_taken() {
        let inflight = ["a.txt".to_owned()];
        let taken = |name: &str| name == "b.txt" || inflight.iter().any(|held| held == name);
        // On disk (b.txt) and in flight (a.txt) both number up …
        assert_eq!(unique_download_name(&taken, "a.txt"), "a (1).txt");
        assert_eq!(unique_download_name(&taken, "b.txt"), "b (1).txt");
        // … while a free name passes through.
        assert_eq!(unique_download_name(&taken, "c.txt"), "c.txt");
    }

    #[test]
    fn reserving_a_path_creates_it_and_numbers_up_on_collision() {
        let dir = std::env::temp_dir().join(format!("aui-cef-dltest-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let taken = |name: &str| dir.join(name).exists();
        let (first, first_path) = reserve_download_path(&dir, "a.txt", &taken);
        assert_eq!(first, "a.txt");
        assert!(first_path.exists());
        // The reservation itself counts: the next identical download moves on.
        let (second, second_path) = reserve_download_path(&dir, "a.txt", &taken);
        assert_eq!(second, "a (1).txt");
        assert!(second_path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_cancelled_before_data_removes_the_placeholder() {
        assert!(download_placeholder_cleanup(DownloadStatus::Cancelled, 0));
        assert!(!download_placeholder_cleanup(DownloadStatus::Cancelled, 1));
        assert!(!download_placeholder_cleanup(DownloadStatus::Complete, 0));
        assert!(!download_placeholder_cleanup(DownloadStatus::Failed, 0));
        assert!(!download_placeholder_cleanup(DownloadStatus::Downloading, 0));
    }

    #[test]
    fn dropping_the_opener_drops_its_staged_popups() {
        let page: Page = Arc::new(Mutex::new(Shared::default()));
        {
            let mut s = locked(&page);
            s.popups.push(staged(1).1);
            s.pending_popups.push(staged(2));
        }
        // Runs the drain (closing each page, a no-op before CEF init) without
        // hanging or panicking.
        drop(CefBackend { page });
    }

    #[test]
    fn only_foreground_dispositions_take_the_foreground_tab() {
        assert!(popup_foreground(WindowOpenDisposition::NEW_FOREGROUND_TAB));
        // A new window would have shown itself.
        assert!(popup_foreground(WindowOpenDisposition::NEW_WINDOW));
        for disposition in [
            WindowOpenDisposition::UNKNOWN,
            WindowOpenDisposition::CURRENT_TAB,
            WindowOpenDisposition::SINGLETON_TAB,
            WindowOpenDisposition::NEW_BACKGROUND_TAB,
            WindowOpenDisposition::NEW_POPUP,
            WindowOpenDisposition::SAVE_TO_DISK,
        ] {
            assert!(!popup_foreground(disposition), "{disposition:?} should open behind");
        }
    }
}
