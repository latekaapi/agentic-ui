//! Card 35 · Approval card: the pending request, then the four resolved
//! states in a two-column grid. Reproduces
//! `design/src/cards/transcript/35-approval.html` at 760×640.
//!
//! Below the design grid, the shapes a provider-minted request adds: the
//! server's own choices on a staged shell subject (stage 1 of 2, then stage 2
//! with the first resolved), the feedback field a choice opens, the header
//! badges, and the three resolutions nobody was asked for.

use aui::protocol::{sample, Block};
use aui::transcript::{approval_card, ApprovalCard};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::input::Textarea;

/// The caps rule that separates the design grid from the provider shapes.
fn section(label: &'static str, p: &Palette) -> impl IntoElement {
    div()
        .w_full()
        .mt(px(scale::SP_3))
        .text_role(aui_tokens::TextRole::Caps)
        .line_height(relative(scale::LH_UI))
        .text_color(p.ink_3)
        .child(label)
}

/// `.ap{margin-bottom:14px}` between the pending card and the grid.
const BLOCK_GAP: f32 = 14.0;
/// `.two{gap:12px}`.
const GRID_GAP: f32 = 12.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

/// Turns one sample [`Block::Approval`] into a card.
fn card(id: &'static str, block: &Block) -> ApprovalCard {
    let Block::Approval { tool, command, reason, cwd, capabilities, scope, state, rule, .. } = block else {
        unreachable!("sample::approvals yields approval blocks only")
    };
    let mut el = approval_card(id, tool.clone(), command.clone(), state.clone())
        .reason(reason.clone())
        .cwd(cwd.clone())
        .capabilities(capabilities.clone())
        .scope(*scope)
        .at_rest();
    if let Some(rule) = rule {
        el = el.rule(rule.clone());
    }
    el
}

/// Turns one sample [`Block::Approval`] from [`sample::muse_approvals`] into a
/// card, choices, stages, badges and resolution included.
fn muse_card(id: &'static str, block: &Block) -> ApprovalCard {
    let Block::Approval { tool, command, reason, cwd, capabilities, scope, state, choices, stages, current_stage, badges, feedback, resolved_by, .. } = block
    else {
        unreachable!("sample::muse_approvals yields approval blocks only")
    };
    let mut el = approval_card(id, tool.clone(), command.clone(), state.clone())
        .title("Allow Muse to run this command?")
        .reason(reason.clone())
        .cwd(cwd.clone())
        .capabilities(capabilities.clone())
        .scope(*scope)
        .choices(choices.clone())
        .stages(stages.clone(), *current_stage)
        .badges(*badges)
        .resolved_by(*resolved_by)
        .at_rest();
    if let Some(feedback) = feedback {
        el = el.feedback(feedback.clone());
    }
    el
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let blocks = sample::approvals();
    let muse = sample::muse_approvals();
    // The feedback field is the host's, as it is in the harness: the card is
    // handed an element and never a character of text.
    let feedback = window.use_keyed_state("card35-feedback-input", cx, |window, cx| {
        TextareaState::new(window, cx).placeholder("Why not? The agent will read this.").auto_grow(2, 4)
    });
    // The denied sample runs `rm -rf node_modules`; the auto-allowed one
    // carries its own remembered rule.
    let quiet = |i: usize, id: &'static str| card(id, &blocks[i]).into_any_element();
    let row = |a: AnyElement, b: AnyElement| {
        h_flex()
            .w_full()
            .items_stretch()
            .gap(px(GRID_GAP))
            // The grid stretches both cells, so the shorter card grows to the
            // height of its neighbour (`.two{display:grid}`).
            .child(h_flex().flex_1().min_w(px(0.0)).items_stretch().child(a))
            .child(h_flex().flex_1().min_w(px(0.0)).items_stretch().child(b))
    };
    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(card("card35-pending", &blocks[0]).on_decide(|_, _, _| {}))
        .child(
            v_flex()
                .w_full()
                .gap(px(GRID_GAP))
                .child(row(quiet(1, "card35-approving"), quiet(2, "card35-allowed")))
                .child(row(quiet(3, "card35-denied"), card("card35-rule", &blocks[4]).on_manage_rules(|_, _| {}).into_any_element())),
        )
        .child(
            v_flex()
                .w_full()
                .gap(px(GRID_GAP))
                .child(section("Server-minted choices", &p))
                .child(muse_card("card35-stage-1", &muse[0]).on_choose(|_, _, _, _| {}).on_feedback_toggle(|_, _, _| {}))
                .child(muse_card("card35-stage-2", &muse[1]).on_choose(|_, _, _, _| {}).on_feedback_toggle(|_, _, _| {}))
                .child(
                    muse_card("card35-feedback", &muse[0])
                        .feedback_open(Some("deny".into()))
                        .feedback_slot(Textarea::new(&feedback).text_size(aui_tokens::scaled(scale::FS_12)))
                        .on_choose(|_, _, _, _| {})
                        .on_feedback_toggle(|_, _, _| {}),
                )
                .child(muse_card("card35-badges", &muse[2]).on_choose(|_, _, _, _| {}).on_feedback_toggle(|_, _, _| {}))
                .child(section("Resolved without asking", &p))
                .child(row(muse_card("card35-policy-allowed", &muse[3]).into_any_element(), muse_card("card35-policy-denied", &muse[4]).into_any_element()))
                .child(row(muse_card("card35-judge-denied", &muse[5]).into_any_element(), muse_card("card35-user-denied", &muse[6]).into_any_element())),
        )
        .child(
            div()
                .mt(px(scale::SP_4) - px(BLOCK_GAP))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Pending is the only marked state: a plain 1 px border in the warning colour, nothing else. Buttons enter staggered by 40 ms on ease-out, primary on the right, keys on the left. The remembered rule is spelled out on the Always button so the person knows exactly what they are granting. Once resolved the card goes quiet and single-line."),
        )
        .into_any_element()
}
