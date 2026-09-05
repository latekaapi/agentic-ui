//! The assistant's sheet pane (spec §5.5): formula bar, grid with row and
//! column headers, the selected cell, and the sheet tabs.

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{chip, tag};

/// `.fx{gap:8px;height:30px;padding:0 10px;font-size:12px}`.
const FX_H: f32 = 30.0;
const FX_GAP: f32 = 8.0;
const FX_PAD: f32 = 10.0;
/// `.fx .cell{width:52px;height:22px;radius:4px;font:500 11px mono}`; `.fx .f{height:22px;padding:0 8px;font:11px mono}`.
const CELL_W: f32 = 52.0;
const FIELD_H: f32 = 22.0;
const FIELD_PAD: f32 = 8.0;
/// The chip's sparkle glyph: 11 px.
const CHIP_GLYPH: f32 = 11.0;
/// `.grid{grid-template-columns:32px repeat(5,1fr);font-size:11.5px}`; cells 26 px, padding 0 8.
const ROW_HEADER_W: f32 = 32.0;
const CELL_H: f32 = 26.0;
const CELL_PAD: f32 = 8.0;
const CELL_TEXT: f32 = 11.5;
/// `.grid .h{font:500 10.5px mono}`.
const HEADER_TEXT: f32 = 10.5;
/// `.selc{outline:2px accent;outline-offset:-2px}`.
const SELECT_OUTLINE: f32 = 2.0;
/// `.stabs{gap:2px;height:30px;padding:0 8px;font-size:11.5px}` with 22 px radius-5 tabs, padding 0 10.
const TABS_H: f32 = 30.0;
const TABS_GAP: f32 = 2.0;
const TABS_PAD: f32 = 8.0;
const TAB_H: f32 = 22.0;
const TAB_PAD: f32 = 10.0;
const TAB_RADIUS: f32 = 5.0;

/// One cell.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetCell {
    /// Text.
    pub text: SharedString,
    /// Right-aligned tabular mono.
    pub numeric: bool,
    /// Weight 600.
    pub bold: bool,
}

impl SheetCell {
    /// A text cell.
    pub fn text(text: impl Into<SharedString>) -> Self {
        Self { text: text.into(), numeric: false, bold: false }
    }

    /// A number cell.
    pub fn num(text: impl Into<SharedString>) -> Self {
        Self { text: text.into(), numeric: true, bold: false }
    }

    /// Bold.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

/// The sheet pane. Build with [`sheet_pane`].
#[derive(IntoElement)]
pub struct SheetPane {
    id: ElementId,
    columns: Vec<SharedString>,
    rows: Vec<Vec<SheetCell>>,
    header_row: bool,
    selected: Option<(usize, usize)>,
    formula: Option<(SharedString, SharedString)>,
    tabs: Vec<SharedString>,
    active_tab: usize,
    tabs_note: Option<SharedString>,
}

/// A pane over `rows` (row 0 is the bold header row when `header_row`).
pub fn sheet_pane(id: impl Into<ElementId>, columns: Vec<SharedString>, rows: Vec<Vec<SheetCell>>) -> SheetPane {
    SheetPane { id: id.into(), columns, rows, header_row: true, selected: None, formula: None, tabs: Vec::new(), active_tab: 0, tabs_note: None }
}

impl SheetPane {
    /// The selected cell `(row, column)` (0-based data indices).
    pub fn selected(mut self, row: usize, column: usize) -> Self {
        self.selected = Some((row, column));
        self
    }

    /// The formula bar: cell reference and formula.
    pub fn formula(mut self, cell: impl Into<SharedString>, formula: impl Into<SharedString>) -> Self {
        self.formula = Some((cell.into(), formula.into()));
        self
    }

    /// Sheet tabs at the bottom.
    pub fn tabs(mut self, tabs: Vec<SharedString>, active: usize) -> Self {
        self.tabs = tabs;
        self.active_tab = active;
        self
    }

    /// The mono note at the right of the tabs (`weighted by Annex A`).
    pub fn tabs_note(mut self, note: impl Into<SharedString>) -> Self {
        self.tabs_note = Some(note.into());
        self
    }
}

impl RenderOnce for SheetPane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut pane = v_flex().id(id.clone()).size_full().bg(p.surface_1).ui(CELL_TEXT).text_color(p.ink);

