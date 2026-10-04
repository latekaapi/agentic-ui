//! `aui-cef` — a Chromium (CEF, Chrome style) backend for the aui-webview
//! browser pane. macOS only.
//!
//! - [`runtime`]: the process lifecycle — [`preload`] at `main()`, a lazy
//!   `CefInitialize` on the first page, the external message pump on the main
//!   run loop, and [`shutdown`] at quit; plus [`run_helper`] for the helper
//!   executable CEF launches for its subprocesses.
//! - [`backend`]: [`CefBackend`], a [`aui_webview::WebBackend`] whose page is
//!   a frameless Chrome-style window adopted as a child of the host window.
//!
//! The host bundles the CEF framework at
//! `App.app/Contents/Frameworks/Chromium Embedded Framework.framework` and
//! five helper apps (`App Helper.app`, `App Helper (GPU|Renderer|Plugin|Alerts).app`)
//! whose executable calls [`run_helper`]. Without the framework, [`preload`]
//! returns false and the host keeps its WebKit backend.

pub mod backend;
pub mod runtime;

pub use backend::CefBackend;
pub use runtime::{available, configure, ensure_initialized, preload, run_helper, shutdown, state, warm_up_soon, Config, State};
