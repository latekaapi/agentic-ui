//! Provider surfaces: the first-run connect screen, the Settings provider
//! card with its Details, the signed-out banner and the account-menu usage
//! card (`docs/23-providers-connect.md` §4, redrawn as the D5 Providers board,
//! harness docs/66 §3).
//!
//! Everything here is stateless like [`login`](super::login): the caller owns
//! the statuses (it runs the probes, caches them and words the status line)
//! and receives intent callbacks back. Nothing here opens a browser, runs an
//! installer, copies to the clipboard or writes state.
//!
//! Five concepts, no more (docs/65 §3): a **provider**, its **status line**
//! ([`StatusLine`]: a tone and one line of text), its **one next action**
//! ([`ProviderActionDef`]), the **setup sheet** where that action happens
//! ([`setup_sheet`](super::setup_sheet)) and the **default provider** (the
//! connect screen's footer picker, [`DefaultPicker`]).
//!
//! Every interactive element carries a role and a human label: buttons via
//! [`Button::accessibility_label`](crate::data::Button::accessibility_label)
//! (which sets `role=Button`), the default picker and the Details disclosure
//! as buttons with their state in the label, and the assist line's links as
//! `role=Link`.
//!
//! ```ignore
//! use aui::screens::{provider_card, ProviderActionDef, ProviderCardData, StatusLine};
//!
//! provider_card("card-claude", &ProviderCardData::new(
//!     "claude",
//!     aui::icons::Provider::Claude,
//!     StatusLine::ok("Connected · ada@example.com · Max"),
//! )
//! .actions(vec![ProviderActionDef::recheck_text(), ProviderActionDef::sign_out()]))
//! .on_intent(|intent, _, _| println!("{} -> {:?}", intent.id, intent.action))
//! ```

use std::rc::Rc;

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::composer::{provider_display_name, provider_menu, PickerRow};
use crate::data::{button, spinner, ButtonSize};

// The board's measures (harness `docs/34-refs/v3-redo/canvas/gen_d5p.py`,
// the `p5*` CSS). Text sizes are design pixels: the app's 1.1 scale turns
// 13 into 14.3, 12 into 13.2, 18 into 19.8. Line heights are the board's
// absolute pixels.

/// The connect column.
const CONNECT_W: f32 = 440.0;
/// The column's gap between masthead, rows and footer.
const COLUMN_GAP: f32 = 8.0;
/// Title 19.8/26 semibold.
const TITLE_TEXT: f32 = scale::FS_18;
const TITLE_LH: f32 = 26.0;
/// Body 14.3/20: names and the intro.
const BODY_TEXT: f32 = scale::FS_13;
const BODY_LH: f32 = 20.0;
/// Caption 13.2/18: status lines, footers, details.
const CAPTION_TEXT: f32 = scale::FS_12;
const CAPTION_LH: f32 = 18.0;
/// Mono 12.65: paths and codes.
const MONO_TEXT: f32 = 11.5;
/// The rows container and the card: radius 10.
const PANEL_R: f32 = 10.0;
/// A connect row: at least 60 tall, padded 12/14, gap 12.
const ROW_MIN_H: f32 = 60.0;
const ROW_PAD_Y: f32 = 12.0;
const ROW_PAD_X: f32 = 14.0;
const ROW_GAP: f32 = 12.0;
/// The provider mark in rows and cards.
const MARK: f32 = 24.0;
/// The status dot and the gap after it.
const DOT: f32 = 6.0;
const DOT_GAP: f32 = 6.0;
/// The checking spinner.
const SPIN: f32 = 12.0;
/// Buttons in a cluster.
const BUTTON_GAP: f32 = 8.0;
/// The default picker: 24 tall, radius 6, a 16 px mark.
const PICKER_H: f32 = 24.0;
const PICKER_R: f32 = 6.0;
const PICKER_MARK: f32 = 16.0;
/// The footer's "nothing ready" note wraps at this width.
const NOTHING_W: f32 = 230.0;
const NOTHING_LH: f32 = 17.0;
/// The card's top row padding and the Details block's.
const CARD_PAD: f32 = 14.0;
/// Details: a 96 px key column, gaps 6/12.
const KEY_W: f32 = 96.0;
const KV_GAP_Y: f32 = 6.0;
const KV_GAP_X: f32 = 12.0;
/// The advisory box inside Details and the banner: radius 8.
const BOX_R: f32 = 8.0;
/// The usage card's popover width.
const USAGE_W: f32 = 300.0;
/// Usage fills at or above this fraction wear the warning ink.
const WARN_AT: f32 = 0.8;
/// The usage card's own measures (unchanged by D5).
const USAGE_PAD: f32 = scale::SP_4;
const USAGE_GAP: f32 = scale::SP_2;
const NAME_TEXT: f32 = scale::FS_13;
const DETAIL_TEXT: f32 = scale::FS_11;
const BAR_H: f32 = 4.0;
const BAR_R: f32 = 2.0;

/// A click reported with the window (row buttons, the banner's action).
type ClickFn = Rc<dyn Fn(&mut Window, &mut App)>;
/// A link reported by its key (assist lines).
type LinkFn = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

// ------------------------------------------------------------ status

