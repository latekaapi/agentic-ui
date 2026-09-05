//! Card 13: the toast stack.
//!
//! A toast is a small overlay card — icon tile, title, body, optional actions
//! and an auto-dismiss hairline. Several of them form a [`ToastStack`]: the
//! newest is in front at full size, older ones tuck behind it, lifted,
//! scaled down and faded. Hovering the stack fans them out on the layout
//! spring so every toast is readable at once.
//!
//! gpui has no element transform, so the CSS `scale()` of the tucked toasts is
//! expressed as a relative width with a matching left inset (the CSS
//! `transform-origin: top center`), and `translateY()` as a `top` offset.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, spring_phase, EnterExit, PresenceStyle, SpringKind};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{div, prelude::*, px, relative, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, ButtonSize};
use crate::util::{interaction_flags, TrackInteraction};

/// `.toast{border-radius:var(--r-lg);padding:10px 12px;gap:2px 10px}`.
const TOAST_RADIUS: f32 = scale::R_LG;
const TOAST_PAD_X: f32 = 12.0;
const TOAST_PAD_Y: f32 = 10.0;
/// The grid's column gap (icon → text) and row gap (title → body).
const COLUMN_GAP: f32 = 10.0;
const ROW_GAP: f32 = 2.0;
/// `.toast .ic{width:22px;height:22px;border-radius:6px}`.
const ICON_TILE: f32 = 22.0;
const ICON_TILE_RADIUS: f32 = scale::R_SM;
/// The glyph inside the tile is the standard 14 px `.i`.
const ICON_GLYPH: f32 = 14.0;
/// `.toast .x{margin-top:2px}` — the 14 px close glyph in the third column.
const CLOSE_GLYPH: f32 = 14.0;
const CLOSE_TOP: f32 = 2.0;
/// `.toast .acts{gap:6px;margin-top:8px}`, on top of the grid's 2 px row gap.
const ACTIONS_GAP: f32 = 6.0;
const ACTIONS_TOP: f32 = ROW_GAP + 8.0;
/// `.toast .prog{left:12px;right:12px;bottom:0;height:2px;border-radius:1px}`.
const PROGRESS_INSET: f32 = 12.0;
const PROGRESS_HEIGHT: f32 = 2.0;
const PROGRESS_RADIUS: f32 = 1.0;
/// `@keyframes tin{from{opacity:0;transform:translateY(8px) scale(.97)}}`.
const ENTER_RISE: f32 = 8.0;
const ENTER_SCALE: f32 = 0.97;
/// `.toast.t2/.t3`: each step back lifts the toast 10 px, shrinks it 4 % and
/// fades it 30 % (1 → .96 → .92 and 1 → .7 → .4).
const TUCK_RISE: f32 = 10.0;
const TUCK_SCALE_STEP: f32 = 0.04;
const TUCK_OPACITY_STEP: f32 = 0.3;
/// Fanned out, the toasts sit in a list with the standard 8 px rhythm.
const FAN_GAP: f32 = scale::SP_3;

/// The icon tile of a toast: neutral, success or warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastKind {
    /// `.ic`: surface-3 ground, ink-2, no glyph.
    #[default]
    Neutral,
    /// `.ic.ok`: success-soft ground, success check.
    Ok,
    /// `.ic.warn`: warning-soft ground, warning shield.
    Warn,
}

impl ToastKind {
    /// `(ground, ink, glyph)` for the 22 px tile.
    fn tile(self, p: &Palette) -> (Hsla, Hsla, Option<IconName>) {
        match self {
            ToastKind::Neutral => (p.surface_3, p.ink_2, None),
            ToastKind::Ok => (p.success_soft, p.success, Some(IconName::Check)),
            ToastKind::Warn => (p.warning_soft, p.warning, Some(IconName::Shield)),
        }
    }
}

/// One xs button in a toast's action row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastAction {
    /// Identifies the action in the `on_action` callback.
    pub id: SharedString,
    /// The button label.
    pub label: SharedString,
    /// Drawn as the accent-filled primary button instead of the secondary one.
    pub primary: bool,
}

impl ToastAction {
    /// A secondary action.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), primary: false }
    }

    /// Marks the action as the primary (accent) one.
    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }
}

/// Everything a toast draws. The app owns these; the component is stateless.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastData {
    /// Stable identity, used to key the enter/exit of this toast.
    pub id: SharedString,
    /// Which icon tile to draw.
    pub kind: ToastKind,
    /// Title, 13 px / 600.
    pub title: SharedString,
    /// Body line, 12 px ink-2.
    pub body: SharedString,
    /// Optional xs action buttons, 8 px below the body.
    pub actions: Vec<ToastAction>,
    /// The auto-dismiss hairline, `0..=1`; `None` hides the bar (a toast that
    /// waits for the person rather than for the clock).
    pub progress: Option<f32>,
}

