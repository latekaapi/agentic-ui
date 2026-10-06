//! The clickable Browse prototype running the PGI story.
//!
//! [`story`] is the state machine the view renders and the timers advance;
//! [`pages`] draws the web pages it visits; `screen` wires the shell,
//! the sidebar, the overlays and the keyboard.

pub mod pages;
pub mod story;
mod screen;

pub use screen::build;
