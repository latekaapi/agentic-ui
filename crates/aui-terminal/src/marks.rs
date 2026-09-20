//! Nonce-checked OSC 133 marks and the blocks assembled from them.
//!
//! Decision D44 ("grid is the truth, blocks are an overlay"): the emulator in
//! [`TerminalSession`](crate::grid::TerminalSession) owns the grid, and the
//! blocks drawn over it are computed from the same byte stream the grid sees,
//! stored as absolute line numbers. Decision D45 (the nonce rule): every
//! marker must carry `k=<nonce>` where the nonce is the session's own, minted
//! per session by the `pty` module. A marker with the wrong nonce — or with
//! no `k=` at all — is ignored entirely, so a program that prints a bare
//! `OSC 133` cannot forge a block boundary.
//!
//! The grammar scanned here is the one the shell-integration snippets emit
//! (see `pty.rs`, which is the specification when in doubt):
//!
//! ```text
//! OSC 133 ; A ; k=<nonce> BEL                             prompt start
//! OSC 133 ; B ; k=<nonce> BEL                             prompt end / command start
//! OSC 133 ; C ; k=<nonce> ; cmd=<b64|raw> ; enc=<b64|raw> BEL  pre-exec: output begins
//! OSC 133 ; D ; <exit> ; k=<nonce> BEL                    command done, with exit status
//! ```
//!
//! Our own snippets no longer emit `B` (see `pty.rs` for why the prompt
//! cannot be bracketed): the command text travels on `C` as `cmd=`,
//! base64-encoded (`enc=b64`) or a sanitised literal (`enc=raw`) when
//! `base64` is missing from `PATH`. A nonced `B` from an emitter that is
//! not ours is still parsed — the fallback scrape needs it.
//!
//! [`MarkScanner`] splits each chunk the session feeds the emulator into
//! [`ScanSegment`]s: raw bytes that pass through verbatim, and complete
//! nonce-checked markers. The session feeds both halves to the emulator in
//! order — the mark and the screen can never drift apart — and records the
//! emulator's absolute line number at each marker. [`assemble_blocks`] then
//! folds the mark stream into [`Block`]s.

#![warn(missing_docs)]

use std::time::Instant;

/// Introducer byte shared by every escape sequence the scanner skips over.
const ESC: u8 = 0x1b;
/// `BEL` terminates the OSC sequences the shell snippets emit.
const BEL: u8 = 0x07;
/// The trailing bytes of the `ST` terminator (`ESC \`).
const ST_FINAL: u8 = 0x5c;
/// OSC introducer second byte (`ESC ]`).
const OSC_INTRO: u8 = b']';
/// How many trailing bytes an incomplete sequence may hold across chunks.
/// Longer than any marker the snippets emit; anything past it is flushed
/// through rather than held forever.
const MAX_PENDING: usize = 512;

/// Which of the four shell-integration markers a [`Mark`] records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    /// `OSC 133 ; A` — the shell is about to draw a prompt.
    PromptStart,
    /// `OSC 133 ; B` — the prompt is drawn; the echoed command follows.
    PromptEnd,
    /// `OSC 133 ; C` — pre-exec: the command is running, its output follows.
    OutputStart,
    /// `OSC 133 ; D ; <exit>` — the command finished; [`Mark::exit`]
    /// carries the status.
    CommandDone,
}

/// One shell-integration marker at an absolute grid line.
///
/// `line` is monotonic for the life of the session: the session counts every
/// line evicted from its retained scrollback window — overflow trims as well
/// as `ESC[3J`/`ESC c` drops — and adds that count to the grid's own history
/// offset, so absolute lines keep advancing past the scrollback cap instead
/// of stalling at it. The count is exact unless a single burst reached the
/// emulator's own history cap before the session could count it (see
/// `TerminalSession::history_desyncs`); monotonicity holds either way. A
/// resize reflows the grid (the session marks its blocks stale and rebuilds
/// them lazily); that imprecision is accepted per D44. Anchors below the
/// evicted floor are pruned, and the text of evicted lines is gone: only the
/// retained window reads back.
#[derive(Debug, Clone)]
pub struct Mark {
    /// Absolute grid line where the marker was seen.
    pub line: i32,
    /// Which marker was seen.
    pub kind: MarkKind,
    /// Exit status for [`MarkKind::CommandDone`]; `None` for the other kinds.
    pub exit: Option<i32>,
    /// When the session accepted the marker; blocks read durations off this.
    pub at: Instant,
    /// The command text carried on [`MarkKind::OutputStart`], decoded from
    /// the `C` marker's `cmd=` payload (see [`decode_command`]). `None` when
    /// the emitter sent none (an OSC 133 emitter that is not ours) or when
    /// the payload was rejected; the block then falls back to the grid
    /// scrape. `None` for the other kinds.
    pub command: Option<String>,
}

