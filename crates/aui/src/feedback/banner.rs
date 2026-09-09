//! Card 13: inline banners.
//!
//! A banner is one line of the transcript or the pane: an icon on the left,
//! the message, one xs button on the right. Per the design rules only the
//! states that need the person are tinted — waiting (warning-soft) and error
//! (danger-soft); info and success stay on the plain surface with the
//! hairline border.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette};
use gpui::{div, prelude::*, px, App, ElementId, FontWeight, HighlightStyle, Hsla, IntoElement, SharedString, StyledText, Window};
use gpui_kit::base::h_flex;

use crate::data::{button, status_dot, ButtonSize, Button};

/// `.ban{gap:10px;padding:8px 10px 8px 12px;border-radius:var(--r-md);font-size:12.5px}`.
const BANNER_GAP: f32 = 10.0;
const BANNER_PAD_Y: f32 = 8.0;
const BANNER_PAD_LEFT: f32 = 12.0;
const BANNER_PAD_RIGHT: f32 = 10.0;
const BANNER_RADIUS: f32 = scale::R_MD;
const BANNER_TEXT: f32 = 12.5;
/// `.ban .ic{width:14px;height:14px;flex:none}`.
const BANNER_GLYPH: f32 = 14.0;
/// The connection banner's dot: `.dot.done.ic{width:7px;height:7px;margin:0 3px}`.
const BANNER_DOT: f32 = 7.0;
const BANNER_DOT_MARGIN: f32 = 3.0;

/// Which state a banner reports. The kind picks the icon, the ink and whether
/// the row is tinted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerKind {
    /// `.ban.warn`: the agent needs the person — warning shield, warning-soft
    /// ground and border.
    Waiting,
    /// `.ban.info`: a fact about the session — info clock on the plain surface.
    #[default]
    Info,
    /// `.ban.err`: something failed — danger `x`, danger-soft ground and border.
    Error,
    /// A quiet "it worked" line: the 7 px success dot on the plain surface.
    Success,
}

impl BannerKind {
    /// `(ground, border)` for the row.
    fn surface(self, p: &Palette) -> (Hsla, Hsla) {
        match self {
            BannerKind::Waiting => (p.warning_soft, p.warning_soft),
            BannerKind::Error => (p.danger_soft, p.danger_soft),
            BannerKind::Info | BannerKind::Success => (p.surface_1, p.line),
        }
    }

    /// The 14 px glyph and its ink; `None` for [`BannerKind::Success`], which
    /// leads with a status dot instead.
    fn glyph(self, p: &Palette) -> Option<(IconName, Hsla)> {
        match self {
            BannerKind::Waiting => Some((IconName::Shield, p.warning)),
            BannerKind::Info => Some((IconName::Clock, p.info)),
            BannerKind::Error => Some((IconName::X, p.danger)),
            BannerKind::Success => None,
        }
    }
}

/// One run of a banner's message. A banner line mixes weights and faces
/// ("**Waiting for you.** Allow `pnpm test` to run?").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BannerRun {
    /// Plain UI text.
    Text(SharedString),
    /// The leading clause, 600.
    Bold(SharedString),
    /// A command or a path, in the mono face.
    Mono(SharedString),
}

impl BannerRun {
    fn text(&self) -> &SharedString {
        match self {
            BannerRun::Text(t) | BannerRun::Bold(t) | BannerRun::Mono(t) => t,
        }
    }
}

/// How the banner's single button is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerActionStyle {
    /// Accent fill — the action the person is being asked for.
    Primary,
    /// The filled, bordered default.
    #[default]
    Secondary,
    /// Quiet, for dismissals.
    Ghost,
}

/// The banner's single action button was pressed.
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// An inline banner. Build with [`banner`].
#[derive(IntoElement)]
pub struct Banner {
    id: ElementId,
    kind: BannerKind,
    runs: Vec<BannerRun>,
    action: Option<(SharedString, BannerActionStyle)>,
    on_action: Option<ActionHandler>,
    secondary: Option<(SharedString, BannerActionStyle)>,
    on_secondary: Option<ActionHandler>,
}

/// A one-line banner in `kind`'s colours, carrying `runs` as its message.
pub fn banner(id: impl Into<ElementId>, kind: BannerKind, runs: Vec<BannerRun>) -> Banner {
    Banner { id: id.into(), kind, runs, action: None, on_action: None, secondary: None, on_secondary: None }
}

