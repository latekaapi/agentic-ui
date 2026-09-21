//! Card 50 (bottom dock): the terminal tab strip and the dock frame around it
//! (decisions D42–D43, spec §5.7).
//!
//! [`terminal_tabs`] is the stateless strip of per-project terminal tabs: each
//! tab shows its title, an agent mark, a busy dot and a hover close control,
//! with a trailing `+`. [`terminal_dock`] is the frame: the shell's
//! [`resize_handle`](crate::shell::resize_handle) along the top edge, a header
//! cell seating caller-built tabs beside the hint text and the maximize /
//! close buttons, and the body — or the empty state when the host has no
//! terminal yet.
//!
//! Like every other workbench component these are pure: data in, intents out.
//! No timers, no I/O, no `Entity`. The host owns the tabs, the body, the dock
//! height and the open state, and wires the resize drag callbacks the way
//! [`crate::shell::resize_handle`] documents.
//!
//! ```ignore
//! terminal_dock("dock", terminal_tabs("tabs", tabs, active), Some(block_terminal("term", blocks)))
//!     .hint("Muse can type here")
//!     .on_action(|action, _, _| match action {
//!         TerminalDockAction::Maximize => { /* toggle the host height */ }
//!         TerminalDockAction::Close => { /* close the dock */ }
//!         TerminalDockAction::NewTerminal => { /* open a tab */ }
//!     })
//! ```

use std::{cell::RefCell, rc::Rc};

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_motion::{spring_px, tween, SpringKind, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, Bounds, ElementId, IntoElement, Pixels, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex, ElementExt};

use crate::data::{button, icon_button, icon_content_button, ButtonSize};
use crate::shell::{resize_handle, RESIZE_HANDLE_W};
use crate::util::{interaction_flags, TrackInteraction};

/// Tabs share the shell strip's tab anatomy: 7 px gaps, 12 px side padding,
/// 12 px type.
const TAB_GAP: f32 = 7.0;
const TAB_PAD: f32 = 12.0;
/// The strip frame matches the shell strip too: 6 px side padding, 2 px gaps
/// and the 2 px ink indicator under the active tab, inset 8 px from the tab
/// edges. (The shell strip's own numbers, kept in step with it.)
const STRIP_PAD: f32 = 6.0;
const STRIP_GAP: f32 = scale::SP_1;
const INDICATOR_H: f32 = scale::SP_1;
const INDICATOR_INSET: f32 = scale::SP_3;
/// The agent mark in a tab: 12 px, the strip's glyph size.
const TAB_MARK: f32 = 12.0;
/// The busy dot while a command runs: 6 px in ink, like every tab indicator
/// in the design (never accent).
const BUSY_DOT: f32 = 6.0;
/// The close affordance: the shell strip's 14 px box (radius 3) with a 10 px x.
const CLOSE_SIZE: f32 = 14.0;
const CLOSE_RADIUS: f32 = 3.0;
const CLOSE_ICON: f32 = 10.0;
/// Header action glyphs: 12 px inside 20 px ghost buttons.
const ACTION_GLYPH: f32 = 12.0;

// ---------------------------------------------------------------------------
// Tab strip
// ---------------------------------------------------------------------------

/// One terminal tab: the data [`terminal_tabs`] draws.
#[derive(Debug, Clone, PartialEq)]
pub struct TermTab {
    /// Stable id (keys the tab's elements; intents report indices).
    pub id: SharedString,
    /// The tab title.
    pub title: SharedString,
    /// Which agent owns the tab, if any. Drawn as the provider mark — the same
    /// treatment as `TermBlock::agent`, reused here rather than inventing a
    /// second author type.
    pub agent: Option<Provider>,
    /// A command is running: draws the accent busy dot.
    pub busy: bool,
}

impl TermTab {
    /// A tab with a title and nothing running.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self { id: id.into(), title: title.into(), agent: None, busy: false }
    }

    /// Marks the tab as agent-owned (draws the provider mark).
    pub fn agent(mut self, provider: Provider) -> Self {
        self.agent = Some(provider);
        self
    }

    /// Whether a command is running (draws the busy dot).
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }
}

