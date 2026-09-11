//! Markdown: the block renderer assistant and user turns use — paragraphs,
//! headings, bullet and ordered lists, fenced code blocks, tables, quotes,
//! rules and image placeholders — parsed with pulldown-cmark (no HTML
//! features) and rendered with the same tokens as [`prose`](super::prose).
//!
//! Inline code and fenced blocks highlight through the transcript's own
//! [`syntax_runs`](super::syntax_runs_in), so fences need no extra backend.
//! Raw HTML has no card in the design and no native element to land on, so
//! HTML blocks and inline tags are dropped while their surrounding text is
//! kept. An unclosed fence is an in-progress code block, not raw text: the
//! parser closes it at the end of input, which is exactly what a streaming
//! turn holds mid-chunk.
//!
//! Links: `[label](destination)` and `<autolink>` destinations starting with
//! `http://` or `https://` become [`LinkTarget::Url`]; every other explicit
//! destination becomes [`LinkTarget::Path`]. Bare `http(s)://` URLs and bare
//! workspace paths (`a/b.rs`, with an optional `:line` or `:line:col`
//! suffix) inside plain text are linkified too; code spans are never
//! linkified, because paths inside code are copy targets, not navigation.
//! Clicks leave the component as [`Markdown::on_link`] intents — data in,
//! intents out — so the app (which owns the workspace root and the browser)
//! resolves them; the hover pointer comes free from
//! `InteractiveText::on_click`, which paints `PointingHand` over its ranges.
//!
//! Memoisation: [`parsed_markdown`] parses once per distinct source and
//! shares the blocks across frames through an [`Arc`]. The key is the source
//! alone — blocks do not depend on the style, because runs are built at
//! render time, so one parse serves every style and both themes — and hits
//! are verified against the stored source, so a hash collision cannot hand
//! back another turn's blocks. The bound is [`PARSED_CACHE_CAP`] entries,
//! evicted one at a time by least-recent use: a streaming turn churns a new
//! source per chunk, and per-entry eviction is what keeps that churn from
//! throwing out the settled turns above it, which are re-read every frame and
//! so are always the most recently used. See [`super::memo`].

use std::ops::Range;
use std::sync::{Arc, LazyLock};

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{
    div, font, prelude::*, px, relative, App, ElementId, FontStyle, FontWeight, Hsla, IntoElement,
    SharedString, StrikethroughStyle, TextRun, UnderlineStyle, Window,
};
use gpui_kit::base::{h_flex, v_flex};
use pulldown_cmark::{Alignment, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};

use super::code::code_block;
use super::memo::Memo;
use super::selectable::{
    clamp_range, selectable_text, SelectableText, SelectionHandler, SelectionKey, TextSelection,
};
use super::ProseStyle;
use crate::data::tag;
use crate::util::push_usize;

/// How many parsed sources the memo holds before evicting the least recently
/// used one. Streaming turns churn one source per chunk, so the bound is what
/// keeps the cache from growing with the transcript; 128 is comfortably more
/// than the turns one virtualised viewport can show at once, so every visible
/// turn stays resident while a streaming turn churns through the rest.
const PARSED_CACHE_CAP: usize = 128;
/// Maximum nesting of block quotes (and other recursive block parsing).
/// Markdown from a model never nests this deep; the cap turns adversarial
/// input into flat text instead of a stack overflow.
const MAX_NESTING: u8 = 32;
/// `.a ul{padding-left:18px}` — list markers align with [`prose`](super::prose).
const LIST_INDENT: f32 = 18.0;
/// Gap after a list block: prose gives lists a 6 px trailing margin where
/// paragraphs get the full paragraph gap, and the markdown renderer keeps
/// both so turns that only ever held prose do not move a pixel.
/// Quote rail inset from the rail.
const LIST_AFTER_GAP: f32 = 6.0;
const QUOTE_INDENT: f32 = 12.0;
/// Table cell padding.
const CELL_PAD_X: f32 = 12.0;
/// Table cell vertical padding.
const CELL_PAD_Y: f32 = 6.0;
/// Narrowest a table column gets before the table's own horizontal scroll
/// takes over instead of squeezing the text further.
const TABLE_MIN_COL: f32 = 96.0;
/// Image placeholder geometry: 16 px glyph in a bordered tile.
const IMAGE_GLYPH: f32 = 16.0;
/// Link underline thickness.
const LINK_UNDERLINE: f32 = 1.0;
/// Heading sizes walk down the token ramp so H1 stays under dialog-title
/// scale; H5/H6 stay at body size and read as headings through weight alone.
const HEADING_SIZES: [f32; 6] = [
    scale::FS_20,
    scale::FS_18,
    scale::FS_16,
    scale::FS_14,
    scale::FS_13,
    scale::FS_13,
];
/// Extensions that make a `slash/path` token a workspace-path link.
const PATH_EXTENSIONS: &[&str] = &[
    "rs", "md", "toml", "json", "jsonl", "txt", "py", "ts", "tsx", "js", "css", "html", "yaml",
    "yml", "sh",
];

/// Where a link points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// An `http(s)` URL: `[t](http…)` (explicit) or a bare `https?://` token.
    Url(String),
    /// A workspace path: `[t](relative/path)`, a bare `a/b.rs`, or a bare
    /// `a/b.rs:line` token. The app resolves it against the session workspace.
    Path(String),
}

/// One inline segment.
#[derive(Debug, Clone, PartialEq)]
pub enum Span {
    /// Plain text.
    Text(String),
    /// An inline code span (mono face on the code ground).
    Code(String),
    /// Strong emphasis (semibold).
    Bold(String),
    /// Emphasis (italic).
    Italic(String),
    /// `~struck~` (strikethrough in the body face).
    Strikethrough(String),
    /// A clickable run: explicit `[label](dest)` / `<autolink>`, or a
    /// linkified bare URL / workspace path. The label is flattened text —
    /// emphasis inside a label does not get its own run, because gpui runs
    /// cannot nest and the link style already marks the range.
    Link {
        /// What is drawn (and underlined).
        label: String,
        /// Where a click goes.
        target: LinkTarget,
    },
}

/// Column alignment of a table, from the delimiter row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlign {
    /// No alignment marker (`---`).
    None,
    /// `:--`.
    Left,
    /// `:-:`.
    Center,
    /// `--:`.
    Right,
}

/// One block of a parsed transcript.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A paragraph of inline spans.
    Paragraph(Vec<Span>),
    /// `#`–`######`: `level` is 1–6.
    Heading {
        /// 1–6.
        level: u8,
        /// The heading text.
        spans: Vec<Span>,
    },
    /// A `-`/`*` list; one span vector per item. Nested blocks inside an
    /// item are flattened to space-joined text: list items keep their words,
    /// and full sub-block rendering would need recursive rows the transcript
    /// never fills from model output.
    BulletList(Vec<Vec<Span>>),
    /// A `1.` list starting at `start`.
    OrderedList {
        /// The first item's number.
        start: u64,
        /// One span vector per item.
        items: Vec<Vec<Span>>,
    },
    /// A fenced or indented code block. Renders through [`code_block`], so it
    /// highlights exactly like every other code block in the transcript.
    CodeBlock {
        /// The fence info string's first word, if any.
        lang: Option<String>,
        /// The code without the trailing newline the parser includes.
        text: String,
    },
    /// A pipe table.
    Table {
        /// One span vector per header cell.
        header: Vec<Vec<Span>>,
        /// Rows of cells; each row is padded/truncated to the header width.
        rows: Vec<Vec<Vec<Span>>>,
        /// One alignment per column, from the delimiter row.
        align: Vec<TableAlign>,
    },
    /// `>` lines; nested blocks render recursively, dimmed one ink step.
    Quote(Vec<Block>),
    /// `---` / `***` / `___`: a hairline rule.
    Rule,
    /// `![alt](url)`: a placeholder tile, never a fetch — the native view
    /// has no image loader, and a transcript must not hit the network to
    /// paint.
    Image {
        /// The alt text, or empty when the markup carries none.
        alt: String,
        /// The destination, shown as a tag so it stays scrutable.
        url: String,
    },
}

