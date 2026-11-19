//! Palette lead: a non-row element under the section title.
//!
//! The lead must not take a selection index — the arrow keys walk rows
//! only — and a section may carry a lead with no rows at all.

use std::cell::RefCell;
use std::rc::Rc;

use aui::keys::{Confirm, SelectNext, SelectPrev, MENU_CONTEXT};
use aui::overlay::{command_palette, PaletteIcon, PaletteItem, PaletteSection};
use gpui::{div, prelude::*, App, Context, FocusHandle, IntoElement, SharedString, TestAppContext};

type Log = Rc<RefCell<Vec<String>>>;

struct LeadHost {
    focus: FocusHandle,
    selected: usize,
    items: Vec<SharedString>,
    lead_only_first: bool,
    log: Log,
}

impl LeadHost {
    fn select(&self, cx: &mut App) {
        let id = self.items[self.selected].clone();
        self.log.borrow_mut().push(format!("select:{id}"));
        let _ = cx;
    }

    fn items(&self) -> Vec<PaletteItem> {
        self.items
            .iter()
            .map(|id| PaletteItem::new(id.clone(), PaletteIcon::Glyph(aui::icons::IconName::Search), id.clone()))
            .collect()
    }
}

impl Render for LeadHost {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.listener(|this, _: &SharedString, _, cx| this.select(cx));
        let items = self.items();
        let sections = if self.lead_only_first {
            vec![
                PaletteSection::new("Add", Vec::new()).lead(div().child("drop a folder here")),
                PaletteSection::new("Actions", items),
            ]
        } else {
            vec![PaletteSection::new("Add", items).lead(div().child("drop a folder here"))]
        };
        div()
            .key_context(MENU_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                this.selected = (this.selected + 1).min(this.items.len() - 1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| {
                this.selected = this.selected.saturating_sub(1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Confirm, _, cx| this.select(cx)))
            .child(command_palette("test-palette-lead", "", sections, self.selected).at_rest().on_select(move |id, w, cx| select(id, w, cx)))
            .into_any_element()
    }
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
}

/// The lead is not a row: `down` from the first row still lands on the
/// second row, and `enter` reports it.
#[gpui::test]
fn lead_does_not_take_a_selection_index(cx: &mut TestAppContext) {
    init(cx);
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |window, cx: &mut Context<LeadHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            LeadHost { focus, selected: 0, items: vec!["one".into(), "two".into()], lead_only_first: false, log }
        }
    });
    cx.simulate_keystrokes("down enter");
    assert_eq!(&*log.borrow(), &["select:two".to_string()]);
}

/// A section may carry a lead and no rows: the following section's rows
/// still select from the top.
#[gpui::test]
fn lead_only_section_selects_the_first_row(cx: &mut TestAppContext) {
    init(cx);
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |window, cx: &mut Context<LeadHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            LeadHost { focus, selected: 0, items: vec!["one".into(), "two".into()], lead_only_first: true, log }
        }
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(&*log.borrow(), &["select:one".to_string()]);
}

/// A palette whose query left nothing: sections that are empty (or none),
/// and the caller's empty line.
struct EmptyHost {
    rows: Vec<SharedString>,
}

impl Render for EmptyHost {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let items = self
            .rows
            .iter()
            .map(|id| PaletteItem::new(id.clone(), PaletteIcon::Glyph(aui::icons::IconName::Search), id.clone()))
            .collect();
        div().child(command_palette("test-palette-empty", "zz", vec![PaletteSection::new("Commands", items)], 0).at_rest().empty("Nothing matches"))
    }
}

/// Gate 4: a query that matched nothing drew an empty "Commands" heading.
/// The body now says "Nothing matches" instead, and only when it is empty.
#[gpui::test]
fn an_empty_palette_says_nothing_matches_and_only_then(cx: &mut TestAppContext) {
    init(cx);
    aui::data::arm_ax_probe(true);
    aui::data::take_ax_labels();
    let (host, cx) = cx.add_window_view(|_, _| EmptyHost { rows: Vec::new() });
    cx.run_until_parked();
    let labels = aui::data::take_ax_labels();
    assert!(labels.iter().any(|label| label == "Nothing matches"), "the empty line draws: {labels:?}");
    host.update(cx, |host, cx| {
        host.rows = vec!["one".into()];
        cx.notify();
    });
    cx.run_until_parked();
    let labels = aui::data::take_ax_labels();
    aui::data::arm_ax_probe(false);
    assert!(!labels.iter().any(|label| label == "Nothing matches"), "a palette with a row draws no empty line: {labels:?}");
}