/// What the tab strip asks the host to do. Indices count into the tab list
/// the strip was built with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTabsAction {
    /// The tab at the index was clicked.
    Select(usize),
    /// The tab's close control at the index was clicked.
    Close(usize),
    /// The trailing `+` was clicked.
    New,
}

type TabsHandler = Rc<dyn Fn(TerminalTabsAction, &mut Window, &mut App)>;

/// Previous-frame geometry the indicator is positioned from: the shell
/// strip's own mechanism, so the two strips move as one.
#[derive(Default)]
struct StripGeometry {
    strip: Option<Bounds<Pixels>>,
    tabs: Vec<Option<Bounds<Pixels>>>,
}

/// The terminal tab strip. Build with [`terminal_tabs`].
#[derive(IntoElement)]
pub struct TerminalTabs {
    id: ElementId,
    tabs: Vec<TermTab>,
    active: usize,
    on_action: Option<TabsHandler>,
}

/// A strip over `tabs` with `active` selected. It looks and moves like the
/// shell strip ([`crate::shell::tab_strip`]): same tab anatomy, same 2 px ink
/// indicator sliding under the active tab on the swap spring, and tabs keep
/// their natural width — a strip short of room clips at its edge. An
/// out-of-range `active` selects nothing. Seat it in the dock header (or any
/// fixed-height row); it fills the row's height the way the shell strip does.
/// The dock header cell owns the bottom hairline, so the strip draws none.
pub fn terminal_tabs(id: impl Into<ElementId>, tabs: Vec<TermTab>, active: usize) -> TerminalTabs {
    TerminalTabs { id: id.into(), tabs, active, on_action: None }
}

