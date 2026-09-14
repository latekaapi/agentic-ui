//! `.rail`: the collapsed sidebar (⌘B), 48 px wide — nav icons, one dot per
//! active session in its state colour, the avatar at the bottom.

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, tween, Tween};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::v_flex;

use crate::data::{avatar, status_dot};
use crate::util::{interaction_flags, TrackInteraction};

/// `.rail{width:48px;padding:8px 0;gap:4px}`.
pub const RAIL_WIDTH: f32 = 48.0;
const RAIL_PAD_Y: f32 = 8.0;
const RAIL_GAP: f32 = 4.0;
/// `.rail .b{width:32px;height:32px;border-radius:var(--r-sm)}`.
const CELL: f32 = 32.0;
/// `.rail .b .i.lg{width:16px;height:16px}`.
const GLYPH: f32 = 16.0;
/// `.rail .b .bd{top:4px;right:4px;width:7px;height:7px;border:2px solid var(--surface-1)}`.
const BADGE: f32 = 7.0;
const BADGE_INSET: f32 = 4.0;
const BADGE_BORDER: f32 = 2.0;
/// `.rail .sepline{width:24px;height:1px;margin:6px 0}`.
const SEP_WIDTH: f32 = 24.0;
const SEP_MARGIN: f32 = 6.0;
/// `.rail .av{width:24px;height:24px;margin-top:auto}`.
const AVATAR: f32 = 24.0;
/// The session dot in a rail cell: `.dot{width:8px;height:8px}`.
const SESSION_DOT: f32 = 8.0;
/// A titled session tile: its initial, and the state dot tucked in the
/// corner so the letter stays the cell's content.
const TILE_TEXT: f32 = scale::FS_13;
const TILE_DOT: f32 = 6.0;
const TILE_DOT_INSET: f32 = 3.0;
/// The tooltip's width cap: a long title truncates rather than spanning the pane.
const TIP_MAX_W: f32 = 280.0;

/// The tooltip a titled session cell shows: the title on a small surface.
struct RailTip(SharedString);

impl gpui::Render for RailTip {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        div()
            .px(px(scale::SP_3))
            .py(px(scale::SP_2))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .text_color(p.ink)
            .ui(scale::FS_12)
            .max_w(px(TIP_MAX_W))
            .truncate()
            .child(self.0.clone())
    }
}

/// One cell of the rail. Build with [`RailItem::nav`], [`RailItem::separator`]
/// or [`RailItem::session`].
#[derive(Debug, Clone, PartialEq)]
pub enum RailItem {
    /// A primary nav glyph, optionally carrying the warning badge.
    Nav {
        /// Reported by `on_action`; also keys the cell's hover state.
        name: SharedString,
        /// The 16 px glyph.
        glyph: IconName,
        /// The 7 px warning badge in the top-right corner.
        badge: bool,
        /// The cell the screen is showing: accent-soft ground, accent ink.
        current: bool,
    },
    /// The 24 × 1 hairline between the nav glyphs and the sessions.
    Separator,
    /// An active session: an 8 px dot in its state colour.
    Session {
        /// Reported by `on_select`.
        id: SharedString,
        /// Colours the dot.
        state: AgentState,
        /// The dot pulses (running / waiting).
        pulse: bool,
        /// The current session: accent-soft ground.
        selected: bool,
        /// The session's title. With one, the cell is a tile bearing its
        /// initial with the state dot as a corner badge, and the title is
        /// the cell's tooltip; without one, the cell is the bare dot.
        label: Option<SharedString>,
        /// The tile's initial is drawn in this colour (the project label)
        /// instead of the default ink; ignored without a `label`, and by
        /// other kinds.
        tint: Option<gpui::Hsla>,
    },
}

impl RailItem {
    /// A nav glyph named `name` (the name `on_action` reports).
    pub fn nav(name: impl Into<SharedString>, glyph: IconName) -> Self {
        RailItem::Nav { name: name.into(), glyph, badge: false, current: false }
    }

    /// The hairline separator.
    pub fn separator() -> Self {
        RailItem::Separator
    }

    /// A session cell.
    pub fn session(id: impl Into<SharedString>, state: AgentState) -> Self {
        RailItem::Session { id: id.into(), state, pulse: false, selected: false, label: None, tint: None }
    }

