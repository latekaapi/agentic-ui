//! Screens · Providers — the D5 Providers board (harness docs/66 §3), one
//! band per board state: the connect screen (proposed, nothing ready,
//! checking, too old · couldn't check, Codex signing in, Codex's device
//! code), the setup sheet (install, sign in, done) and the Muse sign-in
//! sheet over it, the Settings provider card (Details closed and open, an
//! API-key login, too old, not installed, Codex waiting) and the signed-out
//! banner. Plus the account-menu usage cards and a settled whole-JSON
//! assistant message.

use aui::screens::{
    connect_providers, login, provider_card, setup_sheet, sheet_scrim, signed_out_banner,
    usage_card, AssistRun, CardDetails, ConnectRowData, DefaultPicker, DetailRow, LoginState,
    ProviderActionDef, ProviderCardData, SetupStatus, StatusLine, UsageWindow,
};
use aui::transcript::assistant_turn;
use aui_icons::Provider;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// The board's frame for one screen state.
const FRAME_W: f32 = 664.0;
const FRAME_H: f32 = 520.0;

/// One labelled band: a caption over the component.
fn band(caption: &'static str, content: impl IntoElement) -> AnyElement {
    v_flex()
        .gap(px(6.0))
        .child(
            div()
                .text_px(scale::FS_11)
                .line_height(gpui::relative(scale::LH_UI))
                .child(caption),
        )
        .child(content)
        .into_any_element()
}

/// A board frame: `content` clipped to 664×520 over the frame ground.
fn frame(content: impl IntoElement, h: f32) -> AnyElement {
    div().relative().w(px(FRAME_W)).h(px(h)).overflow_hidden().child(content).into_any_element()
}

fn row(id: &'static str, provider: Provider, status: StatusLine, action: Option<ProviderActionDef>) -> ConnectRowData {
    let connected = status.tone == aui::screens::StatusTone::Ok;
    ConnectRowData { id: id.into(), provider, status, action, connected }
}

fn claude_connected() -> ConnectRowData {
    row("claude", Provider::Claude, StatusLine::ok("ada@example.com · Max"), None)
}

fn get_muse() -> ConnectRowData {
    row("muse", Provider::Muse, StatusLine::off("Not installed"), Some(ProviderActionDef::link("Get Muse")))
}

fn picker(current: Provider, choices: Vec<Provider>) -> Option<DefaultPicker> {
    Some(DefaultPicker { current, choices, open: false })
}

/// The connect screen in `rows`, footer from `picker`, assist line `assist`.
fn connect(id: &'static str, rows: Vec<ConnectRowData>, picker: Option<DefaultPicker>, assist: Vec<AssistRun>) -> AnyElement {
    connect_providers(id, rows).default_picker(picker).assist(assist).on_intent(|_, _, _| {}).into_any_element()
}

/// The setup sheet's backdrop: the nothing-ready connect screen.
fn nothing_ready(id: &'static str) -> AnyElement {
    connect(
        id,
        vec![
            row("claude", Provider::Claude, StatusLine::off("Not installed"), Some(ProviderActionDef::install())),
            row("codex", Provider::Codex, StatusLine::warn("Signed out"), Some(ProviderActionDef::sign_in())),
            get_muse(),
        ],
        None,
        vec![],
    )
}

/// A static stand-in for the host's terminal session: the gallery has no pty.
fn fake_terminal(lines: &'static str, cx: &App) -> AnyElement {
    let p = cx.aui().colors;
    div()
        .size_full()
        .px(px(12.0))
        .py(px(10.0))
        .font_family(scale::FONT_MONO)
        .text_px(11.5)
        .line_height(px(19.0))
        .text_color(p.ink)
        .children(lines.lines().map(|line| div().child(SharedString::from(line))))
        .into_any_element()
}

