//! Card 12: the command palette.
//!
//! The palette is a stateless list: the caller owns the query string, the
//! sections and which row is selected, and receives intents (select, hover,
//! dismiss) back. It knows nothing about how it is anchored — [`palette_scrim`]
//! draws the dimmed ground the design card puts behind it, and the app is free
//! to place the palette itself.

use std::ops::Range;
use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, tint_fade, tween, EnterExit, PresenceStyle, Tween};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette, TextRole};
use gpui::{
    black, div, linear_color_stop, linear_gradient, prelude::*, px, App, ElementId, FontWeight, HighlightStyle, Hsla, IntoElement, SharedString,
    StyledText, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{kbd, pill, status_dot, PillVariant};
use crate::nav::project_mark;
use crate::util::{interaction_flags, Interaction};

/// `.scrim{height:470px;border-radius:var(--r-lg);padding-top:56px}`.
const SCRIM_H: f32 = 470.0;
const SCRIM_PAD_TOP: f32 = 56.0;
/// `.scrim{background:linear-gradient(180deg,…),var(--bg)}` — the two stops
/// live in the module so the modal [`crate::overlay::Dialog`] shares them.
use super::{SCRIM_TINT_BOTTOM, SCRIM_TINT_TOP};
const SCRIM_ANGLE: f32 = 180.0;

/// `.pal{width:560px;border-radius:var(--r-xl)}` on the overlay ground with a
/// 1 px line-strong border and `box-shadow:var(--shadow-3)`.
const PAL_W: f32 = 560.0;
const PAL_SHADOW: u8 = 3;
/// `@keyframes in{from{opacity:0;transform:translateY(-6px) scale(.985)}}` —
/// the palette drops in from 6 px above at 98.5 % of its width.
const PAL_DROP: f32 = 6.0;
const PAL_FROM_SCALE: f32 = 0.985;

/// `.q{gap:10px;padding:0 14px;height:46px;font-size:14px}` with a 16 px
/// (`.i.lg`) search icon.
const Q_H: f32 = 46.0;
/// How tall the list between the query row and the footer may grow before
/// it scrolls: a palette that ran to the window's bottom edge read as broken.
const BODY_MAX_H: f32 = 320.0;
const Q_GAP: f32 = 10.0;
const Q_PAD_X: f32 = 14.0;
const Q_TEXT: f32 = scale::FS_14;
const Q_ICON: f32 = 16.0;

/// `.sec{padding:8px 6px 4px}` and `.sec .caps{padding:0 10px 6px}`.
const SEC_PAD_TOP: f32 = 8.0;
const SEC_PAD_X: f32 = 6.0;
const SEC_PAD_BOTTOM: f32 = 4.0;
const CAPS_PAD_X: f32 = 10.0;
const CAPS_PAD_BOTTOM: f32 = 6.0;

/// `.it{gap:10px;height:34px;padding:0 10px;border-radius:var(--r-sm);
/// font-size:13px}` with a 14 px (`.i`) leading glyph.
const ITEM_H: f32 = 34.0;
const ITEM_GAP: f32 = 10.0;
const ITEM_PAD_X: f32 = 10.0;
const ITEM_ICON: f32 = 14.0;
/// `.it .hint{gap:4px}`.
const HINT_GAP: f32 = 4.0;
/// A project mark in the leading slot: 14 px, as menu rows draw it.
const MARK: f32 = 14.0;
/// `.it .subtle.mono{font-size:11px}` — the worktree's repo.
const CONTEXT_TEXT: f32 = scale::FS_11;
/// `.it .pill{height:16px}` — the "needs you" badge is the short pill.
const BADGE_H: f32 = 16.0;

/// `.ft{gap:14px;height:32px;padding:0 14px;font-size:11px}`.
const FT_H: f32 = 32.0;
const FT_GAP: f32 = 14.0;
const FT_PAD_X: f32 = 14.0;
const FT_TEXT: f32 = scale::FS_11;
/// The single space between a footer keycap and its word (`<span class="kbd">↩
/// </span> open`), which flex cannot express as a text node here.
const FT_KEY_GAP: f32 = 4.0;

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The leading glyph of a palette row: an icon for actions and files, a status
/// dot for worktrees, a project mark for projects.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteIcon {
    /// A line icon from the sprite (ink-3, ink when the row is highlighted).
    Glyph(IconName),
    /// A 7 px status dot in the agent state's colour.
    Dot(AgentState),
    /// A 14 px project mark in the project's label colour.
    Mark {
        /// The mark's initial.
        initial: SharedString,
        /// The mark's fill (the project label colour).
        colour: Hsla,
    },
}