/// A provider's headline state, first match wins (design §2): Checking →
/// Disabled → Not installed → Can't run → Signed out → Installed ·
/// sign-in not verified → Connected. Hosts word the status line themselves
/// ([`StatusLine`]); this names the state and its default tone.
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
    /// The short state name (Settings' overview rows, screen readers).
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

    /// The dot this state wears: ok when Connected, needs-action for Signed
    /// out / Unverified, can't-check for Can't run, a spinner while
    /// Checking, absent otherwise.
    pub fn tone(self) -> StatusTone {
        match self {
            ProviderHeadline::Checking => StatusTone::Busy,
            ProviderHeadline::Connected => StatusTone::Ok,
            ProviderHeadline::SignedOut | ProviderHeadline::Unverified => StatusTone::Warn,
            ProviderHeadline::CantRun => StatusTone::Bad,
            ProviderHeadline::NotInstalled | ProviderHeadline::Disabled => StatusTone::Off,
        }
    }
}

/// The mark before a status line: a 6 px dot in one of four inks, or the
/// 12 px spinner while something is in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    /// Connected: the success ink.
    Ok,
    /// Needs an action (signed out, too old, an API-key login): warning.
    Warn,
    /// Absent (not installed, disabled): the faint ink.
    Off,
    /// Couldn't check: danger.
    Bad,
    /// Checking, or waiting on a flow: the spinner instead of a dot.
    Busy,
}

/// One status line: its tone and its words (`"ada@example.com · Max"`,
/// `"Not installed"`, `"Checking…"`). It never wraps; a long line ellipsizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusLine {
    /// The dot or spinner.
    pub tone: StatusTone,
    /// The line.
    pub text: SharedString,
}

impl StatusLine {
    /// A line in `tone`.
    pub fn new(tone: StatusTone, text: impl Into<SharedString>) -> Self {
        StatusLine { tone, text: text.into() }
    }

    /// A Connected line.
    pub fn ok(text: impl Into<SharedString>) -> Self {
        Self::new(StatusTone::Ok, text)
    }

    /// A needs-action line.
    pub fn warn(text: impl Into<SharedString>) -> Self {
        Self::new(StatusTone::Warn, text)
    }

    /// An absent line.
    pub fn off(text: impl Into<SharedString>) -> Self {
        Self::new(StatusTone::Off, text)
    }

    /// A couldn't-check line.
    pub fn bad(text: impl Into<SharedString>) -> Self {
        Self::new(StatusTone::Bad, text)
    }

    /// A line with the spinner (`"Checking…"`, `"Finish signing in…"`).
    pub fn busy(text: impl Into<SharedString>) -> Self {
        Self::new(StatusTone::Busy, text)
    }

    /// The headline's own default: its tone and its label.
    pub fn headline(headline: ProviderHeadline) -> Self {
        Self::new(headline.tone(), headline.label())
    }
}

/// The dot colour for a tone (the spinner draws its own).
fn tone_ink(tone: StatusTone, p: &Palette) -> gpui::Hsla {
    match tone {
        StatusTone::Ok => p.success,
        StatusTone::Warn => p.warning,
        StatusTone::Bad => p.danger,
        StatusTone::Off | StatusTone::Busy => p.ink_4,
    }
}

/// The status line: dot (or spinner) + 13.2/18 muted text, one line,
/// ellipsized — never wrapped (the live check found "· Claude Max" wrapping).
fn status_line(id: ElementId, line: &StatusLine, p: &Palette) -> AnyElement {
    let lead: AnyElement = match line.tone {
        StatusTone::Busy => spinner(id).size(px(SPIN)).into_any_element(),
        tone => div().flex_none().size(px(DOT)).rounded_full().bg(tone_ink(tone, p)).into_any_element(),
    };
    h_flex()
        .w_full()
        .min_w(px(0.0))
        .items_center()
        .gap(px(DOT_GAP))
        .child(lead)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_px(CAPTION_TEXT)
                .line_height(px(CAPTION_LH))
                .text_color(p.ink_2)
                .child(line.text.clone()),
        )
        .into_any_element()
}

// ------------------------------------------------------------ actions

/// What the person asked for on a provider row or card, always with its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderAction {
    /// Start the provider's sign-in flow.
    SignIn,
    /// Sign out of the provider (hosts confirm first: a CLI sign-out is a
    /// real `logout` of that CLI).
    SignOut,
    /// Re-run the probes for this provider.
    Recheck,
    /// Open the setup sheet with the vendor's install command typed, not run.
    Install,
    /// Open the setup sheet with the CLI's own update command typed, not run.
    Update,
    /// Abandon a sign-in that is waiting (Codex's in-row flow).
    Cancel,
    /// Open the provider's page in the browser (docs, or "Get Muse ↗").
    Docs,
    /// The enabled switch flipped; the value is the new state.
    SetEnabled(bool),
    /// Open or close the card's Details.
    ToggleDetails,
    /// Details' "Change path…": pick the binary Baaz runs.
    ChangePath,
    /// Forget the chosen binary: back to the automatic search.
    ClearPath,
    /// Details' "Copy diagnostics".
    CopyDiagnostics,
}

/// One provider intent: the row id plus what was pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderIntent {
    /// The row's or card's id.
    pub id: SharedString,
    /// What was pressed.
    pub action: ProviderAction,
}

/// How an action's button is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionStyle {
    /// The light fill: Install, Sign in, Update, Continue, Done.
    Primary,
    /// The tinted button: Sign out, Cancel, Re-check on a row, Get Muse.
    Secondary,
    /// A text button: the card's Re-check, Skip for now.
    Text,
}

