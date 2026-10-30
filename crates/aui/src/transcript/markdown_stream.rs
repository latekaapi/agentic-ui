//! Incremental markdown for a growing answer: re-parsing a streamed turn
//! costs the open tail, not the whole answer.
//!
//! A streaming answer only ever grows at its end, and CommonMark parses
//! blocks line by line: once a top-level block has been followed by the
//! *complete first line* of another top-level block, nothing appended later
//! can reopen it. [`MarkdownStream`] keeps the blocks of that settled prefix
//! and, on each append, re-parses only from the start of the last top-level
//! block whose first line is complete (the "open tail").
//!
//! # The settled-boundary rule
//!
//! After an append, parse `source[tail_start..]` and list its top-level
//! items (a `Start` event at depth 0, or a top-level rule). Walking back from
//! the last item, the first item `k ≥ 1` that satisfies all of
//!
//! * its first line is complete — a line terminator (`\n` or `\r`) follows
//!   its start, so a half-typed line such as `#` (heading today, `#x`
//!   paragraph continuation tomorrow) or `***` cannot settle the block above
//!   it;
//! * it starts after the previous item has ended, measured from the start of
//!   its line (the boundary is always snapped back to a line start, so an
//!   indented list or code block keeps its indent in the tail);
//!
//! becomes the new boundary: items before it join the settled blocks and
//! `tail_start` moves to its line start. Everything that can still change —
//! a paragraph that may become a setext heading or a table header, a list or
//! indented code block that continues across a blank line, an unclosed fence
//! — is by construction the last item, so it is always in the tail.
//!
//! The one CommonMark feature that reaches across blocks is the link
//! reference definition (`[label]: dest`), which can turn an *earlier*
//! `[label]` into a link. A definition needs the two bytes `]:`, so the
//! stream stops being incremental the moment those bytes appear anywhere in
//! the source and re-parses the whole source on every append from then on
//! (exactly [`parse_markdown`]). That is conservative — `]:` inside a code
//! block also trips it — but it never gets the answer wrong. Footnotes, the
//! other cross-block feature, are off in the parser options.
//!
//! The invariant, checked by the tests at every char boundary of several
//! real documents: [`MarkdownStream::blocks`] equals
//! `parse_markdown(stream.source())` after every append.

use std::sync::Mutex;

use pulldown_cmark::{Event, Parser};

use super::markdown::{blocks_until, parse_markdown, parser_options, Block};

/// A markdown source that grows by appends, parsed incrementally. See the
/// module docs for the settled-boundary rule.
#[derive(Debug, Clone, Default)]
pub struct MarkdownStream {
    source: String,
    /// The blocks of `source[..tail_start]`, final.
    settled: Vec<Block>,
    /// Byte offset where the open tail begins; always a line start.
    tail_start: usize,
    /// The blocks of `source[tail_start..]`, rebuilt on every append.
    tail: Vec<Block>,
    /// Set once `]:` appears: a reference definition may reach back, so
    /// every append re-parses the whole source.
    whole: bool,
}

impl MarkdownStream {
    /// An empty stream.
    pub fn new() -> Self {
        Self::default()
    }

    /// A stream holding `source`, parsed once.
    pub fn from_source(source: &str) -> Self {
        let mut stream = Self::new();
        stream.push_str(source);
        stream
    }

    /// The whole source so far.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Byte length of the settled prefix — the part no append re-parses.
    pub fn settled_len(&self) -> usize {
        self.tail_start
    }

    /// Whether appends are still incremental (false once the source holds a
    /// possible link reference definition; see the module docs).
    pub fn is_incremental(&self) -> bool {
        !self.whole
    }

    /// Appends `chunk` and re-parses the open tail. A chunk may end anywhere
    /// — inside a `**` marker, a fence, a table row, between `\r` and `\n`.
    pub fn push_str(&mut self, chunk: &str) {
        let scan_from = self.source.len().saturating_sub(1);
        self.source.push_str(chunk);
        if !self.whole && self.source.as_bytes()[scan_from..].windows(2).any(|w| w == b"]:") {
            self.whole = true;
            self.settled.clear();
            self.tail_start = 0;
        }
        self.reparse_tail();
    }

