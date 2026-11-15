//! The one hover / press / selected model (audit C-04, docs/83 §1 and §9).
//!
//! Anything you can point at paints its states the same way: gpui's own
//! `.hover()` / `.active()` style refinements carrying the palette's overlay
//! tints, composited over the element's own fill. No keyed state, no
//! per-frame tween, no gliding block: the refinement is instant, and gpui
//! re-renders only on a hover transition.
//!
//! - **rest**: the element's own fill ([`StatePaint::on`]), or nothing;
//! - **hover**: the rest fill with `ov-hover` over it;
//! - **pressed**: the rest fill with `ov-pressed` over it (stateful
//!   elements only — gpui's `.active()` needs an id);
//! - **selected**: `ov-selected` over the rest fill, held at rest; hover
//!   and press still tint on top of it;
//! - **highlighted**: the hover look held at rest — a keyboard highlight, or
//!   a trigger whose menu is open;
//! - **disabled**: no state paint at all.
//!
//! The tint over a filled element follows the fill, not the theme: a dark
//! fill lightens (white at the overlay alpha), a light fill darkens (black),
//! so a near-white primary button still answers the pointer in dark.
//!
//! Text colour can lift with the state ([`StatePaint::ink`]): children that
//! do not set their own colour — text and glyphs built without `.color()` —
//! inherit it, hovered or not.
//!
//! A list the keyboard also drives (menus, pickers) turns the pointer's
//! refinement off with [`StatePaint::pointer`]`(false)` and paints its one
//! lit row with [`StatePaint::highlighted`], so the pointer and the arrows
//! never light two rows at once.
//!
//! Gallery cards and captures pin a state with [`StatePaint::preview`]: the
//! element paints that state as its rest look, no pointer needed.
//!
//! Motion is for things that appear, disappear or collapse — never for hover.
//!
//! ```ignore
//! div().id("row").paint_states(StatePaint::new().selected(on), cx)          // a row
//! div().id("chip").paint_states(StatePaint::new().on(p.surface_4), cx)     // a filled control
//! div().paint_hover(StatePaint::new().ink(p.fg_muted, p.fg), cx)           // hover only, no id
//! ```

use aui_tokens::{ActiveAui, Palette};
use gpui::{App, Hsla, InteractiveElement, StatefulInteractiveElement, Styled};

use super::InteractState;

/// Where a paint reads the palette from: the app (`cx`), a window's
/// context, or a palette already in hand.
pub trait PaletteSource {
    /// The active palette.
    fn palette(&self) -> Palette;
}

impl PaletteSource for App {
    fn palette(&self) -> Palette {
        self.aui().colors
    }
}

impl<T> PaletteSource for gpui::Context<'_, T> {
    fn palette(&self) -> Palette {
        self.aui().colors
    }
}

impl PaletteSource for Palette {
    fn palette(&self) -> Palette {
        *self
    }
}

/// Which overlay a state paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    /// `ov-hover`.
    Hover,
    /// `ov-selected`.
    Selected,
    /// `ov-pressed`.
    Pressed,
}

/// `tint` over `base` (or over nothing), with the overlay's colour chosen
/// by the ground it lands on: white on a dark fill, black on a light one;
/// over nothing, the theme's own overlay. The one place the overlay
/// arithmetic lives.
pub fn tinted(base: Option<Hsla>, tint: Tint, p: &Palette) -> Hsla {
    let theme = match tint {
        Tint::Hover => p.hover_overlay(),
        Tint::Selected => p.selected_overlay(),
        Tint::Pressed => p.pressed_overlay(),
    };
    match base {
        None => theme,
        Some(base) if base.a <= 0.0 => theme,
        Some(base) => {
            // A light fill darkens, a dark fill lightens, at the theme's alpha.
            let overlay = if base.l > 0.6 { gpui::black().alpha(theme.a) } else { gpui::white().alpha(theme.a) };
            over(base, overlay)
        }
    }
}