/// One action button on a provider row or card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderActionDef {
    /// What the button reports.
    pub action: ProviderAction,
    /// The button label.
    pub label: SharedString,
    /// How the button is drawn.
    pub style: ActionStyle,
    /// Whether it leaves the app (a trailing ↗).
    pub external: bool,
}

impl ProviderActionDef {
    /// An action in `style`.
    pub fn new(action: ProviderAction, label: impl Into<SharedString>, style: ActionStyle) -> Self {
        ProviderActionDef { action, label: label.into(), style, external: false }
    }

    /// "Sign in", primary.
    pub fn sign_in() -> Self {
        Self::new(ProviderAction::SignIn, "Sign in", ActionStyle::Primary)
    }

    /// "Sign out", secondary.
    pub fn sign_out() -> Self {
        Self::new(ProviderAction::SignOut, "Sign out", ActionStyle::Secondary)
    }

    /// "Re-check", secondary: a row whose check failed.
    pub fn recheck() -> Self {
        Self::new(ProviderAction::Recheck, "Re-check", ActionStyle::Secondary)
    }

    /// "Re-check" as a text button: the card's standing re-check.
    pub fn recheck_text() -> Self {
        Self::new(ProviderAction::Recheck, "Re-check", ActionStyle::Text)
    }

    /// "Install", primary.
    pub fn install() -> Self {
        Self::new(ProviderAction::Install, "Install", ActionStyle::Primary)
    }

    /// "Update", primary: below the version floor.
    pub fn update() -> Self {
        Self::new(ProviderAction::Update, "Update", ActionStyle::Primary)
    }

    /// "Cancel", secondary: a sign-in that is waiting.
    pub fn cancel() -> Self {
        Self::new(ProviderAction::Cancel, "Cancel", ActionStyle::Secondary)
    }

    /// "Docs", secondary.
    pub fn docs() -> Self {
        Self::new(ProviderAction::Docs, "Docs", ActionStyle::Secondary)
    }

    /// A secondary link out of the app with a trailing ↗ (`"Get Muse"`).
    pub fn link(label: impl Into<SharedString>) -> Self {
        ProviderActionDef { external: true, ..Self::new(ProviderAction::Docs, label, ActionStyle::Secondary) }
    }

    /// Details' "Change path…".
    pub fn change_path() -> Self {
        Self::new(ProviderAction::ChangePath, "Change path…", ActionStyle::Secondary)
    }

    /// Details' "Copy diagnostics".
    pub fn copy_diagnostics() -> Self {
        Self::new(ProviderAction::CopyDiagnostics, "Copy diagnostics", ActionStyle::Secondary)
    }
}

/// A 28 px row button for `def`, labelled `"<provider> · <label>"` for
/// screen readers, reporting `on_click`.
fn action_button(
    id: impl Into<ElementId>,
    provider_name: &str,
    def: &ProviderActionDef,
    p: &Palette,
    on_click: Option<ClickFn>,
) -> AnyElement {
    let mut btn = button(id, def.label.clone())
        .size(ButtonSize::Row)
        .accessibility_label(format!("{provider_name} · {}", def.label));
    btn = match def.style {
        ActionStyle::Primary => btn.primary(),
        ActionStyle::Secondary => btn,
        ActionStyle::Text => btn.ghost(),
    };
    if def.external {
        btn = btn.trailing(icon(IconName::External).size(px(12.0)).color(p.ink_2));
    }
    if let Some(handler) = on_click {
        btn = btn.on_click(move |_, w, cx| handler(w, cx));
    }
    btn.into_any_element()
}

/// The `✓ Connected` a Connected row shows instead of a button.
fn connected_mark(p: &Palette) -> AnyElement {
    h_flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .text_px(CAPTION_TEXT)
        .line_height(px(CAPTION_LH))
        .text_color(p.success)
        .child(icon(IconName::Check).size(px(13.0)).color(p.success))
        .child("Connected")
        .into_any_element()
}

/// One piece of an assist line: plain muted text, a code in mono, or a
/// link that reports its key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistRun {
    /// Muted text.
    Text(SharedString),
    /// A code or a path, in the mono face and the full ink.
    Code(SharedString),
    /// An underlined link: its key (reported) and its words.
    Link(SharedString, SharedString),
}

/// The one muted line under the rows (or a card's top row): "Browser didn't
/// open? Copy the link · Use a code instead". Links report their key.
fn assist_line(
    id: &ElementId,
    runs: &[AssistRun],
    p: &Palette,
    on_link: Option<LinkFn>,
) -> AnyElement {
    let mut line = h_flex()
        .w_full()
        .flex_wrap()
        .items_center()
        .gap(px(4.0))
        .text_px(CAPTION_TEXT)
        .line_height(px(CAPTION_LH))
        .text_color(p.ink_2);
    for run in runs {
        line = match run {
            AssistRun::Text(text) => line.child(div().child(text.clone())),
            AssistRun::Code(code) => line.child(
                div().font_family(scale::FONT_MONO).text_px(MONO_TEXT).text_color(p.ink).child(code.clone()),
            ),
            AssistRun::Link(key, label) => {
                let handler = on_link.clone();
                let key = key.clone();
                line.child(
                    div()
                        .id((id.clone(), SharedString::from(format!("assist-{key}"))))
                        .cursor_pointer()
                        .underline()
                        .role(gpui::Role::Link)
                        .aria_label(label.clone())
                        .hover(|s| s.text_color(p.ink))
                        .on_click(move |_, w, cx| {
                            if let Some(handler) = handler.clone() {
                                handler(key.clone(), w, cx);
                            }
                        })
                        .child(label.clone()),
                )
            }
        };
    }
    line.into_any_element()
}

