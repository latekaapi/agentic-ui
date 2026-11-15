//! `.btn`: secondary (a see-through tint) by default; primary (accent), ghost
//! (quiet icon/text), outline and danger (outlined) variants; md 28 / sm 24 /
//! xs 20 heights; square icon buttons. Hover and press are the one model
//! ([`crate::interact::paint`]): overlays over the resting fill, no tween.
//!
//! Buttons are tab stops and activate on `enter` / `space` while focused (gpui
//! turns an activation keystroke on a focused element into a `ClickEvent`, so
//! the keyboard and the mouse run the same [`Button::on_click`] handler).
//!
//! gpui has no `:focus-visible`, so "the focus came from the keyboard" is
//! approximated with one process-wide flag: any key pressed on a button turns
//! it on, any mouse press on a button turns it off, and the ring paints only
//! while it is on. A mouse press on something that is not a button therefore
//! does not clear it; the ring still never appears from a click on the button
//! itself, which is what the design cares about.

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, Hsla, IntoElement, MouseButton, Pixels, SharedString, Window};

use crate::interact::paint::{PaintStates, StatePaint};
use crate::interact::InteractState;
use crate::util::ClickHandler;

use std::cell::RefCell;

// Test probe for accessible names: gpui offers no accessibility-tree query,
// so a test arms this, draws cards in a real window, and reads back the
// effective AX label of every rendered button — the AX analogue of
// `Window::painted_quads` and the [`crate::transcript::take_drawn_chips`]
// chip probe. Arming records; it changes nothing drawn.
thread_local! {
    static AX_PROBE_ARMED: RefCell<bool> = const { RefCell::new(false) };
    static DRAWN_AX_LABELS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Arms (`true`) or disarms the AX-label probe. Disarmed — the default —
/// nothing is recorded and rendering is byte-for-byte what it was.
pub fn arm_ax_probe(armed: bool) {
    AX_PROBE_ARMED.with(|flag| *flag.borrow_mut() = armed);
}

/// Drains the effective AX labels recorded since the last call, in draw
/// order. An empty string is an icon-only button with no
/// [`Button::accessibility_label`]: a control the accessibility tree has no
/// name for.
pub fn take_ax_labels() -> Vec<String> {
    DRAWN_AX_LABELS.with(|labels| std::mem::take(&mut *labels.borrow_mut()))
}

/// Records one effective AX label when the probe is armed; a no-op
/// otherwise. Interactive `div`s (option rows, fold rows, card headers) call
/// this with the same label they put in `aria_label`, so one probe covers
/// every transcript control, not just [`Button`]s.
pub fn record_ax_label(label: &str) {
    if AX_PROBE_ARMED.with(|flag| *flag.borrow()) {
        DRAWN_AX_LABELS.with(|labels| labels.borrow_mut().push(label.to_owned()));
    }
}

/// `.btn` variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// FF secondary: a see-through tint (white 25 % / black 8 %), no border (the default).
    #[default]
    Secondary,
    /// FF primary: the foreground fill with background-coloured text.
    Primary,
    /// FF ghost: transparent, muted text that turns foreground on hover.
    Ghost,
    /// FF tertiary: transparent with the 1 px border ring.
    Outline,
    /// A tertiary button in the danger colour (FF has no destructive variant).
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
    /// 28 px with the caption's 12 px text: the one action at the end of a
    /// list row or card (D5 providers, `height:28px;font-size:13.2px` at
    /// the app's 1.1 scale).
    Row,
}

impl ButtonSize {
    fn height(self, cx: &App) -> Pixels {
        let m = &cx.aui().metrics;
        match self {
            ButtonSize::Md | ButtonSize::Row => m.control_md,
            ButtonSize::Sm => m.control_sm,
            ButtonSize::Xs => m.control_xs,
        }
    }

    fn padding(self) -> f32 {
        match self {
            ButtonSize::Md => 11.0,
            ButtonSize::Sm | ButtonSize::Row => 9.0,
            ButtonSize::Xs => 7.0,
        }
    }

    fn font_size(self) -> f32 {
        match self {
            ButtonSize::Md => scale::FS_13,
            ButtonSize::Sm | ButtonSize::Row => scale::FS_12,
            ButtonSize::Xs => scale::FS_11,
        }
    }

    /// The glyph inside an icon button: 14 px in md and sm, 12 px in xs.
    fn icon_size(self) -> f32 {
        match self {
            ButtonSize::Md | ButtonSize::Sm | ButtonSize::Row => 14.0,
            ButtonSize::Xs => 12.0,
        }
    }
}

/// `.btn{gap:6px}`.
const CONTENT_GAP: f32 = 6.0;
/// Disabled controls: FF `disabled:opacity-50` (button.tsx:20-23).
const DISABLED_OPACITY: f32 = 0.5;

