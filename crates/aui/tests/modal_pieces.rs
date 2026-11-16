//! The host-built modal pieces: [`dialog_sheet`], [`modal_panel`] and
//! [`Dialog::leading`](aui::overlay::Dialog::leading).
//!
//! - `dialog_sheet` is the sheet frame with no provider mark: [`SETUP_W`]
//!   wide, its ✕ named after the title and reporting through `on_close`;
//! - `modal_panel` is drawn at rest, so it is exactly as wide as asked;
//! - a dialog's leading element leads the body — on the body's first line,
//!   the text one gap to its right.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use aui::data::{arm_ax_probe, button, take_ax_labels};
use aui::overlay::{dialog, modal_panel};
use aui::screens::{dialog_sheet, SETUP_W};
use aui_tokens::{scale, ThemeKind};
use gpui::{div, prelude::*, px, Bounds, IntoElement, Pixels, TestAppContext, Window};
use gpui_kit::base::h_flex;

type Slot = Rc<Cell<Option<Bounds<Pixels>>>>;

/// A box of `size` that records where it was laid out.
fn probe(slot: &Slot, size: f32) -> impl IntoElement {
    let slot = slot.clone();
    gpui::canvas(move |bounds, _, _| slot.set(Some(bounds)), |_, _, _, _| {}).size(px(size))
}

fn drawn(slot: &Slot, name: &str) -> Bounds<Pixels> {
    slot.get().unwrap_or_else(|| panic!("{name} was not drawn"))
}

#[gpui::test]
fn dialog_sheet_is_setup_wide_and_its_close_reports(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    struct Host {
        closed: Rc<RefCell<usize>>,
        end: Slot,
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let closed = self.closed.clone();
            h_flex()
                .items_start()
                .child(
                    dialog_sheet("import", "Import from Chrome")
                        .child(div().h(px(scale::SP_7)))
                        .actions(vec![button("import-go", "Import").primary().into_any_element()])
                        .on_close(move |_, _| *closed.borrow_mut() += 1),
                )
                .child(probe(&self.end, 1.0))
        }
    }
    let closed = Rc::new(RefCell::new(0));
    let end: Slot = Rc::default();
    let (c, e) = (closed.clone(), end.clone());
    arm_ax_probe(true);
    take_ax_labels();
    let (_view, vc) = cx.add_window_view(move |_, _| Host { closed: c, end: e });
    vc.run_until_parked();
    let labels = take_ax_labels();
    arm_ax_probe(false);
    assert_eq!(drawn(&end, "the end probe").origin.x, px(SETUP_W), "the sheet is SETUP_W wide by default");
    // The ✕ and the action, nothing else: no provider mark, no unnamed control.
    assert_eq!(labels, vec!["Close Import from Chrome".to_string(), "Import".to_string()]);
    // The ✕ is the last thing in the 44 px header: 8 in from the right edge,
    // a 28 px square, so its centre is 22 in.
    vc.simulate_click(gpui::point(px(SETUP_W - 22.0), px(scale::BAR_DEFAULT / 2.0)), Default::default());
    vc.run_until_parked();
    assert_eq!(*closed.borrow(), 1, "the ✕ reports through on_close");
}

#[gpui::test]
fn modal_panel_is_exactly_as_wide_as_asked(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Light, cx));
    let ends: [Slot; 2] = Default::default();
    let e = ends.clone();
    struct Host {
        ends: [Slot; 2],
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            gpui_kit::base::v_flex()
                .items_start()
                .child(h_flex().child(modal_panel(320.0, cx).h(px(scale::SP_7))).child(probe(&self.ends[0], 1.0)))
                .child(h_flex().child(modal_panel(480.0, cx).h(px(scale::SP_7))).child(probe(&self.ends[1], 1.0)))
        }
    }
    let (_view, vc) = cx.add_window_view(move |_, _| Host { ends: e });
    vc.run_until_parked();
    // At rest: no 98.5 % enter scale, the width as given.
    assert_eq!(drawn(&ends[0], "the first end").origin.x, px(320.0));
    assert_eq!(drawn(&ends[1], "the second end").origin.x, px(480.0));
}

/// Draws a dialog whose body text is a 56 px probe, with or without a
/// 56 px leading element; returns (leading, text) bounds.
fn dialog_probes(cx: &mut TestAppContext, with_leading: bool) -> (Option<Bounds<Pixels>>, Bounds<Pixels>) {
    struct Host {
        lead: Option<Slot>,
        text: Slot,
    }
    impl gpui::Render for Host {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let text = self.text.clone();
            let mut card = dialog("crash", "Baaz stopped the agent")
                .rich_body(move |_| probe(&text, 56.0).into_any_element())
                .primary("Restart")
                .at_rest();
            if let Some(lead) = &self.lead {
                card = card.leading(probe(lead, 56.0));
            }
            div().relative().w(px(800.0)).h(px(600.0)).child(card)
        }
    }
    let lead: Option<Slot> = with_leading.then(Rc::default);
    let text: Slot = Rc::default();
    let (l, t) = (lead.clone(), text.clone());
    let (_view, vc) = cx.add_window_view(move |_, _| Host { lead: l, text: t });
    vc.run_until_parked();
    (lead.map(|slot| drawn(&slot, "the leading element")), drawn(&text, "the body text"))
}

#[gpui::test]
fn dialog_leading_leads_the_body(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
    let (lead, text) = dialog_probes(cx, true);
    let lead = lead.expect("asked for a leading element");
    assert_eq!(lead.origin.y, text.origin.y, "the leading element sits on the body's first line");
    assert_eq!(text.origin.x - lead.right(), px(scale::SP_4), "the text is one card gap to its right");
    // The header is unchanged: with the tile in place of the leading
    // element the body starts at the same height. A 56 px element on the
    // title row would have grown the header and pushed the body down.
    let (_, plain) = dialog_probes(cx, false);
    assert_eq!(plain.origin.y, text.origin.y, "the leading element is in the body, not the header");
    assert!(plain.origin.x < text.origin.x, "without it the text starts at the body's edge");
}
