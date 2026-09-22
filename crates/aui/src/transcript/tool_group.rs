//! Tool-call group: consecutive tool calls folded into one card — one header
//! (status glyph, summary, muted count, chevron) with a collapsed preview of
//! the first two calls plus `+k more`, and every call as a full tool card
//! once open.

use aui_protocol::{ActivityState, Block, ToolCall};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{glyph_err, glyph_ok, spinner};
use crate::nav::chevron;
use crate::transcript::{tool_card, transcript_card, ToolCardAction, ToolCardIntent};

/// Collapsed preview rows before the `+k more` row (the image3 idiom: two
/// rows, then the overflow count).
pub const GROUP_PREVIEW: usize = 2;
/// `.tg-h{gap:8px}`; header text is the card frame's shared 12.5 px.
const HEADER_GAP: f32 = 8.0;
/// `.tg-p{padding:6px 10px 8px}` preview rows read like the activity timeline.
const PREVIEW_PAD_TOP: f32 = 6.0;
const PREVIEW_PAD_X: f32 = 10.0;
const PREVIEW_PAD_BOTTOM: f32 = 8.0;
const PREVIEW_TEXT: f32 = 12.5;
const PREVIEW_GAP: f32 = 6.0;
/// `.tg-o{gap:8px;padding:10px}` — open calls stack as full cards.
const OPEN_GAP: f32 = 8.0;
const OPEN_PAD: f32 = 10.0;

/// What a tool group asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGroupIntent {
    /// Toggle the group between preview and full cards.
    Toggle,
    /// A per-call card intent: which call, and what it asked for.
    Call {
        /// Index into the group's calls.
        index: usize,
        /// The call's own intent (toggle, unfold, open in pane).
        intent: ToolCardIntent,
    },
}

type IntentHandler = std::rc::Rc<dyn Fn(ToolGroupIntent, &mut Window, &mut App)>;

/// The data a tool group renders.
#[derive(Clone, Debug)]
pub struct ToolGroupData {
    /// The calls, in execution order.
    pub calls: Vec<ToolCall>,
    /// Collapsed header line, e.g. `"Checked the form flow"`.
    pub summary: SharedString,
    /// Whether the group is still running, which picks the header glyph.
    pub state: ActivityState,
}

impl ToolGroupData {
    /// The data of a [`Block::ToolGroup`]; `None` for any other variant.
    pub fn from_block(block: &Block) -> Option<Self> {
        match block {
            Block::ToolGroup { calls, summary, state } => {
                Some(ToolGroupData { calls: calls.clone(), summary: summary.clone().into(), state: *state })
            }
            _ => None,
        }
    }
}

/// A group of tool calls. Build with [`tool_group`].
#[derive(IntoElement)]
pub struct ToolGroup {
    id: ElementId,
    group: ToolGroupData,
    open: bool,
    calls_open: Vec<bool>,
    card_actions: Vec<Vec<ToolCardAction>>,
    on_intent: Option<IntentHandler>,
}

/// Consecutive `group` calls under one summary; `open` picks preview rows
/// (`false`) or every call as a full [`tool_card`] (`true`).
///
/// Borrowed, not owned: a caller that keeps the group in its own state used to
/// have to clone the whole payload to hand it over, once per frame, on top of
/// the clone the card itself needs (finding `library-hotpaths-8`).
pub fn tool_group(id: impl Into<ElementId>, group: &ToolGroupData, open: bool) -> ToolGroup {
    let calls_open = vec![true; group.calls.len()];
    let card_actions = vec![Vec::new(); group.calls.len()];
    ToolGroup { id: id.into(), group: group.clone(), open, calls_open, card_actions, on_intent: None }
}

impl ToolGroup {
    /// Whether one call's full card is open (all are, by default).
    pub fn call_open(mut self, index: usize, open: bool) -> Self {
        if let Some(slot) = self.calls_open.get_mut(index) {
            *slot = open;
        }
        self
    }

