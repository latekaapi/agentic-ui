//! [`WryBackend`]: a real WKWebView, behind the `wry` feature.
//!
//! The webview is created as a **child** of the gpui window
//! ([`wry::WebViewBuilder::build_as_child`]), which on macOS means a plain
//! `NSView` added to the window's content view. `gpui::Window` implements
//! [`raw_window_handle::HasWindowHandle`], so no platform code is needed here:
//! the gpui window parents the page directly.
//!
//! # The native-overlay limitation
//!
//! A wry child view is a **native overlay composited above the gpui scene**,
//! not a layer inside it. gpui paints under it and can never paint over it, so
//! **every popover, menu, tooltip and note bubble that would sit over the page
//! is invisible**. There are exactly two ways around it:
//!
//! 1. position the overlay outside the webview's bounds — in the nav row, in
//!    the annotations panel, or in a separate gpui window; or
//! 2. render it *inside the page* through the JS bridge, the way
//!    [`ANNOTATOR_JS`] draws the hover outline.
//!
//! The same applies to the command palette, drag overlays and the loading
//! hairline whenever the browser pane is open. The way out is
//! [`WebBackend::set_visible`]: the pane hides the native view and paints the
//! last [`WebBackend::capture`] in its place, which is what
//! [`crate::view::WebviewState::set_obscured`] does.
//!
//! # Screenshots
//!
//! [`WebBackend::capture`] calls `-[WKWebView
//! takeSnapshotWithConfiguration:completionHandler:]` on the handle
//! [`wry::WebViewExtMacOS::webview`] hands back, and encodes the `NSImage` the
//! completion block delivers as PNG through `NSBitmapImageRep`. The bytes
//! arrive on the queue the poll drains, as
//! [`crate::backend::WebEvent::Screenshot`].
//!
//! The `objc2` crates this needs are pinned to the versions wry 0.55 already
//! resolves to (objc2 0.6, objc2-web-kit 0.3). That is not a nicety: the
//! `Retained<WryWebView>` wry returns is an objc2 0.6 type, and a different
//! objc2 major would make it a different, unrelated Rust type.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aui::workbench::Annotation;
use gpui::SharedString;
use objc2::rc::Retained;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSView};
use objc2_foundation::{NSDictionary, NSError};
use raw_window_handle::HasWindowHandle;
use serde::Deserialize;
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{PageLoadEvent, Rect, WebView, WebViewBuilder, WebViewExtMacOS};

use crate::backend::{ElementInfo, WebBackend, WebEvent, OUTER_HTML_LIMIT};