/// The connect screen in every board state.
pub fn build_connect(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let proposed = connect(
        "g-connect-proposed",
        vec![claude_connected(), row("codex", Provider::Codex, StatusLine::warn("Signed out"), Some(ProviderActionDef::sign_in())), get_muse()],
        picker(Provider::Claude, vec![Provider::Claude]),
        vec![],
    );
    let nothing = connect(
        "g-connect-nothing",
        vec![
            row("claude", Provider::Claude, StatusLine::off("Not installed"), Some(ProviderActionDef::install())),
            row("codex", Provider::Codex, StatusLine::warn("Installed · signed out"), Some(ProviderActionDef::sign_in())),
            get_muse(),
        ],
        None,
        vec![],
    );
    let checking = connect(
        "g-connect-checking",
        vec![
            row("claude", Provider::Claude, StatusLine::busy("Checking…"), None),
            row("codex", Provider::Codex, StatusLine::busy("Checking…"), None),
            row("muse", Provider::Muse, StatusLine::busy("Checking…"), None),
        ],
        None,
        vec![],
    );
    let too_old = connect(
        "g-connect-too-old",
        vec![
            row("claude", Provider::Claude, StatusLine::warn("2.0.9 is older than Baaz supports (2.1.276)"), Some(ProviderActionDef::update())),
            row("codex", Provider::Codex, StatusLine::ok("ada@example.com · ChatGPT Pro"), None),
            row("muse", Provider::Muse, StatusLine::bad("Couldn’t check — the CLI timed out"), Some(ProviderActionDef::recheck())),
        ],
        picker(Provider::Codex, vec![Provider::Codex]),
        vec![],
    );
    let codex_wait = connect(
        "g-connect-codex-wait",
        vec![claude_connected(), row("codex", Provider::Codex, StatusLine::busy("Finish signing in with your browser…"), Some(ProviderActionDef::cancel())), get_muse()],
        picker(Provider::Claude, vec![Provider::Claude]),
        vec![
            AssistRun::Text("Browser didn’t open?".into()),
            AssistRun::Link("copy-link".into(), "Copy the link".into()),
            AssistRun::Text("·".into()),
            AssistRun::Link("use-code".into(), "Use a code instead".into()),
        ],
    );
    let codex_code = connect(
        "g-connect-codex-code",
        vec![claude_connected(), row("codex", Provider::Codex, StatusLine::busy("Finish signing in with your browser…"), Some(ProviderActionDef::cancel())), get_muse()],
        picker(Provider::Claude, vec![Provider::Claude]),
        vec![
            AssistRun::Text("Enter".into()),
            AssistRun::Code("K7QX-9P2M".into()),
            AssistRun::Text("at auth.openai.com/codex/device ·".into()),
            AssistRun::Link("copy-code".into(), "Copy code".into()),
            AssistRun::Text("·".into()),
            AssistRun::Link("open-page".into(), "Open page".into()),
        ],
    );
    let menu_open = connect(
        "g-connect-menu",
        vec![claude_connected(), row("codex", Provider::Codex, StatusLine::ok("ada@example.com · ChatGPT Pro"), None), get_muse()],
        Some(DefaultPicker { current: Provider::Claude, choices: vec![Provider::Claude, Provider::Codex], open: true }),
        vec![],
    );
    grid(vec![
        band("First run · proposed", frame(proposed, FRAME_H)),
        band("Nothing ready yet", frame(nothing, FRAME_H)),
        band("Checking (first second)", frame(checking, FRAME_H)),
        band("Too old · couldn’t check", frame(too_old, FRAME_H)),
        band("Codex · signing in (in the row)", frame(codex_wait, FRAME_H)),
        band("Codex · use a code instead", frame(codex_code, FRAME_H)),
        band("Default picker open (Connected providers only)", frame(menu_open, FRAME_H)),
    ])
}

/// Bands two to a row.
fn grid(bands: Vec<AnyElement>) -> AnyElement {
    let mut col = v_flex().gap(px(24.0));
    let mut it = bands.into_iter();
    while let Some(first) = it.next() {
        let mut r = h_flex().items_start().gap(px(24.0)).child(first);
        if let Some(second) = it.next() {
            r = r.child(second);
        }
        col = col.child(r);
    }
    div().p(px(24.0)).child(col).into_any_element()
}

