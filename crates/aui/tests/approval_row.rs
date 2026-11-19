//! The approval card's action row fits on one line at every text size and
//! density (docs/82 5.9): "Once · Always here · Deny" with the rule
//! "Always here" would install under the row, cut with an ellipsis — never
//! inside its button, where at Large text it pushed Deny onto a line of its
//! own. Measured on real frames of the test platform (its text system
//! advances every glyph 0.6 em, a little wider than the UI face, so a row
//! that fits here fits on screen).

use aui::protocol::{ApprovalChoice, ApprovalDecision, ApprovalScope, ApprovalState};
use aui::transcript::approval_card;
use aui_tokens::{AuiTheme, Density, TextSize};
use gpui::{div, prelude::*, px, size, Bounds, IntoElement, Pixels, TestAppContext, Window};

/// Muse's card as the vocabulary restates it (provider `approval.rs`):
/// Once, Always here with its rule preview, Deny.
fn muse_choices() -> Vec<ApprovalChoice> {
    let choice = |id: &str, label: &str, decision, rule: Option<&str>| ApprovalChoice {
        id: id.into(),
        label: label.into(),
        decision,
        scope: if decision == ApprovalDecision::PolicyAmendment { ApprovalScope::ThisWorktree } else { ApprovalScope::ThisCommand },
        rule_preview: rule.map(str::to_owned),
        accepts_feedback: false,
    };
    vec![
        choice("allow_once", "Once", ApprovalDecision::Once, None),
        choice("allow_local_prefix", "Always here", ApprovalDecision::PolicyAmendment, Some("Always allow in this workspace: echo ...")),
        choice("abort", "Deny", ApprovalDecision::Deny, None),
    ]
}

struct Host {
    width: f32,
}

impl gpui::Render for Host {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(self.width)).child(
            approval_card("row-test", "Shell", "echo hi && ls", ApprovalState::Pending).choices(muse_choices()).at_rest(),
        )
    }
}

/// The three buttons' bounds and the rule line's, at `width` under the
/// given type settings.
fn measure(cx: &mut TestAppContext, width: f32, text: TextSize, density: Density) -> (Vec<Bounds<Pixels>>, Bounds<Pixels>) {
    cx.update(|cx| {
        aui::init(aui_tokens::ThemeKind::Dark, cx);
        AuiTheme::set_text_size(text, None, cx);
        AuiTheme::set_density(density, None, cx);
    });
    let (_host, vc) = cx.add_window_view(move |_, _| Host { width });
    vc.simulate_resize(size(px(1200.0), px(800.0)));
    vc.run_until_parked();
    vc.update(|window, cx| window.draw(cx).clear(cx));
    let buttons = (0..3)
        .map(|index| vc.debug_bounds(Box::leak(format!("approval-choice-{index}").into_boxed_str())).expect("every choice draws"))
        .collect();
    let rule = vc.debug_bounds("approval-rule").expect("the rule preview draws");
    (buttons, rule)
}

#[gpui::test]
fn the_action_row_is_one_line_at_every_text_size(cx: &mut TestAppContext) {
    // The transcript's measure, and the centre with the right pane open.
    for width in [760.0, 600.0] {
        for (text, density) in [
            (TextSize::Default, Density::Comfortable),
            (TextSize::Default, Density::Compact),
            (TextSize::Large, Density::Comfortable),
            (TextSize::Large, Density::Compact),
        ] {
            let (buttons, rule) = measure(cx, width, text, density);
            let at = format!("{width} px, {text:?}, {density:?}");
            let top = buttons[0].top();
            for (index, button) in buttons.iter().enumerate() {
                assert!((f32::from(button.top()) - f32::from(top)).abs() < 0.5, "{at}: choice {index} left the row: {buttons:?}");
            }
            let deny = buttons[2];
            assert!(f32::from(deny.right()) <= width + 0.5, "{at}: Deny is inside the card: {deny:?}");
            let row_bottom = buttons.iter().map(|b| f32::from(b.bottom())).fold(0.0, f32::max);
            assert!(f32::from(rule.top()) >= row_bottom - 0.5, "{at}: the rule sits under the row: {rule:?} vs {row_bottom}");
            assert!(f32::from(rule.right()) <= width + 0.5, "{at}: the rule is cut inside the card: {rule:?}");
        }
    }
}

#[gpui::test]
fn a_narrow_card_cuts_the_rule_and_keeps_every_button(cx: &mut TestAppContext) {
    // Too narrow for the whole rule: it is cut, on one line, inside the card.
    let (buttons, rule) = measure(cx, 360.0, TextSize::Large, Density::Comfortable);
    let line = f32::from(rule.size.height);
    assert!(line < 24.0, "the rule stays one line, {line} px");
    assert!(f32::from(rule.right()) <= 360.5, "the rule is cut inside the card: {rule:?}");
    assert_eq!(buttons.len(), 3, "no button is dropped");
}
