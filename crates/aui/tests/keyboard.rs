//! Real keystroke tests: a window, a focused host entity that mounts a
//! component the way an application does, and `simulate_keystrokes` driving it
//! through `aui::keys`' bindings.
//!
//! Components are stateless `RenderOnce`, so each test owns a tiny `Render`
//! host that holds the state, sets `key_context` / `track_focus` and handles
//! the actions — exactly the shape `crates/aui-gallery/src/assistant/view.rs`
//! uses. The intent closure handed to the component (`on_select`, `on_decide`,
//! `on_dismiss`) is the same `Rc` the host's action handlers call, so a
//! keystroke and a click record through one path.

use std::cell::RefCell;
use std::rc::Rc;

use aui::composer::{command_menu, CommandItem, CommandSection};
use aui::keys::{
    ApproveAlways, ApproveOnce, Cancel, ChooseNth, Confirm, Deny, FocusNext, FocusPrev, SelectNext, SelectPrev, APPROVAL_CONTEXT, MENU_CONTEXT,
    ROOT_CONTEXT,
};
use aui::overlay::{command_palette, settings_dialog, PaletteIcon, PaletteItem, PaletteSection, SettingsRow, SettingsSection};
use aui::protocol::{ApprovalChoice, ApprovalDecision, ApprovalState};
use aui::transcript::approval_card;
use gpui::{div, point, prelude::*, px, App, Context, FocusHandle, IntoElement, Modifiers, MouseButton, SharedString, TestAppContext, Window};

/// What the components recorded, in order.
type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
}

// ---------------------------------------------------------------- the palette

struct PaletteHost {
    focus: FocusHandle,
    selected: usize,
    open: bool,
    items: Vec<SharedString>,
    log: Log,
}

impl PaletteHost {
    fn select(&self, cx: &mut App) {
        let id = self.items[self.selected].clone();
        self.log.borrow_mut().push(format!("select:{id}"));
        let _ = cx;
    }
}

impl Render for PaletteHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().id("palette-closed").into_any_element();
        }
        let items: Vec<PaletteItem> =
            self.items.iter().map(|id| PaletteItem::new(id.clone(), PaletteIcon::Glyph(aui::icons::IconName::Search), id.clone())).collect();
        let select = cx.listener(|this, _: &SharedString, _, cx| this.select(cx));
        let dismiss = Rc::new(cx.listener(|this, _: &(), _, cx| {
            this.open = false;
            this.log.borrow_mut().push("dismiss".into());
            cx.notify();
        }));
        div()
            .key_context(MENU_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                this.selected = (this.selected + 1).min(this.items.len() - 1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| {
                this.selected = this.selected.saturating_sub(1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Confirm, _, cx| this.select(cx)))
            .on_action({
                let dismiss = dismiss.clone();
                move |_: &Cancel, w, cx| dismiss(&(), w, cx)
            })
            .child(
                command_palette("test-palette", "", vec![PaletteSection::new("Actions", items)], self.selected)
                    .at_rest()
                    .on_select(move |id, w, cx| select(id, w, cx))
                    .on_dismiss(move |w, cx| dismiss(&(), w, cx)),
            )
            .into_any_element()
    }
}

#[gpui::test]
fn palette_down_enter_selects_the_second_row(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |window, cx: &mut Context<PaletteHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            PaletteHost { focus, selected: 0, open: true, items: vec!["one".into(), "two".into(), "three".into()], log }
        }
    });

    cx.simulate_keystrokes("down enter");

    assert_eq!(&*log.borrow(), &["select:two".to_string()]);
}

#[gpui::test]
fn palette_escape_dismisses(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (host, cx) = cx.add_window_view({
        let log = log.clone();
        |window, cx: &mut Context<PaletteHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            PaletteHost { focus, selected: 0, open: true, items: vec!["one".into(), "two".into()], log }
        }
    });

    cx.simulate_keystrokes("escape");

    assert_eq!(&*log.borrow(), &["dismiss".to_string()]);
    assert!(!host.read_with(cx, |host, _| host.open));
}