impl TerminalTabs {
    /// Called with every intent the strip emits.
    pub fn on_action(mut self, f: impl Fn(TerminalTabsAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TerminalTabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let active = self.active;

        // The indicator is placed from last frame's bounds; the first frame
        // draws none and asks for another frame. The captured strip bounds are
        // its border box while an absolute `left` is measured from the content
        // box, so the strip's own padding is taken back out — the shell
        // strip's own arithmetic.
        let geometry = window
            .use_keyed_state((id.clone(), "geometry"), cx, |_, _| Rc::new(RefCell::new(StripGeometry::default())))
            .read(cx)
            .clone();
        let target = {
            let g = geometry.borrow();
            match (g.strip, g.tabs.get(active).copied().flatten()) {
                (Some(strip), Some(tab)) => Some((
                    tab.origin.x - strip.origin.x + px(INDICATOR_INSET - STRIP_PAD),
                    tab.size.width - px(2.0 * INDICATOR_INSET),
                )),
                _ => None,
            }
        };
        let indicator = match target {
            Some((left, width)) => {
                let left = spring_px((id.clone(), "indicator-left"), left, SpringKind::Swap, window, cx);
                let width = spring_px((id.clone(), "indicator-width"), width, SpringKind::Swap, window, cx);
                Some(
                    div()
                        .absolute()
                        .bottom(px(0.0))
                        .left(left)
                        .w(width.max(px(0.0)))
                        .h(px(INDICATOR_H))
                        .rounded_t(px(INDICATOR_H))
                        .bg(p.ink)
                        .into_any_element(),
                )
            }
            // No target, no animation (D16): an empty strip, or an
            // out-of-range `active` selecting nothing, has nothing to slide
            // toward, so asking for a frame here redraws the strip forever.
            // A strip with tabs still gets its measuring frame while the
            // geometry is missing; the springs drive every frame after that.
            None => {
                if self.tabs.get(active).is_some() {
                    window.request_animation_frame();
                }
                None
            }
        };

        let mut strip = h_flex()
            .id((id.clone(), "strip"))
            .relative()
            .h(cx.aui().metrics.tab_strip)
            .flex_1()
            .min_w(px(0.0))
            .gap(px(STRIP_GAP))
            .px(px(STRIP_PAD))
            .on_prepaint({
                let geometry = geometry.clone();
                move |bounds, _, _| geometry.borrow_mut().strip = Some(bounds)
            });
        let count = self.tabs.len();
        for (index, tab) in self.tabs.iter().enumerate() {
            let tab_id: ElementId = (id.clone(), tab.id.clone()).into();
            let active = index == self.active;
            let (state, flags) = interaction_flags(tab_id.clone(), window, cx);
            let color = tween((tab_id.clone(), "ink"), if active || flags.hovered { p.ink } else { p.ink_3 }, Tween::FAST, window, cx);
            let show_close = active || flags.hovered;
            let close_opacity = tween((tab_id.clone(), "close"), if show_close { 1.0f32 } else { 0.0 }, Tween::FAST, window, cx);
            // Tabs keep their natural width; a strip short of room clips at
            // its edge, like the shell strip. The active tab takes the body
            // ground so it merges with the pane below; inactive tabs are
            // transparent.
            let mut el = h_flex()
                .id(tab_id.clone())
                .relative()
                .h_full()
                .flex_none()
                .gap(px(TAB_GAP))
                .px(px(TAB_PAD))
                .text_color(color)
                .ui(scale::FS_12)
                .whitespace_nowrap()
                .cursor_pointer()
                .track_interaction(&state)
                .when(active, |d| d.bg(p.term_bg))
                .on_prepaint({
                    let geometry = geometry.clone();
                    move |bounds, _, _| {
                        let mut g = geometry.borrow_mut();
                        if g.tabs.len() < count {
                            g.tabs.resize(count, None);
                        }
                        g.tabs[index] = Some(bounds);
                    }
                });
            if let Some(provider) = tab.agent {
                el = el.child(provider_mark(provider).size(px(TAB_MARK)));
            }
            el = el.child(tab.title.clone());
            if tab.busy {
                el = el.child(div().flex_none().size(px(BUSY_DOT)).rounded_full().bg(p.ink));
            }
            let mut close = div()
                .id((tab_id.clone(), "close"))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .size(px(CLOSE_SIZE))
                .rounded(px(CLOSE_RADIUS))
                .opacity(close_opacity)
                .hover(|s| s.bg(p.surface_3))
                .child(icon(IconName::X).size(px(CLOSE_ICON)).color(color));
            if let Some(on_close) = self.on_action.clone() {
                close = close.on_click(move |_, w, cx| on_close(TerminalTabsAction::Close(index), w, cx));
            }
            el = el.child(close);
            if let Some(on_select) = self.on_action.clone() {
                el = el.on_click(move |_, w, cx| on_select(TerminalTabsAction::Select(index), w, cx));
            }
            strip = strip.child(el);
        }
        let mut plus = icon_button((id.clone(), "plus"), IconName::Plus).ghost().muted().size(ButtonSize::Xs).icon_size(px(ACTION_GLYPH));
        if let Some(on_new) = self.on_action.clone() {
            plus = plus.on_click(move |_, w, cx| on_new(TerminalTabsAction::New, w, cx));
        }
        strip = strip.child(div().flex_none().flex().items_center().h_full().child(plus));
        strip.children(indicator)
    }
}

// ---------------------------------------------------------------------------
// Dock frame
// ---------------------------------------------------------------------------

/// What the dock frame asks the host to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalDockAction {
    /// The maximize button in the header.
    Maximize,
    /// The close button in the header.
    Close,
    /// The empty state's "New terminal" button.
    NewTerminal,
}

type DockHandler = Rc<dyn Fn(TerminalDockAction, &mut Window, &mut App)>;
type ResizeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type ResizeEndHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The bottom-dock frame. Build with [`terminal_dock`].
#[derive(IntoElement)]
pub struct TerminalDock {
    id: ElementId,
    header: AnyElement,
    body: Option<AnyElement>,
    hint: Option<SharedString>,
    maximized: bool,
    on_action: Option<DockHandler>,
    on_resize_start: Option<ResizeHandler>,
    on_resize: Option<ResizeHandler>,
    on_resize_end: Option<ResizeEndHandler>,
}

