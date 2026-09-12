//! Card 41 · the two caret popovers: the `/` command menu and the `@` mention
//! picker (spec §4.2).
//!
//! Both are stateless lists in the shape of the command palette: the caller
//! owns the query, the sections and the selected row, and receives intents
//! (select, hover) back. Neither knows how it is anchored — each renders at the
//! width of its container up to the 520 px dropdown ceiling, above the
//! composer, and enters with the 6 px rise and .98 scale of `.pop`.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{presence, tint_fade, tween, EnterExit, PresenceStyle, Tween};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, Palette, TextRole};
use gpui::{
    div, prelude::*, px, relative, App, Div, ElementId, FontWeight, HighlightStyle, IntoElement, ScrollHandle, SharedString, Stateful,
    StyledText, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{kbd, status_dot, tag};
use crate::util::interaction_flags;

/// `.pop{padding:6px;border-radius:var(--r-lg);box-shadow:var(--shadow-3)}` on
/// the overlay ground with a 1 px line-strong border.
const POP_PAD: f32 = 6.0;
const POP_SHADOW: u8 = 3;
/// `.pop{margin-bottom:8px}` — the gap to the composer the popover sits above.
const POP_GAP: f32 = 8.0;
/// `@keyframes in{from{opacity:0;transform:translateY(6px) scale(.98)}}` — the
/// popover rises out of the composer's top edge.
const POP_RISE: f32 = 6.0;
const POP_FROM_SCALE: f32 = 0.98;
/// The dropdown ceiling: the caret menus never grow past the picker ceiling
/// (`pickers::MENU_W_MAX`), so both read as one control family. The popover
/// keeps its left edge and rows wrap inside the cap.
const MENU_W_MAX: f32 = 520.0;
/// The popover's own scroll ceiling: a long menu scrolls inside its own
/// frame instead of growing past the transcript it occludes.
const MENU_MAX_H: f32 = 560.0;

/// `.pop .caps{padding:6px 8px 4px}`.
const CAPS_PAD_TOP: f32 = 6.0;
const CAPS_PAD_X: f32 = 8.0;
const CAPS_PAD_BOTTOM: f32 = 4.0;

/// `.it{height:32px;gap:10px;padding:0 8px;border-radius:var(--r-sm);
/// font-size:12.5px}`.
const ROW_H: f32 = 32.0;
const ROW_GAP: f32 = 10.0;
const ROW_PAD_X: f32 = 8.0;
const ROW_TEXT: f32 = 12.5;
/// `.it .c{font:500 12px var(--font-mono);width:100px}` — the command column
/// is fixed, so a long command wraps inside it.
const COMMAND_W: f32 = 100.0;
const COMMAND_TEXT: f32 = scale::FS_12;
/// The `font:` shorthand of `.it .c` resets the line height to `normal`.
const COMMAND_LH: f32 = 1.5;
/// `.it .d{font-size:12px}`, and `.it .d.mono{font-size:11px}` for the paths
/// and symbol locations of the mention picker.
const DETAIL_TEXT: f32 = scale::FS_12;
const DETAIL_MONO_TEXT: f32 = scale::FS_11;
/// `.it .k{gap:4px}`.
const KEYS_GAP: f32 = 4.0;
/// `.i{width:14px}` — the file, folder and globe glyphs.
const ROW_ICON: f32 = 14.0;
/// `<span class="ic mono" style="font-size:11px">ƒ</span>` — the symbol glyph.
const SYMBOL_TEXT: f32 = scale::FS_11;

/// `.ft{gap:12px;padding:6px 8px 2px;border-top:1px solid var(--line);
/// margin-top:4px;font-size:11px}`.
const FT_GAP: f32 = 12.0;
const FT_PAD_TOP: f32 = 6.0;
const FT_PAD_X: f32 = 8.0;
const FT_PAD_BOTTOM: f32 = 2.0;
const FT_MARGIN_TOP: f32 = 4.0;
const FT_TEXT: f32 = scale::FS_11;
/// The single space between a footer keycap and its word, which flex has to
/// draw as a gap here.
const FT_KEY_GAP: f32 = 4.0;

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// One row of the `/` menu.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandItem {
    /// Stable identity, handed back by [`CommandMenu::on_select`].
    pub id: SharedString,
    /// The command as typed, leading slash included (`/review`).
    pub command: SharedString,
    /// What the command does, or where a custom command comes from.
    pub description: SharedString,
    /// The keycap at the right edge (`↩` on the selected row).
    pub key: Option<SharedString>,
    /// The source tag at the right edge (`skill`, `project`).
    pub source_tag: Option<SharedString>,
}

