//! Shared data-display primitives every card is built from (`base.css`):
//! buttons, chips, pills, tags, status dots, kbd, avatars, status glyphs, the
//! provider usage meter and the context-window meter. Each helper is named for its purpose; none is
//! reused across unrelated components.
//!
//! All of them are stateless [`gpui::RenderOnce`] elements that read the
//! active palette from [`aui_tokens::ActiveAui`] and animate through
//! [`aui_motion`].

mod avatar;
mod button;
mod chip;
mod context_meter;
mod dot;
mod glyph;
mod kbd;
mod meter;
mod pill;
mod secret_field;
mod tag;

pub use avatar::{avatar, Avatar};
pub use button::{button, icon_button, icon_content_button, Button, ButtonSize, ButtonVariant};
pub use chip::{chip, Chip};
pub use context_meter::{context_meter, ContextMeter, ContextMeterState, ContextPressure};
pub use dot::{status_dot, StatusDot};
pub use glyph::{glyph_err, glyph_ok, spinner, Glyph, GlyphKind, Spinner};
pub use kbd::{kbd, Kbd};
pub use meter::{usage_meter, UsageMeter};
pub use pill::{pill, Pill, PillVariant};
pub use secret_field::{secret_field, SecretField};
pub use tag::{tag, Tag};
