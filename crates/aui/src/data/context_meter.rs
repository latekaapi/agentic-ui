//! `.ctx`: the context-window meter that sits in the composer toolbar — a
//! stroked ring, the percentage, and a hover breakdown with a **Compact**
//! action.
//!
//! MSP reports context as `(windowTokens, usedTokens, pressure)` and omits
//! `windowTokens` entirely when the basis has no limit, so the meter has to
//! render "used, no denominator" as a first-class state rather than inventing a
//! window. Pressure — not the fraction — picks the colour, because the server
//! owns the thresholds ("hard threshold first, both inclusive `>=`") and a
//! client that re-derived them would disagree with the composer that the same
//! server has already blocked.
//!
//! Stateless like every other component here: the state comes in, the
//! **Compact** intent goes out.

use aui_motion::{presence, tint_fade, EnterExit, PresenceStyle, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette, TextRole};
use gpui::{canvas, div, prelude::*, px, App, ElementId, IntoElement, PathBuilder, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, ButtonSize};
use crate::overlay::popover_layer;
use crate::util::interaction_flags;

/// `.ctx i{width:14px;height:14px}` — the ring's box.
const RING: f32 = 14.0;
/// `stroke-width:2px` on a 14 px ring.
const RING_STROKE: f32 = 2.0;
/// How many straight segments approximate the arc. gpui has no arc primitive
/// in the stroke tessellator's path that reads well at this size, so the ring
/// is a sampled polyline; 48 segments is under a third of a pixel of chord
/// error at 14 px.
const RING_SEGMENTS: usize = 48;
/// `.ctx{gap:6px}`.
const GAP: f32 = 6.0;
/// The percentage: mono 11, medium.
const LABEL: f32 = scale::FS_11;

/// `.ctx .pop{bottom:26px;width:240px;padding:10px}`.
const POP_BOTTOM: f32 = 26.0;
const POP_W: f32 = 240.0;
const POP_PAD: f32 = 10.0;
const POP_SHADOW: u8 = 3;
/// `@keyframes in{from{opacity:0;transform:translateY(4px)}}`.
const POP_RISE: f32 = 4.0;
/// `.ctx .pop .r{height:20px}` — one name/value row of the breakdown.
const POP_ROW_H: f32 = 20.0;
/// `.ctx .pop .hd{padding-bottom:6px}`.
const POP_HEAD_GAP: f32 = 6.0;

/// How much context pressure the server reports.
///
/// Exactly MSP's `ContextPressureLevel` (`msp.d.ts:265`). The thresholds are
/// the server's; a client renders what it is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextPressure {
    /// Room to spare.
    #[default]
    Normal,
    /// Close to the limit; the ring and the number go warning-coloured.
    Warning,
    /// Over the limit: the composer refuses to send and the meter offers
    /// compaction inline rather than only on hover.
    Blocked,
}

/// Everything the meter draws.
///
/// `window_tokens` is `None` when the basis has no limit — the meter then shows
/// an empty ring and `N tokens` instead of a percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContextMeterState {
    /// `usedTokens` from `session/contextUsage`.
    pub used_tokens: u64,
    /// `windowTokens`, absent when the basis has no limit.
    pub window_tokens: Option<u64>,
    /// The server's pressure level.
    pub pressure: ContextPressure,
    /// Session cumulative prompt tokens, for the breakdown.
    pub prompt_tokens: u64,
    /// Session cumulative output tokens, for the breakdown.
    pub output_tokens: u64,
    /// Session cumulative total tokens, for the breakdown.
    pub total_tokens: u64,
}

impl ContextMeterState {
    /// Occupancy in `0..=1`, or `None` when there is no denominator.
    pub fn fraction(&self) -> Option<f32> {
        match self.window_tokens {
            Some(window) if window > 0 => Some((self.used_tokens as f32 / window as f32).clamp(0.0, 1.0)),
            _ => None,
        }
    }

    /// The label beside the ring: `62%`, or `19.3k tokens` with no window.
    pub fn label(&self) -> String {
        match self.fraction() {
            Some(fraction) => format!("{}%", (fraction * 100.0).round()),
            None => format!("{} tokens", thousands(self.used_tokens)),
        }
    }
}

type CompactHandler = std::rc::Rc<dyn Fn(&mut Window, &mut App)>;

/// The context meter. Build with [`context_meter`].
#[derive(IntoElement)]
pub struct ContextMeter {
    id: ElementId,
    state: ContextMeterState,
    open: Option<bool>,
    on_compact: Option<CompactHandler>,
}

/// The meter for `state`; hover reveals the breakdown.
pub fn context_meter(id: impl Into<ElementId>, state: ContextMeterState) -> ContextMeter {
    ContextMeter { id: id.into(), state, open: None, on_compact: None }
}

impl ContextMeter {
    /// Forces the breakdown open (or shut) instead of following the pointer —
    /// what a static capture and the keyboard both need.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// The **Compact** action, in the breakdown and — when the pressure is
    /// [`ContextPressure::Blocked`] — inline beside the number.
    pub fn on_compact(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_compact = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for ContextMeter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let ink = pressure_color(&p, self.state.pressure);
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let open = self.open.unwrap_or(flags.hovered);
        let blocked = self.state.pressure == ContextPressure::Blocked;

        let text = tint_fade((id.clone(), "text"), true, ink, Tween::FAST, window, cx);
        let mut row = h_flex()
            .id(id.clone())
            .relative()
            .flex_none()
            .gap(px(GAP))
            .text_role(TextRole::MonoSmall)
            .text_px(LABEL)
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(text)
            .whitespace_nowrap()
            .child(ring(self.state.fraction(), p.surface_3, ink))
            .child(self.state.label());

        // The pointer opens the breakdown; the caller can pin it open instead.
        if self.open.is_none() {
            let hovered = state.clone();
            row = row.on_hover(move |now, _, cx| {
                hovered.update(cx, |s, cx| {
                    if s.hovered != *now {
                        s.hovered = *now;
                        cx.notify();
                    }
                });
            });
        }

        if blocked {
            if let Some(handler) = self.on_compact.clone() {
                row = row.child(
                    button((id.clone(), "compact-inline"), "Compact")
                        .size(ButtonSize::Xs)
                        .on_click(move |_, w, cx| handler(w, cx)),
                );
            }
        }

        if open {
            row = row.child(breakdown(&id, &p, &self.state, self.on_compact.clone(), self.open.is_some(), window, cx));
        }
        row
    }
}

