//! Card 10 · App shell. The framed three-column shell with its header cells,
//! sidebar, transcript stand-ins, docked composer and diff pane. Reproduces
//! `design/src/cards/shell/10-app-shell.html` at 1280×820 (body padding 12).
//!
//! The transcript and workbench content here is sample content built from
//! the data primitives; the real transcript / diff components arrive with
//! cards 30–38 and 52.

use std::time::Duration;

use aui::data::{button, chip, glyph_ok, kbd, spinner, tag};
use aui::nav::{group_header, group_row, nav_item, rail, session_row, sidebar_footer, ActivityKind, MetaItem, RailItem, SessionSummary};
use aui::transcript::{diff_note_inset, DiffNote, NoteInsets};
use aui::shell::{app_shell, centre_header, clamp_sidebar_width, docked_composer, drag_capture_overlay, resize_handle, right_header, sidebar_header, tab_strip, TabItem};
use aui::shell::{SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH};
use aui_icons::{icon, FileType, IconName, Provider};
use aui_motion::{looping, shimmer_text, Loop};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `body.ds{padding:12px}` — the gallery frame adds 20, so the card pulls in by 8
/// and sizes the shell to `.app{height:796px}` at the card width less 24.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 1280.0 - 24.0;
const FRAME_H: f32 = 796.0;
/// `.app{grid-template-columns:252px 1fr 392px}`.
const RIGHT_WIDTH: f32 = 392.0;
/// `.nav{padding:10px 8px 6px}`.
const NAV_PAD_TOP: f32 = 10.0;
const NAV_PAD_X: f32 = 8.0;
const NAV_PAD_BOTTOM: f32 = 6.0;
/// `.wt .meta .trunc{max-width:160px}` and the activity line's `max-width:190px`.
const BRANCH_MAX: f32 = 160.0;
const ACTIVITY_MAX: f32 = 190.0;
/// `.wt{gap:3px 8px}`.
const ROW_GAP: f32 = 3.0;
/// `.wt{margin:2px 8px}`.
const ROW_MARGIN_X: f32 = 8.0;
/// `.tr{padding:18px 28px 0;gap:16px}`; `.comp{padding:12px 28px 12px}`.
const TRANSCRIPT_PAD_TOP: f32 = 18.0;
const CENTRE_PAD_X: f32 = 28.0;
const BLOCK_GAP: f32 = 16.0;
/// `.u{max-width:70%;border-radius:12px 12px 4px 12px;padding:9px 13px}`.
const BUBBLE_MAX: f32 = 0.7;
const BUBBLE_PAD_Y: f32 = 9.0;
const BUBBLE_PAD_X: f32 = 13.0;
/// `.a{font-size:13.5px;line-height:1.65}`.
const BODY_SIZE: f32 = 13.5;
/// `.tc{gap:8px;height:34px;padding:0 10px;font-size:12.5px}`; `.grp2{gap:8px}`.
const TOOL_GAP: f32 = 8.0;
const TOOL_PAD_X: f32 = 10.0;
const TOOL_TEXT: f32 = 12.5;
/// The streaming caret: 2 × 14, accent, blinks every second in two steps.
const CARET_W: f32 = 2.0;
const CARET_H: f32 = 14.0;
const CARET_PERIOD: Duration = Duration::from_millis(1000);
/// `.status{padding:0 0 12px;font-size:12px}` with `.row{gap:8px}`; its spinner is 11 px.
const STATUS_PAD_BOTTOM: f32 = 12.0;
const STATUS_GAP: f32 = 8.0;
const STATUS_SPINNER: f32 = 11.0;
/// Right pane scope row: `padding:10px 12px 6px;font-size:12px` with `.row{gap:8px}`.
const SCOPE_PAD_TOP: f32 = 10.0;
const SCOPE_PAD_X: f32 = 12.0;
const SCOPE_PAD_BOTTOM: f32 = 6.0;
/// `.files{padding:8px 10px 6px;gap:2px;font-size:12px}`; `.fr{gap:7px;height:28px;padding:0 6px}`; `.fr .st{font:600 10px/1 mono}`.
const FILES_PAD_TOP: f32 = 8.0;
const FILES_PAD_X: f32 = 10.0;
const FILES_PAD_BOTTOM: f32 = 6.0;
const FILES_GAP: f32 = 2.0;
const FILE_ROW_H: f32 = 28.0;
const FILE_ROW_GAP: f32 = 7.0;
const FILE_ROW_PAD: f32 = 6.0;
const FILE_STATUS_SIZE: f32 = 10.0;
/// `.diff{margin:8px 10px;font:11.5px/1.75 mono}`; `.fh{gap:8px;padding:7px 10px;font-size:12px}`; `.ln{padding:0 8px}`; `.gutter{width:26px;padding-right:8px}`.
const DIFF_MARGIN_Y: f32 = 8.0;
const DIFF_MARGIN_X: f32 = 10.0;
const DIFF_TEXT: f32 = 11.5;
const DIFF_LH: f32 = 1.75;
const DIFF_HEAD_GAP: f32 = 8.0;
const DIFF_HEAD_PAD_Y: f32 = 7.0;
const DIFF_HEAD_PAD_X: f32 = 10.0;
const DIFF_LINE_PAD_X: f32 = 8.0;
const GUTTER_W: f32 = 26.0;
const GUTTER_PAD: f32 = 8.0;
/// `.note{margin:8px 10px 10px 44px;line-height:1.5;border:1px solid var(--line);padding:6px 8px;font-size:12px}` and `.note .caps{margin-bottom:3px}`. Drawn by `aui::transcript::diff_note_inset`.
const NOTE_MARGIN_TOP: f32 = 8.0;
const NOTE_MARGIN_RIGHT: f32 = 10.0;
const NOTE_MARGIN_BOTTOM: f32 = 10.0;
const NOTE_MARGIN_LEFT: f32 = 44.0;
const NOTE_LH: f32 = 1.5;
const NOTE_CAPS_GAP: f32 = 3.0;
/// `.actions{gap:8px;padding:10px 12px}` with the `.hint` at 11 px.
const ACTIONS_GAP: f32 = 8.0;
const ACTIONS_PAD_Y: f32 = 10.0;
const ACTIONS_PAD_X: f32 = 12.0;

