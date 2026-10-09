//! Screens v2 · sidebar. The Baaz sidebar redesigned on the v2 tokens:
//! the populated sidebar beside an empty pane, its four states, and the
//! compact density.
//!
//! All three cards honour `AUI_GALLERY_STEPS` (comma separated):
//! `hover:<n>` hovers the nth session row, `select:<id>` selects that
//! session, `mode:browse` / `mode:sessions` switch lists, `collapse`
//! collapses to the strip, `leave` clears the hover — so relay can capture
//! each state with `--screenshot-delay`.

use aui::interact::surface;
use aui::nav::{flatten_sidebar, sidebar_list_state, Grouping, SessionScope, SidebarRow};
use aui::screens::sidebar::{sample_model, screen_item_count, sidebar_screen, Mode, SidebarModel};
use aui_tokens::Density;
use gpui::{div, prelude::*, px, AnyElement, App, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

/// The populated sidebar's width beside the pane.
const SIDEBAR_W: f32 = 260.0;
/// A states column: 260 wide, 720 tall.
const COLUMN_W: f32 = 260.0;
const COLUMN_H: f32 = 720.0;
/// The full-bleed window: 1440 × 900.
const WINDOW_W: f32 = 1440.0;
const WINDOW_H: f32 = 900.0;
/// The gap between states columns.
const COLUMN_GAP: f32 = 16.0;

/// One gallery step from `AUI_GALLERY_STEPS`.
fn gallery_steps() -> Vec<String> {
    std::env::var("AUI_GALLERY_STEPS")
        .map(|value| {
            value.split(',').map(|step| step.trim().to_string()).filter(|step| !step.is_empty()).collect()
        })
        .unwrap_or_default()
}

/// The nth session id in flatten order, for `hover:<n>`.
fn nth_session(model: &SidebarModel, n: usize) -> Option<SharedString> {
    let rows = flatten_sidebar(&model.grouping, false);
    let mut seen = 0;
    for row in &rows {
        if let SidebarRow::Session { scope, group, session } = row {
            if seen == n {
                let summary = match (model.grouping.as_ref(), scope) {
                    (Grouping::Status(groups), SessionScope::Status) => {
                        groups.get(*group).and_then(|g| g.sessions.get(*session))
                    }
                    (Grouping::Project(groups), SessionScope::Project) => {
                        groups.get(*group).and_then(|g| g.sessions.get(*session))
                    }
                    (Grouping::Date(groups), _) => {
                        groups.get(*group).and_then(|g| g.sessions.get(*session))
                    }
                    _ => None,
                };
                return summary.map(|s| s.id.clone());
            }
            seen += 1;
        }
    }
    None
}

/// Applies the gallery steps to a model.
fn apply_steps(mut model: SidebarModel) -> SidebarModel {
    for step in gallery_steps() {
        if let Some(rest) = step.strip_prefix("hover:") {
            if let Ok(n) = rest.parse::<usize>() {
                model.hovered = nth_session(&model, n);
            }
        } else if let Some(rest) = step.strip_prefix("select:") {
            model.selected = Some(rest.to_string().into());
        } else if step == "mode:browse" {
            model.mode = Mode::Browse;
        } else if step == "mode:sessions" {
            model.mode = Mode::Sessions;
        } else if step == "collapse" {
            model.collapsed = true;
        } else if step == "leave" {
            model.hovered = None;
        }
    }
    model
}

/// The sidebar at `width` in a fixed-height frame, with its own list state.
fn sidebar_column(
    id: &'static str,
    model: SidebarModel,
    width: f32,
    height: f32,
) -> AnyElement {
    let state = sidebar_list_state(screen_item_count(&model));
    div()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .child(sidebar_screen(id, model, state).width(px(width)).on_intent(|_, _, _| {}))
        .into_any_element()
}

/// `screens/sidebar`: the populated sidebar at 260 px beside an empty
/// `surface(2)` pane, dark, full-bleed at 1440 × 900.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let _ = cx;
    let model = apply_steps(sample_model());
    h_flex()
        .w(px(WINDOW_W))
        .h(px(WINDOW_H))
        .child(sidebar_column("screens-sidebar", model, SIDEBAR_W, WINDOW_H))
        .child(div().flex_1().h_full().child(surface(2, div().size_full())))
        .into_any_element()
}

/// `screens/sidebar-states`: loading, empty, populated (hover on row 3,
/// selection on row 1) and Browse mode as four 260 × 720 columns.
pub fn build_states(_window: &mut Window, cx: &mut App) -> AnyElement {
    let _ = cx;
    let loading = SidebarModel { loading: true, ..sample_model() };

    let mut empty = sample_model();
    empty.grouping = std::rc::Rc::new(aui::nav::Grouping::Date(Vec::new()));
    empty.selected = None;

    let mut populated = sample_model();
    populated.hovered = nth_session(&populated, 2);

    let mut browse = sample_model();
    browse.mode = Mode::Browse;
    browse.selected = None;

    let loading = apply_steps(loading);
    let empty = apply_steps(empty);
    let populated = apply_steps(populated);
    let browse = apply_steps(browse);

    h_flex()
        .gap(px(COLUMN_GAP))
        .child(sidebar_column("screens-sidebar-loading", loading, COLUMN_W, COLUMN_H))
        .child(sidebar_column("screens-sidebar-empty", empty, COLUMN_W, COLUMN_H))
        .child(sidebar_column("screens-sidebar-populated", populated, COLUMN_W, COLUMN_H))
        .child(sidebar_column("screens-sidebar-browse", browse, COLUMN_W, COLUMN_H))
        .into_any_element()
}

/// `screens/sidebar-compact`: the populated sidebar under
/// `Density::Compact`.
pub fn build_compact(window: &mut Window, cx: &mut App) -> AnyElement {
    aui_tokens::AuiTheme::set_density(Density::Compact, Some(window), cx);
    let model = apply_steps(sample_model());
    v_flex()
        .child(sidebar_column("screens-sidebar-compact", model, COLUMN_W, COLUMN_H))
        .into_any_element()
}
