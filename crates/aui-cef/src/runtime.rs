//! The process-wide CEF lifecycle.
//!
//! 1. [`preload`] at the very top of `main()`: maps the framework when the app
//!    bundle carries it (Chromium's allocator shim must own the malloc zone
//!    before the app allocates in earnest — cmux's finding) and does nothing
//!    otherwise, so a `cargo run` binary or a bundle without CEF simply stays
//!    on WebKit.
//! 2. [`configure`] once the host knows where the profile lives.
//! 3. [`ensure_initialized`] on the first page: `CefInitialize` after the
//!    app's run loop has started, driven by an external message pump on the
//!    main run loop (spike S1: 182 ms; the non-pump variant crashes).
//! 4. [`shutdown`] from the host's quit hook: close every page, pump until
//!    they are gone (bounded), then `CefShutdown`.
//!
//! Everything here runs on the main thread, which is CEF's UI thread under an
//! external pump.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cef::*;

unsafe extern "C" {
    fn aui_cef_install_app_conformance() -> i32;
    fn aui_cef_pump_start(work: extern "C" fn());
    fn aui_cef_pump_schedule(delay_ms: i64);
    fn aui_cef_pump_stop();
}

/// Where the runtime is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// No framework in the bundle (or [`preload`] not called): use WebKit.
    Unavailable,
    /// The framework is mapped; `CefInitialize` has not run.
    Loaded,
    /// Initialised and pumping.
    Ready,
    /// `CefInitialize` returned false; pages fall back.
    Failed,
    /// [`shutdown`] ran. CEF cannot be initialised twice in one process.
    ShutDown,
}

/// What the host decides before the first page.
#[derive(Clone, Debug)]
pub struct Config {
    /// The profile root (cookies, storage, cache, passwords).
    pub cache_dir: PathBuf,
    /// Use Chromium's mock keychain instead of a "<App> Safe Storage" item.
    /// For ad-hoc signed development builds, where every re-sign would
    /// re-prompt; passwords saved with it are not protected by the keychain.
    pub mock_keychain: bool,
    /// Where CEF writes its own log, if anywhere.
    pub log_file: Option<PathBuf>,
}

thread_local! {
    static LOADER: RefCell<Option<library_loader::LibraryLoader>> = const { RefCell::new(None) };
    static STATE: Cell<State> = const { Cell::new(State::Unavailable) };
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
}

/// The framework binary inside a bundle whose executable is `exe`
/// (`App.app/Contents/MacOS/App` → `App.app/Contents/Frameworks/…`).
pub fn framework_path(exe: &Path) -> PathBuf {
    exe.parent()
        .unwrap_or(Path::new("."))
        .join("../Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework")
}

/// Maps the CEF framework if the bundle carries it. Call first thing in
/// `main()`. Returns whether CEF is available. `AUI_CEF_DISABLE=1` forces
/// WebKit.
pub fn preload() -> bool {
    if std::env::var_os("AUI_CEF_DISABLE").is_some() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else { return false };
    // `LibraryLoader::new` unwraps a canonicalize: check first, or a bundle
    // without the framework panics instead of falling back.
    if !framework_path(&exe).exists() {
        return false;
    }
    let loader = library_loader::LibraryLoader::new(&exe, false);
    if !loader.load() {
        eprintln!("aui-cef: the framework is in the bundle but did not load");
        return false;
    }
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    // Kept for the process's life: dropping the loader unloads the framework.
    LOADER.with(|slot| *slot.borrow_mut() = Some(loader));
    STATE.set(State::Loaded);
    true
}

/// Records where the profile lives. Call before the first page.
pub fn configure(config: Config) {
    CONFIG.with(|slot| *slot.borrow_mut() = Some(config));
}

/// Where the runtime is.
pub fn state() -> State {
    STATE.get()
}

/// Whether a page can be created on CEF now or after a lazy init.
pub fn available() -> bool {
    matches!(state(), State::Loaded | State::Ready)
}

extern "C" fn pump_work() {
    do_message_loop_work();
}

wrap_app! {
    struct AuiApp {
        mock_keychain: bool,
    }

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(AuiBrowserProcess::new(()))
        }

        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            if let Some(command_line) = command_line {
                if self.mock_keychain {
                    command_line.append_switch(Some(&CefString::from("use-mock-keychain")));
                }
            }
        }
    }
}

wrap_browser_process_handler! {
    struct AuiBrowserProcess {
        _unit: (),
    }

    impl BrowserProcessHandler {
        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            // Any thread; the shim hops to the main run loop.
            unsafe { aui_cef_pump_schedule(delay_ms) };
        }
    }
}

/// Asks the pump to run CEF's work now (after posting a task).
pub(crate) fn kick_pump() {
    unsafe { aui_cef_pump_schedule(0) };
}

/// Removes Chromium's own session-restore data from the profile before CEF
/// starts. The host restores its tabs itself; Chromium's copy is never wanted.
/// Found live (2026-10-06): an unclean exit leaves `exit_type: Crashed`, and
/// at the next launch Chrome-style CEF reopened every saved page from
/// `Default/Sessions` as its OWN standalone browser windows (one visible
/// "about:blank" window with a toolbar, the rest hidden) outside the host.
/// Returns the paths it removed.
pub fn discard_chromium_session(cache_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let profile = cache_dir.join("Default");
    let mut removed = Vec::new();
    for name in ["Sessions", "Current Session", "Current Tabs", "Last Session", "Last Tabs"] {
        let path = profile.join(name);
        let gone = if path.is_dir() {
            std::fs::remove_dir_all(&path).is_ok()
        } else if path.exists() {
            std::fs::remove_file(&path).is_ok()
        } else {
            false
        };
        if gone {
            removed.push(path);
        }
    }
    removed
}

