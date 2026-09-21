//! Shell-command highlighting for the chrome the library draws itself.
//!
//! The shell owns its input line, so the library never colours keystrokes:
//! a highlighting plugin the user installs in their own shell already emits
//! ordinary SGR for that, which the grid renders faithfully. What the
//! library does draw is the block header's command text — the text the `C`
//! marker reports — and that text gets a small honest highlighter here:
//! command words, flags, quoted strings, variables and operators.
//! Everything else stays the default foreground.
//!
//! A construct the scanner cannot tell apart without guessing (paths,
//! globs, arithmetic, substitution interiors, heredoc bodies) is left
//! uncoloured on purpose: a highlighter that guesses is worse than one
//! that does less.
//!
//! Colours come from `aui-tokens` only: every [`SyntaxRole`] resolves to an
//! existing token name via [`role_token`], so both themes read. The palette
//! has no dedicated syntax tokens, so each role maps to the closest
//! existing semantic token (noted on each variant); the one exception is
//! [`SyntaxRole::Command`], where `accent` is the natural decorative hue,
//! not a status colour.

use aui_tokens::Palette;

/// One coloured category of shell-command text.
///
/// Every variant names the token it approximates in its docs: the palette
/// carries no dedicated syntax tokens, so each role borrows the closest
/// existing semantic one until the palette gains proper syntax tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxRole {
    /// The command word: the first word, and the first word after `|`,
    /// `&&`, `||`, `;` or `&`. Reads as `accent`, the decorative hue.
    Command,
    /// A `-x` / `--long` flag. Approximated by `info`.
    Flag,
    /// A quoted string, single or double (unterminated runs to the end).
    /// Approximated by `success`.
    String,
    /// A `$FOO` / `${FOO}` variable. Approximated by `warning`.
    Variable,
    /// An operator or redirection: `|`, `&&`, `||`, `;`, `>`, `>>`, `<`.
    /// Approximated by `ink-2`.
    Operator,
}

impl SyntaxRole {
    /// Every role, for tests that must cover the whole palette mapping.
    pub fn all() -> [SyntaxRole; 5] {
        [
            SyntaxRole::Command,
            SyntaxRole::Flag,
            SyntaxRole::String,
            SyntaxRole::Variable,
            SyntaxRole::Operator,
        ]
    }
}

/// One coloured byte range of a command: `command[start..end]` carries
/// `role`. Ranges are sorted, disjoint, on character boundaries, and cover
/// only the coloured roles — gaps stay the default foreground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxSpan {
    /// Start byte offset in the command text.
    pub start: usize,
    /// End byte offset in the command text.
    pub end: usize,
    /// The coloured category of this range.
    pub role: SyntaxRole,
}

/// The existing token name a role resolves to. Every name is a member of
/// the palette in both themes — never a literal colour.
pub fn role_token(role: SyntaxRole) -> &'static str {
    match role {
        SyntaxRole::Command => "accent",
        SyntaxRole::Flag => "info",
        SyntaxRole::String => "success",
        SyntaxRole::Variable => "warning",
        SyntaxRole::Operator => "ink-2",
    }
}

/// The role's colour on `palette`, looked up by token name.
pub fn role_color(role: SyntaxRole, palette: &Palette) -> gpui::Hsla {
    palette.color(role_token(role)).unwrap_or(palette.term_fg)
}

