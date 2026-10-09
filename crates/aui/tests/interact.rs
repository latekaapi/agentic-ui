//! D1: the interaction primitives' pure rules, labels, elevation and heights.
//!
//! - [`nearest`](aui::interact::nearest): inside wins, gaps / padding /
//!   past-last still land, ties keep the first, empty is none;
//! - [`nested_radius`](aui::interact::nested_radius): the concentric table;
//! - every `interactive` and `row` carries its human label (same AX probe
//!   the transcript rows use);
//! - `Surface::elevated` composes through nesting (1 → 3 → 5, capped 8);
//! - `Row` height equals `metrics.control` at both densities.

use std::cell::RefCell;
use std::rc::Rc;

use aui::data::{arm_ax_probe, take_ax_labels};
use aui::interact::{ambient_surface, interactive, nearest, nested_radius, row, surface, Axis};
use aui_tokens::{AuiTheme, Density, ThemeKind};
use gpui::{div, prelude::*, px, AnyElement, Bounds, IntoElement, Pixels, Point, TestAppContext, Window};

/// Three stacked items: 300 × 36 at y 0, 44 and 88 (an 8 px gap).
fn rows() -> Vec<Bounds<Pixels>> {
    (0..3)
        .map(|index| Bounds {
            origin: Point { x: px(10.0), y: px(index as f32 * 44.0) },
            size: gpui::Size { width: px(300.0), height: px(36.0) },
        })
        .collect()
}

fn at(x: f32, y: f32) -> Point<Pixels> {
    Point { x: px(x), y: px(y) }
}

#[test]
fn nearest_inside_wins() {
    let items = rows();
    assert_eq!(nearest(&items, at(20.0, 10.0), Axis::Y), Some(0));
    assert_eq!(nearest(&items, at(200.0, 50.0), Axis::Y), Some(1));
    assert_eq!(nearest(&items, at(20.0, 100.0), Axis::Y), Some(2));
}

#[test]
fn nearest_gap_between_rows_lands_on_the_closer_centre() {
    let items = rows();
    // Gap 36..44; centres at 18 and 62: y=42 is closer to the second.
    assert_eq!(nearest(&items, at(20.0, 42.0), Axis::Y), Some(1));
}

#[test]
fn nearest_tie_keeps_the_first() {
    let items = rows();
    // y=40 is exactly between centres 18 and 62.
    assert_eq!(nearest(&items, at(20.0, 40.0), Axis::Y), Some(0));
}

#[test]
fn nearest_padding_above_the_first_still_lands() {
    let items = rows();
    assert_eq!(nearest(&items, at(20.0, -20.0), Axis::Y), Some(0));
}

#[test]
fn nearest_past_the_last_still_lands() {
    let items = rows();
    assert_eq!(nearest(&items, at(20.0, 400.0), Axis::Y), Some(2));
}

#[test]
fn nearest_empty_is_none() {
    assert_eq!(nearest(&[], at(0.0, 0.0), Axis::Y), None);
    assert_eq!(nearest(&[], at(0.0, 0.0), Axis::XY), None);
}

#[test]
fn nearest_x_axis_uses_column_centres() {
    let items: Vec<Bounds<Pixels>> = (0..3)
        .map(|index| Bounds {
            origin: Point { x: px(index as f32 * 108.0), y: px(0.0) },
            size: gpui::Size { width: px(100.0), height: px(32.0) },
        })
        .collect();
    // Gap 100..108; centres at 50 and 158: x=106 is closer to the second
    // (x=104 would be an exact tie, which keeps the first).
    assert_eq!(nearest(&items, at(106.0, 16.0), Axis::X), Some(1));
    assert_eq!(nearest(&items, at(104.0, 16.0), Axis::X), Some(0));
    // Past the end still lands.
    assert_eq!(nearest(&items, at(900.0, 16.0), Axis::X), Some(2));
}

#[test]
fn nearest_xy_uses_both_axes() {
    let items: Vec<Bounds<Pixels>> = (0..3)
        .map(|index| Bounds {
            origin: Point { x: px(index as f32 * 108.0), y: px(0.0) },
            size: gpui::Size { width: px(100.0), height: px(100.0) },
        })
        .collect();
    // Below the gap between the first two cards, a little right of the
    // midpoint: nearer the second centre (x=104 would tie).
    assert_eq!(nearest(&items, at(106.0, 120.0), Axis::XY), Some(1));
}

#[test]
fn nested_radius_follows_the_concentric_rule() {
    // (outer, inset) → inner.
    for (outer, inset, want) in [(8.0, 4.0, 4.0), (8.0, 0.0, 8.0), (6.0, 2.0, 4.0), (8.0, 8.0, 0.0), (6.0, 8.0, 0.0)] {
        assert_eq!(nested_radius(px(outer), px(inset)), px(want), "outer {outer} inset {inset}");
    }
}

/// Draws `build` once in a real window and returns the AX probe's labels.
fn drawn_labels(
    cx: &mut TestAppContext,
    build: impl Fn(&mut Window, &mut gpui::App) -> AnyElement + 'static,
) -> Vec<String> {
    struct Host {
        build: Rc<dyn Fn(&mut Window, &mut gpui::App) -> AnyElement>,
    }
    impl gpui::Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            (self.build.clone())(window, cx)
        }
    }
    arm_ax_probe(true);
    take_ax_labels();
    cx.add_window_view(|_, _| Host { build: Rc::new(build) });
    let labels = take_ax_labels();
    arm_ax_probe(false);
    labels
}

