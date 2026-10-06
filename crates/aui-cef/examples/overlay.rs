//! S1 spike: a blurred gpui pop-up window over a live CEF page.
//!
//! Mirrors `probe.rs`: preload, configure, an 1100x760 host window, and a
//! Chromium page (`https://www.wikipedia.org/`) as a child NSWindow over the
//! host at (248,0) size (852,760). Then it opens a SECOND gpui window over
//! the page and probes, through small AppKit shim helpers (`aui_overlay_*`
//! in `src/shim.m`), whether the overlay sits above the page, who holds the
//! keyboard, whether it follows the host, and captures a host screenshot the
//! operator checks for blur by eye.
//!
//! Window-kind choice: `WindowKind::AnchoredPopup` takes a
//! `popup::PopupOptions` (parent handle + anchor), but macOS has no native
//! implementation — `MacPlatform::open_window` rejects it outright with
//! `PopupNotSupportedError` (gpui-pre-macos `platform.rs`). So the spike uses
//! `WindowKind::PopUp` (an `NSPanel` at `NSPopUpWindowLevel` = 101) with
//! `WindowBackgroundAppearance::Blurred` (an `NSVisualEffectView` behind the
//! content). A `PopUp` is NOT anchored, so it is not expected to follow the
//! host when the host moves — a non-follow is the finding, not a hidden
//! failure.
//!
//! AppKit assumptions the PROBE lines rest on: the CEF page window is the
//! host window's first (only) child window; both the overlay and the page
//! windows appear in `[NSWindow windowNumbersWithOptions:0]`, earliest =
//! frontmost; `screencapture -l` takes the host's `windowNumber`.
//!
//! Run from inside an app bundle that carries CEF (copy the binary into
//! `X.app/Contents/MacOS/`). The build gate proves this compiles only —
//! only the operator's bundle run proves the overlay appears, blurs, orders,
//! and focuses as probed.
use std::os::raw::{c_int, c_long, c_void};
use std::time::{Duration, Instant};

use aui_webview::{WebBackend, WebEvent};
use gpui::{
    div, prelude::*, px, rgb, rgba, size, Bounds, Context, Entity, SharedString, TitlebarOptions,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

unsafe extern "C" {
    fn aui_cef_key_window() -> *mut c_void;
    fn aui_overlay_window(view: *mut c_void) -> *mut c_void;
    fn aui_overlay_first_child(view: *mut c_void) -> *mut c_void;
    fn aui_overlay_level(window: *mut c_void) -> c_long;
    fn aui_overlay_is_above(a: *mut c_void, b: *mut c_void) -> c_int;
    fn aui_overlay_is_key(window: *mut c_void) -> c_int;
    fn aui_overlay_move_host(view: *mut c_void, dx: f64, dy: f64);
    fn aui_overlay_frame(window: *mut c_void, x: *mut f64, y: *mut f64, w: *mut f64, h: *mut f64);
    fn aui_overlay_number(window: *mut c_void) -> c_long;
}

fn p(msg: impl AsRef<str>) {
    println!("PROBE {}", msg.as_ref());
}

fn pass(name: &str, ok: bool) {
    p(format!("{} {name}", if ok { "PASS" } else { "FAIL" }));
}

/// The window's AppKit content-view address, for the `aui_overlay_*` probes.
fn ns_view_of<W: HasWindowHandle>(w: &W) -> usize {
    match w.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::AppKit(h)) => h.ns_view.as_ptr() as usize,
        _ => 0,
    }
}

/// The window's frame in screen coordinates, via the shim.
fn frame_of(window: *mut c_void) -> (f64, f64, f64, f64) {
    let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 0.0, 0.0);
    unsafe {
        aui_overlay_frame(window, &mut x, &mut y, &mut w, &mut h);
    }
    (x, y, w, h)
}

/// Which of the three spike windows currently holds the keyboard.
fn key_name(key: *mut c_void, overlay: *mut c_void, page: *mut c_void, host: *mut c_void) -> &'static str {
    if key.is_null() {
        "none"
    } else if key == overlay {
        "overlay"
    } else if key == page {
        "page"
    } else if key == host {
        "host"
    } else {
        "other"
    }
}

struct Blank;
impl Render for Blank {
    fn render(&mut self, _w: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x1b1d22))
    }
}

struct OverlayCard {
    status: Entity<String>,
}

impl Render for OverlayCard {
    fn render(&mut self, _w: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.status.clone();
        let label = SharedString::from(self.status.read(cx).clone());
        div().size_full().flex().items_center().justify_center().child(
            div()
                .id("overlay-card")
                .w(px(360.))
                .rounded(px(12.))
                .bg(rgba(0x23262ed1))
                .border_1()
                .border_color(rgba(0xffffff2e))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(div().text_lg().text_color(rgb(0xffffff)).child("Overlay"))
                .child(div().text_sm().text_color(rgb(0xb8bcc4)).child(label))
                .child(
                    div()
                        .id("take-over")
                        .cursor_pointer()
                        .rounded(px(8.))
                        .bg(rgb(0x3b82f6))
                        .px_4()
                        .py_2()
                        .flex()
                        .justify_center()
                        .text_sm()
                        .text_color(rgb(0xffffff))
                        .child("Take over")
                        .on_click(move |_, _, cx| {
                            status.write(cx, "You're driving".to_string());
                        }),
                ),
        )
    }
}

