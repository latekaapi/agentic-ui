//! Settings shortcut rows: pressing an editable row reports `Record`, the next
//! keystroke while recording reports `Set`, `esc` reports `Cancel` without
//! dismissing the dialog, bare modifiers are swallowed, and reserved rows
//! stay silent. Each phase runs in its own `simulate_keystrokes` call so the
//! host-owned `recording` flag renders before the next keystroke lands.

use std::cell::RefCell;
use std::rc::Rc;

use aui::keys::ROOT_CONTEXT;
use aui::overlay::{settings_dialog, SettingsRow, SettingsSection, ShortcutEdit};
use gpui::{div, prelude::*, Context, FocusHandle, IntoElement, SharedString, TestAppContext, Window};

/// What the dialog reported, in order.
type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
}

/// One shortcut row's host-owned state.
#[derive(Clone)]
struct ShortcutState {
    id: SharedString,
    keystroke: Option<SharedString>,
    recording: bool,
    editable: bool,
}

/// The host owns the sections the way an application owns its settings: the
/// switch value, each shortcut's binding and `recording` flag, and the log.
struct ShortcutHost {
    root: FocusHandle,
    open: bool,
    chevron_on: bool,
    shortcuts: Vec<ShortcutState>,
    log: Log,
}

fn initial_shortcuts() -> Vec<ShortcutState> {
    vec![
        ShortcutState { id: "palette".into(), keystroke: Some("cmd-shift-k".into()), recording: false, editable: true },
        ShortcutState { id: "reserved".into(), keystroke: Some("cmd-q".into()), recording: false, editable: false },
        ShortcutState { id: "hover".into(), keystroke: None, recording: false, editable: true },
    ]
}

impl Render for ShortcutHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().id("shortcut-closed").into_any_element();
        }
        let mut rows = vec![SettingsRow::Switch { id: "chevron".into(), label: "Collapse chevron".into(), detail: None, on: self.chevron_on }];
        rows.extend(self.shortcuts.iter().map(|s| SettingsRow::Shortcut {
            id: s.id.clone(),
            label: SharedString::from(format!("Action {}", s.id)),
            detail: None,
            keystroke: s.keystroke.clone(),
            recording: s.recording,
            editable: s.editable,
        }));
        let sections = vec![SettingsSection { id: "shortcuts".into(), label: "Shortcuts".into(), rows }];

        let host = cx.entity();
        let flip_log = self.log.clone();
        let flip_host = host.clone();
        let edit_log = self.log.clone();
        let edit_host = host.clone();
        let dismiss_log = self.log.clone();
        let dismiss_host = host.clone();
        div()
            .key_context(ROOT_CONTEXT)
            .track_focus(&self.root)
            .child(
                settings_dialog("test-shortcuts", sections, 0)
                    .at_rest()
                    .on_switch(move |id, on, _, cx| {
                        flip_log.borrow_mut().push(format!("flip:{id}:{on}"));
                        flip_host.update(cx, |this, cx| {
                            this.chevron_on = on;
                            cx.notify();
                        });
                    })
                    .on_shortcut(move |id, edit, _, cx| {
                        edit_log.borrow_mut().push(format!("shortcut:{id}:{edit:?}"));
                        edit_host.update(cx, |this, cx| {
                            match edit {
                                ShortcutEdit::Record => {
                                    for s in &mut this.shortcuts {
                                        s.recording = s.id == *id;
                                    }
                                }
                                ShortcutEdit::Set(keys) => {
                                    if let Some(s) = this.shortcuts.iter_mut().find(|s| s.id == *id) {
                                        s.keystroke = Some(keys.clone());
                                        s.recording = false;
                                    }
                                }
                                ShortcutEdit::Clear => {
                                    if let Some(s) = this.shortcuts.iter_mut().find(|s| s.id == *id) {
                                        s.keystroke = None;
                                    }
                                }
                                ShortcutEdit::Cancel => {
                                    for s in &mut this.shortcuts {
                                        s.recording = false;
                                    }
                                }
                            }
                            cx.notify();
                        });
                    })
                    .on_dismiss(move |_, cx| {
                        dismiss_log.borrow_mut().push("dismiss".into());
                        dismiss_host.update(cx, |this, cx| {
                            this.open = false;
                            cx.notify();
                        });
                    }),
            )
            .into_any_element()
    }
}

