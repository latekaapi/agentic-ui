//! The flat primitives of docs/83 (plan task 2.1), one card each, every
//! state pinned with the one model's `preview` so hover and press render
//! without a pointer. Both themes.

use aui::data::{banner, button, card, checkbox, field, icon_button, section, segmented, settings_row, switch, tabs, CheckboxState};
use aui::interact::{dropdown, list_row, DropdownEntry, DropdownItem, InteractState, PaintStates, StatePaint, R_ITEM};
use aui_icons::{icon, IconName};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, AnyElement, App, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

fn caption(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div().role_in(TextRole::Caption, cx).text_color(cx.aui().colors.fg_muted).child(text.into())
}

/// A labelled column: the state's name over the element.
fn tile(name: &'static str, el: impl IntoElement, cx: &App) -> impl IntoElement {
    v_flex().flex_1().min_w(px(0.0)).gap(px(6.0)).child(caption(name, cx)).child(h_flex().child(el))
}

const STATES: [(&str, Option<InteractState>); 3] =
    [("rest", None), ("hover", Some(InteractState::HOVER)), ("pressed", Some(InteractState::PRESSED))];

/// `primitives/hover-model`: [`StatePaint`] on three grounds.
pub fn build_hover_model(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let swatch = |id: String, paint: StatePaint, cx: &App| {
        div()
            .id(SharedString::from(id))
            .h(px(32.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .rounded(px(R_ITEM))
            .role_in(TextRole::Body, cx)
            .paint_states(paint, cx)
            .child("Archive")
    };
    let line = |name: &'static str, base: Option<gpui::Hsla>, ink: (gpui::Hsla, gpui::Hsla), cx: &App| {
        let make = |p: StatePaint| match base {
            Some(b) => p.on(b).ink(ink.0, ink.1),
            None => p.ink(ink.0, ink.1),
        };
        let mut row = h_flex().w_full().gap(px(8.0));
        for (state, pin) in STATES {
            row = row.child(tile(state, swatch(format!("hm-{name}-{state}"), make(StatePaint::new().preview(pin)), cx), cx));
        }
        row = row
            .child(tile("selected", swatch(format!("hm-{name}-sel"), make(StatePaint::new().selected(true)), cx), cx))
            .child(tile("highlighted", swatch(format!("hm-{name}-hl"), make(StatePaint::new().highlighted(true)), cx), cx))
            .child(tile("disabled", swatch(format!("hm-{name}-dis"), make(StatePaint::new().disabled(true)), cx), cx));
        v_flex().w_full().gap(px(6.0)).child(div().role_in(TextRole::Body, cx).text_color(cx.aui().colors.fg).child(name)).child(row)
    };
    v_flex()
        .w_full()
        .gap(px(16.0))
        .child(line("Bare (a row, a ghost control)", None, (p.fg_muted, p.fg), cx))
        .child(line("On surface-4 (a secondary control)", Some(p.surface_4), (p.fg, p.fg), cx))
        .child(line("On the primary fill", Some(p.accent), (p.bg, p.bg), cx))
        .into_any_element()
}

/// `primitives/button`: every variant in every state.
pub fn build_buttons(_window: &mut Window, cx: &mut App) -> AnyElement {
    type Style = fn(aui::data::Button) -> aui::data::Button;
    let variants: [(&str, Style); 5] = [
        ("Secondary", |b| b),
        ("Primary", |b| b.primary()),
        ("Ghost", |b| b.ghost()),
        ("Outline", |b| b.outline()),
        ("Danger", |b| b.danger()),
    ];
    let states: [(&str, Option<InteractState>, bool); 5] = [
        ("rest", None, false),
        ("hover", Some(InteractState::HOVER), false),
        ("pressed", Some(InteractState::PRESSED), false),
        ("focused", Some(InteractState::FOCUSED), false),
        ("disabled", None, true),
    ];
    let mut head = h_flex().w_full().gap(px(8.0)).child(div().w(px(90.0)).flex_none());
    for (state, _, _) in states {
        head = head.child(div().flex_1().child(caption(state, cx)));
    }
    let mut col = v_flex().w_full().gap(px(10.0)).child(head);
    for (name, style) in variants {
        let mut row = h_flex().w_full().gap(px(8.0)).items_center().child(div().w(px(90.0)).flex_none().child(caption(name, cx)));
        for (state, pin, disabled) in states {
            let mut b = style(button(SharedString::from(format!("btn-{name}-{state}")), "Continue")).disabled(disabled);
            if let Some(pin) = pin {
                b = b.preview(pin);
            }
            row = row.child(div().flex_1().child(b));
        }
        col = col.child(row);
    }
    let icons = h_flex()
        .gap(px(8.0))
        .items_center()
        .child(caption("Icon, ghost:", cx))
        .child(icon_button("btn-i-rest", IconName::Dots).ghost().accessibility_label("More"))
        .child(icon_button("btn-i-hover", IconName::Dots).ghost().accessibility_label("More").preview(InteractState::HOVER))
        .child(icon_button("btn-i-toggled", IconName::Dots).ghost().accessibility_label("More").toggled(true))
        .child(icon_button("btn-i-open", IconName::Dots).ghost().accessibility_label("More").menu_open(true));
    col.child(icons).into_any_element()
}

/// `primitives/list-row`: the flat row in each state.
pub fn build_list_row(_window: &mut Window, cx: &mut App) -> AnyElement {
    let lead = || icon(IconName::Folder).size(px(16.0));
    let age = |t: &'static str, cx: &App| div().role_in(TextRole::Micro, cx).text_color(cx.aui().colors.ink_4).child(t);
    let entry = |name: &'static str, row: aui::interact::Row, cx: &App| {
        h_flex().w_full().gap(px(12.0)).items_center().child(div().w(px(80.0)).flex_none().child(caption(name, cx))).child(div().flex_1().child(row))
    };
    v_flex()
        .w_full()
        .gap(px(4.0))
        .child(entry("rest", list_row("lr-rest", "Checkout flow").trailing(age("2h", cx)), cx))
        .child(entry("hover", list_row("lr-hover", "Checkout flow").trailing(age("2h", cx)).preview(InteractState::HOVER), cx))
        .child(entry("pressed", list_row("lr-pressed", "Checkout flow").trailing(age("2h", cx)).preview(InteractState::PRESSED), cx))
        .child(entry("selected", list_row("lr-selected", "Checkout flow").trailing(age("2h", cx)).selected(true), cx))
        .child(entry("sel + hover", list_row("lr-selhover", "Checkout flow").selected(true).preview(InteractState::HOVER), cx))
        .child(entry("disabled", list_row("lr-disabled", "Checkout flow").disabled(true), cx))
        .child(entry("leading", list_row("lr-lead", "acme-web").leading(lead()), cx))
        .child(entry("lead + hover", list_row("lr-lead-h", "acme-web").leading(lead()).preview(InteractState::HOVER), cx))
        .child(entry("two-line", list_row("lr-two", "Auth refresh").two_line("PR #2491 open · 3 ahead"), cx))
        .into_any_element()
}

