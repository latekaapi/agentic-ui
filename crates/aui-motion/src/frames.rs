//! Who asked for frames: a count per call site of every frame a motion
//! primitive (or a host routing its own requests through
//! [`request_frame`]) asked gpui for.
//!
//! gpui dirties every ancestor of a notified view, so one element that asks
//! for the next frame on every render holds the whole window at display rate
//! — the always-rendering bug of the 2026-10-16 perf audit (a blinking caret,
//! a spinner beside "Waiting for approval…", 120 root renders a second). A
//! render counter says *that* the window is busy; this says *who* keeps it
//! busy, by `file:line`, so a gate that expects zero frames at rest can name
//! the culprit instead of only failing.
//!
//! Counting is always on and costs one uncontended lock per request; nothing
//! is printed here. Hosts read it with [`take`] (counts since the last take)
//! or [`snapshot`].

use std::collections::HashMap;
use std::panic::Location;
use std::sync::Mutex;

use gpui::Window;

static COUNTS: Mutex<Option<HashMap<&'static Location<'static>, u64>>> = Mutex::new(None);

/// Count one frame asked for at `at`.
pub fn record(at: &'static Location<'static>) {
    let Ok(mut counts) = COUNTS.lock() else { return };
    *counts.get_or_insert_with(HashMap::new).entry(at).or_insert(0) += 1;
}

/// Ask for the next frame, counted against the caller's `file:line`. Use it
/// in place of `window.request_animation_frame()`.
#[track_caller]
pub fn request_frame(window: &mut Window) {
    record(Location::caller());
    window.request_animation_frame();
}

/// One call site and how many frames it asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requester {
    /// `crates/<crate>/src/file.rs:line` (or the path as compiled when it
    /// has no `crates/` segment).
    pub at: String,
    /// Frames asked for.
    pub frames: u64,
}

/// `file` from `crates/` on (`crates/aui/src/…`): a path dependency reports
/// an absolute path and a git one a checkout path, neither of which a log
/// line needs.
fn short(file: &str) -> &str {
    file.rfind("/crates/").map_or(file, |at| &file[at + 1..])
}

fn sorted(counts: &HashMap<&'static Location<'static>, u64>) -> Vec<Requester> {
    let mut out: Vec<Requester> = counts
        .iter()
        .map(|(at, frames)| Requester { at: format!("{}:{}", short(at.file()), at.line()), frames: *frames })
        .collect();
    out.sort_by(|a, b| b.frames.cmp(&a.frames).then_with(|| a.at.cmp(&b.at)));
    out
}

/// Every call site's count so far, most frames first.
pub fn snapshot() -> Vec<Requester> {
    COUNTS.lock().ok().and_then(|counts| counts.as_ref().map(sorted)).unwrap_or_default()
}

/// The counts since the last take, most frames first, and start again.
pub fn take() -> Vec<Requester> {
    COUNTS.lock().ok().and_then(|mut counts| counts.take().map(|c| sorted(&c))).unwrap_or_default()
}

/// One line for a log: `total by file:line=N file:line=N …`, the `limit`
/// busiest sites. `None` when nothing asked.
pub fn summary(requesters: &[Requester], limit: usize) -> Option<String> {
    if requesters.is_empty() {
        return None;
    }
    let total: u64 = requesters.iter().map(|r| r.frames).sum();
    let sites: Vec<String> = requesters.iter().take(limit).map(|r| format!("{}={}", r.at, r.frames)).collect();
    Some(format!("{total} by {}", sites.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn here() -> &'static Location<'static> {
        Location::caller()
    }

    #[test]
    fn counts_per_call_site_and_take_resets() {
        // The counter is process-wide: start from a clean slate.
        let _ = take();
        let a = here();
        let b = here();
        record(a);
        record(a);
        record(b);
        let got = take();
        assert_eq!(got.len(), 2, "two call sites");
        assert_eq!(got[0].frames, 2, "the busiest first");
        assert!(got[0].at.ends_with(&format!("frames.rs:{}", a.line())));
        assert_eq!(got[1].frames, 1);
        assert!(take().is_empty(), "a take starts again");
        assert_eq!(short("/Users/x/Projects/agentic-ui/crates/aui/src/a.rs"), "crates/aui/src/a.rs");
        assert_eq!(short("src/a.rs"), "src/a.rs");
        assert_eq!(summary(&got, 1).as_deref(), Some(format!("3 by {}=2", got[0].at).as_str()));
        assert_eq!(summary(&[], 3), None);
    }
}
