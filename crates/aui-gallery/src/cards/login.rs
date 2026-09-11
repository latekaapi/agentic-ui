//! Screen · Sign in. The sign-in screen in its five states: the method
//! choice, the device flow mid-poll, the API-key form (masked, then with an
//! inline error), and a failure with both ways out. Full-bleed at 1440 wide,
//! one band per state beside the legend.

use aui::data::secret_field;
use aui::screens::{login, LoginMethod, LoginState};
use aui_icons::Provider;
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::base::input::InputState;

/// `<a>` the person opens.
const URL: &str = "https://meta.ai/device";
/// The device code they type into it.
const CODE: &str = "WXYZ-2946";
/// The action row's hint.
const EXPIRES: &str = "expires in 14:32";
/// The sample key behind the dots.
const SAMPLE_KEY: &str = "sk-ant-9f2Kv7Qx3RmZ8tYvB4nM";
/// The inline validation failure.
const KEY_ERROR: &str = "That key was rejected (401). Check the key and try again.";
/// The failure message.
const ERROR_MESSAGE: &str = "The request timed out. Check your connection and try again.";
/// One band per state: the 420 px card and its rows fit in 480.
const BAND_H: f32 = 480.0;
/// `.legend{gap:10px;padding-top:6px;max-width:34ch}` — 34ch of Geist 12 ≈ 250 px.
const LEGEND_GAP: f32 = 10.0;
const LEGEND_TOP: f32 = 6.0;
const LEGEND_MEASURE: f32 = 262.0;
/// The two columns: the bands and the legend.
const COLUMN_GAP: f32 = 24.0;
const LEGEND_WIDTH: f32 = 300.0;

/// A band: the login screen in `state`, centred on the window ground at a
/// fixed height so all five states share one capture.
fn band(id: &'static str, state: LoginState, subtitle: Option<&'static str>, field: Option<AnyElement>) -> AnyElement {
    let mut screen = login(id, state).product("Muse").provider(Provider::Muse).at_rest().on_intent(|_, _, _| {});
    if let Some(subtitle) = subtitle {
        screen = screen.subtitle(subtitle);
    }
    if let Some(field) = field {
        screen = screen.api_key_field(field);
    }
    div().w_full().h(px(BAND_H)).child(screen).into_any_element()
}

/// The API-key field for a band: the card owns the state, as the harness
/// does, and the eye flips `set_masked` on it.
fn key_field(window: &mut Window, cx: &mut App, key: &'static str, masked: bool) -> AnyElement {
    let state = window.use_keyed_state(key, cx, |window, cx| {
        InputState::new(window, cx).masked(masked).placeholder("sk-ant-…").default_value(SAMPLE_KEY)
    });
    let toggle = state.clone();
    secret_field(format!("{key}-field"), &state)
        .placeholder("sk-ant-…")
        .on_toggle_reveal(move |window, cx| {
            let next = !toggle.read(cx).presentation().is_masked();
            toggle.update(cx, |state, cx| state.set_masked(next, window, cx));
        })
        .into_any_element()
}

/// Builds the screen.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut bands = v_flex().flex_1().min_w(px(0.0));
    bands = bands.child(band("screen-login-choose", LoginState::Choose, None, None));
    bands = bands.child(band(
        "screen-login-device",
        LoginState::Device { url: URL.into(), code: CODE.into(), expires: Some(EXPIRES.into()), waiting: true },
        Some("Open the link, enter the code, and come back here. We'll pick it up automatically."),
        None,
    ));
    let masked = key_field(window, cx, "screen-login-key", true);
    bands = bands.child(band(
        "screen-login-apikey",
        LoginState::ApiKey { can_submit: true, error: None },
        None,
        Some(masked),
    ));
    let errored = key_field(window, cx, "screen-login-key-error", true);
    bands = bands.child(band(
        "screen-login-apikey-error",
        LoginState::ApiKey { can_submit: true, error: Some(KEY_ERROR.into()) },
        None,
        Some(errored),
    ));
    bands = bands.child(band(
        "screen-login-error",
        LoginState::Error { message: ERROR_MESSAGE.into(), method: Some(LoginMethod::ApiKey) },
        None,
        None,
    ));
    h_flex()
        .w_full()
        .items_start()
        .gap(px(COLUMN_GAP))
        .child(bands)
        .child(
            v_flex()
                .flex_none()
                .w(px(LEGEND_WIDTH))
                .max_w(px(LEGEND_MEASURE))
                .pt(px(LEGEND_TOP))
                .gap(px(LEGEND_GAP))
                .ui(scale::FS_12)
                .text_color(p.ink_2)
                .child(lead_paragraph(p, "Choose.", " A Meta account draws on the Muse subscription; an API key bills usage to the key. The primary starts the device flow."))
                .child(lead_paragraph(p, "Device.", " The copyable URL row, the code, the waiting row while the app polls, and the expiry counting down on its own full-width line above the action row. Cancel backs out to the choice."))
                .child(lead_paragraph(p, "API key.", " The caller's masked field in the library's bordered box; the eye reports back and the caller flips the masked flag. The primary stays disabled until the form may submit."))
                .child(lead_paragraph(p, "Inline error.", " A rejected key fails in place in the attention border, so the person can fix it without leaving the form."))
                .child(lead_paragraph(p, "Error.", " A failed flow names both ways out: Try again restarts the method that failed, Choose another way returns to the choice.")),
        )
        .into_any_element()
}

/// A paragraph whose first word is ink / 600.
fn lead_paragraph(p: Palette, lead: &'static str, rest: &'static str) -> impl IntoElement {
    let text = format!("{lead}{rest}");
    let highlight = HighlightStyle { color: Some(p.ink), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    div().child(StyledText::new(text).with_highlights([(0..lead.len(), highlight)]))
}