/// A link's byte range inside its paragraph's shaped text, with its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRange {
    /// Byte range of the label inside the paragraph text.
    pub range: Range<usize>,
    /// Where a click on the range goes.
    pub target: LinkTarget,
}

/// What [`Markdown::on_link`] receives.
pub type LinkHandler = std::rc::Rc<dyn Fn(LinkTarget, &mut Window, &mut App)>;

/// Inline fragments: styled spans, plus images, which cannot ride inside a
/// text run and become their own blocks at the paragraph level.
enum Frag {
    Span(Span),
    Image { alt: String, url: String },
}

/// Pushes a fragment, merging adjacent plain-text spans so runs stay short.
fn push_frag(out: &mut Vec<Frag>, frag: Frag) {
    if let Frag::Span(Span::Text(next)) = &frag {
        if let Some(Frag::Span(Span::Text(prev))) = out.last_mut() {
            prev.push_str(next);
            return;
        }
    }
    out.push(frag);
}

/// Classifies an explicit `[label](dest)` / `<autolink>` destination.
fn classify_dest(link_type: LinkType, dest_url: &str) -> LinkTarget {
    // Email autolinks arrive with a `mailto:` destination, which only a
    // browser can resolve, so they ride the Url arm like every other
    // non-workspace destination.
    if link_type == LinkType::Email {
        return LinkTarget::Url(dest_url.to_string());
    }
    if dest_url.starts_with("http://") || dest_url.starts_with("https://") {
        LinkTarget::Url(dest_url.to_string())
    } else {
        LinkTarget::Path(dest_url.to_string())
    }
}

/// Strips one `:line` / `:line:col` suffix for extension detection; the
/// target keeps the full token including the suffix.
fn strip_line_suffix(core: &str) -> &str {
    let mut rest = core;
    for _ in 0..2 {
        let Some(ix) = rest.rfind(':') else {
            break;
        };
        if rest[ix + 1..].is_empty() || !rest[ix + 1..].bytes().all(|b| b.is_ascii_digit()) {
            break;
        }
        rest = &rest[..ix];
    }
    rest
}

/// Whether a bare word token (affixes already stripped) is a workspace path.
fn is_path_token(core: &str) -> bool {
    if !core.contains('/') {
        return false;
    }
    let base = strip_line_suffix(core);
    let after_slash = base.rsplit('/').next().unwrap_or("");
    let Some(dot) = after_slash.rfind('.') else {
        return false;
    };
    if dot == 0 || dot + 1 == after_slash.len() {
        return false;
    }
    PATH_EXTENSIONS.contains(&after_slash[dot + 1..].to_lowercase().as_str())
}

/// Splits plain text into text spans and bare-URL / bare-path links.
/// Whitespace runs pass through untouched so offsets stay honest.
fn linkify_text(text: &str) -> Vec<Frag> {
    let mut out = Vec::new();
    let mut word_start: Option<usize> = None;
    let flush = |out: &mut Vec<Frag>, word: &str| {
        if word.is_empty() {
            return;
        }
        // Leading affixes (`(`, `"`, …) stay plain text.
        let lead = word.len() - word.trim_start_matches("([{<'\"").len();
        // Trailing sentence punctuation stays plain; a `)` only counts as
        // punctuation when nothing opened one inside the token, so
        // `https://en.wikipedia.org/wiki/X_(disambiguation)` survives.
        let mut trail = word.len();
        while trail > lead {
            let c = word[..trail].chars().next_back().unwrap_or(' ');
            let is_paren = c == ')';
            if ".,;:!?'\"`]}>".contains(c) || (is_paren && !word[lead..trail].contains('(')) {
                trail -= c.len_utf8();
            } else {
                break;
            }
        }
        if lead > 0 {
            push_frag(out, Frag::Span(Span::Text(word[..lead].to_string())));
        }
        let core = &word[lead..trail];
        let target = if core.starts_with("http://") || core.starts_with("https://") {
            Some(LinkTarget::Url(core.to_string()))
        } else if is_path_token(core) {
            Some(LinkTarget::Path(core.to_string()))
        } else {
            None
        };
        match target {
            Some(target) => out.push(Frag::Span(Span::Link {
                label: core.to_string(),
                target,
            })),
            None => {
                if !core.is_empty() {
                    push_frag(out, Frag::Span(Span::Text(core.to_string())));
                }
            }
        }
        if trail < word.len() {
            push_frag(out, Frag::Span(Span::Text(word[trail..].to_string())));
        }
    };
    for (ix, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(start) = word_start.take() {
                let word = &text[start..ix];
                flush(&mut out, word);
            }
            push_frag(&mut out, Frag::Span(Span::Text(c.to_string())));
        } else if word_start.is_none() {
            word_start = Some(ix);
        }
    }
    if let Some(start) = word_start {
        flush(&mut out, &text[start..]);
    }
    out
}

/// Flattens fragments to plain text, for link labels and image alts.
fn frag_text(frags: &[Frag]) -> String {
    let mut s = String::new();
    for frag in frags {
        match frag {
            Frag::Span(Span::Text(t))
            | Frag::Span(Span::Code(t))
            | Frag::Span(Span::Bold(t))
            | Frag::Span(Span::Italic(t))
            | Frag::Span(Span::Strikethrough(t)) => s.push_str(t),
            Frag::Span(Span::Link { label, .. }) => s.push_str(label),
            Frag::Image { alt, .. } => s.push_str(alt),
        }
    }
    s
}

/// Drops fragments' images into alt text, for cells and headings, which have
/// no block structure to host a tile.
fn frag_spans(frags: Vec<Frag>) -> Vec<Span> {
    frags
        .into_iter()
        .filter_map(|frag| match frag {
            Frag::Span(span) => Some(span),
            Frag::Image { alt, .. } if alt.is_empty() => None,
            Frag::Image { alt, .. } => Some(Span::Text(alt)),
        })
        .collect()
}

/// Applies one emphasis level to already-parsed fragments. Code and links
/// keep their own style — a path inside `*…*` still copies as code and still
/// clicks as a link — and `**bold**` wins over `*italic*`, because `Span`
/// carries one style per run and weight reads louder than slant in the
/// transcript.
fn apply_emphasis(frags: Vec<Frag>, strong: bool) -> Vec<Frag> {
    frags
        .into_iter()
        .map(|frag| match frag {
            Frag::Span(Span::Text(t)) if strong => Frag::Span(Span::Bold(t)),
            Frag::Span(Span::Text(t)) => Frag::Span(Span::Italic(t)),
            other => other,
        })
        .collect()
}

/// Applies `~…~` to already-parsed fragments, with the same code/link
/// precedence as [`apply_emphasis`].
fn apply_strikethrough(frags: Vec<Frag>) -> Vec<Frag> {
    frags
        .into_iter()
        .map(|frag| match frag {
            Frag::Span(Span::Text(t)) => Frag::Span(Span::Strikethrough(t)),
            other => other,
        })
        .collect()
}

/// Parser options: CommonMark plus tables and strikethrough. Everything else
/// (footnotes, task lists, math, superscript, heading attributes, definition
/// lists, metadata blocks) stays off, so that markup arrives as literal text
/// instead of structure the transcript cannot render.
fn parser_options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

/// Consumes events through the matching `end`, dropping them. Used for HTML
/// blocks and other structure the transcript has no card for.
fn skip_until(events: &[Event], pos: &mut usize, end: &TagEnd) {
    while let Some(event) = events.get(*pos) {
        *pos += 1;
        if let Event::End(found) = event {
            if found == end {
                return;
            }
        }
    }
}

