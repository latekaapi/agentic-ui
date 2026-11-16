//! The static containers of docs/83: [`card`] (a flat filled object),
//! [`section`] (a page section: a title over line-separated rows, no card),
//! [`settings_row`] (title + description left, a control right) and
//! [`banner`] (the one-line notice above the composer).
//!
//! None of them casts a shadow or draws a border (docs/83 §1 rule 3): fill
//! draws objects, hairlines draw structure.

use std::rc::Rc;

use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, RenderOnce, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use super::button::button;

// Values docs/83 names that have no token yet; task 2.3 moves them into
// tokens.json (see `crate::interact` for the shared row metrics).

/// The container radius of a card and a banner (`r-lg`, 10).
pub const R_CARD: f32 = 10.0;
/// A card's header row height (docs/83 §5: 36).
pub const CARD_HEADER_H: f32 = 36.0;
/// A card's inner padding on x.
pub const CARD_PAD_X: f32 = 16.0;
/// A card's inner padding on y.
pub const CARD_PAD_Y: f32 = 12.0;
/// A settings row's vertical padding (docs/83 §7: 16).
pub const SETTINGS_ROW_PAD_Y: f32 = 16.0;
/// The gap between a settings row's text and its control.
pub const SETTINGS_ROW_GAP: f32 = 24.0;
/// The space between a section title and its first row.
pub const SECTION_TITLE_GAP: f32 = 8.0;
/// A banner's height (docs/83 §5: 36).
pub const BANNER_H: f32 = 36.0;
/// A banner's inner padding on x.
pub const BANNER_PAD_X: f32 = 12.0;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A 1 px `line` separator across its container.
fn rule(cx: &App) -> impl IntoElement {
    div().w_full().h(px(1.0)).flex_none().bg(cx.aui().colors.line)
}

// ── card ───────────────────────────────────────────────────────────────────

/// A flat card: `surface-2` fill, radius 10, no border, no shadow; an
/// optional 36 px header row and line-separated sections. Build with
/// [`card`].
#[derive(IntoElement)]
pub struct Card {
    header: Option<AnyElement>,
    sections: Vec<AnyElement>,
    label: Option<SharedString>,
    pad: bool,
}

/// An empty [`Card`].
pub fn card() -> Card {
    Card { header: None, sections: Vec::new(), label: None, pad: true }
}

impl Card {
    /// The header row: `title` in Body, with an optional trailing element
    /// (actions, a status word) at its right.
    pub fn title(mut self, title: impl Into<SharedString>, trailing: Option<AnyElement>) -> Self {
        let title = title.into();
        self.label.get_or_insert_with(|| title.clone());
        self.header = Some(
            h_flex()
                .w_full()
                .h(px(CARD_HEADER_H))
                .px(px(CARD_PAD_X))
                .items_center()
                .justify_between()
                .child(div().flex_1().min_w(px(0.0)).truncate().text_role(TextRole::Body).child(title))
                .children(trailing)
                .into_any_element(),
        );
        self
    }

    /// A caller-built header row in place of [`Self::title`].
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// Adds a section; sections are separated by 1 px `line`.
    pub fn section(mut self, body: impl IntoElement) -> Self {
        self.sections.push(body.into_any_element());
        self
    }

    /// Sections fill the card edge to edge (a code body, a terminal) instead
    /// of sitting in the card's padding.
    pub fn flush(mut self) -> Self {
        self.pad = false;
        self
    }

    /// The card's accessible name (defaults to its title).
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let label = self.label.unwrap_or_else(|| "Card".into());
        let mut el = v_flex()
            .id(ElementId::Name(label.clone()))
            .w_full()
            .bg(p.surface_2)
            .rounded(px(R_CARD))
            .overflow_hidden()
            .text_color(p.fg)
            .role(gpui::Role::Group)
            .aria_label(label);
        let mut first = true;
        if let Some(header) = self.header {
            el = el.child(header);
            first = false;
        }
        for section in self.sections {
            if !first {
                el = el.child(rule(cx));
            }
            first = false;
            el = el.child(div().w_full().when(self.pad, |d| d.px(px(CARD_PAD_X)).py(px(CARD_PAD_Y))).child(section));
        }
        el
    }
}

// ── section ────────────────────────────────────────────────────────────────

/// A page section: a title in Subtitle (500) over rows separated by 1 px
/// `line` — no card around it (docs/83 §7, Settings). Build with
/// [`section`].
#[derive(IntoElement)]
pub struct Section {
    title: SharedString,
    description: Option<SharedString>,
    rows: Vec<AnyElement>,
    note: Option<SharedString>,
}

/// A [`Section`] titled `title`.
pub fn section(title: impl Into<SharedString>) -> Section {
    Section { title: title.into(), description: None, rows: Vec::new(), note: None }
}

