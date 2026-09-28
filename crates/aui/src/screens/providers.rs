//! Provider surfaces: the Settings card, the first-run connect screen and
//! the account-menu usage card (`docs/23-providers-connect.md` §4).
//!
//! Everything here is stateless like [`login`](super::login): the caller owns
//! the statuses (it runs the probes, caches them and flips the enabled
//! switch) and receives intent callbacks back. Nothing here opens a browser,
//! runs an installer or writes state.
//!
//! Why a standalone row kind instead of a `SettingsRow::Provider` variant:
//! `SettingsRow` is matched explicitly in `overlay::settings` (and in hosts),
//! so a new variant would break exhaustive matches downstream. [`provider_card`]
//! renders the same row shape — mark, name, status, account, version, switch,
//! actions — reached through a builder, additively.
//!
//! Every interactive element carries a role and a human label: action buttons
//! via [`Button::accessibility_label`](crate::data::Button::accessibility_label)
//! (which sets `role=Button`), the enabled switch via
//! `Switch::accessibility_label`, and the Skip link via `role=Link` plus
//! `aria_label`.
//!
//! ```ignore
//! use aui::screens::{provider_card, ProviderAction, ProviderCardData, ProviderHeadline};
//!
//! provider_card("card-claude", &ProviderCardData {
//!     id: "claude".into(),
//!     provider: aui::icons::Provider::Claude,
//!     headline: ProviderHeadline::Connected,
//!     account: Some("Signed in as ada@example.com · Pro".into()),
//!     version: Some("2.1.276".into()),
//!     advisory: None,
//!     enabled: true,
//!     actions: vec![ProviderActionDef::sign_out(), ProviderActionDef::recheck()],
//! })
//! .on_intent(|intent, _, _| println!("{} -> {:?}", intent.id, intent.action))
//! ```

use std::rc::Rc;

use aui_icons::{provider_mark, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::switch::Switch;

use crate::composer::provider_display_name;
use crate::data::{button, spinner, tag, ButtonSize};

/// Card padding and gaps, the settings page's measure.
const CARD_PAD: f32 = scale::SP_4;
const CARD_GAP: f32 = scale::SP_3;
const ROW_GAP: f32 = scale::SP_2;
/// The provider mark in card rows.
const MARK: f32 = 20.0;
/// The name line and the headline line.
const NAME_TEXT: f32 = scale::FS_13;
const HEADLINE_TEXT: f32 = scale::FS_11;
/// The account and version lines under the headline.
const DETAIL_TEXT: f32 = scale::FS_11;
/// The status dot.
const DOT: f32 = 7.0;
/// The checking spinner.
const SPIN: f32 = 11.0;
/// Usage bar geometry: full-width track, 4 px tall.
const BAR_H: f32 = 4.0;
const BAR_R: f32 = 2.0;
/// The connect screen card width: room for a version line and two buttons.
const CONNECT_W: f32 = 480.0;
/// The usage card's popover width.
const USAGE_W: f32 = 300.0;
/// Usage fills at or above this fraction wear the warning ink.
const WARN_AT: f32 = 0.8;

/// A provider's headline state, first match wins (design §2): Checking →
/// Disabled → Not installed → Can't run → Signed out → Installed ·
/// sign-in not verified → Connected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderHeadline {
    /// No probe result yet this launch and no cache.
    Checking,
    /// Signed in with a verified email/plan.
    Connected,
    /// Installed but signed out.
    SignedOut,
    /// Not installed.
    NotInstalled,
    /// Installed but the probe failed: never shown while a probe is pending.
    CantRun,
    /// Installed but sign-in is not verified.
    Unverified,
    /// The person's switch is off.
    Disabled,
}

impl ProviderHeadline {
    /// The status line beside the dot, exactly as designed.
    pub fn label(self) -> &'static str {
        match self {
            ProviderHeadline::Checking => "Checking…",
            ProviderHeadline::Connected => "Connected",
            ProviderHeadline::SignedOut => "Signed out",
            ProviderHeadline::NotInstalled => "Not installed",
            ProviderHeadline::CantRun => "Can't run",
            ProviderHeadline::Unverified => "Installed · sign-in not verified",
            ProviderHeadline::Disabled => "Disabled",
        }
    }
}

