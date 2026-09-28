//! `.menu`: the `+` popover — overlay ground, rising 6 px with a
//! fade on the quick presence tween every composer menu shares.
//!
//! It is anchored to the `+` button (absolute, from the button's holder) but
//! painted on [`crate::overlay::popover_layer`], so the composer's focus ring,
//! chips and toolbar cannot draw over it.
//!
//! The menu is stateless: the host owns `open` and passes a close callback
//! ([`PlusMenu::on_close`]) that the menu invokes on a mouse-down outside it
//! and on Escape, like the chip pickers' scrim and the settings dialog's
//! `Cancel` action. The menu sizes to its content between a floor that fits
//! the longest row ([`plus_menu_min_width`]) and [`MENU_W_MAX`]; rows never
//! ellipsize inside that range.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, FocusHandle, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::kbd;
use crate::keys::{Cancel, MENU_CONTEXT};
use crate::overlay::popover_layer;
use crate::util::{interaction_flags, TrackInteraction};

/// `.menu{bottom:38px;left:0;padding:6px}` — measured from the
/// bar, which has 10 px of bottom padding; the menu anchors to the `+`
/// button, so 28 px above it.
const MENU_BOTTOM: f32 = 28.0;
/// … and `left:0` from the bar, whose left padding is 10 px.
const MENU_LEFT: f32 = -10.0;
/// The ceiling: past it a single-line row ellipsizes instead of pushing the
/// menu over the transcript. Rows fit between the floor and here.
const MENU_W_MAX: f32 = 320.0;
const MENU_PAD: f32 = 6.0;
/// `.menu .it{gap:8px;height:30px;padding:0 8px;font-size:12.5px}`.
const ITEM_GAP: f32 = 8.0;
const ITEM_PAD: f32 = 8.0;
const ITEM_TEXT: f32 = 12.5;
/// The row glyph: [`aui_icons::ICON_SIZE`].
const ROW_ICON: f32 = 14.0;
/// A conservative mean advance of one UI-face glyph at [`ITEM_TEXT`], and of
/// one mono glyph in the keycap, plus the keycap's own horizontal padding
/// (`.kbd{padding:0 5px}`): the floor must clear the longest row, so the
/// estimate errs wide rather than exact.
const LABEL_PX_PER_CHAR: f32 = 8.0;
const KEY_PX_PER_CHAR: f32 = 7.5;
const KEY_PAD_X: f32 = 5.0;
/// The enter rises 6 px from the button's corner at scale .98 — the one
/// presence tween every composer menu shares.
const POP_RISE: f32 = 6.0;
const POP_FROM_SCALE: f32 = 0.98;

/// One row of the menu.
#[derive(Debug, Clone, PartialEq)]
pub struct PlusMenuItem {
    /// Stable id, reported on activation.
    pub id: SharedString,
    /// Glyph.
    pub icon: IconName,
    /// Label.
    pub label: SharedString,
    /// Optional keycap at the right.
    pub key: Option<SharedString>,
}

impl PlusMenuItem {
    /// An item.
    pub fn new(id: impl Into<SharedString>, icon: IconName, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), icon, label: label.into(), key: None }
    }

    /// Adds the keycap.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }
}

/// The floor for the menu's content width: the chrome around and inside the
/// rows (menu padding, row padding, the two gaps, the glyph) plus the widest
/// row's label and keycap at the estimates above. Empty items still clear the
/// chrome, so the menu never collapses to nothing.
pub fn plus_menu_min_width(items: &[PlusMenuItem]) -> f32 {
    let chrome = MENU_PAD * 2.0 + ITEM_PAD * 2.0 + ITEM_GAP * 2.0 + ROW_ICON;
    let widest = items
        .iter()
        .map(|item| {
            let label = item.label.chars().count() as f32 * LABEL_PX_PER_CHAR;
            let key = item
                .key
                .as_ref()
                .map(|key| key.chars().count() as f32 * KEY_PX_PER_CHAR + KEY_PAD_X * 2.0)
                .unwrap_or(0.0);
            label + key
        })
        .fold(0.0f32, f32::max);
    chrome + widest
}

type ActivateHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The menu. Build with [`plus_menu`].
#[derive(IntoElement)]
pub struct PlusMenu {
    id: ElementId,
    items: Vec<PlusMenuItem>,
    open: bool,
    at_rest: bool,
    on_activate: Option<ActivateHandler>,
    on_close: Option<CloseHandler>,
}

/// A menu over `items`; render it as a child of the `+` button's holder.
pub fn plus_menu(id: impl Into<ElementId>, items: Vec<PlusMenuItem>, open: bool) -> PlusMenu {
    PlusMenu { id: id.into(), items, open, at_rest: false, on_activate: None, on_close: None }
}

impl PlusMenu {
    /// Skips the enter (static captures).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// Activation handler.
    pub fn on_activate(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }

    /// Dismissal handler: a mouse-down outside the menu, or Escape while the
    /// menu holds the keyboard, reports here. Unset keeps the old behaviour
    /// (no outside or Escape path), so existing callers compile unchanged.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for PlusMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // At rest the menu is either fully out or fully gone; `open` still
        // decides which, or a closed menu would draw itself in static captures.
        let style = if self.at_rest {
            PresenceStyle { opacity: if self.open { 1.0 } else { 0.0 }, offset_y: px(0.0), scale: 1.0 }
        } else {
            let sample = presence((id.clone(), "enter"), self.open, EnterExit::QUICK, window, cx);
            PresenceStyle::fade_rise_scale(sample, POP_RISE, POP_FROM_SCALE)
        };
        if !self.open && style.opacity <= 0.001 {
            return div().invisible().into_any_element();
        }
        let mut menu = v_flex()
            .id(id.clone())
            .absolute()
            .bottom(px(MENU_BOTTOM) - style.offset_y)
            .left(px(MENU_LEFT))
            // The menu grows with its rows between the floor that fits the
            // longest row and the ceiling: rows stretch to the widest one
            // (never `w_full`, which would collapse the parent to the floor
            // the way the chip pickers document), and the label only
            // ellipsizes past the ceiling.
            .min_w(px(plus_menu_min_width(&self.items) * style.scale))
            .max_w(px(MENU_W_MAX * style.scale))
            .p(px(MENU_PAD))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(3))
            .opacity(style.opacity)
            .overflow_hidden()
            .occlude()
            .role(gpui::Role::Menu)
            .aria_label(SharedString::from("Composer actions"));
        // The dismiss path, after the chip pickers (outside press) and the
        // settings dialog (`Cancel` under its key context). The menu takes
        // focus when pressed — never on open, so typing is undisturbed — and
        // from then on Escape reports here too.
        if let Some(close) = self.on_close.clone() {
            let focus: FocusHandle =
                window.use_keyed_state((id.clone(), "menu-focus"), cx, |_, cx| cx.focus_handle()).read(cx).clone();
            let close_out = close.clone();
            menu = menu.track_focus(&focus).key_context(MENU_CONTEXT).on_action(move |_: &Cancel, w, cx| close(w, cx));
            menu = menu.on_mouse_down_out(move |_, w: &mut Window, cx: &mut App| close_out(w, cx));
        }
        for item in self.items {
            let item_id: ElementId = (id.clone(), SharedString::from(format!("item-{}", item.id))).into();
            let (state, flags) = interaction_flags(item_id.clone(), window, cx);
            // The hover tint fades in and out over the hover duration, like
            // every other list row in the library.
            let ground = aui_motion::tint_fade((item_id.clone(), "bg"), flags.hovered, p.surface_2, aui_motion::Tween::FAST, window, cx);
            let text = aui_motion::tween((item_id.clone(), "text"), if flags.hovered { p.ink } else { p.ink_2 }, aui_motion::Tween::FAST, window, cx);
            let mut row = h_flex()
                .id(item_id)
                .h(cx.aui().metrics.row)
                .gap(px(ITEM_GAP))
                .px(px(ITEM_PAD))
                .rounded(px(scale::R_SM))
                .ui(ITEM_TEXT)
                .text_color(text)
                .bg(ground)
                .cursor_pointer()
                .role(gpui::Role::MenuItem)
                .aria_label(item.label.clone())
                .track_interaction(&state)
                .child(icon(item.icon))
                .child(div().flex_1().truncate().child(item.label.clone()));
            if let Some(key) = &item.key {
                row = row.child(kbd(key.clone()));
            }
            if let Some(h) = self.on_activate.clone() {
                let key = item.id.clone();
                row = row.on_click(move |_, w, cx| h(&key, w, cx));
            }
            menu = menu.child(row);
        }
        popover_layer(menu).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// The Baaz rows from the report: attach, mention, slash commands.
    fn baaz_items() -> Vec<PlusMenuItem> {
        vec![
            PlusMenuItem::new("attach", IconName::Paperclip, "Attach file or photo").key("U"),
            PlusMenuItem::new("mention", IconName::At, "Mention file").key("@"),
            PlusMenuItem::new("slash", IconName::Slash, "Slash commands").key("/"),
        ]
    }

    #[test]
    fn min_width_fits_the_longest_baaz_row() {
        // The old menu was a fixed 200 px, which cut "Attach file or photo"
        // to "Attach file or ph…": the floor for the Baaz rows must clear it.
        assert!(plus_menu_min_width(&baaz_items()) > 200.0, "the floor must grow past the old truncating width");
    }

    #[test]
    fn min_width_covers_label_and_keycap() {
        let without_key: Vec<PlusMenuItem> =
            baaz_items().into_iter().map(|item| PlusMenuItem::new(item.id, item.icon, item.label)).collect();
        // A keycap takes room: dropping every keycap must shrink the floor.
        assert!(plus_menu_min_width(&baaz_items()) > plus_menu_min_width(&without_key));
        // … and an empty menu still clears the chrome, never collapses.
        let chrome_only = plus_menu_min_width(&[]);
        assert!(chrome_only > 0.0);
        assert!(plus_menu_min_width(&without_key) > chrome_only);
    }

    struct MenuHost {
        items: Vec<PlusMenuItem>,
        closed: Rc<Cell<bool>>,
        activated: Rc<RefCell<Vec<SharedString>>>,
    }

    impl gpui::Render for MenuHost {
        fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let closed = self.closed.clone();
            let activated = self.activated.clone();
            div()
                .w_full()
                .h_full()
                .relative()
                .child(
                    plus_menu("test-plus", self.items.clone(), true)
                        .at_rest()
                        .on_activate(move |id, _, _| activated.borrow_mut().push(id.clone()))
                        .on_close(move |_, _| closed.set(true)),
                )
        }
    }

    /// Opens a 400 × 400 window with the menu anchored bottom-left, as the
    /// composer anchors it to the `+` button, and draws it at rest.
    fn open_menu(
        cx: &mut gpui::TestAppContext,
        items: Vec<PlusMenuItem>,
        closed: Rc<Cell<bool>>,
        activated: Rc<RefCell<Vec<SharedString>>>,
    ) -> gpui::AnyWindowHandle {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let host = cx.open_window(gpui::size(px(400.0), px(400.0)), |_, _| MenuHost { items, closed, activated });
        cx.run_until_parked();
        host.into()
    }

    fn mouse_down(cx: &mut gpui::TestAppContext, window: gpui::AnyWindowHandle, x: f32, y: f32) {
        use gpui::InputEvent as _;
        window
            .update(cx, |_, window, cx| {
                window.dispatch_event(
                    gpui::MouseDownEvent {
                        position: gpui::point(px(x), px(y)),
                        button: gpui::MouseButton::Left,
                        modifiers: Default::default(),
                        click_count: 1,
                        first_mouse: false,
                    }
                    .to_platform_input(),
                    cx,
                );
            })
            .unwrap();
    }

    fn click(cx: &mut gpui::TestAppContext, window: gpui::AnyWindowHandle, x: f32, y: f32) {
        use gpui::InputEvent as _;
        mouse_down(cx, window, x, y);
        window
            .update(cx, |_, window, cx| {
                window.dispatch_event(
                    gpui::MouseUpEvent {
                        position: gpui::point(px(x), px(y)),
                        button: gpui::MouseButton::Left,
                        modifiers: Default::default(),
                        click_count: 1,
                    }
                    .to_platform_input(),
                    cx,
                );
            })
            .unwrap();
    }

    /// A press far from the menu (top-right of the window) reports close.
    #[gpui::test]
    fn outside_press_invokes_close(cx: &mut gpui::TestAppContext) {
        let closed = Rc::new(Cell::new(false));
        let activated = Rc::new(RefCell::new(Vec::new()));
        let window = open_menu(cx, baaz_items(), closed.clone(), activated);
        mouse_down(cx, window, 300.0, 100.0);
        cx.run_until_parked();
        assert!(closed.get(), "a mouse-down outside the menu must invoke the close callback");
    }

    /// Escape after pressing a row reports close: the press focuses the menu,
    /// so the `Cancel` binding under its key context reaches it.
    #[gpui::test]
    fn escape_invokes_close(cx: &mut gpui::TestAppContext) {
        let closed = Rc::new(Cell::new(false));
        let activated = Rc::new(RefCell::new(Vec::new()));
        let window = open_menu(cx, baaz_items(), closed.clone(), activated.clone());
        // The first row spans the top of the menu frame.
        click(cx, window, 50.0, 290.0);
        cx.run_until_parked();
        assert_eq!(*activated.borrow(), vec![SharedString::from("attach")], "the probe click must land on the first row");
        assert!(!closed.get(), "pressing a row must not close via the outside path");
        cx.simulate_keystrokes(window, "escape");
        cx.run_until_parked();
        assert!(closed.get(), "Escape while the menu holds the keyboard must invoke the close callback");
    }

    /// A press inside the menu — past the old 200 px edge, inside the grown
    /// frame — neither closes via the outside path nor misses the row: it
    /// activates instead. On the old fixed-width menu the same press fell
    /// outside and would have closed.
    #[gpui::test]
    fn inside_press_does_not_close_via_outside_path(cx: &mut gpui::TestAppContext) {
        let closed = Rc::new(Cell::new(false));
        let activated = Rc::new(RefCell::new(Vec::new()));
        let window = open_menu(cx, baaz_items(), closed.clone(), activated.clone());
        // x = 200 sits past the old 200 px right edge (the menu starts 10 px
        // left of the window) yet inside the lengthened frame; y lands on the
        // last row, just above the menu's bottom padding.
        click(cx, window, 200.0, 364.0);
        cx.run_until_parked();
        assert!(!closed.get(), "a press inside the menu must not invoke the close callback");
        assert_eq!(*activated.borrow(), vec![SharedString::from("slash")], "the probe press must land on the last row");
    }
}