/// `AUI_GALLERY_SIDEBAR_WIDTH`: screenshot hook for the resize range —
/// e.g. `180`, `252`, `420` capture the card at the min, default and max.
const WIDTH_ENV: &str = "AUI_GALLERY_SIDEBAR_WIDTH";

/// The width screenshots start from (the hook above, else the default).
fn initial_width() -> f32 {
    std::env::var(WIDTH_ENV).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(SIDEBAR_WIDTH)
}

/// Interactive state of the card: the shell toggles, the active tab, and the
/// live sidebar resize (`width` / `resizing` / `grab_x` / `start_w`).
#[derive(Clone, Copy)]
struct ShellState {
    right_open: bool,
    sidebar_open: bool,
    tab: usize,
    sidebar_width: f32,
    resizing: bool,
    grab_x: f32,
    start_w: f32,
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let state = window.use_keyed_state("card10-state", cx, |_, _| ShellState {
        right_open: true,
        sidebar_open: !collapsed_by_default(),
        tab: 0,
        sidebar_width: initial_width(),
        resizing: false,
        grab_x: 0.0,
        start_w: 0.0,
    });
    let current = *state.read(cx);
    let update = |f: fn(&mut ShellState)| {
        let state = state.clone();
        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| {
                f(s);
                cx.notify();
            })
        }
    };
    let tab_ids = ["diff", "files", "terminal", "browser"];
    let tabs = vec![
        TabItem::new("diff", "Diff", IconName::Git).closable(false),
        TabItem::new("files", "Files", IconName::Folder).closable(false),
        TabItem::new("terminal", "Terminal", IconName::Terminal).closable(false),
        TabItem::new("browser", "Browser", IconName::Globe).closable(false),
    ];
    let select_state = state.clone();
    let strip = tab_strip("card10-tabs", tabs, current.tab).on_select(move |id, _, cx| {
        let index = tab_ids.iter().position(|t| *t == id.as_ref()).unwrap_or(0);
        select_state.update(cx, |s, cx| {
            s.tab = index;
            cx.notify();
        })
    });
    // The resize intents: press arms the drag and remembers the grab point
    // and the resting width; moves clamp into the resizable range; release
    // disarms. The same two closures feed the handle and the capture overlay.
    let on_move = || {
        let state = state.clone();
        move |x: f32, _: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| {
                s.sidebar_width = clamp_sidebar_width(s.start_w + (x - s.grab_x));
                cx.notify();
            });
        }
    };
    let on_end = || {
        let state = state.clone();
        move |_: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| {
                s.resizing = false;
                cx.notify();
            });
        }
    };
    let on_start = {
        let state = state.clone();
        move |x: f32, _: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| {
                s.resizing = true;
                s.grab_x = x;
                s.start_w = s.sidebar_width;
                cx.notify();
            });
        }
    };
    let mut frame = div()
        .relative()
        .w(px(FRAME_W))
        .h(px(FRAME_H))
        .flex_none()
        .m(px(FRAME_INSET))
        .child(
            app_shell("card10-shell")
                .framed(true)
                .traffic_lights(true)
                .sidebar_width(px(current.sidebar_width))
                .sidebar_min_width(px(SIDEBAR_MIN_WIDTH))
                .sidebar_max_width(px(SIDEBAR_MAX_WIDTH))
                .resizing(current.resizing)
                .right_width(px(RIGHT_WIDTH))
                .right_open(current.right_open)
                .sidebar_open(current.sidebar_open)
                .header_sidebar(
                    sidebar_header("card10-hd-side")
                        .traffic_lights(true)
                        .collapsed(!current.sidebar_open)
                        .on_toggle_sidebar(update(|s| s.sidebar_open = !s.sidebar_open)),
                )
                .header_centre({
                    let mut centre = centre_header("card10-hd-centre", "checkout-flow-v2")
                        .provider(Provider::Claude)
                        .branch("feature/checkout-flow-v2")
                        .on_toggle_right(update(|s| s.right_open = !s.right_open));
                    if !current.sidebar_open {
                        centre = centre.on_expand_sidebar(update(|s| s.sidebar_open = true));
                    }
                    centre
                })
                .header_right(right_header("card10-hd-right").tabs(strip).on_close(update(|s| s.right_open = false)))
                .sidebar(sidebar(cx))
                .rail(shell_rail())
                .centre(centre(window, cx))
                .right(right_pane(cx)),
        );
    // The resize strip rides the divider, centred on the edge; hidden with
    // the sidebar, since there is no divider to grab on the rail.
    if current.sidebar_open {
        frame = frame.child(
            div().absolute().top(px(0.0)).bottom(px(0.0)).left(px(current.sidebar_width - 3.0)).child(
                resize_handle("card10-resize").on_drag_start(on_start).on_drag(on_move()).on_drag_end(on_end()),
            ),
        );
    }
    // Mid-drag the capture overlay owns the window so the drag survives the
    // pointer leaving the 6 px strip.
    if current.resizing {
        frame = frame.child(drag_capture_overlay("card10-resize-capture").on_drag(on_move()).on_drag_end(on_end()));
    }
    frame.into_any_element()
}

