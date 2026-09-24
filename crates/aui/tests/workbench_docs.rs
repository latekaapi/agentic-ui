//! The document pane's new blocks: headings clamp to levels 1–3, tables size
//! their columns from the caller's proportions without panicking on malformed
//! input, margin collapsing still takes the larger gap, and pages carrying the
//! new variants draw in a real window.

use std::cell::Cell;
use std::rc::Rc;

use aui::workbench::{cell_span_width, clamp_heading_level, collapsed_margin, doc_pane, DocBlock, DocCell, DocPage, DocTable};
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
