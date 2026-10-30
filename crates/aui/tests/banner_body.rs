//! A grown banner ([`aui::feedback::Banner::body`]), measured on a real
//! frame: the body draws inside the one frame, under the line, and starts
//! where the message does (past the glyph); a banner without a body keeps
//! its one-line height.

use std::cell::RefCell;
use std::rc::Rc;

use aui::feedback::{banner, BannerKind, BannerRun};
use gpui::{div, prelude::*, px, size, Bounds, Pixels, TestAppContext};

type Seen = Rc<RefCell<Vec<Bounds<Pixels>>>>;

struct Host {
    frame: Seen,
    body: Seen,
    grown: bool,
}

impl gpui::Render for Host {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let frame = self.frame.clone();
        let body = self.body.clone();
        let mut row = banner("banner-body-test", BannerKind::Info, vec![BannerRun::Text("Codex stopped twice.".into())]);
        if self.grown {
            row = row.body(
                div()
                    .on_children_prepainted(move |bounds, _, _| body.borrow_mut().extend(bounds))
                    .child(div().w_full().h(px(40.0))),
            );
        }
        div()
            .w(px(600.0))
            .on_children_prepainted(move |bounds, _, _| frame.borrow_mut().extend(bounds))
            .child(row)
    }
}

fn measure(cx: &mut TestAppContext, grown: bool) -> (Bounds<Pixels>, Option<Bounds<Pixels>>) {
    cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
    let frame: Seen = Rc::new(RefCell::new(Vec::new()));
    let body: Seen = Rc::new(RefCell::new(Vec::new()));
    let _handle = cx.open_window(size(px(800.0), px(400.0)), {
        let (frame, body) = (frame.clone(), body.clone());
        move |_, _| Host { frame, body, grown }
    });
    cx.run_until_parked();
    let frame = frame.borrow().first().copied().expect("the banner prepaints");
    let body = body.borrow().first().copied();
    (frame, body)
}

#[gpui::test]
fn a_grown_banner_draws_its_body_inside_the_frame_under_the_line(cx: &mut TestAppContext) {
    let (frame, body) = measure(cx, true);
    let body = body.expect("the body prepaints");
    let (frame_top, frame_bottom) = (f32::from(frame.origin.y), f32::from(frame.origin.y + frame.size.height));
    let (body_top, body_bottom) = (f32::from(body.origin.y), f32::from(body.origin.y + body.size.height));
    assert!(body_top > frame_top + 20.0, "the body sits under the line: frame {frame:?}, body {body:?}");
    assert!(body_bottom < frame_bottom, "the body sits inside the frame: frame {frame:?}, body {body:?}");
    // Left pad 12 + glyph 14 + gap 10: the body starts where the message does.
    let indent = f32::from(body.origin.x) - f32::from(frame.origin.x);
    assert!((indent - 36.0).abs() < 1.5, "the body aligns with the message, indent {indent}");
}

#[gpui::test]
fn a_banner_without_a_body_keeps_its_one_line(cx: &mut TestAppContext) {
    let (line, body) = measure(cx, false);
    assert!(body.is_none());
    let height = f32::from(line.size.height);
    // One 12.5 px line, 8 px padding above and below, the hairline.
    assert!(height < 44.0, "one line plus its padding, got {height}");
}

#[gpui::test]
fn a_grown_banner_is_its_line_plus_the_gap_and_the_body(cx: &mut TestAppContext) {
    let (line, _) = measure(cx, false);
    let (grown, _) = measure(cx, true);
    let grew = f32::from(grown.size.height) - f32::from(line.size.height);
    assert!((grew - 48.0).abs() < 0.5, "the 8 px gap and the 40 px body, grew {grew}");
}