/// The gallery renders one static frame per screenshot, so the collapsed state
/// is reached with `AUI_GALLERY_COLLAPSED=1` as well as by clicking the toggle.
pub(crate) fn collapsed_by_default() -> bool {
    std::env::var("AUI_GALLERY_COLLAPSED").is_ok_and(|v| v != "0" && !v.is_empty())
}

/// The collapsed sidebar: the nav glyphs, then one dot per active worktree.
pub(crate) fn shell_rail() -> impl IntoElement {
    let (pinned, in_progress) = sessions();
    let mut items = vec![
        RailItem::nav("tasks", IconName::List),
        RailItem::nav("automations", IconName::Zap),
        RailItem::nav("inbox", IconName::Inbox).badge(),
        RailItem::separator(),
    ];
    for (session, selected) in &pinned {
        if session.state == AgentState::Idle {
            continue;
        }
        // Pinned sessions are titled tiles: the initial, the state dot in
        // the corner, the title as the tooltip.
        let mut cell = RailItem::session(session.id.clone(), session.state).label(session.name.clone()).selected(*selected);
        if session.pulse {
            cell = cell.pulse();
        }
        items.push(cell);
    }
    for session in &in_progress {
        if session.state == AgentState::Idle {
            continue;
        }
        items.push(RailItem::session(session.id.clone(), session.state));
    }
    rail("card10-rail", items).flat(true).avatar("B")
}