/// Source-over compositing: `top` painted on `bottom`, alpha included (gpui's
/// `blend` keeps the bottom's alpha, which is wrong for a see-through fill).
pub fn over(bottom: Hsla, top: Hsla) -> Hsla {
    let (b, t): (gpui::Rgba, gpui::Rgba) = (bottom.into(), top.into());
    let a = t.a + b.a * (1.0 - t.a);
    if a <= 0.0 {
        return gpui::transparent_black();
    }
    let mix = |tc: f32, bc: f32| (tc * t.a + bc * b.a * (1.0 - t.a)) / a;
    gpui::Rgba { r: mix(t.r, b.r), g: mix(t.g, b.g), b: mix(t.b, b.b), a }.into()
}

/// What one element paints in each state. Build with [`StatePaint::new`],
/// apply with [`PaintStates::paint_states`] (hover + press) or
/// [`PaintHover::paint_hover`] (hover only, for elements without an id).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatePaint {
    base: Option<Hsla>,
    ink: Option<(Hsla, Hsla)>,
    edge: Option<(Hsla, Hsla)>,
    selected: bool,
    highlighted: bool,
    disabled: bool,
    pointer: bool,
    pressable: bool,
    tint: bool,
    underline: bool,
    preview: Option<InteractState>,
}

impl Default for StatePaint {
    fn default() -> Self {
        Self::new()
    }
}

impl StatePaint {
    /// A transparent element that tints on hover and press.
    pub const fn new() -> Self {
        Self { base: None, ink: None, edge: None, selected: false, highlighted: false, disabled: false, pointer: true, pressable: true, tint: true, underline: false, preview: None }
    }

    /// The element's own fill at rest; the tints composite over it.
    pub fn on(mut self, base: Hsla) -> Self {
        self.base = Some(base);
        self
    }

    /// The text colour at rest and while hovered, pressed, selected or
    /// highlighted. Children without their own colour inherit it.
    pub fn ink(mut self, rest: Hsla, lit: Hsla) -> Self {
        self.ink = Some((rest, lit));
        self
    }

    /// The border colour at rest and while lit (a bordered chip whose edge
    /// strengthens under the pointer). The caller sets the border width.
    pub fn edge(mut self, rest: Hsla, lit: Hsla) -> Self {
        self.edge = Some((rest, lit));
        self
    }

    /// Holds `ov-selected` at rest.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Holds the hover look at rest: the keyboard's row, or a trigger whose
    /// menu is open.
    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    /// No state paint at all (the caller dims the element itself).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Whether the pointer's hover refinement applies (default true). A
    /// list that drives one highlight from both the pointer and the keys
    /// turns it off and paints its lit row with [`Self::highlighted`].
    pub fn pointer(mut self, pointer: bool) -> Self {
        self.pointer = pointer;
        self
    }

    /// Whether a press tints (default true).
    pub fn pressable(mut self, pressable: bool) -> Self {
        self.pressable = pressable;
        self
    }

    /// Whether the states paint the overlay fills (default true). A text
    /// control — a link, a fold toggle, a disclosure — turns them off and
    /// lifts only its ink ([`Self::ink`]).
    pub fn tint(mut self, tint: bool) -> Self {
        self.tint = tint;
        self
    }

    /// Underlines the text while hovered (links).
    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// Paints a fixed state as the rest look (gallery cards, captures).
    pub fn preview(mut self, state: Option<InteractState>) -> Self {
        self.preview = state;
        self
    }

    /// Whether this paint is pinned by [`Self::preview`].
    pub fn is_preview(&self) -> bool {
        self.preview.is_some()
    }

