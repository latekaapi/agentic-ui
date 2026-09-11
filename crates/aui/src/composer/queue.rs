//! Queued messages (`.qi`) above the composer and suggestion chips (`.sugg`)
//! below the last turn.

use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_motion::{presence, stagger_delay, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, ButtonSize};
use crate::util::{indexed_child, interaction_flags, named_child, TrackInteraction};

/// `.qi{gap:8px;height:30px;padding:0 10px;font-size:12.5px}` with a dashed line-strong border.
const ROW_GAP: f32 = 8.0;
const ROW_PAD: f32 = 10.0;
const ROW_TEXT: f32 = 12.5;
/// `.qi .n{font:600 10px/1 mono;padding:3px 6px}`.
const BADGE_TEXT: f32 = 10.0;
const BADGE_PAD_Y: f32 = 3.0;
const BADGE_PAD_X: f32 = 6.0;
/// The xs ghost actions carry 11 px glyphs.
const ACTION_GLYPH: f32 = 11.0;
/// `.sugg{gap:6px}`; chips 26 px, padding 0 10, 12 px, gap 6, 11 px sparkle; 60 ms stagger.
const SUGG_GAP: f32 = 6.0;
const SUGG_H: f32 = 26.0;
const SUGG_PAD: f32 = 10.0;
const SUGG_GLYPH: f32 = 11.0;
const SUGG_STAGGER: Duration = Duration::from_millis(60);
const SUGG_RISE: f32 = 4.0;
/// The strip above the docked composer: `.queue{gap:6px}` with `.caps` 6 px above it.
const STRIP_GAP: f32 = 6.0;
const STRIP_HEAD_GAP: f32 = 2.0;

/// What a queue row asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueIntent {
    /// Unqueue it and put the text back in the composer.
    Edit,
    /// Drop it from the queue.
    Remove,
    /// Unqueue it and interject it into the running turn instead
    /// (MSP `turn/unqueue` then `turn/steer`).
    Steer,
}

type QueueHandler = std::rc::Rc<dyn Fn(QueueIntent, &mut Window, &mut App)>;

/// A queued message row. Build with [`queue_row`].
#[derive(IntoElement)]
pub struct QueueRow {
    id: ElementId,
    text: SharedString,
    editing: bool,
    on_intent: Option<QueueHandler>,
}

/// A queued message.
pub fn queue_row(id: impl Into<ElementId>, text: impl Into<SharedString>) -> QueueRow {
    QueueRow { id: id.into(), text: text.into(), editing: false, on_intent: None }
}

impl QueueRow {
    /// The row whose text is in the composer, waiting for the server's
    /// `turn/unqueued` to take it out of the queue. The badge says so; nothing
    /// is removed until the wire says it was.
    pub fn editing(mut self) -> Self {
        self.editing = true;
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(QueueIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for QueueRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let emit = |intent: QueueIntent| {
            let h = self.on_intent.clone();
            move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(h) = &h {
                    h(intent, w, cx)
                }
            }
        };
        h_flex()
            .id(id.clone())
            .w_full()
            .h(cx.aui().metrics.row)
            .gap(px(ROW_GAP))
            .px(px(ROW_PAD))
            .rounded(px(scale::R_MD))
            .border_1()
            .border_dashed()
            .border_color(p.line_strong)
            .ui(ROW_TEXT)
            .text_color(p.ink_2)
            .child(
                div()
                    .flex_none()
                    .py(px(BADGE_PAD_Y))
                    .px(px(BADGE_PAD_X))
                    .rounded_full()
                    .bg(p.surface_3)
                    .mono(BADGE_TEXT)
                    .line_height(relative(1.0))
                    .semibold()
                    .text_color(p.ink_3)
                    .child(if self.editing { "editing" } else { "queued" }),
            )
            .child(div().flex_1().min_w(px(0.0)).truncate().child(self.text))
            .child(
                h_flex()
                    .flex_none()
                    .gap(px(scale::SP_1))
                    .child(icon_button((id.clone(), "steer"), IconName::ArrowUp).ghost().size(ButtonSize::Xs).icon_size(px(ACTION_GLYPH)).on_click(emit(QueueIntent::Steer)))
                    .child(icon_button((id.clone(), "edit"), IconName::Edit).ghost().size(ButtonSize::Xs).icon_size(px(ACTION_GLYPH)).on_click(emit(QueueIntent::Edit)))
                    .child(icon_button((id, "remove"), IconName::X).ghost().size(ButtonSize::Xs).icon_size(px(ACTION_GLYPH)).on_click(emit(QueueIntent::Remove))),
            )
    }
}

type PickHandler = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Suggestion chips. Build with [`suggestion_chips`].
#[derive(IntoElement)]
pub struct SuggestionChips {
    id: ElementId,
    items: Vec<SharedString>,
    sparkle_first: bool,
    at_rest: bool,
    on_pick: Option<PickHandler>,
}

