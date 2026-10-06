//! The end-of-run summary: the title line, one criteria row per acceptance
//! criterion with its evidence chips, the diff line, what was not proven,
//! and a footer with the next hint. There are no buttons: the person answers
//! by typing. A criterion that was not met shows the error glyph and no
//! chips; a merged run's footer names the merge commit.

use aui_protocol::{EvidenceKind, RunOutcome, RunSummary};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{glyph_err, glyph_ok, record_ax_label};

/// Card rhythm.
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 10.0;
const SECTION_GAP: f32 = 8.0;
const ROW_GAP: f32 = 6.0;
/// Evidence chips: mono 11 px on a 1 px line border.
const CHIP_PAD_X: f32 = 6.0;
const CHIP_PAD_Y: f32 = 2.0;
/// The screenshot thumbnail placeholder: 56x36 with an inset inner rectangle.
const THUMB_W: f32 = 56.0;
const THUMB_H: f32 = 36.0;
const THUMB_INSET: f32 = 6.0;

/// The run summary. Build with [`run_summary`].
#[derive(IntoElement)]
pub struct RunSummaryCard {
    id: ElementId,
    summary: RunSummary,
}

/// An end-of-run rollup for `summary`.
///
/// The card clones the value; it owns no orchestration logic.
pub fn run_summary(id: impl Into<ElementId>, summary: &RunSummary) -> RunSummaryCard {
    RunSummaryCard { id: id.into(), summary: summary.clone() }
}

/// The outcome word and its ink: finished reads in success, stopped is muted,
/// failed is danger.
fn outcome_word(outcome: RunOutcome, p: &Palette) -> (&'static str, gpui::Hsla) {
    match outcome {
        RunOutcome::Finished => ("finished", p.success),
        RunOutcome::Stopped => ("stopped", p.ink_3),
        RunOutcome::Failed => ("failed", p.danger),
    }
}

/// One evidence chip: muted mono text on a 1 px line border. A screenshot
/// evidence also carries a 56x36 thumbnail placeholder — surface-2 with a
/// line border and a faint accent-soft inner rectangle, divs only.
fn evidence_chip(evidence: &aui_protocol::Evidence, p: &Palette) -> AnyElement {
    let chip = div()
        .flex_none()
        .px(px(CHIP_PAD_X))
        .py(px(CHIP_PAD_Y))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(p.line)
        .text_role(TextRole::MonoSmall)
        .text_color(p.ink_3)
        .child(evidence.label.clone());
    if evidence.evidence_kind == EvidenceKind::Screenshot {
        h_flex()
            .flex_none()
            .items_center()
            .gap(px(ROW_GAP))
            .child(
                div()
                    .flex_none()
                    .w(px(THUMB_W))
                    .h(px(THUMB_H))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.surface_2)
                    .p(px(THUMB_INSET))
                    .child(div().w_full().h_full().rounded(px(scale::R_SM)).bg(p.accent_soft)),
            )
            .child(chip)
            .into_any_element()
    } else {
        chip.into_any_element()
    }
}

impl RenderOnce for RunSummaryCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let (word, word_ink) = outcome_word(self.summary.outcome, &p);
        let title = h_flex()
            .w_full()
            .items_center()
            .gap(px(ROW_GAP))
            .py(px(PAD_Y))
            .px(px(PAD_X))
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .ui(scale::FS_13)
                    .medium()
                    .text_color(p.ink)
                    .child(self.summary.title.clone()),
            )
            .child(div().flex_none().ui(scale::FS_13).text_color(word_ink).child(word))
            .child(div().flex_none().ui(scale::FS_12).text_color(p.ink_3).child(format!(
                "· {} · {} worker{}",
                self.summary.elapsed,
                self.summary.workers,
                if self.summary.workers == 1 { "" } else { "s" }
            )));

        let mut criteria = v_flex().w_full().gap(px(ROW_GAP)).px(px(PAD_X));
        for criterion in &self.summary.criteria {
            let mut row = h_flex()
                .w_full()
                .items_center()
                .gap(px(ROW_GAP))
                .ui(scale::FS_12)
                .child(if criterion.met { glyph_ok() } else { glyph_err() })
                .child(
                    div()
                        .min_w(px(0.0))
                        .truncate()
                        .text_color(p.ink)
                        .child(criterion.text.clone()),
                )
                .child(div().flex_1());
            // A criterion that was not met shows the error glyph and no
            // chips: there is no proof to point at.
            if criterion.met {
                let mut chips = h_flex().flex_none().items_center().gap(px(ROW_GAP));
                for evidence in &criterion.evidence {
                    chips = chips.child(evidence_chip(evidence, &p));
                }
                row = row.child(chips);
            }
            criteria = criteria.child(row);
        }

        // `+412 −36 · 6 files · 2 review rounds · <extra>`: removals use a
        // minus sign, not a hyphen.
        let mut diff_parts =
            vec![format!("{} files", self.summary.files), format!("{} review rounds", self.summary.review_rounds)];
        if let Some(extra) = &self.summary.extra {
            diff_parts.push(extra.clone());
        }
        let diff = h_flex()
            .w_full()
            .items_center()
            .gap(px(ROW_GAP))
            .px(px(PAD_X))
            .mono(scale::FS_12)
            .child(div().flex_none().text_color(p.success).child(format!("+{}", self.summary.additions)))
            .child(div().flex_none().text_color(p.danger).child(format!("−{}", self.summary.deletions)))
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .text_color(p.ink_3)
                    .child(SharedString::from(format!("· {}", diff_parts.join(" · ")))),
            );

        let mut body = v_flex().w_full().gap(px(SECTION_GAP)).child(title).child(criteria).child(diff);
        if !self.summary.not_proven.is_empty() {
            let mut unproven =
                v_flex().w_full().gap(px(scale::SP_1)).px(px(PAD_X)).child(
                    div().text_role(TextRole::Meta).text_color(p.ink_3).child("Not proven:"),
                );
            for item in &self.summary.not_proven {
                unproven = unproven.child(
                    h_flex()
                        .w_full()
                        .gap(px(ROW_GAP))
                        .ui(scale::FS_12)
                        .child(div().flex_none().text_color(p.ink_4).child("·"))
                        .child(div().min_w(px(0.0)).text_color(p.ink_2).child(item.clone())),
                );
            }
            body = body.child(unproven);
        }

        // The footer carries the next hint and no buttons; a merged run
        // names its merge commit instead. An empty hint on an unmerged run
        // draws no footer at all.
        let group_label = format!("Run summary: {}", self.summary.title);
        record_ax_label(&group_label);

        let mut card = v_flex()
            .id(self.id)
            .w_full()
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .overflow_hidden()
            .role(gpui::Role::Group)
            .aria_label(group_label)
            .child(body);
        if self.summary.merged.is_some() || !self.summary.next_hint.is_empty() {
            let footer_text = match &self.summary.merged {
                Some(sha) => format!("Merged {sha}"),
                None => self.summary.next_hint.clone(),
            };
            card = card.child(
                h_flex()
                    .w_full()
                    .gap(px(ROW_GAP))
                    .py(px(PAD_Y))
                    .px(px(PAD_X))
                    .border_t_1()
                    .border_color(p.line)
                    .bg(p.surface_2)
                    .child(
                        div()
                            .min_w(px(0.0))
                            .text_role(TextRole::Meta)
                            .text_color(p.ink_3)
                            .child(footer_text),
                    ),
            );
        }
        card.into_any_element()
    }
}
