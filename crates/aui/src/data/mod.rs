//! Shared data-display primitives every card is built from (`base.css`):
//! buttons, chips, pills, tags, status dots, kbd, avatars, status glyphs, the
//! provider usage meter and the context-window meter. Each helper is named for its purpose; none is
//! reused across unrelated components.
//!
//! The flat primitives of docs/83 live here too, by kind: the static
//! containers ([`card`], [`section`], [`settings_row`], [`banner`]), the
//! input frame ([`field`] / [`field_frame`]) and the form controls
//! ([`switch`], [`checkbox`], [`segmented`], [`tabs`]). The pointable list
//! and float primitives ([`list_row`](crate::interact::list_row),
//! [`menu_panel`](crate::interact::menu_panel)) live in
//! [`crate::interact`] beside the one hover model.
//!
//! All of them are stateless [`gpui::RenderOnce`] elements that read the
//! active palette from [`aui_tokens::ActiveAui`] and animate through
//! [`aui_motion`].

mod avatar;
mod button;
mod card;
mod chip;
mod context_meter;
mod controls;
mod dot;
mod field;
mod glyph;
mod kbd;
mod meter;
mod pill;
mod secret_field;
mod tag;
mod tooltip;

pub use avatar::{avatar, Avatar};
pub use button::{arm_ax_probe, button, icon_button, icon_content_button, record_ax_label, take_ax_labels, Button, ButtonSize, ButtonVariant};
pub use chip::{chip, Chip};
pub use context_meter::{context_meter, ContextMeter, ContextMeterState, ContextPressure};
pub use dot::{status_dot, StatusDot};
pub use card::{
    banner, card, section, settings_row, Banner, Card, Section, SettingsRow, BANNER_H, BANNER_PAD_X, CARD_HEADER_H, CARD_PAD_X,
    CARD_PAD_Y, R_CARD, SECTION_TITLE_GAP, SETTINGS_ROW_GAP, SETTINGS_ROW_PAD_Y,
};
pub use controls::{
    checkbox, segmented, switch, tabs, Checkbox, CheckboxIntent, CheckboxState, Segmented, Switch, SwitchIntent, Tabs,
};
pub use field::{field, field_frame, FIELD_H};
pub use glyph::{glyph_err, glyph_ok, spinner, Glyph, GlyphKind, Spinner};
pub use kbd::{kbd, Kbd};
pub use meter::{usage_meter, UsageMeter};
pub use pill::{pill, Pill, PillVariant};
pub use secret_field::{secret_field, SecretField};
pub use tag::{tag, Tag};
pub use tooltip::{tip, tip_within, Tip, TIP_DELAY};
