//! The settings dialog: a modal card with a section rail on the left and the
//! selected section's rows on the right.
//!
//! The dialog is stateless like the palette and the modal dialog: the caller
//! passes `sections` and the selected section index every frame and stores
//! the flips the [`SettingsDialog::on_switch`] intent reports, and the shortcut
//! edits the [`SettingsDialog::on_shortcut`] intent reports. The one state
//! the dialog keeps itself is keyboard plumbing — the card focus, the focused
//! row, and whether the open transition already took the keyboard — in
//! element state keyed by the dialog id, the way hover flags live in
//! [`crate::util::interaction`]. Rail rows keep their own tab stops in
//! per-row keyed state beside their hover flags. A modal that cannot be driven
//! from the keyboard out of the box is broken, and the row focus has no
//! caller-side slot in the API, so the card owns
//! [`SETTINGS_CONTEXT`](crate::keys::SETTINGS_CONTEXT) the way a palette host
//! would own [`MENU_CONTEXT`](crate::keys::MENU_CONTEXT): it focuses itself
//! once when it opens, `esc` dismisses, `↑`/`↓` move the row focus across the
//! selected section's switches and shortcut rows, and `enter` or `space`
//! flips the focused switch or presses the focused shortcut row. The section
//! rail rows are tab stops, and the switches keep their own native tab stops,
//! so `⇥` walks rail then page. The focused row wears the same surface step
//! a palette row does.
//!
//! While any shortcut row is recording, the card-level key handling captures
//! the next keystroke and reports it through `on_shortcut` instead of moving
//! the row focus: `↓` binds rather than stepping down, `esc` cancels the
//! recording without dismissing the dialog, and `enter` / `space` bind
//! rather than flipping — the `Confirm` handler re-enables propagation with
//! `cx.propagate()` (bubble-phase action listeners stop it by default, so
//! merely returning would swallow the keystroke) so the key-down capture
//! below still sees the exact key.

use std::rc::Rc;

