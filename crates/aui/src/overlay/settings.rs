//! The settings dialog: a modal card with a section rail on the left and the
//! selected section's rows on the right.
//!
//! The dialog is stateless like the palette and the modal dialog: the caller
//! passes `sections` and the selected section index every frame and stores
//! the flips the [`SettingsDialog::on_switch`] intent reports. The one state
//! the dialog keeps itself is keyboard plumbing — the card focus, the focused
//! switch row, and whether the open transition already took the keyboard — in
//! element state keyed by the dialog id, the way hover flags live in
//! [`crate::util::interaction`]. Rail rows keep their own tab stops in
//! per-row keyed state beside their hover flags. A modal that cannot be driven
//! from the keyboard out of the box is broken, and the row focus has no
//! caller-side slot in the API, so the card owns
//! [`SETTINGS_CONTEXT`](crate::keys::SETTINGS_CONTEXT) the way a palette host
//! would own [`MENU_CONTEXT`](crate::keys::MENU_CONTEXT): it focuses itself
//! once when it opens, `esc` dismisses, `↑`/`↓` move the row focus across the
//! selected section's switches, and `enter` or `space` flips the focused
//! switch. The section rail rows are tab stops, and the switches keep their
//! own native tab stops, so `⇥` walks rail then page. The focused row wears
//! the same surface step a palette row does.

use std::rc::Rc;

use aui_icons::IconName;
use aui_motion::{tint_fade, EnterExit, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, relative, App, ElementId, FocusHandle, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::switch::Switch;

use super::card::{modal_card, modal_presence, modal_scrim, rest_timing, ModalIntent};
use crate::data::{icon_button, kbd, Button, ButtonSize};
use crate::keys::{Cancel, Confirm, SelectNext, SelectPrev, SETTINGS_CONTEXT};
use crate::util::{interaction_flags, TrackInteraction};

/// The default card width: the rail plus a readable page.
const SETTINGS_W: f32 = 640.0;
/// The section rail: section labels are short ("Sidebar", "Appearance").
const RAIL_W: f32 = 180.0;
/// Card padding and the header/body gap, the dialog's measure.
const CARD_PAD: f32 = scale::SP_5;
const CARD_GAP: f32 = scale::SP_4;
/// The gutter the rail/page divider sits in, and the rows' side padding.
const BODY_GUTTER: f32 = scale::SP_4;
const ROW_PAD_X: f32 = scale::SP_3;
const ROW_GAP: f32 = scale::SP_3;
/// The rail stacks its rows tightly.
const RAIL_GAP: f32 = scale::SP_1;
/// The caps group label inside a page.
const HEADING_PT: f32 = scale::SP_3;
const HEADING_PB: f32 = scale::SP_2;
/// The muted paragraph under a group.
const NOTE_PY: f32 = scale::SP_2;
/// Switch labels read like palette rows; details like palette context lines.
const LABEL_TEXT: f32 = scale::FS_13;
const DETAIL_TEXT: f32 = scale::FS_11;
/// The `esc` keycap and the close glyph share the header's right end.
const HEAD_GAP: f32 = scale::SP_2;

type SectionHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type SwitchHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;

/// One section of the dialog: a rail row and its page.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSection {
    /// Stable identity, reported by [`SettingsDialog::on_select_section`].
    pub id: SharedString,
    /// The rail label.
    pub label: SharedString,
    /// The page rows, in order.
    pub rows: Vec<SettingsRow>,
}

/// One row of a settings page.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsRow {
    /// A labelled switch: the label in ink, the optional detail in ink-3
    /// mono 11 under it, the switch at the right.
    Switch {
        /// Stable identity, reported by [`SettingsDialog::on_switch`].
        id: SharedString,
        /// The row label.
        label: SharedString,
        /// An optional second line under the label.
        detail: Option<SharedString>,
        /// Whether the switch is on. The caller owns this; the dialog only
        /// reports flips.
        on: bool,
    },
    /// A muted paragraph under a group.
    Note {
        /// The paragraph.
        text: SharedString,
    },
    /// A caps group label inside a section.
    Heading {
        /// The label.
        text: SharedString,
    },
}

/// The keyboard state the dialog keeps for itself.
struct SettingsState {
    focus: FocusHandle,
    focused: Option<usize>,
    was_present: bool,
}

