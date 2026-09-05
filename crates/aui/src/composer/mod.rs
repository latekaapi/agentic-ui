//! Composer: the auto-growing input with context chips, the `+` menu, model /
//! mode / effort chips, the send ↔ stop morph, queued messages and suggestion
//! chips (cards 40–42, spec §4). The docked variant used by the shell lives in
//! [`crate::shell::DockedComposer`] until the two are merged.

mod composer;
mod menu;
mod queue;

pub use composer::{composer, composer_state, composer_state_rows, Composer, ComposerChip, ComposerChipKind, ComposerIntent, ComposerMeta};
pub use menu::{plus_menu, PlusMenu, PlusMenuItem};
pub use queue::{queue_row, suggestion_chips, QueueIntent, QueueRow, SuggestionChips};
