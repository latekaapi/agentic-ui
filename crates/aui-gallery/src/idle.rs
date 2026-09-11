//! `--idle-frames <ms>`: how many frames a card draws once it has settled.
//!
//! A component that keeps an animation mounted while it is at rest asks gpui
//! for a frame on every render, which keeps the whole window redrawing for as
//! long as the card is on screen. There is no way to see that in a
//! screenshot, so this mode opens the card exactly as `--screenshot` does,
//! lets it settle for `--screenshot-delay`, then counts the frames it draws
//! over the next `<ms>` and prints the count.
//!
//! ```bash
//! cargo run -p aui-gallery -- --entry transcript/turns --idle-frames 2000
//! # IDLE transcript/turns frames=0 over 2000ms
//! ```
//!
//! A settled card prints `frames=0`. Anything else names a component that is
//! still asking for frames at rest.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use gpui::App;

/// Frames drawn since the counter was last reset. The gallery view bumps it
/// once per `Render::render`.
pub static FRAMES: AtomicU64 = AtomicU64::new(0);

/// Counts one frame.
pub fn count_frame() {
    FRAMES.fetch_add(1, Ordering::Relaxed);
}

/// Waits `settle`, resets the counter, counts for `window`, prints and quits.
pub fn count_and_quit(label: String, settle: Duration, window: Duration, cx: &mut App) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(settle).await;
        let settled_at = FRAMES.swap(0, Ordering::Relaxed);
        cx.background_executor().timer(window).await;
        let frames = FRAMES.load(Ordering::Relaxed);
        println!(
            "IDLE {label} frames={frames} over {}ms (settle drew {settled_at})",
            window.as_millis()
        );
        cx.update(|cx| cx.quit());
    })
    .detach();
}
