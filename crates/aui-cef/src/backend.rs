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
//! The page window is additionally isa-swizzled at creation
//! (`aui_cef_never_main`) so it can become key but never main, keeping the
//! host window's traffic lights coloured.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Weak};

use aui::workbench::Annotation;
use aui_webview::backend::OUTER_HTML_LIMIT;
use aui_webview::wry_backend::navigation_allowed;
use aui_webview::{ElementInfo, WebBackend, WebEvent, ANNOTATOR_JS};
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
}

fn locked(shared: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    match shared.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

type Page = Arc<Mutex<Shared>>;

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

        fn on_address_change(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if frame.map(|f| f.is_main() == 1).unwrap_or(true) {
                let url = url.map(|u| u.to_string()).unwrap_or_default();
                locked(&self.shared).events.push(WebEvent::Url(url));
            }
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
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
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
    }
}

// Hands host-claimed keystrokes to the router before Chromium sees them.
//
// A claimed keystroke is consumed (return 1, with `*is_keyboard_shortcut`
// set where CEF provides it) and delivered on a later main-queue turn via
// `defer` — never inside the CEF callback. Unclaimed keystrokes and the
// no-router case pass through untouched. Holds no lock.
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
            let Some(keystroke) = crate::keys::keystroke_of(event) else { return 0 };
            if !crate::keys::route_pre_key_event(keystroke) {
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

wrap_browser_view_delegate! {
    struct AuiViewDelegate {
        _unit: (),
    }

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn chrome_toolbar_type(&self, _browser_view: Option<&mut BrowserView>) -> ChromeToolbarType {
            ChromeToolbarType::NONE
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::CHROME
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
                // Swizzle before `show` so the window can never become main.
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
            let mut view_delegate = AuiViewDelegate::new(());
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
}

impl Drop for CefBackend {
    fn drop(&mut self) {
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
        let (host, parent) = {
            let s = locked(&self.page);
            (s.browser.as_ref().and_then(|b| b.host()), s.parent_view)
        };
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
    fn the_last_setup_answer_releases_the_waiting_url_even_without_a_browser() {
        let page: Page = Arc::new(Mutex::new(Shared { pending_url: Some(String::from("https://a.example/")), ..Shared::default() }));
        locked(&page).pending.insert(10_003, Pending::Setup { last: true });
        on_devtools(&page, br#"{"id":10003,"result":{}}"#);
        let s = locked(&page);
        assert!(s.ready);
        assert!(s.pending_url.is_none(), "taken for loading");
    }
}