/// One row of the palette.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteItem {
    /// Stable identity, handed back by [`CommandPalette::on_select`].
    pub id: SharedString,
    /// The leading glyph.
    pub icon: PaletteIcon,
    /// The row label.
    pub label: SharedString,
    /// Byte ranges of `label` that matched the query; drawn accent-ink 600.
    pub matched: Vec<Range<usize>>,
    /// Muted mono context after the label (a worktree's repo).
    pub context: Option<SharedString>,
    /// A status pill after the label ("needs you").
    pub badge: Option<SharedString>,
    /// The shortcut keycaps at the right edge.
    pub keys: Vec<SharedString>,
}

impl PaletteItem {
    /// A row with an icon and a label and nothing else.
    pub fn new(id: impl Into<SharedString>, icon: PaletteIcon, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), icon, label: label.into(), matched: Vec::new(), context: None, badge: None, keys: Vec::new() }
    }

    /// Marks a byte range of the label as a query match.
    pub fn matched(mut self, range: Range<usize>) -> Self {
        self.matched.push(range);
        self
    }

    /// Marks the first occurrence of `needle` in the label as a query match.
    pub fn matching(self, needle: &str) -> Self {
        match self.label.find(needle) {
            Some(at) => self.matched(at..at + needle.len()),
            None => self,
        }
    }

    /// Adds the muted mono context after the label.
    pub fn context(mut self, context: impl Into<SharedString>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Adds the status pill after the label.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Adds one keycap to the right-hand hint.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.keys.push(key.into());
        self
    }
}

/// A titled block of rows (`Worktrees`, `Actions`, `Files`).
///
/// A section may carry a `lead` element — rendered inside the palette under
/// the section title and before the rows — as well as zero or more rows.
pub struct PaletteSection {
    /// The caps header.
    pub title: SharedString,
    /// The rows, in order.
    pub items: Vec<PaletteItem>,
    lead: Option<gpui::AnyElement>,
}

impl PaletteSection {
    /// A section with its header and rows.
    pub fn new(title: impl Into<SharedString>, items: Vec<PaletteItem>) -> Self {
        Self { title: title.into(), items, lead: None }
    }

    /// A non-row element drawn under the section title and before the rows,
    /// with the rows' horizontal padding and a `SP_2` gap below it. It is
    /// not a row: keyboard selection skips it, hover does nothing to it,
    /// and it scrolls with the list. A section may have a lead and no rows.
    pub fn lead(mut self, el: impl IntoElement) -> Self {
        self.lead = Some(el.into_any_element());
        self
    }
}

/// The command palette. Build with [`command_palette`].
#[derive(IntoElement)]
pub struct CommandPalette {
    id: ElementId,
    query: SharedString,
    /// A caller-owned editor drawn in the query row instead of the
    /// display-only query text, so typing happens inside the card.
    query_slot: Option<gpui::AnyElement>,
    placeholder: SharedString,
    sections: Vec<PaletteSection>,
    selected: usize,
    present: bool,
    timing: EnterExit,
    on_select: Option<SelectHandler>,
    on_hover: Option<HoverHandler>,
    on_dismiss: Option<DismissHandler>,
}