/// `primitives/menu-panel`: a dropdown at rest with one lit row.
pub fn build_menu_panel(_window: &mut Window, cx: &mut App) -> AnyElement {
    let menu = dropdown("prim-menu", "Session")
        .at_rest()
        .width(px(240.0))
        // Entry indices count labels and separators: 2 is "Pin".
        .highlighted(Some(2))
        .entry(DropdownEntry::Label("Session".into()))
        .entry(DropdownItem::new("Rename").icon(IconName::Edit).keys("⌘R"))
        .entry(DropdownItem::new("Pin").icon(IconName::Pin))
        .entry(DropdownEntry::Separator)
        .entry(DropdownItem::new("Sort by date").current(true))
        .entry(DropdownItem::new("Sort by name"))
        .entry(DropdownItem::new("Export").disabled(true))
        .entry(DropdownEntry::Separator)
        .entry(DropdownItem::new("Archive").danger());
    h_flex()
        .w_full()
        .gap(px(24.0))
        .items_start()
        .child(div().flex_none().child(menu))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(6.0))
                .child(caption("“Pin” is the lit row: the keys’ or the pointer’s, one, never two.", cx))
                .child(caption("“Sort by date” is the current row: ov-selected and the check.", cx)),
        )
        .into_any_element()
}

/// `primitives/card`: a titled card with sections, and a bare card.
pub fn build_card(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let body = |t: &'static str, cx: &App| div().role_in(TextRole::Body, cx).text_color(cx.aui().colors.fg_muted).child(t);
    v_flex()
        .w_full()
        .gap(px(16.0))
        .child(
            card()
                .title("Read crates/aui/src/interact/paint.rs", Some(div().role_in(TextRole::Caption, cx).text_color(p.fg_muted).child("142 lines").into_any_element()))
                .section(body("The one hover / press / selected model.", cx))
                .section(body("Sections are separated by a 1 px line; no border, no shadow.", cx)),
        )
        .child(card().section(body("A card with no header: one section on surface-2.", cx)))
        .into_any_element()
}

/// `primitives/section`: a settings section with every kind of control.
pub fn build_section(window: &mut Window, cx: &mut App) -> AnyElement {
    let _ = window;
    v_flex()
        .w_full()
        .child(
            section("General")
                .row(settings_row("Dark theme").description("Follow the system, or pick one.").control(switch("sec-sw", true, false).accessibility_label("Dark theme")))
                .row(
                    settings_row("Approvals")
                        .description("What an agent may do without asking.")
                        .control(segmented("sec-seg", vec!["Ask".into(), "Auto".into(), "Read only".into()], 0).accessibility_label("Approvals")),
                )
                .row(settings_row("Default folder").description("Where new sessions start.").control(field("sec-field", false, cx).w(px(200.0)).role_in(TextRole::Body, cx).child("~/Projects")))
                .row(settings_row("Archived sessions").description("12 sessions").control(button("sec-btn", "Show"))),
        )
        .into_any_element()
}

