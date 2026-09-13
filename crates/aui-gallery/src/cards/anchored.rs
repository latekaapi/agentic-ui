//! `overlay/anchored` — one seat rule for every trigger menu.
//!
//! Four triggers (top-left, bottom-right, near the right edge, near the
//! bottom) each open a [`view_menu`] through [`anchored_menu`], below-start
//! of the trigger: the bottom triggers flip above and the right-edge trigger
//! slides inside the window. The trigger bounds are measured in prepaint
//! with `on_children_prepainted` and handed back to the seat — never guessed.

use aui::nav::{view_menu, MenuRow};
use aui::overlay::{anchored_menu, MenuAlign, MenuSide};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// The stage the triggers sit in: fixed, so the four seats are comparable.
const STAGE_H: f32 = 430.0;
/// Every trigger is the same fixed box; only its position varies.
const TRIG_W: f32 = 120.0;
const TRIG_H: f32 = 32.0;
/// The note above the stage.
const NOTE_GAP: f32 = 8.0;

/// Where the four triggers sit in the stage: top-left, bottom-right, near
/// the right edge, near the bottom.
static TRIGGERS: [(&str, f32, f32); 4] = [
    ("Top-left", 0.0, 0.0),
    ("Bottom-right", 640.0, 398.0),
    ("Right edge", 640.0, 40.0),
    ("Bottom", 80.0, 398.0),
];

/// The trigger bounds `build` has measured so far, one per trigger.
#[derive(Clone)]
struct AnchoredCard {
    triggers: [Option<Bounds<Pixels>>; 4],
}

/// The rows every demo menu shows.
fn menu_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::Toggle { label: "Show empty groups".into(), checked: false },
        MenuRow::Toggle { label: "Show PR status".into(), checked: true },
        MenuRow::Separator,
        MenuRow::Submenu { label: "Group by".into(), value: "Project".into(), highlighted: false },
        MenuRow::Toggle { label: "Show archived".into(), checked: false },
    ]
}

/// One trigger box: a fixed bordered box whose bounds are reported in
/// prepaint, so the menu seats at the trigger however the stage is placed.
fn trigger(state: Entity<AnchoredCard>, index: usize, label: &'static str, x: f32, y: f32, cx: &mut App) -> impl IntoElement {
    let p = cx.aui().colors;
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(TRIG_W))
        .h(px(TRIG_H))
        .on_children_prepainted(move |bounds, _, cx| {
            let first = bounds.first().copied();
            state.update(cx, |s, cx| {
                if s.triggers[index] != first {
                    s.triggers[index] = first;
                    cx.notify();
                }
            });
        })
        .child(
            h_flex()
                .w_full()
                .h_full()
                .items_center()
                .justify_center()
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(p.line_strong)
                .bg(p.surface_1)
                .ui(scale::FS_12)
                .text_color(p.ink_2)
                .child(label),
        )
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let state = window.use_keyed_state("overlay-anchored", cx, |_, _| AnchoredCard { triggers: [None, None, None, None] });
    let measured = state.read(cx).triggers;

    let mut stage = div().relative().w_full().h(px(STAGE_H));
    for (i, spot) in TRIGGERS.iter().enumerate() {
        stage = stage.child(trigger(state.clone(), i, spot.0, spot.1, spot.2, cx));
    }
    let mut menus: Vec<AnyElement> = Vec::new();
    for (i, trigger) in measured.iter().enumerate() {
        if let Some(trigger) = trigger {
            menus.push(
                anchored_menu(*trigger, MenuSide::Below, MenuAlign::Start, view_menu(("anchored-menu", i), menu_rows()).at_rest())
                    .into_any_element(),
            );
        }
    }
    stage = stage.children(menus);

    v_flex()
        .w_full()
        .child(
            div().mb(px(NOTE_GAP)).ui(scale::FS_12).text_color(p.ink_3).child(
                "Every trigger menu seats below-start of its trigger, flips above near the window bottom, and slides to stay inside — all four through anchored_menu.",
            ),
        )
        .child(stage)
        .into_any_element()
}
