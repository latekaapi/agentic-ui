//! [`Surface`]: the elevation ground everything sits on.

use aui_tokens::ActiveAui;
use gpui::{
    div, prelude::*, px, AnyElement, App, Bounds, Element, ElementId, Global, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, RenderOnce, Style, Window,
};

use super::Shape;

/// The ambient surface step while a subtree lays out (see
/// [`ambient_surface`]). Set by [`AmbientScope`] around its child and put
/// back afterwards, so the step is truly scoped to the subtree — a sibling
/// rendered later reads the enclosing step again.
struct AmbientSurface(u8);

impl Global for AmbientSurface {}

/// A surface step painted flat: `colors.surface(step)`, with the optional
/// elevation shadow and the 1 px ring.
///
/// A plain `surface(step, child)` paints exactly `step` and provides it as
/// the ambient for its subtree. With [`.elevated`](Surface::elevated) the
/// step is derived instead: `min(ambient + offset, 8)`, read from the
/// enclosing surface through the keyed window state and re-provided to the
/// subtree — so a card opens at 3 on a step-1 page, its popover at 5, and
/// so on, capping at 8. gpui renders parents before children, which is what
/// makes the nesting compose; prefer nesting over sequencing elevated
/// siblings (a sibling reads whatever ambient the previous subtree left).
/// Build with [`surface`]; popovers, menus and tooltips take
/// [`menu_surface`].
#[derive(IntoElement)]
pub struct Surface {
    step: u8,
    child: AnyElement,
    offset: Option<u8>,
    shape: Shape,
    shadow: bool,
    ring: bool,
}

/// Paints `colors.surface(step)` under `child` and provides `step` as the
/// ambient for the subtree.
pub fn surface(step: u8, child: impl IntoElement) -> Surface {
    Surface { step: step.clamp(1, 8), child: child.into_any_element(), offset: None, shape: Shape::Container, shadow: false, ring: false }
}

impl Surface {
    /// Derives the step from the ambient instead of using the constructor
    /// step: paints and re-provides `min(ambient + offset, 8)`.
    pub fn elevated(mut self, offset: u8) -> Self {
        self.offset = Some(offset);
        self
    }

    /// The corner shape (default [`Shape::Container`]).
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// Whether to apply `colors.elevation_shadow(step)` (default false).
    pub fn shadow(mut self, shadow: bool) -> Self {
        self.shadow = shadow;
        self
    }

    /// Whether to draw the 1 px ring (`colors.ring()`, default false).
    pub fn ring(mut self, ring: bool) -> Self {
        self.ring = ring;
        self
    }
}

/// The ambient surface step: what the enclosing [`Surface`] provided,
/// defaulting to 1 (the page). Read it inside `render` — a component's
/// `render` runs while its parent lays out, which is exactly when the
/// enclosing [`AmbientScope`] holds the step.
pub fn ambient_surface(_window: &mut Window, cx: &mut App) -> u8 {
    cx.try_global::<AmbientSurface>().map_or(1, |g| g.0)
}

/// Holds `step` as the ambient surface for exactly the layout of `child`,
/// restoring the previous step afterwards. [`Surface`] wraps its child in
/// one; it is public so a host that paints its own ground (a native pane,
/// a webview chrome) can still tell the popovers inside it which step they
/// sit on.
pub struct AmbientScope {
    step: u8,
    child: AnyElement,
}

impl AmbientScope {
    /// Scopes `child` to `step`.
    pub fn new(step: u8, child: impl IntoElement) -> Self {
        Self { step: step.clamp(1, 8), child: child.into_any_element() }
    }
}

impl IntoElement for AmbientScope {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for AmbientScope {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let previous = cx.try_global::<AmbientSurface>().map(|g| g.0);
        cx.set_global(AmbientSurface(self.step));
        let child = self.child.request_layout(window, cx);
        match previous {
            Some(step) => cx.set_global(AmbientSurface(step)),
            None => {
                cx.remove_global::<AmbientSurface>();
            }
        }
        let mut style = Style::default();
        style.size.width = gpui::relative(1.0).into();
        (window.request_layout(style, [child], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

/// The one ground every popover, menu and tooltip adopts: the ambient plus
/// two steps, `Shape::Item`, with shadow and ring.
pub fn menu_surface(child: impl IntoElement) -> Surface {
    surface(1, child).elevated(2).shape(Shape::Item).shadow(true).ring(true)
}

/// The concentric rule as a pure function: an inner surface inset by
/// `inset` under an `outer` radius keeps `max(0, outer − inset)` — a 4 px
/// inset under an 8 px card leaves a 4 px inner radius, never the same 8.
pub fn nested_radius(outer: Pixels, inset: Pixels) -> Pixels {
    let inner = outer - inset;
    if inner > px(0.0) { inner } else { px(0.0) }
}

impl RenderOnce for Surface {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let step = match self.offset {
            Some(offset) => ambient_surface(window, cx).saturating_add(offset).clamp(1, 8),
            None => self.step,
        };
        let mut el = div().bg(p.surface(step)).child(AmbientScope::new(step, self.child));
        el = super::Shape::rounded(self.shape, el);
        if self.shadow {
            el = el.shadow(p.elevation_shadow(step));
        }
        if self.ring {
            el = el.border_1().border_color(p.ring());
        }
        el
    }
}
