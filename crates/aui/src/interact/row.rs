//! [`Row`]: a ladder-sized row that never paints hover.

use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, Pixels, RenderOnce, Role, SharedString, Window};
use gpui_kit::base::h_flex;

use crate::data::record_ax_label;
use crate::util::ClickHandler;

/// A one- or two-line row on the size ladder: height `metrics.control`
/// (36 / 28) for one line, `metrics.row_2` (52 / 44) with
/// [`.two_line`](Row::two_line).
///
/// The leading slot is `item_px + icon` wide with the text starting on the
/// text axis (`item_px + icon + gap`); the trailing slot carries a count, a
/// time, a kbd or actions. Title text is [`TextRole::Body`], the byline
/// [`TextRole::Caption`] in `fg-muted`; on a selected row the title turns
/// semibold with its width reserved (an invisible semibold copy in flow,
/// the visible label painted over it).
///
/// A `Row` does NOT paint hover: it lives inside a
/// [`FluidList`](super::FluidList) (which paints the highlight) or is
/// wrapped in [`interactive`](super::interactive) when standalone. There is
/// no focus ring on rows. Build with [`row`].
#[derive(IntoElement)]
pub struct Row {
    id: ElementId,
    label: SharedString,
    byline: Option<SharedString>,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    selected: bool,
    depth: u8,
    on_click: Option<ClickHandler>,
}

/// A one-line [`Row`] with `label`.
pub fn row(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Row {
    Row {
        id: id.into(),
        label: label.into(),
        byline: None,
        leading: None,
        trailing: None,
        selected: false,
        depth: 0,
        on_click: None,
    }
}

impl Row {
    /// Adds the second line, making the row `metrics.row_2` tall.
    pub fn two_line(mut self, byline: impl Into<SharedString>) -> Self {
        self.byline = Some(byline.into());
        self
    }

    /// The leading glyph, on the icon axis.
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// The trailing slot: a count, a time, a kbd, actions.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// Whether the row is selected: the title turns semibold (width
    /// reserved) and the enclosing [`FluidList`](super::FluidList) paints
    /// the selected block beneath it.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The nesting depth: indents `inset` per level.
    pub fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Click handler for standalone rows. Rows inside a
    /// [`FluidList`](super::FluidList) leave this unset: activation flows
    /// through the list's `on_activate`.
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// The row height at the active density: `control` for one line,
    /// `row_2` for two lines.
    pub fn height(&self, cx: &App) -> Pixels {
        let metrics = &cx.aui().metrics;
        if self.byline.is_some() { metrics.row_2 } else { metrics.control }
    }

    /// One copy of the title. The `hidden` copy is always semibold and lays
    /// out in flow, reserving the selected width; the visible copy paints
    /// over it and is semibold only while selected, so selecting a row
    /// never reflows its neighbours.
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
        let clickable = self.on_click.is_some();

        // The leading slot reserves icon-axis space even without an icon,
        // so text aligns across rows in a list.
        let mut leaders = h_flex()
            .flex_none()
            .w(metrics.item_px + metrics.icon)
            .h_full()
            .items_center()
            .justify_center();
        // Width reservation for the selected semibold title (see `title`),
        // built before the slots take ownership of their elements.
        let (title_hidden, title_shown) = (self.title(true, cx), self.title(false, cx));
        if let Some(leading) = self.leading {
            leaders = leaders.child(leading);
        }
        let byline = |text: &SharedString| {
            div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).truncate().child(text.clone())
        };

        let mut text = div().relative().flex_1().min_w(px(0.0));
        if let Some(second) = self.byline.as_ref() {
            text = text
                .child(div().child(title_hidden).child(byline(second)))
                .child(div().absolute().inset(px(0.0)).child(title_shown).child(byline(second)));
        } else {
            text = text
                .child(title_hidden)
                .child(div().absolute().inset(px(0.0)).truncate().child(title_shown));
        }

        let mut el = h_flex()
            .id(self.id)
            .w_full()
            .h(height)
            .items_center()
            .gap(metrics.gap)
            .pr(metrics.item_px)
            .when(self.depth > 0, |d| d.ml(metrics.inset * f32::from(self.depth)))
            .child(leaders)
            .child(text);
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
        } else {
            el = el.role(Role::ListItem);
        }
        el
    }
}
