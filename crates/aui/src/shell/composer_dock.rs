//! `.comp`: the docked composer — full pane width, 1 px top hairline,
//! surface-1, padding 12 32 12. This is the shell's placeholder: text area
//! stand-in plus the toolbar (`+`, model chip, mode chip, context, send/stop).
//! The real editor arrives with card 40.

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_motion::{icon_morph, IconMorph};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{chip, icon_button};

/// `.comp{padding:12px 32px 12px}`.
const PAD_Y: f32 = 12.0;
/// Default horizontal padding (card 10 uses 28).
pub const PAD_X: f32 = 32.0;
/// `.comp .txt{font-size:13.5px;line-height:1.5;min-height:40px;padding:0 2px}`.
const TEXT_SIZE: f32 = 13.5;
const TEXT_MIN_H: f32 = 40.0;
const TEXT_PAD_X: f32 = 2.0;
/// `.comp .bar{gap:6px;margin-top:10px}`.
const BAR_GAP: f32 = 6.0;
const BAR_TOP: f32 = 10.0;
/// The mark inside the model chip: 12 px.
const CHIP_MARK: f32 = 12.0;
/// `.subtle{font-size:11px;margin-left:6px}` for the context percentage.
const CONTEXT_MARGIN: f32 = 6.0;
/// The send / stop glyph: 13 px.
const SEND_ICON: f32 = 13.0;

/// What the composer toolbar asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockedComposerIntent {
    /// Send the draft.
    Send,
    /// Interrupt the running turn.
    Stop,
    /// Open the `+` menu.
    Plus,
    /// Open the model picker.
    Model,
    /// Open the mode picker.
    Mode,
}

type IntentHandler = std::rc::Rc<dyn Fn(DockedComposerIntent, &mut Window, &mut App)>;

/// The docked composer. Build with [`docked_composer`].
#[derive(IntoElement)]
pub struct DockedComposer {
    id: ElementId,
    placeholder: SharedString,
    provider: Provider,
    model: SharedString,
    mode: SharedString,
    context_percent: Option<u8>,
    streaming: bool,
    pad_x: f32,
    on_intent: Option<IntentHandler>,
}

/// A composer for `provider` / `model`.
pub fn docked_composer(id: impl Into<ElementId>, provider: Provider, model: impl Into<SharedString>) -> DockedComposer {
    DockedComposer {
        id: id.into(),
        placeholder: "Reply, or type / for commands and @ to mention files".into(),
        provider,
        model: model.into(),
        mode: "Plan".into(),
        context_percent: None,
        streaming: false,
        pad_x: PAD_X,
        on_intent: None,
    }
}

impl DockedComposer {
    /// The placeholder text.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// The mode chip label.
    pub fn mode(mut self, mode: impl Into<SharedString>) -> Self {
        self.mode = mode.into();
        self
    }

    /// Shows `context N%` after the chips.
    pub fn context_percent(mut self, percent: u8) -> Self {
        self.context_percent = Some(percent);
        self
    }

    /// A turn is running: the send button morphs to stop.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Overrides the horizontal padding.
    pub fn pad_x(mut self, pad: f32) -> Self {
        self.pad_x = pad;
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(DockedComposerIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for DockedComposer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let control = cx.aui().metrics.control_md;
        let emit = |intent: DockedComposerIntent| {
            let handler = self.on_intent.clone();
            move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(intent, w, cx)
                }
            }
        };

        let sample = icon_morph((id.clone(), "send-stop"), self.streaming, window, cx);
        let glyph = |name: IconName| icon(name).size(px(SEND_ICON)).color(gpui::white());
        let send_intent = if self.streaming { DockedComposerIntent::Stop } else { DockedComposerIntent::Send };
        let send = div()
            .id((id.clone(), "send"))
            .flex_none()
            .size(control)
            .rounded(px(scale::R_SM))
            .bg(p.accent)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(p.accent_strong))
            .on_click(emit(send_intent))
            .child(IconMorph::new(sample, px(SEND_ICON), glyph(IconName::ArrowUp), glyph(IconName::Stop)));

        let mut bar = h_flex()
            .w_full()
            .mt(px(BAR_TOP))
            .gap(px(BAR_GAP))
            .child(icon_button((id.clone(), "plus"), IconName::Plus).on_click(emit(DockedComposerIntent::Plus)))
            .child(
                chip((id.clone(), "model"), self.model)
                    .composer()
                    .leading(provider_mark(self.provider).size(px(CHIP_MARK)))
                    .chevron()
                    .on_click(emit(DockedComposerIntent::Model)),
            )
            .child(chip((id.clone(), "mode"), self.mode).composer().chevron().on_click(emit(DockedComposerIntent::Mode)));
        if let Some(percent) = self.context_percent {
            bar = bar.child(div().ml(px(CONTEXT_MARGIN)).ui(scale::FS_11).text_color(p.ink_3).whitespace_nowrap().child(format!("context {percent}%")));
        }
        bar = bar.child(div().flex_1()).child(send);

        v_flex()
            .id(id)
            .w_full()
            .flex_none()
            .border_t_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .py(px(PAD_Y))
            .px(px(self.pad_x))
            .child(
                div()
                    .w_full()
                    .min_h(px(TEXT_MIN_H))
                    .px(px(TEXT_PAD_X))
                    .ui(TEXT_SIZE)
                    .text_color(p.ink_3)
                    .child(self.placeholder),
            )
            .child(bar)
    }
}