/// The 560 px palette. `selected` indexes the rows of all sections in order,
/// as the arrow keys walk them.
pub fn command_palette(id: impl Into<ElementId>, query: impl Into<SharedString>, sections: Vec<PaletteSection>, selected: usize) -> CommandPalette {
    CommandPalette {
        id: id.into(),
        query: query.into(),
        query_slot: None,
        placeholder: SharedString::default(),
        sections,
        selected,
        present: true,
        timing: EnterExit::DEFAULT,
        on_select: None,
        on_hover: None,
        on_dismiss: None,
    }
}

impl CommandPalette {
    /// A real editor for the query row: the caller's own field, drawn
    /// chromeless where the query text would be, so the palette is one
    /// surface with the field inside it rather than a field floating above.
    pub fn query_slot(mut self, editor: impl IntoElement) -> Self {
        self.query_slot = Some(editor.into_any_element());
        self
    }

    /// The ink-4 text shown in the query row while the query is empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Whether the palette is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the palette is drawn at rest on its first frame. For a
    /// palette that is part of a static composition (the design card) rather
    /// than one the person just opened.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// A row was clicked; the argument is its [`PaletteItem::id`].
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// The pointer entered a row; the argument is its index across all
    /// sections, so the caller can move the selection to it.
    pub fn on_hover(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    /// The `esc` keycap was clicked.
    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let style = PresenceStyle::fade_rise_scale(sample, PAL_DROP, PAL_FROM_SCALE);

        let pal = v_flex()
            .flex_none()
            .relative()
            // The CSS enters from `translateY(-6px)`: the palette drops onto
            // its resting place, so the rise is applied upwards.
            .top(-style.offset_y)
            .opacity(style.opacity)
            .w(px(PAL_W * style.scale))
            .rounded(px(scale::R_XL))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .shadow(p.shadow(PAL_SHADOW))
            .overflow_hidden()
            .text_color(p.ink)
            // The open palette owns the wheel over it: it occludes the
            // surface behind (no hover/click/scroll-through) and stops the
            // wheel on this same element — after the body's own scroll
            // handler, which is registered later and so runs first in the
            // bubble phase — so the palette's list scrolls and the surface
            // beneath never does. Mirrors the composer menus.
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(query_row(&id, &p, &self.query, &self.placeholder, self.query_slot, self.on_dismiss.clone()));
        // The list scrolls inside a bounded body; the query row and the
        // footer stay put.
        let mut body = v_flex().id((id.clone(), "body")).flex_none().w_full().max_h(px(BODY_MAX_H)).overflow_y_scroll();

        // One highlight, not one per row. The pointer and the arrow keys drive
        // the same row: while the pointer is over a row it owns the highlight,
        // and the keyboard's `selected` takes it back the moment the pointer
        // leaves, so the palette never lights two rows at once — with or
        // without a caller wired to `on_hover`.
        let mut rows: Vec<(ElementId, gpui::Entity<Interaction>)> = Vec::new();
        let mut hovered_row = None;
        for (s, section) in self.sections.iter().enumerate() {
            for _ in &section.items {
                let index = rows.len();
                let key: ElementId = (id.clone(), SharedString::from(format!("row-{s}-{index}"))).into();
                let (state, flags) = interaction_flags(key.clone(), window, cx);
                if flags.hovered {
                    hovered_row = Some(index);
                }
                rows.push((key, state));
            }
        }
        let active = hovered_row.unwrap_or(self.selected);

        let mut index = 0usize;
        for section in self.sections.into_iter() {
            let mut block = v_flex()
                .flex_none()
                .w_full()
                .pt(px(SEC_PAD_TOP))
                .px(px(SEC_PAD_X))
                .pb(px(SEC_PAD_BOTTOM))
                .child(
                    div()
                        .flex_none()
                        .px(px(CAPS_PAD_X))
                        .pb(px(CAPS_PAD_BOTTOM))
                        .text_role(TextRole::Caps)
                        .line_height(gpui::relative(scale::LH_UI))
                        .text_color(p.ink_3)
                        .child(section.title.to_uppercase()),
                );
            // The lead is not a row: it draws under the title with the
            // rows' horizontal padding and a `SP_2` gap below it, creates
            // no selection index and wires no hover, and scrolls with the
            // list because it lives inside the scrolling body.
            if let Some(lead) = section.lead {
                block = block.child(div().flex_none().w_full().px(px(ITEM_PAD_X)).pb(px(scale::SP_2)).child(lead));
            }
            for item in section.items {
                let (key, state) = rows[index].clone();
                block = block.child(palette_row(key, state, &p, item, index, index == active, &self.on_select, &self.on_hover, window, cx));
                index += 1;
            }
            body = body.child(block);
        }

        pal.child(body).child(footer(&p))
    }
}

