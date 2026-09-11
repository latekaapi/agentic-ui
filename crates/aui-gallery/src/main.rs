//! # aui-gallery
//!
//! Native macOS gallery for the Agentic UI library. Every design card from
//! `design/src/cards` is a live entry; the title bar switches theme and
//! density; `--screenshot <entry> <out.png>` renders one entry at the card's
//! declared size for the parity loop in `docs/03-parity-process.md`, and
//! `--idle-frames <ms>` counts the frames a settled card still draws (see
//! [`idle`]).
//!
//! ```bash
//! cargo run -p aui-gallery
//! cargo run -p aui-gallery -- --theme light
//! cargo run -p aui-gallery -- --screenshot foundations/colour /tmp/colour.png
//! cargo run -p aui-gallery -- --entry transcript/turns --idle-frames 2000
//! cargo run -p aui-gallery -- --list
//! ```

mod assistant;
mod cards;
mod gallery;
mod idle;
mod registry;
mod shot;

use std::path::PathBuf;

use aui_tokens::ThemeKind;
use gpui_kit::component::Root;
use gpui_kit::*;

use gallery::Gallery;
use registry::{Entry, ENTRIES};

/// Parsed command line.
struct Args {
    theme: Option<ThemeKind>,
    entry: Option<&'static Entry>,
    screenshot: Option<PathBuf>,
    /// `--screenshot-window`: capture the whole gallery window (chrome included).
    window_shot: Option<PathBuf>,
    /// `--screenshot-delay <ms>`: how long to wait before capturing (default 350).
    delay_ms: u64,
    /// `--text-scale <factor>`: text size multiplier. Defaults to the product
    /// default (1.1) interactively and to 1.0 for parity screenshots.
    text_scale: Option<f32>,
    /// `--idle-frames <ms>`: count the frames the settled card draws over this
    /// window, print the count and quit. See [`idle`].
    idle_ms: Option<u64>,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut out = Args { theme: None, entry: None, screenshot: None, window_shot: None, delay_ms: 350, text_scale: None, idle_ms: None };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => {
                let value = args.next().unwrap_or_default();
                out.theme = Some(ThemeKind::parse(&value).unwrap_or_else(|| usage(&format!("unknown theme `{value}`"))));
            }
            "--entry" => {
                let id = args.next().unwrap_or_default();
                out.entry = Some(registry::find(&id).unwrap_or_else(|| usage(&format!("unknown entry `{id}`"))));
            }
            "--screenshot" => {
                let id = args.next().unwrap_or_default();
                let path = args.next().unwrap_or_else(|| usage("--screenshot needs <entry> <out.png>"));
                out.entry = Some(registry::find(&id).unwrap_or_else(|| usage(&format!("unknown entry `{id}`"))));
                out.screenshot = Some(PathBuf::from(path));
            }
            "--screenshot-window" => {
                let path = args.next().unwrap_or_else(|| usage("--screenshot-window needs <out.png>"));
                out.window_shot = Some(PathBuf::from(path));
            }
            "--screenshot-delay" => {
                let value = args.next().unwrap_or_default();
                out.delay_ms = value.parse().unwrap_or_else(|_| usage("--screenshot-delay needs milliseconds"));
            }
            "--idle-frames" => {
                let value = args.next().unwrap_or_default();
                out.idle_ms = Some(value.parse().unwrap_or_else(|_| usage("--idle-frames needs milliseconds")));
            }
            "--text-scale" => {
                let value = args.next().unwrap_or_default();
                out.text_scale = Some(value.parse().unwrap_or_else(|_| usage("--text-scale needs a factor such as 1.1")));
            }
            "--list" => {
                for e in ENTRIES {
                    println!("{:<32} {:>4}×{:<4} {:?}  {}", e.id, e.width, e.height, e.theme, e.title);
                }
                std::process::exit(0);
            }
            "-h" | "--help" => usage(""),
            other => usage(&format!("unknown argument `{other}`")),
        }
    }
    out
}

fn usage(err: &str) -> ! {
    if !err.is_empty() {
        eprintln!("error: {err}\n");
    }
    eprintln!(
        "usage: aui-gallery [--theme light|dark] [--entry <id>] [--screenshot <id> <out.png>] [--screenshot-window <out.png>] [--screenshot-delay <ms>] [--idle-frames <ms>] [--text-scale <factor>] [--list]"
    );
    std::process::exit(if err.is_empty() { 0 } else { 2 });
}

fn main() {
    let args = parse_args();
    let screenshot = args.screenshot.clone();
    let window_shot = args.window_shot.clone();
    let delay = std::time::Duration::from_millis(args.delay_ms);
    let text_scale = args
        .text_scale
        .unwrap_or(if args.screenshot.is_some() { 1.0 } else { aui_tokens::scale::TEXT_SCALE });
    let entry = args.entry;
    let idle_ms = args.idle_ms;
    // The idle count has to see the card exactly as a capture does: bare, at
    // the card's declared size, with the pointer occluded.
    let bare_mode = args.screenshot.is_some();
    // Parity screenshots default to the theme the card was designed in;
    // otherwise the harness default (dark) unless overridden.
    let theme = args
        .theme
        .or_else(|| screenshot.as_ref().and(entry).and_then(|e| e.theme.fixed_kind()))
        .unwrap_or(ThemeKind::Dark);

    gpui_kit::application().with_assets(aui::assets::AuiAssets).run(move |cx| {
        aui::init(theme, cx);
        aui_tokens::AuiTheme::set_text_scale(text_scale, None, cx);

        let window_size = match (bare_mode, entry) {
            (true, Some(e)) => size(px(e.width), px(e.height)),
            _ => size(px(1280.0), px(820.0)),
        };
        // Parity renders sit at the display's top-left corner, away from the
        // pointer, so no hover state leaks into the capture.
        let bounds = if bare_mode {
            Bounds::new(point(px(0.0), px(0.0)), window_size)
        } else {
            Bounds::centered(None, window_size, cx)
        };
        let options = if bare_mode {
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                is_resizable: false,
                // The idle count needs the window on screen and drawing;
                // a capture wants the pointer and focus kept away from it.
                focus: idle_ms.is_some(),
                ..Default::default()
            }
        } else {
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(720.0), px(480.0))),
                ..gpui_kit::component::TitleBar::window_options()
            }
        };

        let shot_mode = bare_mode;
        let handle = cx
            .open_window(options, |window, cx| {
                let view = cx.new(|cx| Gallery::new(entry, shot_mode, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("open gallery window");

        if let Some(ms) = idle_ms {
            cx.activate(true);
            let label = entry.map(|e| e.id.to_string()).unwrap_or_else(|| "first".to_string());
            idle::count_and_quit(label, delay, std::time::Duration::from_millis(ms), cx);
        } else if let Some(path) = screenshot.or(window_shot) {
            shot::capture_and_quit(handle, path, delay, cx);
        } else {
            cx.activate(true);
        }
    });
}
