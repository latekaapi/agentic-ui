//! The composer model chip's chevron and click.
//!
//! The chip shows its chevron and opens the model picker only when there is
//! more than one model to pick from; with a single choice it renders quiet.
//! Headless windows have no rasterizer, so what a test can prove is intent:
//! a click sweep across the toolbar band can log `Model` only through a
//! clickable model chip.

use std::cell::RefCell;
use std::rc::Rc;

use aui::composer::{composer, composer_state, ComposerIntent};
use aui::icons::Provider;
use gpui::{point, px, IntoElement, Modifiers, TestAppContext, VisualTestContext};

/// Sweep band: below the text area, across the toolbar and a little beyond
/// it, so bar-height drift still leaves several rows inside the chips.
const SWEEP_TOP: f32 = 40.0;
const SWEEP_BOTTOM: f32 = 120.0;
const SWEEP_STEP_Y: f32 = 4.0;
/// Clicks sweep the left of the toolbar, where `+` and the chips live; the
/// send button at the far right is outside the band on purpose.
const SWEEP_LEFT: f32 = 4.0;
const SWEEP_RIGHT: f32 = 600.0;
const SWEEP_STEP_X: f32 = 6.0;

/// What the composer reported, in order.
type Log = Rc<RefCell<Vec<String>>>;

struct ModelHost {
    log: Log,
    choices: usize,
}

impl gpui::Render for ModelHost {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let text = window.use_keyed_state("model-chip-text", cx, |window, cx| {
            composer_state("Reply, or type / for commands", window, cx)
        });
        let log = self.log.clone();
        composer("model-chip", &text, Provider::Muse, "muse-spark-1.3-contributor")
            .model_choices(self.choices)
            .on_intent(move |intent, _, _| {
                log.borrow_mut().push(format!("{intent:?}"));
            })
    }
}

/// Clicks across the toolbar band; returns what the composer reported. Only
/// the `+` button and clickable chips have handlers, so only they can log.
fn sweep_toolbar(cx: &mut TestAppContext, choices: usize) -> Vec<String> {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        |_, _| ModelHost { log, choices }
    });
    let vcx: &mut VisualTestContext = vcx;
    let mut y = SWEEP_TOP;
    while y <= SWEEP_BOTTOM {
        let mut x = SWEEP_LEFT;
        while x <= SWEEP_RIGHT {
            vcx.simulate_click(point(px(x), px(y)), Modifiers::none());
            x += SWEEP_STEP_X;
        }
        vcx.run_until_parked();
        y += SWEEP_STEP_Y;
    }
    let out = log.borrow().clone();
    out
}

/// With two models to pick from, the chip is clickable and reports `Model`.
#[gpui::test]
fn model_chip_with_two_choices_reports_model(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let log = sweep_toolbar(cx, 2);
    assert!(log.iter().any(|entry| entry == "Model"), "the model chip must be clickable with 2 choices, got {log:?}");
}

/// With a single choice there is nothing to pick: the chip carries no
/// chevron and no click, while the neighbouring chips still report.
#[gpui::test]
fn model_chip_with_one_choice_is_quiet(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let log = sweep_toolbar(cx, 1);
    assert!(!log.iter().any(|entry| entry == "Model"), "the model chip must not be clickable with 1 choice, got {log:?}");
    assert!(log.iter().any(|entry| entry == "Provider"), "the sweep must still reach the provider chip, got {log:?}");
}

/// The intent the chip reports when it is clickable.
#[test]
fn model_intent_debug_name_is_stable() {
    assert_eq!(format!("{:?}", ComposerIntent::Model), "Model");
}