/// What the person asked for on a provider row, always with the row id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderAction {
    /// Start the provider's sign-in flow.
    SignIn,
    /// Sign out of the provider (hosts confirm first: a CLI sign-out is a
    /// real `logout` of that CLI).
    SignOut,
    /// Re-run the probes for this provider.
    Recheck,
    /// Open the dock terminal with the vendor's install command typed, not run.
    Install,
    /// Open the provider's docs.
    Docs,
    /// The enabled switch flipped; the value is the new state.
    SetEnabled(bool),
}

/// One provider intent: the row id plus what was pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderIntent {
    /// The row's [`ProviderCardData::id`].
    pub id: SharedString,
    /// What was pressed.
    pub action: ProviderAction,
}

/// One action button on a provider row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderActionDef {
    /// What the button reports.
    pub action: ProviderAction,
    /// The button label.
    pub label: SharedString,
    /// Whether the button is the row's primary.
    pub primary: bool,
}

impl ProviderActionDef {
    /// "Sign in" primary button.
    pub fn sign_in() -> Self {
        ProviderActionDef { action: ProviderAction::SignIn, label: "Sign in".into(), primary: true }
    }

    /// "Sign out" button.
    pub fn sign_out() -> Self {
        ProviderActionDef { action: ProviderAction::SignOut, label: "Sign out".into(), primary: false }
    }

    /// "Re-check" button.
    pub fn recheck() -> Self {
        ProviderActionDef { action: ProviderAction::Recheck, label: "Re-check".into(), primary: false }
    }

    /// "Install" primary button.
    pub fn install() -> Self {
        ProviderActionDef { action: ProviderAction::Install, label: "Install".into(), primary: true }
    }

    /// "Docs" button.
    pub fn docs() -> Self {
        ProviderActionDef { action: ProviderAction::Docs, label: "Docs".into(), primary: false }
    }
}

/// The data behind [`provider_card`]: the same shape the connect screen's
/// compact rows render from (see [`ConnectRowData`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCardData {
    /// Stable identity, reported by the intent callback.
    pub id: SharedString,
    /// Which provider this row is for.
    pub provider: Provider,
    /// The status headline.
    pub headline: ProviderHeadline,
    /// `"Signed in as <email> · <plan>"`, if known.
    pub account: Option<SharedString>,
    /// The installed version, if known.
    pub version: Option<SharedString>,
    /// An advisory on the version (`"Too old — need ≥ 2.0"`), if any.
    pub advisory: Option<SharedString>,
    /// The person's enabled switch.
    pub enabled: bool,
    /// Up to three buttons; extras are ignored, never wrapped.
    pub actions: Vec<ProviderActionDef>,
}

type ProviderHandler = Rc<dyn Fn(ProviderIntent, &mut Window, &mut App)>;

/// A Settings provider card: mark, name, status dot + headline, account
/// line, version chip with optional advisory, enabled switch, and up to
/// three action buttons. Build with [`provider_card`].
#[derive(IntoElement)]
pub struct ProviderCard {
    id: ElementId,
    data: ProviderCardData,
    on_intent: Option<ProviderHandler>,
}

/// A provider card for a settings page, over `data`.
pub fn provider_card(id: impl Into<ElementId>, data: &ProviderCardData) -> ProviderCard {
    ProviderCard { id: id.into(), data: data.clone(), on_intent: None }
}

