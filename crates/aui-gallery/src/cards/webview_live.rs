//! `workbench/webview` — the browser pane driven by a live `WebBackend`.
//!
//! Card 51 is a still of this pane; this is the pane itself, over
//! [`FakeWebBackend`]. The chrome around it — the frame and the tab strip — is
//! card 51's, so the two entries can be compared side by side, but everything
//! inside comes from `aui-webview`: the nav row acts, the pointer outlines
//! elements as it moves over the page, a click drops a numbered pin and opens
//! its note, and the annotations panel collects them.
//!
//! It opens in the state card 51 froze: annotate mode on, two elements already
//! pinned, the panel open. The seeds go through the same click path a person
//! would, so nothing here is a second way of making an annotation.

use aui::data::{icon_button, ButtonSize};
use aui::shell::{tab_strip, TabItem};
use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui};
use aui_webview::{webview_pane, FakeWebBackend, WebviewState};
use gpui::*;
use gpui_kit::base::v_flex;

/// The card is 980×640 and the gallery frame adds 20, so the pane pulls in by
/// 8 the way card 51 does and fills the rest.
pub(super) const FRAME_INSET: f32 = -8.0;
pub(super) const FRAME_W: f32 = 980.0 - 24.0;
pub(super) const FRAME_H: f32 = 610.0;
/// `.tabs .btn.icon.xs svg{width:12px;height:12px}`.
pub(super) const TAB_GLYPH: f32 = 12.0;

/// The two pins the card opens with, as page coordinates and their notes. The
/// points fall inside the headline and the Starter card of the fake page.
const SEEDS: [((f32, f32), &str); 2] = [
    ((120.0, 44.0), "Headline is fine; subtitle needs the free-trial length."),
    ((110.0, 180.0), "Make the Starter card stand out; it is the plan most people pick."),
];

/// The pane's tabs, as card 51 shows them.
pub(super) fn tabs() -> Vec<TabItem> {
    vec![
        TabItem::new("checkout", "localhost:3000/checkout", IconName::Globe).closable(false),
        TabItem::new("pricing", "localhost:3000/pricing", IconName::Globe),
    ]
}

/// A ghost icon button of the size card 51's tab strip uses.
pub(super) fn action(name: &'static str, glyph: IconName) -> aui::data::Button {
    icon_button(name, glyph).ghost().size(ButtonSize::Xs).icon_size(px(TAB_GLYPH))
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;

    let state = window.use_keyed_state(SharedString::from("webview-live"), cx, |_, cx| {
        let mut state = WebviewState::new(Box::new(FakeWebBackend::new()), cx);
        state.set_annotate(true);
        state.seed_pins(&SEEDS);
        state
    });

    v_flex()
        .w(px(FRAME_W))
        .h(px(FRAME_H))
        .flex_none()
        .m(px(FRAME_INSET))
        .overflow_hidden()
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line_strong)
        .bg(p.surface_1)
        .child(
            tab_strip("webview-live-tabs", tabs(), 1)
                .after_tabs(action("webview-live-plus", IconName::Plus))
                .trailing(action("webview-live-layout", IconName::Layout))
                .trailing(action("webview-live-link", IconName::Link))
                .trailing(action("webview-live-close", IconName::X)),
        )
        .child(webview_pane("webview-live-pane", &state).on_intent(|_, _, _| {}))
        .into_any_element()
}

// ---------------------------------------------------------------------------
// The real backend (`--features wry`)
// ---------------------------------------------------------------------------

/// `workbench/webview-real`: the same pane over a real WKWebView.
#[cfg(feature = "wry")]
mod real {
    use super::{action, tabs, FRAME_H, FRAME_INSET, FRAME_W};
    use aui::overlay::{command_palette, PaletteIcon, PaletteItem, PaletteSection};
    use aui::shell::tab_strip;
    use aui_icons::IconName;
    use aui_tokens::{scale, ActiveAui};
    use aui_webview::{webview_pane, WebviewIntent, WebviewState, WryBackend};
    use gpui::*;
    use gpui_kit::base::v_flex;

    /// The page the card opens on. A real document on a real host, so the
    /// title, the loading hairline and the annotator all have something true
    /// to report; `⌘L` types any other.
    const START_URL: &str = "https://example.com";
    /// The scrim over the page while the palette is open, as a fraction.
    const SCRIM: f32 = 0.55;
    /// The palette's box inside the frame.
    const PALETTE_W: f32 = 560.0;
    const PALETTE_TOP: f32 = 96.0;