/// The dock frame: `header` (usually [`terminal_tabs`]) seated on the left of
/// the header cell, and the terminal `body` below — or the centred empty state
/// when `body` is `None`. The dock owns neither its height nor its open state;
/// the host gives it a box and wires the resize callbacks.
pub fn terminal_dock(id: impl Into<ElementId>, header: impl IntoElement, body: Option<impl IntoElement>) -> TerminalDock {
    TerminalDock {
        id: id.into(),
        header: header.into_any_element(),
        body: body.map(|b| b.into_any_element()),
        hint: None,
        maximized: false,
        on_action: None,
        on_resize_start: None,
        on_resize: None,
        on_resize_end: None,
    }
}

impl TerminalDock {
    /// The hint text at the right of the header cell.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Whether the dock is at its maximized height. Flips the maximize button
    /// between expand (chevron-up) and restore (chevron-down); the host still
    /// owns the height itself.
    pub fn maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    /// Called with every intent the frame emits.
    pub fn on_action(mut self, f: impl Fn(TerminalDockAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    /// The resize drag started; the argument is the grab position in window
    /// pixels (the [`resize_handle`](crate::shell::resize_handle) contract).
    /// Arm the drag and remember the start height.
    pub fn on_resize_start(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize_start = Some(Rc::new(f));
        self
    }

    /// The resize drag moved; set the host height from the delta.
    pub fn on_resize(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Rc::new(f));
        self
    }

    /// The resize drag ended. Disarm the drag and persist the height.
    pub fn on_resize_end(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_resize_end = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TerminalDock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();

        // The resize strip along the top edge: the shell's handle, centred on
        // the dock's top hairline. Drag callbacks pass straight to the host,
        // which owns the height.
        let mut grip = resize_handle((id.clone(), "resize"));
        if let Some(f) = self.on_resize_start.clone() {
            grip = grip.on_drag_start(move |x, w, cx| f(x, w, cx));
        }
        if let Some(f) = self.on_resize.clone() {
            grip = grip.on_drag(move |x, w, cx| f(x, w, cx));
        }
        if let Some(f) = self.on_resize_end.clone() {
            grip = grip.on_drag_end(move |w, cx| f(w, cx));
        }
        let top = div().w_full().h(px(RESIZE_HANDLE_W)).flex_none().flex().justify_center().child(grip);

        // Header cell: tabs left; hint text plus maximize / close right. The
        // sprite carries no expand glyph, so maximize is a chevron-up that
        // flips down while maximized.
        let chevron = icon(IconName::ChevronDown).size(px(ACTION_GLYPH));
        let chevron = if self.maximized { chevron } else { chevron.rotate(gpui::radians(std::f32::consts::PI)) };
        let mut maximize = icon_content_button((id.clone(), "maximize"), chevron).ghost().muted().size(ButtonSize::Xs);
        if let Some(f) = self.on_action.clone() {
            maximize = maximize.on_click(move |_, w, cx| f(TerminalDockAction::Maximize, w, cx));
        }
        let mut dismiss = icon_button((id.clone(), "close"), IconName::X).ghost().muted().size(ButtonSize::Xs).icon_size(px(ACTION_GLYPH));
        if let Some(f) = self.on_action.clone() {
            dismiss = dismiss.on_click(move |_, w, cx| f(TerminalDockAction::Close, w, cx));
        }
        let mut header = h_flex()
            .id((id.clone(), "header"))
            .w_full()
            .flex_none()
            .h(cx.aui().metrics.tab_strip)
            .gap(px(scale::SP_3))
            .pr(px(scale::SP_3))
            .bg(p.surface_1)
            .border_b_1()
            .border_color(p.line)
            .child(div().flex_1().min_w(px(0.0)).flex().child(self.header));
        if let Some(hint) = self.hint {
            header = header.child(div().flex_none().ui(scale::FS_12).text_color(p.ink_3).whitespace_nowrap().child(hint));
        }
        header = header.child(maximize).child(dismiss);

        let mut body = div().flex_1().min_w(px(0.0)).min_h(px(0.0)).w_full().relative().overflow_hidden().bg(p.term_bg);
        match self.body {
            Some(content) => body = body.child(content),
            None => {
                let mut empty = v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(scale::SP_4))
                    .child(div().ui(scale::FS_13).text_color(p.ink_2).child("No terminal yet"));
                let mut new = button((id.clone(), "new-terminal"), "New terminal");
                if let Some(f) = self.on_action.clone() {
                    new = new.on_click(move |_, w, cx| f(TerminalDockAction::NewTerminal, w, cx));
                }
                empty = empty.child(new);
                body = body.child(empty);
            }
        }

        v_flex().id(id).size_full().min_h(px(0.0)).bg(p.surface_1).child(top).child(header).child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use gpui::TestAppContext;

    /// Clock time allowed for the strip to settle before its frames are
    /// counted: comfortably longer than the slowest tween and spring.
    const SETTLE: Duration = Duration::from_millis(1000);
    /// Clock time each settling draw advances by (one 60 Hz frame).
    const FRAME: Duration = Duration::from_millis(16);
    /// Draws taken while counting.
    const COUNTED_DRAWS: usize = 30;

    type Build = dyn Fn(&mut Window, &mut gpui::App) -> AnyElement;
    struct IdleHost {
        build: Rc<Build>,
    }

    impl gpui::Render for IdleHost {
        fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            (self.build.clone())(window, cx)
        }
    }

