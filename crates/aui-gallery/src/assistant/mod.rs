//! The assistant mock: the day-job assistant's three-pane shell assembled
//! from the library, with sample roles, a grounded answer and a document
//! pane. Lives in the gallery as the `screens/assistant` entry; the real app
//! wires the same components to its own state.
//!
//! [`model`] holds the transcript the view renders, [`script`] plays a turn out
//! over timers, and [`view`] is the shell, the overlays and the keyboard.

pub mod model;
pub mod script;
mod view;

pub use view::{build, build_all};
