//! The alacritty grid model behind the TUI pane (feature `tui`).
//!
//! A full-screen program — an agent's TUI, `htop`, `vim` — redraws in place:
//! it moves the cursor, clears regions and rewrites cells, so the block parser
//! (which only ever appends) cannot represent it. [`TuiTerm`] runs the bytes
//! through `alacritty_terminal`'s `Term`, which *is* a screen, and
//! [`TuiTerm::snapshot`] flattens the visible grid into [`TuiGrid`]: one
//! [`TuiRow`] per screen line, each already a `String` plus the
//! [`gpui::TextRun`]s that colour it on the [`Palette`]'s ANSI 16.
//!
//! The snapshot carries the cursor: [`TuiGrid::cursor`] is its `(row, column)`
//! when the program has asked for a visible one, and that cell's run is
//! already inverted (`term_cursor` behind `term_bg`) so any caller drawing the
//! rows shows the cursor without knowing where it is.
//!
//! **Out of scope.** No selection and no mouse interaction on the grid yet;
//! the cursor's *shape* (block, beam, underline) is not modelled, only its
//! position. Nothing else about a cell — blink, strikethrough width,
//! wide-character spacing — is modelled either.

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};
use aui_tokens::{scale, Palette};
use gpui::{font, FontWeight, Hsla, TextRun};

/// A TUI screen opens at this many columns until the pane measures itself.
const DEFAULT_COLS: usize = 100;
/// …and this many rows.
const DEFAULT_ROWS: usize = 30;
/// A grid is a screen, not a scrollback: the pane shows the visible page only.
const SCROLLBACK: usize = 0;
/// Trailing blank cells are not drawn, so a run never carries them.
const TRIM_TRAILING_BLANKS: bool = true;

/// Swallows the terminal's side-channel events (title changes, bells,
/// clipboard requests). The pane has no window chrome of its own to update.
#[derive(Debug, Clone, Copy, Default)]
pub struct EventProxy;

impl EventListener for EventProxy {
    fn send_event(&self, _event: Event) {}
}

/// One rendered screen line: the text and the runs that colour it.
#[derive(Debug, Clone, PartialEq)]
pub struct TuiRow {
    /// The line's characters, trailing blanks trimmed.
    pub text: String,
    /// One run per stretch of identical style, covering `text` exactly.
    pub runs: Vec<TextRun>,
}

/// A snapshot of the visible screen, ready to draw.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TuiGrid {
    /// Top to bottom, one entry per screen row.
    pub rows: Vec<TuiRow>,
    /// The text cursor as `(row, column)`, or `None` when the program has
    /// hidden it (`DECTCEM`). The cell it names is already inverted in
    /// [`TuiRow::runs`], so drawing the rows draws the cursor.
    pub cursor: Option<(usize, usize)>,
}

impl TuiGrid {
    /// The rows as plain strings, for a caller that wants
    /// [`aui::workbench::tui_pane`]'s ANSI path instead of the runs.
    pub fn plain_lines(&self) -> Vec<String> {
        self.rows.iter().map(|r| r.text.clone()).collect()
    }
}

/// A terminal screen fed a byte stream.
pub struct TuiTerm {
    term: Term<EventProxy>,
    processor: Processor,
}

impl TuiTerm {
    /// A screen at the default size.
    pub fn new() -> Self {
        Self::with_size(DEFAULT_COLS, DEFAULT_ROWS)
    }

    /// A screen `cols` × `rows` cells.
    pub fn with_size(cols: usize, rows: usize) -> Self {
        let config = Config { scrolling_history: SCROLLBACK, ..Config::default() };
        let size = TermSize::new(cols.max(1), rows.max(1));
        Self { term: Term::new(config, &size, EventProxy), processor: Processor::new() }
    }