// ------------------------------------------------------------ connect screen

/// One row of the connect screen: mark, name, status line, one action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectRowData {
    /// Stable identity, reported by the intent callback.
    pub id: SharedString,
    /// Which provider this row is for.
    pub provider: Provider,
    /// The status line under the name.
    pub status: StatusLine,
    /// The row's one action; `None` draws `✓ Connected` on a connected row
    /// and nothing otherwise (Checking).
    pub action: Option<ProviderActionDef>,
    /// Whether the provider is Connected: it enables Continue, and with no
    /// action the row shows `✓ Connected`. Separate from the tone, because
    /// an API-key login is Connected with a warning dot.
    pub connected: bool,
}

/// The footer's "New sessions start in ⌄" picker: the Connected providers
/// only, the current one checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultPicker {
    /// The provider new sessions start in.
    pub current: Provider,
    /// The providers the menu lists (Connected only), in row order.
    pub choices: Vec<Provider>,
    /// Whether the menu is open (the caller owns it).
    pub open: bool,
}

/// What the person asked for on the connect screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectIntent {
    /// A row button.
    Provider(ProviderIntent),
    /// The primary Continue.
    Continue,
    /// "Skip for now".
    Skip,
    /// The default picker was pressed: open (or close) its menu.
    ToggleDefaultMenu,
    /// A click outside the open menu.
    CloseDefaultMenu,
    /// A provider was picked in the menu.
    SetDefault(Provider),
    /// A link in the assist line, by its key.
    Assist(SharedString),
}

type ConnectHandler = Rc<dyn Fn(ConnectIntent, &mut Window, &mut App)>;

/// The first-run Connect screen (D5 B1): the app's mark, title, one intro
/// line, one rows container, an optional assist line, and the footer —
/// the default picker (or the "nothing is ready" note), "Skip for now" and
/// Continue (enabled once any row is Connected). Build with
/// [`connect_providers`].
#[derive(IntoElement)]
pub struct ConnectScreen {
    id: ElementId,
    title: SharedString,
    intro: SharedString,
    nothing_ready: SharedString,
    mark: Option<AnyElement>,
    rows: Vec<ConnectRowData>,
    assist: Vec<AssistRun>,
    picker: Option<DefaultPicker>,
    on_intent: Option<ConnectHandler>,
}

