//! Screen · Sign in. The device-code sign-in screen mid-flow: the code is on
//! screen, the app is polling, and the expiry counts down in the action row's
//! hint. Full-bleed at 1440×900 like the other screens.

use aui::screens::{login, LoginState};
use aui_icons::Provider;
use gpui::*;

/// `<a>` the person opens.
const URL: &str = "https://meta.ai/device";
/// The device code they type into it.
const CODE: &str = "WXYZ-2946";
/// The action row's hint.
const EXPIRES: &str = "expires in 14:32";

/// Builds the screen.
pub fn build(_window: &mut Window, _cx: &mut App) -> AnyElement {
    login(
        "screen-login",
        LoginState::Device { url: URL.into(), code: CODE.into(), expires: Some(EXPIRES.into()), waiting: true },
    )
    .product("Muse")
    .provider(Provider::Muse)
    .subtitle("Open the link, enter the code, and come back here. We'll pick it up automatically.")
    // A static capture, not a screen the person just reached.
    .at_rest()
    .on_intent(|_, _, _| {})
    .into_any_element()
}