    /// The four fills this paint resolves to: rest, hover, pressed — the
    /// last two `None` when the state paints nothing new. Rest is `None`
    /// for a transparent element at rest.
    pub fn fills(&self, p: &Palette) -> (Option<Hsla>, Hsla, Hsla) {
        if !self.tint {
            let base = self.base.unwrap_or_else(gpui::transparent_black);
            return (self.base, base, base);
        }
        let ground = if self.selected && !self.disabled { Some(tinted(self.base, Tint::Selected, p)) } else { self.base };
        let rest = if self.highlighted && !self.disabled { Some(tinted(ground, Tint::Hover, p)) } else { ground };
        (rest, tinted(ground, Tint::Hover, p), tinted(ground, Tint::Pressed, p))
    }

    /// The resting style plus the hover refinement (no press).
    fn base_and_hover<E: InteractiveElement + Styled>(self, el: E, p: &Palette) -> E {
        let (rest, hover, pressed) = self.fills(p);
        let held = (self.selected || self.highlighted) && !self.disabled;
        let lit_ink = self.ink.map(|(_, lit)| lit);
        let rest_ink = self.ink.map(|(rest, lit)| if held { lit } else { rest });
        let lit_edge = self.edge.map(|(_, lit)| lit);
        let rest_edge = self.edge.map(|(rest, lit)| if held { lit } else { rest });
        if let Some(fixed) = self.preview.filter(|_| !self.disabled) {
            let fill = if fixed.pressed && self.pressable {
                Some(pressed)
            } else if fixed.hovered {
                Some(hover)
            } else {
                rest
            };
            let ink = if fixed.hovered || fixed.pressed { lit_ink } else { rest_ink };
            let edge = if fixed.hovered || fixed.pressed { lit_edge } else { rest_edge };
            let el = match edge {
                Some(edge) => el.border_color(edge),
                None => el,
            };
            let el = match fill {
                Some(fill) => el.bg(fill),
                None => el,
            };
            let el = if fixed.hovered && self.underline { el.underline() } else { el };
            return match ink {
                Some(ink) => el.text_color(ink),
                None => el,
            };
        }
        let mut el = match rest {
            Some(fill) => el.bg(fill),
            None => el,
        };
        if let Some(ink) = rest_ink {
            el = el.text_color(ink);
        }
        if let Some(edge) = rest_edge {
            el = el.border_color(edge);
        }
        if self.disabled || !self.pointer {
            return el;
        }
        let (tint, underline) = (self.tint, self.underline);
        el.hover(move |s| {
            let s = if tint { s.bg(hover) } else { s };
            let s = if underline { s.underline() } else { s };
            let s = match lit_edge {
                Some(edge) => s.border_color(edge),
                None => s,
            };
            match lit_ink {
                Some(ink) => s.text_color(ink),
                None => s,
            }
        })
    }
}

/// The hover group a row names so its children can answer the row's hover
/// ([`PaintHover::hover_group`] / [`PaintHover::reveal_on_hover`]). gpui
/// resolves a group name to the innermost ancestor carrying it, so nested
/// rows share the one name safely.
pub const HOVER_GROUP: &str = "aui-hover";

/// The one model on elements with an id: rest, hover and press.
pub trait PaintStates: StatefulInteractiveElement + Styled + Sized {
    /// Paints `paint`'s rest look, hover refinement and press refinement.
    fn paint_states<S: PaletteSource + ?Sized>(self, paint: StatePaint, source: &S) -> Self {
        let p = source.palette();
        let el = paint.base_and_hover(self, &p);
        if paint.disabled || !paint.pressable || !paint.tint || paint.is_preview() {
            return el;
        }
        let (_, _, pressed) = paint.fills(&p);
        let lit_ink = paint.ink.map(|(_, lit)| lit);
        el.active(move |s| {
            let s = s.bg(pressed);
            match lit_ink {
                Some(ink) => s.text_color(ink),
                None => s,
            }
        })
    }
}

impl<E: StatefulInteractiveElement + Styled + Sized> PaintStates for E {}

/// The one model on elements without an id: rest and hover only.
pub trait PaintHover: InteractiveElement + Styled + Sized {
    /// Paints `paint`'s rest look and hover refinement.
    fn paint_hover<S: PaletteSource + ?Sized>(self, paint: StatePaint, source: &S) -> Self {
        paint.base_and_hover(self, &source.palette())
    }