/// `.q`: the search icon, the query (or its placeholder) and the `esc` keycap.
fn query_row(
    id: &ElementId,
    p: &Palette,
    query: &SharedString,
    placeholder: &SharedString,
    slot: Option<gpui::AnyElement>,
    on_dismiss: Option<DismissHandler>,
) -> impl IntoElement {
    let empty = query.is_empty();
    let mut esc = div().id((id.clone(), "esc")).flex_none().ml_auto().child(kbd("esc"));
    if let Some(handler) = on_dismiss {
        esc = esc.cursor_pointer().on_click(move |_, w, cx| handler(w, cx));
    }
    let text: gpui::AnyElement = match slot {
        Some(editor) => div().flex_1().min_w(px(0.0)).child(editor).into_any_element(),
        None => div()
            .flex_1()
            .min_w(px(0.0))
            .truncate()
            .text_color(if empty { p.ink_4 } else { p.ink })
            .child(if empty { placeholder.clone() } else { query.clone() })
            .into_any_element(),
    };
    h_flex()
        .flex_none()
        .w_full()
        .h(px(Q_H))
        .gap(px(Q_GAP))
        .px(px(Q_PAD_X))
        .border_b_1()
        .border_color(p.line)
        .ui(Q_TEXT)
        .child(icon(IconName::Search).size(px(Q_ICON)).color(p.ink_3))
        .child(text)
        .child(esc)
}

/// `.it`: one row. `active` is the palette's single highlight — the hovered row
/// if the pointer is over one, otherwise the keyboard's selection — so the
/// pointer and the arrow keys always agree on what `↩` would open. The tint,
/// the label and the glyph all cross-fade over the hover duration, which lets
/// the highlight travel between neighbouring rows instead of flashing.
#[allow(clippy::too_many_arguments)]
fn palette_row(
    id: ElementId,
    state: gpui::Entity<Interaction>,
    p: &Palette,
    item: PaletteItem,
    index: usize,
    on: bool,
    on_select: &Option<SelectHandler>,
    on_hover: &Option<HoverHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let ground = tint_fade((id.clone(), "bg"), on, p.surface_3, Tween::FAST, window, cx);
    let glyph_color = tween((id.clone(), "glyph"), if on { p.ink } else { p.ink_3 }, Tween::FAST, window, cx);
    let text_color = tween((id.clone(), "text"), if on { p.ink } else { p.ink_2 }, Tween::FAST, window, cx);

    let mut row = h_flex()
        .id(id.clone())
        .flex_none()
        .w_full()
        .h(px(ITEM_H))
        .gap(px(ITEM_GAP))
        .px(px(ITEM_PAD_X))
        .rounded(px(scale::R_SM))
        .bg(ground)
        .ui(scale::FS_13)
        .text_color(text_color)
        .cursor_pointer();

    // One `on_hover` per element is all gpui allows, so the row's own hover
    // tint and the caller's hover intent share a single handler instead of
    // going through `track_interaction`.
    {
        let hovered_state = state.clone();
        let handler = on_hover.clone();
        row = row.on_hover(move |hovered, w, cx| {
            hovered_state.update(cx, |s, cx| {
                if s.hovered != *hovered {
                    s.hovered = *hovered;
                    cx.notify();
                }
            });
            if *hovered {
                if let Some(handler) = &handler {
                    handler(index, w, cx);
                }
            }
        });
    }

    row = match item.icon {
        PaletteIcon::Glyph(name) => row.child(icon(name).size(px(ITEM_ICON)).color(glyph_color)),
        PaletteIcon::Dot(state) => row.child(status_dot((id.clone(), "dot"), state)),
        PaletteIcon::Mark { initial, colour } => row.child(project_mark(initial, colour).size(px(MARK))),
    };

    row = row.child(label(p, &item.label, &item.matched));

    if let Some(context) = item.context {
        // The context yields after the label: a long snippet truncates
        // inside the row instead of running past the card's right edge.
        row = row.child(div().flex_shrink(1.0).min_w(px(0.0)).truncate().mono(CONTEXT_TEXT).text_color(p.ink_3).child(context));
    }
    if let Some(badge) = item.badge {
        row = row.child(pill(badge).variant(PillVariant::Warning).height(BADGE_H));
    }
    if !item.keys.is_empty() {
        let mut hint = h_flex().flex_none().ml_auto().gap(px(HINT_GAP));
        for key in item.keys {
            hint = hint.child(kbd(key));
        }
        row = row.child(hint);
    }

    if let Some(handler) = on_select.clone() {
        let item_id = item.id.clone();
        row = row.on_click(move |_, w, cx| handler(&item_id, w, cx));
    }
    row
}

