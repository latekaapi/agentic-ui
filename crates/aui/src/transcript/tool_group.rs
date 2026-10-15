//! Tool-call group (D4, docs/41 §2.5): consecutive tool calls as ONE
//! surface. Closed: the header (status glyph, summary, chevron) over a
//! caption preview of the first two calls plus `+k more`. Open: every call
//! is a row in the same card — the tool header's layout over a 7 % rule,
//! its output opening on the Well right under it. Nothing nests: no card
//! inside a card.

use aui_motion::{open_close, SpringKind, COLLAPSE_EXIT};
use aui_protocol::{ActivityState, Block, ToolCall, ToolStatus};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use crate::data::record_ax_label;
use gpui::{div, prelude::*, px, radians, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::spinner;
use crate::icons::{icon, IconName};
use crate::transcript::quiet_row::{status_check, status_cross, STATUS_GLYPH};
use crate::transcript::{tool_card, transcript_card, ToolCardIntent};

/// Collapsed preview rows before the `+k more` row (the image3 idiom: two
/// rows, then the overflow count).
pub const GROUP_PREVIEW: usize = 2;
/// `.tg-h{gap:8px}`; header text is the card frame's shared 12.5 px.
const HEADER_GAP: f32 = 8.0;
/// The closed preview: padding 6 / 10 / 8, caption.
const PREVIEW_PAD_TOP: f32 = 6.0;
const PREVIEW_PAD_X: f32 = 10.0;
const PREVIEW_PAD_BOTTOM: f32 = 8.0;
const PREVIEW_TEXT: f32 = 12.0;
const PREVIEW_GAP: f32 = 6.0;
/// The preview rows' glyph.
const PREVIEW_GLYPH: f32 = 12.0;

/// What a tool group asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGroupIntent {
    /// Toggle the group between preview and full cards.
    Toggle,
    /// A per-call card intent: which call, and what it asked for.
    Call {
        /// Index into the group's calls.
        index: usize,
        /// The call's own intent (toggle, unfold).
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
    calls_unfolded: Vec<bool>,
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
    let calls_unfolded = vec![false; group.calls.len()];
    ToolGroup { id: id.into(), group: group.clone(), open, calls_open, calls_unfolded, on_intent: None }
}

impl ToolGroup {
    /// Whether one call's full card is open (all are, by default).
    pub fn call_open(mut self, index: usize, open: bool) -> Self {
        if let Some(slot) = self.calls_open.get_mut(index) {
            *slot = open;
        }
        self
    }

    /// Whether one call's long output shows in full (its "N more lines"
    /// row was pressed).
    pub fn call_unfolded(mut self, index: usize, unfolded: bool) -> Self {
        if let Some(slot) = self.calls_unfolded.get_mut(index) {
            *slot = unfolded;
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

/// Whether the summary already states this group's own count — `calls`
/// as a whole word (`Read 3 files` with 3 calls) — in which case the header
/// drops its own `N calls` instead of saying the count twice. Any other
/// number (`Checked 2FA flow` with 3 calls) or the same digits inside a
/// word (`2FA` with 2 calls) keeps the suffix.
pub fn summary_has_count(summary: &str, calls: usize) -> bool {
    let want = calls.to_string();
    let bytes = summary.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if summary[start..i] == want {
                let left = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
                let right = i == bytes.len() || !bytes[i].is_ascii_alphanumeric();
                if left && right {
                    return true;
                }
            }
        } else {
            i += 1;
        }
    }
    false
}

impl RenderOnce for ToolGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let count = self.group.calls.len();

        let glyph: gpui::AnyElement = match self.group.state {
            ActivityState::Working => spinner((id.clone(), "spinner")).size(px(STATUS_GLYPH)).fluid(cx).into_any_element(),
            ActivityState::Done => status_check(p.success).into_any_element(),
            ActivityState::Failed => status_cross(p.danger).into_any_element(),
        };
        // The card frame stays expanded: closed means the preview, not an
        // empty card, so the chevron turns from the group's own state on the
        // fold's curve. The muted count is dropped when the summary already
        // states the number (`Read 2 files · 2 calls` said it twice).
        let turn = open_close((id.clone(), "chevron"), self.open, SpringKind::Moderate, COLLAPSE_EXIT, window, cx);
        let mut card = transcript_card(id.clone(), true)
            .chevron(false)
            .header(glyph)
            .header(div().whitespace_nowrap().child(self.group.summary.clone()));
        if !summary_has_count(&self.group.summary, count) {
            card = card.header(div().text_color(p.ink_2).child(count_label(count)));
        }
        card = card.header(div().flex_1()).header(
            div()
                .flex_none()
                .ml(px(4.0))
                .child(icon(IconName::Chev).size(px(12.0)).color(p.ink_3).rotate(radians(turn * std::f32::consts::FRAC_PI_2))),
        );

        if self.open {
            // Every call is a row of this one card: the tool header's layout
            // over a 7 % rule, its output on the Well under it.
            let mut list = v_flex().w_full();
            for (index, call) in self.group.calls.iter().enumerate() {
                let call_open = self.calls_open.get(index).copied().unwrap_or(true);
                let unfolded = self.calls_unfolded.get(index).copied().unwrap_or(false);
                let handler = self.on_intent.clone();
                list = list.child(
                    tool_card(
                        (id.clone(), SharedString::from(format!("call-{index}"))),
                        call.verb.clone(),
                        call.target.clone(),
                        call.status,
                        call.body.clone(),
                    )
                    .flat(true)
                    .duration_ms(call.duration_ms)
                    // The server's whole-patch summary rides along with no
                    // caller opt-in, exactly as a lone card draws it.
                    .diff_stat(call.diff_stat)
                    .open(call_open)
                    .unfolded(unfolded)
                    .on_intent(move |intent, w, cx| {
                        if let Some(h) = &handler {
                            h(ToolGroupIntent::Call { index, intent }, w, cx);
                        }
                    }),
                );
            }
            card = card.body_border(false).body(list);
        } else {
            let mut preview = v_flex()
                .w_full()
                .pt(px(PREVIEW_PAD_TOP))
                .px(px(PREVIEW_PAD_X))
                .pb(px(PREVIEW_PAD_BOTTOM))
                .gap(px(PREVIEW_GAP))
                .ui(PREVIEW_TEXT)
                .text_color(p.ink_2);
            for (index, call) in self.group.calls.iter().take(GROUP_PREVIEW).enumerate() {
                let glyph: gpui::AnyElement = match call.status {
                    ToolStatus::Pending | ToolStatus::Running => {
                        spinner((id.clone(), SharedString::from(format!("preview-{index}")))).size(px(PREVIEW_GLYPH)).fluid(cx).into_any_element()
                    }
                    ToolStatus::Success => div().flex_none().child(icon(IconName::Check).size(px(PREVIEW_GLYPH)).color(p.success).lit(true)).into_any_element(),
                    ToolStatus::Error | ToolStatus::Cancelled => {
                        div().flex_none().child(icon(IconName::X).size(px(PREVIEW_GLYPH)).color(p.danger).lit(true)).into_any_element()
                    }
                };
                preview = preview.child(
                    h_flex()
                        .w_full()
                        .gap(px(HEADER_GAP))
                        .child(glyph)
                        .child(div().whitespace_nowrap().child(call.verb.clone()))
                        .child(div().flex_1().min_w(px(0.0)).truncate().mono(scale::FS_12).child(call.target.clone())),
                );
            }
            let hidden = preview_hidden(count);
            if hidden > 0 {
                preview = preview.child(
                    div()
                        .id((id.clone(), "more"))
                        .text_color(p.ink_3)
                        .hover(|d| d.text_color(p.ink).underline())
                        .child(more_label(hidden)),
                );
            }
            // The closed preview is part of the toggle: a click anywhere on
            // it — the `+k more` row most of all — opens the group (owner
            // 2026-10-14: "not able to click/toggle this").
            let preview = match self.on_intent.clone() {
                Some(on_intent) => {
                    record_ax_label("Show all calls");
                    preview
                        .id((id.clone(), "preview"))
                        .cursor_pointer()
                        .role(gpui::Role::Button)
                        .aria_label("Show all calls")
                        .on_click(move |_, w, cx| on_intent(ToolGroupIntent::Toggle, w, cx))
                        .into_any_element()
                }
                None => preview.into_any_element(),
            };
            card = card.body_border(true).body(preview);
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
            body: aui_protocol::ToolBody::Read { lines: 12, output_lines: Vec::new() },
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
    fn only_the_group_count_itself_drops_the_second_count() {
        // The summary states this group's own count as a whole word.
        assert!(summary_has_count("Ran 2 commands", 2));
        assert!(summary_has_count("Read 12 files", 12));
        assert!(summary_has_count("11 tool calls", 11));
        assert!(summary_has_count("Read 3 files", 3));
        // Any other number keeps the suffix — even another digit.
        assert!(!summary_has_count("Ran 2 commands", 3));
        assert!(!summary_has_count("Read 12 files", 2));
        assert!(!summary_has_count("Checked 2FA flow", 3));
        assert!(!summary_has_count("Checked the form flow", 3));
        assert!(!summary_has_count("", 3));
        // The same digits inside a word are not the count.
        assert!(!summary_has_count("Checked 2FA flow", 2));
        assert!(!summary_has_count("abc3def", 3));
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
    fn grouped_calls_tell_their_intents_apart() {
        let group = tool_group("g", &grouped(3), true).call_unfolded(1, true);
        assert_eq!(group.calls_unfolded, vec![false, true, false]);
        let first = ToolGroupIntent::Call { index: 1, intent: ToolCardIntent::Unfold };
        let second = ToolGroupIntent::Call { index: 2, intent: ToolCardIntent::Unfold };
        assert_ne!(first, second);
    }
}