/// Initialises CEF once, lazily. Must run on the main thread after the app's
/// run loop has started.
pub fn ensure_initialized() -> Result<(), String> {
    match state() {
        State::Ready => return Ok(()),
        State::Loaded => {}
        other => return Err(format!("CEF is not available ({other:?})")),
    }
    let config = CONFIG
        .with(|slot| slot.borrow().clone())
        .ok_or_else(|| String::from("aui-cef: configure() was not called"))?;
    let started = Instant::now();
    let conformance = unsafe { aui_cef_install_app_conformance() };
    unsafe { aui_cef_pump_start(pump_work) };
    discard_chromium_session(&config.cache_dir);
    let cache = config.cache_dir.to_string_lossy().into_owned();
    let log = config.log_file.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let settings = Settings {
        external_message_pump: 1,
        no_sandbox: 0,
        // The host's own argv (its flags) is never Chromium's.
        command_line_args_disabled: 1,
        root_cache_path: CefString::from(cache.as_str()),
        // An empty cache_path would make the global context incognito.
        cache_path: CefString::from(cache.as_str()),
        persist_session_cookies: 1,
        log_severity: LogSeverity::WARNING,
        log_file: CefString::from(log.as_str()),
        ..Default::default()
    };
    let args = cef::args::Args::new();
    let mut app = AuiApp::new(config.mock_keychain);
    let ok = initialize(Some(args.as_main_args()), Some(&settings), Some(&mut app), std::ptr::null_mut());
    if ok == 1 {
        STATE.set(State::Ready);
        eprintln!(
            "aui-cef: initialised in {} ms (app conformance {conformance:#b}, profile {cache})",
            started.elapsed().as_millis()
        );
        Ok(())
    } else {
        unsafe { aui_cef_pump_stop() };
        STATE.set(State::Failed);
        Err(String::from("CefInitialize returned false"))
    }
}

/// Starts CEF on the main queue's next turn, outside whatever host update is
/// running. Call once the app's window is up: a page opened later finds CEF
/// ready, so `CefInitialize` never runs inside a host update (inside one it
/// lost that update live — the pane asking for the page closed).
pub fn warm_up_soon() {
    if state() != State::Loaded {
        return;
    }
    crate::backend::defer(|| {
        if let Err(error) = ensure_initialized() {
            eprintln!("aui-cef: warm-up failed: {error}");
        }
    });
}

/// Closes every page, pumps until they are gone (at most `budget`), then shuts
/// CEF down. Safe to call when CEF never started.
pub fn shutdown(budget: Duration) {
    if state() != State::Ready {
        return;
    }
    let started = Instant::now();
    crate::backend::close_all();
    let deadline = started + budget;
    while crate::backend::open_count() > 0 && Instant::now() < deadline {
        do_message_loop_work();
        std::thread::sleep(Duration::from_millis(10));
    }
    // A short settle: Chrome-style browsers outlive their CEF browsers by a
    // few tasks, and shutting down inside that window CHECK-crashed for cmux.
    let settle = Instant::now() + Duration::from_millis(300);
    while Instant::now() < settle {
        do_message_loop_work();
        std::thread::sleep(Duration::from_millis(10));
    }
    unsafe { aui_cef_pump_stop() };
    cef::shutdown();
    STATE.set(State::ShutDown);
    eprintln!("aui-cef: shut down in {} ms", started.elapsed().as_millis());
}

/// Entry point for the helper executable CEF launches for its renderer, GPU,
/// network and other processes. Returns the process exit code.
pub fn run_helper() -> i32 {
    let args = cef::args::Args::new();
    let _sandbox = {
        let mut sandbox = cef::sandbox::Sandbox::new();
        sandbox.initialize(args.as_main_args());
        sandbox
    };
    let Ok(exe) = std::env::current_exe() else { return 1 };
    let loader = library_loader::LibraryLoader::new(&exe, true);
    if !loader.load() {
        eprintln!("aui-cef helper: the framework did not load");
        return 1;
    }
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let code = execute_process(Some(args.as_main_args()), None::<&mut App>, std::ptr::null_mut());
    drop(loader);
    code
}

#[cfg(test)]
mod session_tests {
    use super::discard_chromium_session;

    #[test]
    fn chromium_session_data_is_removed_and_nothing_else() {
        let root = std::env::temp_dir().join(format!("aui-cef-session-{}", std::process::id()));
        let profile = root.join("Default");
        std::fs::create_dir_all(profile.join("Sessions")).unwrap();
        std::fs::write(profile.join("Sessions/Session_1"), b"x").unwrap();
        std::fs::write(profile.join("Current Session"), b"x").unwrap();
        std::fs::write(profile.join("Preferences"), b"{}").unwrap();
        std::fs::create_dir_all(profile.join("Local Storage")).unwrap();
        let removed = discard_chromium_session(&root);
        assert_eq!(removed.len(), 2, "{removed:?}");
        assert!(!profile.join("Sessions").exists());
        assert!(!profile.join("Current Session").exists());
        assert!(profile.join("Preferences").exists(), "settings are kept");
        assert!(profile.join("Local Storage").exists(), "site data is kept");
        assert!(discard_chromium_session(&root).is_empty(), "a second run finds nothing");
        let _ = std::fs::remove_dir_all(&root);
    }
}