/// Gathers text through the matching `end`, flattening any nested structure
/// to space-joined words. Used past [`MAX_NESTING`] and for blocks nested
/// inside list items, where keeping the words beats keeping the tree.
fn flatten_until(events: &[Event], pos: &mut usize, end: &TagEnd) -> String {
    let mut s = String::new();
    let push = |s: &mut String, word: &str| {
        if !s.is_empty() && !word.is_empty() {
            s.push(' ');
        }
        s.push_str(word);
    };
    while let Some(event) = events.get(*pos) {
        match event {
            Event::Text(t) | Event::Code(t) => {
                *pos += 1;
                for word in t.split_whitespace() {
                    push(&mut s, word);
                }
            }
            Event::End(found) => {
                *pos += 1;
                if found == end {
                    return s;
                }
            }
            Event::Start(_) => {
                *pos += 1;
            }
            _ => {
                *pos += 1;
            }
        }
    }
    s
}

/// Collects inline fragments through the matching `end`, consuming it.
/// `nested` counts enclosing block parsers so adversarial input flattens
/// instead of recursing without bound.
fn inline_until(events: &[Event], pos: &mut usize, end: &TagEnd, nested: u8) -> Vec<Frag> {
    let mut out = Vec::new();
    while let Some(event) = events.get(*pos) {
        match event {
            Event::Start(Tag::Emphasis) => {
                *pos += 1;
                let inner = inline_until(events, pos, &TagEnd::Emphasis, nested);
                out.extend(apply_emphasis(inner, false));
            }
            Event::Start(Tag::Strong) => {
                *pos += 1;
                let inner = inline_until(events, pos, &TagEnd::Strong, nested);
                out.extend(apply_emphasis(inner, true));
            }
            Event::Start(Tag::Strikethrough) => {
                *pos += 1;
                let inner = inline_until(events, pos, &TagEnd::Strikethrough, nested);
                out.extend(apply_strikethrough(inner));
            }
            // Superscript/subscript need their options, which stay off, so
            // these arms are defensive: the content is kept, the style lost.
            Event::Start(Tag::Superscript) => {
                *pos += 1;
                out.extend(inline_until(events, pos, &TagEnd::Superscript, nested));
            }
            Event::Start(Tag::Subscript) => {
                *pos += 1;
                out.extend(inline_until(events, pos, &TagEnd::Subscript, nested));
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => {
                let target = classify_dest(*link_type, dest_url);
                *pos += 1;
                let inner = inline_until(events, pos, &TagEnd::Link, nested);
                let label = frag_text(&inner);
                if label.is_empty() {
                    push_frag(&mut out, Frag::Span(Span::Text(dest_url.to_string())));
                } else {
                    out.push(Frag::Span(Span::Link { label, target }));
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                *pos += 1;
                let inner = inline_until(events, pos, &TagEnd::Image, nested);
                out.push(Frag::Image {
                    alt: frag_text(&inner),
                    url: dest_url.to_string(),
                });
            }
            Event::Text(text) => {
                *pos += 1;
                if nested > MAX_NESTING {
                    push_frag(&mut out, Frag::Span(Span::Text(text.to_string())));
                } else {
                    out.extend(linkify_text(text));
                }
            }
            Event::Code(code) => {
                *pos += 1;
                // Code spans are never linkified: see the module docs.
                push_frag(&mut out, Frag::Span(Span::Code(code.to_string())));
            }
            Event::SoftBreak => {
                *pos += 1;
                push_frag(&mut out, Frag::Span(Span::Text(" ".to_string())));
            }
            Event::HardBreak => {
                *pos += 1;
                push_frag(&mut out, Frag::Span(Span::Text("\n".to_string())));
            }
            // No HTML rendering in the transcript: the tag goes, the words
            // around it stay.
            Event::Html(_) | Event::InlineHtml(_) => {
                *pos += 1;
            }
            // Footnotes, math and task markers need their options, which stay
            // off; the reference arm below is defensive only.
            Event::FootnoteReference(label) => {
                let label = format!("[^{label}]");
                *pos += 1;
                push_frag(&mut out, Frag::Span(Span::Text(label)));
            }
            Event::InlineMath(math) | Event::DisplayMath(math) => {
                *pos += 1;
                push_frag(&mut out, Frag::Span(Span::Text(math.to_string())));
            }
            Event::TaskListMarker(_) | Event::Rule => {
                *pos += 1;
            }
            // A block nested inside inline content (a loose list's second
            // paragraph, a sub-list, a fence inside an item): flatten to
            // words rather than drop them.
            Event::Start(Tag::Paragraph) => {
                *pos += 1;
                if !out.is_empty() {
                    push_frag(&mut out, Frag::Span(Span::Text(" ".to_string())));
                }
                out.extend(inline_until(events, pos, &TagEnd::Paragraph, nested + 1));
            }
            Event::Start(Tag::CodeBlock(_)) => {
                *pos += 1;
                let text = flatten_until(events, pos, &TagEnd::CodeBlock);
                if !text.is_empty() {
                    push_frag(&mut out, Frag::Span(Span::Code(text)));
                }
            }
            Event::Start(_) => {
                *pos += 1;
            }
            Event::End(found) => {
                *pos += 1;
                if found == end {
                    return out;
                }
            }
        }
    }
    out
}

/// Splits a paragraph's fragments into blocks: text runs stay a paragraph,
/// each image becomes its own [`Block::Image`] in place.
fn paragraph_blocks(frags: Vec<Frag>) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut spans = Vec::new();
    let flush = |spans: &mut Vec<Span>, blocks: &mut Vec<Block>| {
        if spans.is_empty() {
            return;
        }
        let text: String = spans
            .iter()
            .map(|span| match span {
                Span::Text(t)
                | Span::Code(t)
                | Span::Bold(t)
                | Span::Italic(t)
                | Span::Strikethrough(t) => t.as_str(),
                Span::Link { label, .. } => label.as_str(),
            })
            .collect();
        if !text.trim().is_empty() {
            blocks.push(Block::Paragraph(std::mem::take(spans)));
        } else {
            spans.clear();
        }
    };
    for frag in frags {
        match frag {
            Frag::Span(span) => spans.push(span),
            Frag::Image { alt, url } => {
                flush(&mut spans, &mut blocks);
                blocks.push(Block::Image { alt, url });
            }
        }
    }
    flush(&mut spans, &mut blocks);
    blocks
}

/// Parses one table row: each `TableCell` becomes a span vector.
fn parse_row(events: &[Event], pos: &mut usize, end: &TagEnd, nested: u8) -> Vec<Vec<Span>> {
    let mut row = Vec::new();
    while let Some(event) = events.get(*pos) {
        match event {
            Event::Start(Tag::TableCell) => {
                *pos += 1;
                let frags = inline_until(events, pos, &TagEnd::TableCell, nested);
                row.push(frag_spans(frags));
            }
            Event::End(found) => {
                *pos += 1;
                if found == end {
                    return row;
                }
            }
            _ => {
                *pos += 1;
            }
        }
    }
    row
}

/// Collects blocks through the matching `end` (or the end of input when
/// `end` is `None`), consuming the terminator.
fn blocks_until(events: &[Event], pos: &mut usize, end: Option<&TagEnd>, nested: u8) -> Vec<Block> {
    let mut blocks = Vec::new();
    while let Some(event) = events.get(*pos) {
        match event {
            Event::Start(Tag::Paragraph) => {
                *pos += 1;
                let frags = inline_until(events, pos, &TagEnd::Paragraph, nested);
                blocks.extend(paragraph_blocks(frags));
            }
            Event::Start(Tag::Heading { level, .. }) => {
                let level = *level as u8;
                *pos += 1;
                let frags = inline_until(
                    events,
                    pos,
                    &TagEnd::Heading(level_as_heading(level)),
                    nested,
                );
                let spans = frag_spans(frags);
                if !spans.is_empty() {
                    blocks.push(Block::Heading { level, spans });
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                *pos += 1;
                if nested > MAX_NESTING {
                    let text = flatten_until(events, pos, &TagEnd::BlockQuote(None));
                    if !text.trim().is_empty() {
                        blocks.push(Block::Paragraph(vec![Span::Text(text)]));
                    }
                } else {
                    let inner =
                        blocks_until(events, pos, Some(&TagEnd::BlockQuote(None)), nested + 1);
                    if !inner.is_empty() {
                        blocks.push(Block::Quote(inner));
                    }
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Indented => None,
                    CodeBlockKind::Fenced(info) => {
                        let lang = info.split_whitespace().next().unwrap_or("");
                        if lang.is_empty() {
                            None
                        } else {
                            Some(lang.to_string())
                        }
                    }
                };
                *pos += 1;
                let mut text = String::new();
                while let Some(event) = events.get(*pos) {
                    match event {
                        Event::Text(t) | Event::Code(t) => {
                            *pos += 1;
                            text.push_str(t);
                        }
                        Event::End(found) => {
                            *pos += 1;
                            if *found == TagEnd::CodeBlock {
                                break;
                            }
                        }
                        _ => {
                            *pos += 1;
                        }
                    }
                }
                // The parser includes the fence's closing newline; the block
                // renderer numbers lines, so a trailing blank line would
                // number nothing.
                let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
                blocks.push(Block::CodeBlock { lang, text });
            }
            Event::Start(Tag::List(ordered)) => {
                let start = (*ordered).unwrap_or(1);
                let ordered = ordered.is_some();
                *pos += 1;
                let mut items = Vec::new();
                while let Some(event) = events.get(*pos) {
                    match event {
                        Event::Start(Tag::Item) => {
                            *pos += 1;
                            let frags = inline_until(events, pos, &TagEnd::Item, nested);
                            let spans = frag_spans(frags);
                            if !spans.is_empty() {
                                items.push(spans);
                            }
                        }
                        Event::End(found) => {
                            *pos += 1;
                            if *found == TagEnd::List(ordered) {
                                break;
                            }
                        }
                        // A blank line inside a list can surface as a stray
                        // nested event; skip it rather than end the list.
                        _ => {
                            *pos += 1;
                        }
                    }
                }
                if !items.is_empty() {
                    if ordered {
                        blocks.push(Block::OrderedList { start, items });
                    } else {
                        blocks.push(Block::BulletList(items));
                    }
                }
            }
            Event::Start(Tag::Table(alignments)) => {
                let align: Vec<TableAlign> = alignments
                    .iter()
                    .map(|alignment| match alignment {
                        Alignment::None => TableAlign::None,
                        Alignment::Left => TableAlign::Left,
                        Alignment::Center => TableAlign::Center,
                        Alignment::Right => TableAlign::Right,
                    })
                    .collect();
                *pos += 1;
                // The head holds bare cells with no row wrapper.
                let mut header = Vec::new();
                while let Some(event) = events.get(*pos) {
                    match event {
                        Event::Start(Tag::TableHead) => {
                            *pos += 1;
                        }
                        Event::Start(Tag::TableCell) => {
                            *pos += 1;
                            let frags = inline_until(events, pos, &TagEnd::TableCell, nested);
                            header.push(frag_spans(frags));
                        }
                        Event::End(found) => {
                            *pos += 1;
                            if *found == TagEnd::TableHead {
                                break;
                            }
                        }
                        _ => {
                            *pos += 1;
                        }
                    }
                }
                let mut rows = Vec::new();
                while let Some(event) = events.get(*pos) {
                    match event {
                        Event::Start(Tag::TableRow) => {
                            *pos += 1;
                            rows.push(parse_row(events, pos, &TagEnd::TableRow, nested));
                        }
                        Event::End(found) => {
                            *pos += 1;
                            if *found == TagEnd::Table {
                                break;
                            }
                        }
                        _ => {
                            *pos += 1;
                        }
                    }
                }
                if !header.is_empty() {
                    let width = header.len();
                    let mut align = align;
                    align.resize(width, TableAlign::None);
                    let rows: Vec<Vec<Vec<Span>>> = rows
                        .into_iter()
                        .map(|mut row| {
                            row.resize(width, Vec::new());
                            row
                        })
                        .collect();
                    blocks.push(Block::Table {
                        header,
                        rows,
                        align,
                    });
                }
            }
            Event::Start(Tag::HtmlBlock) => {
                *pos += 1;
                skip_until(events, pos, &TagEnd::HtmlBlock);
            }
            // Definition lists, footnotes and metadata need their options,
            // which stay off; skip them structurally if they ever appear.
            Event::Start(Tag::FootnoteDefinition(_)) => {
                *pos += 1;
                skip_until(events, pos, &TagEnd::FootnoteDefinition);
            }
            Event::Start(_) => {
                *pos += 1;
            }
            Event::Rule => {
                *pos += 1;
                blocks.push(Block::Rule);
            }
            // Inline events never open a block, but a defensive paragraph
            // keeps stray text visible instead of dropping it.
            Event::Text(_) | Event::Code(_) | Event::SoftBreak | Event::HardBreak => {
                let mut frags = Vec::new();
                while let Some(event) = events.get(*pos) {
                    match event {
                        Event::Text(text) => {
                            *pos += 1;
                            frags.extend(linkify_text(text));
                        }
                        Event::Code(code) => {
                            *pos += 1;
                            push_frag(&mut frags, Frag::Span(Span::Code(code.to_string())));
                        }
                        Event::SoftBreak => {
                            *pos += 1;
                            push_frag(&mut frags, Frag::Span(Span::Text(" ".to_string())));
                        }
                        Event::HardBreak => {
                            *pos += 1;
                            push_frag(&mut frags, Frag::Span(Span::Text("\n".to_string())));
                        }
                        _ => break,
                    }
                }
                blocks.extend(paragraph_blocks(frags));
            }
            Event::End(found) => {
                *pos += 1;
                if end.is_some_and(|end| found == end) {
                    return blocks;
                }
            }
            _ => {
                *pos += 1;
            }
        }
    }
    blocks
}

/// Rebuilds a [`pulldown_cmark::HeadingLevel`] from its number for end-tag
/// matching; the parser only ever emits 1–6, anything else closes as H1.
fn level_as_heading(level: u8) -> pulldown_cmark::HeadingLevel {
    use pulldown_cmark::HeadingLevel::*;
    match level {
        1 => H1,
        2 => H2,
        3 => H3,
        4 => H4,
        5 => H5,
        _ => H6,
    }
}

/// Parses `source` into blocks, uncached. Prefer [`parsed_markdown`], which
/// memoises this across frames.
pub fn parse_markdown(source: &str) -> Vec<Block> {
    let events: Vec<Event> = Parser::new_ext(source, parser_options()).collect();
    let mut pos = 0;
    blocks_until(&events, &mut pos, None, 0)
}

/// The memo behind [`parsed_markdown`].
static PARSED: LazyLock<Memo<Vec<Block>>> = LazyLock::new(|| Memo::new(PARSED_CACHE_CAP));

/// Parses `source` into shared blocks, memoised across frames: every render
/// of the same turn hits the memo instead of re-running the parser, which is
/// the per-delta re-parse the transcript diagnosis attributes the scroll jank
/// to. See the module docs for the key and the bound.
///
/// `style` is accepted for call-site symmetry with the renderers and is **not**
/// part of the key: [`Block`]s carry no colours or sizes, so blocks parsed
/// under one style are exactly the blocks another style would parse.
pub fn parsed_markdown(source: &str, style: &ProseStyle) -> Arc<Vec<Block>> {
    let _ = style;
    parsed_blocks(source)
}

/// [`parsed_markdown`] without the vestigial style argument: the one entry
/// point every in-crate caller (renderers and the copy path alike) uses.
pub(super) fn parsed_blocks(source: &str) -> Arc<Vec<Block>> {
    PARSED.get_or_insert("", source, parse_markdown)
}

/// The UI face, built once. `font(name)` is cheap but not free, and the run
/// builders below call it twice per text block per frame; a `Font` is a
/// handle (`SharedString` family plus feature and fallback lists), so cloning
/// the shared one is an atomic bump instead of a rebuild.
static UI_FONT: LazyLock<gpui::Font> = LazyLock::new(|| font(scale::FONT_UI));
/// The mono face, built once. See [`UI_FONT`].
static MONO_FONT: LazyLock<gpui::Font> = LazyLock::new(|| font(scale::FONT_MONO));

/// A clone of the shared UI face.
pub(super) fn ui_font() -> gpui::Font {
    UI_FONT.clone()
}

/// A clone of the shared mono face.
pub(super) fn mono_font() -> gpui::Font {
    MONO_FONT.clone()
}

/// Builds the shaped text, runs and link ranges for `spans`. This is the one
/// run builder for both [`prose`](super::prose) and [`Markdown`]: `link_ink`
/// colours link runs, and callers that never emit links pass a shaping-only
/// stand-in, which is safe because colour and underline do not change glyph
/// shaping.
pub fn span_runs(
    spans: &[Span],
    style: &ProseStyle,
    link_ink: Hsla,
) -> (String, Vec<TextRun>, Vec<LinkRange>) {
    let ui: gpui::Font = ui_font();
    let mono: gpui::Font = mono_font();
    let mut text = String::new();
    let mut runs = Vec::new();
    let mut links = Vec::new();
    for span in spans {
        let run = match span {
            Span::Text(s) => TextRun {
                len: s.len(),
                font: ui.clone(),
                color: style.ink,
                background_color: None,
                underline: None,
                strikethrough: None,
            },
            Span::Code(s) => TextRun {
                len: s.len(),
                font: mono.clone(),
                color: style.code_ink,
                background_color: Some(style.code_bg),
                underline: None,
                strikethrough: None,
            },
            Span::Bold(s) => {
                let mut bold = ui.clone();
                bold.weight = FontWeight::SEMIBOLD;
                TextRun {
                    len: s.len(),
                    font: bold,
                    color: style.ink,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }
            }
            Span::Italic(s) => {
                let mut italic = ui.clone();
                italic.style = FontStyle::Italic;
                TextRun {
                    len: s.len(),
                    font: italic,
                    color: style.ink,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }
            }
            // The colour is explicit: gpui paints a `None` strikethrough
            // colour as transparent, so it would silently not draw.
            Span::Strikethrough(s) => TextRun {
                len: s.len(),
                font: ui.clone(),
                color: style.ink,
                background_color: None,
                underline: None,
                strikethrough: Some(StrikethroughStyle {
                    thickness: px(LINK_UNDERLINE),
                    color: Some(style.ink),
                }),
            },
            Span::Link { label, .. } => TextRun {
                len: label.len(),
                font: ui.clone(),
                color: link_ink,
                background_color: None,
                underline: Some(UnderlineStyle {
                    thickness: px(LINK_UNDERLINE),
                    color: Some(link_ink),
                    wavy: false,
                }),
                strikethrough: None,
            },
        };
        let (s, is_link) = match span {
            Span::Text(s)
            | Span::Code(s)
            | Span::Bold(s)
            | Span::Italic(s)
            | Span::Strikethrough(s) => (s, None),
            Span::Link { label, target } => (label, Some(target)),
        };
        if let Some(target) = is_link {
            let start = text.len();
            text.push_str(s);
            if start != text.len() {
                links.push(LinkRange {
                    range: start..text.len(),
                    target: target.clone(),
                });
            }
        } else {
            text.push_str(s);
        }
        runs.push(run);
    }
    (text, runs, links)
}

/// The text and runs of the block that closes `source`, built exactly as
/// [`Markdown`] builds them, so a caller that has to measure where the text
/// ends (the streaming caret) shapes the same glyphs that are painted.
/// `None` when a list, code block, table, rule or image closes the text: the
/// caret does not follow those. Headings count as text — a turn can stream a
/// heading first.
pub fn last_block_runs(
    source: &str,
    style: &ProseStyle,
    link_ink: Hsla,
) -> Option<(String, Vec<TextRun>)> {
    let blocks = parsed_blocks(source);
    let spans = match blocks.last()? {
        Block::Paragraph(spans) => spans,
        Block::Heading { spans, .. } => spans,
        _ => return None,
    };
    let (text, runs, _) = span_runs(spans, style, link_ink);
    if text.is_empty() {
        None
    } else {
        Some((text, runs))
    }
}

/// Render state threaded through block rendering: the link ink, the app's
/// stored selection, the intents, and the highlight colour. `prefix` scopes
/// cell keys inside nested quotes; see [`SelectionKey::quote_prefix`].
#[derive(Clone)]
struct SelCtx {
    link_ink: Hsla,
    selection: Option<TextSelection>,
    on_change: Option<SelectionHandler>,
    on_link: Option<LinkHandler>,
    color: Hsla,
    prefix: String,
}

/// Applies the selection state to a freshly built cell element: the visible
/// range when the app's selection sits on `key`, link ranges with their click
/// intent, and the selection-change intent.
fn finish_cell(
    mut element: SelectableText,
    key: &SelectionKey,
    links: Vec<LinkRange>,
    sel: &SelCtx,
) -> SelectableText {
    if let Some(range) = sel
        .selection
        .as_ref()
        .filter(|current| current.cell == *key)
        .map(|current| current.range.clone())
    {
        element = element.selection(Some(range));
    }
    if !links.is_empty() {
        element = element.links(links);
        if let Some(handler) = &sel.on_link {
            let handler = handler.clone();
            element = element.on_link(move |target, window, cx| handler(target, window, cx));
        }
    }
    if let Some(handler) = &sel.on_change {
        let handler = handler.clone();
        element = element.on_selection_change(move |next, window, cx| handler(next, window, cx));
    }
    element
}

/// Renders `spans` as one selectable text cell: the selection paints behind
/// the glyphs when the app's selection sits on `key`, and a press-release
/// without movement on a link range still clicks through `on_link`.
fn inline_element(
    id: ElementId,
    key: SelectionKey,
    spans: &[Span],
    style: &ProseStyle,
    sel: &SelCtx,
) -> gpui::AnyElement {
    let (text, runs, links) = span_runs(spans, style, sel.link_ink);
    if text.is_empty() {
        return div().into_any_element();
    }
    let element = selectable_text(id, key.clone(), text)
        .runs(runs)
        .selection_color(sel.color);
    div()
        .w_full()
        .child(finish_cell(element, &key, links, sel))
        .into_any_element()
}

/// Renders one list item row: a fixed marker cell plus the item text.
fn list_item(
    marker: String,
    spans: &[Span],
    style: &ProseStyle,
    id: ElementId,
    key: SelectionKey,
    sel: &SelCtx,
) -> gpui::AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .child(
            div()
                .flex_none()
                .w(px(LIST_INDENT))
                .flex()
                .justify_center()
                .text_color(style.ink)
                .child(marker),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(inline_element(id, key, spans, style, sel)),
        )
        .into_any_element()
}

/// Renders one table cell with its column alignment.
fn table_cell(
    spans: &[Span],
    align: TableAlign,
    header: bool,
    style: &ProseStyle,
    id: ElementId,
    key: SelectionKey,
    sel: &SelCtx,
) -> gpui::AnyElement {
    // Headers are uniformly semibold: map plain text onto the bold run
    // rather than threading a weight flag through the run builder.
    let bolded;
    let spans = if header {
        bolded = spans
            .iter()
            .map(|span| match span {
                Span::Text(t) => Span::Bold(t.clone()),
                Span::Italic(t) => Span::Bold(t.clone()),
                other => other.clone(),
            })
            .collect::<Vec<_>>();
        &bolded
    } else {
        spans
    };
    let mut cell = div()
        .flex_1()
        .min_w(px(TABLE_MIN_COL))
        .px(px(CELL_PAD_X))
        .py(px(CELL_PAD_Y))
        .child(inline_element(id, key, spans, style, sel));
    cell = match align {
        TableAlign::None | TableAlign::Left => cell.text_left(),
        TableAlign::Center => cell.text_center(),
        TableAlign::Right => cell.text_right(),
    };
    cell.into_any_element()
}

/// Element ids nest one `(parent, name)` level at a time, so block children
/// key off a name under the turn id. The name is built into one `String` of
/// known capacity rather than through `format!`: this runs for every block of
/// every visible turn on every frame.
pub(super) fn block_id(id: &ElementId, index: usize, name: &str) -> ElementId {
    (id.clone(), SharedString::from(block_name(index, name, None, None))).into()
}

/// `block_id` for a block's `item`-th row (a list item), without the
/// intermediate `format!("li{item}")` the name used to be built with.
fn block_item_id(id: &ElementId, index: usize, name: &str, item: usize) -> ElementId {
    (id.clone(), SharedString::from(block_name(index, name, Some(item), None))).into()
}

/// `block_id` for a table cell: `md-{index}-h{col}` for a header cell,
/// `md-{index}-c{row}-{col}` for a body cell.
fn block_cell_id(id: &ElementId, index: usize, row: Option<usize>, col: usize) -> ElementId {
    let name = match row {
        None => block_name(index, "h", Some(col), None),
        Some(row) => block_name(index, "c", Some(row), Some(col)),
    };
    (id.clone(), SharedString::from(name)).into()
}

/// [`block_item_id`] under a name the measurement module can reach.
#[cfg(test)]
pub(super) fn block_item_id_for_bench(id: &ElementId, index: usize, name: &str, item: usize) -> ElementId {
    block_item_id(id, index, name, item)
}

/// [`markdown_selected_text`] as it was before it went through the memo: a
/// full uncached parse per copy. Kept for the measurement module.
#[cfg(test)]
pub(super) fn selected_text_uncached_for_bench(source: &str, selection: &TextSelection) -> Option<String> {
    selected_text_in_blocks(&parse_markdown(source), selection)
}

/// `md-{index}-{name}{first}{-second}` in one allocation.
fn block_name(index: usize, name: &str, first: Option<usize>, second: Option<usize>) -> String {
    let mut out = String::with_capacity(8 + name.len() + 8);
    out.push_str("md-");
    push_usize(&mut out, index);
    out.push('-');
    out.push_str(name);
    if let Some(first) = first {
        push_usize(&mut out, first);
    }
    if let Some(second) = second {
        out.push('-');
        push_usize(&mut out, second);
    }
    out
}

/// Rendered blocks with the gap applied between (never after) them.
fn render_blocks(
    id: &ElementId,
    blocks: &[Block],
    style: &ProseStyle,
    palette: &aui_tokens::Palette,
    sel: &SelCtx,
) -> Vec<gpui::AnyElement> {
    let mut out = Vec::new();
    let count = blocks.len();
    for (index, block) in blocks.iter().enumerate() {
        let child = render_block(id, index, block, style, palette, sel);
        if index + 1 == count {
            out.push(child);
        } else {
            let gap = match block {
                Block::BulletList(_) | Block::OrderedList { .. } => LIST_AFTER_GAP,
                _ => style.paragraph_gap,
            };
            out.push(div().w_full().mb(px(gap)).child(child).into_any_element());
        }
    }
    out
}

/// Renders one block. `index` keys element ids within the turn; selection
/// keys scope under `sel.prefix`, so quoted cells never collide.
fn render_block(
    id: &ElementId,
    index: usize,
    block: &Block,
    style: &ProseStyle,
    palette: &aui_tokens::Palette,
    sel: &SelCtx,
) -> gpui::AnyElement {
    match block {
        Block::Paragraph(spans) => inline_element(
            block_id(id, index, "p"),
            SelectionKey::paragraph(&sel.prefix, index),
            spans,
            style,
            sel,
        ),
        Block::Heading { level, spans } => {
            let size =
                HEADING_SIZES[(level.saturating_sub(1) as usize).min(HEADING_SIZES.len() - 1)];
            let (text, mut runs, links) = span_runs(spans, style, sel.link_ink);
            // Headings are uniformly semibold; code spans keep the mono
            // face at its own weight so code still reads as code.
            let mono = font(scale::FONT_MONO);
            for run in runs.iter_mut() {
                if run.font.family != mono.family {
                    run.font.weight = FontWeight::SEMIBOLD;
                }
            }
            if text.is_empty() {
                return div().into_any_element();
            }
            let key = SelectionKey::heading(&sel.prefix, index);
            let element = selectable_text(block_id(id, index, "h"), key.clone(), text)
                .runs(runs)
                .selection_color(sel.color);
            let body = div()
                .w_full()
                .child(finish_cell(element, &key, links, sel))
                .into_any_element();
            div()
                .w_full()
                .ui(size)
                .line_height(relative(scale::LH_TIGHT))
                .text_color(style.ink)
                .child(body)
                .into_any_element()
        }
        Block::BulletList(items) => {
            let mut list = v_flex().w_full();
            for (n, item) in items.iter().enumerate() {
                list = list.child(list_item(
                    "•".to_string(),
                    item,
                    style,
                    block_item_id(id, index, "li", n),
                    SelectionKey::list_item(&sel.prefix, index, false, n),
                    sel,
                ));
            }
            list.into_any_element()
        }
        Block::OrderedList { start, items } => {
            let mut list = v_flex().w_full();
            for (n, item) in items.iter().enumerate() {
                list = list.child(list_item(
                    format!("{}.", start + n as u64),
                    item,
                    style,
                    block_item_id(id, index, "li", n),
                    SelectionKey::list_item(&sel.prefix, index, true, n),
                    sel,
                ));
            }
            list.into_any_element()
        }
        Block::CodeBlock { lang, text } => {
            // The header keeps the `code_block` shape (name plus language
            // tag); a fence carries no filename, so the name stays `code`
            // and the info string feeds the tag and the highlighter.
            let mut fenced =
                code_block(block_id(id, index, "code"), "code", text.clone());
            if let Some(lang) = lang {
                fenced = fenced.language(lang.clone());
            }
            // Fences select like every other cell: the block's lines share
            // one key with byte offsets over the whole block text.
            let key = SelectionKey::code(&sel.prefix, index);
            fenced = fenced.selection_key(key.clone()).selection_color(sel.color);
            if let Some(range) = sel
                .selection
                .as_ref()
                .filter(|current| current.cell == key)
                .map(|current| current.range.clone())
            {
                fenced = fenced.selection(Some(range));
            }
            if let Some(handler) = &sel.on_change {
                let handler = handler.clone();
                fenced =
                    fenced.on_selection_change(move |next, window, cx| handler(next, window, cx));
            }
            fenced.into_any_element()
        }
        Block::Table {
            header,
            rows,
            align,
        } => {
            let mut grid = v_flex().w_full();
            let mut head = h_flex().w_full().bg(palette.surface_2);
            for (n, cell) in header.iter().enumerate() {
                head = head.child(table_cell(
                    cell,
                    align.get(n).copied().unwrap_or(TableAlign::None),
                    true,
                    style,
                    block_cell_id(id, index, None, n),
                    SelectionKey::table_cell(&sel.prefix, index, None, n),
                    sel,
                ));
            }
            grid = grid.child(
                div()
                    .w_full()
                    .border_b_1()
                    .border_color(palette.line)
                    .child(head),
            );
            for (r, row) in rows.iter().enumerate() {
                let mut line = h_flex().w_full();
                for (n, cell) in row.iter().enumerate() {
                    line = line.child(table_cell(
                        cell,
                        align.get(n).copied().unwrap_or(TableAlign::None),
                        false,
                        style,
                        block_cell_id(id, index, Some(r), n),
                        SelectionKey::table_cell(&sel.prefix, index, Some(r), n),
                        sel,
                    ));
                }
                if r > 0 {
                    line = line.border_t_1().border_color(palette.line);
                }
                grid = grid.child(line);
            }
            // The table scrolls inside its own frame instead of squeezing
            // columns or spilling past the turn.
            div()
                .w_full()
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(palette.line)
                .bg(palette.surface_1)
                .overflow_hidden()
                .child(div().id(block_id(id, index, "scroll")).w_full().overflow_x_scroll().child(grid))
                .into_any_element()
        }
        Block::Quote(inner) => {
            // Quotes nest the same renderer one ink step dimmer, behind a
            // hairline rail; no accent, per the calm-colour rule. Selection
            // keys scope under the quote prefix so quoted cells never collide
            // with top-level ones.
            let mut dimmed = *style;
            dimmed.ink = palette.ink_2;
            let nested: ElementId =
                (id.clone(), SharedString::from(format!("md-{index}-q"))).into();
            let quoted = SelCtx {
                prefix: SelectionKey::quote_prefix(&sel.prefix, index),
                ..sel.clone()
            };
            div()
                .w_full()
                .border_l_1()
                .border_color(palette.line)
                .pl(px(QUOTE_INDENT))
                .children(render_blocks(&nested, inner, &dimmed, palette, &quoted))
                .into_any_element()
        }
        Block::Rule => div()
            .w_full()
            .h(px(1.0))
            .bg(palette.line)
            .into_any_element(),
        Block::Image { alt, url } => {
            let caption = if alt.is_empty() {
                "Image"
            } else {
                alt.as_str()
            };
            h_flex()
                .w_full()
                .items_center()
                .gap(px(CELL_PAD_X))
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(palette.line)
                .bg(palette.surface_2)
                .px(px(CELL_PAD_X))
                .py(px(CELL_PAD_Y))
                .child(
                    icon(IconName::Image)
                        .size(px(IMAGE_GLYPH))
                        .color(palette.ink_3),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .ui(scale::FS_12)
                        .line_height(relative(scale::LH_UI))
                        .text_color(palette.ink_2)
                        .child(div().w_full().truncate().child(caption.to_string()))
                        .child(tag(url.clone())),
                )
                .into_any_element()
        }
    }
}

/// A markdown block column. Build with [`markdown`].
#[derive(IntoElement)]
pub struct Markdown {
    id: ElementId,
    source: SharedString,
    style: ProseStyle,
    on_link: Option<LinkHandler>,
    selection: Option<TextSelection>,
    on_selection_change: Option<SelectionHandler>,
}

/// Renders `source` as markdown blocks in `style`.
pub fn markdown(
    id: impl Into<ElementId>,
    source: impl Into<SharedString>,
    style: ProseStyle,
) -> Markdown {
    Markdown {
        id: id.into(),
        source: source.into(),
        style,
        on_link: None,
        selection: None,
        on_selection_change: None,
    }
}

impl Markdown {
    /// Click handler for links: the argument is the clicked range's target.
    pub fn on_link(mut self, f: impl Fn(LinkTarget, &mut Window, &mut App) + 'static) -> Self {
        self.on_link = Some(std::rc::Rc::new(f));
        self
    }

    /// The stored selection this render highlights: the app owns one
    /// [`Option<TextSelection>`] per markdown view and passes it back here.
    pub fn selection(mut self, selection: Option<&TextSelection>) -> Self {
        self.selection = selection.cloned();
        self
    }

    /// Selection intents: drags and word / paragraph picks arrive as `Some`,
    /// plain clicks elsewhere in a cell arrive as `None` (clearing).
    pub fn on_selection_change(
        mut self,
        f: impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::rc::Rc::new(f));
        self
    }

    /// Copies the selected text out of `selection`: the slice of the holding
    /// cell's shaped text (code spans and link labels read as plain words),
    /// or `None` when the key addresses no cell or the range is empty. The
    /// app puts this on the clipboard on ⌘C; the keybinding stays with the
    /// app.
    pub fn selected_text(&self, selection: &TextSelection) -> Option<String> {
        selected_text_in_blocks(&parsed_blocks(&self.source), selection)
    }
}

