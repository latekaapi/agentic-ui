//! `anchored_menu` keeps its window fit from inside `popover_layer`.
//!
//! The popovers diagnosis assumed — but never exercised — that gpui's
//! `Anchored` still flips and clamps when it rides in `deferred`. This test
//! opens a real 800×600 window, seats a 200×160 menu below-start of a trigger
//! in the bottom-right corner (where it fits neither below nor to the
//! right), and asserts the painted menu bounds stay inside the window and
//! flip above the trigger.

use std::cell::RefCell;
use std::rc::Rc;

use aui::overlay::{anchored_menu, MenuAlign, MenuSide};
use gpui::{div, point, prelude::*, px, size, Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window};

/// Painted menu bounds, recorded in prepaint.
type Seen = Rc<RefCell<Vec<Bounds<Pixels>>>>;

struct MenuHost {
    seen: Seen,
}

impl Render for MenuHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let seen = self.seen.clone();
        // Bottom-right corner: below overflows (550 + 24 + 4 + 160 > 600)
        // and start-aligned overflows right (700 + 200 > 800).
        let trigger = Bounds::new(point(px(700.0), px(550.0)), size(px(60.0), px(24.0)));
        let menu = div()
            .w(px(200.0))
            .h(px(160.0))
            .on_children_prepainted(move |bounds, _, _| {
                *seen.borrow_mut() = bounds;
            })
            .child(div().w(px(200.0)).h(px(160.0)));
        div().size_full().child(anchored_menu(trigger, MenuSide::Below, MenuAlign::Start, menu))
    }
}

#[gpui::test]
fn anchored_menu_in_popover_layer_flips_and_clamps_to_window(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let seen: Seen = Rc::new(RefCell::new(Vec::new()));
    let _handle = cx.open_window(size(px(800.0), px(600.0)), {
        let seen = seen.clone();
        move |_, _| MenuHost { seen }
    });
    cx.run_until_parked();

    let painted = seen.borrow().first().copied().expect("the menu must prepaint inside the seat");
    assert!(
        f32::from(painted.origin.x) >= 0.0 && f32::from(painted.origin.x + painted.size.width) <= 800.0,
        "menu must clamp horizontally, got {painted:?}"
    );
    assert!(
        f32::from(painted.origin.y) >= 0.0 && f32::from(painted.origin.y + painted.size.height) <= 600.0,
        "menu must stay inside the window vertically, got {painted:?}"
    );
    // `SwitchAnchor` pivots on the same origin: flipping above puts the
    // menu's bottom edge at trigger-bottom + gap (550 + 24 + 4 = 578), and
    // flipping the align puts its right edge at the trigger's left (700).
    assert!(
        (f32::from(painted.origin.y + painted.size.height) - 578.0).abs() < 1.0
            && f32::from(painted.origin.y) < 550.0,
        "below-space is short, so the seat must flip above the trigger, got {painted:?}"
    );
    assert!(
        (f32::from(painted.origin.x + painted.size.width) - 700.0).abs() < 1.0,
        "right-space is short, so the seat must slide inside the window, got {painted:?}"
    );
}
