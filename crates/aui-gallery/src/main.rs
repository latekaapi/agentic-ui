//! # aui-gallery
//!
//! Native macOS gallery for the Agentic UI library. Every design card from
//! `design/src/cards` is a live entry; the title bar switches theme and
//! density; `--screenshot <entry> <out.png>` renders one entry at the card's
//! declared size for the parity loop in `docs/03-parity-process.md`.
//!
//! ```bash
//! cargo run -p aui-gallery
//! cargo run -p aui-gallery -- --theme light
//! cargo run -p aui-gallery -- --screenshot foundations/colour /tmp/colour.png
//! cargo run -p aui-gallery -- --list
//! ```

mod assistant;
mod cards;
mod gallery;
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
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut out = Args { theme: None, entry: None, screenshot: None, window_shot: None, delay_ms: 350, text_scale: None };
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
        "usage: aui-gallery [--theme light|dark] [--entry <id>] [--screenshot <id> <out.png>] [--screenshot-window <out.png>] [--screenshot-delay <ms>] [--text-scale <factor>] [--list]"
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
    // Parity screenshots default to the theme the card was designed in;
    // otherwise the harness default (dark) unless overridden.
    let theme = args
        .theme
        .or_else(|| screenshot.as_ref().and(entry).and_then(|e| e.theme.fixed_kind()))
        .unwrap_or(ThemeKind::Dark);

    gpui_kit::application().with_assets(aui::assets::AuiAssets).run(move |cx| {
        aui::init(theme, cx);
        aui_tokens::AuiTheme::set_text_scale(text_scale, None, cx);

        let window_size = match (&screenshot, entry) {
            (Some(_), Some(e)) => size(px(e.width), px(e.height)),
            _ => size(px(1280.0), px(820.0)),
        };
        // Parity renders sit at the display's top-left corner, away from the
        // pointer, so no hover state leaks into the capture.
        let bounds = if screenshot.is_some() {
            Bounds::new(point(px(0.0), px(0.0)), window_size)
        } else {
            Bounds::centered(None, window_size, cx)
        };
        let options = if screenshot.is_some() {
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                is_resizable: false,
                focus: false,
                ..Default::default()
            }
        } else {
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(720.0), px(480.0))),
                ..gpui_kit::component::TitleBar::window_options()
            }
        };

        let shot_mode = screenshot.is_some();
        let handle = cx
            .open_window(options, |window, cx| {
                let view = cx.new(|cx| Gallery::new(entry, shot_mode, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("open gallery window");

        if let Some(path) = screenshot.or(window_shot) {
            shot::capture_and_quit(handle, path, delay, cx);
        } else {
            cx.activate(true);
        }
    });
}
