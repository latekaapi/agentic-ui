//! Card 12: the command palette.
//!
//! The palette is a stateless list: the caller owns the query string, the
//! sections and which row is selected, and receives intents (select, hover,
//! dismiss) back. It knows nothing about how it is anchored: the host lays
//! it on [`super::scrim`], centred, its top at [`palette_top`] of the window
//! (docs/83 §7).

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette, TextRole};
use gpui::{
    div, prelude::*, px, App, ElementId, FontWeight, HighlightStyle, Hsla, IntoElement, ScrollHandle,
    SharedString, StyledText, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::interact::paint::{PaintStates, StatePaint};
use crate::data::{kbd, pill, status_dot, PillVariant};
use crate::nav::project_mark;
use crate::interact::R_ITEM;
use crate::util::{interaction_flags, Interaction};

/// The palette and the Browse Field are one floating panel (docs/83 §7):
/// 640 wide (token `palette-w`) on the Dialog level (`surface-2`) with the
/// dialog shadow, a 1 px `line-strong` border and the dialog radius.
const PAL_W: f32 = scale::PALETTE_W;
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
const Q_ICON: f32 = 16.0;

/// A section's padding, and its group label's (sentence case, `t-caption`
/// `fg-muted`, docs/83 §2.4).
const SEC_PAD_TOP: f32 = 8.0;
const SEC_PAD_X: f32 = 6.0;
const SEC_PAD_BOTTOM: f32 = 4.0;
const CAPS_PAD_X: f32 = 10.0;
const CAPS_PAD_BOTTOM: f32 = 6.0;

/// A row: 36 tall (token `palette-row`), padded 10, `t-body`, the item
/// radius, with a 14 px leading glyph.
const ITEM_H: f32 = scale::PALETTE_ROW;
const ITEM_GAP: f32 = 10.0;
const ITEM_PAD_X: f32 = 10.0;
const ITEM_ICON: f32 = 14.0;
/// `.it .hint{gap:4px}`.
const HINT_GAP: f32 = 4.0;
/// A project mark in the leading slot: 14 px, as menu rows draw it.
const MARK: f32 = 14.0;
/// `.it .pill{height:16px}` — the "needs you" badge is the short pill.
const BADGE_H: f32 = 16.0;

/// `.ft{gap:14px;height:32px;padding:0 14px;font-size:11px}`.
const FT_H: f32 = 32.0;
const FT_GAP: f32 = 14.0;
const FT_PAD_X: f32 = 14.0;
/// The single space between a footer keycap and its word (`<span class="kbd">↩
/// </span> open`), which flex cannot express as a text node here.
const FT_KEY_GAP: f32 = 4.0;

/// The top of the palette (and the Browse Field) in a window `height` tall:
/// 18 % of the height (token `palette-top`, docs/83 §7).
pub fn palette_top(height: gpui::Pixels) -> gpui::Pixels {
    height * scale::PALETTE_TOP
}

/// The palette's width in a window `width` wide: [`scale::PALETTE_W`], or
/// the window less a 16 px margin each side when it is narrower — never
/// the full width.
pub fn palette_width(width: gpui::Pixels) -> gpui::Pixels {
    px(PAL_W).min(width - px(scale::SP_5 * 2.0))
}

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The leading glyph of a palette row: an icon for actions and files, a status
/// dot for worktrees, a project mark for projects.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteIcon {
    /// A line icon from the sprite (fg-muted, ink when the row is highlighted).
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
    /// The muted description after the label (a command's summary, a
    /// worktree's repo): `t-caption` in the UI face.
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

    /// Adds the muted description after the label.
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
    /// The group label, drawn as given (sentence case).
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

/// The 640 px palette. `selected` indexes the rows of all sections in order,
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

/// What the list asked its scroll container for: the selection it scrolled
/// and whether that row has been seen in view since. The handle only learns
/// the overflow from paint, so a first-frame `scroll_to_item` lands before
/// it knows it scrolls and is dropped; the flag below re-fires until the row
/// arrives instead of trusting the first request.
#[derive(Clone, Copy)]
struct PaletteScroll {
    selected: usize,
    settled: bool,
}

/// The direct-child index of the `selected`th row inside the scrolling body:
/// every section draws its group label, its optional lead and then its rows
/// as direct children, so the child index leads the row index by the titles
/// and leads before it. Sections with no rows still draw their title (and
/// lead); `None` when `selected` is past the last row.
fn selected_child_index(counts: &[usize], leads: &[bool], selected: usize) -> Option<usize> {
    let mut child = 0usize;
    let mut remaining = selected;
    for (i, count) in counts.iter().enumerate() {
        // The group label every section draws, even an empty one.
        child += 1;
        if leads.get(i).copied().unwrap_or(false) {
            child += 1;
        }
        if remaining < *count {
            return Some(child + remaining);
        }
        remaining -= *count;
        child += *count;
    }
    None
}

/// The per-section shapes `selected_child_index` needs: the row count and
/// whether the section draws a lead.
fn child_shapes(sections: &[PaletteSection]) -> (Vec<usize>, Vec<bool>) {
    (sections.iter().map(|s| s.items.len()).collect(), sections.iter().map(|s| s.lead.is_some()).collect())
}

/// Whether the `target`th direct child is inside the scrolled viewport.
/// `false` while the container never painted (no bounds, no children yet),
/// which is exactly when the request needs (re-)firing.
fn palette_row_visible(scroll: &ScrollHandle, target: usize) -> bool {
    if scroll.bounds_for_item(target).is_none() {
        return false;
    }
    target >= scroll.top_item() && target <= scroll.bottom_item()
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
            .rounded(px(scale::SHAPE_DIALOG))
            .bg(p.surface(aui_tokens::Palette::DIALOG_LEVEL))
            .border_1()
            .border_color(p.line_strong)
            .shadow(p.dialog_shadow())
            .overflow_hidden()
            .text_color(p.ink)
            // The open palette owns the wheel over it: it occludes the
            // surface behind (no hover/click/scroll-through) and stops the
            // wheel on this same element — after the body's own scroll
            // handler, which is registered later and so runs first in the
            // bubble phase — so the palette's list scrolls and the surface
            // beneath never does. Mirrors the composer menus.
            .occlude()
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .child(query_row(&id, &p, &self.query, &self.placeholder, self.query_slot, self.on_dismiss.clone()));
        // The list scrolls inside a bounded body; the query row and the
        // footer stay put. Every section draws its group label, its optional
        // lead and then its rows as direct children of the body, so the
        // scroll handle can address the selected row (see
        // [`selected_child_index`]); the paddings match the old per-section
        // blocks exactly, so a list that fits draws as before.
        let (counts, leads) = child_shapes(&self.sections);
        let scroll: ScrollHandle =
            window.use_keyed_state((id.clone(), "scroll"), cx, |_, _| ScrollHandle::new()).read(cx).clone();
        let scrolled: Rc<RefCell<PaletteScroll>> = window
            .use_keyed_state((id.clone(), "scrolled"), cx, |_, _| Rc::new(RefCell::new(PaletteScroll { selected: usize::MAX, settled: false })))
            .read(cx)
            .clone();
        let mut body = v_flex()
            .id((id.clone(), "body"))
            .flex_none()
            .w_full()
            .px(px(SEC_PAD_X))
            .pb(px(SEC_PAD_BOTTOM))
            .max_h(px(BODY_MAX_H))
            .overflow_y_scroll()
            .track_scroll(&scroll);

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

        // Keyboard only: while the pointer owns the highlight (the hovered
        // row is the selection) the list never scrolls under it; the arrow
        // keys' `selected` scrolls into view the moment the pointer leaves.
        if hovered_row.is_none_or(|hovered| hovered != self.selected) {
            let mut kept = scrolled.borrow_mut();
            if kept.selected != self.selected {
                kept.selected = self.selected;
                kept.settled = false;
            }
            if !kept.settled {
                match selected_child_index(&counts, &leads, self.selected) {
                    Some(target) if !palette_row_visible(&scroll, target) => {
                        scroll.scroll_to_item(target);
                        // The re-fire above may itself be dropped (see
                        // [`PaletteScroll`]); ask for the frame that retries it.
                        aui_motion::request_frame(window);
                    }
                    _ => kept.settled = true,
                }
            }
        }

        let mut index = 0usize;
        let last_section = self.sections.len().saturating_sub(1);
        for (s, section) in self.sections.into_iter().enumerate() {
            body = body.child(
                div()
                    .flex_none()
                    .pt(px(SEC_PAD_TOP))
                    .px(px(CAPS_PAD_X))
                    .pb(px(CAPS_PAD_BOTTOM))
                    .role_in(TextRole::Caption, cx)
                    .text_color(p.fg_muted)
                    .child(section.title.clone()),
            );
            // The lead is not a row: it draws under the title with the
            // rows' horizontal padding and a `SP_2` gap below it, creates
            // no selection index and wires no hover, and scrolls with the
            // list because it lives inside the scrolling body.
            if let Some(lead) = section.lead {
                body = body.child(div().flex_none().w_full().px(px(ITEM_PAD_X)).pb(px(scale::SP_2)).child(lead));
            }
            let last_row = section.items.len().saturating_sub(1);
            for (r, item) in section.items.into_iter().enumerate() {
                let (key, state) = rows[index].clone();
                // Every section used to end in its block's bottom padding;
                // the last row carries it as a margin instead, except on the
                // last section, where the body's own bottom padding does.
                let trailing = r == last_row && s != last_section;
                body = body.child(palette_row(
                    key,
                    state,
                    &p,
                    item,
                    index,
                    index == active,
                    trailing,
                    &self.on_select,
                    &self.on_hover,
                    window,
                    cx,
                ));
                index += 1;
            }
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
        .ui(TextRole::Subtitle)
        .child(icon(IconName::Search).size(px(Q_ICON)).color(p.fg_muted))
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
    trailing: bool,
    on_select: &Option<SelectHandler>,
    on_hover: &Option<HoverHandler>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    // The one lit row (the pointer's or the keys') paints the hover look
    // through the one model; the pointer's own refinement is off, so a
    // resting pointer never lights a second row.
    let glyph_color = if on { p.ink } else { p.fg_muted };

    let mut row = h_flex()
        .id(id.clone())
        .flex_none()
        .w_full()
        .h(px(ITEM_H))
        .gap(px(ITEM_GAP))
        .px(px(ITEM_PAD_X))
        .rounded(px(R_ITEM))
        .paint_states(StatePaint::new().pointer(false).highlighted(on).ink(p.fg_muted, p.ink), p)
        .text_role(TextRole::Body)
        .cursor_pointer();
    // The old per-section block ended in bottom padding; the flattened body
    // moves it onto the section's last row, so sections keep their spacing.
    if trailing {
        row = row.mb(px(SEC_PAD_BOTTOM));
    }

    // One `on_hover` per element is all gpui allows, so the pointer report
    // the single highlight reads and the caller's hover intent share one
    // handler.
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
        // The description: `t-caption` `fg-muted` in the UI face, never mono
        // (docs/83 §7).
        row = row.child(
            div()
                .flex_shrink(1.0)
                .min_w(px(0.0))
                .truncate()
                .text_role(TextRole::Caption)
                .text_color(p.fg_muted)
                .child(context),
        );
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
        .ui(TextRole::Micro)
        .text_color(p.fg_muted)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str) -> PaletteItem {
        PaletteItem::new(id, PaletteIcon::Glyph(IconName::Search), id)
    }

    fn section(title: &str, lead: bool, n: usize) -> PaletteSection {
        let items = (0..n).map(|i| row(&format!("{title}-{i}"))).collect();
        let section = PaletteSection::new(title, items);
        if lead { section.lead(div().child("a lead")) } else { section }
    }

    /// docs/83 §7: 640 wide, never the full window, its top at 18 %.
    #[test]
    fn the_palette_is_640_wide_with_its_top_at_18_percent() {
        assert_eq!(palette_top(px(900.0)), px(162.0));
        assert_eq!(palette_width(px(1440.0)), px(640.0));
        assert_eq!(palette_width(px(600.0)), px(568.0), "a narrow window keeps a 16 px margin a side");
    }

    #[test]
    fn selected_child_index_counts_titles_but_not_rows() {
        let sections = vec![section("a", false, 2), section("b", false, 3)];
        let (counts, leads) = child_shapes(&sections);
        // Section `a`: title at 0, rows at 1–2. Section `b`: title at 3,
        // rows at 4–6.
        assert_eq!(selected_child_index(&counts, &leads, 0), Some(1));
        assert_eq!(selected_child_index(&counts, &leads, 1), Some(2));
        assert_eq!(selected_child_index(&counts, &leads, 2), Some(4));
        assert_eq!(selected_child_index(&counts, &leads, 4), Some(6));
        assert_eq!(selected_child_index(&counts, &leads, 5), None);
    }

    #[test]
    fn selected_child_index_skips_leads_and_empty_sections() {
        let sections = vec![section("lead-only", true, 0), section("full", true, 2), section("plain", false, 1)];
        let (counts, leads) = child_shapes(&sections);
        // Children: title, lead (section one, no rows); title, lead, rows at
        // 4–5 (section two); title, row at 7 (section three).
        assert_eq!(selected_child_index(&counts, &leads, 0), Some(4));
        assert_eq!(selected_child_index(&counts, &leads, 1), Some(5));
        assert_eq!(selected_child_index(&counts, &leads, 2), Some(7));
        assert_eq!(selected_child_index(&counts, &leads, 3), None);
    }

    #[test]
    fn selected_child_index_without_rows_is_none() {
        let sections = vec![section("empty", false, 0)];
        let (counts, leads) = child_shapes(&sections);
        assert_eq!(selected_child_index(&counts, &leads, 0), None);
        let none: Vec<PaletteSection> = Vec::new();
        let (counts, leads) = child_shapes(&none);
        assert_eq!(selected_child_index(&counts, &leads, 0), None);
    }
}
