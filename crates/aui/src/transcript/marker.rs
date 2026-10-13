//! Marker rows inside the transcript (D4, docs/41 §2.8) — session started,
//! context compacted, permission mode changed, handed off. One centred
//! caption line in muted between two 7 % rules, 10 above and below, gap 10;
//! a 12 px glyph; a hand-off leads with both provider marks (14) and an
//! arrow. Links are underlined in the foreground.

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;

/// Gap 10, caption, 10 above and below.
const GAP: f32 = 10.0;
const TEXT: f32 = 12.0;
const MARGIN_Y: f32 = 10.0;
/// The leading glyph.
const GLYPH: f32 = 12.0;
/// The hand-off's two marks and the arrow between them.
const HAND_GAP: f32 = 6.0;
const HAND_MARK: f32 = 14.0;
const HAND_ARROW: f32 = 12.0;

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
        let rule = || div().flex_1().min_w(px(16.0)).h(px(1.0)).bg(p.line);
        let mut row = h_flex().id(self.id.clone()).w_full().my(px(MARGIN_Y)).items_center().gap(px(GAP)).ui(TEXT).text_color(p.ink_2).whitespace_nowrap().child(rule());
        if let Some((name, color)) = self.glyph {
            row = row.child(icon(name).size(px(GLYPH)).color(color.unwrap_or(p.ink_2)));
        }
        if let Some(hand) = self.hand_off {
            row = row.child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap(px(HAND_GAP))
                    .child(provider_mark(hand.from.0).size(px(HAND_MARK)))
                    .child(icon(IconName::ArrowRight).size(px(HAND_ARROW)).color(p.ink_2))
                    .child(provider_mark(hand.to.0).size(px(HAND_MARK))),
            );
        }
        // Runs sit side by side; emphasis and links carry their own colour.
        let mut text = h_flex().flex_none().gap(px(scale::SP_1)).text_color(p.ink_2);
        for (i, run) in self.runs.into_iter().enumerate() {
            text = match run {
                Run::Text(t) => text.child(div().whitespace_nowrap().child(t)),
                Run::Strong(t) => text.child(div().whitespace_nowrap().semibold().text_color(p.ink).child(t)),
                Run::Link(t, on_click) => {
                    let id: ElementId = (self.id.clone(), SharedString::from(format!("link-{i}"))).into();
                    crate::data::record_ax_label(&t);
                    text.child(
                        div()
                            .id(id)
                            .whitespace_nowrap()
                            .text_color(p.ink_2)
                            .underline()
                            .cursor_pointer()
                            .hover(|d| d.text_color(p.ink))
                            .role(gpui::Role::Link)
                            .aria_label(t.clone())
                            .on_click(move |e, w, cx| on_click(e, w, cx))
                            .child(t),
                    )
                }
            };
        }
        row.child(text).child(rule())
    }
}