#[gpui::test]
fn palette_up_stops_at_the_first_row(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |window, cx: &mut Context<PaletteHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            PaletteHost { focus, selected: 0, open: true, items: vec!["one".into(), "two".into()], log }
        }
    });

    cx.simulate_keystrokes("up enter");

    assert_eq!(&*log.borrow(), &["select:one".to_string()]);
}

// --------------------------------------------------------------- the approval

struct ApprovalHost {
    focus: FocusHandle,
    log: Log,
}

impl ApprovalHost {
    fn decide(&self, decision: ApprovalDecision, cx: &mut App) {
        self.log.borrow_mut().push(format!("{decision:?}"));
        let _ = cx;
    }
}

impl Render for ApprovalHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let decide = cx.listener(|this, decision: &ApprovalDecision, _, cx| this.decide(*decision, cx));
        div()
            .key_context(APPROVAL_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ApproveOnce, _, cx| this.decide(ApprovalDecision::Once, cx)))
            .on_action(cx.listener(|this, _: &ApproveAlways, _, cx| this.decide(ApprovalDecision::Always, cx)))
            .on_action(cx.listener(|this, _: &Deny, _, cx| this.decide(ApprovalDecision::Deny, cx)))
            .child(
                approval_card("test-approval", "Bash", "pnpm test", ApprovalState::Pending)
                    .reason("Run the test suite")
                    .at_rest()
                    .on_decide(move |decision, w, cx| decide(&decision, w, cx)),
            )
    }
}

fn approval_keys(cx: &mut TestAppContext, keys: &str) -> Vec<String> {
    init(cx);
    let log = log();
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |window, cx: &mut Context<ApprovalHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            ApprovalHost { focus, log }
        }
    });
    cx.simulate_keystrokes(keys);
    let out = log.borrow().clone();
    out
}

#[gpui::test]
fn approval_y_allows_once(cx: &mut TestAppContext) {
    assert_eq!(approval_keys(cx, "y"), vec!["Once".to_string()]);
}

#[gpui::test]
fn approval_a_allows_always(cx: &mut TestAppContext) {
    assert_eq!(approval_keys(cx, "a"), vec!["Always".to_string()]);
}

#[gpui::test]
fn approval_n_denies(cx: &mut TestAppContext) {
    assert_eq!(approval_keys(cx, "n"), vec!["Deny".to_string()]);
}

// ------------------------------------------- the approval's server choices

/// The same card, but with the server's own choice list. There is no fixed
/// `y`/`a`/`n` to bind against a list the provider mints at request time, so
/// the digits carry a zero-based index and the host looks the choice up.
struct ChoicesHost {
    focus: FocusHandle,
    choices: Vec<ApprovalChoice>,
    log: Log,
}

impl ChoicesHost {
    fn choose(&self, index: usize) {
        match self.choices.get(index) {
            Some(choice) => self.log.borrow_mut().push(format!("choose:{}", choice.id)),
            // A digit with no choice behind it is not an error: the person
            // pressed 4 on a three-choice card.
            None => self.log.borrow_mut().push(format!("none:{index}")),
        }
    }
}

impl Render for ChoicesHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let log = self.log.clone();
        div()
            .key_context(APPROVAL_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, nth: &ChooseNth, _, _| this.choose(nth.index)))
            .child(
                approval_card("test-choices", "Shell", "echo hi && ls", ApprovalState::Pending)
                    .choices(self.choices.clone())
                    .at_rest()
                    .on_choose(move |id, feedback, _, _| log.borrow_mut().push(format!("click:{id}:{feedback:?}"))),
            )
    }
}

fn choice(id: &str, decision: ApprovalDecision) -> ApprovalChoice {
    ApprovalChoice {
        id: id.into(),
        label: id.into(),
        decision,
        scope: aui::protocol::ApprovalScope::ThisWorktree,
        rule_preview: None,
        accepts_feedback: false,
    }
}

fn choice_keys(cx: &mut TestAppContext, keys: &str) -> Vec<String> {
    init(cx);
    let log = log();
    let (_host, cx) = cx.add_window_view({
        let log = log.clone();
        |window, cx: &mut Context<ChoicesHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            ChoicesHost {
                focus,
                choices: vec![
                    choice("allow_once", ApprovalDecision::Once),
                    choice("allow_local_prefix", ApprovalDecision::PolicyAmendment),
                    choice("deny", ApprovalDecision::Deny),
                ],
                log,
            }
        }
    });
    cx.simulate_keystrokes(keys);
    let out = log.borrow().clone();
    out
}

