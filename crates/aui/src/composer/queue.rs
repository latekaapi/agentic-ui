//! Queued messages (`.qi`) above the composer and suggestion chips (`.sugg`)
//! below the last turn.

use std::time::Duration;

use aui_icons::{icon, IconName};
use aui_motion::{presence, stagger_delay, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::util::{indexed_child, interaction_flags, named_child, TrackInteraction};

/// The queued row: 32 px, gap 8, caption text.
const ROW_H: f32 = 32.0;
const ROW_GAP: f32 = 8.0;
const ROW_PAD: f32 = 10.0;
const ROW_TEXT: f32 = 12.0;
/// The `queued` cap: 16 px, 10.5 mono, 0 / 6.
const CAP_H: f32 = 16.0;
const BADGE_TEXT: f32 = 10.5;
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
        // D4 (docs/41 §2.8): Raised-lit, radius 6, the edge, 32 px, padded
        // 0 / 6 / 0 / 10: a 16 px `queued` cap, the text in caption, ✕.
        h_flex()
            .id(id.clone())
            .w_full()
            .h(px(ROW_H))
            .gap(px(ROW_GAP))
            .pl(px(ROW_PAD))
            .pr(px(6.0))
            .items_center()
            .rounded(px(6.0))
            .bg(crate::util::raised_lit(&p, p.surface_2))
            .shadow(p.edge())
            .ui(ROW_TEXT)
            .text_color(p.ink)
            .child(
                div()
                    .flex_none()
                    .h(px(CAP_H))
                    .px(px(BADGE_PAD_X))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(p.selected_overlay())
                    .mono(BADGE_TEXT)
                    .text_color(p.ink_2)
                    .child(if self.editing { "editing" } else { "queued" }),
            )
            .child(div().flex_1().min_w(px(0.0)).truncate().child(self.text))
            .child(
                div()
                    .id((id, "remove"))
                    .flex_none()
                    .size(px(24.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(p.ink_2)
                    .hover(|d| d.bg(p.hover_overlay()).text_color(p.ink))
                    .role(gpui::Role::Button)
                    .aria_label("Remove queued message")
                    .on_click(emit(QueueIntent::Remove))
                    .child(icon(IconName::X).size(px(ACTION_GLYPH))),
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
        let _ = (p, count);
        let mut column = v_flex().w_full().gap(px(STRIP_GAP));
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
