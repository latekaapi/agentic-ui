//! A small lexer for the syntax colours the design uses in code blocks:
//! keyword (magenta), function call (blue), string (green), number (yellow),
//! comment (dim, italic). It knows JavaScript / TypeScript shapes, which is
//! what the sample code is; other languages get keywords and literals only.
//!
//! With the crate's optional `tree-sitter` feature the same six classes are
//! produced by gpui-kit's `SyntaxHighlighter` for the languages listed in
//! [`ts_language`]; every other language, and the whole crate without the
//! feature, falls back to the lexer. The feature is off by default.

use aui_tokens::Palette;
use gpui::{font, FontStyle, Hsla, TextRun};
use std::ops::Range;

/// A token class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Plain code.
    Plain,
    /// A reserved word.
    Keyword,
    /// An identifier followed by `(`.
    Function,
    /// A quoted string.
    String,
    /// A numeric literal.
    Number,
    /// A line or block comment.
    Comment,
}

const KEYWORDS: &[&str] = &[
    "export", "function", "if", "else", "return", "const", "let", "var", "await", "async", "class", "import", "from", "type", "interface", "new",
    "true", "false", "null", "undefined", "for", "while", "switch", "case", "break", "continue", "throw", "try", "catch", "finally", "default", "as",
    "in", "of", "typeof", "instanceof", "this", "extends", "implements", "public", "private", "static", "readonly", "enum", "void", "yield",
    "fn", "pub", "impl", "struct", "match", "use", "mod", "mut", "self", "def", "None", "True", "False", "elif", "with", "lambda", "pass",
];

/// Tokenises one line of `code` into `(byte range, kind)` pairs covering it fully.
pub fn tokenize_line(line: &str) -> Vec<(Range<usize>, TokenKind)> {
    let bytes = line.as_bytes();
    let mut out: Vec<(Range<usize>, TokenKind)> = Vec::new();
    let mut i = 0;
    let push = |out: &mut Vec<(Range<usize>, TokenKind)>, range: Range<usize>, kind: TokenKind| {
        if range.is_empty() {
            return;
        }
        if let Some(last) = out.last_mut() {
            if last.1 == kind && last.0.end == range.start {
                last.0.end = range.end;
                return;
            }
        }
        out.push((range, kind));
    };
    while i < bytes.len() {
        let c = bytes[i];
        // comments
        if line[i..].starts_with("//") || line[i..].starts_with('#') && !line[i..].starts_with("#[") {
            push(&mut out, i..bytes.len(), TokenKind::Comment);
            break;
        }
        if line[i..].starts_with("/*") {
            let end = line[i..].find("*/").map(|e| i + e + 2).unwrap_or(bytes.len());
            push(&mut out, i..end, TokenKind::Comment);
            i = end;
            continue;
        }
        // strings
        if c == b'\'' || c == b'"' || c == b'`' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != c {
                if bytes[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(bytes.len());
            push(&mut out, i..end, TokenKind::String);
            i = end;
            continue;
        }
        // numbers
        if c.is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'.' || bytes[j] == b'_') {
                j += 1;
            }
            push(&mut out, i..j, TokenKind::Number);
            i = j;
            continue;
        }
        // identifiers / keywords / calls
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'$') {
                j += 1;
            }
            let word = &line[i..j];
            let kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if bytes.get(j) == Some(&b'(') {
                TokenKind::Function
            } else {
                TokenKind::Plain
            };
            push(&mut out, i..j, kind);
            i = j;
            continue;
        }
        // everything else, one byte (or one UTF-8 sequence) at a time
        let len = line[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        push(&mut out, i..i + len, TokenKind::Plain);
        i += len;
    }
    out
}

/// The grammar name gpui-kit's highlighter knows for `language`, for the six
/// languages the `tree-sitter` feature ships grammars for. `None` — including
/// for every language when the feature is off — means the lexer handles it.
pub fn ts_language(language: &str) -> Option<&'static str> {
    if cfg!(not(feature = "tree-sitter")) {
        return None;
    }
    Some(match language.trim().to_ascii_lowercase().as_str() {
        "rust" | "rs" => "rust",
        "ts" | "typescript" => "typescript",
        "tsx" => "tsx",
        "json" | "jsonc" => "json",
        "py" | "python" => "python",
        "bash" | "sh" | "shell" | "zsh" => "bash",
        _ => return None,
    })
}

