//! `--screenshot`: render the window off-screen after drawing frames through
//! the settle delay,
//! downsample to logical pixels (the baselines in `design/baseline/` are 1×)
//! and quit.

use std::{path::PathBuf, time::Duration};

use gpui::*;
use gpui_kit::component::Root;

/// Waits for the first frames, captures the window and exits the process.
pub fn capture_and_quit(handle: WindowHandle<Root>, path: PathBuf, delay: Duration, cx: &mut App) {
    cx.spawn(async move |cx| {
        // Give fonts, layout and the first paint time to settle — drawing
        // frames all the while. gpui-pre 0.3.12 paints no frame for a
        // window that is not visibly on screen, and one long timer wakes
        // nothing else: content that lands asynchronously (an image
        // decoded off-thread, terminal output, a menu placed after its
        // first paint, a fade) used to reach the capture only when the
        // pointer happened to sit over the window and its move events
        // woke it. The baselines then depended on where the pointer was
        // (Gate 2, 16 Nov). Drawing through `update_window` every
        // `FRAME` applies whatever landed, so the capture shows the
        // settled card wherever the pointer is.
        const FRAME: Duration = Duration::from_millis(50);
        let draw = |cx: &mut AsyncApp| {
            cx.update(|cx| {
                let _ = AppContext::update_window(cx, handle.into(), |_, window, cx| window.draw(cx).clear(cx));
            });
        };
        let deadline = std::time::Instant::now() + delay;
        while std::time::Instant::now() < deadline {
            draw(cx);
            cx.background_executor().timer(FRAME).await;
        }
        // A few more, for work a frame schedules for the next one (lists
        // that scroll to their tail, blocks sized from the previous frame).
        for _ in 0..4 {
            draw(cx);
            cx.background_executor().timer(FRAME).await;
        }
        let result = cx.update(|cx| {
            handle.update(cx, |_root, window, _cx| {
                let scale = window.scale_factor();
                let image = window.render_to_image()?;
                let (w, h) = (image.width(), image.height());
                let target_w = (w as f32 / scale).round() as u32;
                let target_h = (h as f32 / scale).round() as u32;
                let image = if (target_w, target_h) != (w, h) {
                    image::imageops::resize(&image, target_w, target_h, image::imageops::FilterType::Lanczos3)
                } else {
                    image
                };
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                image.save(&path)?;
                anyhow::Ok((target_w, target_h))
            })
        });
        match result {
            Ok(Ok((w, h))) => println!("wrote {} ({w}×{h})", path.display()),
            Ok(Err(err)) => eprintln!("screenshot failed: {err:#}"),
            Err(err) => eprintln!("screenshot failed: {err:#}"),
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}