impl CommandItem {
    /// A command row.
    pub fn new(id: impl Into<SharedString>, command: impl Into<SharedString>, description: impl Into<SharedString>) -> Self {
        Self { id: id.into(), command: command.into(), description: description.into(), key: None, source_tag: None }
    }

    /// Adds the keycap at the right edge.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Adds the source tag at the right edge.
    pub fn source_tag(mut self, source_tag: impl Into<SharedString>) -> Self {
        self.source_tag = Some(source_tag.into());
        self
    }
}

/// A titled block of command rows (`Built-in`, `Skills`, `Custom`).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandSection {
    /// The caps header.
    pub title: SharedString,
    /// The rows, in order.
    pub items: Vec<CommandItem>,
}

impl CommandSection {
    /// A section with its header and rows.
    pub fn new(title: impl Into<SharedString>, items: Vec<CommandItem>) -> Self {
        Self { title: title.into(), items }
    }
}

/// The `/` command menu. Build with [`command_menu`].
#[derive(IntoElement)]
pub struct CommandMenu {
    id: ElementId,
    query: SharedString,
    sections: Vec<CommandSection>,
    selected: usize,
    present: bool,
    timing: EnterExit,
    on_select: Option<SelectHandler>,
    on_hover: Option<HoverHandler>,
}

/// The command menu at the width of its container. `query` is what has been
/// typed so far (`/re`); its first occurrence in each command is drawn in
/// accent-ink 600. `selected` indexes the rows of all sections in order, as the
/// arrow keys walk them.
pub fn command_menu(id: impl Into<ElementId>, query: impl Into<SharedString>, sections: Vec<CommandSection>, selected: usize) -> CommandMenu {
    CommandMenu {
        id: id.into(),
        query: query.into(),
        sections,
        selected,
        present: true,
        timing: EnterExit::QUICK,
        on_select: None,
        on_hover: None,
    }
}

impl CommandMenu {
    /// Whether the menu is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the menu is drawn at rest on its first frame, for a
    /// static composition (the design card) rather than one the person just
    /// opened by typing `/`.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// A row was clicked; the argument is its [`CommandItem::id`].
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
}

impl RenderOnce for CommandMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);

        let counts: Vec<usize> = self.sections.iter().map(|s| s.items.len()).collect();
        let active = active_row(&id, "command", &counts, self.selected, window, cx);
        // The keyboard's selection drives the scroll, never the pointer's
        // hover: scrolling the row under the pointer into view on every
        // frame fought the wheel and read as jank.
        let scroll = menu_scroll(&id, &counts, self.selected, window, cx);
        let mut pop = popover_frame(id.clone(), PresenceStyle::fade_rise_scale(sample, POP_RISE, POP_FROM_SCALE), &p, &scroll);

        let mut index = 0usize;
        for (s, section) in self.sections.into_iter().enumerate() {
            pop = pop.child(caps_header(&p, section.title));
            for item in section.items {
                let key: ElementId = (id.clone(), SharedString::from(format!("command-{s}-{index}"))).into();
                pop = pop.child(command_row(key, &p, item, &self.query, index, index == active, &self.on_select, &self.on_hover, window, cx));
                index += 1;
            }
        }
        pop.child(footer(&p))
    }
}

/// The leading glyph of a mention row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MentionIcon {
    /// A 14 px line icon from the sprite: a file, a folder or a globe.
    Glyph(IconName),
    /// The mono `ƒ` that marks a symbol.
    Symbol,
    /// A 7 px status dot: another worktree, in its agent state's colour.
    Dot(AgentState),
}

