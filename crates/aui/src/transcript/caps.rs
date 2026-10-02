//! One line-and-character cap shared by every wrapping transcript body.
//!
//! Counting `str::lines` bounds newlines only: a 26 000-character body with
//! no newlines, a minified single-line MCP result, or a search hit with a
//! huge snippet all sailed through the line caps and rendered in full,
//! wrapping. Every cap here binds on both axes, so prepared text is bounded
//! no matter how few newlines it holds. Pretty-print-free: JSON is cut, never
//! reformatted.

/// Collapsed preview shows at most this many lines...
pub const PREVIEW_MAX_LINES: usize = 3;
/// ...and at most this many characters in total.
pub const PREVIEW_MAX_CHARS: usize = 360;
/// Each preview line is cut here, with an ellipsis marking the cut.
pub const PREVIEW_LINE_CHARS: usize = 160;
/// An expanded body shows at most this many lines...
pub const EXPANDED_MAX_LINES: usize = 40;
/// ...and at most this many characters.
pub const EXPANDED_MAX_CHARS: usize = 6000;
/// Each expanded line is cut here too, with an ellipsis marking the cut: a
/// minified single-line payload must not render whole no matter the totals.
/// Generous on purpose: an opened body is for reading, so a long prose
/// paragraph (a skill's text) wraps whole; only a pathological line is cut.
pub const EXPANDED_LINE_CHARS: usize = 2000;

/// What a cap cut away: whole lines never shown, and characters never shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Remainder {
    /// Lines past the line cap (0 when the character cap bound first).
    pub lines: usize,
    /// Characters past the character cap (0 when only whole lines were cut).
    pub chars: usize,
}

impl Remainder {
    /// Nothing was cut.
    pub fn is_empty(&self) -> bool {
        self.lines == 0 && self.chars == 0
    }

    /// The fold-row label naming what was cut: `N more lines` when only
    /// whole lines went, otherwise `M more characters` — e.g.
    /// `12,400 more characters`. `None` when nothing was cut.
    pub fn label(&self) -> Option<String> {
        if self.chars > 0 {
            Some(format!("{} more characters", format_count(self.chars)))
        } else if self.lines > 0 {
            Some(format!("{} more lines", self.lines))
        } else {
            None
        }
    }
}

/// `12400` reads `12,400`.
fn format_count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Byte index of the `max_chars`-th character boundary: cutting there never
/// splits a multi-byte character.
fn char_cut(text: &str, max_chars: usize) -> usize {
    text.char_indices().map(|(i, _)| i).nth(max_chars).unwrap_or(text.len())
}

/// One line cut at `max` characters, with an ellipsis marking the cut.
/// Short lines come back unchanged.
fn cut_line(line: &str, max: usize) -> String {
    if line.chars().count() > max {
        let mut cut: String = line.chars().take(max).collect();
        cut.push('…');
        cut
    } else {
        line.to_string()
    }
}

/// One line cut at [`PREVIEW_LINE_CHARS`], with an ellipsis marking the cut.
/// Short lines come back unchanged.
pub fn cut_preview_line(line: &str) -> String {
    cut_line(line, PREVIEW_LINE_CHARS)
}

/// The collapsed preview: at most [`PREVIEW_MAX_LINES`] lines and
/// [`PREVIEW_MAX_CHARS`] characters in total, each long line cut at
/// [`PREVIEW_LINE_CHARS`] with an ellipsis. Empty text previews to nothing.
pub fn cap_preview(text: &str) -> String {
    let mut out =
        text.lines().take(PREVIEW_MAX_LINES).map(cut_preview_line).collect::<Vec<_>>().join("\n");
    if out.chars().count() > PREVIEW_MAX_CHARS {
        out.truncate(char_cut(&out, PREVIEW_MAX_CHARS));
    }
    out
}