/// Who started a [`Block`]: the human at the keyboard, or an agent driving
/// the same tab.
///
/// An enum, not a bool, and deliberately free of product names: the library
/// does not know what any particular agent is called. The host sets the
/// author (see `TerminalSession::set_block_author`) and decides the label —
/// D43's `M` mark, for example, is a host rendering choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockAuthor {
    /// The human typed the command.
    #[default]
    Human,
    /// An agent ran the command; the host labels it.
    Agent,
}

/// One command block assembled from consecutive [`Mark`]s.
///
/// `prompt`, `command_line` and `output` are absolute line ranges
/// (`start..end`, end exclusive); `end` is the absolute line where the block
/// closed. A block with a `C` but no `D` yet is still running: `exit` and
/// `ended` are both `None`.
#[derive(Debug, Clone)]
pub struct Block {
    /// Absolute lines holding the prompt: the `A` line up to the `B` line.
    pub prompt: (i32, i32),
    /// Absolute lines holding the echoed command: the `B` line up to the `C` line.
    pub command_line: (i32, i32),
    /// Absolute lines holding the output: the `C` line up to `end`.
    pub output: (i32, i32),
    /// Absolute line where the block ended (the `D` line, or the live tail
    /// while running).
    pub end: i32,
    /// Exit status; `None` while the block is still running.
    pub exit: Option<i32>,
    /// The block's command text. Our own shell integration carries it as a
    /// payload on the `C` marker (see [`Mark::command`]), which is exact:
    /// the shell hands us the command line, so no screen geometry is
    /// involved. Only when the payload is absent (an OSC 133 emitter that is
    /// not ours) is the text read off the grid between `B` and `C` instead —
    /// and that scrape includes the prompt's own text, because the grid does
    /// not record where the prompt ends and the echo begins, so the split is
    /// approximate.
    pub command: String,
    /// When the block started (the `A` mark, or the `C` mark when no `A` came).
    pub started: Instant,
    /// When the `D` mark arrived; `None` while the block is still running.
    pub ended: Option<Instant>,
    /// Who started the block. The assembler always leaves [`BlockAuthor::Human`];
    /// the host upgrades agent blocks afterwards.
    pub author: BlockAuthor,
    /// Monotonic id assigned when the block is first assembled, never reused.
    /// Rebuilds carry host-set state across by this id, not by index —
    /// pruning shifts positional indices — so a host can hold it as a stable
    /// reference.
    pub id: u64,
}

impl Block {
    /// Whether the block is still running: no `D` has arrived, so `exit`
    /// and `ended` are both `None`.
    pub fn running(&self) -> bool {
        self.ended.is_none()
    }
}

/// A resumable read position for the session's `range_text`.
///
/// The anchor is the last absolute grid line already delivered; `range_text`
/// returns what follows it. Use `TextCursor::start` for the first poll and
/// the session's `tail_cursor` to advance after each read.
///
/// **Invalidation rule.** The anchor is an absolute grid line, and content
/// can move under it: after a resize reflow the lines may have shifted, and
/// once output passes the scrollback cap the oldest lines are trimmed. A
/// cursor older than the retained window clamps to the oldest retained line
/// (lines may repeat); a cursor past the tail reads empty. Cursors are
/// per-session: a cursor from another session anchors a meaningless line in
/// this one's grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCursor {
    /// The last absolute grid line already delivered (exclusive).
    pub line: i32,
}

impl TextCursor {
    /// A cursor before the first line: `range_text` returns everything retained.
    pub fn start() -> Self {
        Self { line: i32::MIN }
    }
}

/// One piece of a chunk after [`MarkScanner::split_feed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanSegment {
    /// Raw bytes — program text and escapes that are not accepted markers.
    /// Feed them to the emulator verbatim, in order.
    Emit(Vec<u8>),
    /// A complete marker whose `k=` matched the session nonce. Feed `raw`
    /// to the emulator verbatim (it sees the identical stream), then record
    /// a [`Mark`] at the emulator's current absolute line.
    Marker {
        /// The marker's raw bytes, terminator included.
        raw: Vec<u8>,
        /// Which marker was accepted.
        kind: MarkKind,
        /// Exit status for [`MarkKind::CommandDone`]; `None` otherwise.
        exit: Option<i32>,
        /// Decoded command text for [`MarkKind::OutputStart`] (see
        /// [`Mark::command`]); `None` otherwise, or when the `C` marker
        /// carried no usable payload.
        command: Option<String>,
    },
}

/// Incremental, nonce-checked OSC 133 scanner over the session's byte stream.
///
/// Chunk boundaries are free: a marker split across two `split_feed` calls
/// is held in `pending` and reported once its terminator arrives. Markers
/// whose `k=` is missing or does not equal the nonce never become
/// [`ScanSegment::Marker`]s — their bytes still flow through as
/// [`ScanSegment::Emit`], so the emulator sees the identical stream.
pub struct MarkScanner {
    /// The `k=<nonce>` every accepted marker must carry. Empty matches
    /// nothing: a session that has not pinned its nonce records no marks.
    nonce: String,
    /// The tail of the previous chunk that may complete next chunk.
    pending: Vec<u8>,
    /// Accepted marks, in stream order.
    marks: Vec<Mark>,
}

