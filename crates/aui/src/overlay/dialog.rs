//! The modal dialog: a scrim over the whole window with one card centred in
//! it.
//!
//! The dialog is stateless like the palette. It paints the scrim, centres the
//! card, and hands back three intents — primary, secondary, dismiss. It does
//! **not** trap focus and it does **not** listen for `esc`: a stateless
//! [`gpui::RenderOnce`] element cannot own a focus handle across frames, so
//! the caller does that, which is also the caller that knows whether the
//! dialog is the frontmost layer.
//!
//! The full wrapping a caller owes a dialog:
//!
//! ```ignore
//! use aui::keys::{Cancel, MENU_CONTEXT};
//! use aui::overlay::{dialog, popover_layer, DialogKind};
//!
//! let focus = self.dialog_focus.clone();
//! window.focus(&focus);
//!
//! popover_layer(
//!     div()
//!         .track_focus(&focus)
//!         .key_context(MENU_CONTEXT)
//!         .on_action(cx.listener(|this, _: &Cancel, window, cx| this.close(window, cx)))
//!         .child(
//!             dialog("session-in-use", "Session already in use")
//!                 .kind(DialogKind::Error)
//!                 .body("Another window holds the writer lease on this session.")
//!                 .secondary("Dismiss")
//!                 .primary("Open another session")
//!                 .on_primary(|_, _| {})
//!                 .on_secondary(|_, _| {})
//!                 .on_dismiss(|_, _| {}),
//!         ),
//! )
//! ```

use std::rc::Rc;
use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{black, div, prelude::*, px, relative, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, ButtonSize};

use super::SCRIM_TINT_TOP;

/// The default card width, the same measure as the sign-in card.
const DIALOG_W: f32 = 420.0;
/// `.dlg{padding:16px;gap:12px;border-radius:var(--r-lg)}`.
const CARD_PAD: f32 = scale::SP_5;
const CARD_GAP: f32 = scale::SP_4;
/// The kind tile, the same 26 px square the approval card leads with.
const TILE: f32 = 26.0;
const TILE_RADIUS: f32 = 7.0;
const TILE_GLYPH: f32 = 14.0;
/// The gap between the tile and the title.
const HEAD_GAP: f32 = scale::SP_3;
/// The mono detail line.
const DETAIL_PAD_Y: f32 = scale::SP_3;
const DETAIL_PAD_X: f32 = scale::SP_4;
/// The action row.
const ACTION_GAP: f32 = scale::SP_3;
/// The card enters like the palette does: a small drop at 98.5 % of its width.
const CARD_DROP: f32 = 6.0;
const CARD_FROM_SCALE: f32 = 0.985;
/// The card's elevation over the scrim.
const CARD_SHADOW: u8 = 3;

/// What a dialog is about. The kind picks the tile's glyph and its tint;
/// nothing else in the card is coloured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DialogKind {
    /// A fact the person has to acknowledge — info clock on the info tint.
    #[default]
    Info,
    /// Something that will cost them if they carry on — warning shield.
    Warning,
    /// Something failed — danger `x`.
    Error,
}

impl DialogKind {
    /// `(ground, ink, glyph)` for the tile.
    fn tile(self, p: &Palette) -> (Hsla, Hsla, IconName) {
        match self {
            DialogKind::Info => (p.info_soft, p.info, IconName::Clock),
            DialogKind::Warning => (p.warning_soft, p.warning, IconName::Shield),
            DialogKind::Error => (p.danger_soft, p.danger, IconName::X),
        }
    }
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A modal dialog. Build with [`dialog`].
///
/// The rendered element is `absolute().inset_0()`: it covers the window it is
/// rendered into, so render it inside the window's root — through
/// [`super::popover_layer`] when anything else in the tree would paint over
/// it. Focus and `esc` are the caller's, as the module docs spell out.
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: SharedString,
    kind: DialogKind,
    body: Option<SharedString>,
    detail: Option<SharedString>,
    primary: SharedString,
    secondary: Option<SharedString>,
    danger: bool,
    width: f32,
    present: bool,
    timing: EnterExit,
    on_primary: Option<Handler>,
    on_secondary: Option<Handler>,
    on_dismiss: Option<Handler>,
}

