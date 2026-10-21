//! `aui-cef` — a Chromium (CEF, Chrome style) backend for the aui-webview
//! browser pane. macOS only.
//!
//! - [`runtime`]: the process lifecycle — [`preload`] at `main()`, a lazy
//!   `CefInitialize` on the first page, the external message pump on the main
//!   run loop, and [`shutdown`] at quit; plus [`run_helper`] for the helper
//!   executable CEF launches for its subprocesses.
//! - [`backend`]: [`CefBackend`], a [`aui_webview::WebBackend`] whose page is
//!   a frameless Chrome-style window adopted as a child of the host window.
//! - [`keys`]: host shortcut routing ([`set_key_router`]), keystroke
//!   conversion ([`keystroke_of`]) and the Chromium command filter
//!   ([`chrome_command_allowed`], [`take_blocked_commands`]). The page window
//!   can become key (typing keeps working) but never main, so the host
//!   window keeps its coloured traffic lights.
//!
//! The host bundles the CEF framework at
//! `App.app/Contents/Frameworks/Chromium Embedded Framework.framework` and
//! five helper apps (`App Helper.app`, `App Helper (GPU|Renderer|Plugin|Alerts).app`)
//! whose executable calls [`run_helper`]. Without the framework, [`preload`]
//! returns false and the host keeps its WebKit backend.

pub mod backend;
pub mod keys;
pub mod runtime;
pub mod wheel;

pub use backend::{CefBackend, PopupTab, ZOOM_STEPS, media_kinds_from_bits, permission_kinds_from_bits, snap_zoom_percent, zoom_level_to_percent, zoom_percent_to_level};
pub use keys::{chrome_command_allowed, clear_key_router, context_menu_open_foreground, keystroke_of, set_key_router, take_blocked_commands};
pub use wheel::{set_wheel_router, wheel_phase, PageWheel, WheelPhase};
pub use runtime::{available, configure, ensure_initialized, preload, run_helper, shutdown, state, warm_up_soon, Config, State};
