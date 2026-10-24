//! Full-window screens: the whole window is the component, not a card inside
//! one.
//!
//! A screen owns the window ground and centres its own content on it, so an
//! app can hand a window straight to one before its shell exists. The
//! screens so far are [`login`], the two-method sign-in (also drawn as a
//! sheet), the provider surfaces in [`providers`] — the first-run connect
//! screen, the Settings provider card, the signed-out banner and the usage
//! card — and the [`setup_sheet`] every install and CLI sign-in happens in.
//! [`sidebar`] is the Baaz sidebar redesigned on the v2 tokens: a view model
//! in, intents out.

mod login;
mod providers;
mod setup_sheet;
pub mod sidebar;

pub use login::{login, Login, LoginIntent, LoginMethod, LoginState};
pub use providers::{
    connect_providers, connect_ready, provider_card, signed_out_banner, usage_card, usage_warns,
    ActionStyle, AssistRun, CardDetails, ConnectIntent, ConnectRowData, ConnectScreen,
    DefaultPicker, DetailRow, ProviderAction, ProviderActionDef, ProviderCard, ProviderCardData,
    ProviderHeadline, ProviderIntent, SignedOutBanner, StatusLine, StatusTone, UsageCard,
    UsageRowData, UsageRowState, UsageWindow,
};
pub use setup_sheet::{
    setup_sheet, sheet_frame, sheet_scrim, SetupSheet, SetupStatus, SheetFrame, SETUP_W, SIGN_IN_W,
};