/// The connect screen over `rows`.
pub fn connect_providers(id: impl Into<ElementId>, rows: Vec<ConnectRowData>) -> ConnectScreen {
    ConnectScreen {
        id: id.into(),
        title: "Connect your providers".into(),
        intro: "Baaz runs the coding agents you already use. Connect at least one to start.".into(),
        nothing_ready: "Nothing is ready yet. Set up later in Settings › Providers.".into(),
        mark: None,
        rows,
        assist: Vec::new(),
        picker: None,
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

    /// Overrides the footer note shown while nothing is Connected.
    pub fn nothing_ready(mut self, text: impl Into<SharedString>) -> Self {
        self.nothing_ready = text.into();
        self
    }

    /// The app mark above the title (Baaz passes its mascot; the library
    /// draws no product glyph itself).
    pub fn mark(mut self, mark: impl IntoElement) -> Self {
        self.mark = Some(mark.into_any_element());
        self
    }

    /// The muted line under the rows ("Browser didn't open? Copy the link ·
    /// Use a code instead"); links report [`ConnectIntent::Assist`].
    pub fn assist(mut self, runs: Vec<AssistRun>) -> Self {
        self.assist = runs;
        self
    }

    /// The footer's default picker. Without one (nothing Connected) the
    /// footer's left side reads the nothing-ready note.
    pub fn default_picker(mut self, picker: Option<DefaultPicker>) -> Self {
        self.picker = picker;
        self
    }

    /// The intent callback: row buttons, the picker, Continue, Skip and the
    /// assist links all report through it.
    pub fn on_intent(mut self, f: impl Fn(ConnectIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

/// Whether Continue is enabled: any row is Connected.
pub fn connect_ready(rows: &[ConnectRowData]) -> bool {
    rows.iter().any(|row| row.connected)
}

impl RenderOnce for ConnectScreen {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let any_connected = connect_ready(&self.rows);
        let on_intent = self.on_intent.clone();

        let mut masthead = v_flex().w_full().items_center().gap(px(COLUMN_GAP));
        if let Some(mark) = self.mark {
            masthead = masthead.child(div().mb(px(4.0)).child(mark));
        }
        masthead = masthead
            .child(
                div()
                    .text_px(TITLE_TEXT)
                    .line_height(px(TITLE_LH))
                    .semibold()
                    .text_color(p.ink)
                    .child(self.title),
            )
            .child(
                div()
                    .mb(px(8.0))
                    .text_center()
                    .text_px(BODY_TEXT)
                    .line_height(px(BODY_LH))
                    .text_color(p.ink_2)
                    .child(self.intro),
            );

        let mut rows_el = v_flex()
            .w_full()
            .rounded(px(PANEL_R))
            .bg(p.surface_2)
            .shadow(p.edge())
            .overflow_hidden();
        for (n, row) in self.rows.iter().enumerate() {
            rows_el = rows_el.child(connect_row(&self.id, row, n > 0, &p, on_intent.clone()));
        }

        let mut column = v_flex()
            .w(px(CONNECT_W))
            .max_w_full()
            .items_center()
            .gap(px(COLUMN_GAP))
            .child(masthead)
            .child(rows_el);
        if !self.assist.is_empty() {
            let handler = on_intent.clone();
            let on_link: LinkFn = Rc::new(move |key, w, cx| {
                if let Some(handler) = handler.clone() {
                    handler(ConnectIntent::Assist(key), w, cx);
                }
            });
            column = column.child(div().w_full().mt(px(6.0)).child(assist_line(&self.id, &self.assist, &p, Some(on_link))));
        }
        column = column.child(footer(&self.id, self.picker, self.nothing_ready, any_connected, &p, on_intent));

        div()
            .id(self.id)
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(p.surface_1)
            .child(column)
    }
}

/// The footer: the default picker (or the nothing-ready note), a spacer,
/// "Skip for now" and Continue.
fn footer(
    id: &ElementId,
    picker: Option<DefaultPicker>,
    nothing_ready: SharedString,
    any_connected: bool,
    p: &Palette,
    on_intent: Option<ConnectHandler>,
) -> AnyElement {
    let left: AnyElement = match picker {
        Some(picker) => default_picker(id, picker, p, on_intent.clone()),
        None => div()
            .max_w(px(NOTHING_W))
            .text_px(CAPTION_TEXT)
            .line_height(px(NOTHING_LH))
            .text_color(p.ink_2)
            .child(nothing_ready)
            .into_any_element(),
    };
    let mut skip = button((id.clone(), "skip"), "Skip for now")
        .size(ButtonSize::Row)
        .ghost()
        .accessibility_label("Skip for now");
    let mut cont = button((id.clone(), "continue"), "Continue")
        .size(ButtonSize::Row)
        .primary()
        .disabled(!any_connected)
        .accessibility_label("Continue");
    if let Some(handler) = on_intent.clone() {
        skip = skip.on_click(move |_, w, cx| handler(ConnectIntent::Skip, w, cx));
    }
    if let Some(handler) = on_intent {
        cont = cont.on_click(move |_, w, cx| handler(ConnectIntent::Continue, w, cx));
    }
    h_flex()
        .w_full()
        .mt(px(8.0))
        .items_center()
        .gap(px(BUTTON_GAP))
        .child(left)
        .child(div().flex_1())
        .child(skip)
        .child(cont)
        .into_any_element()
}

/// "New sessions start in [mark Name ⌄]", the menu hanging above the chip
/// while open.
fn default_picker(id: &ElementId, picker: DefaultPicker, p: &Palette, on_intent: Option<ConnectHandler>) -> AnyElement {
    let name = provider_display_name(picker.current);
    let toggle = on_intent.clone();
    let chip = h_flex()
        .id((id.clone(), "default-picker"))
        .flex_none()
        .h(px(PICKER_H))
        .pl(px(4.0))
        .pr(px(6.0))
        .gap(px(6.0))
        .items_center()
        .rounded(px(PICKER_R))
        .bg(p.surface_4)
        .shadow(p.edge())
        .cursor_pointer()
        .text_color(p.ink)
        .role(gpui::Role::Button)
        .aria_label(format!("New sessions start in {name}. Change"))
        .hover(|s| s.bg(p.surface_5))
        .on_click(move |_, w, cx| {
            if let Some(handler) = toggle.clone() {
                handler(ConnectIntent::ToggleDefaultMenu, w, cx);
            }
        })
        .child(provider_mark(picker.current).size(px(PICKER_MARK)))
        .child(name.clone())
        .child(icon(IconName::ChevronDown).size(px(12.0)).color(p.ink_2));
    let mut anchor = div().relative().flex_none().child(chip);
    if picker.open {
        let rows: Vec<PickerRow> = picker
            .choices
            .iter()
            .map(|provider| PickerRow::new(provider_id(*provider), provider_display_name(*provider), "").mark(*provider))
            .collect();
        let selected = picker.choices.iter().position(|c| *c == picker.current).unwrap_or(0);
        let choices = picker.choices.clone();
        let pick = on_intent.clone();
        let close = on_intent.clone();
        anchor = anchor.child(
            provider_menu((id.clone(), "default-menu"), rows, selected, true)
                .title("New sessions start in")
                .on_pick(move |row, w, cx| {
                    let chosen = choices.iter().copied().find(|c| provider_id(*c) == row.as_ref());
                    if let (Some(handler), Some(chosen)) = (pick.clone(), chosen) {
                        handler(ConnectIntent::SetDefault(chosen), w, cx);
                    }
                })
                .on_close(move |w, cx| {
                    if let Some(handler) = close.clone() {
                        handler(ConnectIntent::CloseDefaultMenu, w, cx);
                    }
                }),
        );
    }
    h_flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .whitespace_nowrap()
        .text_px(CAPTION_TEXT)
        .line_height(px(CAPTION_LH))
        .text_color(p.ink_2)
        .child("New sessions start in")
        .child(anchor)
        .into_any_element()
}

/// A stable id for a provider in the picker menu.
fn provider_id(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
        Provider::Muse => "muse",
        Provider::Gemini => "gemini",
        Provider::Grok => "grok",
        Provider::Pi => "pi",
        Provider::Cursor => "cursor",
    }
}

/// One connect row: mark, name over status line, the one action (or
/// `✓ Connected`). Rows after the first carry a hairline above.
fn connect_row(
    id: &ElementId,
    row: &ConnectRowData,
    rule: bool,
    p: &Palette,
    on_intent: Option<ConnectHandler>,
) -> AnyElement {
    let name = provider_display_name(row.provider);
    let text = v_flex()
        .flex_1()
        .min_w(px(0.0))
        .child(
            div()
                .text_px(BODY_TEXT)
                .line_height(px(BODY_LH))
                .semibold()
                .text_color(p.ink)
                .child(name.clone()),
        )
        .child(status_line((id.clone(), SharedString::from(format!("{}-status", row.id))).into(), &row.status, p));

    let action: Option<AnyElement> = match &row.action {
        Some(def) => {
            let handler = on_intent.map(|handler| {
                let row_id = row.id.clone();
                let action = def.action;
                Rc::new(move |w: &mut Window, cx: &mut App| {
                    handler(ConnectIntent::Provider(ProviderIntent { id: row_id.clone(), action }), w, cx)
                }) as ClickFn
            });
            Some(action_button((id.clone(), SharedString::from(format!("{}-action", row.id))), &name, def, p, handler))
        }
        None if row.connected => Some(connected_mark(p)),
        None => None,
    };

    let mut el = h_flex()
        .id((id.clone(), row.id.clone()))
        .w_full()
        .min_h(px(ROW_MIN_H))
        .items_center()
        .gap(px(ROW_GAP))
        .px(px(ROW_PAD_X))
        .py(px(ROW_PAD_Y))
        .child(provider_mark(row.provider).size(px(MARK)))
        .child(text)
        .children(action);
    if rule {
        el = el.border_t_1().border_color(p.line);
    }
    el.into_any_element()
}

// ------------------------------------------------------------ provider card

/// One Details row: a faint key and its value (mono for paths).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailRow {
    /// `"Version"`, `"Path"`, `"Found"`, `"Signed in"`.
    pub key: SharedString,
    /// The value.
    pub value: SharedString,
    /// Whether the value is a path or a command (mono 12.65).
    pub mono: bool,
}

impl DetailRow {
    /// A plain row.
    pub fn new(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        DetailRow { key: key.into(), value: value.into(), mono: false }
    }

    /// A row whose value is a path or a command.
    pub fn mono(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        DetailRow { mono: true, ..Self::new(key, value) }
    }
}

/// The card's Details disclosure (closed by default; the caller owns the
/// open flag and flips it on [`ProviderAction::ToggleDetails`]).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CardDetails {
    /// Whether the disclosure is open.
    pub open: bool,
    /// The key/value rows.
    pub rows: Vec<DetailRow>,
    /// An advisory box under the rows (the API-key billing note), as runs.
    pub notice: Vec<AssistRun>,
    /// The buttons at the bottom ("Change path…", "Copy diagnostics").
    pub actions: Vec<ProviderActionDef>,
}