/// Runs one `simulate_keystrokes` call per phase — each call parks the
/// executor, so the host-owned `recording` flag renders before the next
/// keystroke lands — and returns the log plus whether the dialog is open.
fn shortcut_keys(cx: &mut TestAppContext, phases: &[&str]) -> (Vec<String>, bool) {
    init(cx);
    let log = log();
    let (host, cx) = cx.add_window_view({
        let log = log.clone();
        |_, cx: &mut Context<ShortcutHost>| ShortcutHost {
            root: cx.focus_handle(),
            open: true,
            chevron_on: true,
            shortcuts: initial_shortcuts(),
            log,
        }
    });
    for keys in phases {
        cx.simulate_keystrokes(keys);
    }
    let out = log.borrow().clone();
    let open = host.read_with(cx, |host, _| host.open);
    (out, open)
}

/// Like [`shortcut_keys`], but also returns the palette row's stored binding
/// and whether it is still recording.
fn recording_state(cx: &mut TestAppContext, phases: &[&str]) -> (Vec<String>, bool, Option<SharedString>, bool) {
    init(cx);
    let log = log();
    let (host, cx) = cx.add_window_view({
        let log = log.clone();
        |_, cx: &mut Context<ShortcutHost>| ShortcutHost {
            root: cx.focus_handle(),
            open: true,
            chevron_on: true,
            shortcuts: initial_shortcuts(),
            log,
        }
    });
    for keys in phases {
        cx.simulate_keystrokes(keys);
    }
    let out = log.borrow().clone();
    let (open, keystroke, recording) =
        host.read_with(cx, |host, _| (host.open, host.shortcuts[0].keystroke.clone(), host.shortcuts[0].recording));
    (out, open, keystroke, recording)
}

/// Focus stops are chevron(0), palette(1), reserved(2), hover(3): `down up`
/// lands back on the switch, and `enter` flips it without touching shortcuts.
#[gpui::test]
fn shortcut_arrows_and_enter_flip_switches_when_idle(cx: &mut TestAppContext) {
    let (log, open) = shortcut_keys(cx, &["down up enter"]);

    assert_eq!(log, vec!["flip:chevron:false".to_string()]);
    assert!(open);
}

/// `enter` on a focused editable shortcut row presses it: `Record`.
#[gpui::test]
fn shortcut_enter_on_focused_editable_row_reports_record(cx: &mut TestAppContext) {
    let (log, open, _, recording) = recording_state(cx, &["down down enter"]);

    assert_eq!(log, vec!["shortcut:palette:Record".to_string()]);
    assert!(open);
    assert!(recording);
}

/// While recording, `↓` binds instead of moving focus: it reports
/// `Set("down")`, and the focus stays where it was — the next `down enter`
/// lands on the reserved row and reports nothing.
#[gpui::test]
fn shortcut_recording_swallows_down_and_reports_set(cx: &mut TestAppContext) {
    let (log, open, keystroke, recording) = recording_state(cx, &["down down enter", "down", "down enter"]);

    assert_eq!(log, vec!["shortcut:palette:Record".to_string(), "shortcut:palette:Set(\"down\")".to_string()]);
    assert!(open);
    assert_eq!(keystroke, Some("down".into()));
    assert!(!recording);
}

/// While recording, `space` binds instead of flipping the focused switch: it
/// reports `Set("space")`, and nothing else is reported.
#[gpui::test]
fn shortcut_recording_space_reports_set(cx: &mut TestAppContext) {
    let (log, open, keystroke, recording) = recording_state(cx, &["down down enter", "space"]);

    assert_eq!(log, vec!["shortcut:palette:Record".to_string(), "shortcut:palette:Set(\"space\")".to_string()]);
    assert!(open);
    assert_eq!(keystroke, Some("space".into()));
    assert!(!recording);
}

/// While recording, `enter` binds instead of confirming anything: it reports
/// `Set("enter")`, and the dialog does not flip or dismiss.
#[gpui::test]
fn shortcut_recording_enter_reports_set(cx: &mut TestAppContext) {
    let (log, open, keystroke, recording) = recording_state(cx, &["down down enter", "enter"]);

    assert_eq!(log, vec!["shortcut:palette:Record".to_string(), "shortcut:palette:Set(\"enter\")".to_string()]);
    assert!(open);
    assert_eq!(keystroke, Some("enter".into()));
    assert!(!recording);
}