/// One row of the `@` picker.
#[derive(Debug, Clone, PartialEq)]
pub struct MentionItem {
    /// Stable identity, handed back by [`MentionPicker::on_select`].
    pub id: SharedString,
    /// The leading glyph.
    pub icon: MentionIcon,
    /// The name that would be inserted as a chip.
    pub label: SharedString,
    /// The byte range of `label` that matched the query, drawn accent-ink 600.
    /// Empty means "highlight the query itself wherever it occurs".
    pub matched: Range<usize>,
    /// The muted line after the label: a path, a symbol location or a note.
    pub detail: SharedString,
    /// Whether the detail is mono 11 (paths, locations) rather than UI 12.
    pub detail_mono: bool,
    /// The keycap at the right edge (`↩` on the selected row).
    pub key: Option<SharedString>,
}

impl MentionItem {
    /// A mention row with a UI-face detail line.
    pub fn new(id: impl Into<SharedString>, icon: MentionIcon, label: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self { id: id.into(), icon, label: label.into(), matched: 0..0, detail: detail.into(), detail_mono: false, key: None }
    }

    /// Draws the detail line as mono 11 (a path, a `file:line`).
    pub fn detail_mono(mut self) -> Self {
        self.detail_mono = true;
        self
    }

    /// Marks a byte range of the label as the query match.
    pub fn matched(mut self, range: Range<usize>) -> Self {
        self.matched = range;
        self
    }

    /// Marks the first occurrence of `needle` in the label as the query match.
    pub fn matching(self, needle: &str) -> Self {
        match self.label.find(needle) {
            Some(at) => self.matched(at..at + needle.len()),
            None => self,
        }
    }

    /// Adds the keycap at the right edge.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }
}

/// A titled block of mention rows (`Files in acme-web`, `Symbols`).
#[derive(Debug, Clone, PartialEq)]
pub struct MentionSection {
    /// The caps header.
    pub title: SharedString,
    /// The rows, in order.
    pub items: Vec<MentionItem>,
}

impl MentionSection {
    /// A section with its header and rows.
    pub fn new(title: impl Into<SharedString>, items: Vec<MentionItem>) -> Self {
        Self { title: title.into(), items }
    }
}

/// The `@` mention picker. Build with [`mention_picker`].
#[derive(IntoElement)]
pub struct MentionPicker {
    id: ElementId,
    query: SharedString,
    sections: Vec<MentionSection>,
    selected: usize,
    present: bool,
    timing: EnterExit,
    on_select: Option<SelectHandler>,
    on_hover: Option<HoverHandler>,
}

/// The mention picker at the width of its container. `query` is what has been
/// typed after the `@`; a row that carries no explicit
/// [`MentionItem::matched`] range highlights the query wherever it occurs in
/// the label. `selected` indexes the rows of all sections in order.
pub fn mention_picker(id: impl Into<ElementId>, query: impl Into<SharedString>, sections: Vec<MentionSection>, selected: usize) -> MentionPicker {
    MentionPicker {
        id: id.into(),
        query: query.into(),
        sections,
        selected,
        present: true,
        timing: EnterExit::QUICK,
        on_select: None,
        on_hover: None,
    }
}

impl MentionPicker {
    /// Whether the picker is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the picker is drawn at rest on its first frame, for a
    /// static composition (the design card).
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// A row was clicked; the argument is its [`MentionItem::id`].
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// The pointer entered a row; the argument is its index across all
    /// sections.
    pub fn on_hover(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for MentionPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);

        let counts: Vec<usize> = self.sections.iter().map(|s| s.items.len()).collect();
        let active = active_row(&id, "mention", &counts, self.selected, window, cx);
        let scroll = menu_scroll(&id, &counts, self.selected, window, cx);
        let mut pop = popover_frame(id.clone(), PresenceStyle::fade_rise_scale(sample, POP_RISE, POP_FROM_SCALE), &p, &scroll);

        let mut index = 0usize;
        for (s, section) in self.sections.into_iter().enumerate() {
            pop = pop.child(caps_header(&p, section.title));
            for item in section.items {
                let key: ElementId = (id.clone(), SharedString::from(format!("mention-{s}-{index}"))).into();
                pop = pop.child(mention_row(key, &p, item, &self.query, index, index == active, &self.on_select, &self.on_hover, window, cx));
                index += 1;
            }
        }
        pop
    }
}