/// Concatenates the shaped text of `spans`: what [`span_runs`] draws, minus
/// styling. Keep the arms in sync with `span_runs` — a string added there
/// must read here, or copies silently drop it.
fn spans_text(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|span| match span {
            Span::Text(s)
            | Span::Code(s)
            | Span::Bold(s)
            | Span::Italic(s)
            | Span::Strikethrough(s) => s.as_str(),
            Span::Link { label, .. } => label.as_str(),
        })
        .collect()
}

/// Copies the selected text out of `source` without building a view: the
/// slice of the holding cell's shaped text, or `None` when the key addresses
/// no cell or the range is empty. Blocks do not depend on the render style,
/// so this reads exactly what [`Markdown::selected_text`] would return for a
/// view of the same source. Turn views ([`UserTurn`](super::UserTurn),
/// [`AssistantTurn`](super::AssistantTurn)) have no `selected_text` method of
/// their own — the app copies through this, or through
/// [`turn_selected_text`](super::turn_selected_text).
pub fn markdown_selected_text(source: &str, selection: &TextSelection) -> Option<String> {
    selected_text_in_blocks(&parsed_blocks(source), selection)
}

/// Slices `selection` out of already-parsed `blocks`: the shared lookup
/// behind [`Markdown::selected_text`] and [`markdown_selected_text`].
fn selected_text_in_blocks(blocks: &[Block], selection: &TextSelection) -> Option<String> {
    let mut found = None;
    walk_units(blocks, "", &mut |key, content| {
        if found.is_none() && key == selection.cell {
            found = Some(match content {
                CellContent::Spans(spans) => spans_text(spans),
                CellContent::Code(code) => code.to_string(),
            });
        }
    });
    let cell = found?;
    clamp_range(selection.range.clone(), &cell).map(|range| cell[range].to_string())
}

