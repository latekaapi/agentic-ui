//! Card 11 · Panel chrome and tabs. The tab strip with its sliding indicator
//! beside the floating-panel header with drop zones and a dragged ghost tab.
//! Reproduces `design/src/cards/shell/11-panel-chrome.html` at 860×460.

use aui::{
    data::{icon_button, ButtonSize},
    shell::{drop_zones, panel_header, tab_ghost, tab_strip, TabItem},
};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.demo{grid-template-columns:1fr 1fr;gap:16px}`.
const COLUMN_GAP: f32 = 16.0;
/// `.strip .btn.icon.xs svg{width:12px;height:12px}`.
const ACTION_GLYPH: f32 = 12.0;
/// `.body{padding:12px;height:150px}`.
const BODY_PAD: f32 = 12.0;
const BODY_HEIGHT: f32 = 150.0;
/// `.ghost{left:150px;top:60px}`.
const GHOST_LEFT: f32 = 150.0;
const GHOST_TOP: f32 = 60.0;
/// `.ds-note{margin-top:12px;max-width:80ch}` — 80ch of Geist 12 ≈ 616 px.
const NOTE_TOP: f32 = 12.0;
const NOTE_MEASURE: f32 = 616.0;

/// The tabs of the left pane.
fn tabs() -> Vec<TabItem> {
    vec![
        TabItem::new("terminal", "Terminal 1", IconName::Terminal),
        TabItem::new("browser", "localhost:3000", IconName::Globe),
        TabItem::new("plan", "PLAN.md", IconName::File).dirty(true),
    ]
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let active_state = window.use_keyed_state("card11-active", cx, |_, _| 0usize);
    let active = *active_state.read(cx);

    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                // The grid rows stretch: the shorter tab-strip pane matches the
                // 36 px header pane's height.
                .items_stretch()
                .gap(px(COLUMN_GAP))
                .child(pane(p).child(strip_row(active, active_state, cx)).child(
                    body(p).child("Click the tabs. The indicator glides on the swap spring; the close affordance only appears on hover or on the active tab, so the strip stays quiet."),
                ))
                .child(
                    pane(p)
                        .child(
                            panel_header("card11-header", "Diff")
                                .icon(IconName::Git)
                                .subtitle("3 files · +21 −7")
                                .action("search", IconName::Search, |_, _, _| {})
                                .action("layout", IconName::Layout, |_, _, _| {})
                                .action("close", IconName::X, |_, _, _| {}),
                        )
                        .child(
                            body(p)
                                .child("Dragging a tab over another panel shows five drop zones: centre to tab-merge, edges to split.")
                                // `.zones i.c{opacity:.9;border-style:solid}`: the card shows the
                                // resting overlay, so no zone is hot under the pointer.
                                .child(drop_zones("card11-zones", true))
                                .child(div().absolute().left(px(GHOST_LEFT)).top(px(GHOST_TOP)).child(tab_ghost(Some(IconName::Globe), "localhost:3000"))),
                        ),
                ),
        )
        .child(note(p))
        .into_any_element()
}

/// `.pan{border:1px solid var(--line);border-radius:12px;background:var(--surface-1);overflow:hidden}`.
fn pane(p: Palette) -> Div {
    v_flex()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line)
        .bg(p.surface_1)
}

/// `.body{padding:12px;height:150px;position:relative;font-size:12px;color:var(--ink-3)}`.
fn body(p: Palette) -> Div {
    div().relative().flex_none().h(px(BODY_HEIGHT)).p(px(BODY_PAD)).ui(scale::FS_12).text_color(p.ink_3)
}

/// The 34 px strip: the tab strip with the `+`, split and dots actions in
/// the same band; the strip draws its own bottom hairline.
fn strip_row(active: usize, state: Entity<usize>, _cx: &mut App) -> impl IntoElement {
    let action = |name: &'static str, glyph: IconName| icon_button(name, glyph).ghost().size(ButtonSize::Xs).icon_size(px(ACTION_GLYPH));
    tab_strip("card11-strip", tabs(), active)
        .on_select(move |id, _, cx| {
            let index = tabs().iter().position(|t| &t.id == id).unwrap_or(0);
            state.update(cx, |value, cx| {
                if *value != index {
                    *value = index;
                    cx.notify();
                }
            });
        })
        .after_tabs(action("card11-plus", IconName::Plus))
        .trailing(action("card11-split", IconName::Split))
        .trailing(action("card11-dots", IconName::Dots))
}

/// `.ds-note{font-size:12px;color:var(--ink-3);margin-top:12px;max-width:80ch}`.
fn note(p: Palette) -> impl IntoElement {
    div()
        .mt(px(NOTE_TOP))
        .max_w(px(NOTE_MEASURE))
        .ui(scale::FS_12)
        .text_color(p.ink_3)
        .child("In the app shell the right pane's tab strip sits in its 44 px header cell with a single close control; pane-specific actions (scope chips, split, search) live inside the pane body. The 36 px panel header with grip and actions is for floating and docked panels outside the shell.")
}