impl Section {
    /// A muted line under the title.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Adds a row (a [`settings_row`], a list row, anything).
    pub fn row(mut self, row: impl IntoElement) -> Self {
        self.rows.push(row.into_any_element());
        self
    }

    /// Adds rows.
    pub fn rows(mut self, rows: impl IntoIterator<Item = AnyElement>) -> Self {
        self.rows.extend(rows);
        self
    }

    /// A muted Caption line under the last row, for a pointer that is not
    /// a setting ("Grouping and sort live in the sidebar's view menu").
    pub fn note(mut self, text: impl Into<SharedString>) -> Self {
        self.note = Some(text.into());
        self
    }
}

impl RenderOnce for Section {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut head = v_flex()
            .w_full()
            .pb(px(SECTION_TITLE_GAP))
            .child(div().role_in(TextRole::Subtitle, cx).medium().text_color(p.fg).child(self.title.clone()));
        if let Some(description) = self.description {
            head = head.child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(description));
        }
        let mut el = v_flex().id(ElementId::Name(self.title.clone())).w_full().role(gpui::Role::Group).aria_label(self.title).child(head);
        for (i, row) in self.rows.into_iter().enumerate() {
            if i > 0 {
                el = el.child(rule(cx));
            }
            el = el.child(row);
        }
        if let Some(note) = self.note {
            el = el.child(div().w_full().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(note));
        }
        el
    }
}

// ── settings row ───────────────────────────────────────────────────────────

/// One settings row: the title (Body) and a muted description (Caption)
/// on the left, a control on the right, 16 px vertical padding. Its
/// separators belong to the [`Section`] it sits in. Build with
/// [`settings_row`].
#[derive(IntoElement)]
pub struct SettingsRow {
    title: SharedString,
    description: Option<SharedString>,
    control: Option<AnyElement>,
}

/// A [`SettingsRow`] titled `title`.
pub fn settings_row(title: impl Into<SharedString>) -> SettingsRow {
    SettingsRow { title: title.into(), description: None, control: None }
}

impl SettingsRow {
    /// The muted line under the title.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// The control at the right: a switch, a segmented, a field, a button.
    /// The control carries its own accessible name.
    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }
}

impl RenderOnce for SettingsRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut text = v_flex()
            .flex_1()
            .min_w(px(0.0))
            .child(div().role_in(TextRole::Body, cx).text_color(p.fg).child(self.title.clone()));
        if let Some(description) = self.description {
            text = text.child(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child(description));
        }
        h_flex()
            .id(ElementId::Name(self.title.clone()))
            .w_full()
            .py(px(SETTINGS_ROW_PAD_Y))
            .gap(px(SETTINGS_ROW_GAP))
            .items_center()
            .role(gpui::Role::Group)
            .aria_label(self.title)
            .child(text)
            .children(self.control.map(|c| div().flex_none().child(c)))
    }
}

// ── banner ─────────────────────────────────────────────────────────────────

/// The notice above the composer (child lost, plan mode, signed out):
/// `surface-2`, radius 10, no border, 36 tall, Body text, an optional
/// leading glyph and a text-button action at the right. Build with
/// [`banner`].
#[derive(IntoElement)]
pub struct Banner {
    id: ElementId,
    text: SharedString,
    leading: Option<AnyElement>,
    action: Option<(SharedString, Handler)>,
}

/// A [`Banner`] keyed `id` saying `text`.
pub fn banner(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Banner {
    Banner { id: id.into(), text: text.into(), leading: None, action: None }
}

impl Banner {
    /// A glyph (or a dot) before the text.
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// The text-button action at the right; `label` is its accessible name.
    pub fn action(mut self, label: impl Into<SharedString>, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.action = Some((label.into(), Rc::new(f)));
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let metrics = cx.aui().metrics;
        let mut el = h_flex()
            .id(self.id.clone())
            .w_full()
            .h(px(BANNER_H))
            .pl(px(BANNER_PAD_X))
            .pr(px(BANNER_PAD_X / 2.0))
            .gap(metrics.gap)
            .items_center()
            .bg(p.surface_2)
            .rounded(px(R_CARD))
            .role(gpui::Role::Status)
            .aria_label(self.text.clone());
        if let Some(leading) = self.leading {
            el = el.child(div().flex_none().text_color(p.fg_muted).child(leading));
        }
        el = el.child(div().flex_1().min_w(px(0.0)).truncate().role_in(TextRole::Body, cx).text_color(p.fg).child(self.text));
        if let Some((label, f)) = self.action {
            el = el.child(button((self.id, "action"), label).ghost().on_click(move |_, w, cx| f(w, cx)));
        }
        el
    }
}