impl ProviderCard {
    /// The intent callback: every button and the enabled switch report
    /// through it with the row id.
    pub fn on_intent(mut self, f: impl Fn(ProviderIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

/// The dot colour for a headline: success for Connected, danger for Can't
/// run, warning for Signed out / Unverified, muted ink for the rest.
fn headline_dot(headline: ProviderHeadline, p: &aui_tokens::Palette) -> gpui::Hsla {
    match headline {
        ProviderHeadline::Connected => p.success,
        ProviderHeadline::CantRun => p.danger,
        ProviderHeadline::SignedOut | ProviderHeadline::Unverified => p.warning,
        ProviderHeadline::Checking | ProviderHeadline::NotInstalled | ProviderHeadline::Disabled => {
            p.ink_3
        }
    }
}

impl RenderOnce for ProviderCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let data = self.data;
        let name = provider_display_name(data.provider);

        // Status: a subtle spinner while checking (never "Not installed"
        // while a probe is pending), else the dot + headline.
        let status: AnyElement = if data.headline == ProviderHeadline::Checking {
            h_flex()
                .gap(px(ROW_GAP))
                .items_center()
                .child(spinner((self.id.clone(), "checking")).size(px(SPIN)))
                .child(
                    div()
                        .text_px(HEADLINE_TEXT)
                        .line_height(gpui::relative(scale::LH_UI))
                        .text_color(p.ink_2)
                        .child(ProviderHeadline::Checking.label()),
                )
                .into_any_element()
        } else {
            h_flex()
                .gap(px(ROW_GAP))
                .items_center()
                .child(
                    div()
                        .flex_none()
                        .w(px(DOT))
                        .h(px(DOT))
                        .rounded_full()
                        .bg(headline_dot(data.headline, &p)),
                )
                .child(
                    div()
                        .text_px(HEADLINE_TEXT)
                        .line_height(gpui::relative(scale::LH_UI))
                        .text_color(p.ink_2)
                        .child(data.headline.label()),
                )
                .into_any_element()
        };

        let mut body = v_flex().gap(px(2.0)).child(
            div()
                .text_px(NAME_TEXT)
                .line_height(gpui::relative(scale::LH_UI))
                .text_color(p.ink)
                .medium()
                .child(name.clone()),
        ).child(status);
        if let Some(account) = data.account.clone() {
            body = body.child(
                div()
                    .text_px(DETAIL_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(account),
            );
        }
        if data.version.is_some() || data.advisory.is_some() {
            let mut ver = h_flex().gap(px(ROW_GAP)).items_center();
            if let Some(version) = data.version.clone() {
                ver = ver.child(tag(version));
            }
            if let Some(advisory) = data.advisory.clone() {
                ver = ver.child(
                    div()
                        .text_px(DETAIL_TEXT)
                        .line_height(gpui::relative(scale::LH_UI))
                        .text_color(p.warning)
                        .child(advisory),
                );
            }
            body = body.child(ver);
        }

        let mut actions = h_flex().gap(px(ROW_GAP)).items_center().flex_wrap();
        for def in data.actions.iter().take(3) {
            let row_id = data.id.clone();
            let action = def.action;
            let label = format!("{} · {}", name, def.label);
            let mut btn = button((self.id.clone(), def.label.clone()), def.label.clone())
                .size(ButtonSize::Sm)
                .accessibility_label(label);
            if def.primary {
                btn = btn.primary();
            }
            if let Some(handler) = self.on_intent.clone() {
                btn = btn.on_click(move |_, w, cx| {
                    handler(ProviderIntent { id: row_id.clone(), action }, w, cx);
                });
            }
            actions = actions.child(btn);
        }

        let switch_id = data.id.clone();
        let switch_label: SharedString = format!("Enable {}", name).into();
        let mut toggle = Switch::new((self.id.clone(), "enabled"))
            .checked(data.enabled)
            .color(p.accent)
            .accessibility_label(switch_label);
        if let Some(handler) = self.on_intent.clone() {
            toggle = toggle.on_click(move |next, w, cx| {
                handler(
                    ProviderIntent { id: switch_id.clone(), action: ProviderAction::SetEnabled(*next) },
                    w,
                    cx,
                );
            });
        }

        h_flex()
            .id(self.id)
            .w_full()
            .items_start()
            .gap(px(CARD_GAP))
            .p(px(CARD_PAD))
            .child(provider_mark(data.provider).size(px(MARK)))
            .child(div().flex_1().min_w(px(0.0)).child(body))
            .child(v_flex().gap(px(ROW_GAP)).items_end().child(toggle).child(actions))
    }
}

/// One row of the connect screen: the card data in compact form — one
/// primary action and an optional Docs secondary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectRowData {
    /// Stable identity, reported by the intent callback.
    pub id: SharedString,
    /// Which provider this row is for.
    pub provider: Provider,
    /// The status headline.
    pub headline: ProviderHeadline,
    /// `"Signed in as <email> · <plan>"`, if known.
    pub account: Option<SharedString>,
    /// The row's primary action (`Install` / `Sign in` / `Re-check`);
    /// `None` for Connected rows, which show a static `✓ Connected`.
    pub primary: Option<ProviderActionDef>,
    /// Whether the secondary Docs button shows.
    pub docs: bool,
}

/// What the person asked for on the connect screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectIntent {
    /// A row button or switch.
    Provider(ProviderIntent),
    /// The primary Continue.
    Continue,
    /// "Skip for now".
    Skip,
}