/// The data behind [`provider_card`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCardData {
    /// Stable identity, reported by the intent callback.
    pub id: SharedString,
    /// Which provider this card is for.
    pub provider: Provider,
    /// The status line under the name.
    pub status: StatusLine,
    /// The top row's buttons, in order (the text Re-check, then the state's
    /// action). Up to three; extras are ignored, never wrapped.
    pub actions: Vec<ProviderActionDef>,
    /// A muted line under the top row (Codex's "Browser didn't open?").
    pub assist: Vec<AssistRun>,
    /// The Details disclosure; `None` draws none.
    pub details: Option<CardDetails>,
}

impl ProviderCardData {
    /// A card with no actions and no Details.
    pub fn new(id: impl Into<SharedString>, provider: Provider, status: StatusLine) -> Self {
        ProviderCardData { id: id.into(), provider, status, actions: Vec::new(), assist: Vec::new(), details: None }
    }

    /// The top row's buttons.
    pub fn actions(mut self, actions: Vec<ProviderActionDef>) -> Self {
        self.actions = actions;
        self
    }

    /// The muted line under the top row.
    pub fn assist(mut self, runs: Vec<AssistRun>) -> Self {
        self.assist = runs;
        self
    }

    /// The Details disclosure.
    pub fn details(mut self, details: CardDetails) -> Self {
        self.details = Some(details);
        self
    }
}

type ProviderHandler = Rc<dyn Fn(ProviderIntent, &mut Window, &mut App)>;

/// A Settings provider card (D5 B4): mark, name over the status line, the
/// text Re-check and the state's action; then the Details disclosure —
/// version, path, how it was found, how it is signed in, an optional
/// advisory box, Change path… and Copy diagnostics. Build with
/// [`provider_card`].
#[derive(IntoElement)]
pub struct ProviderCard {
    id: ElementId,
    data: ProviderCardData,
    on_intent: Option<ProviderHandler>,
    on_assist: Option<LinkFn>,
}

/// A provider card for a settings page, over `data`.
pub fn provider_card(id: impl Into<ElementId>, data: &ProviderCardData) -> ProviderCard {
    ProviderCard { id: id.into(), data: data.clone(), on_intent: None, on_assist: None }
}

