//! The shared chrome of the modal overlays: the flat scrim, the card ground
//! and the enter/exit motion.
//!
//! [`Dialog`](super::Dialog)
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

/// The card shell both modals share (docs/83 §7): the Dialog level
/// (`surface-2`) with the dialog shadow (one deep drop), a 1 px
/// `line-strong` border and the dialog radius (12), riding the shared
/// motion. Padding, gap and
/// content are the caller's.
pub(crate) fn modal_card(p: &Palette, width: f32, style: &PresenceStyle) -> gpui::Div {
    div()
        .relative()
        // The card drops onto its resting place, so the rise goes upwards.
        .top(-style.offset_y)
        .flex_none()
        .w(px(width * style.scale))
        .rounded(px(scale::SHAPE_DIALOG))
        .overflow_hidden()
        .bg(p.surface(aui_tokens::Palette::DIALOG_LEVEL))
        .border_1()
        .border_color(p.line_strong)
        .shadow(p.dialog_shadow())
        .text_color(p.ink)
}

/// A modal's header (docs/83 §7): 44 tall including its 1 px `line-region`
/// underline, padded 16 on the left and 8 on the right, `lead` (a kind tile,
/// a provider mark) before the title in `t-title`, `trail` (a ✕) at the
/// right. Dialogs, sheets and the import preview all draw this one.
pub fn modal_header(
    title: impl Into<gpui::SharedString>,
    lead: Option<gpui::AnyElement>,
    trail: Option<gpui::AnyElement>,
    cx: &App,
) -> gpui::Div {
    use aui_tokens::{ActiveAui as _, AuiStyled as _, TextRole};
    let p = cx.aui().colors;
    div()
        .flex()
        .flex_none()
        .items_center()
        .w_full()
        .h(px(scale::BAR_DEFAULT))
        .gap(px(scale::SP_3))
        .pl(px(MODAL_PAD))
        .pr(px(scale::SP_3))
        .border_b_1()
        .border_color(p.line_region)
        .children(lead.map(|lead| div().flex_none().flex().items_center().child(lead)))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .role_in(TextRole::Title, cx)
                .text_color(p.fg)
                .child(title.into()),
        )
        .children(trail)
}

/// A modal's footer (docs/83 §7): `hint` at the left, then the actions
/// right-aligned in the order given — secondary, then primary last.
pub fn modal_footer(hint: Option<gpui::AnyElement>, actions: Vec<gpui::AnyElement>) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .w_full()
        .gap(px(scale::SP_3))
        .px(px(MODAL_PAD))
        .pb(px(MODAL_PAD))
        .children(hint)
        .child(div().flex_1().min_w(px(0.0)))
        .children(actions)
}

/// The inset of a modal's body and footer from its edge.
pub const MODAL_PAD: f32 = scale::SP_5;

/// The dialog card's ground for a host-built modal body (a form, a
/// preview, a list to pick from): the same fill, radius and shadow as
/// [`Dialog`](super::Dialog), at rest. Padding, gap, role and content are
/// the host's ([`modal_header`] and [`modal_footer`] draw the two ends);
/// centre it on [`crate::screens::sheet_scrim`].
pub fn modal_panel(width: f32, cx: &App) -> gpui::Div {
    use aui_tokens::ActiveAui as _;
    let rest = PresenceStyle { opacity: 1.0, offset_y: px(0.0), scale: 1.0 };
    modal_card(&cx.aui().colors, width, &rest)
}

/// The scrim both modals sit on: [`super::scrim`] over the whole window,
/// faded with the card, with the card centred on it. The card eats its own clicks (`occlude`), so only
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
        .bg(black().opacity(aui_tokens::scale::SCRIM * opacity));
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