type ConnectHandler = Rc<dyn Fn(ConnectIntent, &mut Window, &mut App)>;

/// The first-run Connect screen: app mark, title, one intro line, a tally
/// line, one compact row per provider, a primary Continue (enabled once any
/// provider is Connected) and a "Skip for now" link. Build with
/// [`connect_providers`].
#[derive(IntoElement)]
pub struct ConnectScreen {
    id: ElementId,
    title: SharedString,
    intro: SharedString,
    mark: Option<AnyElement>,
    rows: Vec<ConnectRowData>,
    on_intent: Option<ConnectHandler>,
}

/// The connect screen over `rows`.
pub fn connect_providers(id: impl Into<ElementId>, rows: Vec<ConnectRowData>) -> ConnectScreen {
    ConnectScreen {
        id: id.into(),
        title: "Connect your providers".into(),
        intro: "Connect at least one provider to start working.".into(),
        mark: None,
        rows,
        on_intent: None,
    }
}

impl ConnectScreen {
    /// Overrides the title.
    pub fn title(mut self, text: impl Into<SharedString>) -> Self {
        self.title = text.into();
        self
    }

    /// Overrides the one-line intro under the title.
    pub fn intro(mut self, text: impl Into<SharedString>) -> Self {
        self.intro = text.into();
        self
    }

    /// The app mark above the title (Baaz passes its own mark; the library
    /// draws no product glyph itself).
    pub fn mark(mut self, mark: impl IntoElement) -> Self {
        self.mark = Some(mark.into_any_element());
        self
    }

