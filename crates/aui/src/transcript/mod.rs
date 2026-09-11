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
mod prose;
mod turns;

pub use activity::{activity_group, ActivityGroup};
pub use card::{transcript_card, TranscriptCard};
pub use thinking::{thinking_block, ThinkingBlock};
pub use prose::{caret_top_in_line, caret_visible, prose, ProseStyle, CARET_BASELINE_DROP, CARET_H, CARET_MARGIN_LEFT, CARET_W};
pub use markdown::{last_block_runs, markdown, markdown_selected_text, parse_markdown, parsed_markdown, span_runs, Block, Block as MarkdownBlock, LinkHandler, LinkRange, LinkTarget, Markdown, Span, Span as MarkdownSpan, TableAlign};
pub use selectable::{selectable_text, SelectableText, SelectionHandler, SelectionKey, TextSelection};
pub use marker::{marker_row, HandOff, MarkerRow};
pub use turns::{assistant_turn, turn_selected_text, user_turn, AssistantTurn, AssistantTurnAction, UserTurn, UserTurnAction};
pub use ansi::{ansi_runs, parse_ansi, AnsiSpan};
pub use tool_card::{format_duration, tool_card, ToolCard, ToolCardIntent, SHELL_FOLD};
pub use tool_group::{
    count_label, more_label, preview_hidden, tool_group, ToolGroup, ToolGroupData, ToolGroupIntent,
    GROUP_PREVIEW,
};
pub use approval::{approval_card, ApprovalCard};
pub use question::{answered_row, question_card, AnsweredRow, QuestionCard, QuestionOutcome};
pub use item::{generic_item_card, goal_card, GenericItemCard, GoalCard};
pub use plan::{plan_card, PlanCard};
pub use todo::{todo_list, TodoList};
pub use code::{code_block, diff_block, diff_note, diff_note_inset, CodeBlock, CodeBlockAction, DiffBlock, DiffBlockAction, DiffNote, NoteInsets};
pub use syntax::{syntax_runs, syntax_runs_in, token_color, tokenize_line, tokenize_line_in, ts_language, TokenKind};
pub use summary::{summary_card, SummaryAction, SummaryCard};
pub use status::{error_card, jump_pill, needs_you_banner, retry_row, status_row, ErrorCard, JumpPill, NeedsYouBanner, StatusLead, StatusRow};
