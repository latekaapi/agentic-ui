//! Prose: the small markdown subset assistant and user turns use —
//! paragraphs, bullet lists, inline code and bold — rendered as styled text
//! runs so inline code can switch to the mono face (gpui-kit's markdown view
//! keeps one font per paragraph).

use aui_motion::{looping, Loop};
use aui_tokens::{scale, AuiStyled};
use gpui::{div, font, prelude::*, px, relative, App, ElementId, Font, FontStyle, FontWeight, Hsla, IntoElement, StrikethroughStyle, StyledText, TextRun, UnderlineStyle, Window};
use gpui_kit::base::{h_flex, v_flex};

use super::markdown::{Span, last_block_runs};

/// `.a ul{padding-left:18px}`.
const LIST_INDENT: f32 = 18.0;
/// `.a ul{margin:6px 0}` — the top margin collapses into the paragraph gap
/// above; the bottom one stands when a paragraph follows.
const LIST_MARGIN: f32 = 6.0;

/// `.caret{width:2px;height:15px;background:var(--accent);vertical-align:-3px;
/// margin-left:1px;animation:blink 1s steps(2) infinite}` — the streaming
/// caret's geometry, shared by every turn that can stream.
pub const CARET_W: f32 = 2.0;
/// The caret's height.
pub const CARET_H: f32 = 15.0;
/// The caret's gap from the last glyph.
pub const CARET_MARGIN_LEFT: f32 = 1.0;
/// `vertical-align:-3px`: the caret's box hangs 3 px below the text baseline.
pub const CARET_BASELINE_DROP: f32 = 3.0;
/// One blink.
const CARET_PERIOD: std::time::Duration = std::time::Duration::from_millis(1000);

/// Whether the streaming caret is on this frame. `blink 1s steps(2)` is on for
/// the first half of every second; reduced motion holds it on.
pub fn caret_visible(id: impl Into<gpui_kit::base::TransitionId>, window: &mut Window, cx: &mut App) -> bool {
    if cx.reduce_motion() {
        return true;
    }
    looping(id, Loop::linear(CARET_PERIOD), window, cx) < 0.5
}

/// The caret's offset from the top of the line box it closes: centred in the
/// line, then dropped by `vertical-align`.
pub fn caret_top_in_line(line_height: gpui::Pixels, caret_height: gpui::Pixels, text_scale: f32) -> gpui::Pixels {
    (line_height - caret_height) / 2.0 + px(CARET_BASELINE_DROP * text_scale)
}

/// Colours and sizes for a prose block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProseStyle {
    /// Body text colour.
    pub ink: Hsla,
    /// Inline code text colour (the mention chip uses accent-ink).
    pub code_ink: Hsla,
    /// Inline code ground.
    pub code_bg: Hsla,
    /// Body size in design px.
    pub size: f32,
    /// Line height ratio.
    pub line_height: f32,
    /// Gap between paragraphs.
    pub paragraph_gap: f32,
}

// `Span` lives in `super::markdown` so both renderers shape the same runs;
// this module's subset parser only ever emits Text, Code and Bold.

/// One block.
#[derive(Debug, Clone, PartialEq)]
enum Block {
    Paragraph(Vec<Span>),
    List(Vec<Vec<Span>>),
}

fn parse_inline(s: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '`' => {
                if !buf.is_empty() {
                    out.push(Span::Text(std::mem::take(&mut buf)));
                }
                let mut code = String::new();
                for d in chars.by_ref() {
                    if d == '`' {
                        break;
                    }
                    code.push(d);
                }
                out.push(Span::Code(code));
            }
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if !buf.is_empty() {
                    out.push(Span::Text(std::mem::take(&mut buf)));
                }
                let mut bold = String::new();
                while let Some(d) = chars.next() {
                    if d == '*' && chars.peek() == Some(&'*') {
                        chars.next();
                        break;
                    }
                    bold.push(d);
                }
                out.push(Span::Bold(bold));
            }
            _ => buf.push(c),
        }
    }
    if !buf.is_empty() {
        out.push(Span::Text(buf));
    }
    out
}

fn parse(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    for chunk in markdown.split("\n\n") {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let lines: Vec<&str> = chunk.lines().collect();
        if lines.iter().all(|l| l.trim_start().starts_with("- ")) {
            blocks.push(Block::List(lines.iter().map(|l| parse_inline(l.trim_start()[2..].trim())).collect()));
        } else {
            blocks.push(Block::Paragraph(parse_inline(&lines.join(" "))));
        }
    }
    blocks
}

