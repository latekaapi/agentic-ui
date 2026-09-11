//! The sign-in screen: a Meta account (device-code flow) or an API key.
//!
//! The screen is stateless like every other component: the caller owns the
//! [`LoginState`] — it is the one that runs the login command, polls, counts
//! the expiry down, and validates the API key — and receives [`LoginIntent`]s
//! back. Nothing here opens a browser, copies a string or writes a log line.
//!
//! The API-key form draws the caller's own field (see
//! [`Login::api_key_field`]): the screen never sees the key itself.
//!
//! Colour stays calm: the card is the plain surface with the hairline border,
//! the only status colour is the ink on the success and error lines, and a
//! failure is marked with the attention border the design rules describe (a
//! plain 1 px border in the status colour, no glow).

use std::rc::Rc;
use std::time::Duration;

use aui_icons::{provider_mark, IconName, Provider};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, relative, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, glyph_ok, icon_button, spinner, Button, ButtonSize};

/// The sign-in card: wide enough for a device URL on one line, no wider.
const CARD_W: f32 = 420.0;
/// The card's padding and the gap between its blocks.
const CARD_PAD: f32 = scale::SP_6;
const CARD_GAP: f32 = scale::SP_5;
/// The product mark above the headline.
const MARK: f32 = 32.0;
/// The headline, one step above the title role's 13 px.
const HEADLINE: f32 = scale::FS_18;
/// The gap between the mark, the headline and the subtitle.
const MASTHEAD_GAP: f32 = scale::SP_3;
/// The copyable URL row: `h 34`, the same 6 px radius as a control.
const URL_H: f32 = 34.0;
const URL_PAD_X: f32 = 10.0;
const URL_GAP: f32 = scale::SP_3;
/// The device code. gpui has no letter-spacing, so the code carries its weight
/// by size alone: the top of the type scale, centred, in the mono face.
const CODE_SIZE: f32 = scale::FS_24;
const CODE_PAD_Y: f32 = scale::SP_4;
/// The spinner row and the resolved rows (success, error).
const STATUS_GAP: f32 = scale::SP_3;
const STATUS_TEXT: f32 = scale::FS_12;
/// The error message box.
const ERROR_PAD_Y: f32 = scale::SP_3;
const ERROR_PAD_X: f32 = scale::SP_4;
/// The action row: hint on the left, spacer, secondary, primary at the right.
const ACTION_GAP: f32 = scale::SP_3;
const HINT_TEXT: f32 = scale::FS_11;
/// The API-key field label, one step below the hint: 11 px in ink-3.
const FIELD_LABEL_TEXT: f32 = scale::FS_11;
/// `@keyframes in{from{transform:translateY(8px)}}` — the card rises into place.
const CARD_RISE: f32 = 8.0;

/// The default product name, and the headline built from it.
const DEFAULT_PRODUCT: &str = "Muse";

/// Which way in the person chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginMethod {
    /// The Meta-account device-code flow (the subscription lane).
    Account,
    /// An API key (pay-as-you-go).
    ApiKey,
}

/// What the sign-in screen is showing.
#[derive(Clone, Debug, PartialEq)]
pub enum LoginState {
    /// Nothing started: choose a method.
    Choose,
    /// The device flow is starting; a spinner row, no code yet.
    Starting,
    /// The device code is on screen.
    Device {
        /// The verification URL the person opens.
        url: SharedString,
        /// The code they type into it.
        code: SharedString,
        /// How long the code is good for, e.g. `"expires in 14:32"`; shown as
        /// the action row's hint.
        expires: Option<SharedString>,
        /// Whether the app is polling: adds the spinner row "Waiting for you
        /// to finish in the browser…".
        waiting: bool,
    },
    /// The API-key form. The field itself is the caller's (see
    /// [`Login::api_key_field`]).
    ApiKey {
        /// Whether the form may be submitted; the primary is disabled until it
        /// is set.
        can_submit: bool,
        /// A validation problem, shown in the attention border.
        error: Option<SharedString>,
    },
    /// `account/loginStart {apiKey}` in flight.
    Validating,
    /// Signed in; a success row before the app enters.
    Success,
    /// Failed; `message` is shown in the danger ink.
    Error {
        /// What went wrong, in the caller's words.
        message: SharedString,
        /// Which method failed, so "Try again" restarts that one; "Choose
        /// another way" always returns to the method choice.
        method: Option<LoginMethod>,
    },
}

