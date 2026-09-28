//! Screens · Providers. The Settings provider card in every headline state,
//! the first-run connect screen over mixed rows, the account-menu usage
//! cards, and a settled whole-JSON assistant message as a `json` code block.

use aui::screens::{
    connect_providers, provider_card, usage_card, ConnectRowData, ProviderActionDef,
    ProviderCardData, ProviderHeadline, UsageWindow,
};
use aui::transcript::assistant_turn;
use aui_icons::Provider;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// One labelled band: a caption over the component.
fn band(caption: &'static str, content: impl IntoElement) -> AnyElement {
    v_flex()
        .w_full()
        .gap(px(6.0))
        .child(
            div()
                .text_px(scale::FS_11)
                .line_height(gpui::relative(scale::LH_UI))
                .child(caption),
        )
        .child(content)
        .into_any_element()
}

fn card_data(
    id: &'static str,
    provider: Provider,
    headline: ProviderHeadline,
    version_advisory: bool,
) -> ProviderCardData {
    let (account, actions) = match headline {
        ProviderHeadline::Connected => (
            Some("Signed in as ada@example.com · Pro".into()),
            vec![ProviderActionDef::sign_out(), ProviderActionDef::recheck()],
        ),
        ProviderHeadline::SignedOut => {
            (None, vec![ProviderActionDef::sign_in(), ProviderActionDef::recheck()])
        }
        ProviderHeadline::NotInstalled => {
            (None, vec![ProviderActionDef::install(), ProviderActionDef::docs()])
        }
        ProviderHeadline::CantRun => {
            (None, vec![ProviderActionDef::recheck(), ProviderActionDef::docs()])
        }
        ProviderHeadline::Unverified => {
            (None, vec![ProviderActionDef::sign_in(), ProviderActionDef::recheck()])
        }
        ProviderHeadline::Checking => (None, vec![ProviderActionDef::recheck()]),
        ProviderHeadline::Disabled => {
            (Some("Was signed in as ada@example.com · Pro".into()), vec![ProviderActionDef::recheck()])
        }
    };
    ProviderCardData {
        id: id.into(),
        provider,
        headline,
        account,
        version: Some("2.1.276".into()),
        advisory: version_advisory.then(|| "Too old — need ≥ 2.2".into()),
        enabled: headline != ProviderHeadline::Disabled,
        actions,
    }
}

/// The Settings provider card in every headline state.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let states = [
        ("Checking — spinner, never “Not installed” while probing", ProviderHeadline::Checking, false),
        ("Connected", ProviderHeadline::Connected, false),
        ("Connected with a version advisory", ProviderHeadline::Connected, true),
        ("Signed out", ProviderHeadline::SignedOut, false),
        ("Installed · sign-in not verified", ProviderHeadline::Unverified, false),
        ("Can't run", ProviderHeadline::CantRun, false),
        ("Not installed", ProviderHeadline::NotInstalled, false),
        ("Disabled", ProviderHeadline::Disabled, false),
    ];
    let mut bands = v_flex().w_full().gap(px(16.0));
    for (n, (caption, headline, advisory)) in states.iter().enumerate() {
        let data = card_data(
            match headline {
                ProviderHeadline::Connected => "claude",
                ProviderHeadline::SignedOut | ProviderHeadline::Unverified => "codex",
                _ => "muse",
            },
            match headline {
                ProviderHeadline::SignedOut | ProviderHeadline::Unverified => Provider::Codex,
                ProviderHeadline::NotInstalled | ProviderHeadline::Disabled => Provider::Muse,
                _ => Provider::Claude,
            },
            *headline,
            *advisory,
        );
        let card = provider_card(format!("gallery-provider-{n}"), &data)
            .on_intent(|_, _, _| {});
        bands = bands.child(band(
            caption,
            div()
                .w_full()
                .rounded(px(scale::R_SM))
                .border_1()
                .border_color(p.line)
                .bg(p.surface_1)
                .child(card),
        ));
    }
    bands.text_color(p.ink_2).into_any_element()
}

/// The first-run connect screen over mixed rows: one connected, one
/// checking, one signed out, one not installed.
pub fn build_connect(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let rows = vec![
        ConnectRowData {
            id: "claude".into(),
            provider: Provider::Claude,
            headline: ProviderHeadline::Connected,
            account: Some("ada@example.com · Pro".into()),
            primary: None,
            docs: true,
        },
        ConnectRowData {
            id: "codex".into(),
            provider: Provider::Codex,
            headline: ProviderHeadline::Checking,
            account: None,
            primary: None,
            docs: false,
        },
        ConnectRowData {
            id: "muse".into(),
            provider: Provider::Muse,
            headline: ProviderHeadline::SignedOut,
            account: None,
            primary: Some(ProviderActionDef::sign_in()),
            docs: true,
        },
        ConnectRowData {
            id: "gemini".into(),
            provider: Provider::Gemini,
            headline: ProviderHeadline::NotInstalled,
            account: None,
            primary: Some(ProviderActionDef::install()),
            docs: true,
        },
    ];
    div()
        .w_full()
        .h(px(640.0))
        .child(
            connect_providers("gallery-connect", rows)
                .intro("Connect at least one provider to start working.")
                .on_intent(|_, _, _| {}),
        )
        .into_any_element()
}

/// The usage cards: empty, one window, two windows with a warning fill.
pub fn build_usage(_window: &mut Window, _cx: &mut App) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap(px(16.0))
        .child(v_flex().gap(px(16.0)).child(band(
            "Empty — not reported yet",
            usage_card("gallery-usage-empty", Provider::Claude, vec![]).as_of("never"),
        )))
        .child(v_flex().gap(px(16.0)).child(band(
            "One window",
            usage_card(
                "gallery-usage-one",
                Provider::Codex,
                vec![UsageWindow::new("Primary · 5h", 0.42, "3h 12m")],
            )
            .plan("Pro")
            .as_of("2m ago"),
        )))
        .child(v_flex().gap(px(16.0)).child(band(
            "Two windows, one at warning",
            usage_card(
                "gallery-usage-two",
                Provider::Muse,
                vec![
                    UsageWindow::new("Weekly", 0.83, "2d 4h"),
                    UsageWindow::new("Primary · 5h", 0.31, "41m"),
                ],
            )
            .plan("Business")
            .as_of("just now"),
        )))
        .into_any_element()
}

/// A settled whole-JSON assistant message beside its streaming twin: the
/// settled one is a single `json` code block, the streaming one stays prose.
pub fn build_json(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let source: SharedString =
        "{\"name\": \"checkout-flow\", \"files\": [\"src/a.ts\", \"src/b.ts\"], \"ok\": true}".into();
    v_flex()
        .w_full()
        .gap(px(16.0))
        .child(band(
            "Settled — one json code block",
            assistant_turn("gallery-json-settled", source.clone()),
        ))
        .child(band(
            "Streaming — plain paragraph, never flips mid-chunk",
            assistant_turn("gallery-json-streaming", source).streaming(true),
        ))
        .into_any_element()
}
