//! `.btn`: secondary (filled, bordered, hairline shadow) by default; primary
//! (accent), ghost (quiet icon/text), outline and danger (outlined) variants;
//! md 28 / sm 24 / xs 20 heights; square icon buttons; the press spring.

use aui_icons::{icon, IconName};
use aui_motion::{spring_phase, tween, SpringKind, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, Hsla, IntoElement, Pixels, SharedString, Window};

use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// `.btn` variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Filled surface-2, line-strong border, hairline shadow (the default).
    #[default]
    Secondary,
    /// Accent fill, white text.
    Primary,
    /// Transparent, ink-2 text; surface-2 on hover.
    Ghost,
    /// Transparent with the line-strong border, no shadow.
    Outline,
    /// Transparent, danger text, danger-soft border.
    Danger,
}

/// `.btn` heights: md 28 (padding 11, 13 px), sm 24 (9, 12 px), xs 20 (7, 11 px).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    /// 28 px.
    #[default]
    Md,
    /// 24 px.
    Sm,
    /// 20 px.
    Xs,
}

impl ButtonSize {
    fn height(self, cx: &App) -> Pixels {
        let m = &cx.aui().metrics;
        match self {
            ButtonSize::Md => m.control_md,
            ButtonSize::Sm => m.control_sm,
            ButtonSize::Xs => m.control_xs,
        }
    }

    fn padding(self) -> f32 {
        match self {
            ButtonSize::Md => 11.0,
            ButtonSize::Sm => 9.0,
            ButtonSize::Xs => 7.0,
        }
    }

    fn font_size(self) -> f32 {
        match self {
            ButtonSize::Md => scale::FS_13,
            ButtonSize::Sm => scale::FS_12,
            ButtonSize::Xs => scale::FS_11,
        }
    }

    /// The glyph inside an icon button: 14 px in md and sm, 12 px in xs.
    fn icon_size(self) -> f32 {
        match self {
            ButtonSize::Md | ButtonSize::Sm => 14.0,
            ButtonSize::Xs => 12.0,
        }
    }
}

/// `transform: scale(.97)` on `:active`, expressed as an inset because gpui
/// has no element transform.
const PRESS_SCALE: f32 = 0.97;
/// `.btn{gap:6px}`.
const CONTENT_GAP: f32 = 6.0;
/// Disabled controls (`opacity:.45` on the forward arrow in the shell header).
const DISABLED_OPACITY: f32 = 0.45;

/// A button. Build with [`button`] or [`icon_button`].
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<IconName>,
    icon_size: Option<Pixels>,
    trailing: Option<AnyElement>,
    variant: ButtonVariant,
    size: ButtonSize,
    square: bool,
    muted: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

/// A labelled button (secondary by default).
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button {
        id: id.into(),
        label: Some(label.into()),
        icon: None,
        icon_size: None,
        trailing: None,
        variant: ButtonVariant::Secondary,
        size: ButtonSize::Md,
        square: false,
        muted: false,
        disabled: false,
        on_click: None,
    }
}

/// A square icon button (`.btn.icon`), secondary by default; call
/// [`Button::ghost`] for the quiet header / row kind.
pub fn icon_button(id: impl Into<ElementId>, glyph: IconName) -> Button {
    let mut b = button(id, "");
    b.label = None;
    b.icon = Some(glyph);
    b.square = true;
    b
}

impl Button {
    /// Sets the variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Accent fill.
    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    /// Quiet, transparent.
    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    /// Outlined, no fill.
    pub fn outline(self) -> Self {
        self.variant(ButtonVariant::Outline)
    }

    /// Outlined in danger.
    pub fn danger(self) -> Self {
        self.variant(ButtonVariant::Danger)
    }

    /// Sets the height.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// 24 px.
    pub fn sm(self) -> Self {
        self.size(ButtonSize::Sm)
    }

    /// 20 px.
    pub fn xs(self) -> Self {
        self.size(ButtonSize::Xs)
    }

    /// A leading glyph before the label.
    pub fn icon(mut self, glyph: IconName) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Overrides the glyph size (the shell header uses 12 px glyphs in 20 px buttons).
    pub fn icon_size(mut self, size: impl Into<Pixels>) -> Self {
        self.icon_size = Some(size.into());
        self
    }

