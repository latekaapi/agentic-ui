//! The three controls the D4 cards use (docs/41 §2.6–2.7): a **text**
//! control (28 px, radius 6, muted → foreground on hover), a **secondary**
//! one (Raised-lit with the edge) and the one **primary** (the foreground
//! fill, frame-coloured text, 600). And the faint mono key hints that open a
//! card's footer.

use aui_tokens::{scale, AuiStyled, Palette};
use gpui::{div, prelude::*, px, ElementId, SharedString};
use gpui_kit::base::h_flex;

use crate::data::record_ax_label;
use crate::icons::{icon, IconName};

/// Control height and radius.
pub const CONTROL_H: f32 = 28.0;
const CONTROL_RADIUS: f32 = 6.0;
const CONTROL_PAD_X: f32 = 10.0;

/// Which of the three a control is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    /// Text only: muted, the foreground and the 5 % overlay on hover.
    Text,
    /// Raised-lit with the one edge.
    Secondary,
    /// The foreground fill, frame-coloured text, 600.
    Primary,
}

/// One control labelled `label` (its accessible name too). The caller adds
/// `on_click`.
pub fn control(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: ControlKind, p: &Palette) -> gpui::Stateful<gpui::Div> {
    control_with(id, None, label, kind, p)
}

/// [`control`] with a leading 12 px glyph (Back's arrow).
pub fn control_with(id: impl Into<ElementId>, glyph: Option<IconName>, label: impl Into<SharedString>, kind: ControlKind, p: &Palette) -> gpui::Stateful<gpui::Div> {
    let label: SharedString = label.into();
    record_ax_label(label.as_ref());
    let base = h_flex()
        .id(id.into())
        .flex_none()
        .h(px(CONTROL_H))
        .px(px(CONTROL_PAD_X))
        .gap(px(6.0))
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .ui(scale::FS_13)
        .whitespace_nowrap()
        .cursor_pointer()
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .children(glyph.map(|g| icon(g).size(px(12.0))));
    let (fg, hover) = (p.ink, p.hover_overlay());
    match kind {
        ControlKind::Text => base.text_color(p.ink_2).hover(move |d| d.text_color(fg).bg(hover)).child(label),
        ControlKind::Secondary => {
            let lit = crate::util::raised_lit(p, p.surface_2);
            base.bg(lit).shadow(p.edge()).text_color(p.ink).hover(move |d| d.bg(lit.blend(hover))).child(label)
        }
        ControlKind::Primary => {
            let (fill, text) = (p.ink, p.bg);
            base.bg(fill).text_color(text).font_weight(gpui::FontWeight::SEMIBOLD).hover(move |d| d.bg(fill.opacity(0.9))).child(label)
        }
    }
}

/// `⏎ allow · ⎋ deny`: a footer's key hints in faint mono.
pub fn key_hints(text: impl Into<SharedString>, p: &Palette) -> gpui::Div {
    div().flex_none().mono(scale::FS_11).text_color(p.ink_4).whitespace_nowrap().child(text.into())
}