/// The annotator bridge, injected into every document before it runs.
///
/// In annotate mode it outlines the element under the pointer and, on click,
/// posts the element's selector, box and trimmed `outerHTML` back to Rust
/// through `window.ipc.postMessage`. `window.__aui.setAnnotate(on)` is what
/// [`WebBackend::set_annotate`] calls.
///
/// The outline's colours are literals here on purpose: this is page-side
/// chrome injected before any document loads, so it cannot read the app's
/// palette. They mirror the light palette's accent, which is what the design
/// draws the outline in (`crates/aui/src/workbench/browser.rs`).
pub const ANNOTATOR_JS: &str = r#"
(function () {
  var state = { on: false, hovered: null, box: null, tag: null };
  var LIMIT = 2048;

  function ensure() {
    if (state.box) return;
    state.box = document.createElement('div');
    state.box.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;' +
      'border:1px solid rgba(90,106,255,.6);background:rgba(90,106,255,.06);border-radius:4px;display:none';
    state.tag = document.createElement('div');
    state.tag.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;' +
      'background:#5a6aff;color:#fff;font:500 10px/1 ui-monospace,monospace;' +
      'padding:2px 6px;border-radius:4px 4px 0 0;display:none;white-space:nowrap';
    document.documentElement.appendChild(state.box);
    document.documentElement.appendChild(state.tag);
  }

  function selectorFor(el) {
    if (!el || !el.tagName) return 'unknown';
    var name = el.tagName.toLowerCase();
    if (el.id) return name + '#' + el.id;
    var classes = (el.getAttribute('class') || '').trim().split(/\s+/).filter(Boolean).slice(0, 3);
    return classes.length ? name + '.' + classes.join('.') : name;
  }

  function clear() {
    state.hovered = null;
    if (state.box) { state.box.style.display = 'none'; state.tag.style.display = 'none'; }
  }

  function outline(el) {
    ensure();
    var r = el.getBoundingClientRect();
    state.box.style.display = 'block';
    state.box.style.left = r.left + 'px';
    state.box.style.top = r.top + 'px';
    state.box.style.width = r.width + 'px';
    state.box.style.height = r.height + 'px';
    state.tag.style.display = 'block';
    state.tag.style.left = (r.left - 1) + 'px';
    state.tag.style.top = (r.top - 20) + 'px';
    state.tag.textContent = el.tagName.toLowerCase() + ' · ' +
      Math.round(r.width) + ' × ' + Math.round(r.height);
  }

  document.addEventListener('mousemove', function (e) {
    if (!state.on) return;
    var el = e.target;
    if (!el || el === state.box || el === state.tag) return;
    state.hovered = el;
    outline(el);
  }, true);

  document.addEventListener('click', function (e) {
    if (!state.on) return;
    e.preventDefault();
    e.stopPropagation();
    var el = state.hovered || e.target;
    if (!el || !el.getBoundingClientRect) return;
    var r = el.getBoundingClientRect();
    var html = (el.outerHTML || '');
    window.ipc.postMessage(JSON.stringify({
      selector: selectorFor(el),
      label: el.tagName.toLowerCase(),
      rect: { x: r.left, y: r.top, w: r.width, h: r.height },
      outerHTML: html.length > LIMIT ? html.slice(0, LIMIT) : html
    }));
  }, true);

  window.__aui = {
    setAnnotate: function (on) {
      state.on = !!on;
      document.documentElement.style.cursor = on ? 'crosshair' : '';
      if (!on) clear();
    }
  };
})();
"#;

/// One click, as [`ANNOTATOR_JS`] posts it.
#[derive(Debug, Deserialize)]
struct Hit {
    selector: String,
    label: String,
    rect: HitRect,
    #[serde(rename = "outerHTML")]
    outer_html: String,
}

/// The clicked element's box, in page coordinates.
#[derive(Debug, Deserialize)]
struct HitRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// What the IPC and page-load handlers write into, and [`WryBackend`] reads.
///
/// The handlers are `'static` closures owned by the webview, so everything
/// they touch is shared behind an `Arc<Mutex<…>>`.
#[derive(Default)]
struct Shared {
    events: Mutex<Vec<WebEvent>>,
    /// The metadata side table described in [`crate::backend`], by annotation
    /// number.
    elements: Mutex<HashMap<usize, ElementInfo>>,
    /// Evaluation answers, by request id, for
    /// [`WebBackend::take_eval_results`](crate::backend::WebBackend::take_eval_results).
    eval_results: Mutex<Vec<(u64, Result<String, String>)>>,
    /// The nonce each in-flight evaluation must echo back, by request id:
    /// a page can post anything through `window.ipc.postMessage`, so an
    /// answer is only taken when it carries the nonce this backend minted.
    eval_nonces: Mutex<HashMap<u64, String>>,
}

/// Wraps `js` so its answer comes back through the IPC channel.
///
/// WKWebView's `evaluateJavaScript` — what wry's
/// `evaluate_script_with_callback` calls — **cannot return a Promise**: an
/// async wrapper's completion value is "an unsupported type" and the callback
/// sees an empty payload. Seen live: every `browser_read` failed with "the
/// script failed to run". So the wrapper runs the host script in an async IIFE
/// (a host script may `await`), and posts `{"__aui_eval": id, "nonce": …, ok,
/// value | error}` through `window.ipc.postMessage` — the channel the annotator
/// already uses — and the statement itself evaluates to `undefined`, which
/// WebKit can always return. [`on_ipc`] routes the answer by id and nonce.
fn wrap_eval(request_id: u64, nonce: &str, js: &str) -> String {
    let nonce = serde_json::to_string(nonce).expect("a string serializes");
    format!(
        "(async () => {{ let m; try {{ const value = await (async () => {{\n{js}\n}})(); \
         m = {{ __aui_eval: {request_id}, nonce: {nonce}, ok: true, value: JSON.stringify(value === undefined ? null : value) }}; }} \
         catch (error) {{ m = {{ __aui_eval: {request_id}, nonce: {nonce}, ok: false, error: String((error && error.message) || error) }}; }} \
         window.ipc.postMessage(JSON.stringify(m)); }})(); undefined;"
    )
}

