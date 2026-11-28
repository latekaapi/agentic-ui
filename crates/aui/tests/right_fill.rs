//! The right column's fill (a maximised pane, `AppShell::right_fill`),
//! measured on real frames: where the pane lands with the sidebar open and
//! closed, that the centre gives up its width, and that the fill comes and
//! goes in one frame — every frame after a switch is prepainted at the new
//! width, never the old one or a frame of a slide between them.

use std::cell::RefCell;
use std::rc::Rc;

use aui::shell::{app_shell, right_fill_width, RIGHT_WIDTH, SIDEBAR_WIDTH};
use gpui::{div, prelude::*, px, size, Bounds, Pixels, TestAppContext};

type Seen = Rc<RefCell<Vec<Bounds<Pixels>>>>;

const WIN_W: f32 = 1600.0;
/// The pane beside the centre sits inside the column's 1 px divider; a
/// filling pane draws none. Two pixels of slack covers that and rounding.
const SLACK: f32 = 2.0;

struct Host {
    pane: Seen,
    centre: Seen,
    fill: bool,
    sidebar_open: bool,
    right_open: bool,
}

impl gpui::Render for Host {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let (pane_seen, centre_seen) = (self.pane.clone(), self.centre.clone());
        // What shows of the pane: its child's bounds cut by the clip the
        // column puts on it (the column clips; the pane inside is laid out
        // at the column's resting width whatever the column's motion).
        let pane = div()
            .size_full()
            .on_children_prepainted(move |bounds, window, _| {
                let mask = window.content_mask().bounds;
                pane_seen.borrow_mut().extend(bounds.into_iter().map(|b| b.intersect(&mask)));
            })
            .child(div().size_full());
        // The centre's column, seen through a child that fills it: what the
        // person would see of the centre.
        let centre = div().size_full();
        app_shell("right-fill-test")
            .sidebar_open(self.sidebar_open)
            .right_open(self.right_open)
            .right_width(px(RIGHT_WIDTH))
            .right_fill(self.fill)
            .right(pane)
            .centre(centre)
            .sidebar(div().size_full())
            .chrome_title(div().size_full().on_children_prepainted(move |bounds, _, _| centre_seen.borrow_mut().extend(bounds)).child(div().size_full()))
    }
}

fn open(cx: &mut TestAppContext, fill: bool, sidebar_open: bool) -> (gpui::WindowHandle<Host>, Seen, Seen) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let (pane, centre): (Seen, Seen) = Default::default();
    let handle = cx.open_window(size(px(WIN_W), px(900.0)), {
        let (pane, centre) = (pane.clone(), centre.clone());
        move |_, _| Host { pane, centre, fill, sidebar_open, right_open: true }
    });
    cx.run_until_parked();
    (handle, pane, centre)
}

/// What each frame showed of the pane (already cut by the column's clip):
/// its width, 0 when none shows, and NaN — which fails every comparison —
/// when what shows does not reach the window's right edge (a column whose
/// width and content disagree).
fn shown_widths(seen: &Seen) -> Vec<f32> {
    seen.borrow()
        .iter()
        .map(|b| {
            let w = f32::from(b.size.width);
            if w <= 0.5 {
                0.0
            } else if (f32::from(b.origin.x) + w - WIN_W).abs() <= SLACK {
                w
            } else {
                f32::NAN
            }
        })
        .collect()
}

fn last(seen: &Seen) -> Bounds<Pixels> {
    *seen.borrow().last().expect("prepainted")
}

#[test]
fn the_fill_width_is_the_window_less_the_sidebar() {
    assert_eq!(right_fill_width(1600.0, 256.0), 1344.0);
    assert_eq!(right_fill_width(1600.0, 0.0), 1600.0);
    // Past the divider's cap: a filled pane is not clamped.
    assert!(right_fill_width(1600.0, 256.0) > aui::shell::RIGHT_MAX_WIDTH);
    // A window narrower than the sidebar leaves nothing, never a negative.
    assert_eq!(right_fill_width(200.0, 256.0), 0.0);
}

