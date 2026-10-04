//! Drives `CefBackend` directly: load, eval (value, number, async, throw),
//! screenshot, history, hide, move, shutdown. Run from inside an app bundle
//! that carries CEF (copy the binary into `X.app/Contents/MacOS/`).
use std::time::{Duration, Instant};

use aui_webview::{WebBackend, WebEvent};
use gpui::{div, prelude::*, px, rgb, size, Bounds, Context, TitlebarOptions, WindowBounds, WindowOptions};

struct Blank;
impl Render for Blank {
    fn render(&mut self, _w: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x1b1d22))
    }
}

fn p(msg: impl AsRef<str>) {
    println!("PROBE {}", msg.as_ref());
}

fn main() {
    let t0 = Instant::now();
    let ok = aui_cef::preload();
    p(format!("preload={ok} in {} ms", t0.elapsed().as_millis()));
    aui_cef::configure(aui_cef::Config {
        cache_dir: std::env::temp_dir().join("aui-cef-probe"),
        mock_keychain: true,
        log_file: None,
    });
    gpui_platform::application().run(move |cx: &mut gpui::App| {
        let bounds = Bounds::centered(None, size(px(1100.), px(760.)), cx);
        let win = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions { title: Some("aui-cef probe".into()), ..Default::default() }),
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
            let backend = cx.update(|cx| {
                win.update(cx, |_, window, _| {
                    aui_cef::CefBackend::new_at(&*window, "https://www.wikipedia.org/", (200.0, 0.0), (900.0, 760.0), (27, 29, 34, 255))
                })
            });
            let mut backend = match backend {
                Ok(Ok(b)) => b,
                Ok(Err(e)) => { p(format!("FAIL new_at: {e}")); cx.update(|cx| cx.quit()); return; }
                Err(e) => { p(format!("FAIL window: {e}")); cx.update(|cx| cx.quit()); return; }
            };
            p(format!("new_at ok in {} ms", started.elapsed().as_millis()));
            // Wait for the first real load to finish.
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut loaded = false;
            while Instant::now() < deadline && !loaded {
                ex.timer(Duration::from_millis(100)).await;
                for ev in backend.poll_events() {
                    match &ev {
                        WebEvent::Loading(false) => { p(format!("event Loading(false) at {} ms", started.elapsed().as_millis())); }
                        WebEvent::Title(t) => { p(format!("event Title({t})")); if t.contains("Wikipedia") { loaded = true; } }
                        WebEvent::Url(u) => p(format!("event Url({u})")),
                        other => p(format!("event {other:?}").chars().take(120).collect::<String>()),
                    }
                }
            }
            p(format!("loaded={loaded}"));
            backend.eval_with_result(1, "return document.title;");
            backend.eval_with_result(2, "return document.querySelectorAll('a').length;");
            backend.eval_with_result(3, "await new Promise(r => setTimeout(r, 50)); return {ok: true, n: [1,2]};");
            backend.eval_with_result(4, "throw new Error('element not found: #nope');");
            backend.capture();
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut got = 0;
            let mut shot = false;
            while Instant::now() < deadline && (got < 4 || !shot) {
                ex.timer(Duration::from_millis(100)).await;
                for (id, r) in backend.take_eval_results() {
                    got += 1;
                    let r = match r { Ok(v) => format!("Ok({})", v.chars().take(80).collect::<String>()), Err(e) => format!("Err({e})") };
                    p(format!("eval {id} -> {r}"));
                }
                for ev in backend.poll_events() {
                    if let WebEvent::Screenshot(bytes) = ev {
                        shot = true;
                        let path = std::env::temp_dir().join("aui-cef-probe.png");
                        let _ = std::fs::write(&path, &bytes);
                        p(format!("screenshot {} bytes, png={} -> {}", bytes.len(), bytes.starts_with(&[0x89, b'P', b'N', b'G']), path.display()));
                    }
                }
            }
            backend.navigate("https://en.wikipedia.org/wiki/Container_ship");
            ex.timer(Duration::from_millis(4000)).await;
            let _ = backend.poll_events();
            p(format!("after second navigation: can_go_back={} can_go_forward={}", backend.can_go_back(), backend.can_go_forward()));
            backend.set_visible(false);
            p("set_visible(false)");
            ex.timer(Duration::from_millis(600)).await;
            p("SHOT hidden");
            ex.timer(Duration::from_millis(1200)).await;
            backend.set_visible(true);
            backend.set_bounds((300.0, 40.0), (700.0, 600.0));
            p("set_visible(true) + set_bounds(300,40,700x600)");
            ex.timer(Duration::from_millis(800)).await;
            p("SHOT moved");
            ex.timer(Duration::from_millis(1500)).await;
            let q = Instant::now();
            drop(backend);
            aui_cef::shutdown(Duration::from_secs(3));
            p(format!("shutdown done in {} ms", q.elapsed().as_millis()));
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