use aui_icons::IconName;
use aui_motion::{tint_fade, EnterExit, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, relative, App, ElementId, FocusHandle, IntoElement, Keystroke, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::switch::Switch;

use super::card::{modal_card, modal_presence, modal_scrim, rest_timing, ModalIntent};
use crate::data::{icon_button, kbd, pill, Button, ButtonSize, PillVariant};
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
/// A reserved shortcut row keeps its keycaps at this opacity: readable but
/// clearly not interactive. A ratio, not a colour, size or duration, matching
/// the disabled-button precedent elsewhere in the library.
const RESERVED_OPACITY: f32 = 0.55;

type SectionHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type SwitchHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type ShortcutHandler = Rc<dyn Fn(&SharedString, ShortcutEdit, &mut Window, &mut App)>;

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
    /// A shortcut row: the label in ink with its detail under it, exactly the
    /// switch row's layout, and the binding at the right — keycaps plus a
    /// clear affordance when bound, an "unassigned" placeholder when not, a
    /// "Press a key…" pill in the accent role while recording, dimmed
    /// keycaps with no press target when reserved.
    Shortcut {
        /// Stable identity, reported back through `on_shortcut`.
        id: SharedString,
        /// The row label, e.g. "Open the command palette".
        label: SharedString,
        /// An optional second line under the label.
        detail: Option<SharedString>,
        /// The current binding as gpui's `KeyBinding` spells it, e.g.
        /// `"cmd-shift-k"`; `None` when the action is unbound.
        keystroke: Option<SharedString>,
        /// True while this row is capturing. The host owns this; the dialog
        /// only reports what happened.
        recording: bool,
        /// False when the binding is reserved and refuses to be rebound.
        /// A reserved row still shows its keystroke; it just cannot be pressed.
        editable: bool,
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

/// What a shortcut row asks for, reported through
/// [`SettingsDialog::on_shortcut`]. The first argument to the handler is the
/// row's [`SettingsRow::Shortcut`] `id`; the host owns all state and stores
/// the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutEdit {
    /// The row was pressed. The host should set `recording` on that row.
    Record,
    /// A keystroke was captured while recording, as gpui spells it (see
    /// [`SettingsDialog::on_shortcut`]): the host should store it as the
    /// row's binding and clear `recording`.
    Set(SharedString),
    /// The clear affordance was pressed: unbind the action.
    Clear,
    /// Recording ended without a binding — `esc` while recording, or the
    /// dialog losing focus. The dialog reports `esc` itself and never
    /// dismisses for it; a host that moves focus away from the dialog
    /// mid-recording should report this itself, since a stateless component
    /// cannot observe the blur. Either way the host should clear `recording`.
    Cancel,
}

/// The keyboard state the dialog keeps for itself.
struct SettingsState {
    focus: FocusHandle,
    focused: Option<usize>,
    was_present: bool,
}

/// One switch of the open page, in page order.
#[derive(Clone)]
struct SwitchData {
    id: SharedString,
    label: SharedString,
    detail: Option<SharedString>,
    on: bool,
}

/// One shortcut row of the open page, in page order.
#[derive(Clone)]
struct ShortcutData {
    id: SharedString,
    label: SharedString,
    detail: Option<SharedString>,
    keystroke: Option<SharedString>,
    recording: bool,
    editable: bool,
}

/// One arrow-focus stop of the open page: the switches and shortcut rows in
/// page order. Notes and headings are never stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageFocus {
    /// Index into the page's switches.
    Switch(usize),
    /// Index into the page's shortcut rows.
    Shortcut(usize),
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
    on_shortcut: Option<ShortcutHandler>,
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
        on_shortcut: None,
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

    /// A shortcut row edit — by pressing the row ([`ShortcutEdit::Record`]),
    /// capturing a keystroke while recording ([`ShortcutEdit::Set`]),
    /// pressing the clear affordance ([`ShortcutEdit::Clear`]), or aborting
    /// the recording with `esc` ([`ShortcutEdit::Cancel`]). The first
    /// argument is the row's id; the caller stores the edit and passes it
    /// back as `keystroke` / `recording` next frame.
    ///
    /// [`ShortcutEdit::Set`] spells the keystroke the way
    /// `gpui::KeyBinding::new` parses it: modifiers in the fixed order `fn`,
    /// `ctrl`, `alt`, `cmd`, `shift`, lowercase and hyphen-separated, e.g.
    /// `"cmd-shift-k"`. The platform modifier always spells `cmd`, on every
    /// host OS, so the string round-trips through the parser wherever the
    /// consuming app stores it.
    pub fn on_shortcut(mut self, f: impl Fn(&SharedString, ShortcutEdit, &mut Window, &mut App) + 'static) -> Self {
        self.on_shortcut = Some(Rc::new(f));
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

/// Spells a captured keystroke the way [`gpui::KeyBinding::new`] parses it:
/// modifiers in the fixed order `fn`, `ctrl`, `alt`, `cmd`, `shift`,
/// lowercase and hyphen-separated, e.g. `"cmd-shift-k"`. The order mirrors
/// gpui's own `Keystroke::unparse` except the platform modifier always
/// spells `cmd` (never `super` / `win`), so the string round-trips through
/// the parser on every host OS.
fn format_keystroke(keystroke: &Keystroke) -> SharedString {
    let modifiers = &keystroke.modifiers;
    let mut out = String::new();
    if modifiers.function {
        out.push_str("fn-");
    }
    if modifiers.control {
        out.push_str("ctrl-");
    }
    if modifiers.alt {
        out.push_str("alt-");
    }
    if modifiers.platform {
        out.push_str("cmd-");
    }
    if modifiers.shift {
        out.push_str("shift-");
    }
    out.push_str(&keystroke.key.to_lowercase());
    out.into()
}

/// Whether a key-down carries no key of its own — a bare modifier press.
/// An empty key is never a keystroke; a modifier name with no other
/// modifier held (`shift` alone, `cmd` alone) is the release-time echo of
/// one, not something to bind. A modifier name arriving *with* other
/// modifiers held still formats (e.g. `"ctrl-shift"` round-trips through
/// the parser), so only the truly bare press is ignored.
fn is_bare_modifier(keystroke: &Keystroke) -> bool {
    if keystroke.key.is_empty() {
        return true;
    }
    let modifiers = &keystroke.modifiers;
    if modifiers.control || modifiers.alt || modifiers.shift || modifiers.platform || modifiers.function {
        return false;
    }
    matches!(
        keystroke.key.as_str(),
        "shift" | "control" | "ctrl" | "alt" | "cmd" | "platform" | "super" | "win" | "fn" | "function"
    )
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
        // The selected section's switches and shortcut rows, in page order,
        // plus the arrow-focus stops across both. A page of only switches
        // builds exactly the old focus order.
        let mut switches: Vec<SwitchData> = Vec::new();
        let mut shortcuts: Vec<ShortcutData> = Vec::new();
        let mut order: Vec<PageFocus> = Vec::new();
        if let Some(section) = self.sections.get(selected) {
            for row in &section.rows {
                match row {
                    SettingsRow::Switch { id, label, detail, on } => {
                        order.push(PageFocus::Switch(switches.len()));
                        switches.push(SwitchData { id: id.clone(), label: label.clone(), detail: detail.clone(), on: *on });
                    }
                    SettingsRow::Shortcut { id, label, detail, keystroke, recording, editable } => {
                        order.push(PageFocus::Shortcut(shortcuts.len()));
                        shortcuts.push(ShortcutData {
                            id: id.clone(),
                            label: label.clone(),
                            detail: detail.clone(),
                            keystroke: keystroke.clone(),
                            recording: *recording,
                            editable: *editable,
                        });
                    }
                    SettingsRow::Note { .. } | SettingsRow::Heading { .. } => {}
                }
            }
        }
        let focus_count = order.len();
        let focused = state.read(cx).focused.filter(|at| *at < focus_count);
        // The recording row, if any: its id owns every keystroke until the
        // host clears `recording`.
        let recording: Option<SharedString> = shortcuts.iter().find(|s| s.recording).map(|s| s.id.clone());

        let card_focus = state.read(cx).focus.clone();

        // --- the card-level keyboard: esc, the arrows, return/space --------
        // While a row is recording, the bound keys below report through
        // `on_shortcut` instead of navigating: the `↓`/`↑` guards emit `Set`
        // and stop propagation so the focus never moves, `esc` emits `Cancel`
        // instead of dismissing, and `enter` / `space` re-enable propagation
        // with `cx.propagate()` — merely returning from the `Confirm`
        // handler would not reach the key-down capture, because a
        // bubble-phase action listener stops propagation by default before
        // the handler runs — so the capture below still sees the exact key
        // (`"enter"` vs `"space"`, which the `Confirm` action itself cannot
        // tell apart) and reports it. Keys with no binding reach that
        // capture directly; either way one keystroke reports exactly once.
        let confirm_order = order.clone();
        let confirm_switches = switches.clone();
        let confirm_shortcuts = shortcuts.clone();
        let confirm_switch = self.on_switch.clone();
        let confirm_shortcut = self.on_shortcut.clone();
        let recording_confirm = recording.clone();
        // Every dismissal ends the recording too: the scrim click, the `esc`
        // keycap, the close glyph and the `esc` key all funnel through here,
        // so a host that only watches `on_dismiss` can never strand a row in
        // `recording`. (`esc` while recording never reaches this — its guard
        // reports `Cancel` and stops first.)
        let dismiss = match (self.on_dismiss.clone(), recording.clone(), self.on_shortcut.clone()) {
            (Some(dismiss), Some(row_id), Some(on_shortcut)) => {
                let wrapped: ModalIntent = Rc::new(move |w, cx| {
                    on_shortcut(&row_id, ShortcutEdit::Cancel, w, cx);
                    dismiss(w, cx);
                });
                Some(wrapped)
            }
            (dismiss, _, _) => dismiss,
        };
        let dismiss_key = dismiss.clone();
        let capture_shortcut = self.on_shortcut.clone();
        let capture_recording = recording.clone();
        let mut card = modal_card(&p, self.width, &style)
            .p(px(CARD_PAD))
            .gap(px(CARD_GAP))
            .key_context(SETTINGS_CONTEXT)
            .track_focus(&card_focus)
            .on_action({
                let card_state = state.clone();
                let recording = recording.clone();
                let on_shortcut = self.on_shortcut.clone();
                move |_: &SelectNext, w, cx| {
                    if let Some(row_id) = &recording {
                        if let Some(handler) = &on_shortcut {
                            handler(row_id, ShortcutEdit::Set("down".into()), w, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    card_state.update(cx, |s, cx| {
                        s.focused = step_focus(s.focused, focus_count, 1);
                        cx.notify();
                    });
                }
            })
            .on_action({
                let card_state = state.clone();
                let recording = recording.clone();
                let on_shortcut = self.on_shortcut.clone();
                move |_: &SelectPrev, w, cx| {
                    if let Some(row_id) = &recording {
                        if let Some(handler) = &on_shortcut {
                            handler(row_id, ShortcutEdit::Set("up".into()), w, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    card_state.update(cx, |s, cx| {
                        s.focused = step_focus(s.focused, focus_count, -1);
                        cx.notify();
                    });
                }
            })
            .on_action(move |_: &Confirm, w, cx| {
                if recording_confirm.is_some() {
                    // Re-enable propagation so the key-down capture below
                    // sees the exact key (`"enter"` vs `"space"`) and
                    // reports it: a bubble-phase action listener stops
                    // propagation by default before this handler runs, so
                    // merely returning would swallow the keystroke
                    // entirely. The capture stops propagation there.
                    cx.propagate();
                    return;
                }
                if let Some(focused) = focused {
                    match confirm_order.get(focused).copied() {
                        Some(PageFocus::Switch(at)) => {
                            if let Some(data) = confirm_switches.get(at) {
                                if let Some(handler) = &confirm_switch {
                                    handler(&data.id, !data.on, w, cx);
                                }
                            }
                        }
                        Some(PageFocus::Shortcut(at)) => {
                            if let Some(data) = confirm_shortcuts.get(at) {
                                if data.editable && !data.recording {
                                    if let Some(handler) = &confirm_shortcut {
                                        handler(&data.id, ShortcutEdit::Record, w, cx);
                                    }
                                }
                            }
                        }
                        None => {}
                    }
                }
            })
            .on_action({
                let recording = recording.clone();
                let on_shortcut = capture_shortcut.clone();
                let dismiss_key = dismiss_key.clone();
                move |_: &Cancel, w, cx| {
                    if let Some(row_id) = &recording {
                        // Aborting a recording must never dismiss the dialog.
                        if let Some(handler) = &on_shortcut {
                            handler(row_id, ShortcutEdit::Cancel, w, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    if let Some(handler) = &dismiss_key {
                        handler(w, cx);
                    }
                }
            })
            .on_key_down(move |event, w, cx| {
                let Some(row_id) = &capture_recording else {
                    return;
                };
                // Bound keys never reach here — their action guards above
                // either report or stop first — except `enter` / `space`,
                // which the `Confirm` handler re-propagates so this capture
                // sees the exact key. A bare `escape` never reaches here
                // either: it always matches the `Cancel` binding first, and
                // that guard reports `Cancel` and stops, so no escape guard
                // is needed here. What remains is the unbound keys plus the
                // re-propagated `enter` / `space`.
                // A lone modifier is not a keystroke: swallow it and stay
                // recording.
                let edit = if is_bare_modifier(&event.keystroke) {
                    None
                } else {
                    Some(ShortcutEdit::Set(format_keystroke(&event.keystroke)))
                };
                if let Some(edit) = edit {
                    if let Some(handler) = &capture_shortcut {
                        handler(row_id, edit, w, cx);
                    }
                }
                cx.stop_propagation();
            });

        card = card.child(header(&id, &p, dismiss));
        card = card.child(body(
            &id,
            &p,
            row_h,
            &self.sections,
            selected,
            focused,
            &order,
            &switches,
            &shortcuts,
            self.on_select_section.clone(),
            self.on_switch.clone(),
            self.on_shortcut.clone(),
            window,
            cx,
        ));

        modal_scrim(id, style.opacity, dismiss_key.clone(), card)
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
    order: &[PageFocus],
    switches: &[SwitchData],
    shortcuts: &[ShortcutData],
    on_select_section: Option<SectionHandler>,
    on_switch: Option<SwitchHandler>,
    on_shortcut: Option<ShortcutHandler>,
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
    // The focus stop of each switch / shortcut row, for the highlight: the
    // focused index into `order`, or nothing when it points elsewhere.
    let stop_of = |stop: PageFocus| focused.filter(|at| order.get(*at).copied() == Some(stop));
    // The switch / shortcut index within the selected section.
    let mut switch_at = 0usize;
    let mut shortcut_at = 0usize;
    for (n, row) in rows.iter().enumerate() {
        match row {
            SettingsRow::Switch { .. } => {
                let data = switches[switch_at].clone();
                let on_row = stop_of(PageFocus::Switch(switch_at)).is_some();
                page = page.child(switch_row(
                    id,
                    p,
                    row_h,
                    switch_at,
                    data.id,
                    data.label,
                    data.detail,
                    data.on,
                    on_row,
                    on_switch.clone(),
                    window,
                    cx,
                ));
                switch_at += 1;
            }
            SettingsRow::Shortcut { .. } => {
                let data = shortcuts[shortcut_at].clone();
                let on_row = stop_of(PageFocus::Shortcut(shortcut_at)).is_some();
                page = page.child(shortcut_row(id, p, row_h, shortcut_at, &data, on_row, on_shortcut.clone(), window, cx));
                shortcut_at += 1;
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

/// The label column both row kinds share: the label in ink with its detail
/// under it, so switch and shortcut rows line up.
fn row_text(p: &Palette, label: SharedString, detail: Option<SharedString>) -> impl IntoElement {
    let mut text = v_flex().flex_1().min_w(px(0.0)).child(
        div().flex_none().w_full().truncate().ui(LABEL_TEXT).line_height(relative(scale::LH_UI)).text_color(p.ink).child(label),
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
    text
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

    let text = row_text(p, label.clone(), detail);

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

/// One shortcut row: the shared label column, and the binding at the right.
/// Resting and bound, the keycaps (the library's own keycap, never a second
/// style) plus a clear affordance; resting and unbound, a quiet "unassigned"
/// in ink-3; recording, a "Press a key…" pill in the accent role, unmissable
/// against the resting states; reserved, the keycaps dimmed with no clear
/// affordance and no press target at all. Pressing an editable resting row
/// reports `Record`; pressing anything else reports nothing.
#[allow(clippy::too_many_arguments)]
fn shortcut_row(
    id: &ElementId,
    p: &Palette,
    row_h: gpui::Pixels,
    shortcut_at: usize,
    data: &ShortcutData,
    highlighted: bool,
    on_shortcut: Option<ShortcutHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let key: ElementId = (id.clone(), SharedString::from(format!("shortcut-{shortcut_at}"))).into();
    let (istate, flags) = interaction_flags(key.clone(), window, cx);
    // One highlight, the palette's rule: the pointer owns it while it is over
    // a row, otherwise the keyboard's focused row keeps it.
    let lit = flags.hovered || highlighted;
    let ground = tint_fade((key.clone(), "bg"), lit, p.surface_3, Tween::FAST, window, cx);

    let text = row_text(p, data.label.clone(), data.detail.clone());

    let pressable = data.editable && !data.recording;
    let mut row = h_flex()
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
        .child(shortcut_control(id, p, shortcut_at, data, on_shortcut.clone()));
    if pressable {
        if let Some(handler) = on_shortcut {
            let row_id = data.id.clone();
            row = row.cursor_pointer().on_click(move |_, w, cx| handler(&row_id, ShortcutEdit::Record, w, cx));
        }
    }
    row
}

/// The right end of a shortcut row: keycaps, placeholder, recording pill or
/// dimmed reserved keycaps, per the row's state.
fn shortcut_control(
    id: &ElementId,
    p: &Palette,
    shortcut_at: usize,
    data: &ShortcutData,
    on_shortcut: Option<ShortcutHandler>,
) -> gpui::AnyElement {
    if data.recording {
        return pill("Press a key…").variant(PillVariant::Accent).into_any_element();
    }
    if !data.editable {
        // Reserved: the keycaps dimmed, no clear affordance, no press
        // target. An unbound reserved row still names its state.
        match &data.keystroke {
            Some(keystroke) => return div().opacity(RESERVED_OPACITY).child(kbd(keystroke.clone())).into_any_element(),
            None => {
                return div()
                    .opacity(RESERVED_OPACITY)
                    .mono(DETAIL_TEXT)
                    .line_height(relative(scale::LH_MONO))
                    .text_color(p.ink_3)
                    .child("unassigned")
                    .into_any_element();
            }
        }
    }
    match &data.keystroke {
        Some(keystroke) => {
            let clear_key: ElementId = (id.clone(), SharedString::from(format!("shortcut-clear-{shortcut_at}"))).into();
            let mut clear = icon_button(clear_key, IconName::X).ghost().muted().size(ButtonSize::Xs);
            if let Some(handler) = on_shortcut {
                let row_id = data.id.clone();
                clear = clear.on_click(move |_, w, cx| {
                    // The row itself reports `Record` on click; the clear
                    // button must not press the row beneath it.
                    cx.stop_propagation();
                    handler(&row_id, ShortcutEdit::Clear, w, cx);
                });
            }
            h_flex().flex_none().items_center().gap(px(ROW_GAP)).child(kbd(keystroke.clone())).child(clear).into_any_element()
        }
        None => div()
            .mono(DETAIL_TEXT)
            .line_height(relative(scale::LH_MONO))
            .text_color(p.ink_3)
            .child("unassigned")
            .into_any_element(),
    }
}

/// The hairline between two page rows.
fn divider(p: &Palette) -> impl IntoElement {
    div().flex_none().w_full().h(px(1.0)).bg(p.line)
}
