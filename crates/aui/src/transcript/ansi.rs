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
            // A parameter that is not a number this parser knows is dropped, as
            // are the ones it knows nothing about; only an explicit `0` resets.
            for code in params.split(';').filter_map(|s| s.parse::<u8>().ok()) {
                match code {
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

    /// Everything the spans would paint, in order.
    fn joined(spans: &[AnsiSpan]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// The state the last span carries.
    fn last(line: &str) -> AnsiSpan {
        parse_ansi(line).pop().expect("parse_ansi always yields a span")
    }

    #[test]
    fn standard_colours_map_to_indices_zero_to_seven() {
        for n in 30u8..=37 {
            assert_eq!(last(&format!("\u{1b}[{n}mx")).color, Some(n - 30), "SGR {n}");
        }
    }

    #[test]
    fn bright_colours_map_to_indices_eight_to_fifteen() {
        for n in 90u8..=97 {
            assert_eq!(last(&format!("\u{1b}[{n}mx")).color, Some(n - 90 + 8), "SGR {n}");
        }
    }

    #[test]
    fn bold_and_dim_are_independent_flags() {
        let s = last("\u{1b}[1m\u{1b}[2mx");
        assert!(s.bold && s.dim);
    }

    #[test]
    fn twenty_two_clears_the_weight_but_keeps_the_colour() {
        let s = last("\u{1b}[1;2;31ma\u{1b}[22mb");
        assert!(!s.bold && !s.dim);
        assert_eq!(s.color, Some(1));
    }

    #[test]
    fn thirty_nine_clears_the_colour_but_keeps_the_weight() {
        let s = last("\u{1b}[1;31ma\u{1b}[39mb");
        assert_eq!(s.color, None);
        assert!(s.bold);
    }

    #[test]
    fn reset_clears_everything() {
        let s = last("\u{1b}[1;2;31ma\u{1b}[0mb");
        assert_eq!(s, AnsiSpan { text: "b".into(), color: None, bold: false, dim: false });
    }

    #[test]
    fn several_params_in_one_escape_all_apply() {
        let s = last("\u{1b}[1;32mx");
        assert_eq!(s.color, Some(2));
        assert!(s.bold);
    }

    #[test]
    fn a_later_colour_overrides_the_earlier_one() {
        let spans = parse_ansi("\u{1b}[31ma\u{1b}[34mb");
        assert_eq!(spans.iter().map(|s| s.color).collect::<Vec<_>>(), vec![Some(1), Some(4)]);
    }

    #[test]
    fn escapes_with_no_text_between_them_make_no_empty_spans() {
        let spans = parse_ansi("\u{1b}[31m\u{1b}[1mx");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0], AnsiSpan { text: "x".into(), color: Some(1), bold: true, dim: false });
    }

    #[test]
    fn empty_input_is_one_empty_unstyled_span() {
        let spans = parse_ansi("");
        assert_eq!(spans, vec![AnsiSpan { text: String::new(), color: None, bold: false, dim: false }]);
    }

    #[test]
    fn unknown_and_non_numeric_params_leave_the_state_alone() {
        // `.parse::<u8>()` fails on `xyz` and on `256`; neither may act as a reset.
        for garbage in ["\u{1b}[xyzm", "\u{1b}[256m", "\u{1b}[38;5;196m"] {
            let s = last(&format!("\u{1b}[1;31ma{garbage}b"));
            assert_eq!((s.color, s.bold), (Some(1), true), "`{garbage}` disturbed the state");
        }
    }

    #[test]
    fn malformed_sequences_are_dropped_without_panicking() {
        assert_eq!(joined(&parse_ansi("a\u{1b}[")), "a");
        assert_eq!(joined(&parse_ansi("a\u{1b}[32")), "a");
        assert_eq!(joined(&parse_ansi("a\u{1b}[;;mb")), "ab");
        // A bare ESC is not an SGR introducer, so it stays in the text.
        assert_eq!(joined(&parse_ansi("a\u{1b}b")), "a\u{1b}b");
    }

    #[test]
    fn span_text_concatenates_back_to_the_line_without_its_escapes() {
        for (line, bare) in [
            ("\u{1b}[32m✓\u{1b}[0m ok \u{1b}[2m(18)\u{1b}[0m", "✓ ok (18)"),
            ("plain — π", "plain — π"),
            ("\u{1b}[1;90m›\u{1b}[22m tail", "› tail"),
            ("", ""),
        ] {
            assert_eq!(joined(&parse_ansi(line)), bare);
        }
    }

    #[test]
    fn run_lengths_match_the_span_texts() {
        let spans = parse_ansi("\u{1b}[31mé\u{1b}[0m ok");
        let (text, runs) = ansi_runs(&spans, &aui_tokens::dark(), aui_tokens::scale::FONT_MONO);
        assert_eq!(text, "é ok");
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), text.len());
    }
}
