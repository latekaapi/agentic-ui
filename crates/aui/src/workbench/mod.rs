//! Workbench: the block terminal, terminal tabs and dock, and TUI pane,
//! browser with annotator, diff review, git and PR forms, file tree and
//! document panes, sources and citations (cards 50–55, spec §5).

mod browser;
mod citations;
mod diff_review;
mod docs;
mod file_card;
mod files;
mod git;
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
pub use pdf::{pdf_pane, PdfAction, PdfControls, PdfPage, PdfPane, PdfRun};
pub use sheet::{sheet_pane, SheetCell, SheetPane};