/// What the person asked the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginIntent {
    /// Choose → the device flow.
    StartAccount,
    /// Choose → the API-key form.
    UseApiKey,
    /// The API-key form → validating.
    SubmitApiKey,
    /// The eye button in the API-key form; the caller flips the field's
    /// masked flag.
    ToggleReveal,
    /// The API-key form → the method choice.
    Back,
    /// Open the verification URL in the browser.
    OpenBrowser,
    /// Put the device code on the clipboard.
    CopyCode,
    /// Try again after a failure: the method in `method`, or the choice when
    /// there is none.
    Retry,
    /// A failure → the method choice.
    ChooseAnother,
    /// Abandon a flow that is already running; back to the method choice.
    Cancel,
}

type IntentHandler = Rc<dyn Fn(LoginIntent, &mut Window, &mut App)>;

/// The sign-in screen. Build with [`login`].
///
/// ```ignore
/// use aui::data::secret_field;
/// use aui::screens::{login, LoginIntent, LoginState};
///
/// login("sign-in", state.clone())
///     .product("Muse")
///     .api_key_field(secret_field("key", &key_state))
///     .on_intent(|intent, _, cx| match intent {
///         LoginIntent::OpenBrowser => cx.open_url(&url),
///         LoginIntent::CopyCode => cx.write_to_clipboard(gpui::ClipboardItem::new_string(code.to_string())),
///         _ => {}
///     })
/// ```
///
/// Enter inside the API-key field and Escape are the caller's business: the
/// field is the caller's element (gpui-base `set_submit_on_enter` and its
/// event for Enter), and the screen never binds keys itself.
#[derive(IntoElement)]
pub struct Login {
    id: ElementId,
    state: LoginState,
    product: SharedString,
    headline: Option<SharedString>,
    subtitle: Option<SharedString>,
    provider: Provider,
    api_key_field: Option<AnyElement>,
    timing: EnterExit,
    on_intent: Option<IntentHandler>,
}

/// The sign-in screen in `state`. It fills the window it is given and centres
/// its card on the window ground.
pub fn login(id: impl Into<ElementId>, state: LoginState) -> Login {
    Login {
        id: id.into(),
        state,
        product: DEFAULT_PRODUCT.into(),
        headline: None,
        subtitle: None,
        provider: Provider::Claude,
        api_key_field: None,
        timing: EnterExit::DEFAULT,
        on_intent: None,
    }
}

impl Login {
    /// The product being signed in to; the default headline is built from it.
    pub fn product(mut self, name: impl Into<SharedString>) -> Self {
        self.product = name.into();
        self
    }

    /// Overrides the headline (`"Sign in to <product>"` by default).
    pub fn headline(mut self, text: impl Into<SharedString>) -> Self {
        self.headline = Some(text.into());
        self
    }

    /// The muted line under the headline.
    pub fn subtitle(mut self, text: impl Into<SharedString>) -> Self {
        self.subtitle = Some(text.into());
        self
    }

    /// The product mark above the headline.
    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = provider;
        self
    }

    /// The element drawn in the API-key form's field slot — the harness
    /// passes a [`secret_field`](crate::data::secret_field). The screen owns
    /// no text: Enter inside the field and the reveal toggle stay the
    /// caller's business.
    pub fn api_key_field(mut self, field: impl IntoElement) -> Self {
        self.api_key_field = Some(field.into_any_element());
        self
    }

    /// A button was pressed.
    pub fn on_intent(mut self, f: impl Fn(LoginIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }

    /// Skips the enter: the card is drawn at rest on its first frame, for a
    /// static capture.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = Duration::ZERO;
        self
    }
}

impl Login {
    /// The headline actually drawn.
    fn headline_text(&self) -> SharedString {
        self.headline.clone().unwrap_or_else(|| format!("Sign in to {}", self.product).into())
    }

}

/// The one action-row pattern: the hint on the left, a spacer, then the
/// buttons with the primary at the far right. Buttons never wrap.
fn action_row(p: &Palette, hint: Option<SharedString>, buttons: Vec<gpui::AnyElement>) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_center()
        .gap(px(ACTION_GAP))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .ui(HINT_TEXT)
                .text_color(p.ink_3)
                .child(hint.unwrap_or_default()),
        )
        .children(buttons)
}

/// The caller's field in the API-key form's slot; an empty line when the
/// caller passed none, so the form keeps its shape in a static capture.
fn api_key_slot(field: Option<AnyElement>) -> AnyElement {
    match field {
        Some(field) => field,
        None => div().w_full().into_any_element(),
    }
}

