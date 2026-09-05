//! A small lexer for the syntax colours the design uses in code blocks:
//! keyword (magenta), function call (blue), string (green), number (yellow),
//! comment (dim, italic). It knows JavaScript / TypeScript shapes, which is
//! what the sample code is; other languages get keywords and literals only.

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

/// Text runs for one line in the mono face.
pub fn syntax_runs(line: &str, p: &Palette, mono_family: &'static str) -> Vec<TextRun> {
    tokenize_line(line)
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
}