    /// Feeds bytes. Chunk boundaries are free.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
    }

    /// Resizes the screen; the program is expected to redraw itself.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(TermSize::new(cols.max(1) as usize, rows.max(1) as usize));
    }

    /// The terminal being driven, for a caller that needs more than a snapshot.
    pub fn term(&self) -> &Term<EventProxy> {
        &self.term
    }

    /// Flattens the visible screen into styled rows on `palette`, inverting
    /// the cursor cell.
    pub fn snapshot(&self, palette: &Palette) -> TuiGrid {
        let grid = self.term.grid();
        let ansi = palette.ansi16();
        let (screen_lines, columns) = (self.term.screen_lines(), self.term.columns());
        // The cursor is a grid point, so a scrolled-back view has to bring it
        // back into screen coordinates; `display_offset` is zero here because
        // the pane keeps no scrollback, but the arithmetic is the honest one.
        let cursor = if self.term.mode().contains(TermMode::SHOW_CURSOR) {
            let point = grid.cursor.point;
            let line = point.line.0 + grid.display_offset() as i32;
            let col = point.column.0;
            (line >= 0 && (line as usize) < screen_lines && col < columns).then_some((line as usize, col))
        } else {
            None
        };
        let mut rows = Vec::with_capacity(screen_lines);
        for line in 0..screen_lines {
            let row = &grid[Line(line as i32)];
            let cursor_col = cursor.filter(|(r, _)| *r == line).map(|(_, c)| c);
            let mut text = String::new();
            let mut runs: Vec<TextRun> = Vec::new();
            let mut last: Option<(Hsla, Option<Hsla>, bool)> = None;
            // Byte length of the row up to and including the cursor cell, so
            // trimming the trailing blanks cannot swallow the cursor.
            let mut keep_bytes = 0usize;
            for col in 0..columns {
                let cell = &row[Column(col)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let bold = cell.flags.contains(Flags::BOLD);
                let dim = cell.flags.contains(Flags::DIM);
                let hidden = cell.flags.contains(Flags::HIDDEN);
                let inverse = cell.flags.contains(Flags::INVERSE);
                let base_fg = resolve(cell.fg, &ansi, palette, dim);
                let base_bg = resolve_bg(cell.bg, &ansi, palette);
                let (mut fg, mut bg) = if inverse {
                    (base_bg.unwrap_or(palette.term_bg), Some(base_fg))
                } else {
                    (base_fg, base_bg)
                };
                if hidden {
                    fg = bg.unwrap_or(palette.term_bg);
                }
                // The cursor is drawn as an inverted cell rather than a
                // separate element, so it survives into whatever draws the
                // rows: ink on `term_cursor`.
                if cursor_col == Some(col) {
                    fg = palette.term_bg;
                    bg = Some(palette.term_cursor);
                }
                let c = if cell.c == '\0' { ' ' } else { cell.c };
                let style = (fg, bg, bold);
                if last == Some(style) {
                    if let Some(run) = runs.last_mut() {
                        run.len += c.len_utf8();
                    }
                } else {
                    runs.push(TextRun {
                        len: c.len_utf8(),
                        font: mono(bold),
                        color: fg,
                        background_color: bg,
                        underline: None,
                        strikethrough: None,
                    });
                    last = Some(style);
                }
                text.push(c);
                if cursor_col == Some(col) {
                    keep_bytes = text.len();
                }
            }
            if TRIM_TRAILING_BLANKS {
                trim_trailing(&mut text, &mut runs, keep_bytes);
            }
            rows.push(TuiRow { text, runs });
        }
        TuiGrid { rows, cursor }
    }
}

impl Default for TuiTerm {
    fn default() -> Self {
        Self::new()
    }
}

fn mono(bold: bool) -> gpui::Font {
    let mut f = font(scale::FONT_MONO);
    if bold {
        f.weight = FontWeight::SEMIBOLD;
    }
    f
}

fn rgb(c: Rgb) -> Hsla {
    gpui::Rgba { r: c.r as f32 / 255.0, g: c.g as f32 / 255.0, b: c.b as f32 / 255.0, a: 1.0 }.into()
}

/// A cell foreground on the design palette. Named colours 0–15 and indexed
/// 0–15 come from [`Palette::ansi16`]; the 256-colour cube and true colour
/// come through as themselves; everything else is the terminal's own ink.
fn resolve(color: Color, ansi: &[Hsla; 16], palette: &Palette, dim: bool) -> Hsla {
    match color {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) if (i as usize) < 16 => ansi[i as usize],
        Color::Indexed(_) => palette.term_fg,
        Color::Named(named) => match named {
            NamedColor::Background => palette.term_bg,
            NamedColor::Cursor => palette.term_cursor,
            NamedColor::Foreground => {
                if dim {
                    palette.term_dim
                } else {
                    palette.term_fg
                }
            }
            NamedColor::BrightForeground => palette.term_fg,
            NamedColor::DimForeground => palette.term_dim,
            other => {
                let i = other as usize;
                if i < 16 {
                    ansi[i]
                } else {
                    // The Dim* block sits at 259..=266 in NamedColor order.
                    palette.term_dim
                }
            }
        },
    }
}