/// One button, wired to `intent`.
fn action(id: &ElementId, on_intent: &Option<IntentHandler>, key: &'static str, label: impl Into<SharedString>, intent: LoginIntent, primary: bool) -> impl IntoElement {
    let mut b = button((id.clone(), key), label).size(ButtonSize::Md);
    if primary {
        b = b.primary();
    }
    if let Some(handler) = on_intent.clone() {
        b = b.on_click(move |_, w, cx| handler(intent, w, cx));
    }
    b
}

/// The API-key form's primary: "Sign in", disabled until `can_submit`.
fn submit_action(id: &ElementId, on_intent: &Option<IntentHandler>, can_submit: bool) -> Button {
    let mut submit = button((id.clone(), "submit"), "Sign in").size(ButtonSize::Md).primary().disabled(!can_submit);
    if let Some(handler) = on_intent.clone() {
        submit = submit.on_click(move |_, w, cx| handler(LoginIntent::SubmitApiKey, w, cx));
    }
    submit
}

/// A spinner and one muted line: "Starting sign-in…", "Waiting for you…".
fn status_row(id: ElementId, p: &Palette, text: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_center()
        .gap(px(STATUS_GAP))
        .ui(STATUS_TEXT)
        .text_color(p.ink_3)
        .child(spinner(id))
        .child(div().flex_1().min_w(px(0.0)).child(text.into()))
}

impl RenderOnce for Login {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let headline = self.headline_text();
        // Destructured up front: the API-key slot moves the caller's field
        // out, and the buttons borrow the id and the intent handler.
        let Login { id, state, subtitle, provider, api_key_field, timing, on_intent, .. } = self;
        let sample = presence((id.clone(), "presence"), true, timing, window, cx);
        let style = PresenceStyle::fade_rise(sample, CARD_RISE);

        let mut card = v_flex()
            .relative()
            .top(style.offset_y)
            .opacity(style.opacity)
            .flex_none()
            .w(px(CARD_W))
            .gap(px(CARD_GAP))
            .p(px(CARD_PAD))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .shadow(p.shadow(2))
            .text_color(p.ink)
            .child(masthead(&p, provider, headline, subtitle));

        match state {
            LoginState::Choose => {
                card = card
                    .child(
                        div()
                            .w_full()
                            .ui(STATUS_TEXT)
                            .line_height(relative(scale::LH_UI))
                            .text_center()
                            .text_color(p.ink_3)
                            .child("A Meta account draws on your Muse subscription. An API key bills usage to that key."),
                    )
                    .child(action_row(
                        &p,
                        None,
                        vec![
                            action(&id, &on_intent, "api-key", "Use an API key", LoginIntent::UseApiKey, false).into_any_element(),
                            action(&id, &on_intent, "account", "Continue with Meta account", LoginIntent::StartAccount, true).into_any_element(),
                        ],
                    ));
            }
            LoginState::Starting => {
                card = card
                    .child(status_row((id.clone(), "starting").into(), &p, "Starting sign-in…"))
                    .child(action_row(&p, None, vec![action(&id, &on_intent, "cancel", "Cancel", LoginIntent::Cancel, false).into_any_element()]));
            }
            LoginState::Device { url, code, expires, waiting } => {
                card = card.child(url_row(&id, &p, url, on_intent.clone())).child(code_line(&p, code));
                if waiting {
                    card = card.child(status_row((id.clone(), "waiting").into(), &p, "Waiting for you to finish in the browser…"));
                }
                card = card.child(action_row(
                    &p,
                    expires.or_else(|| Some("Approve the request in your browser, then come back here.".into())),
                    vec![
                        action(&id, &on_intent, "cancel", "Cancel", LoginIntent::Cancel, false).into_any_element(),
                        action(&id, &on_intent, "copy-code", "Copy code", LoginIntent::CopyCode, false).into_any_element(),
                        action(&id, &on_intent, "open", "Open in browser", LoginIntent::OpenBrowser, true).into_any_element(),
                    ],
                ));
            }
            LoginState::ApiKey { can_submit, error } => {
                card = card
                    .child(div().w_full().ui(FIELD_LABEL_TEXT).text_color(p.ink_3).child("Meta API key"))
                    .child(api_key_slot(api_key_field))
                    .child(
                        div()
                            .w_full()
                            .ui(HINT_TEXT)
                            .text_color(p.ink_3)
                            .child("Saved by the muse CLI to ~/.config/muse/auth.json. Never logged."),
                    );
                if let Some(error) = error {
                    card = card.child(error_box(&p, error));
                }
                card = card.child(action_row(
                    &p,
                    None,
                    vec![
                        action(&id, &on_intent, "back", "Back", LoginIntent::Back, false).into_any_element(),
                        submit_action(&id, &on_intent, can_submit).into_any_element(),
                    ],
                ));
            }
            LoginState::Validating => {
                card = card.child(status_row((id.clone(), "validating").into(), &p, "Checking the key…"));
            }
            LoginState::Success => {
                card = card.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap(px(STATUS_GAP))
                        .ui(STATUS_TEXT)
                        .text_color(p.success)
                        .child(glyph_ok())
                        .child(div().flex_1().min_w(px(0.0)).child("Signed in.")),
                );
            }
            LoginState::Error { message, method: _ } => {
                card = card.child(error_box(&p, message)).child(action_row(
                    &p,
                    None,
                    vec![
                        action(&id, &on_intent, "choose", "Choose another way", LoginIntent::ChooseAnother, false).into_any_element(),
                        action(&id, &on_intent, "retry", "Try again", LoginIntent::Retry, true).into_any_element(),
                    ],
                ));
            }
        }

        div().size_full().flex().items_center().justify_center().bg(p.bg).text_role(TextRole::Ui).child(card)
    }
}