/// The expanded body: at most [`EXPANDED_MAX_LINES`] lines, each cut at
/// [`EXPANDED_LINE_CHARS`], and [`EXPANDED_MAX_CHARS`] characters in total,
/// with the [`Remainder`] naming what was cut. The remainder counts whole
/// lines dropped by the line cap separately from characters cut short
/// inside shown lines, so short bodies keep the `N more lines` row while a
/// minified single line reports `M more characters`. Short text hides
/// nothing.
pub fn cap_expanded(text: &str) -> (String, Remainder) {
    let total_lines = text.lines().count();
    let shown_raw: Vec<&str> = text.lines().take(EXPANDED_MAX_LINES).collect();
    let raw_chars: usize = shown_raw.iter().map(|line| line.chars().count()).sum();
    let mut visible = shown_raw
        .iter()
        .map(|line| cut_line(line, EXPANDED_LINE_CHARS))
        .collect::<Vec<_>>()
        .join("\n");
    let mut cut_chars = raw_chars.saturating_sub(visible.chars().count());
    if visible.chars().count() > EXPANDED_MAX_CHARS {
        let before = visible.chars().count();
        visible.truncate(char_cut(&visible, EXPANDED_MAX_CHARS));
        cut_chars += before - visible.chars().count();
    }
    let remainder = Remainder {
        lines: total_lines.saturating_sub(visible.lines().count()),
        chars: cut_chars,
    };
    (visible, remainder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_group_by_thousands() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_000), "1,000");
        assert_eq!(format_count(12_400), "12,400");
        assert_eq!(format_count(20_000), "20,000");
    }

    #[test]
    fn the_label_names_lines_or_characters() {
        assert_eq!(Remainder { lines: 0, chars: 0 }.label(), None);
        assert_eq!(
            Remainder { lines: 81, chars: 0 }.label().as_deref(),
            Some("81 more lines")
        );
        assert_eq!(
            Remainder { lines: 0, chars: 12_400 }.label().as_deref(),
            Some("12,400 more characters")
        );
        // Both cut: the character count subsumes the line count.
        assert_eq!(
            Remainder { lines: 114, chars: 115_000 }.label().as_deref(),
            Some("115,000 more characters")
        );
    }

    #[test]
    fn preview_lines_cut_at_160_chars_with_an_ellipsis() {
        let line = "x".repeat(1_000);
        let cut = cut_preview_line(&line);
        assert_eq!(cut.chars().count(), PREVIEW_LINE_CHARS + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(cut_preview_line("short"), "short");
        assert_eq!(cut_preview_line(&"y".repeat(PREVIEW_LINE_CHARS)).chars().count(), PREVIEW_LINE_CHARS);
    }

    #[test]
    fn preview_binds_lines_and_characters() {
        assert_eq!(cap_preview(""), "");
        assert_eq!(cap_preview("one"), "one");
        // Three 1,000-char lines: each line is cut, then the total is cut.
        let three = (0..3).map(|_| "z".repeat(1_000)).collect::<Vec<_>>().join("\n");
        let preview = cap_preview(&three);
        assert!(
            preview.chars().count() <= PREVIEW_MAX_CHARS,
            "three huge preview lines total {}, over the {PREVIEW_MAX_CHARS} cap",
            preview.chars().count()
        );
        // Few newlines never escape the cap either.
        let single = "w".repeat(26_000);
        assert!(cap_preview(&single).chars().count() <= PREVIEW_MAX_CHARS);
    }

    #[test]
    fn expanded_binds_lines_and_characters() {
        // Many short lines: the line cap binds, characters report nothing.
        let many = (1..=121).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
        let (visible, remainder) = cap_expanded(&many);
        assert_eq!(visible.lines().count(), EXPANDED_MAX_LINES);
        assert_eq!(remainder, Remainder { lines: 121 - EXPANDED_MAX_LINES, chars: 0 });
        assert_eq!(remainder.label().as_deref(), Some("81 more lines"));
        // One 26k-char line: the per-line cut binds with no lines to count.
        let single = "v".repeat(26_000);
        let (visible, remainder) = cap_expanded(&single);
        assert_eq!(visible, format!("{}…", "v".repeat(EXPANDED_LINE_CHARS)));
        assert!(
            visible.chars().count() <= EXPANDED_MAX_CHARS,
            "single-line body shows {}, over the {EXPANDED_MAX_CHARS} cap",
            visible.chars().count()
        );
        assert_eq!(remainder.lines, 0);
        assert_eq!(remainder.chars, 26_000 - (EXPANDED_LINE_CHARS + 1));
        assert_eq!(remainder.label().as_deref(), Some("23,999 more characters"));
        // Short text hides nothing.
        let (visible, remainder) = cap_expanded("a\nb");
        assert_eq!(visible, "a\nb");
        assert!(remainder.is_empty());
        assert_eq!(cap_expanded("").1, Remainder { lines: 0, chars: 0 });
    }

    #[test]
    fn cuts_land_on_character_boundaries() {
        // Emoji are four bytes each: the cut must never split one.
        let text = "😀".repeat(10_000);
        let (visible, remainder) = cap_expanded(&text);
        assert_eq!(visible, format!("{}…", "😀".repeat(EXPANDED_LINE_CHARS)));
        assert_eq!(remainder.chars, 10_000 - (EXPANDED_LINE_CHARS + 1));
        let preview = cap_preview(&text);
        assert!(preview.chars().count() <= PREVIEW_MAX_CHARS);
    }
}
