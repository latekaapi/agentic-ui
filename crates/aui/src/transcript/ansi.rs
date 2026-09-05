//! A small ANSI SGR parser for tool output lines: colours 30–37 / 90–97,
//! bold, dim and reset. Everything else is dropped.

use aui_tokens::Palette;
use gpui::{font, FontWeight, Hsla, TextRun};

/// One coloured run of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsiSpan {
    /// The text.
    pub text: String,
    /// ANSI colour index 0–15, if set.
    pub color: Option<u8>,
    /// Bold (SGR 1).
    pub bold: bool,
    /// Dim (SGR 2).
    pub dim: bool,
}

/// Splits `line` into spans at its SGR escapes.
pub fn parse_ansi(line: &str) -> Vec<AnsiSpan> {
    let mut spans = Vec::new();
    let mut current = AnsiSpan { text: String::new(), color: None, bold: false, dim: false };
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            let mut params = String::new();
            for d in chars.by_ref() {
                if d == 'm' {
                    break;
                }
                params.push(d);
            }
            if !current.text.is_empty() {
                spans.push(AnsiSpan { text: std::mem::take(&mut current.text), ..current.clone() });
            }
            for code in params.split(';').filter(|s| !s.is_empty()) {
                match code.parse::<u8>().unwrap_or(0) {
                    0 => {
                        current.color = None;
                        current.bold = false;
                        current.dim = false;
                    }
                    1 => current.bold = true,
                    2 => current.dim = true,
                    22 => {
                        current.bold = false;
                        current.dim = false;
                    }
                    39 => current.color = None,
                    n @ 30..=37 => current.color = Some(n - 30),
                    n @ 90..=97 => current.color = Some(n - 90 + 8),
                    _ => {}
                }
            }
        } else {
            current.text.push(c);
        }
    }
    if !current.text.is_empty() || spans.is_empty() {
        spans.push(current);
    }
    spans
}

/// Turns spans into text runs in the mono face on the terminal palette.
pub fn ansi_runs(spans: &[AnsiSpan], p: &Palette, mono_family: &'static str) -> (String, Vec<TextRun>) {
    let ansi = p.ansi16();
    let mut text = String::new();
    let mut runs = Vec::new();
    for span in spans {
        let color: Hsla = match (span.color, span.dim) {
            (Some(i), _) => ansi[i as usize % 16],
            (None, true) => p.term_dim,
            (None, false) => p.term_fg,
        };
        let mut f = font(mono_family);
        if span.bold {
            f.weight = FontWeight::SEMIBOLD;
        }
        text.push_str(&span.text);
        runs.push(TextRun { len: span.text.len(), font: f, color, background_color: None, underline: None, strikethrough: None });
    }
    (text, runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colours_and_reset() {
        let spans = parse_ansi("\u{1b}[32m✓\u{1b}[0m ok \u{1b}[2m(18)\u{1b}[0m");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].color, Some(2));
        assert_eq!(spans[1].text, " ok ");
        assert!(spans[2].dim);
    }

    #[test]
    fn plain_line_is_one_span() {
        let spans = parse_ansi("hello");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello");
    }
}