    /// Names this element the hover group its children answer to.
    fn hover_group(self) -> Self {
        self.group(HOVER_GROUP)
    }

    /// A child that shows while its [`HOVER_GROUP`] ancestor is hovered (or
    /// always, when `shown`): hidden → visible as a style refinement — the
    /// trailing `×`, a row's action tray. Hidden paints nothing and takes
    /// no clicks, but keeps its layout box, so nothing reflows.
    fn reveal_on_hover(self, shown: bool) -> Self {
        if shown {
            return self;
        }
        self.invisible().group_hover(HOVER_GROUP, |s| s.visible())
    }

    /// The opposite of [`Self::reveal_on_hover`]: shown at rest, hidden while
    /// the [`HOVER_GROUP`] ancestor is hovered (a row's age under its
    /// actions). `keep` holds it shown regardless.
    fn hide_on_hover(self, keep: bool) -> Self {
        if keep {
            return self;
        }
        self.group_hover(HOVER_GROUP, |s| s.invisible())
    }

    /// A child at `rest` opacity that comes up to full while its
    /// [`HOVER_GROUP`] ancestor is hovered (a block's header actions).
    fn dim_until_hover(self, rest: f32) -> Self {
        self.opacity(rest).group_hover(HOVER_GROUP, |s| s.opacity(1.0))
    }

    /// A child whose ink lifts while its [`HOVER_GROUP`] ancestor is
    /// hovered: `rest`, then `lit` (a glyph that has its own colour).
    fn ink_on_hover(self, rest: Hsla, lit: Hsla) -> Self {
        self.text_color(rest).group_hover(HOVER_GROUP, move |s| s.text_color(lit))
    }
}

impl<E: InteractiveElement + Styled + Sized> PaintHover for E {}

#[cfg(test)]
mod tests {
    use super::*;

    fn dark() -> Palette {
        aui_tokens::dark()
    }

    #[test]
    fn over_nothing_is_the_theme_overlay() {
        let p = dark();
        assert_eq!(tinted(None, Tint::Hover, &p), p.hover_overlay());
        assert_eq!(tinted(Some(gpui::transparent_black()), Tint::Pressed, &p), p.pressed_overlay());
    }

    #[test]
    fn a_dark_fill_lightens_and_a_light_fill_darkens() {
        let p = dark();
        let lit = tinted(Some(p.surface_4), Tint::Hover, &p);
        assert!(lit.l > p.surface_4.l, "{lit:?} over {:?}", p.surface_4);
        let primary = tinted(Some(p.accent), Tint::Hover, &p);
        assert!(primary.l < p.accent.l, "{primary:?} over {:?}", p.accent);
        assert!((lit.a - 1.0).abs() < 1e-4, "an opaque fill stays opaque");
    }

    #[test]
    fn selected_holds_at_rest_and_highlighted_adds_hover() {
        let p = dark();
        let (rest, hover, pressed) = StatePaint::new().selected(true).fills(&p);
        assert_eq!(rest, Some(p.selected_overlay()));
        assert!(hover.a > p.selected_overlay().a && pressed.a > hover.a);
        let (rest, _, _) = StatePaint::new().highlighted(true).fills(&p);
        assert_eq!(rest, Some(p.hover_overlay()));
        let (rest, _, _) = StatePaint::new().highlighted(true).disabled(true).fills(&p);
        assert_eq!(rest, None, "disabled paints nothing");
    }

    #[test]
    fn over_composites_alpha() {
        let half = gpui::white().alpha(0.5);
        let out = over(gpui::transparent_black(), half);
        assert!((out.a - 0.5).abs() < 1e-4);
        let out = over(gpui::black(), half);
        assert!((out.a - 1.0).abs() < 1e-4 && (out.l - 0.5).abs() < 0.01);
    }
}
