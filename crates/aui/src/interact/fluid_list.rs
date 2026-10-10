//! [`FluidList`]: Fluid Functionalism's fluid hover — one highlight per list
//! that glides to the nearest item on the fast spring (0.08 s, no bounce),
//! fades in place on a fresh pointer entry, and never blinks between rows.
//! The selected block springs on the moderate tier. List rows carry no press
//! geometry (FF: only buttons press).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use aui_motion::{spring_px, tint_fade, SpringKind, Tween};
use aui_tokens::{scale, ActiveAui, Easing, Tier};
use gpui::{
    div, prelude::*, px, AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, RenderOnce, Role, SharedString,
    Window,
};
use gpui_kit::base::ElementExt;

use crate::util::indexed_child;

/// Which axis the fluid highlight travels on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Axis {
    /// A vertical row list: the nearest row centre wins.
    #[default]
    Y,
    /// A horizontal strip: the nearest column centre wins.
    X,
    /// A grid: the nearest centre in both axes wins.
    XY,
}

/// The fluid-highlight rule as a pure function: the pointer-inside item
/// wins; otherwise the item whose centre is nearest (a pointer in a gap, in
/// the padding or past the last item still lands); ties keep the first;
/// empty is `None`.
///
/// `items` parallels the caller's item order — only built items, so a
/// virtualised `list()` passes the bounds it actually built this frame —
/// and the return is the position in that slice.
pub fn nearest(items: &[Bounds<Pixels>], pointer: Point<Pixels>, axis: Axis) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    if let Some(index) = items.iter().position(|bounds| bounds.contains(&pointer)) {
        return Some(index);
    }
    let unit = px(1.0);
    let mut best: Option<(usize, f32)> = None;
    for (index, bounds) in items.iter().enumerate() {
        let centre_x = bounds.origin.x + bounds.size.width * 0.5;
        let centre_y = bounds.origin.y + bounds.size.height * 0.5;
        let dx = (pointer.x - centre_x) / unit;
        let dy = (pointer.y - centre_y) / unit;
        let distance = match axis {
            Axis::Y => dy.abs(),
            Axis::X => dx.abs(),
            Axis::XY => dx * dx + dy * dy,
        };
        // Strictly less, so exact ties keep the first item.
        if best.is_none_or(|(_, known)| distance < known) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

/// Last-frame geometry plus the pointer session of one [`FluidList`].
#[derive(Default)]
struct FluidState {
    /// When the list last rendered, and whether a block was still gliding
    /// then: a glide interrupted by a stalled main thread (a modal prompt, a
    /// blocking call) snaps to its target on the next frame instead of
    /// resuming from a stale position, which drew two pills overlapping
    /// mid-flight (seen live 2026-10-10).
    last_frame: Option<std::time::Instant>,
    gliding: bool,
    /// Bumped on such a stall: it re-keys the springs, which start at
    /// their targets.
    epoch: u64,
    /// The `selected` prop as last rendered, and a counter bumped when the
    /// selection lands on the item the pointer is already lighting (a
    /// click): that move snaps — the hover pill is already there, and a
    /// selection gliding in from another row crossed it (seen live).
    last_selected: Option<usize>,
    select_snaps: u64,
    /// Items at or past this index live inside a scrolling region (a
    /// virtualised `gpui::list`) whose viewport is the second field: their
    /// blocks are clipped to it, so a row half scrolled under a header never
    /// paints its highlight over that header.
    clip: Option<(usize, Bounds<Pixels>)>,
    /// The container's bounds last frame (`None` before the first measure).
    container: Option<Bounds<Pixels>>,
    /// Item bounds measured last frame, keyed by item index. Drained at the
    /// top of every render and refilled by prepaint, so a virtualised list
    /// never highlights an item it did not build.
    bounds: HashMap<usize, Bounds<Pixels>>,
    /// The pointer position in window coordinates (`None` outside).
    pointer: Option<Point<Pixels>>,
    /// Bumped on every pointer entry: the hover tint is re-keyed
    /// with it, so a fresh entry fades in at the nearest item on the fast
    /// tier instead of sliding in from wherever it was.
    session: u64,
    /// Set with the session bump and consumed by the first frame that
    /// paints the re-keyed tint: that frame renders the tint clear, so the
    /// new transition starts at alpha 0 and fades in instead of popping.
    fresh: bool,
    /// The effective keyboard override this frame (see
    /// [`effective_keyboard`]), so the pointer handlers see the same
    /// highlight the render does.
    keyboard: Option<usize>,
    /// The `highlighted` prop as last seen: a changed prop (a new key
    /// press) re-arms the keyboard override.
    last_prop: Option<usize>,
    /// The pointer moved since the keyboard override was set: the pointer
    /// owns the highlight until the prop changes again.
    pointer_override: bool,
    /// The last highlight reported through `on_highlight`, so the pointer
    /// moving inside one item does not re-fire it.
    reported: Option<Option<usize>>,
    /// The last non-empty highlight and selection: the fade-out anchor when
    /// the pointer leaves or the selection clears.
    previous_highlight: Option<usize>,
    previous_selected: Option<usize>,
}

type HighlightHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;
type ActivateHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

struct FluidItem {
    index: usize,
    element: AnyElement,
    highlightable: bool,
    /// The wrap grows to fill the list's main axis (a virtualised
    /// `gpui::list` child needs this to get a height at all).
    fill: bool,
}

/// A handle to one [`FluidList`]'s geometry, owned by the caller like a
/// [`gpui::ListState`]: rows built OUTSIDE the list's own children — the
/// render closure of a virtualised `gpui::list` placed inside the list —
/// register their bounds through [`FluidHandle::item`], so the one
/// highlight sits on virtual rows exactly as on static ones.
///
/// Hand the same handle to [`FluidList::handle`]; the list then keeps its
/// pointer session, highlight and selection in the handle instead of in
/// window keyed state (which is scoped to the element path and cannot be
/// shared with a `list` closure).
#[derive(Clone, Default)]
pub struct FluidHandle {
    fluid: Rc<RefCell<FluidState>>,
}

impl FluidHandle {
    /// A fresh handle with no geometry yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wraps `element` as highlightable item `index` of the list that owns
    /// this handle: its bounds are recorded in prepaint every frame it is
    /// built, and forgotten the frame it is not (a virtualised row that
    /// scrolled away is never a highlight target).
    pub fn item(&self, index: usize, element: impl IntoElement) -> AnyElement {
        let fluid = self.fluid.clone();
        div()
            .id(("aui-fluid-ext", index))
            .relative()
            .w_full()
            .on_prepaint(move |item_bounds, _, _| {
                fluid.borrow_mut().bounds.insert(index, item_bounds);
            })
            .child(element)
            .into_any_element()
    }

    /// Item `index`'s bounds as last measured, in window coordinates —
    /// `None` when the item was not built last frame.
    pub fn bounds_of(&self, index: usize) -> Option<Bounds<Pixels>> {
        self.fluid.borrow().bounds.get(&index).copied()
    }

    /// Wraps the scrolling region that holds items `min_index..`: its
    /// laid-out bounds become their clip (see [`FluidState::clip`]).
    pub fn clip_region(&self, min_index: usize, element: impl IntoElement) -> AnyElement {
        let fluid = self.fluid.clone();
        div()
            .id(("aui-fluid-clip", min_index))
            .size_full()
            .on_prepaint(move |bounds, _, _| {
                fluid.borrow_mut().clip = Some((min_index, bounds));
            })
            .child(element)
            .into_any_element()
    }

    /// The pointer-derived highlight as of the last event, if any.
    pub fn highlight(&self) -> Option<usize> {
        self.fluid.borrow().reported.flatten()
    }
}

/// One highlight per list that sits on the nearest item.
///
/// Children arrive through [`FluidList::item`] (measured and highlightable)
/// or [`FluidList::item_static`] (measured for layout, never highlighted —
/// a divider or a disabled row). The list renders them in a `relative`
/// container with two absolutely positioned overlays painted FIRST, so they
/// sit under the items: the hover highlight (`hover_overlay`, fast fade,
/// placed on its item) and the selected block (`selected_overlay`), plus
/// nothing else: rows carry no press geometry.
///
/// The pointer-inside item wins; otherwise the nearest centre, so a pointer
/// in a gap, in the padding or past the last item still lands. Keyboard
/// navigation drives the same highlight through
/// [`FluidList::highlighted`]: there is no separate focus ring inside a
/// list. A gap click activates the highlighted item. Build with
/// [`fluid_list`].
#[derive(IntoElement)]
pub struct FluidList {
    id: ElementId,
    axis: Axis,
    items: Vec<FluidItem>,
    selected: Option<usize>,
    highlighted: Option<usize>,
    gap_click: bool,
    label: Option<SharedString>,
    on_highlight: Option<HighlightHandler>,
    on_activate: Option<ActivateHandler>,
    gap: Option<Pixels>,
    handle: Option<FluidHandle>,
    fill: bool,
    block_inset_y: Pixels,
}

/// A [`FluidList`] over `axis` with `id`.
pub fn fluid_list(id: impl Into<ElementId>, axis: Axis) -> FluidList {
    FluidList {
        id: id.into(),
        axis,
        items: Vec::new(),
        selected: None,
        highlighted: None,
        gap_click: false,
        label: None,
        on_highlight: None,
        on_activate: None,
        gap: None,
        handle: None,
        fill: false,
        block_inset_y: px(0.0),
    }
}

impl FluidList {
    /// Adds a highlightable item: measured in prepaint, eligible for the
    /// highlight, reported and activated by index.
    pub fn item(mut self, index: usize, element: impl IntoElement) -> Self {
        self.items.push(FluidItem { index, element: element.into_any_element(), highlightable: true, fill: false });
        self
    }

    /// Adds a static item that grows to fill the list's main axis: the
    /// slot for a virtualised `gpui::list` whose rows register through a
    /// [`FluidHandle`]. Never highlighted itself.
    pub fn item_static_fill(mut self, index: usize, element: impl IntoElement) -> Self {
        self.items.push(FluidItem { index, element: element.into_any_element(), highlightable: false, fill: true });
        self
    }

    /// The gap between items (default `metrics.gap`); a row list on the
    /// ladder passes zero so rows touch.
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = Some(gap);
        self
    }

    /// Insets the hover and selected blocks this much from the top and
    /// bottom of their item (paint only; the rows still touch), so two lit
    /// neighbours keep a gap of twice it between their pills, as Claude
    /// Desktop's sidebar does.
    pub fn block_inset_y(mut self, inset: Pixels) -> Self {
        self.block_inset_y = inset;
        self
    }

    /// The list grows to fill its parent's main axis (so a
    /// [`FluidList::item_static_fill`] slot inside it can fill too).
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Keeps the list's geometry in `handle` instead of window keyed
    /// state, so rows built elsewhere ([`FluidHandle::item`]) share it.
    pub fn handle(mut self, handle: &FluidHandle) -> Self {
        self.handle = Some(handle.clone());
        self
    }

    /// Adds a static item: measured for layout like [`FluidList::item`],
    /// but never highlighted, reported or activated (a divider, a section
    /// header, a disabled row).
    pub fn item_static(mut self, index: usize, element: impl IntoElement) -> Self {
        self.items.push(FluidItem { index, element: element.into_any_element(), highlightable: false, fill: false });
        self
    }

    /// The selected item: a second overlay placed on that item.
    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// The keyboard-driven highlight override. When set it wins over the
    /// pointer: the same highlight moves there, and there is no separate
    /// focus ring inside the list.
    pub fn highlighted(mut self, highlighted: Option<usize>) -> Self {
        self.highlighted = highlighted;
        self
    }

    /// Whether a click anywhere in the container (a gap, the padding)
    /// activates the highlighted item (default false).
    pub fn gap_click(mut self, gap_click: bool) -> Self {
        self.gap_click = gap_click;
        self
    }

    /// The human label of the list (its accessible name).
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Fires when the pointer-driven highlight changes. Keyboard moves
    /// arrive through [`FluidList::highlighted`], which the parent already
    /// owns, so they are adopted silently.
    pub fn on_highlight(mut self, f: impl Fn(Option<usize>, &mut Window, &mut App) + 'static) -> Self {
        self.on_highlight = Some(Rc::new(f));
        self
    }

    /// Fires with the highlighted item on a gap click (and on a click over
    /// an item, which is its own nearest). Items carry no click handlers of
    /// their own: activation flows through here.
    pub fn on_activate(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }
}

