//! `.hd` of a floating or docked panel outside the shell: 36 px, grip, icon,
//! title 600, subtitle ink-3, spacer, quiet xs actions.

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::h_flex;

use crate::data::{icon_button, ButtonSize};

/// `.hd{gap:8px;padding:0 10px 0 12px}`.
const GAP: f32 = 8.0;
const PAD_LEFT: f32 = 12.0;
const PAD_RIGHT: f32 = 10.0;
/// `.grip{width:8px;height:14px}` — ink-4 dots on a 4 px grid.
const GRIP_W: f32 = 8.0;
const GRIP_H: f32 = 14.0;
const GRIP_DOT: f32 = 2.0;
const GRIP_STEP: f32 = 4.0;
/// Action glyphs are 12 px inside 20 px ghost buttons.
const ACTION_GLYPH: f32 = 12.0;

/// A panel header. Build with [`panel_header`].
#[derive(IntoElement)]
pub struct PanelHeader {
    id: ElementId,
    icon: Option<IconName>,
    title: SharedString,
    subtitle: Option<SharedString>,
    grip: bool,
    actions: Vec<AnyElement>,
}

/// A header with a title.
pub fn panel_header(id: impl Into<ElementId>, title: impl Into<SharedString>) -> PanelHeader {
    PanelHeader { id: id.into(), icon: None, title: title.into(), subtitle: None, grip: true, actions: Vec::new() }
}

impl PanelHeader {
    /// The 14 px glyph before the title.
    pub fn icon(mut self, glyph: IconName) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// The ink-3 subtitle after the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Whether to show the drag grip (floating panels).
    pub fn grip(mut self, grip: bool) -> Self {
        self.grip = grip;
        self
    }

    /// A quiet xs icon action at the right.
    pub fn action(mut self, name: &'static str, glyph: IconName, on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.actions.push(
            icon_button((self.id.clone(), name), glyph)
                .ghost()
                .size(ButtonSize::Xs)
                .icon_size(px(ACTION_GLYPH))
                .on_click(on_click)
                .into_any_element(),
        );
        self
    }
}

impl RenderOnce for PanelHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut row = h_flex()
            .id(self.id)
            .w_full()
            .h(cx.aui().metrics.panel_header)
            .flex_none()
            .gap(px(GAP))
            .pl(px(PAD_LEFT))
            .pr(px(PAD_RIGHT))
            .border_b_1()
            .border_color(p.line)
            .text_color(p.ink)
            .ui(scale::FS_13);
        if self.grip {
            let mut grip = div().flex_none().w(px(GRIP_W)).h(px(GRIP_H)).relative().cursor_grab();
            let cols = (GRIP_W / GRIP_STEP) as usize;
            let rows = (GRIP_H / GRIP_STEP) as usize;
            for r in 0..rows {
                for c in 0..cols {
                    grip = grip.child(
                        div()
                            .absolute()
                            .left(px(c as f32 * GRIP_STEP))
                            .top(px(r as f32 * GRIP_STEP))
                            .size(px(GRIP_DOT))
                            .rounded_full()
                            .bg(p.ink_4),
                    );
                }
            }
            row = row.child(grip);
        }
        if let Some(glyph) = self.icon {
            row = row.child(icon(glyph).color(p.ink));
        }
        row = row.child(div().semibold().whitespace_nowrap().child(self.title));
        if let Some(subtitle) = self.subtitle {
            row = row.child(div().text_color(p.ink_3).min_w(px(0.0)).truncate().child(subtitle));
        }
        row.child(div().flex_1()).children(self.actions)
    }
}