impl ProviderCard {
    /// The intent callback: every button and the Details disclosure report
    /// through it with the card id.
    pub fn on_intent(mut self, f: impl Fn(ProviderIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }

    /// A link in the assist line was pressed; the argument is its key.
    pub fn on_assist(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_assist = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ProviderCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let data = self.data;
        let name = provider_display_name(data.provider);
        let on_intent = self.on_intent.clone();
        let report = |action: ProviderAction| -> Option<ClickFn> {
            let handler = on_intent.clone()?;
            let card_id = data.id.clone();
            Some(Rc::new(move |w: &mut Window, cx: &mut App| {
                handler(ProviderIntent { id: card_id.clone(), action }, w, cx)
            }))
        };

        let mut top = h_flex()
            .w_full()
            .items_center()
            .gap(px(ROW_GAP))
            .p(px(CARD_PAD))
            .child(provider_mark(data.provider).size(px(MARK)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(
                        div()
                            .text_px(BODY_TEXT)
                            .line_height(px(BODY_LH))
                            .semibold()
                            .text_color(p.ink)
                            .child(name.clone()),
                    )
                    .child(status_line((self.id.clone(), "status").into(), &data.status, &p)),
            );
        for (n, def) in data.actions.iter().take(3).enumerate() {
            top = top.child(action_button(
                (self.id.clone(), SharedString::from(format!("action-{n}"))),
                &name,
                def,
                &p,
                report(def.action),
            ));
        }

        let mut card = v_flex()
            .id(self.id.clone())
            .w_full()
            .rounded(px(PANEL_R))
            .bg(p.surface_2)
            .shadow(p.edge())
            .child(top);

        if !data.assist.is_empty() {
            card = card.child(
                div()
                    .px(px(CARD_PAD))
                    .pb(px(12.0))
                    .mt(px(-4.0))
                    .child(assist_line(&self.id, &data.assist, &p, self.on_assist.clone())),
            );
        }

        if let Some(details) = data.details {
            card = card.child(details_block(&self.id, &name, &details, &p, report));
        }
        card
    }
}

/// The Details disclosure under a hairline: the chevron + "Details"
/// toggle, then (open) the key/value grid, the advisory box and the buttons.
fn details_block(
    id: &ElementId,
    name: &str,
    details: &CardDetails,
    p: &Palette,
    report: impl Fn(ProviderAction) -> Option<ClickFn>,
) -> AnyElement {
    let toggle = report(ProviderAction::ToggleDetails);
    let chevron = if details.open { IconName::ChevronDown } else { IconName::Chev };
    let head = h_flex()
        .id((id.clone(), "details-toggle"))
        .pt(px(6.0))
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .text_px(CAPTION_TEXT)
        .line_height(px(CAPTION_LH))
        .text_color(p.ink_2)
        .hover(|s| s.text_color(p.ink))
        .role(gpui::Role::Button)
        .aria_label(format!("{name} details, {}", if details.open { "open" } else { "closed" }))
        .on_click(move |_, w, cx| {
            if let Some(handler) = toggle.clone() {
                handler(w, cx);
            }
        })
        .child(icon(chevron).size(px(12.0)).color(p.ink_2))
        .child("Details");
    let mut block = v_flex()
        .w_full()
        .border_t_1()
        .border_color(p.line)
        .px(px(CARD_PAD))
        .pt(px(6.0))
        .pb(px(12.0))
        .child(head);
    if !details.open {
        return block.into_any_element();
    }
    let mut grid = v_flex().w_full().pt(px(8.0)).gap(px(KV_GAP_Y));
    for row in &details.rows {
        let value = div()
            .flex_1()
            .min_w(px(0.0))
            .truncate()
            .text_color(p.ink);
        let value = if row.mono {
            value.font_family(scale::FONT_MONO).text_px(MONO_TEXT)
        } else {
            value
        };
        grid = grid.child(
            h_flex()
                .w_full()
                .gap(px(KV_GAP_X))
                .text_px(CAPTION_TEXT)
                .line_height(px(CAPTION_LH))
                .child(div().flex_none().w(px(KEY_W)).text_color(p.ink_4).child(row.key.clone()))
                .child(value.child(row.value.clone())),
        );
    }
    block = block.child(grid);
    if !details.notice.is_empty() {
        block = block.child(
            h_flex()
                .w_full()
                .mt(px(10.0))
                .items_start()
                .gap(px(8.0))
                .px(px(10.0))
                .py(px(8.0))
                .rounded(px(BOX_R))
                .bg(p.surface_3)
                .shadow(p.edge())
                .child(div().flex_none().mt(px(2.0)).child(icon(IconName::Alert).size(px(13.0)).color(p.warning)))
                .child(div().flex_1().min_w(px(0.0)).child(notice_text(&details.notice, p))),
        );
    }
    if !details.actions.is_empty() {
        let mut row = h_flex().mt(px(12.0)).gap(px(BUTTON_GAP));
        for (n, def) in details.actions.iter().enumerate() {
            row = row.child(action_button(
                (id.clone(), SharedString::from(format!("detail-action-{n}"))),
                name,
                def,
                p,
                report(def.action),
            ));
        }
        block = block.child(row);
    }
    block.into_any_element()
}

/// The advisory box's text: one wrapping paragraph, codes in mono.
fn notice_text(runs: &[AssistRun], p: &Palette) -> AnyElement {
    let mut text = String::new();
    let mut highlights = Vec::new();
    for run in runs {
        let (s, mono) = match run {
            AssistRun::Text(s) | AssistRun::Link(_, s) => (s, false),
            AssistRun::Code(s) => (s, true),
        };
        let start = text.len();
        text.push_str(s);
        if mono {
            highlights.push((start..text.len(), gpui::HighlightStyle { color: Some(p.ink), ..Default::default() }));
        }
    }
    div()
        .text_px(CAPTION_TEXT)
        .line_height(px(CAPTION_LH))
        .text_color(p.ink_2)
        .child(gpui::StyledText::new(text).with_highlights(highlights))
        .into_any_element()
}

// ------------------------------------------------------------ signed-out banner

/// The one quiet banner above the composer when the session's provider is
/// signed out (D5 B5): a warning dot, the line, and Sign in. Build with
/// [`signed_out_banner`].
#[derive(IntoElement)]
pub struct SignedOutBanner {
    id: ElementId,
    text: SharedString,
    action: SharedString,
    on_action: Option<ClickFn>,
}

/// The banner reading `text` (`"Claude Code is signed out. Sign in to send."`).
pub fn signed_out_banner(id: impl Into<ElementId>, text: impl Into<SharedString>) -> SignedOutBanner {
    SignedOutBanner { id: id.into(), text: text.into(), action: "Sign in".into(), on_action: None }
}

impl SignedOutBanner {
    /// Overrides the button's words.
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action = label.into();
        self
    }

    /// The button was pressed.
    pub fn on_action(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SignedOutBanner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut btn = button((self.id.clone(), "action"), self.action.clone())
            .size(ButtonSize::Row)
            .primary()
            .accessibility_label(format!("{} — {}", self.text, self.action));
        if let Some(handler) = self.on_action {
            btn = btn.on_click(move |_, w, cx| handler(w, cx));
        }
        h_flex()
            .id(self.id)
            .w_full()
            .items_center()
            .gap(px(10.0))
            .pl(px(12.0))
            .pr(px(8.0))
            .py(px(8.0))
            .rounded(px(BOX_R))
            .bg(p.surface_3)
            .shadow(p.edge())
            .role(gpui::Role::Status)
            .child(div().flex_none().size(px(DOT)).rounded_full().bg(p.warning))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_px(CAPTION_TEXT)
                    .line_height(px(CAPTION_LH))
                    .text_color(p.ink)
                    .child(self.text),
            )
            .child(btn)
    }
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

