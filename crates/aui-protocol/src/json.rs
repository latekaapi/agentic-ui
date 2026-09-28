//! Whole-message JSON: detecting a transcript message that is entirely a JSON
//! document, so the transcript can render it as a `json` code block instead
//! of a linkified paragraph.
//!
//! The check lives here (rather than in `aui`) so the transcript has one
//! settled-aware entry point next to the session model, and it uses only
//! `std` — no JSON dependency — because the crate manifests are frozen to
//! the task's file allow-list.

/// Maximum nesting depth: deeper documents read as prose rather than
/// risking stack overflow on adversarial input.
const MAX_DEPTH: usize = 128;

/// Pretty-prints `source` when the whole (trimmed) text is a JSON object or
/// array: `Some` with the 2-space-indented document, `None` otherwise.
///
/// Bare numbers, strings, booleans and `null` never count — a turn that is
/// just `42` stays prose. Invalid or partial JSON (still streaming) is also
/// `None`, so a streaming turn never flips between paragraph and code while
/// chunks arrive; callers apply this only once the turn is settled.
///
/// The printer preserves number literals and string escapes byte-for-byte;
/// only structural whitespace is rewritten.
pub fn pretty_json_document(source: &str) -> Option<String> {
    let trimmed = source.trim();
    let first = trimmed.as_bytes().first()?;
    if *first != b'{' && *first != b'[' {
        return None;
    }
    let mut parser = Parser { bytes: trimmed.as_bytes(), pos: 0, out: String::new(), indent: 0 };
    parser.value(0)?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return None;
    }
    Some(parser.out)
}

/// A strict JSON scanner that pretty-prints as it validates.
struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    out: String,
    indent: usize,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn newline_indent(&mut self) {
        self.out.push('\n');
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
    }

    fn value(&mut self, depth: usize) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.skip_ws();
        match self.peek()? {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => self.string(),
            b't' => self.literal("true"),
            b'f' => self.literal("false"),
            b'n' => self.literal("null"),
            c if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => None,
        }
    }

    fn object(&mut self, depth: usize) -> Option<()> {
        self.pos += 1; // `{`
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            self.out.push_str("{}");
            return Some(());
        }
        self.out.push('{');
        self.indent += 1;
        loop {
            self.newline_indent();
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return None;
            }
            self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return None;
            }
            self.pos += 1;
            self.out.push_str(": ");
            self.value(depth + 1)?;
            self.skip_ws();
            match self.peek()? {
                b',' => {
                    self.pos += 1;
                    self.out.push(',');
                }
                b'}' => {
                    self.pos += 1;
                    self.indent -= 1;
                    self.newline_indent();
                    self.out.push('}');
                    return Some(());
                }
                _ => return None,
            }
        }
    }

    fn array(&mut self, depth: usize) -> Option<()> {
        self.pos += 1; // `[`
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            self.out.push_str("[]");
            return Some(());
        }
        self.out.push('[');
        self.indent += 1;
        loop {
            self.newline_indent();
            self.value(depth + 1)?;
            self.skip_ws();
            match self.peek()? {
                b',' => {
                    self.pos += 1;
                    self.out.push(',');
                }
                b']' => {
                    self.pos += 1;
                    self.indent -= 1;
                    self.newline_indent();
                    self.out.push(']');
                    return Some(());
                }
                _ => return None,
            }
        }
    }

    /// Copies one string verbatim (escapes preserved byte-for-byte),
    /// rejecting unterminated strings and bad escapes.
    fn string(&mut self) -> Option<()> {
        let start = self.pos;
        self.pos += 1; // `"`
        loop {
            let c = *self.bytes.get(self.pos)?;
            match c {
                b'"' => {
                    self.pos += 1;
                    self.out.push_str(str::from_utf8(&self.bytes[start..self.pos]).ok()?);
                    return Some(());
                }
                b'\\' => {
                    self.pos += 1;
                    match *self.bytes.get(self.pos)? {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                            self.pos += 1;
                        }
                        b'u' => {
                            for _ in 0..4 {
                                self.pos += 1;
                                if !self.bytes.get(self.pos)?.is_ascii_hexdigit() {
                                    return None;
                                }
                            }
                            self.pos += 1;
                        }
                        _ => return None,
                    }
                }
                0x00..=0x1F => return None,
                _ => {
                    self.pos += 1;
                }
            }
        }
    }

    fn literal(&mut self, word: &str) -> Option<()> {
        let end = self.pos + word.len();
        if self.bytes.get(self.pos..end) == Some(word.as_bytes()) {
            self.pos = end;
            self.out.push_str(word);
            Some(())
        } else {
            None
        }
    }

    /// Copies one number verbatim, validating the JSON number grammar.
    fn number(&mut self) -> Option<()> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek()? {
            b'0' => {
                self.pos += 1;
            }
            c if c.is_ascii_digit() => {
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            _ => return None,
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return None;
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return None;
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        self.out.push_str(str::from_utf8(&self.bytes[start..self.pos]).ok()?);
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_pretty_prints_with_two_spaces() {
        let out = pretty_json_document(r#"{"b":1,"a":[1,2]}"#).expect("an object");
        assert_eq!(out, "{\n  \"b\": 1,\n  \"a\": [\n    1,\n    2\n  ]\n}");
    }

    #[test]
    fn empty_containers_stay_compact() {
        assert_eq!(pretty_json_document("{}").as_deref(), Some("{}"));
        assert_eq!(pretty_json_document("[]").as_deref(), Some("[]"));
        assert_eq!(
            pretty_json_document("  [1, 2]\n").as_deref(),
            Some("[\n  1,\n  2\n]")
        );
    }

    #[test]
    fn rejects_non_documents_and_partial_json() {
        assert_eq!(pretty_json_document("42"), None, "bare number");
        assert_eq!(pretty_json_document("\"hi\""), None, "bare string");
        assert_eq!(pretty_json_document("null"), None, "null");
        assert_eq!(pretty_json_document("true"), None, "bool");
        assert_eq!(pretty_json_document(r#"{"a":1"#), None, "partial object");
        assert_eq!(pretty_json_document(r#"{"a":}"#), None, "missing value");
        assert_eq!(pretty_json_document(r#"{"a":1} trailing"#), None, "trailing text");
        assert_eq!(pretty_json_document(r#"{"a":01}"#), None, "bad number");
        assert_eq!(pretty_json_document("just prose"), None, "prose");
        assert_eq!(pretty_json_document("  \n "), None, "blank");
    }
}
