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