/// The product mark, the headline and the muted subtitle, all centred.
fn masthead(p: &Palette, provider: Provider, headline: SharedString, subtitle: Option<SharedString>) -> impl IntoElement {
    let mut column = v_flex()
        .w_full()
        .items_center()
        .gap(px(MASTHEAD_GAP))
        .child(provider_mark(provider).size(px(MARK)))
        .child(
            div()
                .text_role(TextRole::Title)
                .text_px(HEADLINE)
                .text_color(p.ink)
                .child(headline),
        );
    if let Some(subtitle) = subtitle {
        column = column.child(
            div()
                .w_full()
                .ui(STATUS_TEXT)
                .line_height(relative(scale::LH_UI))
                .text_center()
                .text_color(p.ink_3)
                .child(subtitle),
        );
    }
    column
}

/// The verification URL as a full-width mono line on surface-2, with a globe
/// button that opens it. The glyph names what the button does, so the row's
/// affordance is never confused with the code's "Copy code".
fn url_row(id: &ElementId, p: &Palette, url: SharedString, on_intent: Option<IntentHandler>) -> impl IntoElement {
    let mut open = icon_button((id.clone(), "open-url"), IconName::Globe).ghost().sm();
    if let Some(handler) = on_intent {
        open = open.on_click(move |_, w, cx| handler(LoginIntent::OpenBrowser, w, cx));
    }
    h_flex()
        .w_full()
        .items_center()
        .h(px(URL_H))
        .gap(px(URL_GAP))
        .pl(px(URL_PAD_X))
        .pr(px(scale::SP_2))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_2)
        .mono(STATUS_TEXT)
        .text_color(p.ink_2)
        .child(div().flex_1().min_w(px(0.0)).truncate().child(url))
        .child(open)
}

/// The device code: big, mono, 600, centred. gpui has no letter-spacing, so
/// size is what makes it read as something to copy out.
fn code_line(p: &Palette, code: SharedString) -> impl IntoElement {
    div()
        .w_full()
        .py(px(CODE_PAD_Y))
        .mono(CODE_SIZE)
        .semibold()
        .line_height(relative(scale::LH_TIGHT))
        .text_center()
        .text_color(p.ink)
        .child(code)
}

/// The failure message in the danger ink, inside the attention border: a plain
/// 1 px line in the status colour, no glow.
fn error_box(p: &Palette, message: SharedString) -> impl IntoElement {
    div()
        .w_full()
        .py(px(ERROR_PAD_Y))
        .px(px(ERROR_PAD_X))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(p.attention_border(p.danger))
        .ui(STATUS_TEXT)
        .line_height(relative(scale::LH_UI))
        .text_color(p.danger)
        .child(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headline_defaults_to_the_product() {
        assert_eq!(login("l", LoginState::Choose).headline_text(), SharedString::from("Sign in to Muse"));
        assert_eq!(login("l", LoginState::Choose).product("Orca").headline_text(), SharedString::from("Sign in to Orca"));
        assert_eq!(login("l", LoginState::Choose).headline("Welcome back").headline_text(), SharedString::from("Welcome back"));
    }
}