/// The highlight for `keyboard` / `pointer` over the parallel `keys` /
/// `rects` slices, as an item index. A keyboard override for an unbuilt
/// item yields nothing: the highlight waits for the item to exist.
fn highlight_for(
    keys: &[usize],
    rects: &[Bounds<Pixels>],
    keyboard: Option<usize>,
    pointer: Option<Point<Pixels>>,
    axis: Axis,
) -> Option<usize> {
    if let Some(forced) = keyboard {
        return keys.contains(&forced).then_some(forced);
    }
    pointer.and_then(|at| nearest(rects, at, axis).and_then(|position| keys.get(position).copied()))
}

/// Resolves the keyboard override for this frame. The `highlighted` prop
/// wins while it is unchanged and the pointer has not moved since it was
/// set; a changed prop (a new key press) re-arms it; real pointer movement
/// ([`pointer_moved`]) hands the highlight back to the pointer.
fn effective_keyboard(state: &mut FluidState, prop: Option<usize>) -> Option<usize> {
    if state.last_prop != prop {
        state.last_prop = prop;
        state.pointer_override = false;
    }
    let keyboard = if state.pointer_override { None } else { prop };
    state.keyboard = keyboard;
    keyboard
}

/// Records a real pointer movement: the pointer takes the highlight over
/// from any keyboard override until the prop changes again.
fn pointer_moved(state: &mut FluidState, at: Point<Pixels>) {
    state.pointer = Some(at);
    if state.keyboard.is_some() {
        state.keyboard = None;
        state.pointer_override = true;
    }
}