fn main() {
    const OVERLAY_W: f32 = 400.0;
    const OVERLAY_H: f32 = 520.0;

    let t0 = Instant::now();
    let ok = aui_cef::preload();
    p(format!("preload={ok} in {} ms", t0.elapsed().as_millis()));
    let cache = std::env::temp_dir().join("aui-cef-overlay");
    std::fs::create_dir_all(&cache).ok();
    aui_cef::configure(aui_cef::Config { cache_dir: cache.clone(), mock_keychain: true, log_file: None });
    gpui_platform::application().run(move |cx: &mut gpui::App| {
        let bounds = Bounds::centered(None, size(px(1100.), px(760.)), cx);
        let win = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions { title: Some("aui-cef overlay".into()), ..Default::default() }),
                    focus: false,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Blank),
            )
            .unwrap();
        cx.spawn(async move |cx| {
            let ex = cx.background_executor().clone();
            ex.timer(Duration::from_millis(800)).await;
            let started = Instant::now();
            let setup = cx.update(|cx| {
                let backend = win.update(cx, |_, window, _| {
                    aui_cef::CefBackend::new_at(
                        &*window,
                        "https://www.wikipedia.org/",
                        (248.0, 0.0),
                        (852.0, 760.0),
                        (27, 29, 34, 255),
                    )
                });
                let origin = win.update(cx, |_, window, _| window.bounds().origin);
                let host_view = win.update(cx, |_, window, _| ns_view_of(&*window));
                // Bottom-right over the page: host origin + (1100-400-20, 760-520-72).
                let overlay = match origin {
                    Ok(mut o) => {
                        o.x += px(1100.0 - OVERLAY_W - 20.0);
                        o.y += px(760.0 - OVERLAY_H - 72.0);
                        p(format!("overlay requested origin=({:?},{:?}) size={OVERLAY_W}x{OVERLAY_H}", o.x, o.y));
                        cx.open_window(
                            WindowOptions {
                                window_bounds: Some(WindowBounds::Windowed(Bounds {
                                    origin: o,
                                    size: size(px(OVERLAY_W), px(OVERLAY_H)),
                                })),
                                titlebar: None,
                                focus: false,
                                kind: WindowKind::PopUp,
                                window_background: WindowBackgroundAppearance::Blurred,
                                ..Default::default()
                            },
                            |_, cx| {
                                let status = cx.new(|_| String::from("Idle — page underneath"));
                                cx.new(|_| OverlayCard { status })
                            },
                        )
                        .map_err(|e| e.to_string())
                    }
                    Err(e) => Err(format!("host bounds: {e}")),
                };
                (backend, host_view, overlay)
            });
            let (backend, host_view, overlay) = setup;
            let mut backend = match backend {
                Ok(Ok(b)) => b,
                Ok(Err(e)) => {
                    p(format!("FAIL new_at: {e}"));
                    cx.update(|cx| cx.quit());
                    return;
                }
                Err(e) => {
                    p(format!("FAIL window: {e}"));
                    cx.update(|cx| cx.quit());
                    return;
                }
            };
            p(format!("new_at ok in {} ms", started.elapsed().as_millis()));
            let overlay = match overlay {
                Ok(o) => o,
                Err(e) => {
                    p(format!("FAIL overlay open: {e}"));
                    cx.update(|cx| cx.quit());
                    return;
                }
            };
            p("overlay open ok (WindowKind::PopUp, Blurred)");
            let host_view = match host_view {
                Ok(v) if v != 0 => v,
                _ => {
                    p("FAIL host ns_view");
                    cx.update(|cx| cx.quit());
                    return;
                }
            };
            let overlay_view = match cx.update(|cx| overlay.update(cx, |_, window, _| ns_view_of(&*window))) {
                Ok(v) if v != 0 => v,
                _ => {
                    p("FAIL overlay ns_view");
                    cx.update(|cx| cx.quit());
                    return;
                }
            };
            let host_v = host_view as *mut c_void;
            let overlay_v = overlay_view as *mut c_void;
            let host_w = unsafe { aui_overlay_window(host_v) };
            let overlay_w = unsafe { aui_overlay_window(overlay_v) };
            // Wait for the first real load to finish (also gives CEF time to
            // create the page child window).
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut loaded = false;
            while Instant::now() < deadline && !loaded {
                ex.timer(Duration::from_millis(100)).await;
                for ev in backend.poll_events() {
                    match &ev {
                        WebEvent::Loading(false) => {
                            p(format!("event Loading(false) at {} ms", started.elapsed().as_millis()));
                        }
                        WebEvent::Title(t) => {
                            p(format!("event Title({t})"));
                            if t.contains("Wikipedia") {
                                loaded = true;
                            }
                        }
                        WebEvent::Url(u) => p(format!("event Url({u})")),
                        other => p(format!("event {other:?}").chars().take(120).collect::<String>()),
                    }
                }
            }
            p(format!("loaded={loaded}"));
            // (a) Both windows exist: is the overlay above the page?
            let page_w = unsafe { aui_overlay_first_child(host_v) };
            p(format!("windows host={host_w:p} overlay={overlay_w:p} page={page_w:p}"));
            let level = unsafe { aui_overlay_level(overlay_w) };
            let above = unsafe { aui_overlay_is_above(overlay_w, page_w) };
            let overlay_key = unsafe { aui_overlay_is_key(overlay_w) };
            let page_key = unsafe { aui_overlay_is_key(page_w) };
            p(format!("overlay_level={level} overlay_above_page={above} overlay_is_key={overlay_key} page_is_key={page_key}"));
            pass("overlay above page", above == 1);
            // (b) The page takes the keyboard; the overlay must stay above.
            backend.set_focused(true);
            ex.timer(Duration::from_millis(1000)).await;
            let page_w = unsafe { aui_overlay_first_child(host_v) };
            let above = unsafe { aui_overlay_is_above(overlay_w, page_w) };
            let overlay_key = unsafe { aui_overlay_is_key(overlay_w) };
            let page_key = unsafe { aui_overlay_is_key(page_w) };
            let key = unsafe { aui_cef_key_window() };
            let key_after_page_focus =
                key_name(key, overlay_w, page_w, host_w).to_string();
            p(format!("after page focus: overlay_above_page={above} overlay_is_key={overlay_key} page_is_key={page_key} key={key_after_page_focus}"));
            pass("overlay still above page after page focus", above == 1);
            // (c) Move the host by (40,30); a PopUp is not anchored, so it is
            // expected NOT to follow — a non-follow is the finding.
            let (hx0, hy0, _, _) = frame_of(host_w);
            let (ox0, oy0, _, _) = frame_of(overlay_w);
            unsafe {
                aui_overlay_move_host(host_v, 40.0, 30.0);
            }
            let (mut ox1, mut oy1) = (ox0, oy0);
            for _ in 0..10 {
                ex.timer(Duration::from_millis(100)).await;
                (ox1, oy1, _, _) = frame_of(overlay_w);
                if (ox1 - ox0).abs() > 0.5 || (oy1 - oy0).abs() > 0.5 {
                    break;
                }
            }
            let (hx1, hy1, _, _) = frame_of(host_w);
            let host_moved = (hx1 - hx0 - 40.0).abs() < 2.0 && (hy1 - hy0 - 30.0).abs() < 2.0;
            let follows_host = (ox1 - ox0 - 40.0).abs() < 2.0 && (oy1 - oy0 - 30.0).abs() < 2.0;
            p(format!(
                "host frame ({hx0:.0},{hy0:.0}) -> ({hx1:.0},{hy1:.0}); overlay origin ({ox0:.0},{oy0:.0}) -> ({ox1:.0},{oy1:.0})"
            ));
            pass("host moved by (40,30)", host_moved);
            pass("overlay follows host (anchored only; PopUp is not expected to)", follows_host);
            // (d) Whole-host screenshot for the operator's by-eye blur check.
            let host_num = unsafe { aui_overlay_number(host_w) };
            let shot = cache.join("overlay-host.png");
            match std::process::Command::new("screencapture")
                .arg("-x")
                .arg("-o")
                .arg("-l")
                .arg(host_num.to_string())
                .arg(&shot)
                .output()
            {
                Ok(out) => p(format!("shot path={} status={}", shot.display(), out.status)),
                Err(e) => p(format!("shot failed: {e}")),
            }
            // (e) Hide/show the page once; the overlay must stay above.
            backend.set_visible(false);
            p("set_visible(false)");
            ex.timer(Duration::from_millis(600)).await;
            let page_w = unsafe { aui_overlay_first_child(host_v) };
            let above_hidden = unsafe { aui_overlay_is_above(overlay_w, page_w) };
            p(format!("hidden: overlay_above_page={above_hidden}"));
            backend.set_visible(true);
            ex.timer(Duration::from_millis(800)).await;
            let page_w = unsafe { aui_overlay_first_child(host_v) };
            let above_shown = unsafe { aui_overlay_is_above(overlay_w, page_w) };
            p(format!("reshown: overlay_above_page={above_shown}"));
            let overlay_above = above_shown == 1;
            pass("overlay stays above across hide/show", above_hidden == 1 && overlay_above);
            // (f) Shutdown cleanly, same order as probe.rs.
            let q = Instant::now();
            drop(backend);
            aui_cef::shutdown(Duration::from_secs(3));
            p(format!("shutdown done in {} ms", q.elapsed().as_millis()));
            p(format!(
                "SUMMARY overlay_above={overlay_above} follows_host={follows_host} key_after_page_focus={key_after_page_focus}"
            ));
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