/// One evaluation answer as the wrapper posts it.
#[derive(serde::Deserialize)]
struct EvalAnswer {
    #[serde(rename = "__aui_eval")]
    id: u64,
    nonce: String,
    ok: bool,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Takes an evaluation answer off the IPC channel when `body` is one that an
/// in-flight request is waiting for (right id, right nonce). Returns whether
/// the message was consumed as an evaluation answer.
fn take_eval_answer(shared: &Shared, body: &str) -> bool {
    let Ok(answer) = serde_json::from_str::<EvalAnswer>(body) else { return false };
    let expected = locked(&shared.eval_nonces).get(&answer.id).cloned();
    if expected.as_deref() != Some(answer.nonce.as_str()) {
        // A forged or stale answer: consumed (it is eval-shaped), never delivered.
        return true;
    }
    locked(&shared.eval_nonces).remove(&answer.id);
    let result = if answer.ok {
        Ok(answer.value.unwrap_or_else(|| String::from("null")))
    } else {
        Err(answer.error.unwrap_or_else(|| String::from("the script failed")))
    };
    locked(&shared.eval_results).push((answer.id, result));
    true
}

/// A fresh nonce: not secret against the page's own scripts (they could
/// wrap `postMessage`), but unguessable in advance, so a page cannot answer
/// a request it has not seen.
fn mint_nonce(request_id: u64) -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(request_id);
    hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    format!("{:016x}", hasher.finish())
}

/// Locks `mutex`, recovering the value if a handler panicked while holding it.
/// A poisoned queue is still a usable queue, and losing the pane over it would
/// be worse than carrying on.
fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Reports a failed webview call without taking the pane down with it.
fn report(what: &str, result: wry::Result<()>) {
    if let Err(error) = result {
        eprintln!("aui-webview: {what} failed: {error}");
    }
}

/// A real page in a `wry` child webview.
pub struct WryBackend {
    webview: WebView,
    shared: Arc<Shared>,
    /// History depth is not observable through wry 0.55, so the pane's back
    /// and forward controls are driven by what this backend has been asked to
    /// do rather than by the page's real history.
    behind: usize,
    ahead: usize,
    /// What the native view was last told; `set_visible` is a no-op when it
    /// would not change anything, so an obscuring overlay can push the same
    /// value every frame.
    visible: bool,
}

impl WryBackend {
    /// Builds a child webview inside `parent` — a `gpui::Window` — showing
    /// `url` at `bounds` (logical pixels, relative to the window).
    ///
    /// The annotator bridge is injected before the first document runs, so a
    /// page is annotatable the moment it loads.
    pub fn new<W: HasWindowHandle>(parent: &W, url: &str, bounds: Rect) -> wry::Result<Self> {
        let shared = Arc::new(Shared::default());
        let ipc = Arc::clone(&shared);
        let loads = Arc::clone(&shared);
        let titles = Arc::clone(&shared);

        let webview = WebViewBuilder::new()
            .with_url(url)
            .with_bounds(bounds)
            // Every navigation the page itself starts (a link, a redirect, a
            // form) meets the same scheme rule a host applies to the URLs it
            // opens: an agent that clicks a `javascript:`/`data:` link gets
            // nowhere.
            .with_navigation_handler(|url: String| navigation_allowed(&url))
            .with_initialization_script(ANNOTATOR_JS)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                on_ipc(&ipc, request.body());
            })
            .with_document_title_changed_handler(move |title| {
                locked(&titles.events).push(WebEvent::Title(title));
            })
            .with_on_page_load_handler(move |event, url| {
                let mut queue = locked(&loads.events);
                match event {
                    PageLoadEvent::Started => queue.push(WebEvent::Loading(true)),
                    PageLoadEvent::Finished => queue.push(WebEvent::Loading(false)),
                }
                queue.push(WebEvent::Url(url));
            })
            .build_as_child(parent)?;

        Ok(Self { webview, shared, behind: 0, ahead: 0, visible: true })
    }

    /// A child webview at `origin` with size `size`, in logical pixels
    /// relative to the window's top-left — the same rectangle
    /// [`WebBackend::set_bounds`] takes, so a host never has to name a `wry`
    /// type to build one.
    pub fn new_at<W: HasWindowHandle>(parent: &W, url: &str, origin: (f32, f32), size: (f32, f32)) -> wry::Result<Self> {
        Self::new(parent, url, logical_rect(origin, size))
    }

    /// The webview itself, for the things only the host can do: moving it with
    /// [`wry::WebView::set_bounds`] when the pane is laid out, hiding it with
    /// [`wry::WebView::set_visible`] when a gpui overlay needs the space, and
    /// opening the dev tools.
    pub fn webview(&self) -> &WebView {
        &self.webview
    }
}