impl ToastData {
    /// A toast with a title and a body line.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>, body: impl Into<SharedString>) -> Self {
        Self { id: id.into(), kind: ToastKind::Neutral, title: title.into(), body: body.into(), actions: Vec::new(), progress: None }
    }

    /// Sets the icon tile.
    pub fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = kind;
        self
    }

    /// Adds an action button.
    pub fn action(mut self, action: ToastAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Shows the auto-dismiss hairline at `progress` (`0..=1`).
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = Some(progress);
        self
    }

    /// The toast's resting height, derived from its CSS box: 10 px padding top
    /// and bottom, the 13/1.3 title, the 2 px row gap, the 12/1.5 body, the
    /// 1 px borders and — when it carries actions — 10 px plus the 20 px xs
    /// control. [`ToastStack`] needs it to know how far to fan the stack out;
    /// gpui cannot measure a sibling during layout.
    pub fn height(&self, text_scale: f32) -> f32 {
        let title = scale::FS_13 * text_scale * scale::LH_TIGHT;
        let body = scale::FS_12 * text_scale * scale::LH_UI;
        let actions = if self.actions.is_empty() { 0.0 } else { ACTIONS_TOP + scale::H_XS };
        TOAST_PAD_Y * 2.0 + 2.0 + title + ROW_GAP + body + actions
    }
}

/// Called with the id of the action that was pressed.
type ActionHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;
/// Called when the close glyph is pressed.
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A single toast. Build with [`toast`].
#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    data: ToastData,
    present: bool,
    timing: EnterExit,
    close_opacity: f32,
    on_action: Option<ActionHandler>,
    on_close: Option<CloseHandler>,
}

/// A toast card that fills the width it is given.
pub fn toast(id: impl Into<ElementId>, data: &ToastData) -> Toast {
    Toast { id: id.into(), data: data.clone(), present: true, timing: EnterExit::DEFAULT, close_opacity: 1.0, on_action: None, on_close: None }
}

impl Toast {
    /// Whether the toast is on screen; `false` plays the 160 ms fade out.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the toast is drawn at rest on its first frame, for a
    /// static composition (the design card, a restored stack) rather than one
    /// that just arrived.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// An action button was pressed; the argument is [`ToastAction::id`].
    pub fn on_action(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    /// The close glyph was pressed.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }

    /// Shares the callbacks of a [`ToastStack`] with every toast it draws.
    fn with_handlers(mut self, action: Option<ActionHandler>, close: Option<CloseHandler>) -> Self {
        self.on_action = action;
        self.on_close = close;
        self
    }

    /// Fades the close glyph. A tucked toast in a stack hides it (the card's
    /// `.t2` / `.t3` carry an empty third grid cell) and fades it in as the
    /// stack fans out and the toast becomes reachable.
    fn close_opacity(mut self, opacity: f32) -> Self {
        self.close_opacity = opacity;
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let enter = PresenceStyle::fade_rise_scale(sample, ENTER_RISE, ENTER_SCALE);
        let (tile_bg, tile_ink, glyph) = self.data.kind.tile(&p);

        let mut text = v_flex()
            .flex_1()
            .min_w(px(0.0))
            .child(div().text_role(TextRole::Title).text_color(p.ink).child(self.data.title.clone()))
            .child(div().mt(px(ROW_GAP)).ui(scale::FS_12).text_color(p.ink_2).child(self.data.body.clone()));
        if !self.data.actions.is_empty() {
            let mut row = h_flex().mt(px(ACTIONS_TOP)).gap(px(ACTIONS_GAP));
            for (i, action) in self.data.actions.iter().enumerate() {
                let key: ElementId = (id.clone(), SharedString::from(format!("action-{i}"))).into();
                // `.acts` in the card: the leading action is the secondary
                // button, everything after it is ghost, unless it asked to be
                // the primary one.
                let mut b = button(key, action.label.clone()).size(ButtonSize::Xs);
                b = match (action.primary, i) {
                    (true, _) => b.primary(),
                    (false, 0) => b,
                    (false, _) => b.ghost(),
                };
                if let Some(h) = self.on_action.clone() {
                    let action_id = action.id.clone();
                    b = b.on_click(move |_, w, cx| h(action_id.as_ref(), w, cx));
                }
                row = row.child(b);
            }
            text = text.child(row);
        }

        let mut close = div()
            .id((id.clone(), "close"))
            .flex_none()
            .mt(px(CLOSE_TOP))
            .opacity(self.close_opacity)
            .child(icon(IconName::X).size(px(CLOSE_GLYPH)).color(p.ink_4));
        if let Some(h) = self.on_close.clone() {
            close = close.cursor_pointer().on_click(move |_, w, cx| h(w, cx));
        }

        let mut card = div()
            .relative()
            .top(enter.offset_y)
            .w(relative(enter.scale))
            .ml(relative((1.0 - enter.scale) / 2.0))
            .opacity(enter.opacity)
            .px(px(TOAST_PAD_X))
            .py(px(TOAST_PAD_Y))
            .rounded(px(TOAST_RADIUS))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(2))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(COLUMN_GAP))
                    .child(
                        h_flex()
                            .flex_none()
                            .size(px(ICON_TILE))
                            .items_center()
                            .justify_center()
                            .rounded(px(ICON_TILE_RADIUS))
                            .bg(tile_bg)
                            .when_some(glyph, |el, g| el.child(icon(g).size(px(ICON_GLYPH)).color(tile_ink))),
                    )
                    .child(text)
                    .child(close),
            );
        if let Some(progress) = self.data.progress {
            card = card.child(
                div()
                    .absolute()
                    .left(px(PROGRESS_INSET))
                    .right(px(PROGRESS_INSET))
                    .bottom(px(0.0))
                    .h(px(PROGRESS_HEIGHT))
                    .rounded(px(PROGRESS_RADIUS))
                    .bg(p.surface_3)
                    .child(div().h_full().w(relative(progress.clamp(0.0, 1.0))).rounded(px(PROGRESS_RADIUS)).bg(p.ink_3)),
            );
        }
        card
    }
}

