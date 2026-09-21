//! Feed throughput bench for the Stage L grid (`--bench feed`).
//!
//! Four measurements, each printed on one line as `name value unit`, using
//! only [`std::time::Instant`]:
//!
//! - `plain_feed`: MiB/s of plain output fed through [`TerminalSession`]
//!   (no markers),
//! - `marked_feed`: the same with OSC 133 markers every ~20 lines, so the
//!   mark-scanner cost is visible,
//! - `feed_before_sat` / `feed_after_sat`: the per-feed cost before and
//!   after the scrollback saturates (feeds past 10,000 lines) — the case a
//!   recent fix changed,
//! - `blocks_500`: block assembly cost with ~500 blocks live.
//!
//! Takes no arguments and needs no fixtures. The whole bench runs in a few
//! seconds, well under 30.

#[cfg(feature = "tui")]
fn main() {
    use std::collections::VecDeque;
    use std::path::Path;
    use std::sync::Mutex;
    use std::time::Instant;

    use aui_terminal::{TermEvent, TerminalBackend, TerminalSession};

    /// A refillable queue backend: the bench pushes output, then pumps.
    struct SharedQueue {
        queue: std::sync::Arc<Mutex<VecDeque<Vec<u8>>>>,
    }

    impl TerminalBackend for SharedQueue {
        fn spawn(&mut self, _shell: &str, _cwd: &Path) -> std::io::Result<()> {
            Ok(())
        }

        fn write(&mut self, _bytes: &[u8]) {}

        fn resize(&mut self, _cols: u16, _rows: u16) {}

        fn poll(&mut self) -> Vec<TermEvent> {
            self.queue.lock().unwrap().drain(..).map(TermEvent::Output).collect()
        }
    }

    /// A nonce-pinned session the bench feeds in batches, one pump per feed.
    struct Live {
        session: TerminalSession,
        queue: std::sync::Arc<Mutex<VecDeque<Vec<u8>>>>,
    }

    impl Live {
        fn feed(&self, bytes: &[u8]) {
            self.queue.lock().unwrap().push_back(bytes.to_vec());
            self.session.pump();
        }
    }

    /// The scrollback cap the grid retains (see `grid.rs`): past it, old
    /// lines are trimmed on a schedule.
    const SCROLLBACK: usize = 10_000;

    fn live(nonce: &str) -> Live {
        let queue = std::sync::Arc::new(Mutex::new(VecDeque::new()));
        let session =
            TerminalSession::new(Box::new(SharedQueue { queue: queue.clone() }), 100, 32)
                .with_nonce(nonce);
        Live { session, queue }
    }

    /// One nonce-checked marker's raw bytes.
    fn marker(nonce: &str, body: &str) -> Vec<u8> {
        format!("\x1b]133;{body};k={nonce}\x07").into_bytes()
    }

    /// A finished command with `lines` output lines between `C` and `D`.
    fn command(nonce: &str, echo: &str, exit: i32, lines: usize) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(marker(nonce, "A"));
        out.extend(b"bench$ ");
        out.extend(marker(nonce, "B"));
        out.extend(echo.as_bytes());
        out.extend(b"\r\n");
        out.extend(marker(nonce, "C"));
        for i in 0..lines {
            out.extend(format!("bench-fill-{i:05} padding padding padding padding padding\r\n").into_bytes());
        }
        out.extend(marker(nonce, &format!("D;{exit}")));
        out
    }

    /// `lines` plain output lines of ~64 bytes each.
    fn plain_lines(start: usize, lines: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..lines {
            out.extend(
                format!("plain-fill-{:06} padding padding padding padding\r\n", start + i)
                    .into_bytes(),
            );
        }
        out
    }

    const NONCE: &str = "feedbench01";

    // 1. Plain output: 20,000 lines (~1.2 MiB) in 500-line feeds.
    {
        let h = live(NONCE);
        let mut bytes = 0usize;
        let t = Instant::now();
        for feed in 0..40 {
            let chunk = plain_lines(feed * 500, 500);
            bytes += chunk.len();
            h.feed(&chunk);
        }
        let mib = bytes as f64 / (1024.0 * 1024.0);
        println!("plain_feed {:.1} MiB/s", mib / t.elapsed().as_secs_f64());
    }

    // 2. The same volume with OSC 133 markers every ~20 lines (1,000
    // commands of 20 lines each, 10 commands per feed).
    {
        let h = live(NONCE);
        let mut bytes = 0usize;
        let t = Instant::now();
        for feed in 0..100 {
            let mut chunk = Vec::new();
            for c in 0..10 {
                chunk.extend(command(NONCE, &format!("cmd-{}-{}", feed, c), 0, 20));
            }
            bytes += chunk.len();
            h.feed(&chunk);
        }
        let mib = bytes as f64 / (1024.0 * 1024.0);
        println!("marked_feed {:.1} MiB/s", mib / t.elapsed().as_secs_f64());
    }

    // 3. Scrollback saturation: 500-line feeds past 10,000 lines; the feeds
    // fully below the cap report separately from the ones past it.
    {
        let h = live(NONCE);
        let mut before: Vec<f64> = Vec::new();
        let mut after: Vec<f64> = Vec::new();
        let mut lines = 0usize;
        for feed in 0..30 {
            let chunk = plain_lines(feed * 500, 500);
            let t = Instant::now();
            h.feed(&chunk);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            lines += 500;
            if lines <= SCROLLBACK {
                before.push(ms);
            } else if lines > SCROLLBACK + 1_000 {
                after.push(ms);
            }
        }
        let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
        println!("feed_before_sat {:.3} ms", mean(&before));
        println!("feed_after_sat {:.3} ms", mean(&after));
    }

    // 4. Block assembly with ~500 blocks live: 500 tiny commands, then one
    // more feed plus the block read, timed together.
    {
        let h = live(NONCE);
        for c in 0..500 {
            h.feed(&command(NONCE, &format!("tiny-{c}"), 0, 1));
        }
        assert_eq!(h.session.blocks().len(), 500);
        let t = Instant::now();
        h.feed(&command(NONCE, "tiny-final", 0, 1));
        let _ = h.session.blocks();
        println!("blocks_500 {:.3} ms", t.elapsed().as_secs_f64() * 1000.0);
    }
}

#[cfg(not(feature = "tui"))]
fn main() {
    println!("feed bench needs aui-terminal/tui");
    std::process::exit(2);
}
