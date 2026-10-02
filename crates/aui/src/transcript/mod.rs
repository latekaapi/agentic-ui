//! Transcript: markers, user and assistant turns with streaming reveal,
//! thinking block, activity group, tool cards and bodies, approval,
//! question / plan / todo, code and diff blocks, summary / error / status
//! rows, citations and sources (cards 30–38, 55).

mod activity;
#[cfg(test)]
mod hotpath_bench;
mod ansi;
mod approval;
mod card;
mod code;
mod handoff;
mod item;
mod markdown;
mod memo;
mod marker;
mod plan;
mod question;
mod selectable;
mod status;
mod summary;
mod syntax;
mod thinking;
mod todo;
mod tool_card;
mod tool_group;
mod turn_fold;
mod prose;
mod turns;

pub use crate::data::{arm_ax_probe, record_ax_label, take_ax_labels};
pub use activity::{activity_group, ActivityGroup};
pub use card::{transcript_card, TranscriptCard};
pub use thinking::{thinking_block, ThinkingBlock};
pub use prose::{caret_top_in_line, caret_visible, prose, ProseStyle, CARET_BASELINE_DROP, CARET_H, CARET_MARGIN_LEFT, CARET_W};
pub use markdown::{last_block_runs, markdown, markdown_fences, markdown_fences_settled, markdown_selected_text, markdown_selected_text_settled, markdown_span_selected_text, message_select_all, message_select_all_settled, message_selected_text, message_selected_text_settled, parse_markdown, parse_markdown_settled, parsed_markdown, span_runs, Block, Block as MarkdownBlock, LinkHandler, LinkRange, LinkTarget, Markdown, MarkdownFence, Span, Span as MarkdownSpan, TableAlign};
pub use selectable::{selectable_text, MessageSelection, SelectableText, SelectionEndpoint, SelectionHandler, SelectionKey, SpanEvent, SpanHandler, SpanSession, TextSelection};
pub use marker::{marker_row, HandOff, MarkerRow};
pub use turns::{assistant_turn, footer_items, format_age, turn_selected_text, turn_selected_text_settled, turn_span_selected_text, turn_span_selected_text_settled, user_turn, AssistantTurn, AssistantTurnAction, UserTurn, UserTurnAction, COPY_HOLD};
pub use ansi::{ansi_runs, parse_ansi, AnsiSpan};
pub use tool_card::{
    arm_chip_probe, format_duration, mcp_visible_lines, search_visible_hits, shell_visible_lines,
    take_drawn_chips, tool_card, ToolCard, ToolCardAction, ToolCardIntent, MCP_BODY_CAP,
    SEARCH_BODY_CAP, SHELL_FOLD,
};
pub use tool_group::{
    count_label, more_label, preview_hidden, summary_has_count, tool_group, ToolGroup,
    ToolGroupData, ToolGroupIntent, GROUP_PREVIEW,
};
pub use turn_fold::{
    earlier_label, live_activity_row, turn_fold, turn_fold_elapsed, turn_fold_summary,
    turn_fold_title, LiveActivityRow, TurnFold, TurnFoldIntent,
};
pub use approval::{approval_card, ApprovalCard};
pub use handoff::{
    handoff_card, handoff_confirm, handoff_confirm_destination, HandoffCard, HandoffIntent, HANDOFF_CONFIRM_WIDTH,
    HANDOFF_FRESH_NOTE,
};
pub use question::{answered_row, question_card, AnsweredRow, QuestionCard, QuestionOutcome};
pub use item::{
    generic_hidden_lines, generic_item_card, generic_preview_lines, generic_visible_lines,
    goal_card, GenericItemCard, GenericItemIntent, GoalCard, GENERIC_BODY_CAP,
    GENERIC_PREVIEW_LINES,
};
pub use plan::{plan_card, PlanCard};
pub use todo::{todo_list, TodoList};
pub use code::{code_block, diff_block, diff_note, diff_note_inset, runnable_command, CodeBlock, CodeBlockAction, CodeBlockHostButton, DiffBlock, DiffBlockAction, DiffNote, NoteInsets};
pub use syntax::{syntax_runs, syntax_runs_in, token_color, tokenize_line, tokenize_line_in, ts_language, TokenKind};
pub use summary::{summary_card, SummaryAction, SummaryCard};
pub use status::{error_card, jump_pill, needs_you_banner, retry_row, status_row, ErrorCard, JumpPill, NeedsYouBanner, StatusLead, StatusRow};