/// Turns one IPC message into an annotation plus its metadata.
fn on_ipc(shared: &Arc<Shared>, body: &str) {
    if take_eval_answer(shared, body) {
        return;
    }
    let hit: Hit = match serde_json::from_str(body) {
        Ok(hit) => hit,
        Err(error) => {
            eprintln!("aui-webview: unreadable annotator message: {error}");
            return;
        }
    };
    let mut elements = locked(&shared.elements);
    let index = elements.len() + 1;
    let mut outer_html = hit.outer_html;
    outer_html.truncate(OUTER_HTML_LIMIT.min(outer_html.len()));
    elements.insert(
        index,
        ElementInfo {
            selector: SharedString::from(hit.selector.clone()),
            label: SharedString::from(hit.label),
            origin: (hit.rect.x, hit.rect.y),
            size: (hit.rect.w, hit.rect.h),
            // A real page reports no source file unless a dev server is mapped,
            // which this backend does not do yet.
            source: SharedString::default(),
            outer_html: SharedString::from(outer_html),
        },
    );
    drop(elements);
    locked(&shared.events).push(WebEvent::Annotation(Annotation::new(index, hit.selector, "").pending(true)));
}

/// Whether the webview — or a view inside it, such as WKWebView's content
/// view — is its window's first responder, and so eats the app's keystrokes.
///
/// wry 0.55 reports no focus change to track: there is no focus callback, the
/// IPC channel carries only what the page's own scripts post, and a native
/// page is hit-tested by AppKit so gpui never sees the mouse-down. A DOM
/// focus/blur listener would not do either — DOM focus is not first
/// responder, and a click on non-focusable content still steals the keyboard.
/// So the check is this live AppKit query at the moment focus matters, not a
/// flag, an event or mouse-down inference.
fn holds_first_responder(webview: &wry::WryWebView) -> bool {
    let Some(window) = webview.window() else { return false };
    let Some(first) = window.firstResponder() else { return false };
    if Retained::as_ptr(&first) as *const () == webview as *const wry::WryWebView as *const () {
        return true;
    }
    // The usual case is a descendant (the content view), not the webview
    // itself; anything that is not a view cannot be inside it.
    match first.downcast::<NSView>() {
        Ok(view) => view.isDescendantOf(webview),
        Err(_) => false,
    }
}

/// A `wry` rectangle from a top-left origin and a size, both logical pixels.
fn logical_rect(origin: (f32, f32), size: (f32, f32)) -> Rect {
    Rect {
        position: LogicalPosition::new(origin.0 as f64, origin.1 as f64).into(),
        size: LogicalSize::new(size.0.max(0.0) as f64, size.1.max(0.0) as f64).into(),
    }
}

impl WebBackend for WryBackend {
    fn navigate(&mut self, url: &str) {
        self.behind += 1;
        self.ahead = 0;
        report("load_url", self.webview.load_url(url));
    }

    fn back(&mut self) {
        if self.behind == 0 {
            return;
        }
        self.behind -= 1;
        self.ahead += 1;
        // wry 0.55 has no history API; the page's own history is the history.
        self.eval("history.back()");
    }

    fn forward(&mut self) {
        if self.ahead == 0 {
            return;
        }
        self.ahead -= 1;
        self.behind += 1;
        self.eval("history.forward()");
    }

    fn reload(&mut self) {
        report("reload", self.webview.reload());
    }

