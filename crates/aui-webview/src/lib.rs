//! `aui-webview` — the embedded browser pane of the workbench (spec §5.2,
//! `design/reference/cards/51-browser.png`).
//!
//! The crate is four layers, and the top one does not know which bottom one it
//! is running on:
//!
//! - [`backend`] — [`WebBackend`], the contract a page fulfils: commands down,
//!   [`WebEvent`]s up on a poll.
//! - [`page`] — the scripted "Simple pricing" document card 51 is drawn over,
//!   plus [`page::fake_elements`], its element boxes as data.
//! - [`fake`] — [`FakeWebBackend`], that document as a backend: real history,
//!   real hit-testing, real annotations, no browser.
//! - [`agent_js`] — ready-made scripts a host can evaluate with a result, so
//!   an agent can read the page and act on it.
//! - [`view`] — [`WebviewState`] and [`webview_pane`], the gpui elements: the
//!   nav row, the page with the annotator's overlays, and the annotations
//!   panel.
//!
//! `wry_backend` (behind the `wry` feature) is the same contract over a real
//! WKWebView parented to the gpui window.
//!
//! # The native-overlay caveat
//!
//! With the real backend the page is a **native overlay composited above the
//! gpui scene**, not a layer inside it. It always paints on top, and gpui
//! cannot draw over it: every popover, menu, tooltip and note bubble that
//! would sit over the page is hidden. Anything over the page must either be
//! positioned outside the webview's bounds or be rendered inside the page
//! through the JS bridge (as `wry_backend::ANNOTATOR_JS` draws the hover
//! outline). The same applies to drag overlays and the command palette while
//! the browser pane is open.
//!
//! None of that constrains [`FakeWebBackend`], whose page is gpui elements —
//! which is why the gallery card can show the note popover over the page at
//! all.
//!
//! # `⌘L` focuses the address field
//!
//! The pane binds [`FocusAddress`] to `cmd-l` in its own key context (see
//! [`WEBVIEW_CONTEXT`] and [`bind_keys`], installed automatically). That
//! binding only fires while gpui holds the keyboard: with a native page
//! focused the keystroke goes to the page and the pane never sees it. A host
//! that wants `⌘L` to work everywhere must bind [`FocusAddress`] once more
//! at window level (no key context) and route it to
//! [`WebviewState::begin_editing`]:
//!
//! ```ignore
//! cx.bind_keys([gpui::KeyBinding::new(
//!     "cmd-l",
//!     aui_webview::FocusAddress,
//!     None,
//! )]);
//! ```

#![warn(missing_docs)]

pub mod agent_js;
pub mod backend;
pub mod fake;
pub mod page;
pub mod view;

#[cfg(feature = "wry")]
pub mod wry_backend;

pub use backend::{ElementInfo, WebBackend, WebEvent};
pub use fake::FakeWebBackend;
pub use page::{fake_elements, FakeElement};
pub use view::{bind_keys, webview_pane, FocusAddress, WebviewIntent, WebviewPane, WebviewState, WEBVIEW_CONTEXT};
#[cfg(feature = "wry")]
pub use wry_backend::{WryBackend, ANNOTATOR_JS};