    /// Draws `build` inside a real view until it has settled, then counts
    /// the frames it keeps requesting over [`COUNTED_DRAWS`] further draws.
    fn frames_at_rest(
        cx: &mut TestAppContext,
        label: &str,
        build: impl Fn(&mut Window, &mut gpui::App) -> AnyElement + 'static,
    ) -> usize {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let (host, cx) = cx.add_window_view(|_, _| IdleHost { build: Rc::new(build) });
        let redraw = |cx: &mut gpui::VisualTestContext| {
            host.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        };
        for _ in 0..(SETTLE.as_millis() / FRAME.as_millis()) as usize {
            redraw(cx);
            cx.update(|window, app| window.simulate_next_frame(app));
            cx.executor().advance_clock(FRAME);
        }
        let mut frames = 0;
        for _ in 0..COUNTED_DRAWS {
            redraw(cx);
            frames += cx.update(|window, app| window.simulate_next_frame(app));
            cx.executor().advance_clock(FRAME);
        }
        println!("IDLE {label} frames={frames} over {COUNTED_DRAWS} draws");
        frames
    }

    /// D16: a settled two-tab strip requests no frames.
    #[gpui::test]
    fn terminal_tabs_with_two_tabs_settle(cx: &mut TestAppContext) {
        let frames = frames_at_rest(cx, "terminal-tabs/2", |_, _| {
            terminal_tabs("tabs", vec![TermTab::new("a", "zsh"), TermTab::new("b", "bash")], 0)
                .into_any_element()
        });
        assert_eq!(frames, 0, "a settled two-tab strip kept requesting frames");
    }

    /// D16: an empty strip (a project with no terminal tab) requests no
    /// frames. Without the fix the indicator's unguarded frame request
    /// redraws it forever.
    #[gpui::test]
    fn terminal_tabs_with_no_tabs_settle(cx: &mut TestAppContext) {
        let frames = frames_at_rest(cx, "terminal-tabs/0", |_, _| {
            terminal_tabs("tabs", Vec::new(), 0).into_any_element()
        });
        assert_eq!(frames, 0, "an empty strip kept requesting frames");
    }

    /// D16: a strip with an out-of-range `active` requests no frames.
    /// Without the fix the missing target still asks for a frame per draw.
    #[gpui::test]
    fn terminal_tabs_with_out_of_range_active_settle(cx: &mut TestAppContext) {
        let frames = frames_at_rest(cx, "terminal-tabs/oor", |_, _| {
            terminal_tabs("tabs", vec![TermTab::new("a", "zsh")], 5).into_any_element()
        });
        assert_eq!(frames, 0, "a strip with an out-of-range active kept requesting frames");
    }
}