    /// Trailing header actions on one call's inner card, drawn exactly as a
    /// lone [`tool_card`]'s: after the duration, in the order given. A press
    /// reports [`ToolGroupIntent::Call`] with the call's index and
    /// [`ToolCardIntent::Action`]'s action index, so the host can tell card
    /// 2's action 0 from card 3's action 0. Calls with no actions render
    /// exactly as before.
    pub fn card_actions(mut self, index: usize, actions: Vec<ToolCardAction>) -> Self {
        if let Some(slot) = self.card_actions.get_mut(index) {
            *slot = actions;
        }
        self
    }

    /// One trailing header action on one call's inner card after any already
    /// set. See [`Self::card_actions`].
    pub fn card_action(mut self, index: usize, action: ToolCardAction) -> Self {
        if let Some(slot) = self.card_actions.get_mut(index) {
            slot.push(action);
        }
        self
    }

    /// Intent handler: [`ToolGroupIntent::Toggle`] for the header, and one
    /// [`ToolGroupIntent::Call`] per call card.
    pub fn on_intent(mut self, f: impl Fn(ToolGroupIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }
}

/// Preview rows past the first [`GROUP_PREVIEW`] collapse into the more row.
pub fn preview_hidden(total: usize) -> usize {
    total.saturating_sub(GROUP_PREVIEW)
}

/// `+3 more` for the collapsed preview's overflow row.
pub fn more_label(hidden: usize) -> String {
    format!("+{hidden} more")
}

/// `1 call` / `N calls` for the muted header count.
pub fn count_label(calls: usize) -> String {
    if calls == 1 { "1 call".to_string() } else { format!("{calls} calls") }
}

impl RenderOnce for ToolGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let count = self.group.calls.len();

        let glyph: gpui::AnyElement = match self.group.state {
            ActivityState::Working => spinner((id.clone(), "spinner")).into_any_element(),
            ActivityState::Done => glyph_ok().into_any_element(),
            ActivityState::Failed => glyph_err().into_any_element(),
        };
        // The card frame stays expanded: collapsed means preview rows, not an
        // empty card, so the chevron is drawn by hand from the group state.
        let mut card = transcript_card(id.clone(), true)
            .chevron(false)
            .header(glyph)
            .header(div().medium().whitespace_nowrap().child(self.group.summary.clone()))
            .header(div().text_color(p.ink_3).child(count_label(count)))
            .header(div().flex_1())
            .header(chevron((id.clone(), "chevron"), self.open, p.ink_3, window, cx));