impl MarkScanner {
    /// A scanner expecting `nonce`. Empty matches nothing (see the type docs).
    pub fn new(nonce: impl Into<String>) -> Self {
        Self { nonce: nonce.into(), pending: Vec::new(), marks: Vec::new() }
    }

    /// Pins (or re-pins) the expected nonce. Marks accepted so far are kept.
    pub fn set_nonce(&mut self, nonce: &str) {
        self.nonce = nonce.to_string();
    }

    /// The accepted marks, in stream order.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Records a mark the session attributed to the emulator's current line.
    /// The session calls this once per [`ScanSegment::Marker`].
    pub fn push_mark(&mut self, mark: Mark) {
        self.marks.push(mark);
    }

    /// Drops marks that have fallen out of the retained window: a mark is
    /// kept when its line is at or past `floor` (the oldest retained absolute
    /// line) or lies inside one of `keep` (absolute `start..=end` ranges of
    /// blocks that straddle the boundary). Straddling blocks keep their marks
    /// so their ranges — and their ids — survive the prune.
    pub fn prune_before(&mut self, floor: i32, keep: &[(i32, i32)]) {
        self.marks
            .retain(|m| m.line >= floor || keep.iter().any(|&(s, e)| s <= m.line && m.line <= e));
    }

    /// Splits `bytes` into emulator-ready segments, honouring the bytes held
    /// from the previous chunk. Concatenating every [`ScanSegment::Emit`]
    /// payload with every [`ScanSegment::Marker::raw`], in order, reproduces
    /// the input stream exactly.
    pub fn split_feed(&mut self, bytes: &[u8]) -> Vec<ScanSegment> {
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(bytes);
        let mut segments: Vec<ScanSegment> = Vec::new();
        let mut text_start = 0usize;
        let mut pos = 0usize;
        // Absolute offset where an incomplete trailing sequence starts, if any.
        let mut held: Option<usize> = None;
        while pos < buf.len() {
            let Some(rel) = buf[pos..].iter().position(|&b| b == ESC) else {
                break;
            };
            let esc = pos + rel;
            if buf[esc..].starts_with(&[ESC, OSC_INTRO]) {
                match osc_end(&buf[esc..]) {
                    Some(len) => {
                        if text_start < esc {
                            segments.push(ScanSegment::Emit(buf[text_start..esc].to_vec()));
                        }
                        let raw = buf[esc..esc + len].to_vec();
                        match parse_133(&raw, &self.nonce) {
                            Some((kind, exit, command)) => {
                                segments.push(ScanSegment::Marker { raw, kind, exit, command });
                            }
                            None => segments.push(ScanSegment::Emit(raw)),
                        }
                        pos = esc + len;
                        text_start = pos;
                    }
                    None => {
                        held = Some(esc);
                        break;
                    }
                }
            } else {
                pos = esc + 1;
            }
        }
        let tail_from = held.unwrap_or(buf.len());
        // Everything before a held incomplete OSC is emittable — except its
        // own trailing end, which may be a partial introducer that only the
        // next chunk completes (a lone `ESC`, `ESC ]` or `ESC [` plus
        // parameter bytes). That suffix is held back with the pending bytes.
        let mut head = buf[text_start..tail_from].to_vec();
        let mut hold = buf[tail_from..].to_vec();
        if held.is_none() {
            if let Some(split) = trailing_partial(&head) {
                hold = head[split..].to_vec();
                head.truncate(split);
            }
        }
        if !head.is_empty() {
            segments.push(ScanSegment::Emit(head));
        }
        if hold.len() > MAX_PENDING {
            // Pathological: an unterminated sequence longer than any real
            // marker. Flush rather than hold forever.
            segments.push(ScanSegment::Emit(hold));
        } else {
            self.pending = hold;
        }
        segments
    }
}

/// Length of the OSC sequence at the start of `seq` (terminator included),
/// or `None` when no terminator has arrived yet. `seq` must start with
/// `ESC ]`.
fn osc_end(seq: &[u8]) -> Option<usize> {
    let mut i = 2usize;
    while i < seq.len() {
        if seq[i] == BEL {
            return Some(i + 1);
        }
        if seq[i] == ESC && seq.get(i + 1) == Some(&ST_FINAL) {
            return Some(i + 2);
        }
        i += 1;
    }
    None
}

/// The most command text a decoded `C` payload may contribute to a block:
/// 4 KB, truncating at a character boundary. Generous for a command line,
/// and a bound so a hostile emitter cannot grow blocks without limit.
pub(crate) const MAX_CMD_LEN: usize = 4096;