/// The colour the ring and the number take at each pressure level. Status
/// colour carries meaning here and nowhere else on the meter.
fn pressure_color(p: &Palette, pressure: ContextPressure) -> gpui::Hsla {
    match pressure {
        ContextPressure::Normal => p.ink_3,
        ContextPressure::Warning => p.warning,
        ContextPressure::Blocked => p.danger,
    }
}

/// The ring: a full-circle track and, when there is a denominator, the used arc
/// drawn clockwise from twelve o'clock.
fn ring(fraction: Option<f32>, track: gpui::Hsla, ink: gpui::Hsla) -> impl IntoElement {
    div().flex_none().size(px(RING)).child(canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let centre = bounds.center();
            let radius = (f32::from(bounds.size.width).min(f32::from(bounds.size.height)) - RING_STROKE) / 2.0;
            paint_arc(window, centre, radius, 1.0, track);
            if let Some(fraction) = fraction.filter(|f| *f > 0.0) {
                paint_arc(window, centre, radius, fraction, ink);
            }
        },
    )
    .size_full())
}

/// One stroked arc from twelve o'clock, clockwise, covering `fraction` of the
/// circle.
fn paint_arc(window: &mut Window, centre: gpui::Point<Pixels>, radius: f32, fraction: f32, color: gpui::Hsla) {
    if radius <= 0.0 {
        return;
    }
    let steps = ((RING_SEGMENTS as f32 * fraction).ceil() as usize).max(2);
    let sweep = std::f32::consts::TAU * fraction;
    let mut path = PathBuilder::stroke(px(RING_STROKE));
    for step in 0..=steps {
        let angle = -std::f32::consts::FRAC_PI_2 + sweep * (step as f32 / steps as f32);
        let point = gpui::point(px(f32::from(centre.x) + radius * angle.cos()), px(f32::from(centre.y) + radius * angle.sin()));
        if step == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// The hover card: what the window is made of, and the one action that changes
/// it.
fn breakdown(
    id: &ElementId,
    p: &Palette,
    state: &ContextMeterState,
    on_compact: Option<CompactHandler>,
    at_rest: bool,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let timing = if at_rest { EnterExit { enter: std::time::Duration::ZERO, ..EnterExit::DEFAULT } } else { EnterExit::DEFAULT };
    let sample = presence((id.clone(), "pop"), true, timing, window, cx);
    let style = PresenceStyle::fade_rise(sample, POP_RISE);
    let mut pop = v_flex()
        .absolute()
        // The card is anchored to the meter's top edge, so the enter's 4 px
        // rise is a smaller `bottom`, never a `top` as well: setting both would
        // stretch the box between them instead of moving it.
        .bottom(px(POP_BOTTOM) - style.offset_y)
        .left(px(0.0))
        .w(px(POP_W))
        .p(px(POP_PAD))
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line_strong)
        .bg(p.overlay)
        .shadow(p.shadow(POP_SHADOW))
        .opacity(style.opacity)
        .child(
            div()
                .pb(px(POP_HEAD_GAP))
                .text_role(TextRole::Caps)
                .text_color(p.ink_3)
                .child(SharedString::from("Context window")),
        );

    let window_label = match state.window_tokens {
        Some(window) => thousands(window),
        None => "no limit".to_owned(),
    };
    for (name, value) in [
        ("Used", thousands(state.used_tokens)),
        ("Window", window_label),
        ("Prompt (session)", thousands(state.prompt_tokens)),
        ("Output (session)", thousands(state.output_tokens)),
        ("Total (session)", thousands(state.total_tokens)),
    ] {
        pop = pop.child(
            h_flex()
                .w_full()
                .h(px(POP_ROW_H))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child(div().flex_1().child(name))
                .child(div().flex_none().mono(scale::FS_11).text_color(p.ink_2).child(value)),
        );
    }
    if let Some(handler) = on_compact {
        pop = pop.child(
            div().pt(px(POP_HEAD_GAP)).child(
                button((id.clone(), "compact"), "Compact now")
                    .size(ButtonSize::Xs)
                    .on_click(move |_, w, cx| handler(w, cx)),
            ),
        );
    }
    popover_layer(pop)
}

/// `19,328` — the breakdown counts tokens exactly; the ring is the rounded view.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_of_zero_is_no_denominator() {
        let state = ContextMeterState { used_tokens: 10, window_tokens: Some(0), ..Default::default() };
        assert_eq!(state.fraction(), None);
        assert_eq!(state.label(), "10 tokens");
    }

    #[test]
    fn a_percentage_needs_a_window() {
        let state = ContextMeterState { used_tokens: 500, window_tokens: Some(1000), ..Default::default() };
        assert_eq!(state.label(), "50%");
    }

    #[test]
    fn used_can_exceed_the_window_without_the_ring_overrunning() {
        let state = ContextMeterState { used_tokens: 2000, window_tokens: Some(1000), ..Default::default() };
        assert_eq!(state.fraction(), Some(1.0));
    }

    #[test]
    fn counts_are_grouped() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_007_997), "1,007,997");
    }
}
