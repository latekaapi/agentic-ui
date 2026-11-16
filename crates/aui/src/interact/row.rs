//! [`Row`]: the one list row. [`list_row`] is the flat list row of
//! docs/83 (32 tall, 10 px inner padding, radius 8, paints its own hover,
//! press and selection with the one model); [`row`] is the same row on the
//! older size ladder, unpainted, for hosts whose list paints the rows
//! ([`RowList`](super::RowList)).

use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, Pixels, RenderOnce, Role, SharedString, Window};
use gpui_kit::base::h_flex;

use super::paint::{PaintStates, StatePaint};
use super::{InteractState, ROW_PAD_X, R_ITEM};
use crate::data::record_ax_label;
use crate::util::ClickHandler;

type PressHandler = Box<dyn Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static>;

/// A one- or two-line row. Build with [`list_row`] (the flat row) or
/// [`row`] (the ladder row).
///
/// The trailing slot carries a count, a time, a kbd or actions. Title text
/// is [`TextRole::Body`], the byline [`TextRole::Caption`] in `fg-muted`.
///
/// A painted row ([`list_row`]) draws its own states with the one model
/// ([`StatePaint`]): hover and press are gpui refinements, selected holds
/// `ov-selected`. Selection is the fill only — the title keeps its weight.
/// An unpainted row ([`row`]) paints nothing: its list does, and on a
/// selected ladder row the title turns semibold with its width reserved.
/// There is no focus ring on rows.
#[derive(IntoElement)]
pub struct Row {
    id: ElementId,
    label: SharedString,
    byline: Option<SharedString>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    selected: bool,
    disabled: bool,
    depth: u8,
    on_click: Option<ClickHandler>,
    on_press: Option<PressHandler>,
    /// The flat list row: painted, 32 tall, 10 px padding, no reserved
    /// leading slot, no bold on selection.
    flat: bool,
    preview: Option<InteractState>,
}