#[gpui::test]
fn digit_one_picks_the_first_server_choice(cx: &mut TestAppContext) {
    assert_eq!(choice_keys(cx, "1"), vec!["choose:allow_once".to_string()]);
}

#[gpui::test]
fn digit_three_picks_the_third_server_choice(cx: &mut TestAppContext) {
    assert_eq!(choice_keys(cx, "3"), vec!["choose:deny".to_string()]);
}

#[gpui::test]
fn a_digit_past_the_last_choice_chooses_nothing(cx: &mut TestAppContext) {
    assert_eq!(choice_keys(cx, "9"), vec!["none:8".to_string()]);
}

// ------------------------------------------------------------- the `/` menu

struct MenuHost {
    focus: FocusHandle,
    selected: usize,
    items: Vec<SharedString>,
    log: Log,
}

impl MenuHost {
    fn select(&self, cx: &mut App) {
        let id = self.items[self.selected].clone();
        self.log.borrow_mut().push(format!("select:{id}"));
        let _ = cx;
    }
}

impl Render for MenuHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items: Vec<CommandItem> = self.items.iter().map(|id| CommandItem::new(id.clone(), format!("/{id}"), "a test command")).collect();
        let select = cx.listener(|this, _: &SharedString, _, cx| this.select(cx));
        div()
            .key_context(MENU_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                this.selected = (this.selected + 1).min(this.items.len() - 1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| {
                this.selected = this.selected.saturating_sub(1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Confirm, _, cx| this.select(cx)))
            .child(
                command_menu("test-menu", "/", vec![CommandSection::new("Commands", items)], self.selected)
                    .at_rest()
                    .on_select(move |id, w, cx| select(id, w, cx)),
            )
    }
}

#[gpui::test]
fn menu_down_down_enter_selects_the_third_row(cx: &mut TestAppContext) {
    init(cx);
    let log = log();
    let (host, cx) = cx.add_window_view({
        let log = log.clone();
        |window, cx: &mut Context<MenuHost>| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            MenuHost { focus, selected: 0, items: vec!["review".into(), "commit".into(), "clear".into()], log }
        }
    });

    cx.simulate_keystrokes("down down enter");

    assert_eq!(&*log.borrow(), &["select:clear".to_string()]);
    assert_eq!(host.read_with(cx, |host, _| host.selected), 2);
}

// ------------------------------------------------------- the settings dialog

/// The settings dialog owns its card focus, so — unlike the palette and menu
/// hosts above — this host holds no focus handle and wires no actions: it
/// only owns the sections, the open section and the log, the way an
/// application owns its persisted settings. Tab still needs the application
/// root around the dialog, so the host carries that: `tab` / `shift-tab` walk
/// the rail and the switches through `focus_next` / `focus_prev`.
struct SettingsHost {
    root: FocusHandle,
    selected: usize,
    open: bool,
    log: Log,
}

fn settings_sections() -> Vec<SettingsSection> {
    vec![
        SettingsSection {
            id: "sidebar".into(),
            label: "Sidebar".into(),
            rows: vec![
                SettingsRow::Switch { id: "chevron".into(), label: "Collapse chevron".into(), detail: None, on: true },
                SettingsRow::Switch {
                    id: "bar".into(),
                    label: "Current-project bar".into(),
                    detail: Some("Marks the open session.".into()),
                    on: false,
                },
            ],
        },
        SettingsSection {
            id: "appearance".into(),
            label: "Appearance".into(),
            rows: vec![SettingsRow::Switch { id: "compact".into(), label: "Compact rows".into(), detail: None, on: false }],
        },
    ]
}