        if self.open {
            let mut list = v_flex().w_full().gap(px(OPEN_GAP)).p(px(OPEN_PAD));
            for (index, call) in self.group.calls.iter().enumerate() {
                let call_open = self.calls_open.get(index).copied().unwrap_or(true);
                // No actions registered and the inner card builds exactly as
                // before; otherwise the actions ride the lone card's own
                // slot, reported under this call's index.
                let actions = self.card_actions.get(index).cloned().unwrap_or_default();
                let handler = self.on_intent.clone();
                list = list.child(
                    tool_card(
                        (id.clone(), SharedString::from(format!("call-{index}"))),
                        call.verb.clone(),
                        call.target.clone(),
                        call.status,
                        call.body.clone(),
                    )
                    .duration_ms(call.duration_ms)
                    // The server's whole-patch summary rides along with no
                    // caller opt-in, exactly as a lone card draws it.
                    .diff_stat(call.diff_stat)
                    .open(call_open)
                    .actions(actions)
                    .on_intent(move |intent, w, cx| {
                        if let Some(h) = &handler {
                            h(ToolGroupIntent::Call { index, intent }, w, cx);
                        }
                    }),
                );
            }
            card = card.body(list);
        } else {
            let mut preview = v_flex()
                .w_full()
                .pt(px(PREVIEW_PAD_TOP))
                .px(px(PREVIEW_PAD_X))
                .pb(px(PREVIEW_PAD_BOTTOM))
                .gap(px(PREVIEW_GAP))
                .ui(PREVIEW_TEXT)
                .text_color(p.ink);
            for call in self.group.calls.iter().take(GROUP_PREVIEW) {
                preview = preview.child(
                    h_flex()
                        .w_full()
                        .gap(px(HEADER_GAP))
                        .child(div().medium().whitespace_nowrap().child(call.verb.clone()))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .truncate()
                                .mono(scale::FS_12)
                                .text_color(p.ink_2)
                                .child(call.target.clone()),
                        ),
                );
            }
            let hidden = preview_hidden(count);
            if hidden > 0 {
                preview = preview.child(
                    div().ui(scale::FS_11).text_color(p.ink_3).child(more_label(hidden)),
                );
            }
            card = card.body(preview);
        }
        if let Some(on_intent) = self.on_intent {
            card = card.on_toggle(move |_, w, cx| on_intent(ToolGroupIntent::Toggle, w, cx));
        }
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aui_protocol::{ToolKind, ToolStatus};

    fn call(id: &str) -> ToolCall {
        ToolCall {
            id: id.into(),
            kind: ToolKind::Read,
            verb: "Read".into(),
            target: "src/main.rs".into(),
            status: ToolStatus::Success,
            duration_ms: Some(4),
            body: aui_protocol::ToolBody::Read { lines: 12 },
            diff_stat: None,
        }
    }

    #[test]
    fn only_rows_past_the_preview_count_as_hidden() {
        assert_eq!(preview_hidden(0), 0);
        assert_eq!(preview_hidden(1), 0);
        assert_eq!(preview_hidden(GROUP_PREVIEW), 0);
        assert_eq!(preview_hidden(GROUP_PREVIEW + 3), 3);
    }

    #[test]
    fn the_more_row_names_the_overflow() {
        assert_eq!(more_label(1), "+1 more");
        assert_eq!(more_label(3), "+3 more");
    }

    #[test]
    fn the_header_count_reads_singular_for_one_call() {
        assert_eq!(count_label(0), "0 calls");
        assert_eq!(count_label(1), "1 call");
        assert_eq!(count_label(5), "5 calls");
    }

    #[test]
    fn data_comes_from_the_tool_group_variant_only() {
        let block = Block::ToolGroup {
            calls: vec![call("a"), call("b")],
            summary: "Checked the flow".into(),
            state: ActivityState::Done,
        };
        let data = ToolGroupData::from_block(&block).expect("group data");
        assert_eq!(data.calls.len(), 2);
        assert_eq!(data.summary.to_string(), "Checked the flow");
        assert_eq!(data.state, ActivityState::Done);
        assert!(ToolGroupData::from_block(&Block::text("hi")).is_none());
    }

    #[test]
    fn tool_call_round_trips_through_the_block_variant() {
        let block = Block::tool_call(call("tc1"));
        let back = block.as_tool_call().expect("tool call");
        assert_eq!(back, call("tc1"));
        assert!(Block::text("hi").as_tool_call().is_none());
    }

    fn grouped(calls: usize) -> ToolGroupData {
        ToolGroupData {
            calls: (0..calls).map(|n| call(&format!("c{n}"))).collect(),
            summary: "Ran things".into(),
            state: ActivityState::Done,
        }
    }

    #[test]
    fn grouped_cards_tell_their_actions_apart() {
        let group = tool_group("g", &grouped(3), true)
            .card_action(1, ToolCardAction::new("retry", "Retry"))
            .card_action(2, ToolCardAction::new("retry", "Retry"));
        assert!(group.card_actions[0].is_empty());
        assert_eq!(group.card_actions[1].len(), 1);
        assert_eq!(group.card_actions[2].len(), 1);
        // The intent names both the card and the action: card 1's action 0
        // is a different press from card 2's action 0.
        let first = ToolGroupIntent::Call {
            index: 1,
            intent: ToolCardIntent::Action(0),
        };
        let second = ToolGroupIntent::Call {
            index: 2,
            intent: ToolCardIntent::Action(0),
        };
        assert_ne!(first, second);
        assert!(matches!(
            first,
            ToolGroupIntent::Call {
                index: 1,
                intent: ToolCardIntent::Action(0),
            }
        ));
    }

    #[test]
    fn a_group_with_no_card_actions_stays_empty() {
        // Nothing registered: every inner card builds exactly as before.
        let group = tool_group("g", &grouped(2), true);
        assert!(group.card_actions.iter().all(|actions| actions.is_empty()));
        let group = tool_group("g", &grouped(2), true).card_actions(5, vec![]);
        assert!(group.card_actions.iter().all(|actions| actions.is_empty()));
    }
}