/// One selectable cell's content for key lookup.
enum CellContent<'a> {
    Spans(&'a [Span]),
    Code(&'a str),
}

/// Walks `blocks` in render order, calling `f` per selectable cell with its
/// key. The traversal mirrors [`render_block`] (including quote prefixes), so
/// the keys found here are the keys cells paint with.
fn walk_units<'a>(
    blocks: &'a [Block],
    prefix: &str,
    f: &mut impl FnMut(SelectionKey, CellContent<'a>),
) {
    for (index, block) in blocks.iter().enumerate() {
        match block {
            Block::Paragraph(spans) => {
                f(
                    SelectionKey::paragraph(prefix, index),
                    CellContent::Spans(spans),
                );
            }
            Block::Heading { spans, .. } => {
                f(
                    SelectionKey::heading(prefix, index),
                    CellContent::Spans(spans),
                );
            }
            Block::BulletList(items) => {
                for (n, item) in items.iter().enumerate() {
                    f(
                        SelectionKey::list_item(prefix, index, false, n),
                        CellContent::Spans(item),
                    );
                }
            }
            Block::OrderedList { items, .. } => {
                for (n, item) in items.iter().enumerate() {
                    f(
                        SelectionKey::list_item(prefix, index, true, n),
                        CellContent::Spans(item),
                    );
                }
            }
            Block::CodeBlock { text, .. } => {
                f(SelectionKey::code(prefix, index), CellContent::Code(text));
            }
            Block::Table { header, rows, .. } => {
                for (n, cell) in header.iter().enumerate() {
                    f(
                        SelectionKey::table_cell(prefix, index, None, n),
                        CellContent::Spans(cell),
                    );
                }
                for (r, row) in rows.iter().enumerate() {
                    for (n, cell) in row.iter().enumerate() {
                        f(
                            SelectionKey::table_cell(prefix, index, Some(r), n),
                            CellContent::Spans(cell),
                        );
                    }
                }
            }
            Block::Quote(inner) => {
                walk_units(inner, &SelectionKey::quote_prefix(prefix, index), f);
            }
            Block::Rule | Block::Image { .. } => {}
        }
    }
}

impl RenderOnce for Markdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.aui().colors;
        let blocks = parsed_blocks(&self.source);
        let sel = SelCtx {
            link_ink: palette.accent,
            selection: self.selection.clone(),
            on_change: self.on_selection_change.clone(),
            on_link: self.on_link.clone(),
            color: palette.selection,
            prefix: String::new(),
        };
        v_flex()
            .id(self.id.clone())
            .w_full()
            .ui(self.style.size)
            .line_height(relative(self.style.line_height))
            .text_color(self.style.ink)
            .children(render_blocks(&self.id, &blocks, &self.style, &palette, &sel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> ProseStyle {
        ProseStyle {
            ink: gpui::black(),
            code_ink: gpui::red(),
            code_bg: gpui::white(),
            size: 13.5,
            line_height: 1.65,
            paragraph_gap: 10.0,
        }
    }

    #[test]
    fn headings_become_leveled_blocks() {
        let blocks = parse_markdown("# Title\n\n## Section\n\nText");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Heading { level: 1, .. }));
        assert!(matches!(&blocks[1], Block::Heading { level: 2, .. }));
        assert!(matches!(&blocks[2], Block::Paragraph(_)));
        let Block::Heading { level, spans } = &blocks[1] else {
            panic!("expected a heading")
        };
        assert_eq!(*level, 2);
        assert_eq!(spans, &vec![Span::Text("Section".into())]);
    }

    #[test]
    fn tables_carry_header_rows_and_alignment() {
        let blocks =
            parse_markdown("| a | b | c |\n| :-- | :-: | --: |\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |");
        assert_eq!(blocks.len(), 1);
        let Block::Table {
            header,
            rows,
            align,
        } = &blocks[0]
        else {
            panic!("expected a table")
        };
        assert_eq!(header.len(), 3);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 3);
        assert_eq!(
            align,
            &vec![TableAlign::Left, TableAlign::Center, TableAlign::Right]
        );
        assert_eq!(header[0], vec![Span::Text("a".into())]);
    }

    #[test]
    fn an_unclosed_fence_is_an_in_progress_code_block() {
        let blocks = parse_markdown("```rs\nlet x = 1;\n");
        assert_eq!(blocks.len(), 1);
        let Block::CodeBlock { lang, text } = &blocks[0] else {
            panic!("expected a code block, got {blocks:?}")
        };
        assert_eq!(lang, &Some("rs".to_string()));
        assert!(text.contains("let x = 1;"), "unexpected code: {text:?}");
        // No stray paragraph keeps the fence text.
        assert!(!blocks
            .iter()
            .any(|block| matches!(block, Block::Paragraph(_))));
    }

    #[test]
    fn links_cover_explicit_bare_and_path_shapes() {
        let blocks = parse_markdown(
            "[Docs](https://example.com) and https://example.org/x plus crates/aui/src/lib.rs:12",
        );
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("expected a paragraph")
        };
        let targets: Vec<&LinkTarget> = spans
            .iter()
            .filter_map(|span| match span {
                Span::Link { target, .. } => Some(target),
                _ => None,
            })
            .collect();
        assert_eq!(
            targets,
            vec![
                &LinkTarget::Url("https://example.com".into()),
                &LinkTarget::Url("https://example.org/x".into()),
                &LinkTarget::Path("crates/aui/src/lib.rs:12".into()),
            ]
        );
    }

    #[test]
    fn relative_explicit_links_are_paths() {
        let blocks = parse_markdown("[turns](crates/aui/src/transcript/turns.rs)");
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("expected a paragraph")
        };
        assert_eq!(
            spans,
            &vec![Span::Link {
                label: "turns".into(),
                target: LinkTarget::Path("crates/aui/src/transcript/turns.rs".into()),
            }]
        );
    }

    #[test]
    fn code_spans_never_linkify() {
        let blocks = parse_markdown("run `crates/aui/src/lib.rs:12` now");
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("expected a paragraph")
        };
        assert!(spans.iter().all(|span| !matches!(span, Span::Link { .. })));
        assert!(spans.contains(&Span::Code("crates/aui/src/lib.rs:12".into())));
    }

    #[test]
    fn emphasis_and_strikethrough_parse() {
        let blocks = parse_markdown("*italic* **bold** ~struck~");
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("expected a paragraph")
        };
        assert!(spans.contains(&Span::Italic("italic".into())));
        assert!(spans.contains(&Span::Bold("bold".into())));
        assert!(spans.contains(&Span::Strikethrough("struck".into())));
    }

    #[test]
    fn link_ranges_cover_exactly_their_labels() {
        let spans = vec![
            Span::Text("see ".into()),
            Span::Link {
                label: "docs".into(),
                target: LinkTarget::Url("https://example.com".into()),
            },
            Span::Text(" now".into()),
        ];
        let (text, runs, links) = span_runs(&spans, &style(), gpui::red());
        assert_eq!(text, "see docs now");
        assert_eq!(runs.iter().map(|run| run.len).sum::<usize>(), text.len());
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].range, 4..8);
        assert_eq!(&text[links[0].range.clone()], "docs");
    }

    #[test]
    fn memoised_parses_share_one_allocation() {
        let style = style();
        let first = parsed_markdown("# Hi\n\nSome text.", &style);
        let second = parsed_markdown("# Hi\n\nSome text.", &style);
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn the_style_is_not_part_of_the_memo_key() {
        // Blocks carry no colours, so a theme switch must not re-parse.
        let mut other = style();
        other.ink = gpui::white();
        other.size += 1.0;
        let first = parsed_markdown("# Theme\n\nSwitch.", &style());
        let second = parsed_markdown("# Theme\n\nSwitch.", &other);
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn the_copy_path_reads_the_same_blocks_as_a_view() {
        let source = "One `two` three.\n\n- bullet";
        let selection = TextSelection {
            cell: SelectionKey::paragraph("", 0),
            range: 0..7,
        };
        assert_eq!(
            markdown_selected_text(source, &selection),
            markdown("copy-path", source, style()).selected_text(&selection),
        );
    }

    #[test]
    fn selected_text_reads_code_and_link_labels_as_plain_words() {
        let doc = markdown(
            "sel-test",
            "Tightened `validateAddress` — see the [turns card](transcript/turns) for context.",
            style(),
        );
        let cell = SelectionKey::paragraph("", 0);
        let whole = doc
            .selected_text(&TextSelection {
                cell: cell.clone(),
                range: 0..1000,
            })
            .expect("a paragraph range selects");
        assert_eq!(
            whole,
            "Tightened validateAddress — see the turns card for context."
        );
        // Sub-ranges address the shaped text: the code span…
        // ("Tightened " is 10 bytes, "validateAddress" 15).
        let code = doc
            .selected_text(&TextSelection {
                cell: cell.clone(),
                range: 10..25,
            })
            .expect("code span selects");
        assert_eq!(code, "validateAddress");
        // …and the link label, without its destination.
        let label = whole.find("turns card").expect("label is drawn");
        let link = doc
            .selected_text(&TextSelection {
                cell: cell.clone(),
                range: label..label + "turns card".len(),
            })
            .expect("link label selects");
        assert_eq!(link, "turns card");
        // Unknown keys and empty ranges select nothing.
        assert!(
            doc.selected_text(&TextSelection {
                cell: SelectionKey::paragraph("", 9),
                range: 0..4,
            })
            .is_none()
        );
        assert!(
            doc.selected_text(&TextSelection {
                cell,
                range: 4..4,
            })
            .is_none()
        );
    }

    #[test]
    fn selected_text_covers_quotes_tables_and_code() {
        let doc = markdown(
            "sel-cells",
            "> Quoted words\n\n| a | b |\n| -- | -- |\n| 1 | 2 |\n\n```rust\nlet x = 1;\n```\n",
            style(),
        );
        let quote = doc
            .selected_text(&TextSelection {
                cell: SelectionKey::paragraph(&SelectionKey::quote_prefix("", 0), 0),
                range: 0..6,
            })
            .expect("quoted cells scope under the quote prefix");
        assert_eq!(quote, "Quoted");
        let cell = doc
            .selected_text(&TextSelection {
                cell: SelectionKey::table_cell("", 1, Some(0), 1),
                range: 0..8,
            })
            .expect("table body cells select");
        assert_eq!(cell, "2");
        let code = doc
            .selected_text(&TextSelection {
                cell: SelectionKey::code("", 2),
                range: 0..10,
            })
            .expect("fenced blocks select over the whole block text");
        assert_eq!(code, "let x = 1;");
    }

    #[test]
    fn last_block_runs_follows_text_not_structure() {
        let style = style();
        let (text, _) = last_block_runs("- one\n\nDone `now`", &style, gpui::red())
            .expect("a paragraph is followed");
        assert_eq!(text, "Done now");
        assert!(last_block_runs("intro\n\n- one", &style, gpui::red()).is_none());
        assert!(last_block_runs("- one\n- two", &style, gpui::red()).is_none());
        assert!(last_block_runs("```rs\nlet x = 1;\n", &style, gpui::red()).is_none());
    }
}