#[gpui::test]
fn filling_beside_an_open_sidebar_takes_everything_right_of_it(cx: &mut TestAppContext) {
    let (_h, pane, centre) = open(cx, true, true);
    let b = last(&pane);
    assert!((f32::from(b.origin.x) - SIDEBAR_WIDTH).abs() <= SLACK, "the pane starts at the sidebar's edge, got {:?}", b.origin.x);
    assert!((f32::from(b.size.width) - (WIN_W - SIDEBAR_WIDTH)).abs() <= SLACK, "the pane fills the rest, got {:?}", b.size.width);
    // The centre header (and its column) has no width left to show in.
    assert!(centre.borrow().iter().all(|c| f32::from(c.size.width) <= 0.5 || f32::from(c.origin.x) >= WIN_W), "no centre on screen: {:?}", centre.borrow());
}

#[gpui::test]
fn filling_with_the_sidebar_closed_takes_the_whole_window(cx: &mut TestAppContext) {
    let (_h, pane, _) = open(cx, true, false);
    let b = last(&pane);
    assert!(f32::from(b.origin.x).abs() <= SLACK, "the pane starts at the window's left, got {:?}", b.origin.x);
    assert!((f32::from(b.size.width) - WIN_W).abs() <= SLACK, "the pane is the window's width, got {:?}", b.size.width);
}

#[gpui::test]
fn the_fill_comes_and_goes_in_one_frame(cx: &mut TestAppContext) {
    let (h, pane, _) = open(cx, false, true);
    let beside = f32::from(last(&pane).size.width);
    assert!((beside - RIGHT_WIDTH).abs() <= SLACK, "beside the centre the pane is its own width, got {beside}");
    let filled = WIN_W - SIDEBAR_WIDTH;
    for fill in [true, false, true, false] {
        pane.borrow_mut().clear();
        h.update(cx, |host, _, cx| {
            host.fill = fill;
            cx.notify();
        })
        .unwrap();
        cx.run_until_parked();
        let shown = shown_widths(&pane);
        assert!(!shown.is_empty(), "the pane drew");
        // Every frame after the switch already shows the new width: a spring
        // shows the old width first, then travels (or, on a test clock, jumps).
        let want = if fill { filled } else { beside };
        assert!(shown.iter().all(|w| (w - want).abs() <= SLACK), "fill={fill}: every frame at {want} (no slide, no late frame): {shown:?}");
    }
}

#[gpui::test]
fn the_sidebar_moves_instantly_under_a_filling_pane(cx: &mut TestAppContext) {
    let (h, pane, _) = open(cx, true, true);
    for open in [false, true] {
        pane.borrow_mut().clear();
        h.update(cx, |host, _, cx| {
            host.sidebar_open = open;
            cx.notify();
        })
        .unwrap();
        cx.run_until_parked();
        let want = if open { WIN_W - SIDEBAR_WIDTH } else { WIN_W };
        let shown = shown_widths(&pane);
        assert!(shown.iter().all(|w| (w - want).abs() <= SLACK), "sidebar open={open}: every frame at {want}, got {shown:?}");
    }
}

#[gpui::test]
fn closing_a_filling_pane_leaves_no_slide_from_the_filled_width(cx: &mut TestAppContext) {
    let (h, pane, _) = open(cx, true, true);
    pane.borrow_mut().clear();
    // The host's close: the pane shuts, and with it the fill.
    h.update(cx, |host, _, cx| {
        host.right_open = false;
        host.fill = false;
        cx.notify();
    })
    .unwrap();
    cx.run_until_parked();
    let shown = shown_widths(&pane);
    assert!(shown.iter().all(|w| *w < RIGHT_WIDTH + SLACK), "no frame at the filled width after the close: {shown:?}");
}