/// [`tokenize_line`] for a line whose `language` is known.
///
/// With the `tree-sitter` feature on and a grammar for `language`, the classes
/// come from the grammar's highlight queries; otherwise the small lexer runs.
/// Either way the ranges are contiguous, non-empty and cover the whole line.
pub fn tokenize_line_in(line: &str, language: Option<&str>) -> Vec<(Range<usize>, TokenKind)> {
    #[cfg(feature = "tree-sitter")]
    if let Some(grammar) = language.and_then(ts_language) {
        if let Some(tokens) = ts::tokenize(line, grammar) {
            return tokens;
        }
    }
    let _ = language;
    tokenize_line(line)
}

/// The tree-sitter path. One highlighter per line: the design's code blocks
/// are a few dozen lines, and a line is the unit the transcript shapes, so
/// there is no tree to keep across calls (constructs that span lines — a
/// multi-line string, an open block comment — are therefore classified per
/// line, exactly as the lexer does).
#[cfg(feature = "tree-sitter")]
mod ts {
    use super::TokenKind;
    use gpui::{Hsla, HighlightStyle};
    use gpui_kit::base::input::{HighlightStyleResolver, Rope};
    use gpui_kit::component::highlighter::SyntaxHighlighter;
    use std::ops::Range;

    /// The six classes, in the order the sentinel hue encodes them.
    const KINDS: &[TokenKind] =
        &[TokenKind::Plain, TokenKind::Keyword, TokenKind::Function, TokenKind::String, TokenKind::Number, TokenKind::Comment];

    /// gpui-component hands capture names to a `HighlightStyleResolver` and
    /// gives back the `HighlightStyle`s it returns, so the only way to read the
    /// capture names out is to answer with a style we can recognise. The
    /// resolver answers with a sentinel hue — `index / 8` — that [`decode`]
    /// reads back; no colour from here is ever painted.
    struct Kinds;

    fn encode(kind: TokenKind) -> HighlightStyle {
        let index = KINDS.iter().position(|k| *k == kind).unwrap_or(0);
        HighlightStyle { color: Some(Hsla { h: index as f32 / 8.0, s: 1.0, l: 0.5, a: 1.0 }), ..Default::default() }
    }

    fn decode(style: &HighlightStyle) -> TokenKind {
        match style.color {
            Some(c) => KINDS.get((c.h * 8.0).round() as usize).copied().unwrap_or(TokenKind::Plain),
            None => TokenKind::Plain,
        }
    }

    impl HighlightStyleResolver for Kinds {
        fn style(&self, name: &str) -> Option<HighlightStyle> {
            // Capture names are dotted (`function.method`, `string.special`);
            // the first segment is the class the design colours.
            let kind = match name.split('.').next().unwrap_or(name) {
                "keyword" | "operator" | "boolean" | "constant" | "type" | "attribute" => TokenKind::Keyword,
                "function" | "method" => TokenKind::Function,
                "string" | "character" => TokenKind::String,
                "number" | "float" => TokenKind::Number,
                "comment" => TokenKind::Comment,
                _ => TokenKind::Plain,
            };
            Some(encode(kind))
        }
    }

    /// Classifies one line, or `None` when the grammar is not registered.
    pub fn tokenize(line: &str, grammar: &str) -> Option<Vec<(Range<usize>, TokenKind)>> {
        let rope = Rope::from(line);
        let mut highlighter = SyntaxHighlighter::new(grammar);
        highlighter.update(None, &rope, None);
        highlighter.tree()?;

        // `styles` may leave gaps and may hand back ranges out of order; the
        // rest of the library relies on a contiguous, merged cover.
        let mut styled: Vec<(Range<usize>, TokenKind)> =
            highlighter.styles(&(0..line.len()), &Kinds).into_iter().map(|(r, s)| (r, decode(&s))).collect();
        styled.sort_by_key(|(r, _)| r.start);

        let mut out: Vec<(Range<usize>, TokenKind)> = Vec::new();
        let mut push = |out: &mut Vec<(Range<usize>, TokenKind)>, range: Range<usize>, kind: TokenKind| {
            if range.is_empty() {
                return;
            }
            if let Some(last) = out.last_mut() {
                if last.1 == kind && last.0.end == range.start {
                    last.0.end = range.end;
                    return;
                }
            }
            out.push((range, kind));
        };
        let mut at = 0;
        for (range, kind) in styled {
            if range.start > at {
                push(&mut out, at..range.start, TokenKind::Plain);
            }
            let start = range.start.max(at);
            if range.end > start {
                push(&mut out, start..range.end, kind);
                at = range.end;
            }
        }
        if at < line.len() {
            push(&mut out, at..line.len(), TokenKind::Plain);
        }
        Some(out)
    }
}