/// A settings dialog over `sections` with `selected` section open. Build with
/// [`settings_dialog`].
///
/// The rendered element covers the window it is rendered into (scrim plus
/// card), so render it inside the window's root — through
/// [`super::popover_layer`] when anything else in the tree would paint over
/// it. Unlike [`super::Dialog`] it needs no caller focus wrapper: the card
/// takes the keyboard once when it opens and owns `esc`, the arrows, return
/// and space while it is open.
#[derive(IntoElement)]
pub struct SettingsDialog {
    id: ElementId,
    sections: Vec<SettingsSection>,
    selected: usize,
    width: f32,
    present: bool,
    timing: EnterExit,
    on_select_section: Option<SectionHandler>,
    on_switch: Option<SwitchHandler>,
    on_dismiss: Option<ModalIntent>,
}

/// A settings dialog over `sections` with the `selected`-th section open.
/// `selected` past the last section clamps to it.
pub fn settings_dialog(id: impl Into<ElementId>, sections: Vec<SettingsSection>, selected: usize) -> SettingsDialog {
    SettingsDialog {
        id: id.into(),
        sections,
        selected,
        width: SETTINGS_W,
        present: true,
        timing: EnterExit::DEFAULT,
        on_select_section: None,
        on_switch: None,
        on_dismiss: None,
    }
}

impl SettingsDialog {
    /// A rail row was clicked; the argument is its index in `sections`.
    /// A rail row focused by `⇥` selects through `enter` / `space` too.
    pub fn on_select_section(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select_section = Some(Rc::new(f));
        self
    }

    /// A switch flipped — by clicking it, or by `enter` / `space` on the
    /// focused switch row. The arguments are the row's id and its new value;
    /// the caller stores the flip and passes it back as `on` next frame.
    pub fn on_switch(mut self, f: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_switch = Some(Rc::new(f));
        self
    }

    /// The scrim was clicked, or the `esc` keycap, the close glyph or the
    /// `esc` key was pressed. A click on the card itself does not reach this.
    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }

    /// Whether the dialog is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the dialog is drawn at rest on its first frame, for a
    /// static capture.
    pub fn at_rest(mut self) -> Self {
        self.timing = rest_timing(self.timing);
        self
    }

    /// Overrides the card width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// Moves the row focus one step: `+1` for `↓`, `-1` for `↑`. From no focus,
/// `↓` lands on the first switch and `↑` on the last; at either end the focus
/// stops, the way the palette's arrow keys stop at the first and last rows.
fn step_focus(focused: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match focused {
        Some(at) => {
            let next = at as isize + delta;
            Some(next.clamp(0, count as isize - 1) as usize)
        }
        None => Some(if delta > 0 { 0 } else { count - 1 }),
    }
}

