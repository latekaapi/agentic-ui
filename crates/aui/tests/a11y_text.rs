//! Accessibility writes must land in the state: `SetValue` replaces the whole
//! value, `ReplaceSelectedText` inserts at the caret or over the selection,
//! `Focus` focuses, `Change` fires for each edit, and anything else is
//! ignored.
//!
//! These tests drive [`aui::a11y_text::apply_a11y_text_action`] directly — the
//! same routing function the element handlers call. gpui's test harness
//! cannot dispatch an `accesskit::ActionRequest` to a window instead:
//! `Window::handle_a11y_action` is `pub(crate)`, and `on_a11y_action`
//! listeners only register while a11y is active, which a test window never is.

use std::cell::RefCell;
use std::rc::Rc;

use aui::a11y_text::{a11y_text_field, a11y_text_input, apply_a11y_text_action};
use aui::composer::{composer, composer_state};
use gpui::accesskit::ActionData;
use gpui::{
    AccessibleAction, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Subscription, TestAppContext, VisualTestContext, Window,
};
use gpui_kit::base::input::{InputEvent, InputState, TextareaState};

/// A host holding the two shapes under test: the composer's multi-line state
/// and a consumer-style single-line search state.
struct Host {
    text: Entity<TextareaState>,
    search: Entity<InputState>,
    changes: Rc<RefCell<usize>>,
    _sub: Subscription,
    _search_sub: Subscription,
}

impl Render for Host {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::div().id("a11y-text-host")
    }
}

fn with_host(cx: &mut TestAppContext, f: impl FnOnce(Entity<Host>, &mut VisualTestContext)) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let (host, ctx) = cx.add_window_view(|window, cx| {
        let text = cx.new(|cx| composer_state("Message", window, cx));
        let search = cx.new(|cx| InputState::new(window, cx));
        let changes = Rc::new(RefCell::new(0));
        let count = changes.clone();
        let sub = cx.subscribe(&text, move |_, _, event: &InputEvent, _| {
            if matches!(event, InputEvent::Change) {
                *count.borrow_mut() += 1;
            }
        });
        let count = changes.clone();
        let search_sub = cx.subscribe(&search, move |_, _, event: &InputEvent, _| {
            if matches!(event, InputEvent::Change) {
                *count.borrow_mut() += 1;
            }
        });
        Host {
            text,
            search,
            changes,
            _sub: sub,
            _search_sub: search_sub,
        }
    });
    f(host, ctx);
}

fn changes_of(host: &Entity<Host>, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| *host.read(cx).changes.borrow())
}

fn text_value(host: &Entity<Host>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| host.read(cx).text.read(cx).value().to_string())
}

#[gpui::test]
fn set_value_replaces_the_whole_value_and_fires_change(cx: &mut TestAppContext) {
    with_host(cx, |host, cx| {
        let text = cx.update(|_, cx| host.read(cx).text.clone());
        cx.update(|window, cx| {
            text.update(cx, |state, cx| state.replace_all("typed hello", window, cx));
        });
        let seeded = changes_of(&host, cx);
        assert_eq!(seeded, 1, "seeding must fire Change like typed text");

        let data = ActionData::Value("dictated text".into());
        let handled = cx.update(|window, cx| {
            apply_a11y_text_action(&text, AccessibleAction::SetValue, Some(&data), window, cx)
        });
        assert!(handled);
        assert_eq!(text_value(&host, cx), "dictated text");
        assert_eq!(
            changes_of(&host, cx),
            seeded + 1,
            "SetValue must fire Change"
        );

        // A second write replaces, not appends.
        let data = ActionData::Value("again".into());
        let handled = cx.update(|window, cx| {
            apply_a11y_text_action(&text, AccessibleAction::SetValue, Some(&data), window, cx)
        });
        assert!(handled);
        assert_eq!(text_value(&host, cx), "again");
    });
}

#[gpui::test]
fn replace_selected_text_inserts_at_the_caret(cx: &mut TestAppContext) {
    with_host(cx, |host, cx| {
        let text = cx.update(|_, cx| host.read(cx).text.clone());
        cx.update(|window, cx| {
            text.update(cx, |state, cx| {
                state.replace_all("hello", window, cx);
                state.set_selected_range(5..5, cx);
            });
        });
        let seeded = changes_of(&host, cx);

        let data = ActionData::Value(" world".into());
        let handled = cx.update(|window, cx| {
            apply_a11y_text_action(
                &text,
                AccessibleAction::ReplaceSelectedText,
                Some(&data),
                window,
                cx,
            )
        });
        assert!(handled);
        assert_eq!(text_value(&host, cx), "hello world");
        assert_eq!(changes_of(&host, cx), seeded + 1, "insert must fire Change");
    });
}