fn sessions() -> (Vec<(SessionSummary, bool)>, Vec<SessionSummary>) {
    let pinned = vec![
        (
            SessionSummary::new("checkout", "checkout-flow-v2", AgentState::Running, "49m")
                .pulse()
                .repo("acme-web")
                .branch("feature/checkout-flow-v2")
                .provider(Provider::Claude)
                .activity(ActivityKind::Working, "running checkout regression tests"),
            true,
        ),
        (
            SessionSummary::new("notifier", "infra/notifier", AgentState::Waiting, "3h")
                .pulse()
                .repo("orca")
                .branch("main")
                .provider(Provider::Codex)
                .activity(ActivityKind::Waiting, "awaiting permission · sudo apt install"),
            false,
        ),
        (SessionSummary::new("auth", "auth-session-refresh", AgentState::Done, "4h").repo("acme-web").meta(MetaItem::Text("PR #2491 open".into())), false),
    ];
    let in_progress = vec![
        SessionSummary::new("cart", "cart-recovery-email", AgentState::Running, "12m").repo("acme-web").branch("feature/cart-recovery").provider(Provider::Claude),
        SessionSummary::new("webhook", "Webhook retry backoff", AgentState::Idle, "2d").repo("acme-internal").branch("fix/webhook-retry"),
        SessionSummary::new("obs", "Observability tiles", AgentState::Failed, "1d").repo("acme-internal").meta(MetaItem::Danger("2 tests failed".into())),
    ];
    (pinned, in_progress)
}