        let mut head = h_flex().gap(px(USAGE_GAP)).items_center().child(
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
            .gap(px(USAGE_GAP))
            .p(px(USAGE_PAD))
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
                    h_flex().w_full().items_baseline().justify_between().gap(px(USAGE_GAP)).child(
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

    fn row(provider: Provider, status: StatusLine, action: Option<ProviderActionDef>) -> ConnectRowData {
        let connected = status.tone == StatusTone::Ok;
        ConnectRowData { id: provider_id(provider).into(), provider, status, action, connected }
    }

    #[test]
    fn headlines_name_their_state_and_tone() {
        assert_eq!(ProviderHeadline::Checking.label(), "Checking…");
        assert_eq!(ProviderHeadline::Connected.label(), "Connected");
        assert_eq!(ProviderHeadline::SignedOut.label(), "Signed out");
        assert_eq!(ProviderHeadline::NotInstalled.label(), "Not installed");
        assert_eq!(ProviderHeadline::Checking.tone(), StatusTone::Busy);
        assert_eq!(ProviderHeadline::Connected.tone(), StatusTone::Ok);
        assert_eq!(ProviderHeadline::SignedOut.tone(), StatusTone::Warn);
        assert_eq!(ProviderHeadline::CantRun.tone(), StatusTone::Bad);
        assert_eq!(ProviderHeadline::NotInstalled.tone(), StatusTone::Off);
    }

    /// Continue waits for a Connected row: an Ok line with no action left.
    /// A row still checking, or signed out, never enables it.
    #[test]
    fn continue_needs_a_connected_row() {
        let checking = vec![
            row(Provider::Claude, StatusLine::busy("Checking…"), None),
            row(Provider::Codex, StatusLine::warn("Signed out"), Some(ProviderActionDef::sign_in())),
        ];
        assert!(!connect_ready(&checking));
        let one = vec![row(Provider::Claude, StatusLine::ok("ada@example.com · Max"), None)];
        assert!(connect_ready(&one));
    }

    #[test]
    fn actions_carry_the_boards_styles() {
        assert_eq!(ProviderActionDef::install().style, ActionStyle::Primary);
        assert_eq!(ProviderActionDef::sign_in().style, ActionStyle::Primary);
        assert_eq!(ProviderActionDef::update().style, ActionStyle::Primary);
        assert_eq!(ProviderActionDef::recheck().style, ActionStyle::Secondary);
        assert_eq!(ProviderActionDef::recheck_text().style, ActionStyle::Text);
        assert_eq!(ProviderActionDef::sign_out().style, ActionStyle::Secondary);
        let get = ProviderActionDef::link("Get Muse");
        assert!(get.external);
        assert_eq!(get.style, ActionStyle::Secondary);
        assert_eq!(get.action, ProviderAction::Docs);
    }

    #[test]
    fn usage_windows_clamp_and_warn_at_eighty_percent() {
        assert!((WARN_AT - 0.8).abs() < f32::EPSILON);
        let w = UsageWindow::new("Weekly", 1.4, "2d");
        assert!((w.used_fraction - 1.4).abs() < f32::EPSILON, "stored raw, clamped on render");
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
        let missing = UsageRowData::new(Provider::Muse, UsageRowState::Unavailable("No reading yet".into()));
        assert_eq!(missing.plan, None);
    }
}
