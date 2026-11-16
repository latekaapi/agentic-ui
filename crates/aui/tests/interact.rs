//! The interaction primitives: the concentric rule, labels, row heights,
//! and the plain list's activation and pointer report.
//!
//! - [`nested_radius`](aui::interact::nested_radius): the concentric table;
//! - every `interactive`, `row`, `list_row` and the data primitives carry
//!   their human labels (same AX probe the transcript rows use);
//! - `Row` height equals `metrics.control` at both densities, `list_row`
//!   is 32;
//! - a [`RowList`](aui::interact::RowList) activates the clicked item and
//!   reports the item under the pointer, never `None` between two items.

use std::cell::RefCell;
use std::rc::Rc;

use aui::data::{arm_ax_probe, banner, card, section, segmented, settings_row, switch, tabs, take_ax_labels};
use aui::interact::{interactive, list_row, nested_radius, row, row_list};
use aui_tokens::{ActiveAui, AuiTheme, Density, ThemeKind};
use gpui::{div, prelude::*, px, AnyElement, IntoElement, TestAppContext, Window};

#[test]
fn nested_radius_follows_the_concentric_rule() {
    // (outer, inset) → inner.
    for (outer, inset, want) in [(8.0, 4.0, 4.0), (8.0, 0.0, 8.0), (6.0, 2.0, 4.0), (8.0, 8.0, 0.0), (6.0, 8.0, 0.0)] {
        assert_eq!(nested_radius(px(outer), px(inset)), px(want), "outer {outer} inset {inset}");
    }
}

type Builder = Rc<dyn Fn(&mut Window, &mut gpui::App) -> AnyElement>;

