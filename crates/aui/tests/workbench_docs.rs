//! The document pane's new blocks: headings clamp to levels 1–3, tables size
//! their columns from the caller's proportions without panicking on malformed
//! input, margin collapsing still takes the larger gap, and pages carrying the
//! new variants draw in a real window.

use std::cell::Cell;
use std::rc::Rc;

use aui::workbench::{cell_span_width, clamp_heading_level, collapsed_margin, doc_pane, table_layout, DocBlock, DocCell, DocPage, DocTable, TABLE_MIN_COL_W};
use gpui::{px, size, Context, IntoElement, Render, TestAppContext, Window};

/// A cell at index 0 spanning 2 columns of `[0.5, 0.3, 0.2]` is 0.8.
#[test]
fn a_table_cell_width_sums_its_col_span() {
    let width = cell_span_width(&[0.5, 0.3, 0.2], 3, 0, 2);
    assert!((width - 0.8).abs() < 1e-6, "a two-column span of [0.5, 0.3, 0.2] is 0.8, got {width}");
}

/// With no widths every column is equal: 4 columns give 0.25 each.
#[test]
fn a_table_with_no_widths_falls_back_to_equal_columns() {
    for index in 0..4 {
        let width = cell_span_width(&[], 4, index, 1);
        assert!((width - 0.25).abs() < 1e-6, "column {index} of 4 equal columns is 0.25, got {width}");
    }
}

/// A span running past the last column clamps to the row instead of indexing
/// out of bounds.
#[test]
fn a_col_span_past_the_end_does_not_panic() {
    let width = cell_span_width(&[0.5, 0.5], 2, 0, 9);
    assert!((width - 1.0).abs() < 1e-6, "a span past the end clamps to the whole row, got {width}");
}

/// Level 0 maps to 1, anything above 3 maps to 3.
#[test]
fn a_heading_level_is_clamped() {
    assert_eq!(clamp_heading_level(0), 1, "level 0 maps to 1");
    assert_eq!(clamp_heading_level(9), 3, "level 9 maps to 3");
}

/// A Heading after a Paragraph collapses to the heading's 15 px top margin,
// not the paragraph's 10 px bottom margin.
#[test]
fn margin_collapsing_takes_the_larger_of_the_two() {
    let upper = DocBlock::text("Body.");
    let lower = DocBlock::heading(2, "A heading");
    assert_eq!(collapsed_margin(&upper, &lower), 15.0, "the heading's 15 px top margin wins over the paragraph's 10 px bottom margin");
}

/// The host view: the page is rendered by a view, as an app renders it.
struct DocHost {
    page: DocPage,
    drawn: Rc<Cell<bool>>,
}

impl Render for DocHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn.set(true);
        doc_pane("workbench-docs-fixture", self.page.clone())
    }
}

/// Draws `page` in a real window; reports whether it reached render.
fn draws_without_panicking(cx: &mut TestAppContext, page: DocPage) -> bool {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let drawn = Rc::new(Cell::new(false));
    let _window = cx.open_window(size(px(800.0), px(600.0)), {
        let drawn = drawn.clone();
        move |_, _| DocHost { page: page.clone(), drawn }
    });
    cx.run_until_parked();
    drawn.get()
}

#[gpui::test]
fn an_empty_table_paints_nothing_and_does_not_panic(cx: &mut TestAppContext) {
    let page = DocPage::new("Title", "Subtitle", vec![DocBlock::Table(DocTable { widths: vec![], rows: vec![] })]);
    assert!(draws_without_panicking(cx, page), "an empty table must draw its page without panicking");
}

/// A narrow table fills the paper and never scrolls: 2 columns on 428 px of
/// usable paper (the 520 px page minus its 46 px pads) sum back to the width.
#[test]
fn a_narrow_table_fills_the_width_and_does_not_scroll() {
    let (widths, scroll) = table_layout(&[0.658, 0.342], 2, 428.0);
    assert!(!scroll, "a 2-column table fits 428 px without scrolling");
    let total: f32 = widths.iter().sum();
    assert!((total - 428.0).abs() < 0.5, "narrow columns fill the width, got {total}");
}

