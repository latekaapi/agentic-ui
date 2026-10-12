//! `shell/v3`: the v3 shell (canvas "Baaz v3" v9, harness docs/31 §9) as a
//! window the owner can drive — Sessions and Browse, sidebar open and
//! closed, the two pills, the peek. Click the sidebar toggle (or "Show
//! sidebar" in the pill) to close and open; the switch changes mode; with
//! the sidebar closed, rest the pointer on the window's left edge to peek.
//!
//! The transcript is the canvas's own fixture (harness
//! `fixtures/msp/synthetic-markdown.jsonl`) through the real turn
//! components; the Browse page is the gallery's stand-in site (Baaz shows
//! the live Chromium page there). The header's actions and the page
//! controls are drawn but do nothing here.
//!
//! `AUI_GALLERY_STEPS`: `mode:browse`, `collapse` (start closed), `peek`
//! (start closed and peeking is not scriptable — hover the edge).

use std::cell::RefCell;
use std::collections::HashMap;

use aui::composer::{composer, composer_state_rows};
use aui::data::{icon_button, ButtonSize};
use aui::interact::FluidHandle;
use aui::nav::sidebar_list_state;
use aui::screens::sidebar::{mode_switch, sample_model, screen_item_count, sidebar_screen, Mode, SidebarIntent, SIDEBAR_WIDTH};
use aui::shell::app_shell;
use aui::transcript::{assistant_turn, user_turn};
use aui_icons::{icon, IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, Entity, IntoElement, Window};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::base::input::TextareaState;

const TITLE: &str = "Show me the validation summary with code and links.";
const ANSWER: &str = "## Validation summary\n\nCA and GB now branch per country, with focused tests over both.\n\n- CA outward codes accept the space form\n- GB outward codes still fail on the space (next)\n- Full suite stays green: `cargo test --workspace`\n\n```rust\nfn validate(code: &str, country: Country) -> bool {\n    match country {\n        Country::Ca => CA_RE.is_match(code),\n        Country::Gb => GB_RE.is_match(code),\n    }\n}\n```\n\n| country | cases | result |\n| --- | --- | --- |\n| CA | 14 | pass |\n| GB | 11 | 1 failing |\n\nNotes live in [the plan](https://example.com/plan) and the harness doc at docs/02-app.md.\n";
/// The transcript measure (Baaz's `TRANSCRIPT_MEASURE`).
const MEASURE: f32 = 880.0;
/// The composer sits 12 px inside the sheet's bottom (18 from the window).
const COMPOSER_BOTTOM: f32 = 12.0;
/// The transcript's top pad under the title row / the pills.
const TOP_OPEN: f32 = 56.0;
const TOP_CLOSED: f32 = 76.0;

#[derive(Clone, Copy)]
struct V3 {
    mode: Mode,
    open: bool,
}

fn steps() -> Vec<String> {
    std::env::var("AUI_GALLERY_STEPS").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default()
}

thread_local! {
    static LISTS: RefCell<HashMap<&'static str, (gpui::ListState, FluidHandle)>> = RefCell::new(HashMap::new());
}

/// The v2 sidebar on the frame (no ground of its own), its list state kept
/// across renders the way a host keeps it.
fn sidebar(id: &'static str, mode: Mode, state: &Entity<V3>) -> AnyElement {
    let mut model = sample_model();
    model.mode = mode;
    let count = screen_item_count(&model);
    let (list, fluid) = LISTS.with(|c| c.borrow_mut().entry(id).or_insert_with(|| (sidebar_list_state(count), FluidHandle::new())).clone());
    if list.item_count() != count {
        list.reset(count);
    }
    let state = state.clone();
    sidebar_screen(id, model, list, fluid)
        .width(px(SIDEBAR_WIDTH))
        .transparent()
        .on_intent(move |intent, _, cx| {
            state.update(cx, |s, cx| {
                match intent {
                    SidebarIntent::Collapse => s.open = !s.open,
                    SidebarIntent::SwitchMode(mode) => s.mode = mode,
                    _ => return,
                }
                cx.notify();
            })
        })
        .into_any_element()
}

fn action(id: &'static str, glyph: IconName, label: &'static str) -> aui::data::Button {
    icon_button(id, glyph).ghost().size(ButtonSize::Sm).accessibility_label(label)
}