/// A modal dialog headed `title`, with an `OK` primary until one is set.
pub fn dialog(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Dialog {
    Dialog {
        id: id.into(),
        title: title.into(),
        kind: DialogKind::Info,
        body: None,
        detail: None,
        primary: "OK".into(),
        secondary: None,
        danger: false,
        width: DIALOG_W,
        present: true,
        timing: EnterExit::DEFAULT,
        on_primary: None,
        on_secondary: None,
        on_dismiss: None,
    }
}

impl Dialog {
    /// What the dialog is about; picks the tile.
    pub fn kind(mut self, kind: DialogKind) -> Self {
        self.kind = kind;
        self
    }

    /// The body paragraph, in the muted body ink.
    pub fn body(mut self, text: impl Into<SharedString>) -> Self {
        self.body = Some(text.into());
        self
    }

    /// An optional mono detail line under the body (a path, an id, an error
    /// code).
    pub fn detail(mut self, text: impl Into<SharedString>) -> Self {
        self.detail = Some(text.into());
        self
    }

    /// The label of the primary button at the far right.
    pub fn primary(mut self, label: impl Into<SharedString>) -> Self {
        self.primary = label.into();
        self
    }

    /// The label of the secondary button; without one only the primary is
    /// drawn.
    pub fn secondary(mut self, label: impl Into<SharedString>) -> Self {
        self.secondary = Some(label.into());
        self
    }

    /// Draws the primary as the outlined danger button instead of the accent
    /// fill, for an action that destroys something.
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// The primary was pressed.
    pub fn on_primary(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_primary = Some(Rc::new(f));
        self
    }

    /// The secondary was pressed.
    pub fn on_secondary(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_secondary = Some(Rc::new(f));
        self
    }

    /// The scrim was clicked. A click on the card itself does not reach this.
    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }

    /// Overrides the card width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Whether the dialog is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the dialog is drawn at rest on its first frame, for a
    /// static capture.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = Duration::ZERO;
        self
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let style = PresenceStyle::fade_rise_scale(sample, CARD_DROP, CARD_FROM_SCALE);
        let (tile_bg, tile_ink, glyph) = self.kind.tile(&p);

        let mut card = v_flex()
            .relative()
            // The card drops onto its resting place, so the rise goes upwards.
            .top(-style.offset_y)
            .flex_none()
            .w(px(self.width * style.scale))
            .gap(px(CARD_GAP))
            .p(px(CARD_PAD))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(CARD_SHADOW))
            .text_color(p.ink)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(HEAD_GAP))
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(TILE))
                            .rounded(px(TILE_RADIUS))
                            .bg(tile_bg)
                            .child(icon(glyph).size(px(TILE_GLYPH)).color(tile_ink)),
                    )
                    .child(div().flex_1().min_w(px(0.0)).text_role(TextRole::Title).text_color(p.ink).child(self.title.clone())),
            );

        if let Some(body) = self.body.clone() {
            card = card.child(
                div()
                    .w_full()
                    .ui(scale::FS_12)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_2)
                    .child(body),
            );
        }
        if let Some(detail) = self.detail.clone() {
            card = card.child(
                div()
                    .w_full()
                    .py(px(DETAIL_PAD_Y))
                    .px(px(DETAIL_PAD_X))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.surface_2)
                    .mono(scale::FS_11)
                    .line_height(relative(scale::LH_MONO))
                    .text_color(p.ink_3)
                    .child(detail),
            );
        }

        // The one action row: spacer, secondary, primary at the far right.
        let mut actions = h_flex().w_full().items_center().gap(px(ACTION_GAP)).child(div().flex_1().min_w(px(0.0)));
        if let Some(label) = self.secondary.clone() {
            let mut b = button((id.clone(), "secondary"), label).size(ButtonSize::Md);
            if let Some(handler) = self.on_secondary.clone() {
                b = b.on_click(move |_, w, cx| handler(w, cx));
            }
            actions = actions.child(b);
        }
        {
            let mut b = button((id.clone(), "primary"), self.primary.clone()).size(ButtonSize::Md);
            b = if self.danger { b.danger() } else { b.primary() };
            if let Some(handler) = self.on_primary.clone() {
                b = b.on_click(move |_, w, cx| handler(w, cx));
            }
            actions = actions.child(b);
        }
        card = card.child(actions);

        // The scrim covers the window; the card sits on top of it and eats its
        // own clicks so only the scrim dismisses.
        let mut scrim = div()
            .id(id.clone())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(black().opacity(SCRIM_TINT_TOP * style.opacity));
        if let Some(handler) = self.on_dismiss.clone() {
            scrim = scrim.on_click(move |_, w, cx| handler(w, cx));
        }
        scrim.child(
            div()
                .id((id, "card"))
                .flex_none()
                .opacity(style.opacity)
                .occlude()
                .child(card),
        )
    }
}
