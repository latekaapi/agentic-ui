//! The PDF pane's toolbar controls: the default is everything, a read-only
//! preview keeps page stepping only, and both variants draw in a real window.

use std::cell::Cell;
use std::rc::Rc;

use aui::workbench::{pdf_pane, PdfAction, PdfControls, PdfPage, PdfRun};
use gpui::{px, size, Context, IntoElement, Render, TestAppContext, Window};

/// A small regulation page, as the gallery's PDF pane shows it.
fn sample_page() -> PdfPage {
    PdfPage {
        heading: "Chapter IV · Eligibility and qualification of bidders".into(),
        paragraphs: vec![vec![
            PdfRun::Bold("14. Eligibility of bidders.".into()),
            PdfRun::Text(" Every bidder shall be a legal entity registered in the State.".into()),
        ]],
        footer: "Procurement Rules 2019 · 31".into(),
    }
}

/// The default offers every control, matching the toolbar before `PdfControls` existed.
#[test]
fn pdf_controls_default_to_everything() {
    let controls = PdfControls::default();
    assert!(controls.pages, "the default keeps the page stepper");
    assert!(controls.zoom, "the default keeps the zoom picker");
    assert!(controls.search, "the default keeps the search glass");
    assert!(controls.insert_quote, "the default keeps the Insert quote button");
}

/// A read-only preview can only step pages: no draft to insert into and no
/// search or zoom implementation behind those controls.
#[test]
fn reading_only_keeps_pages_and_drops_the_rest() {
    let controls = PdfControls::reading_only();
    assert!(controls.pages, "a read-only preview keeps the page stepper");
    assert!(!controls.zoom, "a read-only preview drops the zoom picker");
    assert!(!controls.search, "a read-only preview drops the search glass");
    assert!(!controls.insert_quote, "a read-only preview drops the Insert quote button");
}

/// `PdfAction` must be nameable from outside the crate, so a host can match on
/// the variants instead of comparing their `Debug` names as strings.
#[test]
fn pdf_action_is_nameable_from_outside_the_crate() {
    for action in [PdfAction::Prev, PdfAction::Next, PdfAction::Zoom, PdfAction::Search, PdfAction::InsertQuote] {
        let label = match action {
            PdfAction::Prev => "prev",
            PdfAction::Next => "next",
            PdfAction::Zoom => "zoom",
            PdfAction::Search => "search",
            PdfAction::InsertQuote => "insert",
        };
        assert!(!label.is_empty(), "every PdfAction variant matches by name");
    }
}

/// The host view: the pane is rendered by a view, as an app renders it.
struct PdfHost {
    page: PdfPage,
    reading_only: bool,
    drawn: Rc<Cell<bool>>,
}

impl Render for PdfHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn.set(true);
        if self.reading_only {
            pdf_pane("workbench-pdf-reading-only", self.page.clone(), 31, 88).controls(PdfControls::reading_only())
        } else {
            pdf_pane("workbench-pdf-default", self.page.clone(), 31, 88)
        }
    }
}

/// Draws the pane in a real window; reports whether it reached render.
fn draws_without_panicking(cx: &mut TestAppContext, page: PdfPage, reading_only: bool) -> bool {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let drawn = Rc::new(Cell::new(false));
    let _window = cx.open_window(size(px(800.0), px(600.0)), {
        let drawn = drawn.clone();
        move |_, _| PdfHost { page: page.clone(), reading_only, drawn }
    });
    cx.run_until_parked();
    drawn.get()
}

#[gpui::test]
fn a_reading_only_pane_builds_and_renders_without_panicking(cx: &mut TestAppContext) {
    assert!(draws_without_panicking(cx, sample_page(), true), "a reading-only pane must draw without panicking");
}

#[gpui::test]
fn a_default_pane_still_builds_and_renders(cx: &mut TestAppContext) {
    assert!(draws_without_panicking(cx, sample_page(), false), "a default pane must draw without panicking");
}