    /// The intent callback: row buttons, Continue and Skip all report
    /// through it.
    pub fn on_intent(mut self, f: impl Fn(ConnectIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

/// The tally line: `"1 connected · 1 needs sign-in · 1 not installed"`.
/// Only non-zero segments show. Checking rows are omitted (they resolve
/// without the person reading them as a state) as are Disabled rows (a
/// choice, not a status).
pub fn connect_tally(rows: &[ConnectRowData]) -> String {
    let mut connected = 0;
    let mut signin = 0;
    let mut missing = 0;
    let mut cant = 0;
    for row in rows {
        match row.headline {
            ProviderHeadline::Connected => connected += 1,
            ProviderHeadline::SignedOut | ProviderHeadline::Unverified => signin += 1,
            ProviderHeadline::NotInstalled => missing += 1,
            ProviderHeadline::CantRun => cant += 1,
            ProviderHeadline::Checking | ProviderHeadline::Disabled => {}
        }
    }
    let mut parts = Vec::new();
    if connected > 0 {
        parts.push(format!("{connected} connected"));
    }
    if signin > 0 {
        parts.push(format!("{signin} needs sign-in"));
    }
    if missing > 0 {
        parts.push(format!("{missing} not installed"));
    }
    if cant > 0 {
        parts.push(format!("{cant} can't run"));
    }
    if parts.is_empty() {
        "Checking providers…".to_string()
    } else {
        parts.join(" · ")
    }
}

impl RenderOnce for ConnectScreen {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let any_connected = self.rows.iter().any(|r| r.headline == ProviderHeadline::Connected);
        let tally = connect_tally(&self.rows);

        let mut rows_el = v_flex().gap(px(ROW_GAP)).w_full();
        for row in &self.rows {
            rows_el = rows_el.child(connect_row(&self.id, row, &p, self.on_intent.clone()));
        }

        let mut cont =
            button((self.id.clone(), "continue"), "Continue").accessibility_label("Continue");
        cont = cont.primary().disabled(!any_connected);
        if let Some(handler) = self.on_intent.clone() {
            cont = cont.on_click(move |_, w, cx| handler(ConnectIntent::Continue, w, cx));
        }
        let skip_handler = self.on_intent.clone();
        let skip_id = self.id.clone();
        let skip = div()
            .id((skip_id, "skip"))
            .text_px(DETAIL_TEXT)
            .line_height(gpui::relative(scale::LH_UI))
            .text_color(p.ink_3)
            .cursor_pointer()
            .role(gpui::Role::Link)
            .aria_label("Skip for now")
            .on_click(move |_, w, cx| {
                if let Some(handler) = skip_handler.clone() {
                    handler(ConnectIntent::Skip, w, cx);
                }
            })
            .child("Skip for now");

        let mut masthead = v_flex().gap(px(ROW_GAP)).items_center().text_center();
        if let Some(mark) = self.mark {
            masthead = masthead.child(mark);
        }
        masthead = masthead
            .child(
                div()
                    .text_px(scale::FS_18)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink)
                    .medium()
                    .child(self.title),
            )
            .child(
                div()
                    .text_px(NAME_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_2)
                    .child(self.intro),
            )
            .child(
                div()
                    .text_px(DETAIL_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(tally),
            );

        div()
            .id(self.id)
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(p.bg)
            .child(
                v_flex()
                    .w(px(CONNECT_W))
                    .max_w_full()
                    .gap(px(CARD_GAP))
                    .p(px(scale::SP_6))
                    .child(masthead)
                    .child(rows_el)
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap(px(CARD_GAP))
                            .child(skip)
                            .child(cont),
                    ),
            )
    }
}

/// One compact connect row: mark, name, status (spinner while checking),
/// account line, one primary action plus Docs.
fn connect_row(
    id: &ElementId,
    row: &ConnectRowData,
    p: &aui_tokens::Palette,
    on_intent: Option<ConnectHandler>,
) -> AnyElement {
    let name = provider_display_name(row.provider);
    let status: AnyElement = if row.headline == ProviderHeadline::Checking {
        h_flex()
            .gap(px(ROW_GAP))
            .items_center()
            .child(spinner((id.clone(), SharedString::from(format!("{}-checking", row.id)))).size(px(SPIN)))
            .child(
                div()
                    .text_px(HEADLINE_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_2)
                    .child(ProviderHeadline::Checking.label()),
            )
            .into_any_element()
    } else {
        h_flex()
            .gap(px(ROW_GAP))
            .items_center()
            .child(
                div()
                    .flex_none()
                    .w(px(DOT))
                    .h(px(DOT))
                    .rounded_full()
                    .bg(headline_dot(row.headline, p)),
            )
            .child(
                div()
                    .text_px(HEADLINE_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_2)
                    .child(row.headline.label()),
            )
            .into_any_element()
    };

    let mut text = v_flex().gap(px(2.0)).child(
        div()
            .text_px(NAME_TEXT)
            .line_height(gpui::relative(scale::LH_UI))
            .text_color(p.ink)
            .medium()
            .child(name.clone()),
    ).child(status);
    if let Some(account) = row.account.clone() {
        text = text.child(
            div()
                .text_px(DETAIL_TEXT)
                .line_height(gpui::relative(scale::LH_UI))
                .text_color(p.ink_3)
                .child(account),
        );
    }

    let mut actions = h_flex().gap(px(ROW_GAP)).items_center().flex_none();
    if let Some(def) = row.primary.clone() {
        let row_id = row.id.clone();
        let action = def.action;
        let label = format!("{} · {}", name, def.label);
        let mut btn = button((id.clone(), SharedString::from(format!("{}-primary", row.id))), def.label.clone())
            .size(ButtonSize::Sm)
            .accessibility_label(label);
        if def.primary {
            btn = btn.primary();
        }
        if let Some(handler) = on_intent.clone() {
            btn = btn.on_click(move |_, w, cx| {
                handler(
                    ConnectIntent::Provider(ProviderIntent { id: row_id.clone(), action }),
                    w,
                    cx,
                );
            });
        }
        actions = actions.child(btn);
    } else if row.headline == ProviderHeadline::Connected {
        actions = actions.child(
            div()
                .text_px(HEADLINE_TEXT)
                .line_height(gpui::relative(scale::LH_UI))
                .text_color(p.success)
                .medium()
                .child("✓ Connected"),
        );
    }
    if row.docs {
        let row_id = row.id.clone();
        let label = format!("{} · Docs", name);
        let mut docs = button((id.clone(), SharedString::from(format!("{}-docs", row.id))), "Docs")
            .size(ButtonSize::Sm)
            .accessibility_label(label);
        if let Some(handler) = on_intent.clone() {
            docs = docs.on_click(move |_, w, cx| {
                handler(
                    ConnectIntent::Provider(ProviderIntent {
                        id: row_id.clone(),
                        action: ProviderAction::Docs,
                    }),
                    w,
                    cx,
                );
            });
        }
        actions = actions.child(docs);
    }

    h_flex()
        .id((id.clone(), row.id.clone()))
        .w_full()
        .items_center()
        .gap(px(CARD_GAP))
        .p(px(CARD_PAD))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(p.line)
        .child(provider_mark(row.provider).size(px(MARK)))
        .child(div().flex_1().min_w(px(0.0)).child(text))
        .child(actions)
        .into_any_element()
}

/// One usage window: a labelled quota bar.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageWindow {
    /// The window label (`"Weekly"`, `"Primary · 5h"`, …).
    pub label: SharedString,
    /// Used fraction in `0..=1`.
    pub used_fraction: f32,
    /// `"resets in 3h"`, shown under the bar.
    pub resets_at_text: SharedString,
}