fn row(id: (&'static str, usize), session: SessionSummary, selected: bool) -> impl IntoElement {
    session_row(id, session).selected(selected).margin_x(ROW_MARGIN_X).row_gap(ROW_GAP).branch_max(BRANCH_MAX).activity_max(ACTIVITY_MAX)
}

pub(crate) fn sidebar(_cx: &mut App) -> impl IntoElement {
    let (pinned, in_progress) = sessions();
    let mut col = v_flex()
        .size_full()
        .child(
            v_flex()
                .w_full()
                .pt(px(NAV_PAD_TOP))
                .px(px(NAV_PAD_X))
                .pb(px(NAV_PAD_BOTTOM))
                .child(nav_item("card10-nav-tasks", IconName::List, "Tasks").count("7"))
                .child(nav_item("card10-nav-automations", IconName::Zap, "Automations"))
                .child(nav_item("card10-nav-inbox", IconName::Inbox, "Inbox").count("2").count_warning()),
        )
        .child(group_row("card10-workspaces", "Workspaces").on_view_options(|_, _, _| {}).on_add(|_, _, _| {}))
        .child(group_header("card10-pinned", "Pinned", true).count("3"));
    for (i, (session, selected)) in pinned.into_iter().enumerate() {
        col = col.child(row(("card10-pinned-row", i), session, selected));
    }
    col = col.child(group_header("card10-progress", "In progress", true).count("17"));
    for (i, session) in in_progress.into_iter().enumerate() {
        col = col.child(row(("card10-progress-row", i), session, false));
    }
    col.child(div().flex_1()).child(
        // Three rows: the name, the identity under it, and what the account is
        // entitled to under that. The entitlement is the only line in a footer
        // that ever carries colour, and only when it wants looking at.
        sidebar_footer("card10-footer", "B", "Bharani · Max")
            .detail("bharani@example.com")
            .plan("High Usage · 2% this week", false)
            .meter(Provider::Claude, 0.78),
    )
}

pub(crate) fn centre(window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.aui().colors;
    let caret_on = looping(("card10-caret", "blink"), Loop::linear(CARET_PERIOD).resting(1.0), window, cx) < 0.5;
    let transcript = v_flex()
        .flex_1()
        .min_h(px(0.0))
        .w_full()
        .overflow_hidden()
        .pt(px(TRANSCRIPT_PAD_TOP))
        .px(px(CENTRE_PAD_X))
        .gap(px(BLOCK_GAP))
        .child(
            div()
                .self_end()
                .max_w(relative(BUBBLE_MAX))
                .py(px(BUBBLE_PAD_Y))
                .px(px(BUBBLE_PAD_X))
                .rounded(px(scale::R_LG))
                .rounded_br(px(scale::R_XS))
                .bg(p.surface_3)
                .ui(scale::FS_13)
                .text_color(p.ink)
                .child("tighten address validation and add coverage"),
        )
        .child(assistant_text(p, "I'm checking the existing form flow, then I'll patch the validator and run the focused tests.", false, true))
        .child(
            v_flex()
                .w_full()
                .gap(px(TOOL_GAP))
                .child(tool_row(p, "card10-tool-1", ToolState::Ok, "Ran", "rg -n \"validateAddress\" src/checkout", tag("0.3 s").into_any_element()))
                .child(tool_row(p, "card10-tool-2", ToolState::Ok, "Read", "src/checkout/validators.ts", tag("180 lines").into_any_element()))
                .child(tool_row(
                    p,
                    "card10-tool-3",
                    ToolState::Running,
                    "Update",
                    "src/checkout/validators.ts",
                    h_flex().gap(px(scale::SP_2)).child(tag("+8").color(p.success)).child(tag("−3").color(p.danger)).into_any_element(),
                )),
        )
        .child(assistant_text(
            p,
            "Found the country-specific branch. I'm making the ZIP/postal path explicit and keeping the existing checkout copy unchanged",
            caret_on,
            true,
        ))
        .child(div().flex_1())
        .child(
            h_flex()
                .w_full()
                .flex_none()
                .pb(px(STATUS_PAD_BOTTOM))
                .gap(px(STATUS_GAP))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .whitespace_nowrap()
                .child(spinner("card10-status-spinner").size(px(STATUS_SPINNER)))
                .child(shimmer_text("card10-working", "Working…", cx).text_size(aui_tokens::scaled(scale::FS_12)))
                .child(div().text_role(TextRole::Mono).text_px(scale::FS_12).child("12 s"))
                .child("·")
                .child(kbd("esc"))
                .child("to interrupt"),
        );
    v_flex().size_full().child(transcript).child(
        docked_composer("card10-composer", Provider::Claude, "Opus 4.6").mode("Plan").context_percent(34).streaming(true).pad_x(CENTRE_PAD_X),
    )
}

/// `.a`: 13.5 / 1.65 assistant copy, optionally with the streaming caret.
fn assistant_text(p: Palette, text: &'static str, caret: bool, _stream: bool) -> impl IntoElement {
    let mut el = div().w_full().ui(BODY_SIZE).line_height(relative(scale::LH_BODY)).text_color(p.ink);
    if caret {
        // The caret is an inline-block 2 × 14 accent bar; gpui text cannot host
        // an element inline, so it sits at the end of the paragraph's last line
        // box rather than after the last word (the transcript component of
        // card 31 owns the real streaming caret).
        el = el.child(
            h_flex()
                .w_full()
                .items_end()
                .child(div().flex_1().min_w(px(0.0)).child(text))
                .child(div().flex_none().ml(px(scale::SP_1)).mb(px(3.0)).w(px(CARET_W)).h(px(CARET_H)).bg(p.accent)),
        );
    } else {
        el = el.child(text);
    }
    el
}

#[derive(Clone, Copy)]
enum ToolState {
    Ok,
    Running,
}

/// `.tc`: the 34 px inline tool row.
fn tool_row(p: Palette, id: &'static str, state: ToolState, verb: &'static str, target: &'static str, right: AnyElement) -> impl IntoElement {
    let glyph: AnyElement = match state {
        ToolState::Ok => glyph_ok().into_any_element(),
        ToolState::Running => spinner((ElementId::from(id), "spinner")).into_any_element(),
    };
    h_flex()
        .w_full()
        .h(px(34.0))
        .gap(px(TOOL_GAP))
        .px(px(TOOL_PAD_X))
        .rounded(px(scale::R_MD))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
        .ui(TOOL_TEXT)
        .text_color(p.ink)
        .child(glyph)
        .child(div().medium().child(verb))
        .child(div().min_w(px(0.0)).truncate().mono(scale::FS_12).text_color(p.ink_2).child(target))
        .child(div().flex_1())
        .child(right)
}

pub(crate) fn right_pane(cx: &mut App) -> impl IntoElement {
    let p = cx.aui().colors;
    let file_row = |id: &'static str, kind: FileType, name: &'static str, path: &'static str, status: &'static str, color: Hsla| {
        h_flex()
            .id(id)
            .w_full()
            .h(px(FILE_ROW_H))
            .gap(px(FILE_ROW_GAP))
            .px(px(FILE_ROW_PAD))
            .rounded(px(scale::R_SM))
            .text_color(p.ink_2)
            .hover(|s| s.bg(p.surface_2))
            .child(icon(kind.icon()).color(kind.hue().map(Hsla::from).unwrap_or(p.ink_3)))
            .child(div().child(name))
            .child(tag(path))
            .child(div().flex_1())
            .child(div().text_role(TextRole::MonoSmall).text_px(FILE_STATUS_SIZE).semibold().text_color(color).child(status))
    };
    v_flex()
        .size_full()
        .child(
            h_flex()
                .w_full()
                .pt(px(SCOPE_PAD_TOP))
                .px(px(SCOPE_PAD_X))
                .pb(px(SCOPE_PAD_BOTTOM))
                .gap(px(STATUS_GAP))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child(chip("card10-scope-turn", "This turn").active(true))
                .child(chip("card10-scope-branch", "Branch"))
                .child(chip("card10-scope-unstaged", "Unstaged"))
                .child(div().flex_1())
                .child(tag("3 files · +21 −7")),
        )
        .child(
            v_flex()
                .w_full()
                .pt(px(FILES_PAD_TOP))
                .px(px(FILES_PAD_X))
                .pb(px(FILES_PAD_BOTTOM))
                .gap(px(FILES_GAP))
                .ui(scale::FS_12)
                .child(file_row("card10-file-1", FileType::Ts, "validators.ts", "src/checkout", "M", p.warning))
                .child(file_row("card10-file-2", FileType::Test, "validators.test.ts", "src/checkout", "A", p.success))
                .child(file_row("card10-file-3", FileType::Tsx, "AddressForm.tsx", "src/checkout", "M", p.warning)),
        )
        .child(diff_block(p))
        .child(diff_note_inset(
            &p,
            ElementId::from("card10-note"),
            &DiffNote { line: 46, text: "Also handle 'GB' here, postcode format differs.".into(), pending: false },
            NoteInsets {
                margin: (NOTE_MARGIN_TOP, NOTE_MARGIN_RIGHT, NOTE_MARGIN_BOTTOM, NOTE_MARGIN_LEFT),
                line_height: NOTE_LH,
                caps_gap: NOTE_CAPS_GAP,
            },
            None,
        ))
        .child(div().flex_1())
        .child(
            h_flex()
                .w_full()
                .gap(px(ACTIONS_GAP))
                .py(px(ACTIONS_PAD_Y))
                .px(px(ACTIONS_PAD_X))
                .border_t_1()
                .border_color(p.line)
                .bg(p.surface_2)
                .child(div().ui(scale::FS_11).text_color(p.ink_3).whitespace_nowrap().child("1 note on 1 file"))
                .child(div().flex_1())
                .child(button("card10-commit", "Commit…").ghost().sm())
                .child(button("card10-send", "Send to Claude Code").primary().sm()),
        )
}

/// `.diff`: header row + four lines with gutters.
fn diff_block(p: Palette) -> impl IntoElement {
    let line = |n: &'static str, text: &'static str, kind: Option<bool>| {
        let (bg, gutter) = match kind {
            Some(true) => (Some(p.diff_add), p.success),
            Some(false) => (Some(p.diff_del), p.danger),
            None => (None, p.ink_4),
        };
        let mut l = h_flex().w_full().items_start().px(px(DIFF_LINE_PAD_X));
        if let Some(bg) = bg {
            l = l.bg(bg);
        }
        l.child(div().flex_none().w(px(GUTTER_W)).pr(px(GUTTER_PAD)).text_color(gutter).child(div().w_full().flex().justify_end().child(n)))
            .child(div().flex_1().min_w(px(0.0)).text_color(p.ink).child(text))
    };
    v_flex()
        .my(px(DIFF_MARGIN_Y))
        .mx(px(DIFF_MARGIN_X))
        .rounded(px(scale::R_MD))
        .border_1()
        .border_color(p.line)
        .overflow_hidden()
        .mono(DIFF_TEXT)
        .line_height(relative(DIFF_LH))
        .child(
            h_flex()
                .w_full()
                .gap(px(DIFF_HEAD_GAP))
                .py(px(DIFF_HEAD_PAD_Y))
                .px(px(DIFF_HEAD_PAD_X))
                .bg(p.surface_2)
                .font_family(scale::FONT_UI)
                .text_px(scale::FS_12)
                .text_color(p.ink)
                .child(div().mono(scale::FS_12).line_height(relative(DIFF_LH)).child("validators.ts"))
                .child(h_flex().gap(px(scale::SP_2)).child(tag("+8").color(p.success)).child(tag("−3").color(p.danger)))
                .child(div().flex_1())
                .child(div().font_family(scale::FONT_UI).text_px(scale::FS_11).text_color(p.ink_3).child("1 note")),
        )
        .child(line("44", "export function validateAddress(values) {", None))
        .child(line("45", "  if (!values.country) return true;", Some(false)))
        .child(line("45", "  if (!values.country) return { ok: false, field: 'country' };", Some(true)))
        .child(line("46", "  if (values.country === 'CA') return validateCanadianPostal(values);", Some(true)))
}
