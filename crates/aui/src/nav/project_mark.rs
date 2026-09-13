//! The project mark: a rounded square in the project's label colour with
//! the project's initial in the theme background colour. One shape serves
//! the sidebar header switcher, the project group rows, the rail tiles'
//! palette cousins and the command palette's project rows, so the mark
//! reads the same everywhere.

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, Hsla, IntoElement, Pixels, SharedString, Window};

/// The default mark: 18 px, as the sidebar header and group rows draw it.
const SIZE: f32 = 18.0;
/// The initial at the default size: 11 px semibold.
const FONT: f32 = scale::FS_11;

/// A project mark. Build with [`project_mark`].
#[derive(IntoElement)]
pub struct ProjectMark {
    initial: SharedString,
    colour: Hsla,
    size: Pixels,
}

/// A rounded square in `colour` with `initial` centred in the theme
/// background colour. Stateless; no click handling — the caller owns the
/// interaction.
pub fn project_mark(initial: impl Into<SharedString>, colour: Hsla) -> ProjectMark {
    ProjectMark { initial: initial.into(), colour, size: px(SIZE) }
}

impl ProjectMark {
    /// Overrides the square's side: 14 for menu rows and the palette, 18
    /// for the header and group rows, 22 for the rail. The initial scales
    /// with it.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for ProjectMark {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let font = FONT * f32::from(self.size) / SIZE;
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(self.size)
            .rounded(px(scale::R_SM))
            .bg(self.colour)
            .text_color(p.bg)
            .font_family(scale::FONT_UI)
            .text_px(font)
            .line_height(gpui::relative(1.0))
            .semibold()
            .child(self.initial)
    }
}
