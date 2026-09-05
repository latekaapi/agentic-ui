//! Card 38: the failure card (`.err`), the live status rows (`.status`), the
//! needs-you banner (`.need`) and the jump-to-latest pill (`.jump`).

use std::time::Duration;

use aui_motion::{looping, presence, shimmer_text, EnterExit, Loop, PresenceStyle};
use aui_tokens::{scale, scaled, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, FontWeight, HighlightStyle, IntoElement, SharedString, StyledText, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, kbd, spinner};
use crate::icons::{icon, IconName};
use crate::util::ClickHandler;

/// `.err{padding:10px 12px;gap:2px 10px;font-size:12.5px}` with a 16 px glyph
/// and a 12 px ink-2 body.
const ERR_PAD_Y: f32 = 10.0;
const ERR_PAD_X: f32 = 12.0;
const ERR_ROW_GAP: f32 = 2.0;
const ERR_COL_GAP: f32 = 10.0;
const ERR_TEXT: f32 = 12.5;
const ERR_GLYPH: f32 = 16.0;
/// The space before the inline `details` link in the body line.
const LINK_GAP: f32 = 4.0;

/// `.status{gap:8px;height:28px;font-size:12px}`; its spinner is 11 px.
const STATUS_GAP: f32 = 8.0;
const STATUS_H: f32 = 28.0;
const STATUS_SPINNER: f32 = 11.0;
/// The braille spinner: ten frames at roughly 80 ms each.
const BRAILLE_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const BRAILLE_PERIOD: Duration = Duration::from_millis(800);
/// The frame the design's static card shows (`⠼`), held under reduced motion.
const BRAILLE_RESTING: f32 = 0.45;

/// `.need{gap:10px;padding:8px 10px 8px 12px;font-size:12.5px}`, entering with
/// a 6 px rise.
const NEED_GAP: f32 = 10.0;
const NEED_PAD_Y: f32 = 8.0;
const NEED_PAD_LEFT: f32 = 12.0;
const NEED_PAD_RIGHT: f32 = 10.0;
const NEED_TEXT: f32 = 12.5;
const NEED_GLYPH: f32 = 14.0;
const NEED_RISE: f32 = 6.0;

/// `.jump{gap:6px;height:28px;padding:0 12px 0 10px;font:500 12px}` with a
/// 12 px chevron and the `.n` badge (`font:600 10px mono;padding:3px 6px`).
const JUMP_GAP: f32 = 6.0;
const JUMP_PAD_LEFT: f32 = 10.0;
const JUMP_PAD_RIGHT: f32 = 12.0;
const JUMP_GLYPH: f32 = 12.0;
const BADGE_PAD_Y: f32 = 3.0;
const BADGE_PAD_X: f32 = 6.0;
const BADGE_TEXT: f32 = 10.0;

/// A failure the person may want to retry. Build with [`error_card`].
#[derive(IntoElement)]
pub struct ErrorCard {
    id: ElementId,
    title: SharedString,
    detail: SharedString,
    link: Option<(SharedString, ClickHandler)>,
    retry: SharedString,
    on_retry: Option<ClickHandler>,
}

/// An error card: `title` in 600 over the 12 px `detail` line, framed by the
/// attention border (danger at 70 %, no halo).
pub fn error_card(id: impl Into<ElementId>, title: impl Into<SharedString>, detail: impl Into<SharedString>) -> ErrorCard {
    ErrorCard { id: id.into(), title: title.into(), detail: detail.into(), link: None, retry: "Retry now".into(), on_retry: None }
}

impl ErrorCard {
    /// A danger-coloured link at the end of the detail line (`details`).
    pub fn link(mut self, label: impl Into<SharedString>, on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.link = Some((label.into(), Box::new(on_click)));
        self
    }

    /// Overrides the retry button's label.
    pub fn retry_label(mut self, label: impl Into<SharedString>) -> Self {
        self.retry = label.into();
        self
    }

    /// Shows the retry button and reports its press.
    pub fn on_retry(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Box::new(f));
        self
    }
}