    /// Titles a [`RailItem::Session`]: the tile shows its initial and the
    /// tooltip the whole title; ignored by other kinds.
    pub fn label(mut self, title: impl Into<SharedString>) -> Self {
        if let RailItem::Session { label, .. } = &mut self {
            *label = Some(title.into());
        }
        self
    }

    /// Adds the warning badge to a [`RailItem::Nav`]; ignored by other kinds.
    pub fn badge(mut self) -> Self {
        if let RailItem::Nav { badge, .. } = &mut self {
            *badge = true;
        }
        self
    }

    /// Marks a [`RailItem::Nav`] as the cell the screen is showing; ignored by
    /// other kinds ([`RailItem::selected`] does the same for a session).
    pub fn current(mut self, is_current: bool) -> Self {
        if let RailItem::Nav { current, .. } = &mut self {
            *current = is_current;
        }
        self
    }

    /// Pulses a [`RailItem::Session`] dot; ignored by other kinds.
    pub fn pulse(mut self) -> Self {
        if let RailItem::Session { pulse, .. } = &mut self {
            *pulse = true;
        }
        self
    }

    /// Marks a [`RailItem::Session`] as the current one; ignored by other kinds.
    pub fn selected(mut self, is_selected: bool) -> Self {
        if let RailItem::Session { selected, .. } = &mut self {
            *selected = is_selected;
        }
        self
    }

    /// Draws a [`RailItem::Session`] tile's initial in `colour` (the project
    /// label) instead of the default ink; the state dot and everything else
    /// are unchanged. Ignored without a [`RailItem::label`], and by other
    /// kinds.
    pub fn tint(mut self, colour: gpui::Hsla) -> Self {
        if let RailItem::Session { tint, .. } = &mut self {
            *tint = Some(colour);
        }
        self
    }
}

type SelectHandler = std::rc::Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ActionHandler = std::rc::Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The collapsed sidebar. Build with [`rail`].
#[derive(IntoElement)]
pub struct Rail {
    id: ElementId,
    items: Vec<RailItem>,
    flat: bool,
    initial: Option<SharedString>,
    pulse_phase: Option<f32>,
    on_select: Option<SelectHandler>,
    on_action: Option<ActionHandler>,
}

/// A rail showing `items`, top to bottom.
pub fn rail(id: impl Into<ElementId>, items: Vec<RailItem>) -> Rail {
    Rail { id: id.into(), items, flat: false, initial: None, pulse_phase: None, on_select: None, on_action: None }
}

impl Rail {
    /// Drops the rail's own card (border, radius, ground) and lets it fill the
    /// column it is given: the shell already paints the sidebar column's
    /// surface and divider, so the standalone card would double them.
    pub fn flat(mut self, flat: bool) -> Self {
        self.flat = flat;
        self
    }

    /// Samples every pulsing session dot at `phase` instead of mounting the
    /// looping animation: no frame is requested per render, so the rail only
    /// moves when the caller re-renders it. Unset: dots loop as before.
    pub fn pulse_phase(mut self, phase: f32) -> Self {
        self.pulse_phase = Some(phase);
        self
    }

    /// The account avatar pinned to the bottom.
    pub fn avatar(mut self, initial: impl Into<SharedString>) -> Self {
        self.initial = Some(initial.into());
        self
    }

    /// A session cell was clicked; the argument is the session id.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }

    /// A nav cell (or `"account"`) was clicked; the argument is its name.
    pub fn on_action(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for Rail {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut col = v_flex().flex_none().h_full().items_center().py(px(RAIL_PAD_Y)).gap(px(RAIL_GAP)).overflow_hidden();
        col = if self.flat {
            col.w_full()
        } else {
            col.w(px(RAIL_WIDTH)).rounded(px(scale::R_LG)).border_1().border_color(p.line).bg(p.surface_1)
        };

        for (i, item) in self.items.into_iter().enumerate() {
            match item {
                RailItem::Separator => {
                    col = col.child(div().flex_none().w(px(SEP_WIDTH)).h(px(1.0)).my(px(SEP_MARGIN)).bg(p.line));
                }
                RailItem::Nav { name, glyph, badge, current } => {
                    let cell_id: ElementId = (id.clone(), SharedString::from(format!("nav-{i}"))).into();
                    let (state, flags) = interaction_flags(cell_id.clone(), window, cx);
                    let rest_fg = if current { p.accent_ink } else { p.ink_3 };
                    let tint = if current { p.accent_soft } else { p.surface_2 };
                    let bg = tint_fade((cell_id.clone(), "bg"), current || flags.hovered, tint, Tween::FAST, window, cx);
                    let fg = tween((cell_id.clone(), "fg"), if flags.hovered && !current { p.ink } else { rest_fg }, Tween::FAST, window, cx);
                    let mut cell = div()
                        .id(cell_id)
                        .relative()
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(CELL))
                        .rounded(px(scale::R_SM))
                        .bg(bg)
                        .text_color(fg)
                        .cursor_pointer()
                        .track_interaction(&state)
                        .child(icon(glyph).size(px(GLYPH)).color(fg));
                    if badge {
                        cell = cell.child(
                            div()
                                .absolute()
                                .top(px(BADGE_INSET))
                                .right(px(BADGE_INSET))
                                .size(px(BADGE))
                                .rounded_full()
                                .bg(p.warning)
                                .border(px(BADGE_BORDER))
                                .border_color(p.surface_1),
                        );
                    }
                    if let Some(h) = self.on_action.clone() {
                        cell = cell.on_click(move |_, w, cx| h(&name, w, cx));
                    }
                    col = col.child(cell);
                }
                RailItem::Session { id: session_id, state: agent_state, pulse, selected, label, tint } => {
                    let cell_id: ElementId = (id.clone(), SharedString::from(format!("session-{i}"))).into();
                    let (state, flags) = interaction_flags(cell_id.clone(), window, cx);
                    let ground = if selected { p.accent_soft } else { p.surface_2 };
                    // A titled cell is a tile that reads even when it is not
                    // selected: the ground is always there, only its tint
                    // moves with hover and selection.
                    let lit = selected || flags.hovered || label.is_some();
                    let bg = tint_fade((cell_id.clone(), "bg"), lit, ground, Tween::FAST, window, cx);
                    let initial_color = tint.unwrap_or(if selected { p.accent_ink } else { p.ink_2 });
                    let mut cell = div()
                        .id(cell_id.clone())
                        .flex_none()
                        .relative()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(CELL))
                        .rounded(px(scale::R_SM))
                        .bg(bg)
                        .text_color(initial_color)
                        .cursor_pointer()
                        .track_interaction(&state);
                    match label {
                        Some(title) => {
                            let initial: String = title
                                .chars()
                                .find(|c| c.is_alphanumeric())
                                .map(|c| c.to_uppercase().to_string())
                                .unwrap_or_else(|| "\u{b7}".to_owned());
                            cell = cell
                                .ui(TILE_TEXT)
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(initial)
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(TILE_DOT_INSET))
                                        .right(px(TILE_DOT_INSET))
                                                                        .child({
                                    let mut dot = status_dot((cell_id.clone(), "dot"), agent_state)
                                        .size(px(TILE_DOT))
                                        .pulse(pulse);
                                    if let Some(phase) = self.pulse_phase {
                                        dot = dot.phase(phase);
                                    }
                                    dot
                                }),
                                )
                                .tooltip(move |_, cx| cx.new(|_| RailTip(title.clone())).into());
                        }
                        None => {
                            let mut dot = status_dot((cell_id, "dot"), agent_state).size(px(SESSION_DOT)).pulse(pulse);
                            if let Some(phase) = self.pulse_phase {
                                dot = dot.phase(phase);
                            }
                            cell = cell.child(dot);
                        }
                    }
                    if let Some(h) = self.on_select.clone() {
                        cell = cell.on_click(move |_, w, cx| h(&session_id, w, cx));
                    }
                    col = col.child(cell);
                }
            }
        }

        if let Some(initial) = self.initial {
            let account_id: ElementId = (id, "account").into();
            let mut account = div().id(account_id).flex_none().cursor_pointer().child(avatar(initial).size(px(AVATAR)));
            if let Some(h) = self.on_action.clone() {
                account = account.on_click(move |_, w, cx| h("account", w, cx));
            }
            col = col.child(div().flex_1()).child(account);
        }
        col
    }
}