/// The label with its matched substrings in accent-ink 600 (`.it mark`).
fn label(p: &Palette, text: &SharedString, matched: &[Range<usize>]) -> impl IntoElement {
    let highlight = HighlightStyle { color: Some(p.accent_ink), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    let highlights: Vec<_> = matched.iter().map(|r| (r.clone(), highlight)).collect();
    // The label is the one row part that yields: it shrinks and truncates so a
    // long command never pushes the keycaps off the row.
    div().flex_shrink(1.0).min_w(px(0.0)).truncate().child(StyledText::new(text.clone()).with_highlights(highlights))
}

/// `.ft`: the key hints, then the quick-open note at the right.
fn footer(p: &Palette) -> impl IntoElement {
    h_flex()
        .flex_none()
        .w_full()
        .h(px(FT_H))
        .gap(px(FT_GAP))
        .px(px(FT_PAD_X))
        .border_t_1()
        .border_color(p.line)
        .ui(FT_TEXT)
        .text_color(p.ink_3)
        .child(footer_hint("↑↓", "navigate"))
        .child(footer_hint("↩", "open"))
        .child(footer_hint("⌘↩", "open to the side"))
        .child(div().flex_1())
        .child(div().flex_none().child("⌘P files only"))
}

/// One `<span class="kbd">…</span> word` footer hint.
fn footer_hint(key: &'static str, word: &'static str) -> impl IntoElement {
    h_flex().flex_none().gap(px(FT_KEY_GAP)).child(kbd(key)).child(word)
}

/// The dimmed ground the palette floats on. Build with [`palette_scrim`].
#[derive(IntoElement)]
pub struct PaletteScrim {
    child: gpui::AnyElement,
}

/// `.scrim`: 470 px tall, a black .25 → .45 gradient over the window ground,
/// with `child` centred 56 px below the top edge.
pub fn palette_scrim(child: impl IntoElement) -> PaletteScrim {
    PaletteScrim { child: child.into_any_element() }
}

impl RenderOnce for PaletteScrim {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        div()
            .relative()
            .w_full()
            .h(px(SCRIM_H))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line)
            .bg(p.bg)
            .overflow_hidden()
            .child(
                div().absolute().inset_0().bg(linear_gradient(
                    SCRIM_ANGLE,
                    linear_color_stop(black().opacity(SCRIM_TINT_TOP), 0.0),
                    linear_color_stop(black().opacity(SCRIM_TINT_BOTTOM), 1.0),
                )),
            )
            // The scrim is a flex row with the default `align-items:stretch`,
            // so the palette fills the height left below the 56 px padding
            // (412 px) rather than shrinking to its rows.
            .child(h_flex().relative().w_full().h_full().items_stretch().justify_center().pt(px(SCRIM_PAD_TOP)).child(self.child))
    }
}
