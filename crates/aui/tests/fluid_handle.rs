//! `FluidHandle::item`: rows registered from outside the list's own
//! children (a virtualised list's render closure) take part in the one
//! highlight.

use aui::data::{arm_ax_probe, record_ax_label, take_ax_labels};
use aui::interact::{fluid_list, Axis, FluidHandle};
use aui_tokens::ThemeKind;
use gpui::{div, point, prelude::*, px, IntoElement, Modifiers, TestAppContext, VisualTestContext, Window};

struct Host {
    handle: FluidHandle,
}

impl gpui::Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let h = &self.handle;
        record_ax_label("row 1000");
        record_ax_label("row 1001");
        div().w(px(200.0)).h(px(200.0)).child(
            fluid_list("fh", Axis::Y)
                .label("Fluid handle list")
                .gap(px(0.0))
                .handle(h)
                .selected(Some(1001))
                .item_static(0, h.item(1000, div().h(px(40.0)).w_full()))
                .item_static(1, h.item(1001, div().h(px(40.0)).w_full())),
        )
    }
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(ThemeKind::Dark, cx));
}

#[gpui::test]
fn external_items_build_with_the_list_and_no_highlight_before_a_pointer(cx: &mut TestAppContext) {
    init(cx);
    let handle = FluidHandle::new();
    arm_ax_probe(true);
    take_ax_labels();
    let (_host, vcx) = cx.add_window_view({
        let handle = handle.clone();
        |_, _| Host { handle }
    });
    vcx.run_until_parked();
    let labels = take_ax_labels();
    arm_ax_probe(false);
    assert!(labels.iter().any(|l| l == "row 1000") && labels.iter().any(|l| l == "row 1001"), "{labels:?}");
    assert_eq!(handle.highlight(), None, "no pointer yet");
}

#[gpui::test]
fn pointer_over_an_external_item_highlights_its_registered_index(cx: &mut TestAppContext) {
    init(cx);
    let handle = FluidHandle::new();
    let (_host, vcx) = cx.add_window_view({
        let handle = handle.clone();
        |_, _| Host { handle }
    });
    let vcx: &mut VisualTestContext = vcx;
    vcx.run_until_parked();
    // Item 1000 spans y 0-40, item 1001 y 40-80.
    vcx.simulate_mouse_move(point(px(50.0), px(60.0)), None, Modifiers::none());
    assert_eq!(handle.highlight(), Some(1001));
    vcx.simulate_mouse_move(point(px(50.0), px(10.0)), None, Modifiers::none());
    assert_eq!(handle.highlight(), Some(1000));
}