/// A one-line ladder [`Row`] with `label`: unpainted, `metrics.control`
/// tall, the leading slot always reserved.
pub fn row(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Row {
    Row {
        id: id.into(),
        label: label.into(),
        byline: None,
        leading: None,
        trailing: None,
        selected: false,
        disabled: false,
        depth: 0,
        on_click: None,
        on_press: None,
        flat: false,
        preview: None,
    }
}

/// The flat list row (docs/83 §2.5, §4): 32 tall (`Metrics::control`), 10 px
/// inner padding ([`ROW_PAD_X`]), radius 8 ([`R_ITEM`]), an optional leading
/// element and a trailing slot; hover / press / selected from the one model.
/// Its accessible name is its title; with a click handler it is a button,
/// otherwise a list item.
pub fn list_row(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Row {
    Row { flat: true, ..row(id, label) }
}

impl Row {
    /// Adds the second line, making the row `metrics.row_2` tall.
    pub fn two_line(mut self, byline: impl Into<SharedString>) -> Self {
        self.byline = Some(byline.into());
        self
    }

    /// The leading glyph. A glyph built without its own colour inherits the
    /// row's ink, so it lifts with the row.
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// The trailing slot: a count, a time, a kbd, actions.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// Whether the row is selected: a painted row holds `ov-selected`; a
    /// ladder row's title turns semibold and its list paints the block.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// A disabled row: muted, no state paint, no clicks.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Pins a state for a gallery card or a capture (painted rows only).
    pub fn preview(mut self, state: InteractState) -> Self {
        self.preview = Some(state);
        self
    }

    /// The nesting depth: indents `inset` per level.
    pub fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Click handler. Rows inside a [`RowList`](super::RowList) leave this
    /// unset: activation flows through the list's `on_activate`.
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// The primary button went down on the row — for a row that must act
    /// before the release lands on whatever its action uncovers (a Back
    /// row that closes a page over another control). Keep [`Self::on_click`]
    /// too, so a keyboard or assistive press still reaches it; the host
    /// makes the second call a no-op.
    pub fn on_press(mut self, f: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Box::new(f));
        self
    }

    /// The row height: `metrics.control` for one line (32, compact 28),
    /// `row_2` for two.
    pub fn height(&self, cx: &App) -> Pixels {
        let metrics = &cx.aui().metrics;
        if self.byline.is_some() {
            metrics.row_2
        } else {
            metrics.control
        }
    }

    /// One copy of the title. The `hidden` copy is always semibold and lays
    /// out in flow, reserving the selected width; the visible copy paints
    /// over it and is semibold only while selected, so selecting a row
    /// never reflows its neighbours (ladder rows only).
    ///
    /// The weight is applied AFTER the role: `role_in` sets the role's own
    /// weight last, so the order matters (a `semibold()` before it is
    /// silently overwritten). Public so the tests can inspect the style.
    #[doc(hidden)]
    pub fn title(&self, hidden: bool, cx: &App) -> gpui::Div {
        div()
            .when(hidden, |d| d.opacity(0.0))
            .role_in(TextRole::Body, cx)
            .when(hidden || self.selected, |d| d.semibold())
            .text_color(cx.aui().colors.fg)
            .truncate()
            .child(self.label.clone())
    }
}

impl RenderOnce for Row {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.aui();
        let p = theme.colors;
        let metrics = theme.metrics;
        let height = self.height(cx);
        let clickable = self.on_click.is_some() && !self.disabled;
        let byline = |text: &SharedString| {
            div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).truncate().child(text.clone())
        };

        // Width reservation for the selected semibold title of a ladder row
        // (see `title`), built before the slots take ownership.
        let titles = (!self.flat).then(|| (self.title(true, cx), self.title(false, cx)));
        // A nested row is indented by a margin, so it fills its column's
        // width less the indent (stretched, not `w_full`, which would
        // overflow the column by the indent).
        let mut el = h_flex()
            .id(self.id)
            .when(self.depth == 0, |d| d.w_full())
            .h(height)
            .items_center()
            .gap(metrics.gap)
            .when(self.depth > 0, |d| d.ml(metrics.inset * f32::from(self.depth)));

        if self.flat {
            // The flat row: the leading element only when there is one, the
            // title in the row's ink (fg; muted when disabled), no reserved
            // bold width — selection is the fill.
            let ink = if self.disabled { p.fg_muted } else { p.fg };
            el = el
                .px(px(ROW_PAD_X))
                .rounded(px(R_ITEM))
                .text_color(ink)
                .paint_states(
                    StatePaint::new().selected(self.selected).disabled(self.disabled).preview(self.preview),
                    cx,
                );
            if let Some(leading) = self.leading {
                el = el.child(div().flex_none().flex().items_center().child(leading));
            }
            let title = div().role_in(TextRole::Body, cx).truncate().child(self.label.clone());
            let text = match self.byline.as_ref() {
                Some(second) => div().flex_1().min_w(px(0.0)).child(title).child(byline(second)),
                None => div().flex_1().min_w(px(0.0)).child(title),
            };
            el = el.child(text);
        } else {
            // The leading slot reserves icon-axis space even without an
            // icon, so text aligns across rows in a list.
            let mut leaders = h_flex().flex_none().w(metrics.item_px + metrics.icon).h_full().items_center().justify_center();
            let (title_hidden, title_shown) = titles.unwrap_or_else(|| (div(), div()));
            if let Some(leading) = self.leading {
                leaders = leaders.child(leading);
            }
            let mut text = div().relative().flex_1().min_w(px(0.0));
            if let Some(second) = self.byline.as_ref() {
                text = text
                    .child(div().child(title_hidden).child(byline(second)))
                    .child(div().absolute().inset(px(0.0)).child(title_shown).child(byline(second)));
            } else {
                text = text.child(title_hidden).child(div().absolute().inset(px(0.0)).truncate().child(title_shown));
            }
            el = el.pr(metrics.item_px).child(leaders).child(text);
        }
        if let Some(trailing) = self.trailing {
            el = el.child(h_flex().flex_none().items_center().gap(metrics.gap).child(trailing));
        }
        // The accessible name is the visible title; a clickable standalone
        // row is a button, a list row is a list item.
        record_ax_label(&self.label);
        el = el.aria_label(self.label.clone());
        if clickable {
            el = el.role(Role::Button).cursor_pointer();
            if let Some(on_click) = self.on_click {
                el = el.on_click(move |e, w, cx| on_click(e, w, cx));
            }
            if let Some(on_press) = self.on_press {
                el = el.on_mouse_down(gpui::MouseButton::Left, move |e, w, cx| on_press(e, w, cx));
            }
        } else {
            el = el.role(Role::ListItem);
        }
        el
    }
}