/// Whether the hover tint paints `on` this frame. A fresh session (the
/// pointer just entered) renders one clear frame first: a newly keyed
/// transition initialises AT its target, so without the clear frame the
/// highlight would pop in instead of fading.
fn tint_on(highlight: bool, fresh: bool) -> bool {
    highlight && !fresh
}

/// The item a container click activates: with `gap_click` the highlighted
/// item wherever the click lands; without it only a click over an item
/// (its own nearest), a gap or padding click does nothing.
fn click_target(state: &FluidState, gap_click: bool, axis: Axis) -> Option<usize> {
    if !gap_click {
        if let Some(at) = state.pointer {
            if !state.bounds.values().any(|rect| rect.contains(&at)) {
                return None;
            }
        }
    }
    state_highlight(state, axis)
}

/// A gap between frames longer than this, with a block still gliding, is a
/// stall: the glide snaps instead of resuming.
const STALL: std::time::Duration = std::time::Duration::from_millis(50);

/// Clamps a length at zero: spring overshoot must never flip an overlay
/// into a negative size.
fn non_negative(value: Pixels) -> Pixels {
    if value > px(0.0) { value } else { px(0.0) }
}

/// The highlight computed from the keyed state alone (bounds in index
/// order, so ties keep the first item), for the pointer handlers.
fn state_highlight(state: &FluidState, axis: Axis) -> Option<usize> {
    let mut indices: Vec<usize> = state.bounds.keys().copied().collect();
    indices.sort();
    let mut keys = Vec::with_capacity(indices.len());
    let mut rects = Vec::with_capacity(indices.len());
    for index in indices {
        if let Some(rect) = state.bounds.get(&index) {
            keys.push(index);
            rects.push(*rect);
        }
    }
    highlight_for(&keys, &rects, state.keyboard, state.pointer, axis)
}