/// A cell background. The default background is left unset so the pane's own
/// `term-bg` shows through rather than being painted per character.
fn resolve_bg(color: Color, ansi: &[Hsla; 16], palette: &Palette) -> Option<Hsla> {
    match color {
        Color::Named(NamedColor::Background) => None,
        other => Some(resolve(other, ansi, palette, false)),
    }
}

/// Drops the trailing run of spaces from a row, keeping the runs in step and
/// never cutting back past `keep` bytes (the cursor cell, which is a blank
/// more often than not).
fn trim_trailing(text: &mut String, runs: &mut Vec<TextRun>, keep: usize) {
    let trimmed = text.trim_end_matches(' ').len().max(keep);
    if trimmed >= text.len() {
        return;
    }
    text.truncate(trimmed);
    let mut kept = 0usize;
    runs.retain_mut(|run| {
        if kept >= trimmed {
            return false;
        }
        run.len = run.len.min(trimmed - kept);
        kept += run.len;
        run.len > 0
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use aui_tokens::{Palette, ThemeKind};

    #[test]
    fn a_screen_snapshots_into_styled_rows() {
        let p = Palette::for_kind(ThemeKind::Dark);
        let mut term = TuiTerm::with_size(20, 3);
        term.feed(b"\x1b[32mok\x1b[0m rest");
        let grid = term.snapshot(&p);
        assert_eq!(grid.rows.len(), 3);
        // The trailing blank is the cursor cell, kept so the cursor is drawn.
        assert_eq!(grid.rows[0].text, "ok rest ");
        assert_eq!(grid.rows[0].runs[0].color, p.ansi16()[2]);
        assert_eq!(grid.rows[0].runs.iter().map(|r| r.len).sum::<usize>(), grid.rows[0].text.len());
    }

    #[test]
    fn the_cursor_is_where_the_program_left_it_and_is_drawn_inverted() {
        let p = Palette::for_kind(ThemeKind::Dark);
        let mut term = TuiTerm::with_size(20, 3);
        term.feed(b"ab\r\ncd");
        let grid = term.snapshot(&p);
        // Two characters written on row 1, so the cursor sits on column 2.
        assert_eq!(grid.cursor, Some((1, 2)));
        // …a blank cell, which the trailing-blank trim must not swallow.
        assert_eq!(grid.rows[1].text, "cd ");
        let last = grid.rows[1].runs.last().expect("a run for the cursor cell");
        assert_eq!(last.background_color, Some(p.term_cursor));
        assert_eq!(last.color, p.term_bg);
        assert_eq!(grid.rows[1].runs.iter().map(|r| r.len).sum::<usize>(), grid.rows[1].text.len());
    }

    #[test]
    fn a_hidden_cursor_is_not_reported_and_not_painted() {
        let p = Palette::for_kind(ThemeKind::Dark);
        let mut term = TuiTerm::with_size(20, 2);
        // DECTCEM off — what every full-screen program does while redrawing.
        term.feed(b"\x1b[?25lhi");
        let grid = term.snapshot(&p);
        assert_eq!(grid.cursor, None);
        assert_eq!(grid.rows[0].text, "hi");
        assert!(grid.rows[0].runs.iter().all(|r| r.background_color.is_none()));
    }

    #[test]
    fn the_grid_redraws_in_place() {
        let p = Palette::for_kind(ThemeKind::Dark);
        let mut term = TuiTerm::with_size(20, 2);
        term.feed(b"first\r\nsecond");
        term.feed(b"\x1b[H\x1b[2Kagain");
        let grid = term.snapshot(&p);
        assert_eq!(grid.rows[0].text, "again ", "the trailing blank is the cursor cell");
        assert_eq!(grid.rows[1].text, "second");
    }
}
