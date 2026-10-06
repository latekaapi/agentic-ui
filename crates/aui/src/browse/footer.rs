//! The 44 px Browse footer: downloads and extensions ghost buttons,
//! a spacer, and the bordered space chip.

use aui_icons::IconName;
use aui_tokens::ActiveAui;
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;

use crate::data::{chip, icon_button, record_ax_label, ButtonSize};
use crate::util::ClickHandler;

/// The footer height: 44 px with a hairline above.
const FOOTER_H: f32 = 44.0;
/// The footer's horizontal padding and inner gap.
const FOOTER_PAD_X: f32 = 12.0;
const FOOTER_GAP: f32 = 8.0;

/// The 44 px Browse footer for `space_name`. Build with [`browse_footer`].
#[derive(IntoElement)]
pub struct BrowseFooter {
    id: ElementId,
    space: SharedString,
    on_space: Option<ClickHandler>,
    on_downloads: Option<ClickHandler>,
    on_extensions: Option<ClickHandler>,
}

/// The 44 px footer: Downloads, Extensions, spacer, `<Space name> ▾` chip.
pub fn browse_footer(id: impl Into<ElementId>, space_name: impl Into<SharedString>) -> BrowseFooter {
    BrowseFooter { id: id.into(), space: space_name.into(), on_space: None, on_downloads: None, on_extensions: None }
}

impl BrowseFooter {
    /// The space chip was clicked.
    pub fn on_space(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_space = Some(Box::new(f));
        self
    }

    /// The downloads button was clicked.
    pub fn on_downloads(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_downloads = Some(Box::new(f));
        self
    }

    /// The extensions button was clicked.
    pub fn on_extensions(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_extensions = Some(Box::new(f));
        self
    }
}

impl RenderOnce for BrowseFooter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let space_label: SharedString = format!("Space, {}", self.space).into();
        // `Chip` sets the role and `aria-label` but does not feed the AX
        // probe, so the label is recorded here alongside it.
        record_ax_label(&space_label);
        let mut downloads =
            icon_button((id.clone(), "downloads"), IconName::Inbox).ghost().size(ButtonSize::Sm).accessibility_label("Downloads");
        if let Some(on_downloads) = self.on_downloads {
            downloads = downloads.on_click(move |e, w, cx| on_downloads(e, w, cx));
        }
        let mut extensions =
            icon_button((id.clone(), "extensions"), IconName::Layout).ghost().size(ButtonSize::Sm).accessibility_label("Extensions");
        if let Some(on_extensions) = self.on_extensions {
            extensions = extensions.on_click(move |e, w, cx| on_extensions(e, w, cx));
        }
        let mut space = chip((id, "space"), self.space.clone()).chevron().accessibility_label(space_label);
        if let Some(on_space) = self.on_space {
            space = space.on_click(move |e, w, cx| on_space(e, w, cx));
        }
        h_flex()
            .w_full()
            .h(px(FOOTER_H))
            .flex_none()
            .items_center()
            .gap(px(FOOTER_GAP))
            .px(px(FOOTER_PAD_X))
            .border_t_1()
            .border_color(p.line)
            .child(downloads)
            .child(extensions)
            .child(div().flex_1())
            .child(space)
    }
}