/// The colour for a token class.
pub fn token_color(kind: TokenKind, p: &Palette) -> Hsla {
    match kind {
        TokenKind::Plain => p.term_fg,
        TokenKind::Keyword => p.ansi_magenta,
        TokenKind::Function => p.ansi_blue,
        TokenKind::String => p.ansi_green,
        TokenKind::Number => p.ansi_yellow,
        TokenKind::Comment => p.term_dim,
    }
}

/// Text runs for one line in the mono face, classified by the lexer.
pub fn syntax_runs(line: &str, p: &Palette, mono_family: &'static str) -> Vec<TextRun> {
    syntax_runs_in(line, None, p, mono_family)
}

/// [`syntax_runs`] for a line whose `language` is known; see
/// [`tokenize_line_in`].
pub fn syntax_runs_in(line: &str, language: Option<&str>, p: &Palette, mono_family: &'static str) -> Vec<TextRun> {
    tokenize_line_in(line, language)
        .into_iter()
        .map(|(range, kind)| {
            let mut f = font(mono_family);
            if kind == TokenKind::Comment {
                f.style = FontStyle::Italic;
            }
            TextRun { len: range.len(), font: f, color: token_color(kind, p), background_color: None, underline: None, strikethrough: None }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_sample_line() {
        let toks = tokenize_line("  if (values.country === 'CA') return validateCanadianPostal(values);");
        let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
        assert!(kinds.contains(&TokenKind::Keyword));
        assert!(kinds.contains(&TokenKind::String));
        assert!(kinds.contains(&TokenKind::Function));
        let total: usize = toks.iter().map(|t| t.0.len()).sum();
        assert_eq!(total, "  if (values.country === 'CA') return validateCanadianPostal(values);".len());
    }

    #[test]
    fn comments_take_the_rest_of_the_line() {
        let toks = tokenize_line("  // US ZIP or ZIP+4");
        assert_eq!(toks.last().unwrap().1, TokenKind::Comment);
    }

    /// The kind of the token that starts where `needle` does.
    fn kind_of(line: &str, needle: &str) -> TokenKind {
        let at = line.find(needle).expect("needle is in the line");
        tokenize_line(line).into_iter().find(|(r, _)| r.start == at).unwrap_or_else(|| panic!("no token starts at `{needle}`")).1
    }

    /// Every token is non-empty, the tokens run contiguously from 0 to the end
    /// of the line, and every boundary is a char boundary.
    fn assert_covers(line: &str) {
        let mut at = 0;
        for (range, _) in tokenize_line(line) {
            assert!(!range.is_empty(), "empty token in `{line}`");
            assert_eq!(range.start, at, "gap or overlap in `{line}`");
            assert!(line.is_char_boundary(range.start) && line.is_char_boundary(range.end), "token splits a char in `{line}`");
            at = range.end;
        }
        assert_eq!(at, line.len(), "`{line}` is not covered to its end");
    }

    #[test]
    fn keywords_from_every_language_the_lexer_knows() {
        for (line, word) in [("const x = 1", "const"), ("pub fn main() {}", "fn"), ("def main():", "def"), ("match x {", "match")] {
            assert_eq!(kind_of(line, word), TokenKind::Keyword, "`{word}`");
        }
        // A keyword only counts whole: `constant` is plain.
        assert_eq!(kind_of("constant = 1", "constant"), TokenKind::Plain);
    }

    #[test]
    fn an_identifier_before_a_paren_is_a_call() {
        assert_eq!(kind_of("render(x)", "render"), TokenKind::Function);
        assert_eq!(kind_of("render (x)", "render"), TokenKind::Plain);
        assert_eq!(kind_of("if (x)", "if"), TokenKind::Keyword);
    }

    #[test]
    fn dollar_and_underscore_start_identifiers() {
        assert_eq!(kind_of("$el.focus()", "$el"), TokenKind::Plain);
        assert_eq!(kind_of("_private()", "_private"), TokenKind::Function);
    }

    #[test]
    fn strings_end_at_their_own_quote_and_survive_escapes() {
        let line = r#"a = "x\"y" + 'z' + `t`"#;
        let quoted: Vec<&str> = tokenize_line(line).iter().filter(|t| t.1 == TokenKind::String).map(|t| &line[t.0.clone()]).collect();
        assert_eq!(quoted, vec![r#""x\"y""#, "'z'", "`t`"]);
    }

    #[test]
    fn an_unterminated_string_runs_to_the_end_of_the_line() {
        assert_eq!(tokenize_line("say('hello").last().unwrap().1, TokenKind::String);
        assert_covers("say('hello");
        // A trailing backslash must not run off the end.
        assert_covers("say(\"hello\\");
    }

    #[test]
    fn numbers_cover_decimals_hex_and_separators() {
        for (line, lit) in [("x = 1.5", "1.5"), ("x = 0x1F", "0x1F"), ("x = 1_000", "1_000"), ("x = 42", "42")] {
            assert_eq!(kind_of(line, lit), TokenKind::Number, "`{lit}`");
        }
        // A digit only opens a number at a token boundary: `a1` is one identifier.
        assert_eq!(kind_of("a1 = 2", "a1"), TokenKind::Plain);
    }

    #[test]
    fn hash_starts_a_comment_but_an_attribute_does_not() {
        assert_eq!(kind_of("# python comment", "#"), TokenKind::Comment);
        assert_ne!(kind_of("#[derive(Debug)]", "#["), TokenKind::Comment);
        assert_eq!(kind_of("#[derive(Debug)]", "derive"), TokenKind::Function);
    }

    #[test]
    fn block_comments_stop_at_their_terminator_or_at_the_line_end() {
        let line = "let a = /* note */ 1;";
        let toks = tokenize_line(line);
        let comment = toks.iter().find(|t| t.1 == TokenKind::Comment).expect("a block comment");
        assert_eq!(&line[comment.0.clone()], "/* note */");
        assert!(toks.iter().any(|t| t.1 == TokenKind::Number), "code after the comment is still lexed");

        let open = "let a = /* unterminated";
        let last = tokenize_line(open).pop().expect("a token");
        assert_eq!(last, (open.find("/*").unwrap()..open.len(), TokenKind::Comment));
    }

    #[test]
    fn adjacent_tokens_of_one_kind_merge() {
        // `x`, the spaces and `=` are all plain and collapse into one run.
        assert_eq!(tokenize_line("x = y"), vec![(0..5, TokenKind::Plain)]);
    }

    #[test]
    fn ranges_cover_every_line_including_non_ascii() {
        for line in [
            "",
            "   ",
            "const π = 3.14159 // τ/2",
            "label = 'héllo wörld'",
            "// ✓ done — 18 ms",
            "/* é */ ok()",
            "\"日本語\".length",
            "emoji = \"🙂\" + '🙃'",
            "  if (values.country === 'CA') return validateCanadianPostal(values);",
            "#[derive(Debug, Clone)] // é",
            "x = `template ${a}`",
        ] {
            assert_covers(line);
        }
    }

    #[test]
    fn an_empty_line_has_no_tokens() {
        assert!(tokenize_line("").is_empty());
    }

    #[test]
    fn syntax_runs_lengths_match_the_line() {
        for line in ["const x = 'é'; // ok", "", "🙂"] {
            let runs = syntax_runs(line, &aui_tokens::dark(), aui_tokens::scale::FONT_MONO);
            assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), line.len(), "`{line}`");
        }
        let runs = syntax_runs("// note", &aui_tokens::light(), aui_tokens::scale::FONT_MONO);
        assert_eq!(runs[0].font.style, FontStyle::Italic);
    }
}
