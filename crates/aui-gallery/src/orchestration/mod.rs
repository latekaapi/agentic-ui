//! The orchestration gallery screen: scenarios A–F in a three-pane shell.
//!
//! [`model`] holds the declarative snapshots the view renders, [`script`]
//! plays timed behaviour (auto-advance, workflow retries), and [`view`] is
//! the chrome strip, the shell and the `AUI_GALLERY_STEPS` harness.

pub mod model;
pub mod script;
mod view;

pub use view::build;
