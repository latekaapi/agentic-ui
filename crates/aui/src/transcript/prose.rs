//! Prose: the small markdown subset assistant and user turns use —
//! paragraphs, bullet lists, inline code and bold — rendered as styled text
//! runs so inline code can switch to the mono face (gpui-kit's markdown view
//! keeps one font per paragraph).

use aui_tokens::{scale, AuiStyled};
use gpui::{div, font, prelude::*, px, relative, App, ElementId, Font, FontWeight, Hsla, IntoElement, StyledText, TextRun, Window};
use gpui_kit::base::{h_flex, v_flex};

/// `.a ul{padding-left:18px}`.
const LIST_INDENT: f32 = 18.0;
/// `.a ul{margin:6px 0}` — the top margin collapses into the paragraph gap
/// above; the bottom one stands when a paragraph follows.
const LIST_MARGIN: f32 = 6.0;

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

/// One inline segment.
#[derive(Debug, Clone, PartialEq)]
enum Span {
    Text(String),
    Code(String),
    Bold(String),
}

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
        };
        text.push_str(s);
        runs.push(run);
    }
    (text, runs)
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
}
