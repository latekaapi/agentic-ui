//! `.tag`: plain mono text with no ground — repo, branch, counts, paths.

use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, App, Hsla, IntoElement, SharedString, Window};

/// A tag. Build with [`tag`].
#[derive(IntoElement)]
pub struct Tag {
    text: SharedString,
    color: Option<Hsla>,
    truncate: Option<f32>,
}

/// Mono 10.5 / 500 / ink-3.
pub fn tag(text: impl Into<SharedString>) -> Tag {
    Tag { text: text.into(), color: None, truncate: None }
}

impl Tag {
    /// Overrides ink-3 (a `+8` in success, a `−3` in danger).
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Truncates with an ellipsis at `max_width` px (`.tag.trunc{max-width:…}`).
    pub fn truncate(mut self, max_width: f32) -> Self {
        self.truncate = Some(max_width);
        self
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut el = div()
            .flex_none()
            .text_role(TextRole::Tag)
            .text_color(self.color.unwrap_or(p.ink_3))
            .whitespace_nowrap();
        if let Some(max) = self.truncate {
            el = el.max_w(gpui::px(max)).min_w(gpui::px(0.0)).truncate();
        }
        el.child(self.text)
    }
}
