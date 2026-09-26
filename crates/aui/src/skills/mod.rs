//! Skills page: the list rows, scope sections, cost meter, detail pane,
//! import preview and add-skill menu rows (harness docs `15-skills.md` §4).
//!
//! Every component is a stateless [`gpui::RenderOnce`] element that reads the
//! active palette from [`aui_tokens::ActiveAui`], animates through
//! [`aui_motion`] and emits intents out; the caller owns all state. Menus ride
//! in [`crate::overlay::popover_layer`] at the point where they are anchored.
//!
//! Controls are only defined here where the library had none: the `switch`
//! below wraps the same gpui-kit `Switch` the settings dialog's switch rows
//! use, and [`segmented`](crate::workbench::segmented) is the diff review
//! track (now with `←` / `→` and radio roles), re-exported so the page shares
//! the one control. The tri-state [`checkbox`] is new: gpui-kit's box is
//! boolean-only. The mode chip is the data [`crate::data::chip`] with the
//! chip + menu pattern.

mod controls;
mod import;
mod row;
mod sections;

pub use controls::{checkbox, mode_chip, switch, Checkbox, CheckboxIntent, CheckboxState, Switch, SwitchIntent};
pub use import::{
    added_tokens, default_primary_label, format_tokens, import_preview, menu_row_two_line, selected_rows, ImportPreview,
    ImportPreviewIntent, ImportRow, ImportStatus, MenuTwoLineRow,
};
pub use row::{skill_row, ChipTone, SkillChip, SkillMode, SkillRow, SkillRowIntent, SkillRowModel};
pub use sections::{
    cost_meter, scope_section_header, segment_widths, skill_detail, CostMeter, CostSegment, DetailAction, DetailSection,
    ScopeSectionHeader, SkillDetail, SkillDetailIntent, SkillState, InkLevel,
};
pub use crate::workbench::{segmented, Segmented};