impl Banner {
    /// The single xs button at the right end of the row.
    pub fn action(mut self, label: impl Into<SharedString>, style: BannerActionStyle) -> Self {
        self.action = Some((label.into(), style));
        self
    }

    /// The button was pressed.
    pub fn on_action(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    /// A second xs button, drawn to the **left** of [`Banner::action`].
    ///
    /// One button is the design card, and it stays the default. A banner that
    /// stands between the person and something they were about to do — the
    /// harness's pay-as-you-go guard, which offers "Sign out" beside "Send
    /// anyway" — needs the way out and the way through on the same row, because
    /// a person who is only offered the way through will take it.
    pub fn secondary_action(mut self, label: impl Into<SharedString>, style: BannerActionStyle) -> Self {
        self.secondary = Some((label.into(), style));
        self
    }

    /// The second button was pressed.
    pub fn on_secondary(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_secondary = Some(Rc::new(f));
        self
    }
}

/// The message.
///
/// A line of plain and bold runs is one [`StyledText`], so it wraps like the
/// CSS does. A line that also carries a mono run has to be a row of sibling
/// divs instead, because [`HighlightStyle`] cannot change the font family;
/// such a line does not wrap (no banner in the design needs it to).
fn message(runs: &[BannerRun], p: &Palette) -> gpui::AnyElement {
    if runs.iter().any(|r| matches!(r, BannerRun::Mono(_))) {
        let mut row = h_flex().flex_1().min_w(px(0.0)).items_baseline();
        for run in runs {
            let text = run.text().clone();
            row = row.child(match run {
                BannerRun::Bold(_) => div().semibold().child(text),
                BannerRun::Mono(_) => div().font_family(scale::FONT_MONO).child(text),
                BannerRun::Text(_) => div().child(text),
            });
        }
        return row.into_any_element();
    }

    let mut text = String::new();
    let mut highlights = Vec::new();
    for run in runs {
        let start = text.len();
        text.push_str(run.text());
        if matches!(run, BannerRun::Bold(_)) {
            highlights.push((start..text.len(), HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), color: Some(p.ink), ..Default::default() }));
        }
    }
    div().flex_1().min_w(px(0.0)).child(StyledText::new(text).with_highlights(highlights)).into_any_element()
}

/// The banner's button, in the requested style.
fn action_button(id: ElementId, label: SharedString, style: BannerActionStyle) -> Button {
    let b = button(id, label).size(ButtonSize::Xs);
    match style {
        BannerActionStyle::Primary => b.primary(),
        BannerActionStyle::Secondary => b,
        BannerActionStyle::Ghost => b.ghost(),
    }
}

impl RenderOnce for Banner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let (ground, border) = self.kind.surface(&p);
        let id = self.id.clone();

        let leading = match self.kind.glyph(&p) {
            Some((glyph, ink)) => div().flex_none().child(icon(glyph).size(px(BANNER_GLYPH)).color(ink)),
            None => div()
                .flex_none()
                .mx(px(BANNER_DOT_MARGIN))
                .child(status_dot((id.clone(), "dot"), AgentState::Done).size(px(BANNER_DOT))),
        };

        let mut row = h_flex()
            .w_full()
            .items_center()
            .gap(px(BANNER_GAP))
            .py(px(BANNER_PAD_Y))
            .pl(px(BANNER_PAD_LEFT))
            .pr(px(BANNER_PAD_RIGHT))
            .rounded(px(BANNER_RADIUS))
            .border_1()
            .border_color(border)
            .bg(ground)
            .ui(BANNER_TEXT)
            .text_color(p.ink)
            .child(leading)
            .child(message(&self.runs, &p));
        if let Some((label, style)) = self.secondary {
            let mut b = action_button((id.clone(), "secondary").into(), label, style);
            if let Some(h) = self.on_secondary.clone() {
                b = b.on_click(move |_, w, cx| h(w, cx));
            }
            row = row.child(div().flex_none().child(b));
        }
        if let Some((label, style)) = self.action {
            let mut b = action_button((id, "action").into(), label, style);
            if let Some(h) = self.on_action.clone() {
                b = b.on_click(move |_, w, cx| h(w, cx));
            }
            row = row.child(div().flex_none().child(b));
        }
        row
    }
}
