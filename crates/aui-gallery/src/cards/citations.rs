//! Card 55 · Sources and citations. The assistant answer with inline citation
//! markers, the sources card grouped by retrieval tier and the source hover
//! card. Reproduces `design/src/cards/workbench/55-citations.html` at 760×520,
//! light theme.

use aui::transcript::ProseStyle;
use aui::workbench::{cited_answer, source_hover_card, sources_card, Source, SourceTier, ANSWER_MARGIN};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.a{font-size:13.5px;line-height:1.65}`; `.a p{margin:0 0 10px}` (one
/// paragraph here, so the paragraph gap never applies).
const BODY_TEXT: f32 = 13.5;
const PARAGRAPH_GAP: f32 = 10.0;
/// `.ds-note{max-width:80ch;margin-top:12px}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

/// The answer; `[[n]]` marks a citation, as in the HTML's `<span class="cite">`.
const ANSWER: &str = "Under the current rules a bidder needs a valid registration and three years of comparable placements[[1]]. The 2024 government order tightened document verification to require original certificates at onboarding[[2]], and the project brief sets the cohort at 240 teachers across 38 institutions[[3]]. I have folded all three into section 2 of the draft.";

/// The quoted passage and the matched span (`<mark>`) inside it.
const QUOTE: &str = "“…shall hold a valid registration with the Directorate and shall have completed not less than three years of comparable placements in the preceding five years.”";
const HIGHLIGHT: &str = "not less than three years of comparable placements";

/// The three tiers of the card: role, project, session.
fn tiers() -> Vec<SourceTier> {
    vec![
        SourceTier::new("Role")
            .name("Director, Education")
            .source(Source::cited(
                1,
                "Procurement Rules 2019, Rule 14(2)",
                "procurement-rules-2019.pdf · p. 31 · registration and experience threshold",
                0.92,
            ))
            .source(Source::cited(
                2,
                "GO 2024-18 · Verification of credentials",
                "go-2024-18.pdf · §4 · original certificates at onboarding",
                0.88,
            )),
        SourceTier::new("Project")
            .name("Teacher recruitment RFP")
            .source(Source::cited(3, "Project brief", "brief.docx · cohort size and institution count", 0.97)),
        SourceTier::new("Session").source(Source::uncited("Eligibility criteria review", "earlier in this session · not cited")),
    ]
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let style = ProseStyle {
        ink: p.ink,
        code_ink: p.accent_ink,
        code_bg: p.accent_soft,
        size: BODY_TEXT,
        line_height: scale::LH_BODY,
        paragraph_gap: PARAGRAPH_GAP,
    };
    let start = QUOTE.find(HIGHLIGHT).unwrap_or(0);
    let hover = source_hover_card(
        "card55-hover",
        "Rule 14(2) · Eligibility of bidders",
        QUOTE,
        start..start + HIGHLIGHT.len(),
        31,
    )
    .at_rest();

    v_flex()
        .w_full()
        .child(div().w_full().mb(px(ANSWER_MARGIN)).child(cited_answer("card55-answer", ANSWER, style)))
        .child(sources_card("card55-sources", tiers()).cited(3).retrieved(11).hover_card(0, hover))
        .child(
            div()
                .mt(px(scale::SP_4))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Citation markers are small accent squares that lift on hover and open a source card with the exact passage highlighted. The sources list is grouped by retrieval tier so it is obvious whether a claim rests on a regulation, a project file or session chat; the bar is retrieval confidence."),
        )
        .into_any_element()
}