/// `space` on a focused switch row flips it when nothing is recording — the
/// not-recording path the recording fix must not break.
#[gpui::test]
fn shortcut_space_flips_switch_when_idle(cx: &mut TestAppContext) {
    let (log, open) = shortcut_keys(cx, &["down space"]);

    assert_eq!(log, vec!["flip:chevron:false".to_string()]);
    assert!(open);
}

/// `esc` while recording cancels the recording and must not dismiss: a later
/// `esc` with nothing recording still dismisses.
#[gpui::test]
fn shortcut_escape_while_recording_cancels_without_dismissing(cx: &mut TestAppContext) {
    let (log, open) = shortcut_keys(cx, &["down down enter", "escape", "escape"]);

    assert_eq!(
        log,
        vec!["shortcut:palette:Record".to_string(), "shortcut:palette:Cancel".to_string(), "dismiss".to_string()]
    );
    assert!(!open);
}

/// `esc` while recording leaves the dialog open: after `Record` + `escape`
/// with no further keys, there is a `Cancel` and no `dismiss`.
#[gpui::test]
fn shortcut_escape_while_recording_keeps_dialog_open(cx: &mut TestAppContext) {
    let (log, open) = shortcut_keys(cx, &["down down enter", "escape"]);

    assert_eq!(log, vec!["shortcut:palette:Record".to_string(), "shortcut:palette:Cancel".to_string()]);
    assert!(open);
}

/// A chord formats modifiers in the fixed `fn`-`ctrl`-`alt`-`cmd`-`shift`
/// order, lowercase and hyphen-separated.
#[gpui::test]
fn shortcut_chord_formats_modifiers_in_fixed_order(cx: &mut TestAppContext) {
    let (log, _) = shortcut_keys(cx, &["down down enter", "ctrl-shift-k"]);

    assert_eq!(log, vec!["shortcut:palette:Record".to_string(), "shortcut:palette:Set(\"ctrl-shift-k\")".to_string()]);
}

/// The platform modifier always spells `cmd`, so the string round-trips
/// through `KeyBinding::new`'s parser on every host OS.
#[gpui::test]
fn shortcut_cmd_chord_spells_cmd(cx: &mut TestAppContext) {
    let (log, _) = shortcut_keys(cx, &["down down enter", "cmd-shift-k"]);

    assert_eq!(log, vec!["shortcut:palette:Record".to_string(), "shortcut:palette:Set(\"cmd-shift-k\")".to_string()]);
}

/// A bare modifier is not a keystroke: it reports nothing and the row stays
/// recording, so the next real key still binds.
#[gpui::test]
fn shortcut_bare_modifier_is_ignored_and_stays_recording(cx: &mut TestAppContext) {
    let (log, _) = shortcut_keys(cx, &["down down enter", "shift", "k"]);

    assert_eq!(log, vec!["shortcut:palette:Record".to_string(), "shortcut:palette:Set(\"k\")".to_string()]);
}

/// Pressing a reserved row reports nothing: `enter` on the focused reserved
/// row neither flips nor edits. (A mouse press cannot report either — a
/// reserved row registers no click handler at all.)
#[gpui::test]
fn shortcut_reserved_row_reports_nothing_when_pressed(cx: &mut TestAppContext) {
    let (log, open) = shortcut_keys(cx, &["down down down enter"]);

    assert!(log.is_empty());
    assert!(open);
}

/// The new API is reachable from outside the crate: `ShortcutEdit` names
/// from `aui::overlay`, and `on_shortcut` builds on the dialog.
#[gpui::test]
fn shortcut_api_is_reachable_from_outside_the_crate(_cx: &mut TestAppContext) {
    let _ = ShortcutEdit::Record;
    let _ = ShortcutEdit::Set("cmd-shift-k".into());
    let _ = ShortcutEdit::Clear;
    let _ = ShortcutEdit::Cancel;
    let _ = settings_dialog("reach", Vec::<SettingsSection>::new(), 0).on_shortcut(|_, _, _, _| {});
}