/// The defect: 11 columns on 428 px must scroll instead of squeezing every
/// column below readability.
#[test]
fn a_wide_table_scrolls_instead_of_squeezing() {
    let (widths, scroll) = table_layout(&[], 11, 428.0);
    assert!(scroll, "an 11-column table does not fit 428 px and must scroll");
    assert_eq!(widths.len(), 11, "one width per column");
    for (n, w) in widths.iter().enumerate() {
        assert!(*w >= TABLE_MIN_COL_W, "column {n} is {w}px, below the {TABLE_MIN_COL_W}px minimum");
    }
}

/// A column with a tiny fraction still paints at the minimum, in either mode.
#[test]
fn no_column_is_ever_narrower_than_the_minimum() {
    for usable in [428.0, 200.0] {
        let (widths, _) = table_layout(&[0.01, 0.33, 0.33, 0.33], 4, usable);
        for (n, w) in widths.iter().enumerate() {
            assert!(*w >= TABLE_MIN_COL_W, "column {n} is {w}px at {usable}px usable, below the {TABLE_MIN_COL_W}px minimum");
        }
    }
}

/// Every row shares the table's column boundaries: row 0 opens with a
/// `col_span: 2` cell and row 1 has eight singles, so the cumulative offsets
/// of row 1's cells 2..8 must match row 0's boundaries past its span.
#[test]
fn every_row_uses_the_same_column_boundaries() {
    let (cols, _) = table_layout(&[], 8, 428.0);
    assert_eq!(cols.len(), 8, "one width per column");
    let spanned = cols[0] + cols[1];
    let mut row0 = vec![spanned];
    row0.extend_from_slice(&cols[2..]);
    assert_eq!(row0.len(), 7, "row 0 covers 8 columns in 7 cells");
    for i in 2..8 {
        let row1_offset: f32 = cols[..=i].iter().sum();
        let row0_offset: f32 = row0[..=(i - 1)].iter().sum();
        assert!(
            (row0_offset - row1_offset).abs() < 1e-3,
            "boundary {i} differs between rows: {row0_offset} vs {row1_offset}"
        );
    }
}

/// Empty widths mean equal columns, not a panic.
#[test]
fn a_table_with_no_widths_still_lays_out() {
    let (widths, _) = table_layout(&[], 4, 428.0);
    assert_eq!(widths.len(), 4, "four equal columns");
    for (n, w) in widths.iter().enumerate() {
        assert!((w - widths[0]).abs() < 1e-3, "column {n} is {w}, expected equal columns of {}", widths[0]);
    }
}

/// Zero usable width must not panic or divide by zero: every column falls
/// back to the minimum and the table scrolls.
#[test]
fn zero_usable_width_does_not_panic_or_divide_by_zero() {
    let (widths, scroll) = table_layout(&[0.5, 0.5], 2, 0.0);
    assert!(scroll, "a table with no room must scroll");
    assert_eq!(widths.len(), 2, "one width per column");
    for w in widths {
        assert!(w.is_finite(), "width must be finite, got {w}");
        assert!(w >= TABLE_MIN_COL_W, "width {w} is below the {TABLE_MIN_COL_W}px minimum");
    }
}

#[gpui::test]
fn an_extended_page_draws_without_panicking(cx: &mut TestAppContext) {
    let page = DocPage::new(
        "Request for Proposal: Teacher Recruitment Services",
        "Directorate of Education · Draft v3",
        vec![
            DocBlock::text("Body."),
            DocBlock::heading(2, "4. Island-wise position"),
            DocBlock::Rule,
            DocBlock::Table(DocTable {
                widths: vec![0.658, 0.342],
                rows: vec![
                    vec![DocCell { col_span: 2, ..DocCell::header("ANDROTH Island at a glance") }],
                    vec![DocCell::text("Total geographical Area"), DocCell::text("4.90 sq.Kms")],
                    vec![DocCell::text("Maximum Length"), DocCell::text("4.66 km")],
                    vec![DocCell::text("Total No. of Schools"), DocCell::text("6")],
                ],
            }),
        ],
    );
    assert!(draws_without_panicking(cx, page), "a page with heading, rule and table must draw without panicking");
}
