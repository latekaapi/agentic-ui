//! The right column's resize contract, measured on a real frame.
//!
//! The clamp helper has unit tests, but `clamp_right_width` is not what the
//! shell calls: `render` clamps its own `right_rest` and hands that to the
//! pane as `right_inner`. Reverting `right_inner` to the raw `right_width`
//! leaves every helper test green, so it is measured here instead, on the
//! bounds the pane is actually prepainted at.
//!
//! What is deliberately NOT tested here: the mid-drag spring bypass on
//! `right_w`. That bypass moves the *column*, while the pane inside it sits
//! at `right_inner` either way — so a test that measures the pane cannot see
//! it, and one that appeared to would be measuring the wrong thing. It stays
//! unproven until a host wires a right-pane drag handle.

use std::cell::RefCell;
use std::rc::Rc;

use aui::shell::{app_shell, RIGHT_MAX_WIDTH, RIGHT_MIN_WIDTH, RIGHT_WIDTH};
use gpui::{div, prelude::*, px, size, Bounds, Pixels, TestAppContext};

type Seen = Rc<RefCell<Vec<Bounds<Pixels>>>>;

/// Mounts the shell with `width` asked for and `resizing` set, and records the
/// bounds the right pane's own child is prepainted at.
struct Host {
    seen: Seen,
    width: f32,
    resizing: bool,
    open: bool,
}

impl gpui::Render for Host {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let seen = self.seen.clone();
        let pane = div()
            .size_full()
            .on_children_prepainted(move |bounds, _, _| {
                seen.borrow_mut().extend(bounds);
            })
            .child(div().size_full());
        app_shell("right-column-test")
            .right_open(self.open)
            .right_width(px(self.width))
            .resizing(self.resizing)
            .right(pane)
            .centre(div().size_full())
            .sidebar(div().size_full())
            .rail(div().size_full())
    }
}

fn pane_width(cx: &mut TestAppContext, width: f32, resizing: bool, open: bool) -> f32 {
    let seen: Seen = Rc::new(RefCell::new(Vec::new()));
    let _handle = cx.open_window(size(px(1600.0), px(900.0)), {
        let seen = seen.clone();
        move |_, _| Host { seen, width, resizing, open }
    });
    cx.run_until_parked();
    let bounds = seen.borrow().first().copied().expect("the right pane must prepaint");
    f32::from(bounds.size.width)
}

/// The pane sits inside the column's 1 px left divider, so its measured width
/// is the column's less that hairline. Two pixels of slack covers the border
/// and any sub-pixel rounding without letting a real miss through: the values
/// this test is guarding against are off by hundreds, not by one.
const HAIRLINE: f32 = 2.0;

#[gpui::test]
fn the_right_pane_is_laid_out_at_the_clamped_width_not_the_asked_one(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    // Far above the maximum: the pane must be laid out at the bound, not at
    // what was asked for. This fails if `right_inner` reads `self.right_width`.
    let wide = pane_width(cx, 10_000.0, true, true);
    assert!(
        (wide - RIGHT_MAX_WIDTH).abs() <= HAIRLINE,
        "asked for 10000px, expected the pane clamped to {RIGHT_MAX_WIDTH}, got {wide}"
    );
    // Far below the minimum, the same way.
    let narrow = pane_width(cx, 10.0, true, true);
    assert!(
        (narrow - RIGHT_MIN_WIDTH).abs() <= HAIRLINE,
        "asked for 10px, expected the pane clamped to {RIGHT_MIN_WIDTH}, got {narrow}"
    );
    // And a width inside the range is left alone.
    let rest = pane_width(cx, RIGHT_WIDTH, true, true);
    assert!(
        (rest - RIGHT_WIDTH).abs() <= HAIRLINE,
        "a width inside the range must pass through, got {rest}"
    );
}