impl RenderOnce for ErrorCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut body = h_flex().w_full().ui(scale::FS_12).text_color(p.ink_2).gap(px(LINK_GAP)).child(div().flex_none().child(self.detail.clone()));
        if let Some((label, on_click)) = self.link {
            body = body.child(
                div()
                    .id((id.clone(), "link"))
                    .flex_none()
                    .text_color(p.danger)
                    .cursor_pointer()
                    .on_click(move |e, window, cx| on_click(e, window, cx))
                    .child(label),
            );
        }
        let mut row = h_flex()
            .id(id.clone())
            .w_full()
            .gap(px(ERR_COL_GAP))
            .py(px(ERR_PAD_Y))
            .px(px(ERR_PAD_X))
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.attention_border(p.danger))
            .bg(p.surface_1)
            .child(icon(IconName::X).size(px(ERR_GLYPH)).color(p.danger))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .gap(px(ERR_ROW_GAP))
                    .child(div().w_full().ui(ERR_TEXT).semibold().text_color(p.ink).child(self.title.clone()))
                    .child(body),
            );
        if let Some(on_retry) = self.on_retry {
            row = row.child(button((id, "retry"), self.retry).sm().on_click(move |e, window, cx| on_retry(e, window, cx)));
        }
        row
    }
}

/// The glyph that leads a [`StatusRow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusLead {
    /// No glyph.
    #[default]
    None,
    /// The 11 px accent ring spinner.
    Spinner,
    /// The mono braille spinner in accent.
    Braille,
}

/// One live status line under the transcript. Build with [`status_row`].
#[derive(IntoElement)]
pub struct StatusRow {
    id: ElementId,
    lead: StatusLead,
    label: SharedString,
    shimmer: bool,
    elapsed: Option<SharedString>,
    note: Option<SharedString>,
    key: Option<(SharedString, SharedString)>,
}

/// A status row: `Working… · 12 s · esc to interrupt`.
pub fn status_row(id: impl Into<ElementId>, label: impl Into<SharedString>) -> StatusRow {
    StatusRow { id: id.into(), lead: StatusLead::None, label: label.into(), shimmer: false, elapsed: None, note: None, key: None }
}

impl StatusRow {
    /// The leading glyph.
    pub fn lead(mut self, lead: StatusLead) -> Self {
        self.lead = lead;
        self
    }

    /// Whether the label shimmers (it does while the agent is working).
    pub fn shimmer(mut self, shimmer: bool) -> Self {
        self.shimmer = shimmer;
        self
    }

    /// The mono elapsed time after the label.
    pub fn elapsed(mut self, elapsed: impl Into<SharedString>) -> Self {
        self.elapsed = Some(elapsed.into());
        self
    }

    /// A trailing note after the separator (`1 queued message`).
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// A keycap and its trailing text after the separator (`esc to interrupt`).
    pub fn key_hint(mut self, key: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        self.key = Some((key.into(), text.into()));
        self
    }
}

impl RenderOnce for StatusRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut row = h_flex().id(id.clone()).flex_none().h(px(STATUS_H)).gap(px(STATUS_GAP)).ui(scale::FS_12).text_color(p.ink_3).whitespace_nowrap();
        match self.lead {
            StatusLead::None => {}
            StatusLead::Spinner => row = row.child(spinner((id.clone(), "spinner")).size(px(STATUS_SPINNER))),
            StatusLead::Braille => {
                let phase = looping((id.clone(), "braille"), Loop::linear(BRAILLE_PERIOD).resting(BRAILLE_RESTING), window, cx);
                let frame = BRAILLE_FRAMES[((phase * BRAILLE_FRAMES.len() as f32) as usize).min(BRAILLE_FRAMES.len() - 1)];
                row = row.child(div().flex_none().mono(scale::FS_12).text_color(p.accent).child(frame));
            }
        }
        row = if self.shimmer {
            row.child(shimmer_text((id.clone(), "label"), self.label.clone(), cx).text_size(scaled(scale::FS_12)))
        } else {
            row.child(div().flex_none().child(self.label.clone()))
        };
        if let Some(elapsed) = self.elapsed {
            row = row.child(div().flex_none().text_role(TextRole::Mono).text_px(scale::FS_12).child(elapsed));
        }
        if self.note.is_some() || self.key.is_some() {
            row = row.child(div().flex_none().child("·"));
        }
        if let Some((key, text)) = self.key {
            row = row.child(kbd(key)).child(div().flex_none().child(text));
        }
        if let Some(note) = self.note {
            row = row.child(div().flex_none().child(note));
        }
        row
    }
}

