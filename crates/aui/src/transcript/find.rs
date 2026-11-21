//! Find in a transcript (docs/82 7.5): the matching a host runs over its
//! markdown views, and the marks it hands back for them to paint.
//!
//! The host owns the search. [`markdown_find`] walks a message exactly as
//! [`Markdown`](super::Markdown) draws it — the same settled-aware blocks,
//! the same cells, the same shaped text (link labels, not URLs; code spans
//! without their backticks) — so a count never includes text nobody can see,
//! and every range it returns is a byte range over a cell the view paints.
//! A fenced block folds past [`CODE_FOLD_AT`](super::code::CODE_FOLD_AT)
//! lines, so only its first lines count: the ones drawn folded.
//!
//! The host turns its matches into [`FindMark`]s per view and passes them to
//! [`Markdown::find_marks`](super::Markdown::find_marks) (or the turn
//! builders' own `find_marks`); the view paints every mark in the find tint
//! and the current one stronger ([`find_mark_colors`]). Nothing is searched
//! at paint time.

use std::ops::Range;

use gpui::Hsla;

use super::markdown::{blocks_for_find, slice_cells};
use super::selectable::SelectionKey;

/// One match painted in a markdown view: its cell, the byte range over the
/// cell's shaped text, and whether it is the current match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMark {
    /// The cell holding the match.
    pub cell: SelectionKey,
    /// Byte range over the cell's shaped text.
    pub range: Range<usize>,
    /// The match the find bar's "3 of 12" points at.
    pub current: bool,
}

/// The tints a find paints: every match, then the current one.
pub fn find_mark_colors(palette: &aui_tokens::Palette) -> (Hsla, Hsla) {
    (palette.warning_soft, palette.warning.alpha(palette.warning.a * FIND_CURRENT_ALPHA))
}

/// The current match's tint: the warning hue at this share of its alpha,
/// strong enough to stand out from the other marks with the ink still
/// readable over it in both themes.
const FIND_CURRENT_ALPHA: f32 = 0.5;

/// Every non-overlapping occurrence of `needle` in `hay`, left to right, as
/// byte ranges on char boundaries. Case-insensitive unless `match_case`
/// (Unicode lowercase folding, an ASCII fast path). An empty needle finds
/// nothing.
pub fn find_ranges(hay: &str, needle: &str, match_case: bool) -> Vec<Range<usize>> {
    if needle.is_empty() || hay.is_empty() {
        return Vec::new();
    }
    if match_case {
        return hay.match_indices(needle).map(|(at, m)| at..at + m.len()).collect();
    }
    if hay.is_ascii() && needle.is_ascii() {
        let (h, n) = (hay.as_bytes(), needle.as_bytes());
        let mut out = Vec::new();
        let mut at = 0;
        while at + n.len() <= h.len() {
            if h[at..at + n.len()].eq_ignore_ascii_case(n) {
                out.push(at..at + n.len());
                at += n.len();
            } else {
                at += 1;
            }
        }
        return out;
    }
    let want: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    let mut out = Vec::new();
    let mut resume = 0;
    for (start, _) in hay.char_indices() {
        if start < resume {
            continue;
        }
        let mut k = 0;
        let mut end = None;
        'chars: for (offset, c) in hay[start..].char_indices() {
            for lower in c.to_lowercase() {
                if k < want.len() && want[k] == lower {
                    k += 1;
                } else {
                    break 'chars;
                }
            }
            if k == want.len() {
                end = Some(start + offset + c.len_utf8());
                break;
            }
        }
        if let Some(end) = end {
            out.push(start..end);
            resume = end;
        }
    }
    out
}

/// One match [`markdown_find`] found: its cell, the byte range over the
/// cell's shaped text, and how far through the message's drawn text it
/// starts (0 at the top, towards 1 at the end) — what a host scrolls by
/// inside a message taller than its viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownMatch {
    /// The cell holding the match.
    pub cell: SelectionKey,
    /// Byte range over the cell's shaped text.
    pub range: Range<usize>,
    /// The match's start as a share of the message's drawn text, 0–1.
    pub position: f32,
}

