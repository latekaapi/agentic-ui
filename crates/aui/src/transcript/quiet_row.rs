//! The quiet row (D4, docs/41 §2.4): a transcript line that only reports —
//! "Worked for…", "Thought for…", a resolved approval or answer, a loaded
//! skill, the live activity line. It carries no surface: 32 px, padded 8 at
//! the sides and bleeding 8 px outside the measure so its text sits on the
//! measure's edge, radius 6, gap 8, body text muted. A row that toggles
//! wears a 12 px chevron on the **left** and, on hover, the 5 % overlay with
//! its text in the foreground; the whole row is the toggle. A row that
//! cannot toggle (a resolved approval) leads with its status glyph instead
//! and never reacts to the pointer.

use aui_motion::{collapse_fluid, collapse_fluid_opacity, tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, Easing};
use gpui::{div, prelude::*, px, radians, AnyElement, App, ElementId, Hsla, IntoElement, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::record_ax_label;
use crate::icons::{icon, IconName};
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// Row height.
pub const QUIET_ROW_H: f32 = 32.0;
/// Side padding, and how far the row bleeds past the measure on each side.
pub const QUIET_ROW_BLEED: f32 = 8.0;
/// Gap between the row's parts.
const GAP: f32 = 8.0;
/// Radius of the hover overlay.
const RADIUS: f32 = 6.0;
/// The leading chevron.
const CHEVRON: f32 = 12.0;
/// The hover overlay and text colour change: 80 ms.
const HOVER: Tween = Tween::new(std::time::Duration::from_millis(80), Easing::OUT);

/// What leads a quiet row.
pub enum QuietLead {
    /// A toggling row: the chevron, turned a quarter while open.
    Chevron,
    /// A row that reports a state: its glyph (a check, a spinner).
    Glyph(AnyElement),
}

/// One quiet row. Build with [`quiet_row`].
#[derive(IntoElement)]
pub struct QuietRow {
    id: ElementId,
    lead: QuietLead,
    open: bool,
    parts: Vec<AnyElement>,
    body: Option<AnyElement>,
    label: Option<gpui::SharedString>,
    on_toggle: Option<ClickHandler>,
}

/// A quiet row that toggles: chevron first, then the parts added with
/// [`QuietRow::part`]. `open` turns the chevron and shows the body.
pub fn quiet_row(id: impl Into<ElementId>, open: bool) -> QuietRow {
    QuietRow { id: id.into(), lead: QuietLead::Chevron, open, parts: Vec::new(), body: None, label: None, on_toggle: None }
}

/// A quiet row that reports and never toggles: `glyph` first (a check, a
/// spinner), then the parts.
pub fn quiet_status_row(id: impl Into<ElementId>, glyph: impl IntoElement) -> QuietRow {
    QuietRow { id: id.into(), lead: QuietLead::Glyph(glyph.into_any_element()), open: false, parts: Vec::new(), body: None, label: None, on_toggle: None }
}

impl QuietRow {
    /// Appends a part (a glyph, the title, a mono tail).
    pub fn part(mut self, el: impl IntoElement) -> Self {
        self.parts.push(el.into_any_element());
        self
    }

    /// What the row shows while open, under it (the thinking trace). A row
    /// whose open content the host lays out as its own rows (the turn fold)
    /// has no body.
    pub fn body(mut self, el: impl IntoElement) -> Self {
        self.body = Some(el.into_any_element());
        self
    }

    /// Shows the body of a status row (whose lead is not a chevron): the
    /// live row opens to its run's cards.
    pub fn open_body(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The accessible name of the toggle; defaults to "Expand section" /
    /// "Collapse section".
    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The row's click (and Space / Enter while focused).
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for QuietRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let row_id: ElementId = (id.clone(), "row").into();
        let toggles = self.on_toggle.is_some();
        let (state, flags) = interaction_flags(row_id.clone(), window, cx);
        let lit = toggles && flags.hovered;
        let overlay = tint_fade((row_id.clone(), "bg"), lit, p.hover_overlay(), HOVER, window, cx);
        let ink: Hsla = tween((row_id.clone(), "ink"), if lit { p.ink } else { p.ink_2 }, HOVER, window, cx);

        let fade = collapse_fluid_opacity((id.clone(), "body"), self.open, window, cx);
        let (reveal, progress) = match self.body {
            Some(body) => {
                let body = div().w_full().opacity(fade).child(body).into_any_element();
                let (reveal, progress) = collapse_fluid((id.clone(), "body"), self.open, body, window, cx);
                (Some(reveal), progress)
            }
            None => {
                // No body to measure: the chevron still turns on the fold's
                // own curve, keyed like a body would be.
                let (_, progress) = collapse_fluid((id.clone(), "body"), self.open, div().into_any_element(), window, cx);
                (None, progress)
            }
        };

        let lead = match self.lead {
            QuietLead::Chevron => div()
                .flex_none()
                .flex()
                .items_center()
                .child(icon(IconName::Chev).size(px(CHEVRON)).color(ink).rotate(radians(progress * std::f32::consts::FRAC_PI_2)))
                .into_any_element(),
            QuietLead::Glyph(glyph) => div().flex_none().flex().items_center().child(glyph).into_any_element(),
        };
        let mut row = h_flex()
            .id(row_id)
            .h(px(QUIET_ROW_H))
            .mx(px(-QUIET_ROW_BLEED))
            .px(px(QUIET_ROW_BLEED))
            .gap(px(GAP))
            .items_center()
            .rounded(px(RADIUS))
            .bg(overlay)
            .ui(scale::FS_13)
            .text_color(ink)
            .whitespace_nowrap()
            .overflow_hidden()
            .child(lead)
            .children(self.parts);
        if let Some(on_toggle) = self.on_toggle {
            let label = self.label.unwrap_or_else(|| if self.open { "Collapse section".into() } else { "Expand section".into() });
            record_ax_label(label.as_ref());
            row = row
                .cursor_pointer()
                .track_interaction(&state)
                .role(gpui::Role::Button)
                .aria_label(label)
                .on_click(move |e, w, cx| on_toggle(e, w, cx));
        }
        let mut col = v_flex().w_full().child(row);
        if let Some(reveal) = reveal {
            col = col.child(reveal);
        }
        col
    }
}

/// The D4 status check: a bare check in `color` (success), 1.8 stroke
/// (the bold twin of the 1.5 sprite), 14 px — no soft ground.
pub fn status_check(color: Hsla) -> impl IntoElement {
    icon(IconName::Check).size(px(STATUS_GLYPH)).color(color).lit(true)
}

/// The D4 status cross: a bare ✕ in danger, 14 px.
pub fn status_cross(color: Hsla) -> impl IntoElement {
    icon(IconName::X).size(px(STATUS_GLYPH)).color(color).lit(true)
}

/// The status glyph size in card headers and quiet rows.
pub const STATUS_GLYPH: f32 = 14.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_row_has_no_toggle_and_a_fold_row_starts_closed() {
        let row = quiet_status_row("r", div());
        assert!(row.on_toggle.is_none());
        assert!(matches!(row.lead, QuietLead::Glyph(_)));
        let fold = quiet_row("f", false);
        assert!(!fold.open);
        assert!(matches!(fold.lead, QuietLead::Chevron));
    }

    #[test]
    fn the_row_bleeds_its_own_padding() {
        // The text must land on the measure's edge: the bleed and the
        // padding are the same number.
        assert_eq!(QUIET_ROW_BLEED, 8.0);
        assert_eq!(QUIET_ROW_H, 32.0);
    }
}