    /// Moves the stream to `source`: an append when the current source is a
    /// prefix of it (returns `true`), otherwise a fresh full parse (`false`).
    /// This is the call for a host that holds the accumulated text rather
    /// than the chunks.
    pub fn set_source(&mut self, source: &str) -> bool {
        if source.len() >= self.source.len() && source.starts_with(self.source.as_str()) {
            if source.len() > self.source.len() {
                self.push_str(&source[self.source.len()..]);
            }
            true
        } else {
            *self = Self::from_source(source);
            false
        }
    }

    /// The settled blocks, which no later append changes.
    pub fn settled_blocks(&self) -> &[Block] {
        &self.settled
    }

    /// The open tail's blocks, rebuilt on every append.
    pub fn tail_blocks(&self) -> &[Block] {
        &self.tail
    }

    /// All blocks: exactly `parse_markdown(self.source())`.
    pub fn blocks(&self) -> Vec<Block> {
        let mut out = Vec::with_capacity(self.settled.len() + self.tail.len());
        out.extend_from_slice(&self.settled);
        out.extend_from_slice(&self.tail);
        out
    }

    /// All blocks for a message known to be `settled`: exactly
    /// [`parse_markdown_settled`](super::parse_markdown_settled)`(self.source(), settled)`
    /// — a settled source that is one JSON document is one `json` code block.
    pub fn blocks_settled(&self, settled: bool) -> Vec<Block> {
        if settled {
            if let Some(pretty) = aui_protocol::json::pretty_json_document(&self.source) {
                return vec![Block::CodeBlock { lang: Some("json".into()), text: pretty }];
            }
        }
        self.blocks()
    }

    fn reparse_tail(&mut self) {
        let tail = &self.source[self.tail_start..];
        let (events, ranges): (Vec<Event>, Vec<std::ops::Range<usize>>) =
            Parser::new_ext(tail, parser_options()).into_offset_iter().unzip();
        if self.whole {
            self.tail = blocks_until(&events, &mut 0, None, 0);
            return;
        }
        // Top-level items: (event index, start byte, end byte).
        let mut tops: Vec<(usize, usize, usize)> = Vec::new();
        let mut depth = 0usize;
        for (ix, event) in events.iter().enumerate() {
            match event {
                Event::Start(_) => {
                    if depth == 0 {
                        tops.push((ix, ranges[ix].start, ranges[ix].end));
                    }
                    depth += 1;
                }
                Event::End(_) => depth = depth.saturating_sub(1),
                Event::Rule if depth == 0 => tops.push((ix, ranges[ix].start, ranges[ix].end)),
                _ => {}
            }
        }
        let is_eol = |c: char| c == '\n' || c == '\r';
        let mut cut = None;
        for k in (1..tops.len()).rev() {
            let (ix, start, _) = tops[k];
            let line_start = tail[..start].rfind(is_eol).map_or(0, |p| p + 1);
            if line_start == 0 || line_start < tops[k - 1].2 || !tail[start..].contains(is_eol) {
                continue;
            }
            cut = Some((ix, line_start));
            break;
        }
        match cut {
            Some((ix, line_start)) => {
                self.settled.extend(blocks_until(&events[..ix], &mut 0, None, 0));
                self.tail = blocks_until(&events[ix..], &mut 0, None, 0);
                self.tail_start += line_start;
            }
            None => self.tail = blocks_until(&events, &mut 0, None, 0),
        }
    }
}

/// How many in-flight streams [`streamed_blocks`] keeps. One per answer that
/// is growing at once; a handful covers a visible transcript plus a
/// background session or two.
const STREAM_CAP: usize = 4;

/// The streams behind [`streamed_blocks`], most recently used last.
static STREAMS: Mutex<Vec<MarkdownStream>> = Mutex::new(Vec::new());