/// The stack of toasts. Build with [`toast_stack`].
#[derive(IntoElement)]
pub struct ToastStack {
    id: ElementId,
    toasts: Vec<ToastData>,
    hovered: Option<bool>,
    at_rest: bool,
    on_action: Option<ActionHandler>,
    on_close: Option<CloseHandler>,
}

/// The toast stack: `toasts` oldest first, so the last one is the newest and
/// is drawn in front at full size.
pub fn toast_stack(id: impl Into<ElementId>, toasts: Vec<ToastData>) -> ToastStack {
    ToastStack { id: id.into(), toasts, hovered: None, at_rest: false, on_action: None, on_close: None }
}

impl ToastStack {
    /// Forces the fanned-out state on or off instead of following the pointer
    /// (`None`, the default). A gallery or a test can hold the stack open.
    pub fn hovered(mut self, hovered: Option<bool>) -> Self {
        self.hovered = hovered;
        self
    }

    /// Draws every toast at rest on the first frame, with no enter.
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// An action button was pressed on one of the toasts; the argument is the
    /// [`ToastAction::id`].
    pub fn on_action(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    /// A close glyph was pressed.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ToastStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let text_scale = cx.aui().text_scale;
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let hovered = self.hovered.unwrap_or(flags.hovered);
        // The fan travels on the layout spring, like every other size change.
        let fan = spring_phase((id.clone(), "fan"), hovered, SpringKind::Layout, window, cx);

        // Depth 0 is the newest toast, in front; the fanned position of a
        // deeper toast is the sum of the heights of the toasts in front of it,
        // so the fanned tops are accumulated front to back.
        let count = self.toasts.len();
        let mut fanned = vec![0.0f32; count];
        let mut offset = 0.0;
        for index in (0..count).rev() {
            fanned[index] = offset;
            offset += self.toasts[index].height(text_scale) + FAN_GAP;
        }

        // Painted oldest first, so the newest toast lands on top.
        let mut stack = div().id(id.clone()).relative().size_full().track_interaction(&state);
        for (index, data) in self.toasts.iter().enumerate() {
            let depth = count - 1 - index;
            let rest_top = -TUCK_RISE * depth as f32;
            let rest_scale = 1.0 - TUCK_SCALE_STEP * depth as f32;
            let rest_opacity = 1.0 - TUCK_OPACITY_STEP * depth as f32;
            let top = rest_top + (fanned[index] - rest_top) * fan;
            let scale = rest_scale + (1.0 - rest_scale) * fan;
            let opacity = rest_opacity + (1.0 - rest_opacity) * fan;

            let key: ElementId = (id.clone(), data.id.clone()).into();
            let mut card = toast(key, data)
                .with_handlers(self.on_action.clone(), self.on_close.clone())
                .close_opacity(if depth == 0 { 1.0 } else { fan.clamp(0.0, 1.0) });
            if self.at_rest {
                card = card.at_rest();
            }
            stack = stack.child(
                div()
                    .absolute()
                    .top(px(top))
                    .left(relative((1.0 - scale) / 2.0))
                    .w(relative(scale))
                    .opacity(opacity)
                    .child(card),
            );
        }
        stack
    }
}