/// The setup sheet over the connect screen: install, sign in, done; and the
/// Muse sign-in sheet.
pub fn build_setup(_window: &mut Window, cx: &mut App) -> AnyElement {
    let sheet = |id: &'static str, title: &'static str, status: SetupStatus, term: &'static str, note: &'static str, cx: &App| {
        let s = setup_sheet(id, Provider::Claude, title).status(status).terminal(fake_terminal(term, cx)).note(note).on_close(|_, _| {});
        div().relative().size_full().child(nothing_ready("g-setup-under")).child(sheet_scrim(SharedString::from(format!("{id}-scrim")), s, None)).into_any_element()
    };
    let install = sheet(
        "g-sheet-install",
        "Install Claude Code",
        SetupStatus::Waiting("Waiting for the install to finish…".into()),
        "~ % curl -fsSL https://claude.ai/install.sh | bash",
        "The command is typed, not run: read it, then press Enter. Baaz checks every few seconds and shows Connected here when Claude Code answers.",
        cx,
    );
    let signin = sheet(
        "g-sheet-signin",
        "Sign in to Claude Code",
        SetupStatus::Waiting("Waiting for sign-in…".into()),
        "~ % claude auth login\nOpening your browser to sign in…\n\nIf it doesn’t open, visit:\nhttps://claude.ai/oauth/authorize?code=true&…\n\nPaste code here if prompted >",
        "Claude Code signs in through your browser. This terminal is Claude Code’s own; Baaz never sees your credentials.",
        cx,
    );
    let done = sheet(
        "g-sheet-done",
        "Sign in to Claude Code",
        SetupStatus::Connected("Connected · ada@example.com · Max".into()),
        "~ % claude auth login\nOpening your browser to sign in…\nLogin successful.\n~ %",
        "Claude Code is ready. New sessions can start in it.",
        cx,
    );
    let muse_rows = vec![
        claude_connected(),
        row("codex", Provider::Codex, StatusLine::warn("Signed out"), Some(ProviderActionDef::sign_in())),
        row("muse", Provider::Muse, StatusLine::warn("Signed out"), Some(ProviderActionDef::sign_in())),
    ];
    let muse = div()
        .relative()
        .size_full()
        .child(connect("g-muse-under", muse_rows, picker(Provider::Claude, vec![Provider::Claude]), vec![]))
        .child(sheet_scrim("g-muse-scrim", login("g-muse-sheet", LoginState::Choose).product("Muse").provider(Provider::Muse).sheet().on_intent(|_, _, _| {}), None))
        .into_any_element();
    grid(vec![
        band("Setup sheet · Install", frame(install, FRAME_H)),
        band("Setup sheet · Sign in to Claude Code", frame(signin, FRAME_H)),
        band("Setup sheet · done", frame(done, FRAME_H)),
        band("Muse · sign-in sheet", frame(muse, FRAME_H)),
    ])
}

