//! Composer: the auto-growing input with context chips, the `+` menu, model /
//! mode / effort chips, the send ↔ stop morph, queued messages and suggestion
//! chips (cards 40–42, spec §4). The docked variant used by the shell lives in
//! [`crate::shell::DockedComposer`] until the two are merged.

// The module is named after the card it implements; the re-export below flattens it away.
#[allow(clippy::module_inception)]
mod composer;
mod attachments;
mod menu;
mod menus;
mod pickers;
mod queue;

pub use composer::{composer, composer_state, composer_state_rows, Composer, ComposerChip, ComposerChipAnchor, ComposerChipKind, ComposerIntent, ComposerMeta};
pub use attachments::{attachment_row, drop_overlay, AttachmentRow, AttachmentRowState, DropOverlay, ROW_STACK_GAP};
pub use menu::{plus_menu, PlusMenu, PlusMenuItem};
pub use menus::{command_menu, mention_picker, CommandItem, CommandMenu, CommandSection, MentionIcon, MentionItem, MentionPicker, MentionSection};
pub use pickers::{effort_menu, mode_menu, model_menu, PickerMenu, PickerRow};
pub use queue::{queue_row, queue_strip, suggestion_chips, QueueIntent, QueueRow, QueueStrip, QueueStripRow, SuggestionChips};