/// Every match of `needle` in the markdown `source`, in document order:
/// what [`Markdown`](super::Markdown) draws for `source` at `settled`. Pure
/// and thread-safe (no memo), so a host may run it off the UI thread.
pub fn markdown_find(source: &str, settled: bool, needle: &str, match_case: bool) -> Vec<MarkdownMatch> {
    if needle.is_empty() {
        return Vec::new();
    }
    let blocks = blocks_for_find(source, settled);
    let cells = slice_cells(&blocks);
    let total: usize = cells.iter().map(|(_, text, code)| if *code { folded_prefix(text).len() } else { text.len() }).sum::<usize>().max(1);
    let mut before = 0usize;
    let mut out = Vec::new();
    for (key, text, code) in &cells {
        let shown = if *code { folded_prefix(text) } else { text.as_str() };
        for range in find_ranges(shown, needle, match_case) {
            let position = (before + range.start) as f32 / total as f32;
            out.push(MarkdownMatch { cell: key.clone(), range, position });
        }
        before += shown.len();
    }
    out
}

/// What a folding fence draws before "N more lines": its first
/// [`CODE_FOLD_AT`](super::code::CODE_FOLD_AT) lines (the whole text when
/// it is that short or long enough to virtualise instead).
fn folded_prefix(text: &str) -> &str {
    use super::code::{CODE_FOLD_AT, CODE_VIRTUALIZE_AT};
    let total = text.lines().count();
    if total <= CODE_FOLD_AT || total > CODE_VIRTUALIZE_AT {
        return text;
    }
    let mut end = 0;
    for line in text.split_inclusive('\n').take(CODE_FOLD_AT) {
        end += line.len();
    }
    text[..end].trim_end_matches('\n')
}

/// The marks of `cell` among `marks`, in its local coordinates, tinted.
pub(crate) fn cell_marks(marks: &[FindMark], cell: &SelectionKey, colors: (Hsla, Hsla)) -> Vec<(Range<usize>, Hsla)> {
    marks
        .iter()
        .filter(|mark| mark.cell == *cell)
        .map(|mark| (mark.range.clone(), if mark.current { colors.1 } else { colors.0 }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_fold_case_unless_asked_and_never_overlap() {
        assert_eq!(find_ranges("Foo foo FOO", "foo", false), vec![0..3, 4..7, 8..11]);
        assert_eq!(find_ranges("Foo foo FOO", "foo", true), vec![4..7]);
        assert_eq!(find_ranges("aaaa", "aa", false), vec![0..2, 2..4]);
        assert!(find_ranges("abc", "", false).is_empty());
        assert!(find_ranges("", "a", false).is_empty());
        // Non-ASCII: byte ranges on char boundaries.
        let hay = "Ünïcode ünïCODE";
        let found = find_ranges(hay, "ÜNÏ", false);
        assert_eq!(found.len(), 2);
        for r in &found {
            assert!(hay.is_char_boundary(r.start) && hay.is_char_boundary(r.end));
            assert_eq!(hay[r.clone()].to_lowercase(), "ünï");
        }
    }

    #[test]
    fn markdown_find_counts_drawn_text_only() {
        // Link URLs and markup are not drawn; the label and words are.
        let source = "# Title word\n\nSee [the word](https://word.example) and `word` and **word**.\n\n- a word\n- none";
        let found = markdown_find(source, true, "word", false);
        let cells: Vec<&str> = found.iter().map(|m| m.cell.as_str()).collect();
        assert_eq!(cells, ["h0", "p1", "p1", "p1", "b2-0"]);
        // Every range addresses the shaped text of its cell.
        assert!(found.iter().all(|m| m.range.len() == 4));
        // Positions run down the message.
        assert!(found.windows(2).all(|w| w[0].position <= w[1].position));
        assert!(found[0].position < 0.2 && found[4].position > 0.6);
    }

    #[test]
    fn a_folding_fence_counts_its_first_lines_only() {
        let mut code = String::from("```\n");
        for i in 0..20 {
            code.push_str(&format!("line {i} needle\n"));
        }
        code.push_str("```\n");
        let found = markdown_find(&code, true, "needle", false);
        assert_eq!(found.len(), super::super::code::CODE_FOLD_AT);
        // A short fence counts everything.
        let found = markdown_find("```\nneedle\nneedle\n```", true, "needle", false);
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].range, 7..13, "byte offsets run over the whole block text");
    }

    #[test]
    fn cell_marks_tint_the_current_match_apart() {
        let key = SelectionKey::paragraph("", 0);
        let other = SelectionKey::paragraph("", 1);
        let marks = vec![
            FindMark { cell: key.clone(), range: 0..2, current: false },
            FindMark { cell: key.clone(), range: 4..6, current: true },
            FindMark { cell: other, range: 0..1, current: false },
        ];
        let colors = (gpui::red(), gpui::blue());
        assert_eq!(cell_marks(&marks, &key, colors), vec![(0..2, gpui::red()), (4..6, gpui::blue())]);
    }
}