/// `.pop`: the popover ground both menus share, at the width of its container
/// up to the dropdown ceiling, and lifted 8 px above the composer.
fn popover_frame(id: ElementId, style: PresenceStyle, p: &Palette, scroll: &ScrollHandle) -> Stateful<Div> {
    v_flex()
        .id(id)
        .relative()
        // The CSS enters from `translateY(6px)`: the popover rises out of the
        // composer's top edge onto its resting place.
        .top(style.offset_y)
        .opacity(style.opacity)
        // gpui has no element transform, so the .98 enter scale is a width
        // fraction of the container; the popover keeps its left edge, which is
        // the CSS `transform-origin:bottom left`, and never grows past
        // `MENU_W_MAX` however wide the composer is.
        .w(relative(style.scale))
        .max_w(px(MENU_W_MAX))
        // The menu is its own scroll container: past `MENU_MAX_H` the rows
        // scroll inside this frame instead of growing over the transcript.
        .max_h(px(MENU_MAX_H))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .mb(px(POP_GAP))
        .p(px(POP_PAD))
        .rounded(px(scale::R_LG))
        .border_1()
        .border_color(p.line_strong)
        .bg(p.overlay)
        .shadow(p.shadow(POP_SHADOW))
        // The open menu owns the wheel over it: it occludes the transcript
        // behind (no hover/click/scroll-through, like the chip pickers) and
        // stops the wheel on this same element — after the element's own
        // scroll handler, which is registered later and so runs first in the
        // bubble phase — so the menu scrolls and the transcript never does.
        .occlude()
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
}

/// What the menu asked the scroll container for: the child index and whether
/// it has been seen in view since. The handle only learns the overflow from
/// paint, so a first-frame `scroll_to_item` lands before it knows it scrolls
/// and is dropped; the flag below re-fires until the row arrives instead of
/// trusting the first request.
#[derive(Clone, Copy)]
struct MenuScroll {
    target: usize,
    settled: bool,
}

/// The scroll handle for one menu, scrolled so the active row is visible.
///
/// The request re-fires until the row is seen in view, then stops: a wheel
/// gesture that scrolls the selection out of view afterwards is never
/// snapped back on the next frame.
fn menu_scroll(id: &ElementId, counts: &[usize], active: usize, window: &mut Window, cx: &mut App) -> ScrollHandle {
    let scroll: ScrollHandle =
        window.use_keyed_state((id.clone(), "scroll"), cx, |_, _| ScrollHandle::new()).read(cx).clone();
    let state: Rc<RefCell<MenuScroll>> = window
        .use_keyed_state((id.clone(), "scrolled"), cx, |_, _| Rc::new(RefCell::new(MenuScroll { target: usize::MAX, settled: false })))
        .read(cx)
        .clone();
    let target = row_child_index(counts, active);
    let mut kept = state.borrow_mut();
    if kept.target != target {
        kept.target = target;
        kept.settled = false;
    }
    if !kept.settled {
        if row_visible(&scroll, target) {
            kept.settled = true;
        } else {
            scroll.scroll_to_item(target);
            // The re-fire above may itself be dropped (see [`MenuScroll`]);
            // ask for the frame that retries it.
            window.request_animation_frame();
        }
    }
    scroll
}

/// Whether the `target`th direct child is inside the scrolled viewport.
/// `false` while the container never painted (no bounds, no children yet),
/// which is exactly when the request needs (re-)firing.
fn row_visible(scroll: &ScrollHandle, target: usize) -> bool {
    if scroll.bounds_for_item(target).is_none() {
        return false;
    }
    target >= scroll.top_item() && target <= scroll.bottom_item()
}

/// The direct-child index of the `active`th row inside the popover: rows are
/// interrupted by one caps header per section (and the `/` menu ends in the
/// key-hint footer), so the child index leads the row index by the headers
/// before it.
fn row_child_index(counts: &[usize], active: usize) -> usize {
    let mut child = active + 1;
    let mut remaining = active;
    for count in counts {
        if remaining < *count {
            break;
        }
        remaining -= *count;
        child += 1;
    }
    child
}

/// `.pop .caps`: a section header.
fn caps_header(p: &Palette, title: SharedString) -> impl IntoElement {
    div()
        .flex_none()
        .pt(px(CAPS_PAD_TOP))
        .px(px(CAPS_PAD_X))
        .pb(px(CAPS_PAD_BOTTOM))
        .text_role(TextRole::Caps)
        .line_height(relative(scale::LH_UI))
        .text_color(p.ink_3)
        .child(title.to_uppercase())
}

