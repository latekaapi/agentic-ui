//! [`Surface`]: a flat fill on an explicit surface step, and
//! [`menu_panel`], the one floating panel.
//!
//! A surface states its step: there is no ambient elevation (audit C-14,
//! docs/83 §1 rule 3 — fill draws objects, one or two steps above their
//! ground, and only floating things cast a shadow).

use aui_tokens::ActiveAui;
use gpui::{div, prelude::*, px, AnyElement, App, Div, ElementId, IntoElement, Pixels, RenderOnce, Stateful, Window};

use super::Shape;

/// The floating panel's padding around its rows (docs/83 §7: 4 px).
pub const PANEL_PAD: f32 = 4.0;

/// A surface step painted flat: `colors.surface(step)`, no shadow and no
/// edge (docs/83 §1 rule 3: nothing nested casts a shadow). Build with
/// [`surface`]; menus, popovers and hover cards take [`menu_panel`].
#[derive(IntoElement)]
pub struct Surface {
    step: u8,
    child: AnyElement,
    shape: Shape,
    fill: bool,
}

/// Paints `colors.surface(step)` under `child`.
pub fn surface(step: u8, child: impl IntoElement) -> Surface {
    Surface { step: step.clamp(1, 8), child: child.into_any_element(), shape: Shape::Container, fill: false }
}

impl Surface {
    /// The corner shape (default [`Shape::Container`]).
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// Kept for source compatibility; draws nothing.
    pub fn ring(self, _ring: bool) -> Self {
        self
    }

    /// The surface fills its parent (both axes) and lays its child out as
    /// a column, so a pane or page whose parent has a definite size gets
    /// that size instead of shrinking to its content.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }
}

/// The one floating panel — every dropdown, popover, hover card and
/// picker: the `overlay` fill, a 1 px `line-strong` edge, the container
/// radius (10), the one float shadow and [`PANEL_PAD`] inside (docs/83 §7).
/// Returns a plain `Div` so a host can size it.
pub fn menu_panel(child: impl IntoElement, cx: &App) -> Div {
    let p = cx.aui().colors;
    Shape::Container
        .rounded(div())
        .bg(p.overlay)
        .border_1()
        .border_color(p.line_strong)
        .shadow(p.floating_shadow())
        .p(px(PANEL_PAD))
        .child(child)
}

/// The layer under a float or a modal that takes the outside click: it
/// fills its positioned parent, swallows the pointer so nothing under it
/// reacts, and calls `on_dismiss` on a click (on the release, so a press
/// that began on a row inside the float still lands there first). It is
/// transparent; a modal paints its scrim with `.bg(..)` and centres its
/// card as a child. Its accessible name says what a press does.
pub fn dismiss_layer(
    id: impl Into<ElementId>,
    label: impl Into<gpui::SharedString>,
    on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .absolute()
        .inset_0()
        .occlude()
        .role(gpui::Role::Button)
        .aria_label(label.into())
        .on_click(move |_, w, cx| on_dismiss(w, cx))
}

/// The concentric rule as a pure function: an inner surface inset by
/// `inset` under an `outer` radius keeps `max(0, outer − inset)` — a 4 px
/// inset under a 10 px panel leaves a 6 px row, never the same 10.
pub fn nested_radius(outer: Pixels, inset: Pixels) -> Pixels {
    let inner = outer - inset;
    if inner > px(0.0) { inner } else { px(0.0) }
}

impl RenderOnce for Surface {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut el = div().bg(p.surface(self.step)).child(self.child);
        if self.fill {
            el = el.size_full().flex().flex_col().overflow_hidden();
        }
        Shape::rounded(self.shape, el)
    }
}
