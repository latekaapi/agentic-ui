//! Full-window screens: the whole window is the component, not a card inside
//! one.
//!
//! A screen owns the window ground and centres its own content on it, so an
//! app can hand a window straight to one before its shell exists. The only
//! screen so far is [`login`], the device-code sign-in the app shows before
//! the first session opens.

mod login;

pub use login::{login, Login, LoginIntent, LoginState};