/// The Sessions centre: the transcript on the measure and the raised
/// composer, 12 px inside the sheet's bottom.
fn sessions_centre(open: bool, composer_state: &Entity<TextareaState>) -> AnyElement {
    let transcript = div()
        .id("v3-transcript")
        .size_full()
        .overflow_y_scroll()
        .child(
            v_flex()
                .w_full()
                .max_w(px(MEASURE))
                .mx_auto()
                .pt(px(if open { TOP_OPEN } else { TOP_CLOSED }))
                .pb(px(160.0))
                .gap(px(scale::SP_6))
                .child(user_turn("v3-user", TITLE))
                .child(assistant_turn("v3-answer", ANSWER)),
        );
    let composer = div()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(COMPOSER_BOTTOM))
        .flex()
        .justify_center()
        .child(div().w_full().max_w(px(MEASURE)).child(composer("v3-composer", composer_state, Provider::Muse, "Muse Spark 1.3").raised(true).mode("Auto").effort("Default")));
    div().relative().size_full().child(transcript).child(composer).into_any_element()
}

/// Builds the card.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let steps = steps();
    let state = window.use_keyed_state("v3-shell-state", cx, |_, _| V3 {
        mode: if steps.iter().any(|s| s == "mode:browse") { Mode::Browse } else { Mode::Sessions },
        open: !steps.iter().any(|s| s == "collapse"),
    });
    let composer_state = window.use_keyed_state("v3-composer-state", cx, |w, cx| composer_state_rows("Ask Muse, or type / for commands", 1, 8, w, cx));
    let s = *state.read(cx);
    let p = cx.aui().colors;
    let browse = s.mode == Mode::Browse;

    let toggle = |id: &'static str, label: &'static str| {
        let state = state.clone();
        action(id, IconName::Sidebar, label).on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                s.open = !s.open;
                cx.notify();
            })
        })
    };
    let switch = {
        let state = state.clone();
        mode_switch("v3-pill-switch", s.mode).on_switch(move |mode, _, cx| {
            state.update(cx, |s, cx| {
                s.mode = mode;
                cx.notify();
            })
        })
    };
    let session_actions = || h_flex().gap(px(2.0)).child(action("v3-term", IconName::Terminal, "Terminal")).child(action("v3-menu", IconName::Dots, "Session menu")).child(action("v3-right", IconName::PanelRight, "Show right pane"));

    let (left, right): (AnyElement, AnyElement) = if browse {
        let sep = div().w(px(1.0)).h(px(16.0)).mx(px(6.0)).bg(p.line);
        let address = h_flex()
            .h(cx.aui().metrics.control_md)
            .pl(px(8.0))
            .pr(px(10.0))
            .gap(px(8.0))
            .items_center()
            .whitespace_nowrap()
            .text_px(scale::FS_13)
            .text_color(p.fg_muted)
            .child(icon(IconName::Shield).size(px(14.0)).color(p.fg_muted))
            .child(div().semibold().text_color(p.fg).child("Fluid Functionalism"))
            .child("www.fluidfunctionalism.com");
        (
            h_flex().items_center().child(toggle("v3-show", "Show sidebar")).child(switch).child(sep).child(action("v3-back", IconName::ArrowLeft, "Back")).child(action("v3-fwd", IconName::ArrowRight, "Forward")).child(action("v3-reload", IconName::Refresh, "Reload")).child(address).into_any_element(),
            h_flex().gap(px(2.0)).child(action("v3-ask", IconName::Sparkle, "Ask")).child(action("v3-page-menu", IconName::Dots, "Page menu")).child(action("v3-new-tab", IconName::Plus, "New tab")).into_any_element(),
        )
    } else {
        (
            h_flex()
                .items_center()
                .child(toggle("v3-show", "Show sidebar"))
                .child(switch)
                .child(div().ml(px(8.0)).mr(px(10.0)).whitespace_nowrap().text_px(scale::FS_13).semibold().text_color(p.fg).child(TITLE))
                .into_any_element(),
            session_actions().into_any_element(),
        )
    };

    let centre = if browse {
        crate::cards::screen_sidebar::site_page_content(cx).into_any_element()
    } else {
        sessions_centre(s.open, &composer_state)
    };
    let title_row = h_flex()
        .w_full()
        .items_center()
        .child(div().flex_1().min_w(px(0.0)).truncate().text_px(scale::FS_13).semibold().text_color(p.fg).child(TITLE))
        .child(session_actions());

    let shell = app_shell("v3-shell")
        .sheets(true)
        .traffic_lights(true)
        .page_sheet(browse)
        .sidebar_open(s.open)
        .right_open(false)
        .sidebar(sidebar("v3-sidebar", s.mode, &state))
        .peek(sidebar("v3-peek", s.mode, &state))
        .header_centre(title_row)
        .pills(left, right)
        .centre(centre);
    div().w(px(1440.0)).h(px(900.0)).child(shell).into_any_element()
}
