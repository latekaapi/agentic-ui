//! Workbench: the block terminal, terminal tabs and dock, and TUI pane,
//! browser with annotator, diff review, git and PR forms, file tree and
//! document panes, sources and citations (cards 50–55, spec §5).

mod browser;
mod changes;
mod citations;
mod code_view;
mod diff_review;
mod docs;
mod file_card;
mod files;
mod git;
mod output_view;
mod page;
mod pane;
mod pdf;
mod sheet;
mod terminal;
mod terminal_dock;
mod tui;

pub use terminal::{block_terminal, BlockState, BlockTerminal, TermBlock, TermPrompt, TerminalAction};
pub use terminal_dock::{terminal_dock, terminal_tabs, TermTab, TerminalDock, TerminalDockAction, TerminalTabs, TerminalTabsAction};
pub use tui::{tui_pane, TuiPane};
pub use browser::{
    agent_action_pill, annotation_pin, annotations_panel, browser_nav, element_outline, note_popover, AgentActionPill, Annotation, AnnotationPin,
    AnnotationsPanel, AnnotatorAction, BrowserAction, BrowserNav, ElementOutline, NoteAction, NotePopover,
};
pub use diff_review::{diff_review, segmented, DiffHighlight, DiffReview, DiffReviewAction, DiffScope, DiffView, ReviewFile, ReviewNote, Segmented};
pub use git::{git_changes, pr_check, pr_form, GitAction, GitChanges, PrAction, PrCheck, PrDescription, PrForm};
pub use file_card::{file_card, FileCard};
pub use files::{file_tree, FileNode, FileTree, FileTreeAction, GitBadge};
pub use docs::{
    artifact_strip, doc_pane, doc_saved_hint, doc_tabs, doc_toolbar, pane_status, pane_status_row, Artifact, ArtifactKind, ArtifactStrip,
    DocBlock, DocCell, DocImage, DocPage, DocPane, DocRun, DocTable, DocTabs, DocToolbar, DocToolbarAction, FormatMark, PaneStatusItem,
    PaneStatusRow, cell_span_width, clamp_heading_level, collapsed_margin, doc_select_all, doc_selected_text, doc_span_selected_text,
    flush_paper_layout, heading_text_size, table_layout, TABLE_MIN_COL_W,
};
pub use citations::{cited_answer, citation, source_hover_card, sources_card, Citation, CitedAnswer, Source, SourceHoverCard, SourceTier, SourcesCard, ANSWER_MARGIN};
pub use changes::{
    ahead_behind, base_and_head, base_picker, branch_label, draft_checkbox, form_field, well_input, change_letter, change_row, change_with_diff, changes_caption, commit_foot, commit_frame, commit_message_view,
    diff_body, hook_failed_note, hunk_new_start, hunk_old_range, pr_card, primary_action, split_path, text_action, unchanged_between, ChangeRow,
    DiffBody, PrState, CHANGE_FOLDER_MIN_WIDTH,
};
pub use code_view::{code_skeleton, code_view, markdown_view, reveal_top_row, CodeView, REVEAL_LEAD_LINES};
pub use output_view::{group_thousands, image_facts, image_missing, image_view, output_fetching, output_title, output_view, OutputView, OUTPUT_LINE_CAP};
pub use page::{
    address_field_frame, address_frame, agent_pill, annotate_outline, annotate_pin, no_page_state, notes_panel, page_address, page_error_state,
    page_nav_row, split_url, PageNavAction, PageNote, PAGE_NAV_H, PAGE_NAV_NARROW,
};
pub use pane::{
    pane_action, pane_row, pane_row_fade, pane_strip, pane_toggle, pushed_row, pushed_row_with, secondary_action, skeleton_bar, skeleton_rows,
    state_block, strip_moving, strip_position, strip_visible, filter_field, focus_edge, PaneRow, PaneStrip, PushedRow, StateBlock, CONTEXT_MIN_WIDTH, PANE_ROW_FADE,
    PANE_ROW_H, STRIP_GAP, STRIP_SLIDE,
};
pub use pdf::{pdf_pane, PdfAction, PdfControls, PdfPage, PdfPane, PdfRun};
pub use sheet::{sheet_pane, SheetCell, SheetPane};