#[gpui::test]
fn interactive_and_row_carry_their_labels(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    let labels = drawn_labels(cx, |_, _| {
        div()
            .child(interactive("labels-a", "Archive").label("Archive conversation"))
            .child(row("labels-b", "Checkout flow"))
            .into_any_element()
    });
    assert!(labels.contains(&"Archive conversation".to_string()), "interactive label missing: {labels:?}");
    assert!(labels.contains(&"Checkout flow".to_string()), "row label missing: {labels:?}");
    assert!(labels.iter().all(|label| !label.is_empty()), "unlabelled control: {labels:?}");
}

/// Records the ambient surface step its level sees. Each probe owns one
/// slot, so repeat renders overwrite the same value instead of appending.
#[derive(IntoElement)]
struct AmbientProbe {
    seen: Rc<RefCell<Vec<Option<u8>>>>,
    slot: usize,
}

impl gpui::RenderOnce for AmbientProbe {
    fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        self.seen.borrow_mut()[self.slot] = Some(ambient_surface(window, cx));
        div()
    }
}

fn probed_levels(cx: &mut TestAppContext, depths: usize) -> Vec<u8> {
    struct Host {
        seen: Rc<RefCell<Vec<Option<u8>>>>,
        depths: usize,
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            // Slot 0 is the page probe; slot `level` sits inside the
            // `level`-th elevated surface; the last slot is the innermost
            // probe, one level deeper than the deepest surface.
            let mut inner: AnyElement = AmbientProbe { seen: seen.clone(), slot: self.depths + 1 }.into_any_element();
            for level in (1..=self.depths).rev() {
                let probe = AmbientProbe { seen: seen.clone(), slot: level };
                inner = surface(1, div().child(probe).child(inner)).elevated(2).into_any_element();
            }
            surface(1, div().child(AmbientProbe { seen: seen.clone(), slot: 0 }).child(inner)).into_any_element()
        }
    }
    // One slot per level plus the innermost probe: page, depths levels.
    let seen = Rc::new(RefCell::new(vec![None; depths + 2]));
    let seen_out = seen.clone();
    cx.add_window_view(|_, _| Host { seen, depths });
    let slots = seen_out.borrow().clone();
    slots.into_iter().map(|slot| slot.expect("every probe rendered")).collect()
}

#[gpui::test]
fn surface_elevated_composes_through_nesting(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    // Page 1, then +2 per level; the last slot is the innermost probe,
    // one level deeper than the deepest surface.
    assert_eq!(probed_levels(cx, 2), vec![1, 3, 5, 5]);
}

#[gpui::test]
fn surface_elevated_caps_at_step_8(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    // 1 → 3 → 5 → 7 → 8, then the innermost probe still reads 8.
    assert_eq!(probed_levels(cx, 4), vec![1, 3, 5, 7, 8, 8]);
}

#[gpui::test]
fn row_height_matches_the_ladder_at_both_densities(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    let (one, two) = cx.update(|cx| (row("h-1", "One line").height(cx), row("h-2", "Two lines").two_line("Byline").height(cx)));
    assert_eq!(one, px(36.0), "standard one-line row");
    assert_eq!(two, px(52.0), "standard two-line row");
    cx.update(|cx| AuiTheme::set_density(Density::Compact, None, cx));
    let (one, two) = cx.update(|cx| (row("h-1", "One line").height(cx), row("h-2", "Two lines").two_line("Byline").height(cx)));
    assert_eq!(one, px(28.0), "compact one-line row");
    assert_eq!(two, px(44.0), "compact two-line row");
}

#[gpui::test]
fn row_title_reserves_the_semibold_width(cx: &mut TestAppContext) {
    use gpui::{FontWeight, Styled};
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    cx.update(|cx| {
        let plain = row("w-1", "Checkout flow");
        let picked = row("w-2", "Checkout flow").selected(true);
        let weight = |mut el: gpui::Div| el.style().text.font_weight;
        let size = |mut el: gpui::Div| el.style().text.font_size;
        // The hidden copy is semibold on every row: it is the width the row
        // reserves. The visible copy turns semibold only when selected. The
        // role's weight must not overwrite either (role first, weight last).
        assert_eq!(weight(plain.title(true, cx)), Some(FontWeight::SEMIBOLD), "hidden copy, unselected");
        assert_eq!(weight(picked.title(true, cx)), Some(FontWeight::SEMIBOLD), "hidden copy, selected");
        assert_eq!(weight(plain.title(false, cx)), Some(FontWeight::NORMAL), "visible copy, unselected");
        assert_eq!(weight(picked.title(false, cx)), Some(FontWeight::SEMIBOLD), "visible copy, selected");
        // Both copies set the same role size, so the reservation is real.
        assert_eq!(size(plain.title(true, cx)), size(plain.title(false, cx)));
        assert!(size(plain.title(true, cx)).is_some());
    });
}

#[test]
fn press_inset_is_paint_only_and_capped_at_one_px() {
    use aui::interact::press_inset;
    assert_eq!(press_inset(0.0), px(0.0));
    assert_eq!(press_inset(1.0), px(1.0));
    assert_eq!(press_inset(1.4), px(1.0), "spring overshoot never insets past 1 px");
    assert_eq!(press_inset(-0.2), px(0.0));
}