    fn eval(&mut self, js: &str) {
        report("evaluate_script", self.webview.evaluate_script(js));
    }

    fn eval_with_result(&mut self, request_id: u64, js: &str) {
        let nonce = mint_nonce(request_id);
        locked(&self.shared.eval_nonces).insert(request_id, nonce.clone());
        // The answer arrives on the IPC channel (see `wrap_eval`); until then
        // the poll simply sees no result.
        report("evaluate_script", self.webview.evaluate_script(&wrap_eval(request_id, &nonce, js)));
    }

    fn take_eval_results(&mut self) -> Vec<(u64, Result<String, String>)> {
        std::mem::take(&mut *locked(&self.shared.eval_results))
    }

    fn set_annotate(&mut self, on: bool) {
        let js = if on { "window.__aui.setAnnotate(true)" } else { "window.__aui.setAnnotate(false)" };
        self.eval(js);
    }

    fn poll_events(&mut self) -> Vec<WebEvent> {
        std::mem::take(&mut *locked(&self.shared.events))
    }

    fn can_go_back(&self) -> bool {
        self.behind > 0
    }

    fn can_go_forward(&self) -> bool {
        self.ahead > 0
    }

    fn element_info(&self, index: usize) -> Option<ElementInfo> {
        locked(&self.shared.elements).get(&index).cloned()
    }

    fn is_native(&self) -> bool {
        true
    }

    fn set_bounds(&mut self, origin: (f32, f32), size: (f32, f32)) {
        // wry's macOS `set_bounds` flips the y itself against the parent view,
        // so this is the same top-left-origin logical rectangle gpui laid the
        // page area out in.
        report("set_bounds", self.webview.set_bounds(logical_rect(origin, size)));
    }

    fn set_visible(&mut self, visible: bool) {
        if visible == self.visible {
            return;
        }
        if !visible && self.holds_keyboard() {
            // A hidden webview that is still first responder keeps eating
            // every keystroke (seen live: text input dead app-wide after one
            // click in the page), so hand the keyboard back before hiding it.
            self.set_focused(false);
        }
        self.visible = visible;
        report("set_visible", self.webview.set_visible(visible));
    }

    fn set_focused(&mut self, focused: bool) {
        let result = if focused { self.webview.focus() } else { self.webview.focus_parent() };
        report(if focused { "focus" } else { "focus_parent" }, result);
    }

    fn holds_keyboard(&self) -> bool {
        holds_first_responder(&self.webview.webview())
    }

    fn capture(&mut self) {
        let shared = Arc::clone(&self.shared);
        // The completion block runs on the main thread once WebKit has a
        // bitmap; until then the poll simply sees no screenshot event.
        let handler = block2::RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
            if !error.is_null() {
                // Safety: WebKit hands the block a valid NSError or null.
                let message = unsafe { &*error }.localizedDescription();
                eprintln!("aui-webview: takeSnapshot failed: {message}");
                return;
            }
            if image.is_null() {
                return;
            }
            // Safety: non-null means WebKit produced an NSImage for this call,
            // and the block owns it for the duration of the call.
            let image = unsafe { &*image };
            match png_bytes(image) {
                Some(bytes) => locked(&shared.events).push(WebEvent::Screenshot(bytes)),
                // Say the size: a zero-sized snapshot (the page not on screen —
                // a locked display, a hidden view) fails the TIFF step the same
                // way a real encode fault would, and only the size tells them apart.
                None if image.size().width < 1.0 || image.size().height < 1.0 => {
                    // A 0×0 snapshot is "the view has no frame yet", not an
                    // encode fault; the next layout or load takes another.
                }
                None => {
                    let size = image.size();
                    eprintln!(
                        "aui-webview: takeSnapshot produced an image that would not encode as PNG ({}x{} pt)",
                        size.width, size.height
                    );
                }
            }
        });
        // Safety: a null configuration means "the visible viewport", and the
        // block is retained by WebKit for as long as the capture takes.
        unsafe {
            self.webview.webview().takeSnapshotWithConfiguration_completionHandler(None, &handler);
        }
    }
}

