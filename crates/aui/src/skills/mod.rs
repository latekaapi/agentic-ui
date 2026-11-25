//! Skills page: the list rows, scope sections, cost meter, detail pane,
//! import preview and add-skill menu rows (harness docs `15-skills.md` §4).
//!
//! Every component is a stateless [`gpui::RenderOnce`] element that reads the
//! active palette from [`aui_tokens::ActiveAui`], animates through
//! [`aui_motion`] and emits intents out; the caller owns all state. Menus ride
//! in [`crate::overlay::popover_layer`] at the point where they are anchored.
//!
//! The switch, the tri-state checkbox and the segmented control are the
//! library's form controls ([`crate::data`]), re-exported here because the
//! app imports them from this path. The mode chip is the data
//! [`crate::data::chip`] with the chip + menu pattern.

mod catalog;
mod controls;
mod import;
mod row;
mod sections;

pub use crate::data::{checkbox, segmented, switch, Checkbox, CheckboxIntent, CheckboxState, Segmented, Switch, SwitchIntent};
pub use catalog::{catalog_row, CatalogIntent, CatalogProvider, CatalogReach, CatalogRow, CatalogRowModel};
pub use controls::mode_chip;
pub use import::{
    added_tokens, default_primary_label, format_tokens, import_preview, menu_row_two_line, selected_rows, ImportPreview,
    ImportPreviewIntent, ImportRow, ImportStatus, MenuTwoLineRow,
};
pub use row::{skill_row, ChipTone, SkillChip, SkillMode, SkillRow, SkillRowIntent, SkillRowModel};
pub use sections::{
    cost_meter, scope_section_header, segment_widths, skill_detail, CostMeter, CostSegment, DetailAction, DetailSection,
    ScopeSectionHeader, SkillDetail, SkillDetailIntent, SkillState, InkLevel,
};