impl RenderOnce for SettingsDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let row_h = cx.aui().metrics.row;
        let id = self.id.clone();
        let style = modal_presence(&id, self.present, self.timing, window, cx);

        let state = window.use_keyed_state((id.clone(), "settings-state"), cx, |_, cx| SettingsState {
            focus: cx.focus_handle(),
            focused: None,
            was_present: false,
        });
        // A modal takes the keyboard once when it opens, and never steals it
        // back afterwards.
        if self.present && !state.read(cx).was_present {
            let focus = state.read(cx).focus.clone();
            window.focus(&focus, cx);
            state.update(cx, |s, _| s.was_present = true);
        } else if !self.present && state.read(cx).was_present {
            state.update(cx, |s, _| s.was_present = false);
        }

        let count = self.sections.len();
        let selected = if count == 0 { 0 } else { self.selected.min(count - 1) };
        // The selected section's switches: (row id, label, detail, on).
        let switches: Vec<(SharedString, SharedString, Option<SharedString>, bool)> = self
            .sections
            .get(selected)
            .map(|section| {
                section
                    .rows
                    .iter()
                    .filter_map(|row| match row {
                        SettingsRow::Switch { id, label, detail, on } => Some((id.clone(), label.clone(), detail.clone(), *on)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let switch_count = switches.len();
        let focused = state.read(cx).focused.filter(|at| *at < switch_count);

        let card_focus = state.read(cx).focus.clone();

        // --- the card-level keyboard: esc, the arrows, return/space --------
        let confirm_rows = switches.clone();
        let confirm_handler = self.on_switch.clone();
        let dismiss_key = self.on_dismiss.clone();
        let dismiss = self.on_dismiss.clone();
        let mut card = modal_card(&p, self.width, &style)
            .p(px(CARD_PAD))
            .gap(px(CARD_GAP))
            .key_context(SETTINGS_CONTEXT)
            .track_focus(&card_focus)
            .on_action({
                let card_state = state.clone();
                move |_: &SelectNext, _, cx| {
                    card_state.update(cx, |s, cx| {
                        s.focused = step_focus(s.focused, switch_count, 1);
                        cx.notify();
                    });
                }
            })
            .on_action({
                let card_state = state.clone();
                move |_: &SelectPrev, _, cx| {
                    card_state.update(cx, |s, cx| {
                        s.focused = step_focus(s.focused, switch_count, -1);
                        cx.notify();
                    });
                }
            })
            .on_action(move |_: &Confirm, w, cx| {
                if let Some(focused) = focused {
                    if let Some((row_id, _, _, on)) = confirm_rows.get(focused).cloned() {
                        if let Some(handler) = &confirm_handler {
                            handler(&row_id, !on, w, cx);
                        }
                    }
                }
            })
            .on_action(move |_: &Cancel, w, cx| {
                if let Some(handler) = &dismiss_key {
                    handler(w, cx);
                }
            });

        card = card.child(header(&id, &p, dismiss));
        card = card.child(body(
            &id,
            &p,
            row_h,
            &self.sections,
            selected,
            focused,
            &switches,
            self.on_select_section.clone(),
            self.on_switch.clone(),
            window,
            cx,
        ));

        modal_scrim(id, style.opacity, self.on_dismiss.clone(), card)
    }
}

/// The header: the title, then the `esc` keycap and the close glyph at the
/// right. All three dismissals report through `on_dismiss`.
fn header(id: &ElementId, p: &Palette, on_dismiss: Option<ModalIntent>) -> impl IntoElement {
    let mut esc = div().id((id.clone(), "esc")).flex_none().ml_auto().child(kbd("esc"));
    if let Some(handler) = on_dismiss.clone() {
        esc = esc.cursor_pointer().on_click(move |_, w, cx| handler(w, cx));
    }
    let mut close: Button = icon_button((id.clone(), "close"), IconName::X).ghost().muted().size(ButtonSize::Sm);
    if let Some(handler) = on_dismiss {
        close = close.on_click(move |_, w, cx| handler(w, cx));
    }
    h_flex()
        .flex_none()
        .w_full()
        .items_center()
        .gap(px(HEAD_GAP))
        .child(div().flex_none().text_role(TextRole::Title).text_color(p.ink).child("Settings"))
        .child(esc)
        .child(close)
}

/// The body: the section rail and the selected section's page, split by a
/// hairline. The rail always draws, even with one section, so later sections
/// have somewhere to land.
#[allow(clippy::too_many_arguments)]
fn body(
    id: &ElementId,
    p: &Palette,
    row_h: gpui::Pixels,
    sections: &[SettingsSection],
    selected: usize,
    focused: Option<usize>,
    switches: &[(SharedString, SharedString, Option<SharedString>, bool)],
    on_select_section: Option<SectionHandler>,
    on_switch: Option<SwitchHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let mut rail = v_flex()
        .flex_none()
        .w(px(RAIL_W))
        .gap(px(RAIL_GAP))
        .pr(px(BODY_GUTTER))
        .border_r_1()
        .border_color(p.line);
    for (i, section) in sections.iter().enumerate() {
        rail = rail.child(rail_row(id, p, row_h, section, i, i == selected, on_select_section.clone(), window, cx));
    }

    let mut page = v_flex().flex_1().min_w(px(0.0)).pl(px(BODY_GUTTER));
    let rows = sections.get(selected).map(|s| s.rows.as_slice()).unwrap_or(&[]);
    // The switch index within the selected section, for the focus highlight.
    let mut switch_at = 0usize;
    for (n, row) in rows.iter().enumerate() {
        match row {
            SettingsRow::Switch { .. } => {
                let (row_id, label, detail, on) = switches[switch_at].clone();
                let on_row = focused == Some(switch_at);
                page = page.child(switch_row(id, p, row_h, switch_at, row_id, label, detail, on, on_row, on_switch.clone(), window, cx));
                switch_at += 1;
            }
            SettingsRow::Note { text } => {
                page = page.child(
                    div()
                        .flex_none()
                        .w_full()
                        .py(px(NOTE_PY))
                        .ui(scale::FS_12)
                        .line_height(relative(scale::LH_UI))
                        .text_color(p.ink_3)
                        .child(text.clone()),
                );
            }
            SettingsRow::Heading { text } => {
                let mut head = div()
                    .flex_none()
                    .w_full()
                    .pb(px(HEADING_PB))
                    .text_role(TextRole::Caps)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(text.to_uppercase());
                // The first row needs no top pad; later groups open with one.
                if n > 0 {
                    head = head.pt(px(HEADING_PT));
                }
                page = page.child(head);
            }
        }
        if n + 1 < rows.len() {
            page = page.child(divider(p));
        }
    }

    h_flex().flex_none().w_full().items_start().child(rail).child(page)
}

/// One rail row: the section label at 12 px, on the surface step while
/// selected — or while its tab stop holds the keyboard. A click, `enter` or
/// `space` on the row selects the section; the row's own activation wins over
/// the card's switch flip, since actions stop at the innermost handler.
#[allow(clippy::too_many_arguments)]
fn rail_row(
    id: &ElementId,
    p: &Palette,
    row_h: gpui::Pixels,
    section: &SettingsSection,
    index: usize,
    selected: bool,
    on_select_section: Option<SectionHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let key: ElementId = (id.clone(), SharedString::from(format!("rail-{index}"))).into();
    let (istate, flags) = interaction_flags(key.clone(), window, cx);
    // The row's own tab stop, kept beside its hover flags. Tab order reads
    // the flags off the handle, not the element, so the handle carries them
    // (the way `Button` does) rather than relying on element-level builders.
    let tab_stop = window.use_keyed_state((key.clone(), "rail-focus"), cx, |_, cx| cx.focus_handle());
    let handle = tab_stop.read(cx).clone().tab_stop(true);
    let on = selected || flags.hovered || handle.is_focused(window);
    let ground = tint_fade((key.clone(), "bg"), on, p.surface_3, Tween::FAST, window, cx);

    let mut row = h_flex()
        .id(key)
        .track_focus(&handle)
        .flex_none()
        .w_full()
        .h(row_h)
        .items_center()
        .px(px(ROW_PAD_X))
        .rounded(px(scale::R_SM))
        .bg(ground)
        .ui(scale::FS_12)
        .line_height(relative(scale::LH_UI))
        .text_color(if selected { p.ink } else { p.ink_2 })
        .cursor_pointer()
        .track_interaction(&istate)
        .child(div().flex_1().min_w(px(0.0)).truncate().child(section.label.clone()));
    if let Some(handler) = on_select_section.clone() {
        row = row.on_click(move |_, w, cx| handler(index, w, cx));
    }
    if let Some(handler) = on_select_section {
        row = row.on_action(move |_: &Confirm, w, cx| handler(index, w, cx));
    }
    row
}

/// One switch row: the label in ink with its detail under it, the accent
/// switch at the right. The row is at least the density row tall and grows
/// past it when the detail line needs the room; only the switch itself flips.
#[allow(clippy::too_many_arguments)]
fn switch_row(
    id: &ElementId,
    p: &Palette,
    row_h: gpui::Pixels,
    switch_at: usize,
    row_id: SharedString,
    label: SharedString,
    detail: Option<SharedString>,
    on: bool,
    highlighted: bool,
    on_switch: Option<SwitchHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let key: ElementId = (id.clone(), SharedString::from(format!("switch-{switch_at}"))).into();
    let (istate, flags) = interaction_flags(key.clone(), window, cx);
    // One highlight, the palette's rule: the pointer owns it while it is over
    // a row, otherwise the keyboard's focused row keeps it.
    let lit = flags.hovered || highlighted;
    let ground = tint_fade((key.clone(), "bg"), lit, p.surface_3, Tween::FAST, window, cx);

    let mut text = v_flex().flex_1().min_w(px(0.0)).child(
        div().flex_none().w_full().truncate().ui(LABEL_TEXT).line_height(relative(scale::LH_UI)).text_color(p.ink).child(label.clone()),
    );
    if let Some(detail) = detail {
        text = text.child(
            div()
                .flex_none()
                .w_full()
                .truncate()
                .mono(DETAIL_TEXT)
                .line_height(relative(scale::LH_MONO))
                .text_color(p.ink_3)
                .child(detail),
        );
    }

    let switch_key: ElementId = (id.clone(), SharedString::from(format!("toggle-{switch_at}"))).into();
    let mut toggle = Switch::new(switch_key).checked(on).color(p.accent).accessibility_label(label);
    if let Some(handler) = on_switch {
        toggle = toggle.on_click(move |next, w, cx| handler(&row_id, *next, w, cx));
    }

    h_flex()
        .id(key)
        .flex_none()
        .w_full()
        .min_h(row_h)
        .items_center()
        .gap(px(ROW_GAP))
        .px(px(ROW_PAD_X))
        .rounded(px(scale::R_SM))
        .bg(ground)
        .track_interaction(&istate)
        .child(text)
        .child(toggle)
}

/// The hairline between two page rows.
fn divider(p: &Palette) -> impl IntoElement {
    div().flex_none().w_full().h(px(1.0)).bg(p.line)
}