/// Colours `command` into sorted, disjoint [`SyntaxSpan`]s.
///
/// Covers what is unmistakable in shell syntax and stops there: the
/// command word (the first word, and the first word after `|`, `&&`,
/// `||`, `;` or `&`), flags, quoted strings, `$FOO` / `${FOO}` variables
/// and operators/redirections. A `#` outside quotes starts a comment that
/// stays uncoloured; redirection targets (`> file`) stay uncoloured like
/// any other path. Never panics: empty input yields no spans, and every
/// boundary the scanner emits is ASCII, hence always a character boundary.
pub fn highlight_command(command: &str) -> Vec<SyntaxSpan> {
    let bytes = command.as_bytes();
    let len = bytes.len();
    let mut spans: Vec<SyntaxSpan> = Vec::new();
    let mut i = 0usize;
    // At the start and after a chaining operator the next word is a command.
    let mut expect_command = true;
    while i < len {
        let c = bytes[i];
        if c == b'#' && is_comment_start(bytes, i) {
            // The rest of the line is a comment: default foreground.
            break;
        }
        if c == b'\'' || c == b'"' {
            let quote = c;
            let mut j = i + 1;
            while j < len {
                // A backslash escapes the next character inside double
                // quotes, so an escaped quote cannot close the string.
                // Single quotes have no escapes.
                if quote == b'"' && bytes[j] == b'\\' && j + 1 < len {
                    j += 2;
                    continue;
                }
                if bytes[j] == quote {
                    j += 1;
                    break;
                }
                j += 1;
            }
            // Unterminated quotes run to the end: the remainder IS the
            // string, so colouring it cannot mis-colour anything else.
            spans.push(SyntaxSpan { start: i, end: j, role: SyntaxRole::String });
            i = j;
            expect_command = false;
            continue;
        }
        if c == b'$' {
            if bytes.get(i + 1) == Some(&b'{') {
                match bytes[i + 2..].iter().position(|&b| b == b'}') {
                    Some(rel) if is_var_name(&bytes[i + 2..i + 2 + rel]) => {
                        let end = i + 2 + rel + 1;
                        spans.push(SyntaxSpan { start: i, end, role: SyntaxRole::Variable });
                        i = end;
                        expect_command = false;
                        continue;
                    }
                    // No closing brace, or not a name inside: ambiguous,
                    // leave it (and the brace) uncoloured.
                    _ => {
                        i += 1;
                        continue;
                    }
                }
            }
            if bytes
                .get(i + 1)
                .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
            {
                let mut j = i + 2;
                while j < len && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                spans.push(SyntaxSpan { start: i, end: j, role: SyntaxRole::Variable });
                i = j;
                expect_command = false;
                continue;
            }
            // A lone `$`, `$?`, `$$` and friends: not the documented
            // categories, so plain.
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'{' || c == b'}' {
            // Brace-group punctuation: a boundary but no colour and no
            // state change, so `{ echo hi; }` still reads `echo` as the
            // command it is.
            i += 1;
            continue;
        }
        if c == b'(' {
            i += 1;
            expect_command = true;
            continue;
        }
        if c == b')' {
            i += 1;
            expect_command = false;
            continue;
        }
        if matches!(c, b';' | b'|' | b'&' | b'<' | b'>') {
            let next = bytes.get(i + 1).copied();
            let two = matches!(
                (c, next),
                (b'|', Some(b'|')) | (b'&', Some(b'&')) | (b'>', Some(b'>'))
            );
            if two {
                spans.push(SyntaxSpan { start: i, end: i + 2, role: SyntaxRole::Operator });
                i += 2;
            } else if c == b'&' {
                // A lone `&` is outside the documented operator set: no
                // colour, but it still ends the command.
                i += 1;
            } else {
                spans.push(SyntaxSpan { start: i, end: i + 1, role: SyntaxRole::Operator });
                i += 1;
            }
            // Chaining operators (`;`, `|`, `&&`, `||`, `&`) introduce a
            // new command; redirections (`<`, `>`, `>>`) introduce a
            // filename, which — like any path — stays uncoloured.
            expect_command = !matches!(c, b'<' | b'>');
            continue;
        }
        // A word: commands, flags and plain arguments. Backslash escapes
        // the next byte, so `\#` and `\ ` never start a comment or a split.
        let mut j = i;
        while j < len {
            let d = bytes[j];
            if d.is_ascii_whitespace()
                || matches!(
                    d,
                    b'\'' | b'"' | b'$' | b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')' | b'{' | b'}'
                )
            {
                break;
            }
            if d == b'\\' && j + 1 < len {
                j += 2;
                continue;
            }
            if d == b'#' && is_comment_start(bytes, j) {
                break;
            }
            j += 1;
        }
        if j == i {
            // A trailing lone backslash: plain.
            i += 1;
            continue;
        }
        let word = &bytes[i..j];
        if is_flag(word) {
            spans.push(SyntaxSpan { start: i, end: j, role: SyntaxRole::Flag });
        } else if expect_command {
            spans.push(SyntaxSpan { start: i, end: j, role: SyntaxRole::Command });
        }
        expect_command = false;
        i = j;
    }
    spans
}

/// Batched same-style runs covering `command` exactly: one run per span
/// plus default-foreground runs for the gaps, so the overlay can hand them
/// to `StyledText` directly. An empty command yields a single empty run,
/// mirroring the grid's own row rendering.
pub fn command_runs(command: &str, palette: &Palette) -> Vec<gpui::TextRun> {
    let font = gpui::font(aui_tokens::scale::FONT_MONO);
    let mut runs: Vec<gpui::TextRun> = Vec::new();
    let mut push = |len: usize, color: gpui::Hsla| {
        if len == 0 {
            return;
        }
        match runs.last_mut() {
            Some(run) if run.color == color => run.len += len,
            _ => runs.push(gpui::TextRun {
                len,
                font: font.clone(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            }),
        }
    };
    let mut at = 0usize;
    for span in highlight_command(command) {
        push(span.start.saturating_sub(at), palette.term_fg);
        push(span.end - span.start, role_color(span.role, palette));
        at = span.end;
    }
    push(command.len().saturating_sub(at), palette.term_fg);
    if runs.is_empty() {
        runs.push(gpui::TextRun {
            len: 0,
            font: font.clone(),
            color: palette.term_fg,
            background_color: None,
            underline: None,
            strikethrough: None,
        });
    }
    debug_assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), command.len());
    runs
}