/// The row the menu lights: the row under the pointer while there is one,
/// otherwise the caller's `selected`. Both menus read every row's hover flag
/// before any row is built, so the pointer and the arrow keys drive a single
/// highlight and the menu never shows two rows lit at once. `counts` is the
/// item count of each section, in order, and `prefix` the row key prefix.
fn active_row(id: &ElementId, prefix: &str, counts: &[usize], selected: usize, window: &mut Window, cx: &mut App) -> usize {
    let mut index = 0usize;
    let mut hovered = None;
    for (s, count) in counts.iter().enumerate() {
        for _ in 0..*count {
            let key: ElementId = (id.clone(), SharedString::from(format!("{prefix}-{s}-{index}"))).into();
            if interaction_flags(key, window, cx).1.hovered {
                hovered = Some(index);
            }
            index += 1;
        }
    }
    hovered.unwrap_or(selected)
}

/// `.it`: the row shell both menus share — 32 px, 10 px gaps, accent-soft when
/// it is the selected (or hovered) row, cross-fading over the hover duration
/// so the highlight travels between neighbours instead of flashing.
#[allow(clippy::too_many_arguments)]
fn menu_row(
    id: ElementId,
    p: &Palette,
    on: bool,
    index: usize,
    on_hover: &Option<HoverHandler>,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let (state, _) = interaction_flags(id.clone(), window, cx);
    let ground = tint_fade((id.clone(), "bg"), on, p.accent_soft, Tween::FAST, window, cx);
    let text = tween((id.clone(), "text"), if on { p.ink } else { p.ink_2 }, Tween::FAST, window, cx);
    let mut row = h_flex()
        .id(id)
        .flex_none()
        .w_full()
        .h(px(ROW_H))
        .gap(px(ROW_GAP))
        .px(px(ROW_PAD_X))
        .rounded(px(scale::R_SM))
        .bg(ground)
        .ui(ROW_TEXT)
        .text_color(text)
        .cursor_pointer();

    // gpui allows one `on_hover` per element, so the row's own hover tint and
    // the caller's hover intent share a single handler.
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
    row
}

/// `.it` in the `/` menu: the command column, the description, then the keycap
/// or the source tag.
#[allow(clippy::too_many_arguments)]
fn command_row(
    id: ElementId,
    p: &Palette,
    item: CommandItem,
    query: &SharedString,
    index: usize,
    on: bool,
    on_select: &Option<SelectHandler>,
    on_hover: &Option<HoverHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let mut row = menu_row(id, p, on, index, on_hover, window, cx);

    let matched = match item.command.find(query.as_ref()) {
        Some(at) if !query.is_empty() => Some(at..at + query.len()),
        _ => None,
    };
    row = row
        .child(
            // The name never wraps: a long skill name (`/browser-app-delivery`)
            // widens its own row's name column past `COMMAND_W` instead of
            // folding onto a second line inside a fixed-height row, which
            // is what drew rows over one another.
            div()
                .flex_none()
                .min_w(px(COMMAND_W))
                .whitespace_nowrap()
                .font_family(scale::FONT_MONO)
                .text_px(COMMAND_TEXT)
                .line_height(relative(COMMAND_LH))
                .medium()
                .text_color(if on { p.accent_ink } else { p.ink })
                .child(highlighted(p, &item.command, &matched)),
        )
        // One line: the row is `ROW_H` tall, so a description that outgrows
        // its column truncates (the full text belongs to `/help`), never
        // wraps onto the row below.
        .child(div().flex_1().min_w(px(0.0)).truncate().ui(DETAIL_TEXT).text_color(p.ink_3).child(item.description));

    if let Some(key) = item.key {
        row = row.child(h_flex().flex_none().gap(px(KEYS_GAP)).child(kbd(key)));
    }
    if let Some(source) = item.source_tag {
        row = row.child(tag(source));
    }
    if let Some(handler) = on_select.clone() {
        let item_id = item.id.clone();
        row = row.on_click(move |_, w, cx| handler(&item_id, w, cx));
    }
    row
}