impl UsageWindow {
    /// A window over `used_fraction` (`0..=1`, clamped on render).
    pub fn new(
        label: impl Into<SharedString>,
        used_fraction: f32,
        resets_at_text: impl Into<SharedString>,
    ) -> Self {
        UsageWindow {
            label: label.into(),
            used_fraction,
            resets_at_text: resets_at_text.into(),
        }
    }
}

/// One compact usage row in the account menu: a provider, its plan label
/// and either quota windows or an unavailable reason. The account menu
/// renders these without card chrome; see [`usage_card`] for the standalone
/// popover card over the same data.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageRowData {
    /// Which provider this row is for.
    pub provider: Provider,
    /// The plan label beside the provider name (`"Max"`, `"Pro"`, …), if known.
    pub plan: Option<SharedString>,
    /// Quota windows with their footnote, or why there is nothing to show yet.
    pub state: UsageRowState,
}

impl UsageRowData {
    /// A row for `provider` in `state`, with no plan label.
    pub fn new(provider: Provider, state: UsageRowState) -> Self {
        UsageRowData { provider, plan: None, state }
    }

    /// The plan label beside the provider name (`"Max"`, `"Pro"`, …).
    pub fn plan(mut self, plan: impl Into<SharedString>) -> Self {
        self.plan = Some(plan.into());
        self
    }
}

