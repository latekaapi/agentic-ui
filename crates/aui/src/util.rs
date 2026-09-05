//! Small helpers shared by the components: per-element interaction state
//! (hover / press) kept in the window, keyed by element id.

use gpui::{App, ElementId, Entity, Window};

/// Hover and press flags for one interactive element. Lives in the window's
/// element state, so stateless (`RenderOnce`) components can animate hover
/// tints and press springs without owning a view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Interaction {
    /// The pointer is over the element.
    pub hovered: bool,
    /// The primary button is down on the element.
    pub pressed: bool,
}

/// The interaction state for `id`, created on first use.
pub fn interaction(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Entity<Interaction> {
    let id: ElementId = id.into();
    window.use_keyed_state((id, "aui-interaction"), cx, |_, _| Interaction::default())
}

/// Reads the current flags without keeping the entity.
pub fn interaction_flags(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> (Entity<Interaction>, Interaction) {
    let state = interaction(id, window, cx);
    let flags = *state.read(cx);
    (state, flags)
}

/// Wires hover / press tracking into a stateful element.
pub trait TrackInteraction: gpui::StatefulInteractiveElement + gpui::InteractiveElement + Sized {
    /// Updates `state` from hover, mouse-down and mouse-up events.
    fn track_interaction(self, state: &Entity<Interaction>) -> Self {
        let hover = state.clone();
        let down = state.clone();
        let up = state.clone();
        let up_out = state.clone();
        self.on_hover(move |hovered, _, cx| {
            hover.update(cx, |s, cx| {
                if s.hovered != *hovered {
                    s.hovered = *hovered;
                    if !*hovered {
                        s.pressed = false;
                    }
                    cx.notify();
                }
            })
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            down.update(cx, |s, cx| {
                s.pressed = true;
                cx.notify();
            })
        })
        .on_mouse_up(gpui::MouseButton::Left, move |_, _, cx| {
            up.update(cx, |s, cx| {
                s.pressed = false;
                cx.notify();
            })
        })
        .on_mouse_up_out(gpui::MouseButton::Left, move |_, _, cx| {
            up_out.update(cx, |s, cx| {
                if s.pressed {
                    s.pressed = false;
                    cx.notify();
                }
            })
        })
    }
}

impl<T: gpui::StatefulInteractiveElement + gpui::InteractiveElement + Sized> TrackInteraction for T {}

/// A click handler stored by a component builder.
pub type ClickHandler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;