/// The Settings provider card in the board's states.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let details = |open: bool, signed_in: &'static str, notice: Vec<AssistRun>| CardDetails {
        open,
        rows: vec![
            DetailRow::new("Version", "2.1.276"),
            DetailRow::mono("Path", "~/.local/bin/claude"),
            DetailRow::new("Found", "on your login shell’s PATH (zsh)"),
            DetailRow::new("Signed in", signed_in),
        ],
        notice,
        actions: vec![ProviderActionDef::change_path(), ProviderActionDef::copy_diagnostics()],
    };
    let card = |id: &'static str, provider: Provider, status: StatusLine, actions: Vec<ProviderActionDef>, details: Option<CardDetails>, assist: Vec<AssistRun>| {
        let mut data = ProviderCardData::new(id, provider, status).actions(actions).assist(assist);
        if let Some(details) = details {
            data = data.details(details);
        }
        provider_card(SharedString::from(format!("g-card-{id}")), &data).on_intent(|_, _, _| {}).on_assist(|_, _, _| {})
    };
    let ok = card(
        "claude-open",
        Provider::Claude,
        StatusLine::ok("Connected · ada@example.com · Max"),
        vec![ProviderActionDef::recheck_text(), ProviderActionDef::sign_out()],
        Some(details(true, "claude.ai · ada@example.com · Max", vec![])),
        vec![],
    );
    let closed = card(
        "claude-closed",
        Provider::Claude,
        StatusLine::ok("Connected · ada@example.com · Max"),
        vec![ProviderActionDef::recheck_text(), ProviderActionDef::sign_out()],
        Some(details(false, "", vec![])),
        vec![],
    );
    let key = card(
        "claude-key",
        Provider::Claude,
        StatusLine::warn("Connected · API key · billed per token"),
        vec![ProviderActionDef::recheck_text(), ProviderActionDef::sign_out()],
        Some(details(
            true,
            "API key from ANTHROPIC_API_KEY · billed per token",
            vec![
                AssistRun::Text("An ".into()),
                AssistRun::Code("ANTHROPIC_API_KEY".into()),
                AssistRun::Text(" in your shell signs Claude Code in: every turn is billed to that key, not a subscription. Remove it from your shell profile to use your Claude plan.".into()),
            ],
        )),
        vec![],
    );
    let old = card(
        "codex-old",
        Provider::Codex,
        StatusLine::warn("0.120.0 is older than Baaz supports (0.144.6)"),
        vec![ProviderActionDef::recheck_text(), ProviderActionDef::update()],
        Some(CardDetails { open: false, ..Default::default() }),
        vec![],
    );
    let missing = card(
        "muse-missing",
        Provider::Muse,
        StatusLine::off("Not installed"),
        vec![ProviderActionDef::recheck_text(), ProviderActionDef::link("Get Muse")],
        Some(CardDetails { open: false, ..Default::default() }),
        vec![],
    );
    let codex_wait = card(
        "codex-wait",
        Provider::Codex,
        StatusLine::busy("Finish signing in with your browser…"),
        vec![ProviderActionDef::recheck_text(), ProviderActionDef::cancel()],
        Some(CardDetails { open: false, ..Default::default() }),
        vec![
            AssistRun::Text("Browser didn’t open?".into()),
            AssistRun::Link("copy-link".into(), "Copy the link".into()),
            AssistRun::Text("·".into()),
            AssistRun::Link("use-code".into(), "Use a code instead".into()),
        ],
    );
    let banner = div()
        .w(px(FRAME_W))
        .py(px(16.0))
        .bg(p.surface_2)
        .child(div().mx(px(20.0)).child(signed_out_banner("g-banner", "Claude Code is signed out. Sign in to send.").on_action(|_, _| {})))
        .into_any_element();
    let page = |content: AnyElement| div().w(px(FRAME_W)).p(px(24.0)).bg(p.surface_1).child(content).into_any_element();
    v_flex()
        .p(px(24.0))
        .gap(px(20.0))
        .text_color(p.ink_2)
        .child(band("Settings › Providers › Claude Code (Details open)", page(ok.into_any_element())))
        .child(band("Details closed", page(closed.into_any_element())))
        .child(band("API-key login (say it, don’t block)", page(key.into_any_element())))
        .child(band("Too old — Update opens the setup sheet", page(old.into_any_element())))
        .child(band("Not installed (Muse) — Get Muse ↗", page(missing.into_any_element())))
        .child(band("Codex signing in (Settings)", page(codex_wait.into_any_element())))
        .child(band("Signed out mid-session (every provider)", banner))
        .into_any_element()
}

/// The usage cards: empty, one window, two windows with a warning fill.
pub fn build_usage(_window: &mut Window, _cx: &mut App) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap(px(16.0))
        .child(v_flex().gap(px(16.0)).child(band(
            "Empty — not reported yet",
            usage_card("gallery-usage-empty", Provider::Claude, vec![]).as_of("never"),
        )))
        .child(v_flex().gap(px(16.0)).child(band(
            "One window",
            usage_card(
                "gallery-usage-one",
                Provider::Codex,
                vec![UsageWindow::new("Primary · 5h", 0.42, "3h 12m")],
            )
            .plan("Pro")
            .as_of("2m ago"),
        )))
        .child(v_flex().gap(px(16.0)).child(band(
            "Two windows, one at warning",
            usage_card(
                "gallery-usage-two",
                Provider::Muse,
                vec![
                    UsageWindow::new("Weekly", 0.83, "2d 4h"),
                    UsageWindow::new("Primary · 5h", 0.31, "41m"),
                ],
            )
            .plan("Business")
            .as_of("just now"),
        )))
        .into_any_element()
}

/// A settled whole-JSON assistant message beside its streaming twin: the
/// settled one is a single `json` code block, the streaming one stays prose.
pub fn build_json(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let source: SharedString =
        "{\"name\": \"checkout-flow\", \"files\": [\"src/a.ts\", \"src/b.ts\"], \"ok\": true}".into();
    v_flex()
        .w_full()
        .gap(px(16.0))
        .child(band(
            "Settled — one json code block",
            assistant_turn("gallery-json-settled", source.clone()),
        ))
        .child(band(
            "Streaming — plain paragraph, never flips mid-chunk",
            assistant_turn("gallery-json-streaming", source).streaming(true),
        ))
        .into_any_element()
}