/// The body of a [`UsageRowData`]: quota windows, or why there is nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum UsageRowState {
    /// One quota bar per window, with the `"as of …"` footnote when known.
    Windows(Vec<UsageWindow>, Option<SharedString>),
    /// No reading yet: the reason draws as one wrapping muted line.
    Unavailable(SharedString),
}

/// Whether `used_fraction` wears the warning ink (at or above 80 %).
/// Both [`usage_card`] and the account menu's compact rows read through
/// this so the two surfaces agree on the threshold.
pub fn usage_warns(used_fraction: f32) -> bool {
    used_fraction >= WARN_AT
}

/// A per-provider usage card for the account menu popover (~300 px):
/// provider mark + name + plan, one bar per window with "resets in …", an
/// "as of …" footnote, warning ink at ≥ 80%, and a "Not reported yet" empty
/// state. Build with [`usage_card`].
#[derive(IntoElement)]
pub struct UsageCard {
    id: ElementId,
    provider: Provider,
    plan: Option<SharedString>,
    windows: Vec<UsageWindow>,
    as_of: Option<SharedString>,
}

/// A usage card for `provider` with `windows` quota bars.
pub fn usage_card(
    id: impl Into<ElementId>,
    provider: Provider,
    windows: Vec<UsageWindow>,
) -> UsageCard {
    UsageCard { id: id.into(), provider, plan: None, windows, as_of: None }
}

impl UsageCard {
    /// The plan label beside the provider name (`"Pro"`, `"Business"`, …).
    pub fn plan(mut self, plan: impl Into<SharedString>) -> Self {
        self.plan = Some(plan.into());
        self
    }

    /// The footnote (`"as of 2m ago"`): when the numbers were read.
    pub fn as_of(mut self, as_of: impl Into<SharedString>) -> Self {
        self.as_of = Some(as_of.into());
        self
    }
}

