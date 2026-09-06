//! Transcript: markers, user and assistant turns with streaming reveal,
//! thinking block, activity group, tool cards and bodies, approval,
//! question / plan / todo, code and diff blocks, summary / error / status
//! rows, citations and sources (cards 30–38, 55).

mod activity;
mod ansi;
mod approval;
mod card;
mod code;
mod marker;
mod plan;
mod question;
mod status;
mod summary;
mod syntax;
mod thinking;
mod todo;
mod tool_card;
mod prose;
mod turns;

pub use activity::{activity_group, ActivityGroup};
pub use card::{transcript_card, TranscriptCard};
pub use thinking::{thinking_block, ThinkingBlock};
pub use prose::{prose, ProseStyle};
pub use marker::{marker_row, HandOff, MarkerRow};
pub use turns::{assistant_turn, user_turn, AssistantTurn, AssistantTurnAction, UserTurn, UserTurnAction};
pub use ansi::{ansi_runs, parse_ansi, AnsiSpan};
pub use tool_card::{format_duration, tool_card, ToolCard, ToolCardIntent, SHELL_FOLD};
pub use approval::{approval_card, ApprovalCard};
pub use question::{answered_row, question_card, AnsweredRow, QuestionCard};
pub use plan::{plan_card, PlanCard};
pub use todo::{todo_list, TodoList};
pub use code::{code_block, diff_block, diff_note, diff_note_inset, CodeBlock, CodeBlockAction, DiffBlock, DiffBlockAction, DiffNote, NoteInsets};
pub use syntax::{syntax_runs, token_color, tokenize_line, TokenKind};
pub use summary::{summary_card, SummaryAction, SummaryCard};
pub use status::{error_card, jump_pill, needs_you_banner, status_row, ErrorCard, JumpPill, NeedsYouBanner, StatusLead, StatusRow};