/// `parse_markdown(source)`, but incremental when `source` extends a source
/// seen recently: the memo miss for a streaming turn's next chunk lands here,
/// finds the stream holding the previous chunk's text, and re-parses only
/// its open tail. A source that extends no stream starts a new one (one
/// full parse, as before), evicting the least recently used.
pub(super) fn streamed_blocks(source: &str) -> Vec<Block> {
    let Ok(mut streams) = STREAMS.lock() else {
        return parse_markdown(source);
    };
    let best = streams
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.source.is_empty() && source.starts_with(s.source.as_str()))
        .max_by_key(|(_, s)| s.source.len())
        .map(|(ix, _)| ix);
    let mut stream = match best {
        Some(ix) => streams.remove(ix),
        None => {
            if streams.len() >= STREAM_CAP {
                streams.remove(0);
            }
            MarkdownStream::new()
        }
    };
    stream.set_source(source);
    let blocks = stream.blocks();
    streams.push(stream);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `doc` split at `cut` and checks the stream after each half.
    fn check_split(doc: &str, cut: usize) {
        let mut stream = MarkdownStream::new();
        stream.push_str(&doc[..cut]);
        assert_eq!(stream.blocks(), parse_markdown(&doc[..cut]), "prefix {cut} of {doc:?}");
        stream.push_str(&doc[cut..]);
        assert_eq!(stream.blocks(), parse_markdown(doc), "split at {cut} of {doc:?}");
    }

    /// Splits at every char boundary, and also streams the whole document
    /// in `step`-byte chunks checking every intermediate prefix. Returns the
    /// number of prefixes checked.
    fn check_every_split(doc: &str, step: usize) -> usize {
        let mut checked = 0;
        for cut in (0..=doc.len()).filter(|&c| doc.is_char_boundary(c)) {
            check_split(doc, cut);
            checked += 1;
        }
        let mut stream = MarkdownStream::new();
        let mut at = 0;
        while at < doc.len() {
            let mut next = (at + step).min(doc.len());
            while !doc.is_char_boundary(next) {
                next += 1;
            }
            stream.push_str(&doc[at..next]);
            assert_eq!(stream.blocks(), parse_markdown(&doc[..next]), "stream prefix {next}");
            checked += 1;
            at = next;
        }
        checked
    }

    const ANSWER: &str = "## Why the sidebar waited\n\n\
The list reply carried every row, so `merge_session_list` replaced the boot rows \
wholesale and the [label ladder](docs/51.md) disagreed with `rejoin`. A row whose \
title runs past **eighty characters** was cut on one path and *not* on the other — \
see crates/aui/src/lib.rs:12 and https://example.com/x.\n\n\
- the boot rows join the saved copy\n- the reply joins the wire rows\n\n- `rejoin` reads the index\n  and keeps going\n\n\
1. first\n2. second\n\n3. third after a blank\n\n\
| Path | Label | Cut |\n|:---|:-:|--:|\n| boot | index | no |\n| reply | **wire** | yes |\n\n\
```rust\nfn label(meta: Option<&Meta>) -> Option<&str> {\n\n    meta.and_then(|m| m.name.as_deref())\n}\n```\n\n\
Setext heading\n==============\n\nAnother one\n---\n\n\
> A quote with a list:\n> - one\n> - two\n\n\
    indented code\n\n    still code\n\n\
***\n\n\
Images ![alt text](img/a.png) inline, ~~struck~~ and a café ☕ naïve 日本語.\n\n\
~~~\nunclosed tilde fence at the end\n";

    #[test]
    fn every_split_of_a_real_answer_matches_the_full_parse() {
        let n = check_every_split(ANSWER, 7);
        assert!(n > ANSWER.chars().count());
    }

    #[test]
    fn fences_opened_and_closed_across_chunks() {
        let doc = "Intro\n\n```py\nprint('a')\n\n\nprint('b')\n```\nafter\n\n```\nx\n```\n\n````\n```\nnested\n````\nend\n";
        check_every_split(doc, 3);
    }

    #[test]
    fn lists_and_tables_continue_across_blank_lines_and_rows() {
        let doc = "- a\n- b\n\n- c\n\n  continued\n\npara\n\n| a | b |\n|---|---|\n| 1 | 2 |\nrow without pipes\n\n# done\n";
        check_every_split(doc, 2);
        let doc = "* x\n\n+ y\n\n1) one\n\n2) two\n10. ten\n";
        check_every_split(doc, 1);
    }

    #[test]
    fn setext_headings_and_half_typed_lines() {
        let doc = "Title\n=====\n\nfoo\n#x\nbar\n***y\n\nbaz\n---\n\nqux\n- item\n\nend\n#\n";
        check_every_split(doc, 1);
    }

    #[test]
    fn crlf_and_bare_cr_line_endings() {
        let doc = "# H\r\n\r\npara one\r\nstill one\r\n\r\n- a\r\n- b\r\n\r\n```\r\ncode\r\n```\r\n\r\nlast\rline\r\r\nend";
        check_every_split(doc, 2);
    }

    #[test]
    fn emphasis_markers_split_mid_marker() {
        let doc = "**bold** and __also__ and *it* and `co`de` and **un\n\nclosed** next\n\n***both*** done\n";
        check_every_split(doc, 1);
    }

    #[test]
    fn a_later_reference_definition_reaches_back() {
        let doc = "See [docs] and [other].\n\nMore text.\n\n- item [docs]\n\n[docs]: https://example.com\n\nAfter [docs].\n";
        check_every_split(doc, 4);
        let mut stream = MarkdownStream::new();
        stream.push_str("See [docs].\n\nMore.\n\n");
        assert!(stream.is_incremental());
        assert!(stream.settled_len() > 0);
        stream.push_str("[docs]");
        stream.push_str(": https://example.com\n");
        assert!(!stream.is_incremental());
        assert_eq!(stream.blocks(), parse_markdown(stream.source()));
    }

    #[test]
    fn html_blocks_and_nesting() {
        let doc = "<div>\nhidden\n</div>\n\ntext <b>bold</b>\n\n<!-- open comment\n\nstill\n-->\nafter\n\n> > deep\n> > quote\n\n> lazy\ncontinuation\n";
        check_every_split(doc, 2);
    }

    #[test]
    fn the_settled_prefix_actually_advances() {
        let mut stream = MarkdownStream::new();
        let doc = ANSWER.repeat(4);
        let mut at = 0;
        while at < doc.len() {
            let mut next = (at + 200).min(doc.len());
            while !doc.is_char_boundary(next) {
                next += 1;
            }
            stream.push_str(&doc[at..next]);
            at = next;
        }
        assert_eq!(stream.blocks(), parse_markdown(&doc));
        assert!(stream.settled_len() > doc.len() - 200, "tail too long: {}", doc.len() - stream.settled_len());
    }

    #[test]
    fn a_long_answer_dense_sample() {
        let mut doc = String::new();
        while doc.len() < 100 * 1024 {
            doc.push_str(ANSWER.trim_end_matches("~~~\nunclosed tilde fence at the end\n"));
        }
        // Dense sample: every 997th byte (a char boundary) as a single split, plus a
        // 200-byte chunked stream checked at every chunk.
        let mut checked = 0;
        let mut cut = 0;
        while cut <= doc.len() {
            if doc.is_char_boundary(cut) {
                let mut stream = MarkdownStream::from_source(&doc[..cut]);
                stream.push_str(&doc[cut..]);
                assert_eq!(stream.blocks(), parse_markdown(&doc), "split at {cut}");
                checked += 1;
            }
            cut += 997;
        }
        let mut stream = MarkdownStream::new();
        let mut at = 0;
        while at < doc.len() {
            let mut next = (at + 200).min(doc.len());
            while !doc.is_char_boundary(next) {
                next += 1;
            }
            stream.push_str(&doc[at..next]);
            assert_eq!(stream.blocks(), parse_markdown(&doc[..next]), "stream prefix {next}");
            checked += 1;
            at = next;
        }
        assert!(checked > 500);
    }

    #[test]
    fn set_source_extends_or_restarts() {
        let mut stream = MarkdownStream::new();
        assert!(stream.set_source("# a\n\nb"));
        assert!(stream.set_source("# a\n\nbc\n\nd"));
        assert!(!stream.set_source("# z\n"));
        assert_eq!(stream.blocks(), parse_markdown("# z\n"));
    }

    #[test]
    fn settled_json_matches_parse_markdown_settled() {
        let src = "{\"a\": [1, 2]}";
        let stream = MarkdownStream::from_source(src);
        assert_eq!(stream.blocks_settled(true), super::super::parse_markdown_settled(src, true));
        assert_eq!(stream.blocks_settled(false), super::super::parse_markdown_settled(src, false));
    }

    #[test]
    fn streamed_blocks_matches_full_parse_while_growing() {
        let doc = ANSWER.repeat(2);
        let mut at = 0;
        while at < doc.len() {
            let mut next = (at + 50).min(doc.len());
            while !doc.is_char_boundary(next) {
                next += 1;
            }
            assert_eq!(streamed_blocks(&doc[..next]), parse_markdown(&doc[..next]));
            at = next;
        }
    }
}
