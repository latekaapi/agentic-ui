//! The host's wheel router: a Shift + sideways swipe over a native page is
//! the host's gesture (Baaz: one right-pane screen per swipe, docs/43 §3),
//! but the page is its own child window, so the host never saw it (D6
//! follow-up, docs/60 §3 item 1). The shim's monitor
//! (`aui_cef_set_wheel_router`, shim.m) asks [`route_wheel`] about each
//! Shift-held wheel over a page; the host's `claims` decides synchronously,
//! without touching UI state, and a claimed event is swallowed and handed to
//! `deliver` (which should only queue it).

use std::cell::RefCell;
use std::rc::Rc;

unsafe extern "C" {
    fn aui_cef_set_wheel_router(f: Option<extern "C" fn(f64, f64, f64, f64, i32, i32, i32) -> i32>);
}

/// Where a wheel event sits in its gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelPhase {
    /// A trackpad gesture's first event.
    Started,
    /// Everything between, momentum included, and a mouse wheel's ticks.
    Moved,
    /// The fingers lifted (or the gesture was cancelled).
    Ended,
}

/// One Shift-held wheel event over a page, in the host window's content
/// coordinates (top-left origin, points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageWheel {
    /// Pointer x.
    pub x: f64,
    /// Pointer y.
    pub y: f64,
    /// Horizontal delta, points (AppKit `scrollingDeltaX`).
    pub dx: f64,
    /// Vertical delta, points.
    pub dy: f64,
    /// Its place in the gesture.
    pub phase: WheelPhase,
    /// A trackpad's precise deltas, not a mouse wheel's lines.
    pub precise: bool,
}

/// AppKit's `phase` / `momentumPhase` bits to a [`WheelPhase`]: began → the
/// start; ended or cancelled (with no momentum to follow) → the end;
/// momentum and everything else → moved.
pub fn wheel_phase(phase: i32, momentum_phase: i32) -> WheelPhase {
    const BEGAN: i32 = 1;
    const ENDED: i32 = 8;
    const CANCELLED: i32 = 16;
    if momentum_phase != 0 {
        return if momentum_phase & (ENDED | CANCELLED) != 0 { WheelPhase::Ended } else { WheelPhase::Moved };
    }
    if phase & BEGAN != 0 {
        WheelPhase::Started
    } else if phase & (ENDED | CANCELLED) != 0 {
        WheelPhase::Ended
    } else {
        WheelPhase::Moved
    }
}

struct Router {
    claims: Box<dyn Fn(&PageWheel) -> bool>,
    deliver: Box<dyn Fn(PageWheel)>,
}

thread_local! {
    /// Main-thread only, like the key router: cloned out of the cell before
    /// the host's closures run.
    static ROUTER: RefCell<Option<Rc<Router>>> = const { RefCell::new(None) };
}

/// Installs the host wheel router (main thread only). `claims` runs inside
/// AppKit's event monitor: answer from plain state, never from UI entities.
pub fn set_wheel_router(claims: impl Fn(&PageWheel) -> bool + 'static, deliver: impl Fn(PageWheel) + 'static) {
    ROUTER.with(|slot| *slot.borrow_mut() = Some(Rc::new(Router { claims: Box::new(claims), deliver: Box::new(deliver) })));
    // Safety: a plain function pointer the shim calls on the main thread.
    unsafe { aui_cef_set_wheel_router(Some(route_wheel)) };
}

/// The decision for one event over the installed router: whether it is
/// claimed (and then delivered).
pub(crate) fn claim_wheel(event: PageWheel) -> bool {
    let Some(router) = ROUTER.with(|slot| slot.borrow().clone()) else { return false };
    if (router.claims)(&event) {
        (router.deliver)(event);
        true
    } else {
        false
    }
}

extern "C" fn route_wheel(x: f64, y: f64, dx: f64, dy: f64, phase: i32, momentum_phase: i32, precise: i32) -> i32 {
    claim_wheel(PageWheel { x, y, dx, dy, phase: wheel_phase(phase, momentum_phase), precise: precise != 0 }) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appkit_phases_map_to_the_gesture() {
        assert_eq!(wheel_phase(1, 0), WheelPhase::Started);
        assert_eq!(wheel_phase(4, 0), WheelPhase::Moved);
        assert_eq!(wheel_phase(8, 0), WheelPhase::Ended);
        assert_eq!(wheel_phase(16, 0), WheelPhase::Ended);
        assert_eq!(wheel_phase(0, 0), WheelPhase::Moved, "a mouse wheel tick");
        assert_eq!(wheel_phase(0, 4), WheelPhase::Moved, "momentum");
        assert_eq!(wheel_phase(0, 8), WheelPhase::Ended);
    }

    #[test]
    fn only_a_claimed_event_is_delivered() {
        use std::cell::Cell;
        let got = Rc::new(Cell::new(0));
        let seen = got.clone();
        ROUTER.with(|slot| {
            *slot.borrow_mut() = Some(Rc::new(Router {
                claims: Box::new(|e: &PageWheel| e.dx.abs() > 2.0 * e.dy.abs()),
                deliver: Box::new(move |_| seen.set(seen.get() + 1)),
            }))
        });
        let at = |dx, dy| PageWheel { x: 10.0, y: 10.0, dx, dy, phase: WheelPhase::Moved, precise: true };
        assert!(claim_wheel(at(-30.0, 2.0)), "sideways: the host's");
        assert!(!claim_wheel(at(1.0, 20.0)), "vertical: the page's");
        assert_eq!(got.get(), 1);
        ROUTER.with(|slot| *slot.borrow_mut() = None);
        assert!(!claim_wheel(at(-30.0, 0.0)), "no router, nothing claimed");
    }
}