/// `primitives/field`: the input frame at rest and focused.
pub fn build_field(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .gap(px(12.0))
        .child(tile("rest, placeholder", field("f-rest", false, cx).w(px(320.0)).role_in(TextRole::Body, cx).text_color(p.ink_4).child("Search sessions"), cx))
        .child(tile("focused", field("f-focus", true, cx).w(px(320.0)).role_in(TextRole::Body, cx).text_color(p.fg).child("checkout"), cx))
        .child(tile(
            "multi-line grows",
            field("f-multi", false, cx).w(px(320.0)).role_in(TextRole::Body, cx).text_color(p.fg).child("Say what you would rather happen,\nand the agent will try again."),
            cx,
        ))
        .into_any_element()
}

/// `primitives/banner`: three banners.
pub fn build_banner(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .gap(px(10.0))
        .child(banner("bn-plan", "Plan mode is on: the agent reads but does not change files.").leading(icon(IconName::Doc).size(px(16.0))).action("Turn off", |_, _| {}))
        .child(banner("bn-signed-out", "Codex is signed out.").leading(div().size(px(6.0)).rounded_full().bg(p.warning)).action("Sign in", |_, _| {}))
        .child(banner("bn-lost", "The agent process stopped."))
        .into_any_element()
}

/// `primitives/segmented-tabs`: both in rest, hover, pressed and focused.
pub fn build_segmented_tabs(_window: &mut Window, cx: &mut App) -> AnyElement {
    let labels = || vec![SharedString::from("Ask"), "Auto".into(), "Read only".into()];
    let tab_labels = || vec![SharedString::from("Changes"), "Files".into(), "Terminal".into()];
    v_flex()
        .w_full()
        .gap(px(14.0))
        .child(tile("segmented · rest", segmented("st-seg-rest", labels(), 1).accessibility_label("Approvals"), cx))
        .child(tile("segmented · “Read only” hovered", segmented("st-seg-hover", labels(), 1).accessibility_label("Approvals").preview(2, InteractState::HOVER), cx))
        .child(tile("segmented · “Ask” pressed", segmented("st-seg-press", labels(), 1).accessibility_label("Approvals").preview(0, InteractState::PRESSED), cx))
        .child(tile("segmented · focus-visible", segmented("st-seg-focus", labels(), 1).accessibility_label("Approvals").preview(0, InteractState::FOCUSED), cx))
        .child(tile("tabs · rest", tabs("st-tabs-rest", "Views", tab_labels(), 0), cx))
        .child(tile("tabs · “Files” hovered", tabs("st-tabs-hover", "Views", tab_labels(), 0).preview(1, InteractState::HOVER), cx))
        .child(tile("tabs · “Terminal” pressed", tabs("st-tabs-press", "Views", tab_labels(), 0).preview(2, InteractState::PRESSED), cx))
        .into_any_element()
}

/// `primitives/switch-checkbox`.
pub fn build_switch_checkbox(_window: &mut Window, cx: &mut App) -> AnyElement {
    let sw = h_flex()
        .gap(px(24.0))
        .child(tile("on", switch("sc-on", true, false).accessibility_label("On"), cx))
        .child(tile("off", switch("sc-off", false, false).accessibility_label("Off"), cx))
        .child(tile("on, disabled", switch("sc-on-d", true, true).accessibility_label("On, disabled"), cx))
        .child(tile("off, disabled", switch("sc-off-d", false, true).accessibility_label("Off, disabled"), cx));
    let cb = h_flex()
        .gap(px(24.0))
        .child(tile("off", checkbox("cb-off", CheckboxState::Off, false).accessibility_label("Off"), cx))
        .child(tile("off, hover", checkbox("cb-hover", CheckboxState::Off, false).accessibility_label("Hover").preview(InteractState::HOVER), cx))
        .child(tile("on", checkbox("cb-on", CheckboxState::On, false).accessibility_label("On"), cx))
        .child(tile("mixed", checkbox("cb-mixed", CheckboxState::Mixed, false).accessibility_label("Mixed"), cx))
        .child(tile("focused", checkbox("cb-focus", CheckboxState::Off, false).accessibility_label("Focused").preview(InteractState::FOCUSED), cx))
        .child(tile("disabled", checkbox("cb-dis", CheckboxState::On, true).accessibility_label("Disabled"), cx));
    v_flex().w_full().gap(px(20.0)).child(sw).child(cb).into_any_element()
}