/// Chips for `items`; the first carries the sparkle glyph.
pub fn suggestion_chips(id: impl Into<ElementId>, items: Vec<SharedString>) -> SuggestionChips {
    SuggestionChips { id: id.into(), items, sparkle_first: true, at_rest: false, on_pick: None }
}

impl SuggestionChips {
    /// Skips the staggered enter (static captures).
    pub fn at_rest(mut self) -> Self {
        self.at_rest = true;
        self
    }

    /// Pick handler with the chip index.
    pub fn on_pick(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for SuggestionChips {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let count = self.items.len();
        let mut row = h_flex().w_full().flex_wrap().gap(px(SUGG_GAP));
        for (i, label) in self.items.into_iter().enumerate() {
            let chip_id: ElementId = indexed_child(&id, "sugg-", i);
            let style = if self.at_rest {
                PresenceStyle { opacity: 1.0, offset_y: px(0.0), scale: 1.0 }
            } else {
                let sample = presence((chip_id.clone(), "enter"), true, EnterExit::BASE.with_delay(stagger_delay(i, count, SUGG_STAGGER)), window, cx);
                PresenceStyle::fade_rise(sample, SUGG_RISE)
            };
            let (state, flags) = interaction_flags(chip_id.clone(), window, cx);
            let mut chip = h_flex()
                .id(chip_id)
                .relative()
                .top(style.offset_y)
                .h(px(SUGG_H))
                .px(px(SUGG_PAD))
                .gap(px(SUGG_GAP))
                .rounded_full()
                .border_1()
                .border_color(if flags.hovered { p.line_strong } else { p.line })
                .bg(p.surface_1)
                .ui(scale::FS_12)
                .text_color(if flags.hovered { p.ink } else { p.ink_2 })
                .opacity(style.opacity)
                .cursor_pointer()
                .track_interaction(&state);
            if i == 0 && self.sparkle_first {
                chip = chip.child(icon(IconName::Sparkle).size(px(SUGG_GLYPH)));
            }
            chip = chip.child(label);
            if let Some(h) = self.on_pick.clone() {
                chip = chip.on_click(move |_, w, cx| h(i, w, cx));
            }
            row = row.child(chip);
        }
        row
    }
}

// ---------------------------------------------------------------------------
// The strip
// ---------------------------------------------------------------------------

/// One row of a [`QueueStrip`]: the server's turn id and the text it queued.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueStripRow {
    /// The turn id the queueing ack minted; handed back with every intent, and
    /// what `turn/unqueue` needs verbatim.
    pub id: SharedString,
    /// The submission's text.
    pub text: SharedString,
    /// Whether this row is the one currently being edited — its text is in the
    /// composer and the row is waiting for `turn/unqueued` to confirm it.
    pub editing: bool,
}

impl QueueStripRow {
    /// A queued row.
    pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self { id: id.into(), text: text.into(), editing: false }
    }

    /// Marks the row as the one being edited.
    pub fn editing(mut self) -> Self {
        self.editing = true;
        self
    }
}

type StripHandler = std::rc::Rc<dyn Fn(&SharedString, QueueIntent, &mut Window, &mut App)>;

/// The queued submissions above the docked composer. Build with [`queue_strip`].
///
/// The strip renders **exactly what it is given, in the order it is given**: MSP
/// moves a queued turn only on `turn/unqueued` or `turn/started`, so a client
/// that reordered optimistically would show a queue the server does not have.
#[derive(IntoElement)]
pub struct QueueStrip {
    id: ElementId,
    rows: Vec<QueueStripRow>,
    on_intent: Option<StripHandler>,
}

/// The strip for `rows`, newest last, in server order.
pub fn queue_strip(id: impl Into<ElementId>, rows: Vec<QueueStripRow>) -> QueueStrip {
    QueueStrip { id: id.into(), rows, on_intent: None }
}

impl QueueStrip {
    /// Intent handler; the first argument is the row's [`QueueStripRow::id`].
    pub fn on_intent(mut self, f: impl Fn(&SharedString, QueueIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for QueueStrip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let count = self.rows.len();
        let mut column = v_flex().w_full().gap(px(STRIP_GAP)).child(
            div()
                .w_full()
                .pb(px(STRIP_HEAD_GAP))
                .text_role(TextRole::Caps)
                .text_color(p.ink_3)
                .child(format!("Queued \u{b7} {count}")),
        );
        for row in self.rows {
            let row_id: ElementId = named_child(&id, "q-", &row.id);
            let handler = self.on_intent.clone();
            let key = row.id.clone();
            let mut element = queue_row(row_id, row.text.clone()).on_intent(move |intent, w, cx| {
                if let Some(h) = &handler {
                    h(&key, intent, w, cx)
                }
            });
            if row.editing {
                element = element.editing();
            }
            column = column.child(element);
        }
        column
    }
}
