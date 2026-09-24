//! The composer's provider chip.
//!
//! The provider chip carries the provider mark and the provider's display
//! name, sits immediately left of the model chip, hangs its menu off
//! [`ComposerChipAnchor::Provider`](aui::composer::ComposerChipAnchor), and
//! reports `Provider` on click — while the model chip carries only the model
//! name. Headless windows have no rasterizer, so what a test can prove is
//! intent: the host wires only `on_intent`, and a click sweep across the
//! toolbar band can log only through the chips in the row.

use std::cell::RefCell;
use std::rc::Rc;

use aui::composer::{composer, composer_state, provider_display_name, ComposerChipAnchor};
use aui::icons::Provider;
use gpui::{div, point, px, IntoElement, Modifiers, TestAppContext, VisualTestContext};

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

struct ProviderHost {
    log: Log,
}

impl gpui::Render for ProviderHost {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let text = window.use_keyed_state("provider-chip-text", cx, |window, cx| {
            composer_state("Reply, or type / for commands", window, cx)
        });
        let log = self.log.clone();
        composer("provider-chip", &text, Provider::Muse, "muse-spark-1.3-contributor")
            .chip_menu(ComposerChipAnchor::Provider, div())
            .on_intent(move |intent, _, _| {
                log.borrow_mut().push(format!("{intent:?}"));
            })
    }
}

/// Clicks across the toolbar band; returns what the composer reported. Only
/// the `+` button and the chips have handlers, so only they can log.
fn sweep_toolbar(cx: &mut TestAppContext) -> Vec<String> {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (_host, vcx) = cx.add_window_view({
        let log = log.clone();
        |_, _| ProviderHost { log }
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

fn first(log: &[String], name: &str) -> usize {
    log.iter().position(|entry| entry == name).unwrap_or_else(|| panic!("no click in the band reached {name}, got {log:?}"))
}

/// The provider chip reports `Provider` on click, immediately left of the
/// model chip: the row reads `[+] [provider] [model] [mode]`.
#[gpui::test]
fn provider_chip_click_reports_provider_left_of_model(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let log = sweep_toolbar(cx);
    let plus = first(&log, "TogglePlus");
    let provider = first(&log, "Provider");
    let model = first(&log, "Model");
    let mode = first(&log, "Mode");
    assert!(
        plus < provider && provider < model && model < mode,
        "the action row must read [+] [provider] [model] [mode], got {log:?}"
    );
    assert!(
        log[provider..model].iter().all(|entry| entry == "Provider"),
        "nothing clickable may sit between the provider and model chips, got {log:?}"
    );
}

/// Provider labels are user-visible strings from an explicit table — never
/// the `Debug` form of the enum (`Claude` would read as-is; users see
/// "Claude Code").
#[test]
fn provider_display_names_are_human_readable() {
    assert_eq!(provider_display_name(Provider::Muse).to_string(), "Muse");
    assert_eq!(provider_display_name(Provider::Claude).to_string(), "Claude Code");
    assert_eq!(provider_display_name(Provider::Codex).to_string(), "Codex");
    for provider in Provider::ALL {
        assert!(!provider_display_name(*provider).is_empty(), "{provider:?} needs a display name");
    }
}