/// Records `next` as the current highlight; true when it differs from the
/// last report (so the caller fires `on_highlight`).
fn note_change(state: &mut FluidState, next: Option<usize>) -> bool {
    let changed = state.reported != Some(next);
    if changed {
        state.reported = Some(next);
        if next.is_some() {
            state.previous_highlight = next;
        }
    }
    changed
}

/// Records `next` and reports it when it differs from the last report.
fn report_change(
    fluid: &Rc<RefCell<FluidState>>,
    report: &Option<HighlightHandler>,
    next: Option<usize>,
    window: &mut Window,
    cx: &mut App,
) {
    let changed = note_change(&mut fluid.borrow_mut(), next);
    if changed {
        if let Some(report) = report {
            report(next, window, cx);
        }
    }
}

impl RenderOnce for FluidList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let gap = self.gap.unwrap_or(cx.aui().metrics.gap);
        let id = self.id.clone();
        let axis = self.axis;
        let keyboard = self.highlighted;
        let selected = self.selected;
        let gap_click = self.gap_click;
        let inset_y = self.block_inset_y;

        let fluid = match self.handle.as_ref() {
            Some(handle) => handle.fluid.clone(),
            None => window
                .use_keyed_state((id.clone(), "aui-fluid"), cx, |_, _| Rc::new(RefCell::new(FluidState::default())))
                .read(cx)
                .clone(),
        };

        // Drain last frame's bounds: prepaint refills the map with exactly
        // the items built this frame, so unbuilt (virtualised-away) items
        // are never highlight targets.
        let (container, bounds, pointer, session, keyboard, clip) = {
            let mut state = fluid.borrow_mut();
            let keyboard = effective_keyboard(&mut state, keyboard);
            if keyboard.is_some() {
                state.reported = Some(keyboard);
            }
            let drained = std::mem::take(&mut state.bounds);
            (state.container, drained, state.pointer, state.session, keyboard, state.clip)
        };

        // Parallel highlightable order and rects for the pure function:
        // every item that recorded bounds last frame, in index order — the
        // list's own highlightable children and the rows a
        // [`FluidHandle`] registered from a virtualised list alike (static
        // items never record bounds, so they never enter the geometry).
        let mut indices: Vec<usize> = bounds.keys().copied().collect();
        indices.sort_unstable();
        let mut keys = Vec::with_capacity(indices.len());
        let mut rects = Vec::with_capacity(indices.len());
        for index in indices {
            if let Some(rect) = bounds.get(&index) {
                keys.push(index);
                rects.push(*rect);
            }
        }
        let highlight = highlight_for(&keys, &rects, keyboard, pointer, axis);
        // The pointer may be still while the rows move under it (a wheel
        // scroll of a virtualised list): the overlay follows at once, and
        // the parent hears about the new row on the next frame, never
        // from inside render.
        if pointer.is_some() && keyboard.is_none() && fluid.borrow().reported != Some(highlight) {
            if let Some(report) = self.on_highlight.clone() {
                let fluid = fluid.clone();
                window.on_next_frame(move |window, cx| {
                    let next = state_highlight(&fluid.borrow(), axis);
                    report_change(&fluid, &Some(report), next, window, cx);
                });
            }
        }
        let (previous, previous_selected) = {
            let mut state = fluid.borrow_mut();
            if highlight.is_some() {
                state.previous_highlight = highlight;
            }
            if selected.is_some() {
                state.previous_selected = selected;
            }
            (state.previous_highlight, state.previous_selected)
        };

        // One session string per frame: it re-keys the hover springs so a
        // fresh pointer entry fades in at the nearest item instead of
        // sliding over from wherever it was (FF `key={session}`).
        let epoch = {
            let mut state = fluid.borrow_mut();
            let now = std::time::Instant::now();
            let stalled = state.last_frame.is_some_and(|last| now.duration_since(last) > STALL);
            if stalled && state.gliding {
                state.epoch += 1;
            }
            state.last_frame = Some(now);
            state.epoch
        };
        let snaps = {
            let mut state = fluid.borrow_mut();
            if state.last_selected != selected {
                if selected.is_some() && selected == highlight {
                    state.select_snaps += 1;
                }
                state.last_selected = selected;
            }
            state.select_snaps
        };
        let sess: SharedString = SharedString::from(format!("session-{session}-{epoch}"));
        let sel_key: SharedString = SharedString::from(format!("sel-{epoch}-{snaps}"));
        let mut gliding = false;
        let enter = Tween::new(Tier::Fast.enter(), Easing::STD);
        let mut overlays = Vec::with_capacity(2);
        if let Some(frame) = container {
            let origin = frame.origin;
            // Rect relative to the list, clipped to the list's own frame: a
            // row half scrolled out of a virtualised list must not paint its
            // highlight over whatever sits below the list (seen live).
            let relative = |index: usize, rect: Bounds<Pixels>| {
                let mut rect = rect.intersect(&frame);
                // Inset before the clip, so a pill half under a header keeps
                // its own edge where it is cut.
                rect.origin.y += inset_y;
                rect.size.height = non_negative(rect.size.height - inset_y * 2.0);
                rect = rect.intersect(&frame);
                if let Some((min, region)) = clip {
                    if index >= min {
                        rect = rect.intersect(&region);
                    }
                }
                (rect.origin.x - origin.x, rect.origin.y - origin.y, rect.size.width, rect.size.height)
            };
            // Hover (FF fluid hover): glides between rows on the fast spring
            // (0.08 s, no bounce); opacity fades 0.08 s in, the fast exit out.
            // The previous anchor keeps the fade-out painted where it left.
            let anchor = highlight.or(previous).and_then(|index| bounds.get(&index).map(|rect| (index, *rect)));
            if let Some((index, rect)) = anchor {
                let (x, y, w, h) = relative(index, rect);
                // The first frame of a fresh session paints the re-keyed
                // tint clear and asks for another frame, which fades it in.
                let fresh = std::mem::take(&mut fluid.borrow_mut().fresh);
                if fresh {
                    window.request_animation_frame();
                }
                let tint = tint_fade(((id.clone(), "aui-hl"), sess.clone()), tint_on(highlight.is_some(), fresh), p.hover_overlay(), enter, window, cx);
                let x_to = x;
                let x = spring_px(((id.clone(), "aui-hl-x"), sess.clone()), x, SpringKind::Fast, window, cx);
                gliding |= (x - x_to).abs() > px(0.5);
                let y_to = y;
                let y = spring_px(((id.clone(), "aui-hl-y"), sess.clone()), y, SpringKind::Fast, window, cx);
                gliding |= (y - y_to).abs() > px(0.5);
                let w_to = w;
                let w = spring_px(((id.clone(), "aui-hl-w"), sess.clone()), w, SpringKind::Fast, window, cx);
                gliding |= (w - w_to).abs() > px(0.5);
                let h_to = h;
                let h = spring_px(((id.clone(), "aui-hl-h"), sess.clone()), h, SpringKind::Fast, window, cx);
                gliding |= (h - h_to).abs() > px(0.5);
                overlays.push(div().absolute().left(x).top(y).w(non_negative(w)).h(non_negative(h)).rounded(px(scale::SHAPE_ITEM)).bg(tint));
            }
            // Selected (FF `bg-active`): springs between rows on the moderate
            // tier when the selection moves; a cleared selection fades out
            // where it was.
            let selected_anchor = selected.or(previous_selected).and_then(|index| bounds.get(&index).map(|rect| (index, *rect)));
            if let Some((index, rect)) = selected_anchor {
                let (x, y, w, h) = relative(index, rect);
                let tint = tint_fade((id.clone(), "aui-sel"), selected.is_some(), p.selected_overlay(), enter, window, cx);
                let x_to = x;
                let x = spring_px(((id.clone(), "aui-sel-x"), sel_key.clone()), x, SpringKind::Moderate, window, cx);
                gliding |= (x - x_to).abs() > px(0.5);
                let y_to = y;
                let y = spring_px(((id.clone(), "aui-sel-y"), sel_key.clone()), y, SpringKind::Moderate, window, cx);
                gliding |= (y - y_to).abs() > px(0.5);
                let w_to = w;
                let w = spring_px(((id.clone(), "aui-sel-w"), sel_key.clone()), w, SpringKind::Moderate, window, cx);
                gliding |= (w - w_to).abs() > px(0.5);
                let h_to = h;
                let h = spring_px(((id.clone(), "aui-sel-h"), sel_key.clone()), h, SpringKind::Moderate, window, cx);
                gliding |= (h - h_to).abs() > px(0.5);
                overlays.push(div().absolute().left(x).top(y).w(non_negative(w)).h(non_negative(h)).rounded(px(scale::SHAPE_ITEM)).bg(tint));
            }
        } else {
            window.request_animation_frame();
        }
        fluid.borrow_mut().gliding = gliding;

        let mut list = div().id(id.clone()).relative().flex().gap(gap).role(Role::List);
        if self.fill {
            list = list.flex_1().min_h(px(0.0)).w_full();
        }
        list = match axis {
            Axis::Y => list.flex_col(),
            Axis::X => list.flex_row(),
            Axis::XY => list.flex_row().flex_wrap(),
        };
        if let Some(label) = self.label {
            list = list.aria_label(label);
        }
        // The overlays paint first, so they sit under the items.
        for overlay in overlays {
            list = list.child(overlay);
        }
        for item in self.items {
            let index = item.index;
            let highlightable = item.highlightable;
            let fluid = fluid.clone();
            // Wraps are stateful (roles need the stateful element), keyed
            // off the list id so virtualised items keep stable state.
            let mut wrap = div().id(indexed_child(&id, "item-", index)).relative().role(Role::ListItem);
            if item.fill {
                wrap = wrap.flex_1().min_h(px(0.0)).w_full();
            }
            if highlightable {
                // Highlightable items record their bounds for the nearest
                // rule. Static items take part in layout but stay out of
                // the highlight geometry entirely.
                wrap = wrap.on_prepaint(move |item_bounds, _, _| {
                    fluid.borrow_mut().bounds.insert(index, item_bounds);
                });
            }
            list = list.child(wrap.child(item.element));
        }
        // The container measures itself after the items.
        {
            let fluid = fluid.clone();
            list = list.on_prepaint(move |container_bounds, _, _| {
                fluid.borrow_mut().container = Some(container_bounds);
            });
        }
        list = list
            .on_mouse_move({
                let fluid = fluid.clone();
                let report = self.on_highlight.clone();
                move |event: &gpui::MouseMoveEvent, window: &mut Window, cx: &mut App| {
                    // Real pointer movement takes the highlight over from
                    // a keyboard override and reports where it landed.
                    pointer_moved(&mut fluid.borrow_mut(), event.position);
                    let next = state_highlight(&fluid.borrow(), axis);
                    // Pointer geometry lives in window keyed state rather
                    // than an entity, so refresh the window directly.
                    window.refresh();
                    report_change(&fluid, &report, next, window, cx);
                }
            })
            .on_hover({
                let fluid = fluid.clone();
                let report = self.on_highlight.clone();
                move |hovered: &bool, window: &mut Window, cx: &mut App| {
                    if *hovered {
                        let mut state = fluid.borrow_mut();
                        state.session += 1;
                        state.fresh = true;
                        drop(state);
                        window.refresh();
                    } else {
                        fluid.borrow_mut().pointer = None;
                        let next = state_highlight(&fluid.borrow(), axis);
                        window.refresh();
                        report_change(&fluid, &report, next, window, cx);
                    }
                }
            });
        // Activation flows through the container: items carry no click
        // handlers of their own. A click on an item is its own nearest;
        // with `gap_click` a click in a gap or the padding activates the
        // highlighted item instead; without it, gap clicks do nothing.
        if let Some(activate) = self.on_activate {
            let fluid = fluid.clone();
            list = list.on_click(move |_, window: &mut Window, cx: &mut App| {
                let current = click_target(&fluid.borrow(), gap_click, axis);
                if let Some(index) = current {
                    activate(index, window, cx);
                }
            });
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(y: f32) -> Bounds<Pixels> {
        Bounds { origin: Point { x: px(0.0), y: px(y) }, size: gpui::Size { width: px(200.0), height: px(36.0) } }
    }

    fn at(x: f32, y: f32) -> Point<Pixels> {
        Point { x: px(x), y: px(y) }
    }

    /// Three built rows at y 0 / 44 / 88 (an 8 px gap).
    fn built() -> FluidState {
        let mut state = FluidState::default();
        for index in 0..3 {
            state.bounds.insert(index, rect(index as f32 * 44.0));
        }
        state
    }

    #[test]
    fn keyboard_override_waits_for_an_unbuilt_item() {
        let keys = [0, 1, 2];
        let rects: Vec<_> = (0..3).map(|i| rect(i as f32 * 44.0)).collect();
        assert_eq!(highlight_for(&keys, &rects, Some(1), Some(at(10.0, 10.0)), Axis::Y), Some(1));
        assert_eq!(highlight_for(&keys, &rects, Some(7), Some(at(10.0, 10.0)), Axis::Y), None);
        assert_eq!(highlight_for(&keys, &rects, None, Some(at(10.0, 10.0)), Axis::Y), Some(0));
        assert_eq!(highlight_for(&keys, &rects, None, None, Axis::Y), None);
    }

    #[test]
    fn pointer_movement_takes_over_from_the_keyboard() {
        let mut state = built();
        // A key press: the parent sets the prop; it wins over the pointer.
        assert_eq!(effective_keyboard(&mut state, Some(2)), Some(2));
        state.pointer = Some(at(10.0, 10.0));
        assert_eq!(state_highlight(&state, Axis::Y), Some(2));
        // The pointer moves: it owns the highlight again, and the override
        // stays cleared on the next render with the unchanged prop.
        pointer_moved(&mut state, at(10.0, 12.0));
        assert_eq!(state_highlight(&state, Axis::Y), Some(0));
        assert_eq!(effective_keyboard(&mut state, Some(2)), None);
        // A new key press (changed prop) re-arms the override.
        assert_eq!(effective_keyboard(&mut state, Some(1)), Some(1));
        assert_eq!(state_highlight(&state, Axis::Y), Some(1));
        // Clearing the prop clears the override without a pointer move.
        assert_eq!(effective_keyboard(&mut state, None), None);
        assert_eq!(state_highlight(&state, Axis::Y), Some(0));
    }

    #[test]
    fn pointer_movement_is_reported_after_a_keyboard_override() {
        let mut state = built();
        effective_keyboard(&mut state, Some(2));
        state.reported = Some(Some(2));
        pointer_moved(&mut state, at(10.0, 50.0));
        let next = state_highlight(&state, Axis::Y);
        assert_eq!(next, Some(1));
        assert!(note_change(&mut state, next), "the pointer-derived highlight reaches the parent");
        assert!(!note_change(&mut state, next), "moving inside the same item does not re-fire");
        assert!(note_change(&mut state, None), "leaving reports none");
    }

    #[test]
    fn a_click_after_moving_activates_the_pointer_item_not_the_keyboard_one() {
        let mut state = built();
        effective_keyboard(&mut state, Some(2));
        pointer_moved(&mut state, at(10.0, 10.0));
        assert_eq!(click_target(&state, false, Axis::Y), Some(0));
    }

    #[test]
    fn gap_click_rule() {
        let mut state = built();
        // In the gap between rows 0 and 1, nearer row 1.
        state.pointer = Some(at(10.0, 42.0));
        assert_eq!(click_target(&state, false, Axis::Y), None, "no gap_click: a gap click does nothing");
        assert_eq!(click_target(&state, true, Axis::Y), Some(1), "gap_click: the nearest item activates");
        // Over an item: its own nearest either way.
        state.pointer = Some(at(10.0, 100.0));
        assert_eq!(click_target(&state, false, Axis::Y), Some(2));
        assert_eq!(click_target(&state, true, Axis::Y), Some(2));
    }

    #[test]
    fn a_fresh_session_renders_one_clear_frame_before_fading_in() {
        // Frame 1 after entry: fresh → clear, so the re-keyed transition
        // initialises at alpha 0. Frame 2: on → fades in.
        assert!(!tint_on(true, true));
        assert!(tint_on(true, false));
        assert!(!tint_on(false, false));
        let mut state = FluidState::default();
        state.session += 1;
        state.fresh = true;
        assert!(std::mem::take(&mut state.fresh));
        assert!(!state.fresh, "consumed by the first painted frame");
    }

}
