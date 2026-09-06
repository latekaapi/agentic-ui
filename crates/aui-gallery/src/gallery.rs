//! The gallery window: title bar with theme/density switches, an entry
//! sidebar, and a stage that shows the selected card at its declared size.

use aui_tokens::{scale, ActiveAui, AuiStyled, AuiTheme, TextRole};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::TitleBar;

use crate::registry::{self, Entry, ENTRIES};

/// Root view of the gallery.
pub struct Gallery {
    selected: &'static Entry,
    /// Screenshot mode: no chrome, the card fills the window.
    bare: bool,
    focus: FocusHandle,
}

impl Gallery {
    /// Creates the gallery, opening `entry` (or the first one).
    pub fn new(entry: Option<&'static Entry>, bare: bool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            selected: entry.unwrap_or(&ENTRIES[0]),
            bare,
            focus: cx.focus_handle(),
        }
    }

    fn select(&mut self, entry: &'static Entry, _window: &mut Window, cx: &mut Context<Self>) {
        self.selected = entry;
        cx.notify();
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.aui();
        let colors = t.colors;
        let kind = t.kind;
        let density = t.density;
        let text_scale = t.text_scale;
        TitleBar::new().child(
            h_flex()
                .w_full()
                .h_full()
                .items_center()
                .gap(px(scale::SP_3))
                .pr(px(scale::SP_4))
                .child(div().text_role(TextRole::UiMedium).text_color(colors.ink).child("aui gallery"))
                .child(
                    div()
                        .text_role(TextRole::UiSmall)
                        .text_color(colors.ink_3)
                        .child(format!("{} · {}", self.selected.group, self.selected.title)),
                )
                .child(div().flex_1())
                .child(chrome_button(
                    "theme",
                    format!("Theme: {}", kind.label()),
                    &colors,
                    cx.listener(|_, _, window, cx| AuiTheme::toggle_kind(Some(window), cx)),
                ))
                .child(chrome_button(
                    "text-scale",
                    format!("Text: {}%", (text_scale * 100.0).round()),
                    &colors,
                    cx.listener(move |_, _, window, cx| {
                        // 100 → 110 → 120 → 130 → 100
                        let next = if text_scale >= 1.29 { 1.0 } else { text_scale + 0.1 };
                        AuiTheme::set_text_scale(next, Some(window), cx)
                    }),
                ))
                .child(chrome_button(
                    "density",
                    format!("Density: {}", density.label()),
                    &colors,
                    cx.listener(move |_, _, window, cx| {
                        AuiTheme::set_density(density.toggled(), Some(window), cx)
                    }),
                )),
        )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.aui().colors;
        let row_h = cx.aui().metrics.row;
        let mut list = v_flex()
            .id("gallery-sidebar")
            .w(px(232.0))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .bg(colors.surface_1)
            .border_r_1()
            .border_color(colors.line)
            .py(px(scale::SP_3));
        for group in registry::groups() {
            list = list.child(
                div()
                    .h(px(28.0))
                    .px(px(scale::SP_5))
                    .flex()
                    .items_center()
                    .text_role(TextRole::Caps)
                    .text_color(colors.ink_3)
                    .child(group.to_uppercase()),
            );
            for entry in ENTRIES.iter().filter(|e| e.group == group) {
                let selected = std::ptr::eq(entry, self.selected);
                list = list.child(
                    div()
                        .id(SharedString::from(entry.id))
                        .h(row_h)
                        .mx(px(scale::SP_3))
                        .px(px(scale::SP_3))
                        .rounded(px(scale::R_MD))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_role(if selected { TextRole::UiMedium } else { TextRole::Ui })
                        .text_color(if selected { colors.ink } else { colors.ink_2 })
                        .when(selected, |d| d.bg(colors.surface_3))
                        .hover(|d| d.bg(if selected { colors.surface_3 } else { colors.surface_2 }))
                        .on_click(cx.listener(move |this, _, window, cx| this.select(entry, window, cx)))
                        .child(div().truncate().child(entry.title)),
                );
            }
        }
        list
    }

    fn render_card(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entry = self.selected;
        // Cards follow the active theme; the design's theme only picks the
        // default for a parity screenshot.
        let colors = cx.aui().colors;
        div()
            .w(px(entry.width))
            .h(px(entry.height))
            .flex_none()
            .overflow_hidden()
            .bg(colors.bg)
            .text_color(colors.ink)
            .text_role(TextRole::Ui)
            .when(!entry.full_bleed(), |d| d.p(px(20.0)))
            .child((entry.build)(window, cx))
    }

    fn render_stage(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entry = self.selected;
        let colors = cx.aui().colors;
        v_flex()
            .id("gallery-stage")
            .flex_1()
            .h_full()
            .min_w(px(0.0))
            .overflow_scroll()
            .bg(colors.bg)
            .p(px(scale::SP_6))
            .gap(px(scale::SP_4))
            .child(
                v_flex()
                    .gap(px(scale::SP_1))
                    .child(div().text_role(TextRole::Title).text_color(colors.ink).child(entry.title))
                    .child(div().text_role(TextRole::UiSmall).text_color(colors.ink_3).child(entry.subtitle))
                    .child(
                        div()
                            .text_role(TextRole::Tag)
                            .text_color(colors.ink_3)
                            .child(format!("{} · {}×{} · {:?}", entry.id, entry.width, entry.height, entry.theme)),
                    ),
            )
            .child(
                // A hairline around the card only; the card is the same ground as the stage.
                div()
                    .flex_none()
                    .w(px(entry.width + 2.0))
                    .rounded(px(scale::R_LG))
                    .border_1()
                    .border_color(colors.line)
                    .overflow_hidden()
                    .child(self.render_card(window, cx)),
            )
    }
}

fn chrome_button(
    id: &'static str,
    label: String,
    colors: &aui_tokens::Palette,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let (surface_2, surface_3, line_strong, ink) = (colors.surface_2, colors.surface_3, colors.line_strong, colors.ink);
    div()
        .id(id)
        .h(px(scale::H_SM))
        .px(px(9.0))
        .rounded(px(scale::R_SM))
        .border_1()
        .border_color(line_strong)
        .bg(surface_2)
        .flex()
        .items_center()
        .cursor_pointer()
        .text_role(TextRole::UiMedium)
        .text_px(scale::FS_12)
        .text_color(ink)
        .hover(move |d| d.bg(surface_3))
        .on_click(on_click)
        .child(label)
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.aui().colors;
        if self.bare {
            // Parity captures: an occluding overlay keeps the pointer from
            // hovering anything in the card.
            return div()
                .size_full()
                .relative()
                .child(self.render_card(window, cx))
                .child(div().absolute().inset_0().occlude())
                .into_any_element();
        }
        v_flex()
            .track_focus(&self.focus)
            .size_full()
            .bg(colors.bg)
            .text_color(colors.ink)
            .child(self.render_title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .child(self.render_sidebar(cx))
                    .child(self.render_stage(window, cx)),
            )
            .into_any_element()
    }
}
