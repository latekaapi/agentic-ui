//! The skills page's own control: the mode chip. The switch, the tri-state
//! checkbox and the segmented control are the library's form controls
//! ([`crate::data`]).

use gpui::{ElementId, SharedString};

use crate::data::chip;

use super::row::SkillMode;

/// The mode chip: `Auto` or `Only /{name}`, a data [`chip`](crate::data::chip)
/// with a chevron that opens its menu in
/// [`popover_layer`](crate::overlay::popover_layer). The menu itself is the
/// caller's (the skill row hangs a two-row `Automatic` / `Only /{name}` menu
/// off it); this only builds the chip with its human label.
pub fn mode_chip(id: impl Into<ElementId>, mode: &SkillMode, name: impl Into<SharedString>) -> crate::data::Chip {
    let name = name.into();
    let label = SharedString::from(mode.chip_label(&name));
    chip(id, label.clone())
        .chevron()
        .accessibility_label(SharedString::from(format!("Skill mode, {label}. Opens the mode menu.")))
}

