//! The shared chrome of the modal overlays: the flat scrim, the card ground
//! and the enter/exit motion.
//!
//! [`Dialog`](super::Dialog) and [`SettingsDialog`](super::SettingsDialog)
//! draw through these pieces so the two cards stay identical. Neither adds
//! its own scrim, radius, shadow or motion; each only fills the card shell
//! with its own header and body.

use std::rc::Rc;
use std::time::Duration;

use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, Palette};
use gpui::{black, div, prelude::*, px, App, ElementId, Window};

/// The card drops onto its resting place, like the palette does: a small drop
/// at 98.5 % of its width.
pub(crate) const MODAL_DROP: f32 = 6.0;
pub(crate) const MODAL_FROM_SCALE: f32 = 0.985;
/// The card's elevation over the scrim.
pub(crate) const MODAL_SHADOW: u8 = 3;

/// An intent a modal overlay hands back: dismiss, primary, secondary, a flip.
/// The caller owns the state; the component only reports.
pub(crate) type ModalIntent = Rc<dyn Fn(&mut Window, &mut App)>;

/// Runs the enter/exit presence for a modal card and shapes it into the
/// shared fade-rise-scale.
pub(crate) fn modal_presence(
    id: &ElementId,
    present: bool,
    timing: EnterExit,
    window: &mut Window,
    cx: &mut App,
) -> PresenceStyle {
    let sample = presence((id.clone(), "presence"), present, timing, window, cx);
    PresenceStyle::fade_rise_scale(sample, MODAL_DROP, MODAL_FROM_SCALE)
}

/// The card shell both modals share: overlay ground, 1 px line-strong border,
/// large radius, elevation 3, riding the shared motion. Padding, gap and
/// content are the caller's.
pub(crate) fn modal_card(p: &Palette, width: f32, style: &PresenceStyle) -> gpui::Div {
    div()
        .relative()
        // The card drops onto its resting place, so the rise goes upwards.
        .top(-style.offset_y)
        .flex_none()
        .w(px(width * style.scale))
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line_strong)
        .bg(p.overlay)
        .shadow(p.shadow(MODAL_SHADOW))
        .text_color(p.ink)
}

/// The scrim both modals sit on: the flat top stop over the whole window with
/// the card centred on it. The card eats its own clicks (`occlude`), so only
/// the scrim dismisses.
pub(crate) fn modal_scrim(
    id: ElementId,
    opacity: f32,
    on_dismiss: Option<ModalIntent>,
    card: impl IntoElement,
) -> impl IntoElement {
    let mut scrim = div()
        .id(id.clone())
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(black().opacity(super::SCRIM_TINT_TOP * opacity));
    if let Some(handler) = on_dismiss {
        scrim = scrim.on_click(move |_, w, cx| handler(w, cx));
    }
    scrim.child(div().id((id, "card")).flex_none().opacity(opacity).occlude().child(card))
}

/// Skips the enter of a timing: the overlay is drawn at rest on its first
/// frame, for a static composition (the design card) rather than one the
/// person just opened.
pub(crate) fn rest_timing(mut timing: EnterExit) -> EnterExit {
    timing.enter = Duration::ZERO;
    timing
}