impl RenderOnce for UsageCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let name = provider_display_name(self.provider);

        let mut head = h_flex().gap(px(ROW_GAP)).items_center().child(
            provider_mark(self.provider).size(px(16.0)),
        ).child(
            div()
                .text_px(NAME_TEXT)
                .line_height(gpui::relative(scale::LH_UI))
                .text_color(p.ink)
                .medium()
                .child(name),
        );
        if let Some(plan) = self.plan {
            head = head.child(
                div()
                    .text_px(DETAIL_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(plan),
            );
        }

        let mut card = v_flex()
            .id(self.id)
            .w(px(USAGE_W))
            .max_w_full()
            .gap(px(ROW_GAP))
            .p(px(CARD_PAD))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .child(head);

        if self.windows.is_empty() {
            card = card.child(
                div()
                    .text_px(DETAIL_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child("Not reported yet"),
            );
        }
        for window in &self.windows {
            let used = window.used_fraction.clamp(0.0, 1.0);
            let bar_ink = if used >= WARN_AT { p.warning } else { p.ink_3 };
            card = card.child(
                v_flex().gap(px(2.0)).w_full().child(
                    h_flex().w_full().items_baseline().justify_between().gap(px(ROW_GAP)).child(
                        div()
                            .text_px(DETAIL_TEXT)
                            .line_height(gpui::relative(scale::LH_UI))
                            .text_color(p.ink_2)
                            .child(window.label.clone()),
                    ).child(
                        div()
                            .font_family(scale::FONT_MONO)
                            .text_px(DETAIL_TEXT)
                            .line_height(gpui::relative(scale::LH_UI))
                            .text_color(bar_ink)
                            .child(format!("{}%", (used * 100.0).round())),
                    ),
                ).child(
                    div()
                        .w_full()
                        .h(px(BAR_H))
                        .rounded(px(BAR_R))
                        .bg(p.surface_3)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .rounded(px(BAR_R))
                                .bg(bar_ink)
                                .w(gpui::relative(used)),
                        ),
                ).child(
                    div()
                        .text_px(DETAIL_TEXT)
                        .line_height(gpui::relative(scale::LH_UI))
                        .text_color(p.ink_3)
                        .child(format!("resets in {}", window.resets_at_text)),
                ),
            );
        }
        if let Some(as_of) = self.as_of {
            card = card.child(
                div()
                    .text_px(DETAIL_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(format!("as of {as_of}")),
            );
        }
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card_data(headline: ProviderHeadline) -> ProviderCardData {
        ProviderCardData {
            id: "claude".into(),
            provider: Provider::Claude,
            headline,
            account: Some("Signed in as ada@example.com · Pro".into()),
            version: Some("2.1.276".into()),
            advisory: None,
            enabled: true,
            actions: vec![ProviderActionDef::sign_out(), ProviderActionDef::recheck()],
        }
    }

    #[test]
    fn headlines_read_as_designed() {
        assert_eq!(ProviderHeadline::Checking.label(), "Checking…");
        assert_eq!(ProviderHeadline::Connected.label(), "Connected");
        assert_eq!(ProviderHeadline::SignedOut.label(), "Signed out");
        assert_eq!(ProviderHeadline::NotInstalled.label(), "Not installed");
        assert_eq!(ProviderHeadline::CantRun.label(), "Can't run");
        assert_eq!(
            ProviderHeadline::Unverified.label(),
            "Installed · sign-in not verified"
        );
        assert_eq!(ProviderHeadline::Disabled.label(), "Disabled");
    }

    #[test]
    fn tally_counts_connected_signin_and_missing() {
        let rows = vec![
            ConnectRowData {
                id: "a".into(),
                provider: Provider::Claude,
                headline: ProviderHeadline::Connected,
                account: None,
                primary: None,
                docs: false,
            },
            ConnectRowData {
                id: "b".into(),
                provider: Provider::Codex,
                headline: ProviderHeadline::SignedOut,
                account: None,
                primary: Some(ProviderActionDef::sign_in()),
                docs: true,
            },
            ConnectRowData {
                id: "c".into(),
                provider: Provider::Muse,
                headline: ProviderHeadline::NotInstalled,
                account: None,
                primary: Some(ProviderActionDef::install()),
                docs: true,
            },
        ];
        assert_eq!(
            connect_tally(&rows),
            "1 connected · 1 needs sign-in · 1 not installed"
        );
    }

    #[test]
    fn tally_skips_checking_and_empty_reports_checking() {
        let rows = vec![ConnectRowData {
            id: "a".into(),
            provider: Provider::Claude,
            headline: ProviderHeadline::Checking,
            account: None,
            primary: None,
            docs: false,
        }];
        // Checking rows never read as "Not installed".
        assert_eq!(connect_tally(&rows), "Checking providers…");
    }

    #[test]
    fn usage_windows_clamp_and_warn_at_eighty_percent() {
        assert!((WARN_AT - 0.8).abs() < f32::EPSILON);
        let w = UsageWindow::new("Weekly", 1.4, "2d");
        assert!((w.used_fraction - 1.4).abs() < f32::EPSILON, "stored raw, clamped on render");
        let _ = card_data(ProviderHeadline::Connected);
    }

    #[test]
    fn usage_warns_fires_at_eighty_percent() {
        assert!(!usage_warns(0.799));
        assert!(usage_warns(0.8));
        assert!(usage_warns(1.4));
    }

    #[test]
    fn usage_row_data_keeps_provider_plan_and_state() {
        let row = UsageRowData::new(
            Provider::Claude,
            UsageRowState::Windows(vec![UsageWindow::new("5h", 0.42, "2h 10m")], None),
        )
        .plan("Max");
        assert_eq!(row.provider, Provider::Claude);
        assert_eq!(row.plan, Some("Max".into()));
        match &row.state {
            UsageRowState::Windows(windows, as_of) => {
                assert_eq!(windows.len(), 1);
                assert_eq!(*as_of, None);
            }
            UsageRowState::Unavailable(_) => panic!("expected windows"),
        }
        let missing = UsageRowData::new(Provider::Muse, UsageRowState::Unavailable("No reading yet".into()));
        assert_eq!(missing.plan, None);
    }
}