/// The banner pinned above the composer when the agent is blocked on the
/// person. Build with [`needs_you_banner`].
#[derive(IntoElement)]
pub struct NeedsYouBanner {
    id: ElementId,
    headline: SharedString,
    detail: SharedString,
    action: SharedString,
    at_rest: bool,
    on_jump: Option<ClickHandler>,
}

/// A needs-you banner: bold `headline` then plain `detail`, with a jump action.
pub fn needs_you_banner(id: impl Into<ElementId>, headline: impl Into<SharedString>, detail: impl Into<SharedString>) -> NeedsYouBanner {
    NeedsYouBanner { id: id.into(), headline: headline.into(), detail: detail.into(), action: "Jump to it".into(), at_rest: false, on_jump: None }
}

impl NeedsYouBanner {
    /// Overrides the action's label.
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action = label.into();
        self
    }

    /// Skips the enter animation and draws the settled banner (static captures).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// The action was pressed.
    pub fn on_jump(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_jump = Some(Box::new(f));
        self
    }
}

impl RenderOnce for NeedsYouBanner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let style = if self.at_rest {
            PresenceStyle { opacity: 1.0, offset_y: px(0.0), scale: 1.0 }
        } else {
            PresenceStyle::fade_rise(presence((id.clone(), "enter"), true, EnterExit::DEFAULT, window, cx), NEED_RISE)
        };

        // Bold lead-in and plain rest are one wrapping paragraph.
        let text = format!("{} {}", self.headline, self.detail);
        let bold = 0..self.headline.len();
        let mut row = h_flex()
            .id(id.clone())
            .relative()
            .top(style.offset_y)
            .opacity(style.opacity)
            .w_full()
            .gap(px(NEED_GAP))
            .py(px(NEED_PAD_Y))
            .pl(px(NEED_PAD_LEFT))
            .pr(px(NEED_PAD_RIGHT))
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.warning_soft)
            .bg(p.warning_soft)
            .ui(NEED_TEXT)
            .text_color(p.ink)
            .child(icon(IconName::Shield).size(px(NEED_GLYPH)).color(p.warning))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(StyledText::new(text).with_highlights([(bold, HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })])),
            );
        if let Some(on_jump) = self.on_jump {
            row = row.child(button((id, "jump"), self.action).xs().primary().on_click(move |e, window, cx| on_jump(e, window, cx)));
        }
        row
    }
}

/// The floating jump-to-latest pill. Build with [`jump_pill`].
#[derive(IntoElement)]
pub struct JumpPill {
    id: ElementId,
    label: SharedString,
    count: Option<u32>,
    on_jump: Option<ClickHandler>,
}

/// A jump pill with `label` (`Jump to latest`); [`JumpPill::count`] adds the
/// new-turn badge.
pub fn jump_pill(id: impl Into<ElementId>, label: impl Into<SharedString>) -> JumpPill {
    JumpPill { id: id.into(), label: label.into(), count: None, on_jump: None }
}

impl JumpPill {
    /// The number of new turns below the reader.
    pub fn count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    /// The pill was pressed.
    pub fn on_jump(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_jump = Some(Box::new(f));
        self
    }
}

impl RenderOnce for JumpPill {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut pill = h_flex()
            .id(self.id)
            .flex_none()
            .h(px(scale::H_MD))
            .gap(px(JUMP_GAP))
            .pl(px(JUMP_PAD_LEFT))
            .pr(px(JUMP_PAD_RIGHT))
            .rounded_full()
            .bg(p.overlay)
            .border_1()
            .border_color(p.line_strong)
            .shadow(p.shadow(2))
            .ui(scale::FS_12)
            .medium()
            .text_color(p.ink)
            .whitespace_nowrap()
            .child(icon(IconName::ChevronDown).size(px(JUMP_GLYPH)))
            .child(self.label);
        if let Some(count) = self.count {
            pill = pill.child(
                div()
                    .flex_none()
                    .py(px(BADGE_PAD_Y))
                    .px(px(BADGE_PAD_X))
                    .rounded_full()
                    .bg(p.ink)
                    .text_color(p.bg)
                    .font_family(scale::FONT_MONO)
                    .text_px(BADGE_TEXT)
                    .line_height(gpui::relative(1.0))
                    .semibold()
                    .child(count.to_string()),
            );
        }
        if let Some(on_jump) = self.on_jump {
            pill = pill.cursor_pointer().on_click(move |e, window, cx| on_jump(e, window, cx));
        }
        pill
    }
}