/// The schemes a page may navigate to: the web, local files, and
/// `about:` (blank pages). Everything else — `javascript:`, `data:`,
/// `blob:`, `vbscript:`, custom app schemes — is refused. Case and
/// leading whitespace do not smuggle a scheme past the check.
pub fn navigation_allowed(url: &str) -> bool {
    let url = url.trim_start().to_ascii_lowercase();
    ["http://", "https://", "file://", "about:"].iter().any(|scheme| url.starts_with(scheme))
}

/// Encodes an `NSImage` as PNG the long way round: TIFF representation into an
/// `NSBitmapImageRep`, then that rep out as PNG. `NSImage` has no PNG encoder
/// of its own, and this is the encode AppKit itself uses.
fn png_bytes(image: &NSImage) -> Option<Vec<u8>> {
    // First the CGImage route: WebKit's snapshot is backed by a CGImage, and
    // wrapping that directly in a bitmap rep skips the TIFF encode — which,
    // on a WKWebView snapshot, failed every time live
    // (`CGImageDestinationFinalize failed for output type 'public.tiff'`).
    // Safety: a null proposed rect means "the image's own size"; no context,
    // no hints.
    if let Some(cg) = unsafe { image.CGImageForProposedRect_context_hints(std::ptr::null_mut(), None, None) } {
        let rep = NSBitmapImageRep::initWithCGImage(<NSBitmapImageRep as objc2::AnyThread>::alloc(), &cg);
        let properties = NSDictionary::new();
        // Safety: an empty properties dictionary carries no wrongly-typed value.
        if let Some(png) = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties) } {
            return Some(png.to_vec());
        }
    }
    // The TIFF route, kept as the fallback for images with no CGImage.
    let tiff = image.TIFFRepresentation()?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let properties = NSDictionary::new();
    // Safety: the properties dictionary is empty, so it cannot carry a value of
    // the wrong type for a key.
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties) }?;
    Some(png.to_vec())
}

#[cfg(test)]
mod tests {

    #[test]
    fn page_navigations_meet_the_scheme_rule() {
        for ok in ["https://example.com", "http://localhost:3000/x", "file:///tmp/a.html", "about:blank", "HTTPS://A.B"] {
            assert!(super::navigation_allowed(ok), "{ok} should be allowed");
        }
        for bad in ["javascript:alert(1)", " JavaScript:alert(1)", "\tjavascript:x", "data:text/html,<b>x</b>", "blob:https://a/b", "vbscript:x", "baaz://x", ""] {
            assert!(!super::navigation_allowed(bad), "{bad:?} should be refused");
        }
    }

    use super::*;

    /// One IPC message, as `ANNOTATOR_JS` builds it.
    fn message(selector: &str, html: &str) -> String {
        format!(
            r#"{{"selector":"{selector}","label":"div","rect":{{"x":12.5,"y":30,"w":150,"h":118}},"outerHTML":"{html}"}}"#
        )
    }

    #[test]
    fn a_click_becomes_an_annotation_and_its_element() {
        let shared = Arc::new(Shared::default());
        on_ipc(&shared, &message("div.card.starter", "<div>hi</div>"));

        let events = std::mem::take(&mut *locked(&shared.events));
        let WebEvent::Annotation(annotation) = &events[0] else { panic!("expected an annotation, got {events:?}") };
        assert_eq!(events.len(), 1);
        // The first pin is number 1, and it opens pending so its note can be
        // typed before it is saved.
        assert_eq!(annotation.index, 1);
        assert_eq!(annotation.selector, "div.card.starter");
        assert!(annotation.pending);

        let info = locked(&shared.elements).get(&1).cloned().expect("the side table");
        assert_eq!(info.label, "div");
        assert_eq!(info.origin, (12.5, 30.0));
        assert_eq!(info.size, (150.0, 118.0));
        assert_eq!(info.outer_html, "<div>hi</div>");
        // A real page reports no source file: nothing maps it to one yet.
        assert!(info.source.is_empty());
    }

    #[test]
    fn pins_are_numbered_in_the_order_they_are_clicked() {
        let shared = Arc::new(Shared::default());
        on_ipc(&shared, &message("h1", "<h1/>"));
        on_ipc(&shared, &message("p", "<p/>"));
        let indices: Vec<usize> = locked(&shared.elements).keys().copied().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        assert_eq!(indices, vec![1, 2]);
        assert_eq!(locked(&shared.elements)[&2].selector, "p");
    }

