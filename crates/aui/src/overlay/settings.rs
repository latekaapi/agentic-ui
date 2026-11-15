//! The settings data the app's own Settings page draws from: sections, rows
//! and the shortcut edits a row reports. The modal settings dialog that once
//! rendered these was deleted in docs/82 1.3 — Baaz draws its own page
//! (`baaz/src/settings.rs`) — so only the data stays here.

use gpui::SharedString;

/// One section of the dialog: a rail row and its page.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSection {
    /// Stable identity, reported by [`SettingsDialog::on_select_section`].
    pub id: SharedString,
    /// The rail label.
    pub label: SharedString,
    /// The page rows, in order.
    pub rows: Vec<SettingsRow>,
}

/// One row of a settings page.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsRow {
    /// A labelled switch: the label in ink, the optional detail in fg-muted
    /// mono 11 under it, the switch at the right.
    Switch {
        /// Stable identity, reported by [`SettingsDialog::on_switch`].
        id: SharedString,
        /// The row label.
        label: SharedString,
        /// An optional second line under the label.
        detail: Option<SharedString>,
        /// Whether the switch is on. The caller owns this; the dialog only
        /// reports flips.
        on: bool,
    },
    /// A shortcut row: the label in ink with its detail under it, exactly the
    /// switch row's layout, and the binding at the right — keycaps plus a
    /// clear affordance when bound, an "unassigned" placeholder when not, a
    /// "Press a key…" pill in the accent role while recording, dimmed
    /// keycaps with no press target when reserved.
    Shortcut {
        /// Stable identity, reported back through `on_shortcut`.
        id: SharedString,
        /// The row label, e.g. "Open the command palette".
        label: SharedString,
        /// An optional second line under the label.
        detail: Option<SharedString>,
        /// The current binding as gpui's `KeyBinding` spells it, e.g.
        /// `"cmd-shift-k"`; `None` when the action is unbound.
        keystroke: Option<SharedString>,
        /// True while this row is capturing. The host owns this; the dialog
        /// only reports what happened.
        recording: bool,
        /// False when the binding is reserved and refuses to be rebound.
        /// A reserved row still shows its keystroke; it just cannot be pressed.
        editable: bool,
    },
    /// A muted paragraph under a group.
    Note {
        /// The paragraph.
        text: SharedString,
    },
    /// A caps group label inside a section.
    Heading {
        /// The label.
        text: SharedString,
    },
}

/// What a shortcut row asks for, reported through
/// [`SettingsDialog::on_shortcut`]. The first argument to the handler is the
/// row's [`SettingsRow::Shortcut`] `id`; the host owns all state and stores
/// the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutEdit {
    /// The row was pressed. The host should set `recording` on that row.
    Record,
    /// A keystroke was captured while recording, as gpui spells it (see
    /// [`SettingsDialog::on_shortcut`]): the host should store it as the
    /// row's binding and clear `recording`.
    Set(SharedString),
    /// The clear affordance was pressed: unbind the action.
    Clear,
    /// Recording ended without a binding — `esc` while recording, or the
    /// dialog losing focus. The dialog reports `esc` itself and never
    /// dismisses for it; a host that moves focus away from the dialog
    /// mid-recording should report this itself, since a stateless component
    /// cannot observe the blur. Either way the host should clear `recording`.
    Cancel,
}