/// Whether the `#` at `i` starts a comment: at the start of the line or
/// right after whitespace, an operator, a bracket or a quote. Anywhere
/// else (`foo#bar`) it is a literal word character.
fn is_comment_start(bytes: &[u8], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    matches!(
        bytes[i - 1],
        b' ' | b'\t' | b'\n' | b'\r' | b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')' | b'{' | b'}' | b'\'' | b'"'
    )
}

/// Whether `name` is a plain variable name: non-empty, ASCII letters,
/// digits and underscores, not starting with a digit.
fn is_var_name(name: &[u8]) -> bool {
    if name.is_empty() {
        return false;
    }
    if !(name[0].is_ascii_alphabetic() || name[0] == b'_') {
        return false;
    }
    name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

/// Whether a word is a flag: one or two leading dashes followed by a name
/// (`-x`, `--long`), optionally with `=value` (`--out=file`). A lone `-`
/// or `--`, three dashes and names starting with punctuation are not
/// flags — too ambiguous to colour.
fn is_flag(word: &[u8]) -> bool {
    let body = if let Some(rest) = word.strip_prefix(b"--") {
        if rest.starts_with(b"-") {
            return false;
        }
        rest
    } else if let Some(rest) = word.strip_prefix(b"-") {
        rest
    } else {
        return false;
    };
    let name = match body.iter().position(|&b| b == b'=') {
        Some(eq) => &body[..eq],
        None => body,
    };
    if name.is_empty() {
        return false;
    }
    if !(name[0].is_ascii_alphanumeric()) {
        return false;
    }
    name.iter().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dark() -> Palette {
        Palette::for_kind(aui_tokens::ThemeKind::Dark)
    }

    fn light() -> Palette {
        Palette::for_kind(aui_tokens::ThemeKind::Light)
    }

    /// The coloured text of each span, in order.
    fn pairs(command: &str) -> Vec<(&str, SyntaxRole)> {
        highlight_command(command)
            .into_iter()
            .map(|span| (&command[span.start..span.end], span.role))
            .collect()
    }

    /// Every syntax category colours as intended in one command exercising
    /// all of them at once.
    #[test]
    fn every_category_colours_in_one_command() {
        let command = "git commit --amend -m \"msg\" $HOME ${USER} | grep -i 'pat' && echo done > out; ls";
        assert_eq!(
            pairs(command),
            vec![
                ("git", SyntaxRole::Command),
                ("--amend", SyntaxRole::Flag),
                ("-m", SyntaxRole::Flag),
                ("\"msg\"", SyntaxRole::String),
                ("$HOME", SyntaxRole::Variable),
                ("${USER}", SyntaxRole::Variable),
                ("|", SyntaxRole::Operator),
                ("grep", SyntaxRole::Command),
                ("-i", SyntaxRole::Flag),
                ("'pat'", SyntaxRole::String),
                ("&&", SyntaxRole::Operator),
                ("echo", SyntaxRole::Command),
                (">", SyntaxRole::Operator),
                (";", SyntaxRole::Operator),
                ("ls", SyntaxRole::Command),
            ]
        );
        // Plain arguments and the redirection target stay uncoloured.
        let spans = highlight_command(command);
        for plain in ["commit", "done", "out"] {
            let at = command.find(plain).expect("precondition");
            assert!(
                spans.iter().all(|s| s.end <= at || s.start >= at + plain.len()),
                "{plain:?} must stay the default foreground: {spans:?}"
            );
        }
    }

    /// A `$` inside a quoted string is string text, not a variable — and a
    /// span never covers only part of a string.
    #[test]
    fn dollars_inside_strings_are_not_variables() {
        assert_eq!(pairs("echo \"a $HOME b\""), vec![
            ("echo", SyntaxRole::Command),
            ("\"a $HOME b\"", SyntaxRole::String),
        ]);
        assert_eq!(pairs("echo 'a $HOME b'"), vec![
            ("echo", SyntaxRole::Command),
            ("'a $HOME b'", SyntaxRole::String),
        ]);
    }

    /// Edge cases do not panic and do not mis-colour the remainder: empty,
    /// flag-only, unterminated quote and `#` comments.
    #[test]
    fn edge_cases_do_not_panic_or_mis_colour() {
        assert!(highlight_command("").is_empty(), "empty command colours nothing");
        assert_eq!(pairs("-x"), vec![("-x", SyntaxRole::Flag)]);
        // An unterminated quote runs to the end — the remainder IS the
        // string — and nothing leaks past it.
        let unterminated = "echo \"abc";
        assert_eq!(pairs(unterminated), vec![
            ("echo", SyntaxRole::Command),
            ("\"abc", SyntaxRole::String),
        ]);
        // A comment swallows flags, variables, quotes and operators after it.
        let command = "echo hi # comment --flag $VAR \"q\" | &&";
        assert_eq!(pairs(command), vec![("echo", SyntaxRole::Command)]);
        // ...but a `#` mid-word is a literal, not a comment.
        assert_eq!(pairs("echo#hi"), vec![("echo#hi", SyntaxRole::Command)]);
        // Lone `$` forms outside the documented categories stay plain.
        assert_eq!(pairs("echo $? $$"), vec![("echo", SyntaxRole::Command)]);
        // A lone `&` chains without colour; `<<` reads as two redirections.
        assert_eq!(pairs("a & b"), vec![
            ("a", SyntaxRole::Command),
            ("b", SyntaxRole::Command),
        ]);
        assert_eq!(pairs("a << b"), vec![
            ("a", SyntaxRole::Command),
            ("<", SyntaxRole::Operator),
            ("<", SyntaxRole::Operator),
        ]);
    }

    /// The payload cap (4 KB) highlights without pathological cost: one
    /// linear pass, spans sorted, disjoint, in bounds and on character
    /// boundaries.
    #[test]
    fn the_payload_cap_highlights_without_pathological_cost() {
        let unit = "echo --long-flag \"quoted $HOME\" $USER ${HOME} | grep -i 'x' && ";
        let mut command = unit.repeat(4096 / unit.len() + 1);
        while command.len() > 4096 {
            command.pop();
        }
        assert_eq!(command.len(), 4096);
        let started = std::time::Instant::now();
        let spans = highlight_command(command.as_str());
        let runs = command_runs(command.as_str(), &dark());
        let elapsed = started.elapsed();
        assert!(elapsed < std::time::Duration::from_millis(500), "took {elapsed:?}");
        let mut end = 0usize;
        for span in &spans {
            assert!(span.start >= end, "spans overlap: {spans:?}");
            assert!(span.end <= command.len(), "span out of bounds: {span:?}");
            assert!(command.is_char_boundary(span.start), "split char: {span:?}");
            assert!(command.is_char_boundary(span.end), "split char: {span:?}");
            end = span.end;
        }
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), command.len());
    }

    /// Both themes resolve every role to a real token (never a literal),
    /// and the same command reads differently across themes.
    #[test]
    fn both_themes_resolve_every_role_to_a_token() {
        for role in SyntaxRole::all() {
            let token = role_token(role);
            for (kind, palette) in [("dark", dark()), ("light", light())] {
                assert!(
                    Palette::COLOR_NAMES.contains(&token),
                    "{token:?} is not a palette token"
                );
                assert!(
                    palette.color(token).is_some(),
                    "{token:?} missing from the {kind} palette"
                );
                assert_eq!(
                    role_color(role, &palette),
                    palette.color(token).unwrap(),
                    "{role:?} is not exactly its token on {kind}"
                );
            }
        }
        let command = "git commit --amend -m \"msg\" $HOME ${USER} | grep x";
        let dark_colors: Vec<gpui::Hsla> =
            command_runs(command, &dark()).into_iter().map(|r| r.color).collect();
        let light_colors: Vec<gpui::Hsla> =
            command_runs(command, &light()).into_iter().map(|r| r.color).collect();
        assert_eq!(dark_colors.len(), light_colors.len());
        assert_ne!(dark_colors, light_colors, "both themes must read differently");
    }

    /// Runs cover the command exactly, including the empty command.
    #[test]
    fn runs_cover_the_command_exactly() {
        for command in ["", "git status", "echo \"a $B\" 'c' --flag $D ${E} | x && y > z; w"] {
            let runs = command_runs(command, &dark());
            assert_eq!(
                runs.iter().map(|r| r.len).sum::<usize>(),
                command.len(),
                "runs must cover {command:?} exactly"
            );
            assert!(!runs.is_empty());
        }
    }
}
