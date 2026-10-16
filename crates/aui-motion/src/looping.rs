//! A repeating 0→1 phase for spinners, blinking carets and other ambient
//! loops. Under reduced motion it holds at the resting phase.
//!
//! The loop wakes its view on a timer (then on the next drawn frame), never
//! with `request_animation_frame` on every render.
//! gpui dirties every ancestor of a notified view, so a loop that asked for
//! the next frame on every render held the whole window at display rate —
//! 120 root renders a second for a blinking caret, a spinner, or a progress
//! glyph left running beside "Waiting for approval…" (perf audit
//! 2026-10-16). A smooth loop now redraws at most [`SMOOTH_TICK`] apart; a
//! [`Loop::stepped`] one (a caret, a braille spinner) wakes only at its next
//! step boundary.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use aui_tokens::Easing;
use gpui::{AnyWindowHandle, App, ElementId, EntityId, Window};
use gpui_kit::base::TransitionId;

/// The longest a smooth loop waits between redraws: 30 Hz, a quarter of a
/// 120 Hz display's frames and smooth to the eye for ambient motion.
pub const SMOOTH_TICK: Duration = Duration::from_millis(33);

/// Loop parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loop {
    /// One cycle.
    pub period: Duration,
    /// Easing per cycle (`None` = linear).
    pub easing: Option<Easing>,
    /// Ping-pong instead of restarting.
    pub alternate: bool,
    /// Phase reported under reduced motion.
    pub resting: f32,
    /// Discrete steps per cycle, when the caller only reads which step it is
    /// in (a caret's on/off, a spinner's glyph): the view wakes once per step
    /// instead of at [`SMOOTH_TICK`].
    pub steps: Option<u32>,
}

impl Loop {
    /// A linear loop.
    pub const fn linear(period: Duration) -> Self {
        Self { period, easing: None, alternate: false, resting: 0.0, steps: None }
    }

    /// An eased loop.
    pub const fn eased(period: Duration, easing: Easing) -> Self {
        Self { period, easing: Some(easing), alternate: false, resting: 0.0, steps: None }
    }

    /// Ping-pong.
    pub const fn alternate(mut self) -> Self {
        self.alternate = true;
        self
    }

    /// The value to hold under reduced motion.
    pub const fn resting(mut self, resting: f32) -> Self {
        self.resting = resting;
        self
    }

    /// Wake only at each of `steps` boundaries per cycle (see [`Loop::steps`]).
    pub const fn stepped(mut self, steps: u32) -> Self {
        self.steps = Some(steps);
        self
    }
}

/// When the loop keyed by an id started, kept in the element's keyed state so
/// it restarts at phase 0 whenever the element is mounted again.
struct LoopEpoch(Instant);

thread_local! {
    /// The wake already scheduled for each view, so many loops in one view
    /// share one timer: a later loop only schedules when it needs an
    /// earlier wake.
    static PENDING: RefCell<HashMap<EntityId, Instant>> = RefCell::new(HashMap::new());
}

/// The current phase in `0..=1` of a loop keyed by `id`.
pub fn looping(id: impl Into<TransitionId>, config: Loop, window: &mut Window, cx: &mut App) -> f32 {
    if cx.reduce_motion() {
        return config.resting;
    }
    let id: ElementId = id.into().into();
    let now = cx.background_executor().now();
    let started = window.use_keyed_state(id, cx, |_, _| LoopEpoch(now)).read(cx).0;
    let elapsed = now.saturating_duration_since(started);
    let period = config.period.max(Duration::from_millis(1));
    let cycles = elapsed.as_secs_f64() / period.as_secs_f64();
    let cycle = cycles.floor();
    let mut t = (cycles - cycle) as f32;
    if config.alternate && (cycle as u64) % 2 == 1 {
        t = 1.0 - t;
    }
    let value = config.easing.map_or(t, |easing| easing.sample(t));
    let wait = match config.steps {
        Some(steps) if steps > 0 => {
            let step = period / steps;
            let into = Duration::from_nanos((elapsed.as_nanos() % step.as_nanos().max(1)) as u64);
            // Just past the boundary, so the redraw lands in the next step.
            step.saturating_sub(into) + Duration::from_millis(1)
        }
        _ => SMOOTH_TICK,
    };
    wake_after(window.window_handle(), window.current_view(), now + wait, wait, cx);
    value
}

thread_local! {
    /// The clock origin every [`synced`] loop shares.
    static SYNC_EPOCH: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

/// A linear 0→1 phase on one clock shared by every caller — two shimmers
/// on screen sweep together, as gpui's `repeat_synced` did — waking the
/// rendering view at [`SMOOTH_TICK`]. Holds `resting` under reduced motion.
pub fn synced(period: Duration, resting: f32, window: &mut Window, cx: &mut App) -> f32 {
    if cx.reduce_motion() {
        return resting;
    }
    let now = cx.background_executor().now();
    let epoch = SYNC_EPOCH.with(|epoch| match epoch.get() {
        Some(at) if at <= now => at,
        _ => {
            epoch.set(Some(now));
            now
        }
    });
    let elapsed = now.saturating_duration_since(epoch).as_secs_f64();
    let period = period.max(Duration::from_millis(1)).as_secs_f64();
    wake_after(window.window_handle(), window.current_view(), now + SMOOTH_TICK, SMOOTH_TICK, cx);
    ((elapsed / period).fract()) as f32
}

/// Notify `view` on the first frame after `wait` has passed, unless a wake at
/// or before `at` is already scheduled for it.
///
/// The timer only arms a next-frame callback: gpui stops a window's display
/// link while the window is hidden or covered, so a covered window wakes
/// nothing (a plain timer notify kept ticking behind other apps), and its
/// loops resume on the first frame it draws again.
fn wake_after(window: AnyWindowHandle, view: EntityId, at: Instant, wait: Duration, cx: &mut App) {
    let scheduled = PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        match pending.get(&view) {
            Some(existing) if *existing <= at => true,
            _ => {
                pending.insert(view, at);
                false
            }
        }
    });
    if scheduled {
        return;
    }
    cx.spawn(async move |cx| {
        cx.background_executor().timer(wait).await;
        let _ = window.update(cx, move |_, window, _| {
            window.on_next_frame(move |_, cx| {
                let due = PENDING.with(|pending| {
                    let mut pending = pending.borrow_mut();
                    match pending.get(&view) {
                        // An earlier wake replaced this one and notifies on its own.
                        Some(existing) if *existing < at => false,
                        _ => {
                            pending.remove(&view);
                            true
                        }
                    }
                });
                if due {
                    cx.notify(view);
                }
            });
        });
    })
    .detach();
}
