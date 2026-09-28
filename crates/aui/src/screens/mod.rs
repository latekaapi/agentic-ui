//! Full-window screens: the whole window is the component, not a card inside
//! one.
//!
//! A screen owns the window ground and centres its own content on it, so an
//! app can hand a window straight to one before its shell exists. The only
//! screens so far are [`login`], the two-method sign-in the app shows before
//! the first session opens, and the provider surfaces in [`providers`]: the
//! Settings provider card, the first-run connect screen and the usage card.

mod login;
mod providers;

pub use login::{login, Login, LoginIntent, LoginMethod, LoginState};
pub use providers::{
    connect_providers, connect_tally, provider_card, usage_card, ConnectIntent, ConnectRowData,
    ConnectScreen, ProviderAction, ProviderActionDef, ProviderCard, ProviderCardData,
    ProviderHeadline, ProviderIntent, UsageCard, UsageWindow,
};