/// `.it` in the `@` picker: the glyph, the name, the muted detail, then the
/// keycap.
#[allow(clippy::too_many_arguments)]
fn mention_row(
    id: ElementId,
    p: &Palette,
    item: MentionItem,
    query: &SharedString,
    index: usize,
    on: bool,
    on_select: &Option<SelectHandler>,
    on_hover: &Option<HoverHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let dot_id: ElementId = (id.clone(), "dot").into();
    let mut row = menu_row(id, p, on, index, on_hover, window, cx);

    row = match item.icon {
        MentionIcon::Glyph(name) => row.child(icon(name).size(px(ROW_ICON)).color(p.ink_3)),
        MentionIcon::Symbol => row.child(div().flex_none().mono(SYMBOL_TEXT).text_color(p.ink_3).child("ƒ")),
        MentionIcon::Dot(state) => row.child(status_dot(dot_id, state)),
    };

    let matched = if item.matched.is_empty() {
        match item.label.find(query.as_ref()) {
            Some(at) if !query.is_empty() => Some(at..at + query.len()),
            _ => None,
        }
    } else {
        Some(item.matched.clone())
    };
    // `.c{width:auto}` here: the name takes its natural width and the detail
    // line takes the rest of the row; under the dropdown ceiling a long name
    // shrinks and wraps instead of pushing the detail off the row.
    row = row.child(
        div()
            .flex_shrink_1()
            .min_w(px(0.0))
            .font_family(scale::FONT_MONO)
            .text_px(COMMAND_TEXT)
            .line_height(relative(COMMAND_LH))
            .medium()
            .text_color(if on { p.accent_ink } else { p.ink })
            .child(highlighted(p, &item.label, &matched)),
    );

    let detail = div().flex_1().min_w(px(0.0)).text_color(p.ink_3);
    row = row.child(if item.detail_mono {
        detail.mono(DETAIL_MONO_TEXT).child(item.detail)
    } else {
        detail.ui(DETAIL_TEXT).child(item.detail)
    });

    if let Some(key) = item.key {
        row = row.child(h_flex().flex_none().gap(px(KEYS_GAP)).child(kbd(key)));
    }
    if let Some(handler) = on_select.clone() {
        let item_id = item.id.clone();
        row = row.on_click(move |_, w, cx| handler(&item_id, w, cx));
    }
    row
}

/// `.it mark`: the typed characters, in accent-ink 600.
fn highlighted(p: &Palette, text: &SharedString, matched: &Option<Range<usize>>) -> StyledText {
    let highlight = HighlightStyle { color: Some(p.accent_ink), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    StyledText::new(text.clone()).with_highlights(matched.iter().map(|r| (r.clone(), highlight)).collect::<Vec<_>>())
}

/// `.ft`: the key hints under the `/` menu.
fn footer(p: &Palette) -> impl IntoElement {
    h_flex()
        .flex_none()
        .w_full()
        .gap(px(FT_GAP))
        .pt(px(FT_PAD_TOP))
        .px(px(FT_PAD_X))
        .pb(px(FT_PAD_BOTTOM))
        .mt(px(FT_MARGIN_TOP))
        .border_t_1()
        .border_color(p.line)
        .ui(FT_TEXT)
        .text_color(p.ink_3)
        .child(footer_hint("↑↓", "move"))
        .child(footer_hint("⇥", "complete"))
        .child(footer_hint("esc", "dismiss"))
}

/// One `<span class="kbd">…</span> word` footer hint.
fn footer_hint(key: &'static str, word: &'static str) -> impl IntoElement {
    h_flex().flex_none().gap(px(FT_KEY_GAP)).child(kbd(key)).child(word)
}

#[cfg(test)]
mod tests {
    use super::row_child_index;

    #[test]
    fn row_child_index_skips_section_headers() {
        // [caps, r0, r1, caps, r2, caps, r3, r4, footer]: the `/` menu shape.
        let counts = [2, 1, 2];
        assert_eq!(row_child_index(&counts, 0), 1);
        assert_eq!(row_child_index(&counts, 1), 2);
        assert_eq!(row_child_index(&counts, 2), 4);
        assert_eq!(row_child_index(&counts, 3), 6);
        assert_eq!(row_child_index(&counts, 4), 7);
    }

    #[test]
    fn row_child_index_handles_a_single_section() {
        assert_eq!(row_child_index(&[3], 0), 1);
        assert_eq!(row_child_index(&[3], 2), 3);
    }
}