#[gpui::test]
fn replace_selected_text_replaces_the_selection(cx: &mut TestAppContext) {
    with_host(cx, |host, cx| {
        let text = cx.update(|_, cx| host.read(cx).text.clone());
        cx.update(|window, cx| {
            text.update(cx, |state, cx| {
                state.replace_all("hello world", window, cx);
                state.set_selected_range(0..5, cx);
            });
        });

        let data = ActionData::Value("bye".into());
        let handled = cx.update(|window, cx| {
            apply_a11y_text_action(
                &text,
                AccessibleAction::ReplaceSelectedText,
                Some(&data),
                window,
                cx,
            )
        });
        assert!(handled);
        assert_eq!(text_value(&host, cx), "bye world");
    });
}

#[gpui::test]
fn focus_moves_keyboard_focus_into_the_field(cx: &mut TestAppContext) {
    with_host(cx, |host, cx| {
        let text = cx.update(|_, cx| host.read(cx).text.clone());
        let handled = cx.update(|window, cx| {
            apply_a11y_text_action(&text, AccessibleAction::Focus, None, window, cx)
        });
        assert!(handled);
        cx.update(|window, cx| {
            assert!(
                text.read(cx).focus_handle(cx).is_focused(window),
                "Focus must focus the input"
            );
        });
    });
}

#[gpui::test]
fn unhandled_actions_and_missing_payloads_are_ignored(cx: &mut TestAppContext) {
    with_host(cx, |host, cx| {
        let text = cx.update(|_, cx| host.read(cx).text.clone());
        cx.update(|window, cx| {
            text.update(cx, |state, cx| state.replace_all("keep", window, cx));
        });
        let seeded = changes_of(&host, cx);

        let ignored = cx.update(|window, cx| {
            apply_a11y_text_action(&text, AccessibleAction::Increment, None, window, cx)
        });
        assert!(!ignored, "Increment is not a text action");
        let no_payload = cx.update(|window, cx| {
            apply_a11y_text_action(&text, AccessibleAction::SetValue, None, window, cx)
        });
        assert!(
            !no_payload,
            "SetValue without its value must not clear the field"
        );
        let wrong_payload = cx.update(|window, cx| {
            apply_a11y_text_action(
                &text,
                AccessibleAction::SetValue,
                Some(&ActionData::NumericValue(3.0)),
                window,
                cx,
            )
        });
        assert!(
            !wrong_payload,
            "SetValue with a numeric payload is not a text write"
        );

        assert_eq!(text_value(&host, cx), "keep");
        assert_eq!(
            changes_of(&host, cx),
            seeded,
            "ignored actions fire nothing"
        );
    });
}

#[gpui::test]
fn the_same_routing_serves_a_single_line_search_field(cx: &mut TestAppContext) {
    with_host(cx, |host, cx| {
        let search = cx.update(|_, cx| host.read(cx).search.clone());
        let data = ActionData::Value("query".into());
        let handled = cx.update(|window, cx| {
            apply_a11y_text_action(&search, AccessibleAction::SetValue, Some(&data), window, cx)
        });
        assert!(handled);
        cx.update(|_, cx| {
            assert_eq!(search.read(cx).value().to_string(), "query");
            // Single-line fields park the caret at the end, like HTML inputs.
            assert_eq!(search.read(cx).selected_range(), 5..5);
        });
    });
}

/// The helpers draw a frame each: the search wrapper and a composer frame
/// with its "Message" label.
struct SmokeHost {
    search: Entity<InputState>,
    text: Entity<TextareaState>,
}

impl Render for SmokeHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui::div()
            .id("a11y-smoke")
            .child(a11y_text_input("a11y-search", "Search", &self.search, cx))
            .child(
                a11y_text_field("a11y-composer-text", "Message", &self.text, true, cx).child(
                    composer(
                        "a11y-composer-card",
                        &self.text,
                        aui::icons::Provider::Claude,
                        "Opus 4.6",
                    ),
                ),
            )
    }
}

#[gpui::test]
fn helpers_render_without_panicking(cx: &mut TestAppContext) {
    cx.update(|cx| aui::init(aui::tokens::ThemeKind::Dark, cx));
    let (_host, cx) = cx.add_window_view(|window, cx| {
        let text = cx.new(|cx| composer_state("Message", window, cx));
        let search = cx.new(|cx| InputState::new(window, cx));
        SmokeHost { search, text }
    });
    cx.run_until_parked();
}
