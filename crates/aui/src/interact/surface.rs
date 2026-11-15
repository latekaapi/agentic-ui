//! [`Surface`]: a flat fill on an explicit surface step, and
//! [`menu_panel`], the one floating panel.
//!
//! A surface states its step: there is no ambient elevation (audit C-14,
//! docs/83 §1 rule 3 — fill draws objects, one or two steps above their
//! ground, and only floating things cast a shadow).

use aui_tokens::ActiveAui;
use gpui::{div, prelude::*, px, AnyElement, App, Div, IntoElement, Pixels, RenderOnce, Window};

use super::Shape;

/// The floating panel's padding around its rows (docs/83 §7: 4 px).
pub const PANEL_PAD: f32 = 4.0;

/// A surface step painted flat: `colors.surface(step)`, with the optional
/// elevation shadow the tokens pair with that step. Build with [`surface`];
/// menus, popovers and hover cards take [`menu_panel`].
#[derive(IntoElement)]
pub struct Surface {
    step: u8,
    child: AnyElement,
    shape: Shape,
    shadow: bool,
    fill: bool,
}

/// Paints `colors.surface(step)` under `child`.
pub fn surface(step: u8, child: impl IntoElement) -> Surface {
    Surface { step: step.clamp(1, 8), child: child.into_any_element(), shape: Shape::Container, shadow: true, fill: false }
}

impl Surface {
    /// The corner shape (default [`Shape::Container`]).
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// Whether to paint the shadow token paired with this step (default
    /// **true**). `false` paints the bare fill. The pairing itself is a token
    /// (`elevation_shadow`), so the flat re-skin empties it there.
    pub fn shadow(mut self, shadow: bool) -> Self {
        self.shadow = shadow;
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
        el = Shape::rounded(self.shape, el);
        if self.shadow {
            el = el.shadow(p.elevation_shadow(self.step));
        }
        el
    }
}