/// A button. Build with [`button`] or [`icon_button`].
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<IconName>,
    icon_content: Option<AnyElement>,
    icon_size: Option<Pixels>,
    trailing: Option<AnyElement>,
    variant: ButtonVariant,
    size: ButtonSize,
    square: bool,
    disabled: bool,
    accessibility_label: Option<SharedString>,
    tooltip: bool,
    tooltip_right: Option<gpui::Pixels>,
    menu_open: bool,
    toggled: Option<bool>,
    preview: Option<InteractState>,
    on_click: Option<ClickHandler>,
}

/// A labelled button (secondary by default).
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button {
        id: id.into(),
        label: Some(label.into()),
        icon: None,
        icon_content: None,
        icon_size: None,
        trailing: None,
        variant: ButtonVariant::Secondary,
        size: ButtonSize::Md,
        square: false,
        disabled: false,
        accessibility_label: None,
        tooltip: true,
        tooltip_right: None,
        menu_open: false,
        toggled: None,
        preview: None,
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

/// A square icon button with caller-built content in place of the glyph —
/// an [`IconMorph`](aui_motion::IconMorph), for example. The content fills
/// the button's glyph box and inherits its text colour, so it hovers, focuses
/// and presses exactly like the glyph it replaces.
pub fn icon_content_button(id: impl Into<ElementId>, content: impl IntoElement) -> Button {
    let mut b = button(id, "");
    b.label = None;
    b.icon_content = Some(content.into_any_element());
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

    /// Kept for source compatibility: an FF ghost is always muted at rest.
    pub fn muted(self) -> Self {
        self
    }

    /// Non-interactive at 45 % opacity.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The accessible name for this button. Setting it also gives the button
    /// its role, so a screen reader announces name and role together; a
    /// labelled button without one announces its visible label, while an
    /// icon-only button without one stays silent.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Keep the tooltip left of window x `right` (it opens toward the
    /// pointer's left when it would cross): for controls beside a native
    /// page, which draws above any tooltip.
    pub fn tooltip_within(mut self, right: gpui::Pixels) -> Self {
        self.tooltip_right = Some(right);
        self
    }

    /// No tooltip. By default an icon-only button with an accessibility
    /// label shows that label as Fluid Functionalism's tooltip after its
    /// 200 ms delay; a button whose glyph says it all inside a labelled row
    /// can opt out.
    pub fn no_tooltip(mut self) -> Self {
        self.tooltip = false;
        self
    }

    /// The menu this button opens is open: it holds its hover look (FF's
    /// `data-popup-open`) and shows no tooltip, which would otherwise sit
    /// beside the open menu.
    pub fn menu_open(mut self, open: bool) -> Self {
        self.menu_open = open;
        self
    }

    /// A toggle button: `on` holds the selected overlay with the foreground
    /// glyph (the lit look, docs/43 §2.2) and the button announces itself as
    /// pressed or not (`aria-pressed`). Meant for ghost buttons.
    pub fn toggled(mut self, on: bool) -> Self {
        self.toggled = Some(on);
        self
    }

    /// Pins a state for a gallery card or a capture: the hover or pressed
    /// look, or the focus ring, with no pointer or keyboard.
    pub fn preview(mut self, state: InteractState) -> Self {
        self.preview = Some(state);
        self
    }

    /// Click handler.
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

/// The resting colours of a variant (Fluid Functionalism's `button.tsx`:
/// primary / secondary / tertiary / ghost; Danger is ours). Hover and press
/// are not part of the look: they are the one model's overlays over the
/// resting fill ([`StatePaint`]).
struct Look {
    bg: Hsla,
    border: Hsla,
    text: Hsla,
    text_hover: Hsla,
}

impl Look {
    fn of(variant: ButtonVariant, p: &Palette) -> Self {
        let transparent = gpui::transparent_black();
        // FF --tint: see-through, so the button takes on the surface beneath it.
        let tint = if p.is_dark() { gpui::white().alpha(0.25) } else { gpui::black().alpha(0.08) };
        match variant {
            ButtonVariant::Secondary => Look { bg: tint, border: transparent, text: p.fg, text_hover: p.fg },
            // The foreground fill, background text.
            ButtonVariant::Primary => Look { bg: p.accent, border: transparent, text: p.bg, text_hover: p.bg },
            // Muted text that lights on hover.
            ButtonVariant::Ghost => Look { bg: transparent, border: transparent, text: p.fg_muted, text_hover: p.fg },
            // Transparent with the 1 px ring.
            ButtonVariant::Outline => Look { bg: transparent, border: p.line, text: p.fg, text_hover: p.fg },
            // A tertiary button in the danger colour.
            ButtonVariant::Danger => Look { bg: transparent, border: p.line, text: p.danger, text_hover: p.danger },
        }
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let look = Look::of(self.variant, &p);
        let height = self.size.height(cx);
        let id = self.id.clone();

        // One focus handle per button id; disabled buttons are not tab stops.
        let focus = window
            .use_keyed_state((id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
            .tab_stop(!self.disabled);
        let ring = self.preview.is_some_and(|s| s.focused && !self.disabled)
            || (!self.disabled && focus.is_focused(window) && crate::keys::keyboard_nav(cx));

        // Hover, press, the toggled lit look and the open-menu hold are the
        // one model: gpui refinements over the resting fill, no tween.
        let lit = self.toggled == Some(true);
        let base = if look.bg.a > 0.0 { Some(look.bg) } else { None };
        let mut paint = StatePaint::new()
            .ink(look.text, look.text_hover)
            .selected(lit)
            .highlighted(self.menu_open)
            .disabled(self.disabled)
            .preview(self.preview);
        if let Some(base) = base {
            paint = paint.on(base);
        }

        let pad = if self.square { px(0.0) } else { px(self.size.padding()) };
        let glyph_size = self.icon_size.unwrap_or(px(self.size.icon_size()));

        let mut outer = div()
            .id(id)
            .flex_none()
            .h(height)
            .when(self.square, |d| d.w(height))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(CONTENT_GAP))
            .px(pad)
            .rounded(px(scale::SHAPE_ITEM))
            .border_1()
            .border_color(look.border)
            .paint_states(paint, cx)
            .font_family(scale::FONT_UI)
            .text_px(self.size.font_size())
            .line_height(gpui::relative(1.0))
            .whitespace_nowrap()
            .when(self.disabled, |d| d.opacity(DISABLED_OPACITY))
            .when(!self.disabled, |d| d.cursor_pointer());
        // The one focus-visible ring (an outline, not a shadow: a spread
        // shadow shows through the see-through fills).
        if ring {
            outer = outer.relative().child(crate::interact::focus_ring(px(scale::SHAPE_ITEM), &p));
        }
        // The glyph inherits the button's ink, so it lifts with the hover.
        if let Some(content) = self.icon_content {
            outer = outer.child(content);
        } else if let Some(glyph) = self.icon {
            outer = outer.child(icon(glyph).size(glyph_size).lit(lit || self.menu_open));
        }
        if let Some(label) = self.label.clone().filter(|l| !l.is_empty()) {
            outer = outer.child(label);
        }
        if let Some(trailing) = self.trailing {
            outer = outer.child(trailing);
        }

        if !self.disabled {
            outer = outer
                .track_focus(&focus)
                // The `:focus-visible` approximation: keys arm the ring, the
                // mouse disarms it. `window.refresh()` repaints the flip.
                .on_key_down(|_, window, cx| {
                    if !crate::keys::keyboard_nav(cx) {
                        crate::keys::set_keyboard_nav(true, cx);
                        window.refresh();
                    }
                })
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    if crate::keys::keyboard_nav(cx) {
                        crate::keys::set_keyboard_nav(false, cx);
                        window.refresh();
                    }
                });
            if let Some(on_click) = self.on_click {
                // gpui synthesises a `ClickEvent::Keyboard` from enter/space on
                // a focused element, so this one handler serves both inputs.
                outer = outer.on_click(move |e, w, cx| on_click(e, w, cx));
            }
        }
        // Every button names itself for the accessibility tree: the explicit
        // label when one is set, otherwise the visible label. Only an
        // icon-only button with no explicit label stays silent — and the
        // probe below is how the transcript AX test finds those.
        let effective: Option<SharedString> = self
            .accessibility_label
            .clone()
            .or_else(|| self.label.clone().filter(|l| !l.is_empty()));
        record_ax_label(effective.as_deref().unwrap_or(""));
        let icon_only = self.label.as_ref().is_none_or(|l| l.is_empty());
        if let Some(label) = self.accessibility_label.clone().filter(|_| icon_only && self.tooltip && !self.disabled && !self.menu_open) {
            outer = match self.tooltip_right {
                Some(right) => outer.tooltip(super::tooltip::tip_within(label, right)),
                None => outer.tooltip(super::tooltip::tip(label)),
            }
            .tooltip_show_delay(super::tooltip::TIP_DELAY);
        }
        if let Some(label) = effective {
            outer = outer.role(gpui::Role::Button).aria_label(label);
        }
        if let Some(on) = self.toggled {
            outer = outer.aria_toggled(if on { gpui::Toggled::True } else { gpui::Toggled::False });
        }
        outer
    }
}
