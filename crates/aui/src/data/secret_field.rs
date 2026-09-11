//! The masked single-line secret field: the caller's [`InputState`] drawn
//! inside the library's bordered control box, with a reveal toggle.
//!
//! The state — and the secret text inside it — belongs to the caller: the
//! component only reads [`is_masked`](gpui_kit::base::input::InputBaseState)
//! through the presentation snapshot to pick the eye glyph, and it syncs the
//! [`SecretField::placeholder`] / [`SecretField::disabled`] flags the caller
//! set on the component into that state when drawn. It never changes the
//! masked flag itself — the caller flips `set_masked` from
//! [`on_toggle_reveal`](SecretField::on_toggle_reveal) — and it never logs,
//! copies or formats the text.
//!
//! ```ignore
//! use aui::data::secret_field;
//!
//! secret_field("api-key", &key_state)
//!     .placeholder("sk-ant-…")
//!     .on_toggle_reveal(move |window, cx| {
//!         let next = !key_state.read(cx).presentation().is_masked();
//!         key_state.update(cx, |state, cx| state.set_masked(next, window, cx));
//!     })
//! ```

use std::rc::Rc;

use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, Entity, IntoElement, SharedString, Window};
use gpui_kit::base::{
    h_flex,
    input::{Input, InputState},
};

use crate::data::icon_button;

/// The secret control box: `h 34`, the same 6 px radius as a control.
const SECRET_H: f32 = 34.0;
const SECRET_PAD_LEFT: f32 = 10.0;
const SECRET_PAD_RIGHT: f32 = scale::SP_2;
const SECRET_GAP: f32 = scale::SP_3;
/// The secret text: the 12 px mono role, like the login screen's URL row.
const SECRET_TEXT: f32 = scale::FS_12;
/// `.btn:focus-visible{box-shadow:0 0 0 3px var(--accent-ring)}`: no offset,
/// no blur, a 3 px spread in `accent-ring`. The ring lives on the box, never
/// on the input inside it.
const FOCUS_RING: f32 = 3.0;
/// Disabled controls (`opacity:.45` on the forward arrow in the shell header).
const DISABLED_OPACITY: f32 = 0.45;

type RevealHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A masked single-line field for `state`. Build with [`secret_field`].
#[derive(IntoElement)]
pub struct SecretField {
    id: ElementId,
    state: Entity<InputState>,
    placeholder: Option<SharedString>,
    disabled: bool,
    on_toggle_reveal: Option<RevealHandler>,
}

/// The caller's `state` in the library's bordered control box, with a
/// trailing ghost eye button that reports back through
/// [`.on_toggle_reveal`](SecretField::on_toggle_reveal).
pub fn secret_field(id: impl Into<ElementId>, state: &Entity<InputState>) -> SecretField {
    SecretField {
        id: id.into(),
        state: state.clone(),
        placeholder: None,
        disabled: false,
        on_toggle_reveal: None,
    }
}

impl SecretField {
    /// The greyed hint shown while the field is empty; passed through to the
    /// caller's state when drawn.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Non-interactive at 45 % opacity; passed through to the caller's state
    /// when drawn.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The eye button was pressed. The component never flips the masked flag
    /// itself: read the presentation snapshot and call `set_masked` on the
    /// caller's state.
    pub fn on_toggle_reveal(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_reveal = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SecretField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        // The pass-through flags live on the component; the behaviour lives
        // on the caller's state, so sync them when drawn. Both setters only
        // notify on a change, so a settled frame draws exactly once.
        if let Some(placeholder) = self.placeholder.clone() {
            let current = self.state.read(cx).presentation().placeholder().clone();
            if current != placeholder {
                let state = self.state.clone();
                state.update(cx, |s, cx| s.set_placeholder(placeholder, &mut *window, cx));
            }
        }
        if self.state.read(cx).presentation().is_disabled() != self.disabled {
            let disabled = self.disabled;
            let state = self.state.clone();
            state.update(cx, |s, cx| s.set_disabled(disabled, cx));
        }
        let masked = self.state.read(cx).presentation().is_masked();
        let focused = {
            use gpui::Focusable as _;
            self.state.read(cx).focus_handle(cx).is_focused(window)
        };

        let mut eye = icon_button((self.id.clone(), "reveal"), if masked { IconName::Eye } else { IconName::EyeOff })
            .ghost()
            .sm()
            .disabled(self.disabled);
        if let Some(handler) = self.on_toggle_reveal.clone() {
            eye = eye.on_click(move |_, w, cx| handler(w, cx));
        }

        let mut box_ = h_flex()
            .w_full()
            .items_center()
            .h(px(SECRET_H))
            .gap(px(SECRET_GAP))
            .pl(px(SECRET_PAD_LEFT))
            .pr(px(SECRET_PAD_RIGHT))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(if focused { p.accent } else { p.line })
            .bg(p.surface_2)
            .mono(SECRET_TEXT)
            .text_color(p.ink)
            .child(div().flex_1().min_w(px(0.0)).child(Input::new(&self.state)))
            .child(eye);
        if focused {
            box_ = box_.shadow(vec![gpui::BoxShadow {
                color: p.accent_ring,
                offset: gpui::point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(FOCUS_RING),
                inset: false,
            }]);
        }
        if self.disabled {
            box_ = box_.opacity(DISABLED_OPACITY);
        }
        box_
    }
}