/// Draws `build` once in a real window and returns the AX probe's labels.
fn drawn_labels(
    cx: &mut TestAppContext,
    build: impl Fn(&mut Window, &mut gpui::App) -> AnyElement + 'static,
) -> Vec<String> {
    struct Host {
        build: Builder,
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

#[gpui::test]
fn the_flat_primitives_carry_their_labels(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    let labels = drawn_labels(cx, |_, _| {
        div()
            .child(list_row("p-row", "Auth refresh"))
            .child(tabs("p-tabs", "Views", vec!["Changes".into(), "Files".into()], 0))
            .child(card().title("Read file", None).section("body"))
            .child(section("General").row(settings_row("Theme").control(switch("p-sw", true, false).accessibility_label("Dark theme"))))
            .child(segmented("p-seg", vec!["Ask".into(), "Auto".into()], 1).accessibility_label("Approvals"))
            .child(banner("p-banner", "Signed out").action("Sign in", |_, _| {}))
            .into_any_element()
    });
    for want in ["Auth refresh", "Views", "Changes", "Files", "Sign in"] {
        assert!(labels.contains(&want.to_string()), "{want} missing: {labels:?}");
    }
    assert!(labels.iter().all(|label| !label.is_empty()), "unlabelled control: {labels:?}");
}

#[gpui::test]
fn row_height_matches_the_ladder_at_both_densities(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    let (one, two) = cx.update(|cx| (row("h-1", "One line").height(cx), row("h-2", "Two lines").two_line("Byline").height(cx)));
    assert_eq!(one, px(32.0), "comfortable one-line row");
    assert_eq!(two, px(52.0), "comfortable two-line row");
    // The type follows the same switch, and the kit's rem with it.
    let body = |cx: &mut TestAppContext| cx.update(|cx| aui_tokens::TextRole::Body.spec_in(cx.aui().density, cx.aui().text_size).size);
    assert_eq!(body(cx), 14.0);
    assert_eq!(cx.update(|cx| gpui_kit::component::ActiveTheme::theme(cx).font_size), px(14.0));
    cx.update(|cx| AuiTheme::set_density(Density::Compact, None, cx));
    assert_eq!(body(cx), 13.0);
    assert_eq!(cx.update(|cx| gpui_kit::component::ActiveTheme::theme(cx).font_size), px(13.0));
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

#[gpui::test]
fn list_row_is_32_and_28_compact(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    assert_eq!(cx.update(|cx| list_row("l", "One").height(cx)), px(32.0));
    cx.update(|cx| AuiTheme::set_density(Density::Compact, None, cx));
    assert_eq!(cx.update(|cx| list_row("l", "One").height(cx)), px(28.0));
}

/// Three 32 px items: the pointer moving down them reports each in turn and
/// never `None` in between; leaving reports `None`; a click activates the
/// item under it.
#[gpui::test]
fn row_list_reports_the_pointer_and_activates_the_click(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    struct Host {
        reports: Rc<RefCell<Vec<Option<usize>>>>,
        activated: Rc<RefCell<Vec<usize>>>,
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let reports = self.reports.clone();
            let activated = self.activated.clone();
            let mut list = row_list("list")
                .gap(px(0.0))
                .on_highlight(move |index, _, _| reports.borrow_mut().push(index))
                .on_activate(move |index, _, _| activated.borrow_mut().push(index));
            for i in 0..3 {
                list = list.item(i, row(("r", i), format!("Row {i}")));
            }
            div().w(px(300.0)).h(px(400.0)).child(list)
        }
    }
    let reports = Rc::new(RefCell::new(Vec::new()));
    let activated = Rc::new(RefCell::new(Vec::new()));
    let (r, a) = (reports.clone(), activated.clone());
    let (_view, vc) = cx.add_window_view(move |_, _| Host { reports: r, activated: a });
    vc.run_until_parked();
    for y in [10.0, 50.0, 80.0] {
        vc.simulate_mouse_move(gpui::point(px(100.0), px(y)), None, Default::default());
        vc.run_until_parked();
    }
    vc.simulate_mouse_move(gpui::point(px(100.0), px(300.0)), None, Default::default());
    vc.run_until_parked();
    assert_eq!(*reports.borrow(), vec![Some(0), Some(1), Some(2), None]);
    vc.simulate_click(gpui::point(px(100.0), px(50.0)), Default::default());
    vc.run_until_parked();
    assert_eq!(*activated.borrow(), vec![1]);
}

/// A [`dismiss_layer`](aui::interact::dismiss_layer) over a clickable
/// control: the press alone does nothing, the release calls `on_dismiss`
/// once, and the control under the layer never hears the click.
#[gpui::test]
fn dismiss_layer_dismisses_on_the_release_and_swallows_the_click(cx: &mut TestAppContext) {
    use aui::interact::dismiss_layer;
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    struct Host {
        dismissed: Rc<RefCell<usize>>,
        under: Rc<RefCell<usize>>,
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let dismissed = self.dismissed.clone();
            let under = self.under.clone();
            div()
                .relative()
                .w(px(400.0))
                .h(px(400.0))
                .child(div().id("under").size_full().on_click(move |_, _, _| *under.borrow_mut() += 1))
                .child(dismiss_layer("layer", "Close the menu", move |_, _| *dismissed.borrow_mut() += 1))
        }
    }
    let dismissed = Rc::new(RefCell::new(0));
    let under = Rc::new(RefCell::new(0));
    let (d, u) = (dismissed.clone(), under.clone());
    let (_view, vc) = cx.add_window_view(move |_, _| Host { dismissed: d, under: u });
    vc.run_until_parked();
    let at = gpui::point(px(200.0), px(200.0));
    vc.simulate_mouse_down(at, gpui::MouseButton::Left, Default::default());
    vc.run_until_parked();
    assert_eq!(*dismissed.borrow(), 0, "the press alone must not dismiss");
    vc.simulate_mouse_up(at, gpui::MouseButton::Left, Default::default());
    vc.run_until_parked();
    assert_eq!(*dismissed.borrow(), 1, "the release dismisses once");
    assert_eq!(*under.borrow(), 0, "the layer swallows the click");
}

/// `Row::on_press` runs on the button going down, before the release
/// reaches `on_click`; both run once for one click.
#[gpui::test]
fn row_on_press_fires_on_the_press_before_the_click(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    struct Host {
        log: Rc<RefCell<Vec<&'static str>>>,
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let (pressed, clicked) = (self.log.clone(), self.log.clone());
            div().w(px(300.0)).child(
                row("back", "Back")
                    .on_click(move |_, _, _| clicked.borrow_mut().push("click"))
                    .on_press(move |_, _, _| pressed.borrow_mut().push("press")),
            )
        }
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let l = log.clone();
    let (_view, vc) = cx.add_window_view(move |_, _| Host { log: l });
    vc.run_until_parked();
    let at = gpui::point(px(100.0), px(10.0));
    vc.simulate_mouse_down(at, gpui::MouseButton::Left, Default::default());
    vc.run_until_parked();
    assert_eq!(*log.borrow(), vec!["press"], "on_press runs on the press, the click has not landed");
    vc.simulate_mouse_up(at, gpui::MouseButton::Left, Default::default());
    vc.run_until_parked();
    assert_eq!(*log.borrow(), vec!["press", "click"]);
}

/// `RowList::on_press` reports the pressed item's index on the press, ahead
/// of `on_activate` on the release.
#[gpui::test]
fn row_list_on_press_reports_the_index_on_the_press(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    struct Host {
        log: Rc<RefCell<Vec<(&'static str, usize)>>>,
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let (pressed, activated) = (self.log.clone(), self.log.clone());
            let mut list = row_list("suggestions")
                .gap(px(0.0))
                .on_press(move |index, _, _| pressed.borrow_mut().push(("press", index)))
                .on_activate(move |index, _, _| activated.borrow_mut().push(("activate", index)));
            for i in 0..3 {
                list = list.item(i, row(("s", i), format!("Suggestion {i}")));
            }
            div().w(px(300.0)).h(px(400.0)).child(list)
        }
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let l = log.clone();
    let (_view, vc) = cx.add_window_view(move |_, _| Host { log: l });
    vc.run_until_parked();
    // Rows are 32 tall with no gap: y 80 is the third item.
    let at = gpui::point(px(100.0), px(80.0));
    vc.simulate_mouse_down(at, gpui::MouseButton::Left, Default::default());
    vc.run_until_parked();
    assert_eq!(*log.borrow(), vec![("press", 2)]);
    vc.simulate_mouse_up(at, gpui::MouseButton::Left, Default::default());
    vc.run_until_parked();
    assert_eq!(*log.borrow(), vec![("press", 2), ("activate", 2)]);
}

type Slot = Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>;

/// A box of `size` that records where it was laid out.
fn probe(slot: &Slot, size: f32) -> impl IntoElement {
    let slot = slot.clone();
    gpui::canvas(move |bounds, _, _| slot.set(Some(bounds)), |_, _, _, _| {}).size(px(size))
}

/// `Button::leading` draws the caller's element before the label in place
/// of the glyph: it sits inside the button's left edge, and a button with
/// both a glyph and a leading element is exactly as wide as one with the
/// leading element alone.
#[gpui::test]
fn button_leading_takes_the_glyph_slot_before_the_label(cx: &mut TestAppContext) {
    use aui::data::button;
    use aui_icons::IconName;
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    let slots: [Slot; 4] = Default::default();
    let s = slots.clone();
    let (_view, vc) = cx.add_window_view(move |_, _| {
        struct Host {
            s: [Slot; 4],
        }
        impl gpui::Render for Host {
            fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
                let s = &self.s;
                gpui_kit::base::v_flex()
                    .items_start()
                    .child(gpui_kit::base::h_flex().child(button("a", "Opus").leading(probe(&s[0], 16.0))).child(probe(&s[1], 1.0)))
                    .child(
                        gpui_kit::base::h_flex()
                            .child(button("b", "Opus").icon(IconName::Folder).leading(probe(&s[2], 16.0)))
                            .child(probe(&s[3], 1.0)),
                    )
            }
        }
        Host { s }
    });
    vc.run_until_parked();
    let get = |i: usize| slots[i].get().unwrap_or_else(|| panic!("probe {i} was not drawn"));
    let (lead, end) = (get(0), get(1));
    assert!(lead.origin.x > px(0.0), "the leading element sits inside the button's padding: {lead:?}");
    assert!(lead.right() < end.origin.x - px(16.0), "the label follows the leading element: {lead:?} {end:?}");
    assert_eq!(get(2).origin.x, lead.origin.x, "same slot with a glyph set");
    assert_eq!(get(3).origin.x, end.origin.x, "the leading element replaces the glyph, it does not add to it");
}

/// `Button::decorative`: no handler, nothing in the accessibility tree —
/// the live copy beside it names itself and takes the click.
#[gpui::test]
fn decorative_button_takes_no_click_and_has_no_name(cx: &mut TestAppContext) {
    use aui::data::button;
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let c = clicks.clone();
    struct Host {
        clicks: Rc<RefCell<Vec<&'static str>>>,
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let (still, live) = (self.clicks.clone(), self.clicks.clone());
            gpui_kit::base::v_flex()
                .items_start()
                .child(button("still", "Record").decorative(true).on_click(move |_, _, _| still.borrow_mut().push("still")))
                .child(button("live", "Stop").on_click(move |_, _, _| live.borrow_mut().push("live")))
        }
    }
    arm_ax_probe(true);
    take_ax_labels();
    let (_view, vc) = cx.add_window_view(move |_, _| Host { clicks: c });
    vc.run_until_parked();
    let labels = take_ax_labels();
    arm_ax_probe(false);
    assert!(labels.contains(&"Stop".to_string()), "the live button names itself: {labels:?}");
    assert!(!labels.contains(&"Record".to_string()), "the decorative copy stays out of the AX tree: {labels:?}");
    // Buttons are 32 tall: y 10 is the decorative one, y 42 the live one.
    vc.simulate_click(gpui::point(px(10.0), px(10.0)), Default::default());
    vc.run_until_parked();
    vc.simulate_click(gpui::point(px(10.0), px(42.0)), Default::default());
    vc.run_until_parked();
    assert_eq!(*clicks.borrow(), vec!["live"]);
}
