//! `.mk`: marker rows inside the transcript — session started, hand-off,
//! context compacted, permission mode changed. One line, 11.5 px ink-3, a
//! hairline on both sides, 12 px glyph.

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;

/// `.mk{gap:10px;font-size:11.5px;margin:10px 0}` — block flow collapses the
/// vertical margins, so rows carry a top margin only.
const GAP: f32 = 10.0;
const TEXT: f32 = 11.5;
const MARGIN_Y: f32 = 10.0;
/// `.mk .i{width:12px;height:12px}`.
const GLYPH: f32 = 12.0;
/// `.hand{gap:6px;padding:3px 8px}` with 11 px marks and arrow.
const HAND_GAP: f32 = 6.0;
const HAND_PAD_Y: f32 = 3.0;
const HAND_PAD_X: f32 = 8.0;
const HAND_MARK: f32 = 11.0;

/// A hand-off between two agents, shown as a pill with both marks.
#[derive(Debug, Clone, PartialEq)]
pub struct HandOff {
    /// The agent handing off.
    pub from: (Provider, SharedString),
    /// The agent taking over.
    pub to: (Provider, SharedString),
}

/// A marker row. Build with [`marker_row`].
#[derive(IntoElement)]
pub struct MarkerRow {
    id: ElementId,
    glyph: Option<(IconName, Option<Hsla>)>,
    hand_off: Option<HandOff>,
    runs: Vec<Run>,
}

/// A marker with plain text; add emphasis, links or a hand-off with the builders.
pub fn marker_row(id: impl Into<ElementId>) -> MarkerRow {
    MarkerRow { id: id.into(), glyph: None, hand_off: None, runs: Vec::new() }
}

impl MarkerRow {
    /// The 12 px leading glyph, optionally tinted (a warning shield).
    pub fn glyph(mut self, name: IconName, color: Option<Hsla>) -> Self {
        self.glyph = Some((name, color));
        self
    }

    /// The hand-off pill before the text.
    pub fn hand_off(mut self, hand_off: HandOff) -> Self {
        self.hand_off = Some(hand_off);
        self
    }

    /// Plain ink-3 text.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.runs.push(Run::Text(text.into()));
        self
    }

    /// Emphasised text: ink, weight 500.
    pub fn strong(mut self, text: impl Into<SharedString>) -> Self {
        self.runs.push(Run::Strong(text.into()));
        self
    }

    /// An accent-ink link.
    pub fn link(mut self, text: impl Into<SharedString>, on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.runs.push(Run::Link(text.into(), Box::new(on_click)));
        self
    }
}

/// One run of marker text; colours are resolved at render time.
enum Run {
    Text(SharedString),
    Strong(SharedString),
    Link(SharedString, crate::util::ClickHandler),
}

impl RenderOnce for MarkerRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let rule = || div().flex_1().h(px(1.0)).bg(p.line);
        let mut row = h_flex().id(self.id.clone()).w_full().mt(px(MARGIN_Y)).gap(px(GAP)).ui(TEXT).text_color(p.ink_3).whitespace_nowrap().child(rule());
        if let Some((name, color)) = self.glyph {
            row = row.child(icon(name).size(px(GLYPH)).color(color.unwrap_or(p.ink_3)));
        }
        if let Some(hand) = self.hand_off {
            row = row.child(
                h_flex()
                    .gap(px(HAND_GAP))
                    .py(px(HAND_PAD_Y))
                    .px(px(HAND_PAD_X))
                    .rounded_full()
                    .bg(p.surface_2)
                    .border_1()
                    .border_color(p.line)
                    .child(provider_mark(hand.from.0).size(px(HAND_MARK)))
                    .child(hand.from.1)
                    .child(icon(IconName::ArrowRight).size(px(HAND_MARK)).color(p.ink_3))
                    .child(provider_mark(hand.to.0).size(px(HAND_MARK)))
                    .child(hand.to.1),
            );
        }
        // Runs sit side by side; emphasis and links carry their own colour.
        let mut text = h_flex().gap(px(scale::SP_2)).text_color(p.ink_3);
        for (i, run) in self.runs.into_iter().enumerate() {
            text = match run {
                Run::Text(t) => text.child(div().whitespace_nowrap().child(t)),
                Run::Strong(t) => text.child(div().whitespace_nowrap().medium().text_color(p.ink).child(t)),
                Run::Link(t, on_click) => {
                    let id: ElementId = (self.id.clone(), SharedString::from(format!("link-{i}"))).into();
                    text.child(div().id(id).whitespace_nowrap().text_color(p.accent_ink).cursor_pointer().on_click(move |e, w, cx| on_click(e, w, cx)).child(t))
                }
            };
        }
        row.child(text).child(rule())
    }
}