/// Decodes the command payload of a `C` marker: `cmd` is the raw `cmd=`
/// value, `enc` the raw `enc=` value. Our own snippets emit `enc=b64` with a
/// base64 command line (so newlines, `;`, BEL and UTF-8 pass through
/// untouched), or `enc=raw` with a sanitised literal when `base64` is not on
/// `PATH`. Either encoding carries no `;` by construction, so splitting the
/// marker body on `;` never cuts a value in half.
///
/// A missing `cmd=` is "no command text", not an error: returns `None` and
/// the block falls back to the grid scrape. An `enc=b64` value that is not
/// valid base64 is rejected the same way — never panicked over. Anything
/// longer than [`MAX_CMD_LEN`] bytes is truncated to the first
/// [`MAX_CMD_LEN`] bytes at a character boundary.
pub(crate) fn decode_command(cmd: Option<&str>, enc: Option<&str>) -> Option<String> {
    let raw = cmd?;
    let bytes = match enc {
        Some("b64") => decode_base64(raw)?,
        // `raw`, missing, or unknown: a literal. Lenient on purpose — the
        // text is still better than the scrape when it is present.
        _ => raw.as_bytes().to_vec(),
    };
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if text.len() > MAX_CMD_LEN {
        let mut end = MAX_CMD_LEN;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    Some(text)
}

/// Decodes standard-alphabet base64 (what the `base64` CLI emits) without
/// trusting the input: any whitespace, control, non-alphabet byte, or
/// misplaced padding returns `None` rather than panicking.
fn decode_base64(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() {
        return Some(Vec::new());
    }
    if !s.len().is_multiple_of(4) {
        return None;
    }
    let bytes = s.as_bytes();
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    // Padding (`=`, at most two) may only close the final quantum: once it
    // starts, only more padding may follow.
    let mut pad = 0usize;
    for &c in bytes {
        if c == b'=' {
            pad += 1;
        } else if pad > 0 || val(c).is_none() {
            return None;
        }
    }
    if pad > 2 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    // Exact: the length check above leaves no remainder.
    for quad in bytes.as_chunks::<4>().0 {
        let mut n: u32 = 0;
        for &c in quad {
            n <<= 6;
            if c != b'=' {
                n |= u32::from(val(c)?);
            }
        }
        out.push((n >> 16) as u8);
        if quad[2] != b'=' {
            out.push((n >> 8) as u8);
        }
        if quad[3] != b'=' {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// Parses one complete OSC sequence. Returns the kind, exit status and (for
/// `C`) decoded command text when it is an OSC 133 marker whose `k=` equals
/// `nonce`; anything else — another OSC number, a missing or mismatched
/// `k=` — returns `None` and the caller passes the bytes through untouched.
///
/// A `C` whose payload is missing or rejected still returns its kind: the
/// boundary is valid, only the command text falls back to the scrape.
fn parse_133(raw: &[u8], nonce: &str) -> Option<(MarkKind, Option<i32>, Option<String>)> {
    let mut body = raw.strip_prefix(&[ESC, OSC_INTRO])?;
    if body.last() == Some(&BEL) {
        body = &body[..body.len() - 1];
    } else if body.len() >= 2 && body[body.len() - 2] == ESC && body[body.len() - 1] == ST_FINAL {
        body = &body[..body.len() - 2];
    } else {
        return None;
    }
    let params: Vec<&[u8]> = body.split(|&b| b == b';').collect();
    if params.first() != Some(&b"133".as_slice()) {
        return None;
    }
    let kind = params[1..]
        .iter()
        .filter_map(|p| if p.len() == 1 { Some(p[0]) } else { None })
        .find_map(|b| match b {
            b'A' => Some(MarkKind::PromptStart),
            b'B' => Some(MarkKind::PromptEnd),
            b'C' => Some(MarkKind::OutputStart),
            b'D' => Some(MarkKind::CommandDone),
            _ => None,
        })?;
    if nonce.is_empty() {
        return None;
    }
    let keyed = params.iter().filter_map(|p| p.strip_prefix(b"k=")).any(|k| k == nonce.as_bytes());
    if !keyed {
        return None;
    }
    let exit = (kind == MarkKind::CommandDone).then(|| {
        params[1..]
            .iter()
            // Skip the kind letter (a single ASCII letter), the nonce and
            // the command payload — but keep single-digit statuses: length
            // alone cannot tell `D` from `3` (and a raw `cmd=123` is text,
            // not a status).
            .filter(|p| {
                !(p.len() == 1 && p[0].is_ascii_alphabetic())
                    && p.strip_prefix(b"k=").is_none()
                    && p.strip_prefix(b"cmd=").is_none()
                    && p.strip_prefix(b"enc=").is_none()
            })
            .filter_map(|p| std::str::from_utf8(p).ok()?.trim().parse::<i32>().ok())
            .next()
            .unwrap_or(0)
    });
    let command = (kind == MarkKind::OutputStart).then(|| {
        let cmd = params
            .iter()
            .find_map(|p| p.strip_prefix(b"cmd="))
            .and_then(|c| std::str::from_utf8(c).ok());
        let enc = params
            .iter()
            .find_map(|p| p.strip_prefix(b"enc="))
            .and_then(|e| std::str::from_utf8(e).ok());
        decode_command(cmd, enc)
    });
    Some((kind, exit, command.flatten()))
}

/// Offset of a trailing incomplete introducer in `tail`, if it ends with one:
/// a lone `ESC`, `ESC ]` or `ESC [` possibly followed by parameter bytes but
/// no final byte yet.
fn trailing_partial(tail: &[u8]) -> Option<usize> {
    let esc = tail.iter().rposition(|&b| b == ESC)?;
    let after = &tail[esc + 1..];
    if after.is_empty() {
        return Some(esc);
    }
    if after[0] != b']' && after[0] != b'[' {
        return None;
    }
    let complete = after[1..].iter().any(|b| (0x40..=0x7E).contains(b));
    (!complete).then_some(esc)
}

/// A block under construction while [`assemble_blocks`] walks the marks.
struct Open {
    /// The `A` line (or the `C` line when no `A` came).
    a_line: i32,
    /// When the opening mark arrived.
    a_at: Instant,
    /// The `B` line, once seen.
    b_line: Option<i32>,
    /// The `C` line, once seen.
    c_line: Option<i32>,
    /// The command text the `C` marker carried, once seen.
    cmd: Option<String>,
}

/// The block's command text: the `C` payload when the emitter sent one,
/// else the grid scrape between `B` and `C`.
fn block_command(open: &Open, command_text: &dyn Fn(i32, i32) -> String) -> String {
    if let Some(cmd) = open.cmd.as_deref() {
        return cmd.to_string();
    }
    let b = open.b_line.unwrap_or(open.a_line);
    let c = open.c_line.unwrap_or(b);
    command_text(b, c)
}

/// Closes `open` at the `D` mark (`line`, `at`, `exit`), reading the command
/// text off `command_text` unless the `C` marker carried a payload.
fn close(open: &Open, line: i32, at: Instant, exit: i32, command_text: &dyn Fn(i32, i32) -> String) -> Block {
    let b = open.b_line.unwrap_or(open.a_line);
    let c = open.c_line.unwrap_or(b);
    Block {
        prompt: (open.a_line, b),
        command_line: (b, c),
        output: (c, line),
        end: line,
        exit: Some(exit),
        command: block_command(open, command_text),
        started: open.a_at,
        ended: Some(at),
        author: BlockAuthor::Human,
        // Placeholder: the session's rebuild assigns the real monotonic id.
        id: 0,
    }
}

/// Folds `marks` (stream order) into blocks, oldest first.
///
/// `tail` is the absolute line of the live cursor: a block with a `C` but no
/// `D` yet stays open with `output` running to it and `exit`/`ended` left as
/// `None`. `command_text(a, b)` supplies the trimmed grid text of absolute
/// lines `a..b` for the echoed command — but only as a fallback: a block
/// whose `C` marker carried a command payload (see [`Mark::command`]) reads
/// it from there instead. A prompt abandoned without a `C` (no command ran)
/// produces no block. A `D` with no `C` reports the command with an empty
/// output, mirroring `BlockParser`; an `A` that arrives while a block is
/// still running closes it as exit 0, exactly as the prompt-first fallback
/// in `BlockParser` does.
///
/// A nonced `B` from an OSC 133 emitter that is not ours is still honoured
/// for the fallback scrape — but our own snippets no longer emit one (see
/// `pty.rs`): the payload above replaced it. The existing guards stay: a
/// `B` that arrives after the block's `C` (or while already reading the
/// command line) is ignored rather than rewinding the phase.
pub fn assemble_blocks(
    marks: &[Mark],
    tail: i32,
    command_text: &dyn Fn(i32, i32) -> String,
) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut open: Option<Open> = None;
    for mark in marks {
        match mark.kind {
            MarkKind::PromptStart => {
                if let Some(cur) = open.take() {
                    if cur.c_line.is_some() {
                        blocks.push(close(&cur, mark.line, mark.at, 0, command_text));
                    }
                    // Without a `C` nothing ran: the prompt line is dropped.
                }
                open = Some(Open {
                    a_line: mark.line,
                    a_at: mark.at,
                    b_line: None,
                    c_line: None,
                    cmd: None,
                });
            }
            MarkKind::PromptEnd => {
                if let Some(cur) = open.as_mut() {
                    if cur.c_line.is_none() {
                        cur.b_line = Some(mark.line);
                    }
                }
            }
            MarkKind::OutputStart => {
                let cur = open.get_or_insert(Open {
                    a_line: mark.line,
                    a_at: mark.at,
                    b_line: None,
                    c_line: None,
                    cmd: None,
                });
                if cur.c_line.is_none() {
                    cur.c_line = Some(mark.line);
                    cur.cmd = mark.command.clone();
                }
            }
            MarkKind::CommandDone => {
                let exit = mark.exit.unwrap_or(0);
                let cur = open.take().unwrap_or(Open {
                    a_line: mark.line,
                    a_at: mark.at,
                    b_line: Some(mark.line),
                    c_line: Some(mark.line),
                    cmd: None,
                });
                blocks.push(close(&cur, mark.line, mark.at, exit, command_text));
            }
        }
    }
    if let Some(cur) = open {
        if let Some(c) = cur.c_line {
            let b = cur.b_line.unwrap_or(cur.a_line);
            blocks.push(Block {
                prompt: (cur.a_line, b),
                command_line: (b, c),
                output: (c, tail),
                end: tail,
                exit: None,
                command: block_command(&cur, command_text),
                started: cur.a_at,
                ended: None,
                author: BlockAuthor::Human,
                // Placeholder: the session's rebuild assigns the real monotonic id.
                id: 0,
            });
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scanner pinned to a fixed nonce.
    fn scanner() -> MarkScanner {
        MarkScanner::new("n9_abc-XY")
    }

    fn feed(s: &mut MarkScanner, bytes: &[u8]) -> Vec<ScanSegment> {
        s.split_feed(bytes)
    }

    /// Only the accepted markers in a segment list.
    fn markers_in(segments: &[ScanSegment]) -> Vec<(MarkKind, Option<i32>)> {
        segments
            .iter()
            .filter_map(|s| match s {
                ScanSegment::Marker { kind, exit, .. } => Some((*kind, *exit)),
                ScanSegment::Emit(_) => None,
            })
            .collect()
    }

    /// The raw bytes round-trip: emits plus marker raws reproduce the input.
    fn round_trip(segments: &[ScanSegment]) -> Vec<u8> {
        let mut out = Vec::new();
        for s in segments {
            match s {
                ScanSegment::Emit(raw) => out.extend_from_slice(raw),
                ScanSegment::Marker { raw, .. } => out.extend_from_slice(raw),
            }
        }
        out
    }

    #[test]
    fn nonce_checked_markers_are_accepted_with_kind_and_exit() {
        let mut s = scanner();
        let bytes = b"\x1b]133;A;k=n9_abc-XY\x07prompt\x1b]133;B;k=n9_abc-XY\x07echo hi\x1b]133;C;k=n9_abc-XY\x07hi\n\x1b]133;D;3;k=n9_abc-XY\x07";
        let segments = feed(&mut s, bytes);
        assert_eq!(
            markers_in(&segments),
            vec![
                (MarkKind::PromptStart, None),
                (MarkKind::PromptEnd, None),
                (MarkKind::OutputStart, None),
                (MarkKind::CommandDone, Some(3)),
            ]
        );
        assert_eq!(round_trip(&segments), bytes);
    }

    #[test]
    fn a_wrong_nonce_and_a_missing_nonce_are_ignored_entirely() {
        let mut s = scanner();
        let bytes = b"\x1b]133;A;k=wrong\x07\x1b]133;B\x07\x1b]133;C;k=someone-else\x07output\n\x1b]133;D;0\x07";
        let segments = feed(&mut s, bytes);
        assert!(markers_in(&segments).is_empty(), "forged markers accepted: {segments:?}");
        assert!(s.marks().is_empty());
        // The bytes still pass through: the screen must show them.
        assert_eq!(round_trip(&segments), bytes);
    }

    #[test]
    fn an_empty_nonce_matches_nothing() {
        let mut s = MarkScanner::new(String::new());
        let segments = feed(&mut s, b"\x1b]133;A;k=\x07\x1b]133;C;k=anything\x07");
        assert!(markers_in(&segments).is_empty());
    }

    #[test]
    fn other_osc_sequences_pass_through_untouched() {
        let mut s = scanner();
        // Window title and hyperlink OSCs are not markers.
        let bytes = b"\x1b]0;my title\x07text\x1b]8;;https://example.com\x07link\x1b]8;;\x07";
        let segments = feed(&mut s, bytes);
        assert!(markers_in(&segments).is_empty());
        assert_eq!(round_trip(&segments), bytes);
    }

    #[test]
    fn a_marker_split_across_chunks_is_reported_once() {
        let mut s = scanner();
        let first = feed(&mut s, b"echo\x1b]133;C;k=n9_");
        assert!(markers_in(&first).is_empty(), "partial marker reported early");
        let second = feed(&mut s, b"abc-XY\x07hi\n");
        assert_eq!(markers_in(&second), vec![(MarkKind::OutputStart, None)]);
        assert_eq!([round_trip(&first), round_trip(&second)].concat(), b"echo\x1b]133;C;k=n9_abc-XY\x07hi\n");
    }

    #[test]
    fn a_split_introducer_is_held_for_the_next_chunk() {
        let mut s = scanner();
        let first = feed(&mut s, b"cmd\x1b");
        assert_eq!(round_trip(&first), b"cmd");
        let second = feed(&mut s, b"]133;A;k=n9_abc-XY\x07");
        assert_eq!(markers_in(&second), vec![(MarkKind::PromptStart, None)]);
    }

    #[test]
    fn st_terminated_markers_are_accepted() {
        let mut s = scanner();
        let bytes = b"\x1b]133;D;1;k=n9_abc-XY\x1b\\";
        let segments = feed(&mut s, bytes);
        assert_eq!(markers_in(&segments), vec![(MarkKind::CommandDone, Some(1))]);
    }

    /// Marks at fixed lines and times, for assembly tests.
    fn mark(line: i32, kind: MarkKind, exit: Option<i32>) -> Mark {
        Mark { line, kind, exit, at: Instant::now(), command: None }
    }

    /// Marks at fixed lines and times carrying a `C` command payload.
    fn mark_cmd(line: i32, kind: MarkKind, exit: Option<i32>, command: &str) -> Mark {
        Mark { line, kind, exit, at: Instant::now(), command: Some(command.to_string()) }
    }

    fn no_text(_a: i32, _b: i32) -> String {
        String::new()
    }

    #[test]
    fn consecutive_marks_assemble_into_ranged_blocks() {
        let marks = vec![
            mark(10, MarkKind::PromptStart, None),
            mark(10, MarkKind::PromptEnd, None),
            mark(11, MarkKind::OutputStart, None),
            mark(14, MarkKind::CommandDone, Some(1)),
            mark(15, MarkKind::PromptStart, None),
            mark(15, MarkKind::PromptEnd, None),
            mark(16, MarkKind::OutputStart, None),
        ];
        let blocks = assemble_blocks(&marks, 20, &no_text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].prompt, (10, 10));
        assert_eq!(blocks[0].command_line, (10, 11));
        assert_eq!(blocks[0].output, (11, 14));
        assert_eq!(blocks[0].end, 14);
        assert_eq!(blocks[0].exit, Some(1));
        assert!(blocks[0].ended.is_some());
        assert!(!blocks[0].running());
        // A `C` with no `D` yet is still running: exit and ended are None.
        assert_eq!(blocks[1].output, (16, 20));
        assert_eq!(blocks[1].end, 20);
        assert_eq!(blocks[1].exit, None);
        assert_eq!(blocks[1].ended, None);
        assert!(blocks[1].running());
    }

    #[test]
    fn a_prompt_abandoned_without_output_produces_no_block() {
        let marks =
            vec![mark(3, MarkKind::PromptStart, None), mark(3, MarkKind::PromptEnd, None), mark(5, MarkKind::PromptStart, None)];
        assert!(assemble_blocks(&marks, 9, &no_text).is_empty());
    }

    #[test]
    fn a_done_without_output_reports_an_empty_command() {
        let marks = vec![mark(7, MarkKind::CommandDone, Some(0))];
        let blocks = assemble_blocks(&marks, 7, &no_text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].exit, Some(0));
        assert_eq!(blocks[0].output, (7, 7));
    }

    #[test]
    fn an_a_while_a_block_runs_closes_it_as_exit_zero() {
        let marks = vec![
            mark(2, MarkKind::PromptStart, None),
            mark(2, MarkKind::PromptEnd, None),
            mark(4, MarkKind::OutputStart, None),
            mark(9, MarkKind::PromptStart, None),
        ];
        let blocks = assemble_blocks(&marks, 12, &no_text);
        assert_eq!(blocks.len(), 1, "the running block closes, the bare prompt makes none");
        assert_eq!(blocks[0].exit, Some(0));
        assert_eq!(blocks[0].end, 9);
        assert_eq!(blocks[0].output, (4, 9));
    }

    #[test]
    fn command_text_comes_from_between_b_and_c() {
        let marks = vec![
            mark(2, MarkKind::PromptStart, None),
            mark(2, MarkKind::PromptEnd, None),
            mark(4, MarkKind::OutputStart, None),
            mark(6, MarkKind::CommandDone, Some(0)),
        ];
        let blocks = assemble_blocks(&marks, 6, &|a, b| format!("lines {a}..{b}"));
        assert_eq!(blocks[0].command, "lines 2..4");
    }

    /// The `C` payload beats the scrape: with no `B` at all (what our own
    /// snippets emit now) the block still carries the exact command.
    #[test]
    fn a_c_payload_is_preferred_over_the_grid_scrape() {
        let marks = vec![
            mark(2, MarkKind::PromptStart, None),
            mark_cmd(4, MarkKind::OutputStart, None, "echo exact; echo semi"),
            mark(6, MarkKind::CommandDone, Some(0)),
        ];
        let blocks = assemble_blocks(&marks, 6, &|a, b| format!("lines {a}..{b}"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].command, "echo exact; echo semi");
    }

    /// …and the running block (a `C` with no `D` yet) reads it too.
    #[test]
    fn a_running_block_reads_the_c_payload() {
        let marks =
            vec![mark(2, MarkKind::PromptStart, None), mark_cmd(4, MarkKind::OutputStart, None, "sleep 9")];
        let blocks = assemble_blocks(&marks, 10, &|_, _| "scrape".to_string());
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].running());
        assert_eq!(blocks[0].command, "sleep 9");
    }

    /// A `C` with no payload (a foreign emitter) still falls back to the
    /// scrape between `B` and `C`.
    #[test]
    fn a_c_without_payload_falls_back_to_the_scrape() {
        let marks = vec![
            mark(2, MarkKind::PromptStart, None),
            mark(2, MarkKind::PromptEnd, None),
            mark(4, MarkKind::OutputStart, None),
            mark(6, MarkKind::CommandDone, Some(0)),
        ];
        let blocks = assemble_blocks(&marks, 6, &|a, b| format!("lines {a}..{b}"));
        assert_eq!(blocks[0].command, "lines 2..4");
    }

    /// The full payload round trip through the scanner: base64 in, exact
    /// text (semicolon, newline, quote, emoji) on the mark.
    #[test]
    fn a_c_payload_survives_the_scanner() {
        let mut s = scanner();
        // `printf 'a;b\n"x" ✓' | base64`
        let cmd = "YTtiCiJ4IiDinJM=";
        let bytes = format!("\x1b]133;C;k=n9_abc-XY;cmd={cmd};enc=b64\x07");
        let segments = feed(&mut s, bytes.as_bytes());
        let cmds: Vec<_> = segments
            .iter()
            .filter_map(|seg| match seg {
                ScanSegment::Marker { command, .. } => Some(command.clone()),
                ScanSegment::Emit(_) => None,
            })
            .collect();
        assert_eq!(cmds, vec![Some("a;b\n\"x\" ✓".to_string())]);
        assert_eq!(round_trip(&segments), bytes.as_bytes());
    }

    /// A `C` with no `cmd=` is still a boundary — only the command text is
    /// absent, and the block falls back to the scrape.
    #[test]
    fn a_c_without_cmd_is_still_a_boundary() {
        let mut s = scanner();
        let bytes = b"\x1b]133;C;k=n9_abc-XY\x07";
        let segments = feed(&mut s, bytes);
        assert_eq!(markers_in(&segments), vec![(MarkKind::OutputStart, None)]);
        let cmds: Vec<_> = segments
            .iter()
            .filter_map(|seg| match seg {
                ScanSegment::Marker { command, .. } => Some(command.clone()),
                ScanSegment::Emit(_) => None,
            })
            .collect();
        assert_eq!(cmds, vec![None]);
    }

    /// Invalid base64 is rejected, not panicked over — and the boundary
    /// still lands, with the command falling back to the scrape.
    #[test]
    fn an_invalid_base64_payload_is_rejected_not_panicked_over() {
        let mut s = scanner();
        for bad in ["!!!not-b64!!!", "abc", "AB=C", "A==="] {
            let bytes = format!("\x1b]133;C;k=n9_abc-XY;cmd={bad};enc=b64\x07");
            let segments = feed(&mut s, bytes.as_bytes());
            assert_eq!(markers_in(&segments), vec![(MarkKind::OutputStart, None)], "for {bad:?}");
            let cmds: Vec<_> = segments
                .iter()
                .filter_map(|seg| match seg {
                    ScanSegment::Marker { command, .. } => Some(command.clone()),
                    ScanSegment::Emit(_) => None,
                })
                .collect();
            assert_eq!(cmds, vec![None], "for {bad:?}");
        }
    }

    /// A raw (`enc=raw`) payload is taken literally — the fallback our
    /// snippets use when `base64` is missing from `PATH`.
    #[test]
    fn a_raw_payload_is_taken_literally() {
        let mut s = scanner();
        let bytes = b"\x1b]133;C;k=n9_abc-XY;cmd=echo plain;enc=raw\x07";
        let segments = feed(&mut s, bytes);
        let cmds: Vec<_> = segments
            .iter()
            .filter_map(|seg| match seg {
                ScanSegment::Marker { command, .. } => Some(command.clone()),
                ScanSegment::Emit(_) => None,
            })
            .collect();
        assert_eq!(cmds, vec![Some("echo plain".to_string())]);
    }

    /// A raw payload that looks numeric is command text, not an exit status:
    /// `D` parsing must skip `cmd=`/`enc=` or a `cmd=123` would read as one.
    #[test]
    fn a_numeric_looking_payload_is_not_an_exit_status() {
        let mut s = scanner();
        let bytes = b"\x1b]133;D;1;k=n9_abc-XY;cmd=123;enc=raw\x07";
        let segments = feed(&mut s, bytes);
        assert_eq!(markers_in(&segments), vec![(MarkKind::CommandDone, Some(1))]);
    }

    /// The decoder caps the payload at 4 KB (first bytes, char boundary):
    /// a hostile emitter cannot grow blocks without limit.
    #[test]
    fn an_oversized_payload_is_capped_at_4_kb() {
        // 5000 `x`s: 1666 full `xxx` quanta (`eHh4` each) plus a final
        // 2-byte `xx` quantum (`eHg=`), 6668 chars total. The block keeps
        // the first 4096 bytes.
        let long = "x".repeat(5000);
        let b64 = "eHh4".repeat(1666) + "eHg=";
        assert_eq!(b64.len(), 6668);
        let decoded = decode_command(Some(&b64), Some("b64")).expect("decodes");
        assert_eq!(decoded.len(), MAX_CMD_LEN);
        assert_eq!(decoded, long[..MAX_CMD_LEN]);
    }
}