    #[test]
    fn oversized_outer_html_is_trimmed_to_the_limit() {
        let shared = Arc::new(Shared::default());
        let big = "x".repeat(OUTER_HTML_LIMIT * 2);
        on_ipc(&shared, &message("div", &big));
        assert_eq!(locked(&shared.elements)[&1].outer_html.len(), OUTER_HTML_LIMIT);
    }

    #[test]
    fn an_unreadable_message_is_dropped_rather_than_panicking() {
        let shared = Arc::new(Shared::default());
        // Not JSON at all, JSON of the wrong shape, and a rect with a missing
        // field: a page can post anything through `window.ipc.postMessage`.
        on_ipc(&shared, "not json");
        on_ipc(&shared, r#"{"selector":"div"}"#);
        on_ipc(&shared, r#"{"selector":"div","label":"div","rect":{"x":1,"y":2,"w":3},"outerHTML":""}"#);
        assert!(locked(&shared.events).is_empty());
        assert!(locked(&shared.elements).is_empty());
    }

    #[test]
    fn the_eval_wrapper_posts_over_ipc_and_returns_undefined() {
        let wrapped = wrap_eval(7, "n0nce", "return document.title;");
        assert!(wrapped.contains("return document.title;"));
        assert!(wrapped.contains("window.ipc.postMessage"));
        assert!(wrapped.contains("__aui_eval: 7"));
        assert!(wrapped.contains("\"n0nce\""));
        // WebKit cannot return a Promise from evaluateJavaScript: the
        // statement must end on a plain value.
        assert!(wrapped.trim_end().ends_with("undefined;"));
    }

    #[test]
    fn eval_answers_route_by_id_and_nonce() {
        let shared = Arc::new(Shared::default());
        locked(&shared.eval_nonces).insert(3, String::from("abc"));
        // Forged (wrong nonce) and unknown ids are consumed, never delivered.
        assert!(take_eval_answer(&shared, r#"{"__aui_eval":3,"nonce":"zzz","ok":true,"value":"1"}"#));
        assert!(take_eval_answer(&shared, r#"{"__aui_eval":9,"nonce":"abc","ok":true,"value":"1"}"#));
        assert!(locked(&shared.eval_results).is_empty());
        // The real one lands once.
        assert!(take_eval_answer(&shared, r#"{"__aui_eval":3,"nonce":"abc","ok":true,"value":"{\"a\":1}"}"#));
        assert_eq!(locked(&shared.eval_results).as_slice(), &[(3, Ok(String::from("{\"a\":1}")))]);
        assert!(take_eval_answer(&shared, r#"{"__aui_eval":3,"nonce":"abc","ok":true,"value":"2"}"#));
        assert_eq!(locked(&shared.eval_results).len(), 1, "a replay is not a second answer");
        // Errors carry their message; annotator messages are not eval answers.
        locked(&shared.eval_nonces).insert(4, String::from("d"));
        assert!(take_eval_answer(&shared, r#"{"__aui_eval":4,"nonce":"d","ok":false,"error":"element not found: x"}"#));
        assert_eq!(locked(&shared.eval_results)[1], (4, Err(String::from("element not found: x"))));
        assert!(!take_eval_answer(&shared, r#"{"selector":"div"}"#));
    }

    #[test]
    fn a_bounds_rectangle_survives_the_round_trip_into_wry() {
        let rect = logical_rect((10.0, 20.0), (300.0, 200.0));
        // wry takes logical pixels and flips the y against the parent view
        // itself, so what goes in is exactly what gpui measured.
        assert_eq!(rect.position.to_logical::<f64>(1.0), wry::dpi::LogicalPosition::new(10.0, 20.0));
        assert_eq!(rect.size.to_logical::<f64>(1.0), wry::dpi::LogicalSize::new(300.0, 200.0));
        // A pane laid out at zero (or a negative overflow) must not become a
        // negative frame.
        let empty = logical_rect((0.0, 0.0), (-5.0, 0.0));
        assert_eq!(empty.size.to_logical::<f64>(1.0), wry::dpi::LogicalSize::new(0.0, 0.0));
    }
}