    /// The palette's rows. The card is a demonstration of *obscuring*, so the
    /// items only have to look like a palette.
    fn sections() -> Vec<PaletteSection> {
        vec![PaletteSection::new(
            "Actions",
            vec![
                PaletteItem::new("reload", PaletteIcon::Glyph(IconName::Refresh), "Reload page").key("⌘").key("R"),
                PaletteItem::new("annotate", PaletteIcon::Glyph(IconName::Edit), "Annotate this page").key("⌘").key("⇧").key("A"),
                PaletteItem::new("shot", PaletteIcon::Glyph(IconName::Camera), "Screenshot the page"),
            ],
        )]
    }

    /// Builds the card content.
    pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
        let p = cx.aui().colors;

        let state = window.use_keyed_state(SharedString::from("webview-real"), cx, |window, cx| {
            // The pane moves the webview to the page area's box on its first
            // prepaint, so it is built with an empty one rather than a guess.
            match WryBackend::new_at(&*window, START_URL, (0.0, 0.0), (0.0, 0.0)) {
                Ok(backend) => WebviewState::new(Box::new(backend), cx),
                Err(error) => {
                    eprintln!("aui-gallery: no WKWebView ({error}); falling back to the scripted page");
                    WebviewState::new(Box::new(aui_webview::FakeWebBackend::new()), cx)
                }
            }
        });

        // ⌘K. The palette is a gpui overlay over a native page, which is
        // exactly the case the webview cannot composite: opening it hides the
        // native view and paints its last screenshot instead.
        let palette_open = window.use_keyed_state(SharedString::from("webview-real-palette"), cx, |_, _| false);
        let open = *palette_open.read(cx);

        let mut frame = v_flex()
            .id("webview-real-frame")
            .relative()
            .w(px(FRAME_W))
            .h(px(FRAME_H))
            .flex_none()
            .m(px(FRAME_INSET))
            .overflow_hidden()
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.surface_1)
            .child(
                tab_strip("webview-real-tabs", tabs(), 1)
                    .after_tabs(action("webview-real-plus", IconName::Plus))
                    .trailing(action("webview-real-layout", IconName::Layout))
                    .trailing(action("webview-real-link", IconName::Link))
                    .trailing({
                        // A visible way in for the ⌘K demonstration, since a
                        // native page eats most keystrokes over itself.
                        let toggle = palette_open.clone();
                        let state = state.clone();
                        icon_toggle("webview-real-cmdk", IconName::Search, move |_: &ClickEvent, _, cx| {
                            let now = !*toggle.read(cx);
                            toggle.update(cx, |v, cx| {
                                *v = now;
                                cx.notify();
                            });
                            state.update(cx, |state, cx| {
                                state.set_obscured(now);
                                cx.notify();
                            });
                        })
                    }),
            )
            .child(webview_pane("webview-real-pane", &state).on_intent(|intent, _, _| {
                // The gallery has no agent to send annotations to; printing
                // what would be sent is how the card shows the bridge reached a
                // host. `SendAnnotations` carries the PNG, so it is summarised
                // rather than debug-printed.
                match &intent {
                    WebviewIntent::SendAnnotations { annotations, screenshot, url } => eprintln!(
                        "aui-gallery: send {} annotation(s) from {url} with {}",
                        annotations.len(),
                        match screenshot {
                            Some(bytes) => format!("a {}-byte PNG", bytes.len()),
                            None => String::from("no screenshot"),
                        }
                    ),
                    other => eprintln!("aui-gallery: webview intent {other:?}"),
                }
            }));

        if open {
            frame = frame
                .child(div().absolute().inset_0().bg(p.bg.opacity(SCRIM)))
                .child(
                    div()
                        .absolute()
                        .top(px(PALETTE_TOP))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(div().w(px(PALETTE_W)).child(command_palette("webview-real-palette-ui", "", sections(), 0).at_rest())),
                );
        }

        frame.into_any_element()
    }

    /// A ghost icon button in the tab strip that runs `on_click`.
    fn icon_toggle(id: &'static str, glyph: IconName, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> impl IntoElement {
        aui::data::icon_button(id, glyph)
            .ghost()
            .size(aui::data::ButtonSize::Xs)
            .icon_size(px(super::TAB_GLYPH))
            .on_click(on_click)
    }
}

#[cfg(feature = "wry")]
pub use real::build as build_real;