    /// Something after the label: a kbd, a chevron.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// Ghost buttons in headers are ink-3 instead of ink-2 (`.hd .btn.ghost`).
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// Non-interactive at 45 % opacity.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Click handler.
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

/// Resting and hover colours of a variant.
struct Look {
    bg: Hsla,
    bg_hover: Hsla,
    border: Hsla,
    border_hover: Hsla,
    text: Hsla,
    text_hover: Hsla,
    shadow: bool,
}

impl Look {
    fn of(variant: ButtonVariant, muted: bool, p: &Palette) -> Self {
        let transparent = gpui::transparent_black();
        match variant {
            ButtonVariant::Secondary => Look {
                bg: p.surface_2,
                bg_hover: p.surface_3,
                border: p.line_strong,
                border_hover: p.ink_4,
                text: p.ink,
                text_hover: p.ink,
                shadow: true,
            },
            ButtonVariant::Primary => Look {
                bg: p.accent,
                bg_hover: p.accent_strong,
                border: transparent,
                border_hover: transparent,
                text: gpui::white(),
                text_hover: gpui::white(),
                shadow: true,
            },
            ButtonVariant::Ghost => Look {
                bg: transparent,
                bg_hover: p.surface_2,
                border: transparent,
                border_hover: transparent,
                text: if muted { p.ink_3 } else { p.ink_2 },
                text_hover: p.ink,
                shadow: false,
            },
            ButtonVariant::Outline => Look {
                bg: transparent,
                bg_hover: p.surface_2,
                border: p.line_strong,
                border_hover: p.ink_4,
                text: p.ink,
                text_hover: p.ink,
                shadow: false,
            },
            ButtonVariant::Danger => Look {
                bg: transparent,
                bg_hover: p.danger_soft,
                border: p.danger_soft,
                border_hover: p.danger,
                text: p.danger,
                text_hover: p.danger,
                shadow: false,
            },
        }
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let look = Look::of(self.variant, self.muted, &p);
        let height = self.size.height(cx);
        let id = self.id.clone();

        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let hovered = flags.hovered && !self.disabled;
        let pressed = flags.pressed && !self.disabled;

        // Hover tints tween (fast · std); the press is the press spring.
        let bg = tween((id.clone(), "bg"), if hovered { look.bg_hover } else { look.bg }, Tween::FAST, window, cx);
        let border = tween((id.clone(), "border"), if hovered { look.border_hover } else { look.border }, Tween::FAST, window, cx);
        let text = tween((id.clone(), "text"), if hovered { look.text_hover } else { look.text }, Tween::FAST, window, cx);
        let press = spring_phase((id.clone(), "press"), pressed, SpringKind::Press, window, cx).clamp(0.0, 1.0);
        let inset = height * ((1.0 - PRESS_SCALE) / 2.0) * press;

        let pad = if self.square { px(0.0) } else { px(self.size.padding()) - inset };
        let glyph_size = self.icon_size.unwrap_or(px(self.size.icon_size()));

        let mut inner = div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(CONTENT_GAP))
            .px(pad)
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_color(text)
            .font_family(scale::FONT_UI)
            .text_px(self.size.font_size())
            .line_height(gpui::relative(1.0))
            .medium()
            .whitespace_nowrap();
        if look.shadow {
            inner = inner.shadow(p.shadow(1));
        }
        if let Some(glyph) = self.icon {
            inner = inner.child(icon(glyph).size(glyph_size).color(text));
        }
        if let Some(label) = self.label.filter(|l| !l.is_empty()) {
            inner = inner.child(label);
        }
        if let Some(trailing) = self.trailing {
            inner = inner.child(trailing);
        }

        let mut outer = div()
            .id(id)
            .flex_none()
            .h(height)
            .p(inset)
            .when(self.square, |d| d.w(height))
            .when(self.disabled, |d| d.opacity(DISABLED_OPACITY))
            .when(!self.disabled, |d| d.cursor_pointer())
            .child(inner);
        if !self.disabled {
            outer = outer.track_interaction(&state);
            if let Some(on_click) = self.on_click {
                outer = outer.on_click(move |e, w, cx| on_click(e, w, cx));
            }
        }
        outer
    }
}