        if let Some((cell, formula)) = self.formula {
            pane = pane.child(
                h_flex()
                    .w_full()
                    .h(px(FX_H))
                    .flex_none()
                    .gap(px(FX_GAP))
                    .px(px(FX_PAD))
                    .border_b_1()
                    .border_color(p.line)
                    .child(
                        div()
                            .flex_none()
                            .w(px(CELL_W))
                            .h(px(FIELD_H))
                            .rounded(px(scale::R_XS))
                            .border_1()
                            .border_color(p.line)
                            .bg(p.surface_2)
                            .flex()
                            .items_center()
                            .justify_center()
                            .mono(scale::FS_11)
                            .line_height(relative(1.0))
                            .medium()
                            .child(cell),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .h(px(FIELD_H))
                            .px(px(FIELD_PAD))
                            .rounded(px(scale::R_XS))
                            .border_1()
                            .border_color(p.line)
                            .bg(p.surface_2)
                            .flex()
                            .items_center()
                            .mono(scale::FS_11)
                            .line_height(relative(1.0))
                            .text_color(p.ink_2)
                            .truncate()
                            .child(formula),
                    )
                    .child(chip((id.clone(), "ask"), "Ask about selection").accent().leading(icon(IconName::Sparkle).size(px(CHIP_GLYPH)))),
            );
        }

        // Grid: a header row of column letters, then rows with a row-number header.
        let cell_box = |el: gpui::Div| el.h(px(CELL_H)).px(px(CELL_PAD)).border_r_1().border_b_1().border_color(p.line).flex().items_center().overflow_hidden().whitespace_nowrap();
        let header_cell = |text: SharedString| {
            cell_box(div().flex_1().min_w(px(0.0)))
                .justify_center()
                .bg(p.surface_2)
                .text_color(p.ink_3)
                .mono(HEADER_TEXT)
                .line_height(relative(1.0))
                .medium()
                .child(text)
        };
        let mut grid = v_flex().w_full().flex_1().min_h(px(0.0)).overflow_hidden();
        let mut head = h_flex().w_full();
        head = head.child(cell_box(div().flex_none().w(px(ROW_HEADER_W))).bg(p.surface_2));
        for c in &self.columns {
            head = head.child(header_cell(c.clone()));
        }
        grid = grid.child(head);
        for (r, row) in self.rows.iter().enumerate() {
            let mut line = h_flex().w_full();
            line = line.child(
                cell_box(div().flex_none().w(px(ROW_HEADER_W)))
                    .justify_center()
                    .bg(p.surface_2)
                    .text_color(p.ink_3)
                    .mono(HEADER_TEXT)
                    .line_height(relative(1.0))
                    .medium()
                    .child((r + 1).to_string()),
            );
            for (c, cell) in row.iter().enumerate() {
                let is_header = self.header_row && r == 0;
                let selected = self.selected == Some((r, c));
                let mut el = cell_box(div().flex_1().min_w(px(0.0)).relative());
                if is_header {
                    el = el.bg(p.surface_2).semibold();
                }
                if cell.numeric {
                    el = el.justify_end().font_family(scale::FONT_MONO);
                }
                if cell.bold {
                    el = el.semibold();
                }
                if selected {
                    el = el.bg(p.accent_soft).child(div().absolute().inset_0().border(px(SELECT_OUTLINE)).border_color(p.accent));
                }
                line = line.child(el.child(cell.text.clone()));
            }
            grid = grid.child(line);
        }
        pane = pane.child(grid);

        if !self.tabs.is_empty() {
            let mut tabs = h_flex().w_full().h(px(TABS_H)).flex_none().gap(px(TABS_GAP)).px(px(TABS_PAD)).border_t_1().border_color(p.line).ui(CELL_TEXT);
            for (i, t) in self.tabs.iter().enumerate() {
                let on = i == self.active_tab;
                tabs = tabs.child(
                    div()
                        .h(px(TAB_H))
                        .px(px(TAB_PAD))
                        .rounded(px(TAB_RADIUS))
                        .flex()
                        .items_center()
                        .text_color(if on { p.ink } else { p.ink_3 })
                        .when(on, |d| d.bg(p.surface_3))
                        .child(t.clone()),
                );
            }
            tabs = tabs.child(div().flex_1());
            if let Some(note) = self.tabs_note {
                tabs = tabs.child(tag(note));
            }
            pane = pane.child(tabs);
        }
        pane
    }
}