impl Render for SettingsHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().id("settings-closed").into_any_element();
        }
        let host = cx.entity();
        let flip_log = self.log.clone();
        let section_log = self.log.clone();
        let section_host = host.clone();
        let dismiss_log = self.log.clone();
        let dismiss_host = host.clone();
        div()
            .key_context(ROOT_CONTEXT)
            .track_focus(&self.root)
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrev, window, cx| window.focus_prev(cx)))
            .child(
                settings_dialog("test-settings", settings_sections(), self.selected)
                    .at_rest()
                    .on_select_section(move |index, _, cx| {
                        section_log.borrow_mut().push(format!("section:{index}"));
                        section_host.update(cx, |this, cx| {
                            this.selected = index;
                            cx.notify();
                        });
                    })
                    .on_switch(move |id, on, _, _| flip_log.borrow_mut().push(format!("flip:{id}:{on}")))
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

fn settings_keys(cx: &mut TestAppContext, keys: &str, selected: usize) -> (Vec<String>, bool, usize) {
    init(cx);
    let log = log();
    let (host, cx) = cx.add_window_view({
        let log = log.clone();
        |_, cx: &mut Context<SettingsHost>| SettingsHost { root: cx.focus_handle(), selected, open: true, log }
    });
    cx.simulate_keystrokes(keys);
    let out = log.borrow().clone();
    let (open, selected) = host.read_with(cx, |host, _| (host.open, host.selected));
    (out, open, selected)
}

#[gpui::test]
fn settings_escape_dismisses(cx: &mut TestAppContext) {
    let (log, open, _) = settings_keys(cx, "escape", 0);
    assert_eq!(log, vec!["dismiss".to_string()]);
    assert!(!open);
}

#[gpui::test]
fn settings_down_down_enter_flips_the_second_switch(cx: &mut TestAppContext) {
    let (log, _, _) = settings_keys(cx, "down down enter", 0);
    assert_eq!(log, vec!["flip:bar:true".to_string()]);
}

#[gpui::test]
fn settings_space_flips_the_focused_switch(cx: &mut TestAppContext) {
    let (log, _, _) = settings_keys(cx, "down space", 0);
    assert_eq!(log, vec!["flip:chevron:false".to_string()]);
}

#[gpui::test]
fn settings_enter_without_a_focused_row_flips_nothing(cx: &mut TestAppContext) {
    let (log, _, _) = settings_keys(cx, "enter", 0);
    assert!(log.is_empty());
}

#[gpui::test]
fn settings_up_from_no_focus_lands_on_the_last_switch(cx: &mut TestAppContext) {
    let (log, _, _) = settings_keys(cx, "up enter", 0);
    assert_eq!(log, vec!["flip:bar:true".to_string()]);
}

#[gpui::test]
fn settings_tab_reaches_the_section_rail(cx: &mut TestAppContext) {
    // Appearance is open; tabs walk the close button then the rail, and
    // return on the first rail row selects its section instead of flipping
    // a switch.
    let (log, _, selected) = settings_keys(cx, "tab tab enter", 1);
    assert_eq!(log, vec!["section:0".to_string()]);
    assert_eq!(selected, 0);
}

// ------------------------------------- the `:focus-visible` approximation

/// A bare window root wearing [`aui::keys::track_pointer`], the way an
/// application that does not use [`aui::shell::app_shell`] wires it up.
struct RootHost {
    focus: FocusHandle,
}

impl Render for RootHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        aui::keys::track_pointer(div().id("root").key_context(aui::keys::ROOT_CONTEXT).track_focus(&self.focus).size_full())
    }
}

#[gpui::test]
fn a_mouse_press_outside_a_control_disarms_the_focus_ring(cx: &mut TestAppContext) {
    init(cx);
    let (_host, cx) = cx.add_window_view(|window, cx: &mut Context<RootHost>| {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        RootHost { focus }
    });

    cx.update(|_, cx| aui::keys::set_keyboard_nav(true, cx));
    assert!(cx.update(|_, cx| aui::keys::keyboard_nav(cx)));

    // Empty ground, not a button: before the capture-phase hook this stayed armed.
    let point = point(px(40.0), px(40.0));
    cx.simulate_mouse_move(point, None, Modifiers::none());
    cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
    assert!(!cx.update(|_, cx| aui::keys::keyboard_nav(cx)), "a mouse press on the window ground must disarm the flag");

    cx.simulate_keystrokes("tab");
    assert!(cx.update(|_, cx| aui::keys::keyboard_nav(cx)), "the next key must re-arm it");
}
