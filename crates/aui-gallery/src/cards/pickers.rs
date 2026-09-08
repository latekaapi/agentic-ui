//! Composer controls added for the Muse harness: the three chip menus, the
//! context meter in every state it has, the queue strip, and a docked composer
//! wearing the plan pill and an image chip.
//!
//! There is no design HTML behind this card — these components come from the
//! harness spec (§3.4, §3.5, §3.6, §3.10), not from `design/src` — so it is
//! composed to the same rules rather than measured against a reference.

use aui::composer::{
    composer, composer_state_rows, effort_menu, mode_menu, model_menu, queue_strip, ComposerChip,
    ComposerChipAnchor, ComposerChipKind, PickerRow, QueueStripRow,
};
use aui::data::{context_meter, ContextMeterState, ContextPressure};
use aui_icons::Provider;
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.wrap{max-width:820px;gap:22px}`.
const WRAP_MAX: f32 = 820.0;
const WRAP_GAP: f32 = 22.0;
/// The three menus sit side by side with room for each to morph out.
const ROW_GAP: f32 = 28.0;
/// Each open menu is 280 wide and grows upward, so its column reserves the
/// height the tallest of them needs.
const MENU_COLUMN_H: f32 = 250.0;
/// The docked variant spans the card: `margin:0 -20px`.
const DOCKED_BLEED: f32 = -20.0;
/// The caps label above each group: `margin-top:6px`.
const CAPS_TOP: f32 = 6.0;
/// The meters are laid out in a row that wraps.
const METER_GAP: f32 = 28.0;
/// The breakdown popover is 240 tall enough to need its own band.
const METER_BAND_H: f32 = 230.0;

const DRAFT: &str = "Rework the postcode validator, then run the focused tests.";

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let text = window.use_keyed_state("card43-text", cx, |window, cx| {
        let mut state = composer_state_rows("Ask Muse, or type / for commands", 1, 8, window, cx);
        state.set_value(DRAFT, window, cx);
        state
    });

    let models = vec![
        PickerRow::new("muse-spark-1.3", "muse-spark-1.3", "").meta("1.0M ctx").badge("active"),
        PickerRow::new(
            "muse-spark-1.3-contributor",
            "muse-spark-1.3-contributor",
            "Your content, including inter-session messages, may be used for product improvement.",
        )
        .meta("1.0M ctx")
        .badge("default"),
        PickerRow::new("muse-spark-1.2", "muse-spark-1.2", "").meta("1.0M ctx"),
    ];
    let efforts = vec![
        PickerRow::new("none", "None", "No reasoning at all."),
        PickerRow::new("low", "Low", "A short budget."),
        PickerRow::new("medium", "Medium", "The middle budget, and the usual default."),
        PickerRow::new("ultra", "Ultra", "The largest budget MSP accepts."),
    ];
    let modes = vec![
        PickerRow::new("allowAll", "Full access", "Nothing is gated; every action runs."),
        PickerRow::new("onRequest", "Auto", "Policy decides; unmatched actions ask only when the agent requests it."),
        PickerRow::new("promptUnmatched", "Ask", "Anything without a matching rule raises an approval card."),
        PickerRow::new("denyUnmatched", "Read-only", "Anything without a matching rule is refused."),
    ];

    let meters = [
        ("normal", ContextMeterState { used_tokens: 19_328, window_tokens: Some(200_000), pressure: ContextPressure::Normal, prompt_tokens: 19_213, output_tokens: 115, total_tokens: 19_328 }, false),
        ("warning", ContextMeterState { used_tokens: 168_000, window_tokens: Some(200_000), pressure: ContextPressure::Warning, prompt_tokens: 166_400, output_tokens: 1_600, total_tokens: 168_000 }, false),
        ("blocked", ContextMeterState { used_tokens: 201_400, window_tokens: Some(200_000), pressure: ContextPressure::Blocked, prompt_tokens: 199_000, output_tokens: 2_400, total_tokens: 201_400 }, false),
        ("no window", ContextMeterState { used_tokens: 19_328, window_tokens: None, pressure: ContextPressure::Normal, prompt_tokens: 19_213, output_tokens: 115, total_tokens: 19_328 }, false),
        ("hovered", ContextMeterState { used_tokens: 19_328, window_tokens: Some(200_000), pressure: ContextPressure::Normal, prompt_tokens: 19_213, output_tokens: 115, total_tokens: 19_328 }, true),
    ];
    let mut meter_row = h_flex().w_full().items_end().gap(px(METER_GAP)).h(px(METER_BAND_H));
    for (name, state, open) in meters {
        meter_row = meter_row.child(
            v_flex()
                .flex_none()
                .gap(px(scale::SP_2))
                .child(div().relative().child(context_meter(SharedString::from(format!("card43-meter-{name}")), state).open(open).on_compact(|_, _| {})))
                .child(div().text_role(TextRole::Caps).text_color(p.ink_3).child(name)),
        );
    }

    let queue = queue_strip(
        "card43-queue",
        vec![
            QueueStripRow::new("t1", "Also run the e2e suite for checkout after unit tests pass"),
            QueueStripRow::new("t2", "Then open a PR against main with the release notes").editing(),
            QueueStripRow::new("t3", "And update the changelog"),
        ],
    )
    .on_intent(|_, _, _, _| {});

    v_flex()
        .w_full()
        .max_w(px(WRAP_MAX))
        .gap(px(WRAP_GAP))
        .child(caps(cx, "Chip menus"))
        .child(
            h_flex()
                .w_full()
                .items_end()
                .gap(px(ROW_GAP))
                .h(px(MENU_COLUMN_H))
                .child(div().relative().flex_1().child(model_menu("card43-model", models, 0, true).at_rest().on_pick(|_, _, _| {})))
                .child(div().relative().flex_1().child(effort_menu("card43-effort", efforts, 2, true).at_rest().on_pick(|_, _, _| {})))
                .child(div().relative().flex_1().child(mode_menu("card43-mode", modes, 1, true).at_rest().on_pick(|_, _, _| {}))),
        )
        .child(caps(cx, "Context meter"))
        .child(meter_row)
        .child(caps(cx, "Queue strip"))
        .child(queue)
        .child(caps(cx, "Composer in plan mode"))
        .child(
            div().w_full().mx(px(DOCKED_BLEED)).child(
                composer("card43-composer", &text, Provider::Muse, "muse-spark-1.3")
                    .docked(true)
                    .mode("Read-only")
                    .effort("Medium")
                    .plan(true)
                    .chips(vec![ComposerChip {
                        id: "shot".into(),
                        kind: ComposerChipKind::Image,
                        label: "checkout.png".into(),
                        removable: true,
                    }])
                    .context(ContextMeterState {
                        used_tokens: 19_328,
                        window_tokens: Some(200_000),
                        pressure: ContextPressure::Normal,
                        prompt_tokens: 19_213,
                        output_tokens: 115,
                        total_tokens: 19_328,
                    })
                    .chip_menu(ComposerChipAnchor::Model, div())
                    .on_intent(|_, _, _| {}),
            ),
        )
        .into_any_element()
}

fn caps(cx: &mut App, label: &'static str) -> AnyElement {
    let p = cx.aui().colors;
    div().mt(px(CAPS_TOP)).text_role(TextRole::Caps).text_color(p.ink_3).child(label).into_any_element()
}