fn runs(spans: &[Span], style: &ProseStyle) -> (String, Vec<TextRun>) {
    let ui: Font = font(scale::FONT_UI);
    let mono: Font = font(scale::FONT_MONO);
    let mut text = String::new();
    let mut runs = Vec::new();
    for span in spans {
        let (s, run) = match span {
            Span::Text(s) => (s, TextRun { len: s.len(), font: ui.clone(), color: style.ink, background_color: None, underline: None, strikethrough: None }),
            Span::Code(s) => (s, TextRun { len: s.len(), font: mono.clone(), color: style.code_ink, background_color: Some(style.code_bg), underline: None, strikethrough: None }),
            Span::Bold(s) => {
                let mut f = ui.clone();
                f.weight = FontWeight::SEMIBOLD;
                (s, TextRun { len: s.len(), font: f, color: style.ink, background_color: None, underline: None, strikethrough: None })
            }
            Span::Italic(s) => {
                let mut f = ui.clone();
                f.style = FontStyle::Italic;
                (s, TextRun { len: s.len(), font: f, color: style.ink, background_color: None, underline: None, strikethrough: None })
            }
            Span::Strikethrough(s) => (s, TextRun { len: s.len(), font: ui.clone(), color: style.ink, background_color: None, underline: None, strikethrough: Some(StrikethroughStyle { thickness: px(1.0), color: Some(style.ink) }) }),
            // Unreachable from this module's parser, which never emits
            // links: the body ink stands in for the accent the markdown
            // renderer paints, and shaping is identical either way.
            Span::Link { label, .. } => (label, TextRun { len: label.len(), font: ui.clone(), color: style.ink, background_color: None, underline: Some(UnderlineStyle { thickness: px(1.0), color: Some(style.ink), wavy: false }), strikethrough: None }),
        };
        text.push_str(s);
        runs.push(run);
    }
    (text, runs)
}

/// The text and text runs of the paragraph that closes `markdown`, built
/// exactly as [`prose`] builds them, so a caller that has to measure where the
/// prose ends (the streaming caret) shapes the same glyphs that are painted.
/// `None` when a list closes the prose: the caret does not follow a bullet.
pub fn last_paragraph_runs(markdown: &str, style: &ProseStyle) -> Option<(String, Vec<TextRun>)> {
    // Turns paint markdown blocks now, so the caret measures those; the
    // body ink stands in for the link accent, which does not change shaping.
    // Kept under this name for callers that measured prose paragraphs.
    last_block_runs(markdown, style, style.ink)
}

/// Renders `markdown` as prose blocks.
pub fn prose(id: impl Into<ElementId>, markdown: &str, style: ProseStyle) -> impl IntoElement {
    let id: ElementId = id.into();
    let blocks = parse(markdown);
    let count = blocks.len();
    let mut col = v_flex().w_full().ui(style.size).line_height(relative(style.line_height)).text_color(style.ink);
    for (i, block) in blocks.into_iter().enumerate() {
        let last = i + 1 == count;
        match block {
            Block::Paragraph(spans) => {
                let (text, runs) = runs(&spans, &style);
                col = col.child(div().w_full().when(!last, |d| d.mb(px(style.paragraph_gap))).child(StyledText::new(text).with_runs(runs)));
            }
            Block::List(items) => {
                let mut list = v_flex().w_full().when(!last, |d| d.mb(px(LIST_MARGIN)));
                for item in items {
                    let (text, runs) = runs(&item, &style);
                    list = list.child(
                        h_flex()
                            .w_full()
                            .items_start()
                            .child(div().flex_none().w(px(LIST_INDENT)).flex().justify_center().child("•"))
                            .child(div().flex_1().min_w(px(0.0)).child(StyledText::new(text).with_runs(runs))),
                    );
                }
                col = col.child(list);
            }
        }
    }
    col.id(id)
}

/// Nothing to render (an empty turn) — kept for callers that need a stable type.
#[allow(dead_code)]
fn _unused(_: &mut Window, _: &mut App) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paragraphs_lists_and_inline_marks() {
        let blocks = parse("Hello `code` and **bold**.\n\n- one\n- two `x`\n\nEnd");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Paragraph(s) if s.len() == 5));
        assert!(matches!(&blocks[1], Block::List(items) if items.len() == 2));
        assert_eq!(blocks[2], Block::Paragraph(vec![Span::Text("End".into())]));
    }

    #[test]
    fn shared_string_is_not_needed_for_runs() {
        let style = ProseStyle { ink: gpui::black(), code_ink: gpui::black(), code_bg: gpui::white(), size: 13.5, line_height: 1.65, paragraph_gap: 10.0 };
        let (text, runs) = runs(&parse_inline("a `b` c"), &style);
        assert_eq!(text, "a b c");
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), text.len());
    }

    /// A style with distinguishable colours, for the run assertions.
    fn style() -> ProseStyle {
        ProseStyle { ink: gpui::black(), code_ink: gpui::red(), code_bg: gpui::white(), size: 13.5, line_height: 1.65, paragraph_gap: 10.0 }
    }

    #[test]
    fn a_bold_lead_splits_from_the_rest_of_the_line() {
        assert_eq!(parse_inline("**Lead** rest"), vec![Span::Bold("Lead".into()), Span::Text(" rest".into())]);
    }

    #[test]
    fn bold_in_the_middle_keeps_both_sides() {
        assert_eq!(parse_inline("a **b** c"), vec![Span::Text("a ".into()), Span::Bold("b".into()), Span::Text(" c".into())]);
    }

    #[test]
    fn a_single_star_is_literal_text() {
        assert_eq!(parse_inline("2 * 3 = 6"), vec![Span::Text("2 * 3 = 6".into())]);
    }

    #[test]
    fn inline_code_switches_to_the_mono_face_and_ground() {
        let style = style();
        let (text, r) = runs(&parse_inline("run `cargo test` now"), &style);
        assert_eq!(text, "run cargo test now");
        assert_eq!(r.len(), 3);
        assert_eq!(&*r[0].font.family, scale::FONT_UI);
        assert_eq!(&*r[1].font.family, scale::FONT_MONO);
        assert_eq!(r[1].color, style.code_ink);
        assert_eq!(r[1].background_color, Some(style.code_bg));
        assert_eq!(r[2].background_color, None);
    }

    #[test]
    fn bold_runs_are_semibold_in_the_ui_face() {
        let (_, r) = runs(&parse_inline("**Lead** rest"), &style());
        assert_eq!(r[0].font.weight, FontWeight::SEMIBOLD);
        assert_eq!(&*r[0].font.family, scale::FONT_UI);
        assert_eq!(r[1].font.weight, FontWeight::default());
    }

    #[test]
    fn bullets_lose_their_marker_and_parse_inline() {
        let blocks = parse("- **one** two\n-   three `x`");
        let Block::List(items) = &blocks[0] else { panic!("expected a list") };
        assert_eq!(items[0], vec![Span::Bold("one".into()), Span::Text(" two".into())]);
        assert_eq!(items[1], vec![Span::Text("three ".into()), Span::Code("x".into())]);
    }

    #[test]
    fn indented_bullets_are_still_a_list() {
        assert!(matches!(&parse("  - one\n  - two")[0], Block::List(items) if items.len() == 2));
    }

    #[test]
    fn a_chunk_that_mixes_bullets_and_prose_is_one_paragraph() {
        // Every line has to be a bullet; a lead-in line makes the chunk prose.
        assert_eq!(parse("Steps:\n- one")[0], Block::Paragraph(vec![Span::Text("Steps: - one".into())]));
    }

    #[test]
    fn soft_wrapped_lines_join_with_a_space() {
        assert_eq!(parse("one\ntwo\nthree")[0], Block::Paragraph(vec![Span::Text("one two three".into())]));
    }

    #[test]
    fn blank_lines_separate_blocks_and_extra_ones_are_dropped() {
        let blocks = parse("one\n\n\n\ntwo");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1], Block::Paragraph(vec![Span::Text("two".into())]));
    }

    #[test]
    fn an_unterminated_mark_runs_to_the_end_of_the_line() {
        assert_eq!(parse_inline("a `b"), vec![Span::Text("a ".into()), Span::Code("b".into())]);
        assert_eq!(parse_inline("a **b"), vec![Span::Text("a ".into()), Span::Bold("b".into())]);
    }

    #[test]
    fn empty_and_blank_input_have_no_blocks() {
        assert!(parse("").is_empty());
        assert!(parse("\n\n   \n\n\t").is_empty());
        assert!(parse_inline("").is_empty());
    }

    #[test]
    fn run_lengths_sum_to_the_text_length() {
        let style = style();
        for src in ["", "plain", "a `b` c", "**b**", "``", "****", "a ` b ** c", "π `é` — ok", "`code`**bold**"] {
            let (text, r) = runs(&parse_inline(src), &style);
            assert_eq!(r.iter().map(|run| run.len).sum::<usize>(), text.len(), "runs do not cover `{src}`");
        }
    }

    #[test]
    fn last_paragraph_runs_ignores_a_closing_list() {
        let style = style();
        assert!(last_paragraph_runs("intro\n\n- one\n- two", &style).is_none());
        assert!(last_paragraph_runs("", &style).is_none());
        let (text, _) = last_paragraph_runs("- one\n\nDone `now`", &style).expect("a paragraph closes the prose");
        assert_eq!(text, "Done now");
    }

    #[test]
    fn caret_top_centres_then_drops_by_the_baseline_offset() {
        let top = caret_top_in_line(px(24.0), px(CARET_H), 1.0);
        assert_eq!(top, px((24.0 - CARET_H) / 2.0 + CARET_BASELINE_DROP));
        // The drop scales with the text, the centring with the line box.
        assert_eq!(caret_top_in_line(px(24.0), px(CARET_H), 2.0), top + px(CARET_BASELINE_DROP));
    }
}
