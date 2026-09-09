//! Sidebar building blocks shared by cards 10, 21, 22 and 23: the primary
//! nav rows (`.nav .it`), the group rows (`.grp`), the chevron on the swap
//! spring and the account footer.

use aui_icons::{icon, IconName, Provider};
use aui_motion::{spring_phase, tint_fade, tween, SpringKind, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, radians, AnyElement, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{avatar, icon_button, tag, usage_meter, ButtonSize};
use crate::util::{interaction_flags, ClickHandler, TrackInteraction};

/// `.nav .it{gap:8px;height:30px;padding:0 8px}`.
const NAV_GAP: f32 = 8.0;
const NAV_PAD: f32 = 8.0;
/// `.grp{gap:6px;height:28px;padding:0 12px}`.
const GROUP_H: f32 = 28.0;
const GROUP_GAP: f32 = 6.0;
const GROUP_PAD: f32 = 12.0;
/// `.chev{width:12px;height:12px}` — the default; rows that override it
/// (`.pj .chev` 11 px, `.n .chev` 10 px) pass their own size to
/// [`chevron_sized`].
pub const CHEVRON: f32 = 12.0;
/// xs ghost buttons on group rows carry 12 px glyphs.
const XS_GLYPH: f32 = 12.0;
/// Footer: `padding:10px 12px` (shell) / `8px 12px` (sidebar cards), gap 8.
const FOOTER_PAD_X: f32 = 12.0;
const FOOTER_GAP: f32 = 8.0;

/// A `.chev` glyph rotated 0° (closed) → 90° (open) on the swap spring, at the
/// default 12 px.
pub fn chevron(id: impl Into<ElementId>, open: bool, color: Hsla, window: &mut Window, cx: &mut App) -> impl IntoElement {
    chevron_sized(id, open, color, CHEVRON, window, cx)
}

/// [`chevron`] at an explicit `size` in design px, for the rows whose CSS
/// narrows the glyph (`.pj .chev` 11 px, the file tree's `.n .chev` 10 px).
pub fn chevron_sized(id: impl Into<ElementId>, open: bool, color: Hsla, size: f32, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let id: ElementId = id.into();
    let phase = spring_phase((id, "chevron"), open, SpringKind::Swap, window, cx);
    icon(IconName::Chev).size(px(size)).color(color).rotate(radians(phase * std::f32::consts::FRAC_PI_2))
}

/// A primary nav row. Build with [`nav_item`].
#[derive(IntoElement)]
pub struct NavItem {
    id: ElementId,
    icon: IconName,
    label: SharedString,
    count: Option<SharedString>,
    count_warning: bool,
    on_click: Option<ClickHandler>,
}

/// `Tasks`, `Automations`, `Inbox`…
pub fn nav_item(id: impl Into<ElementId>, glyph: IconName, label: impl Into<SharedString>) -> NavItem {
    NavItem { id: id.into(), icon: glyph, label: label.into(), count: None, count_warning: false, on_click: None }
}

impl NavItem {
    /// The mono count at the right.
    pub fn count(mut self, count: impl Into<SharedString>) -> Self {
        self.count = Some(count.into());
        self
    }

    /// Colours the count in warning (the inbox with items that need the person).
    pub fn count_warning(mut self) -> Self {
        self.count_warning = true;
        self
    }

    /// Click handler.
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for NavItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, flags) = interaction_flags(id.clone(), window, cx);
        let bg = tint_fade((id.clone(), "bg"), flags.hovered, p.surface_2, Tween::FAST, window, cx);
        let text = tween((id.clone(), "text"), if flags.hovered { p.ink } else { p.ink_2 }, Tween::FAST, window, cx);
        let mut row = h_flex()
            .id(id)
            .w_full()
            .h(cx.aui().metrics.row)
            .flex_none()
            .gap(px(NAV_GAP))
            .px(px(NAV_PAD))
            .rounded(px(scale::R_SM))
            .bg(bg)
            .text_color(text)
            .ui(scale::FS_13)
            .medium()
            .cursor_pointer()
            .track_interaction(&state)
            .child(icon(self.icon).color(p.ink_3))
            .child(div().min_w(px(0.0)).truncate().child(self.label));
        if let Some(count) = self.count {
            row = row.child(div().flex_1()).child(
                div()
                    .text_role(TextRole::MonoSmall)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if self.count_warning { p.warning } else { p.ink_3 })
                    .child(count),
            );
        }
        if let Some(on_click) = self.on_click {
            row = row.on_click(move |e, w, cx| on_click(e, w, cx));
        }
        row
    }
}

/// The caps group row (`Workspaces`, `Projects`, `Recent`) with the view
/// options (sliders) and `+` actions. Build with [`group_row`].
#[derive(IntoElement)]
pub struct GroupRow {
    id: ElementId,
    label: SharedString,
    margin_top: f32,
    on_view_options: Option<ClickHandler>,
    on_add: Option<ClickHandler>,
}

/// A caps group row.
pub fn group_row(id: impl Into<ElementId>, label: impl Into<SharedString>) -> GroupRow {
    GroupRow { id: id.into(), label: label.into(), margin_top: scale::SP_3, on_view_options: None, on_add: None }
}

impl GroupRow {
    /// Overrides the 8 px top margin (card 21 has none).
    pub fn margin_top(mut self, margin: f32) -> Self {
        self.margin_top = margin;
        self
    }

    /// Shows the sliders icon and handles its click.
    pub fn on_view_options(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_view_options = Some(Box::new(f));
        self
    }

    /// Shows the `+` and handles its click.
    pub fn on_add(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_add = Some(Box::new(f));
        self
    }
}

impl RenderOnce for GroupRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut row = h_flex()
            .id(id.clone())
            .w_full()
            .h(px(GROUP_H))
            .flex_none()
            .mt(px(self.margin_top))
            .gap(px(GROUP_GAP))
            .px(px(GROUP_PAD))
            .child(div().text_role(TextRole::Caps).text_color(p.ink_3).child(self.label.to_uppercase()))
            .child(div().flex_1());
        if let Some(h) = self.on_view_options {
            row = row.child(
                icon_button((id.clone(), "view-options"), IconName::Sliders)
                    .ghost()
                    .size(ButtonSize::Xs)
                    .icon_size(px(XS_GLYPH))
                    .on_click(move |e, w, cx| h(e, w, cx)),
            );
        }
        if let Some(h) = self.on_add {
            row = row.child(
                icon_button((id, "add"), IconName::Plus)
                    .ghost()
                    .size(ButtonSize::Xs)
                    .icon_size(px(XS_GLYPH))
                    .on_click(move |e, w, cx| h(e, w, cx)),
            );
        }
        row
    }
}

/// A collapsible group header: chevron, 12 px / 600 label, mono count,
/// optional trailing element. Build with [`group_header`].
#[derive(IntoElement)]
pub struct GroupHeader {
    id: ElementId,
    label: SharedString,
    count: Option<SharedString>,
    open: bool,
    margin_top: f32,
    trailing: Option<AnyElement>,
    on_toggle: Option<ClickHandler>,
}

/// A group header (`Pinned 3`, `In progress 17`).
pub fn group_header(id: impl Into<ElementId>, label: impl Into<SharedString>, open: bool) -> GroupHeader {
    GroupHeader { id: id.into(), label: label.into(), count: None, open, margin_top: scale::SP_3, trailing: None, on_toggle: None }
}

impl GroupHeader {
    /// The mono count after the label.
    pub fn count(mut self, count: impl Into<SharedString>) -> Self {
        self.count = Some(count.into());
        self
    }

    /// Overrides the 8 px top margin.
    pub fn margin_top(mut self, margin: f32) -> Self {
        self.margin_top = margin;
        self
    }

    /// Something at the far right (the `main` branch tag).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    /// Toggle click.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for GroupHeader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut row = h_flex()
            .id(id.clone())
            .w_full()
            .h(px(GROUP_H))
            .flex_none()
            .mt(px(self.margin_top))
            .gap(px(GROUP_GAP))
            .px(px(GROUP_PAD))
            .cursor_pointer()
            .child(chevron((id, "chevron"), self.open, p.ink_3, window, cx))
            .child(div().ui(scale::FS_12).semibold().text_color(p.ink).whitespace_nowrap().child(self.label));
        if let Some(count) = self.count {
            row = row.child(tag(count));
        }
        if let Some(trailing) = self.trailing {
            row = row.child(div().flex_1()).child(trailing);
        }
        if let Some(on_toggle) = self.on_toggle {
            row = row.on_click(move |e, w, cx| on_toggle(e, w, cx));
        }
        row
    }
}

/// The account footer: avatar, name, provider usage meter, chevron. Build
/// with [`sidebar_footer`].
#[derive(IntoElement)]
pub struct SidebarFooter {
    id: ElementId,
    initial: SharedString,
    name: SharedString,
    detail: Option<SharedString>,
    plan: Option<(SharedString, bool)>,
    plan_trailing: Option<AnyElement>,
    meter: Option<(Provider, f32)>,
    trailing: Option<AnyElement>,
    pad_y: f32,
    on_click: Option<ClickHandler>,
}

/// A footer for `name` with `initial` in the avatar.
pub fn sidebar_footer(id: impl Into<ElementId>, initial: impl Into<SharedString>, name: impl Into<SharedString>) -> SidebarFooter {
    SidebarFooter { id: id.into(), initial: initial.into(), name: name.into(), detail: None, plan: None, plan_trailing: None, meter: None, trailing: None, pad_y: 10.0, on_click: None }
}

impl SidebarFooter {
    /// Shows the provider usage meter (`fraction` in 0..=1) and the chevron.
    pub fn meter(mut self, provider: Provider, fraction: f32) -> Self {
        self.meter = Some((provider, fraction));
        self
    }

    /// A second, quieter line under the name: the account's email, the
    /// identity a "signed in as" footer is really reporting.
    ///
    /// Off by default, because the shell's own footer is one line of name plus
    /// a usage meter and the design card is that footer.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A third line under the identity: what the account is entitled to, e.g.
    /// `"High Usage \u{b7} 2% this week"`.
    ///
    /// `warning` tints the row — an entitlement the person should look at
    /// ("Pay-as-you-go", "Plan unknown") is the only thing in a footer that
    /// earns colour, and a plan that is simply in force does not.
    ///
    /// Off by default, and independent of [`SidebarFooter::detail`]: either
    /// line alone stacks under the name.
    pub fn plan(mut self, plan: impl Into<SharedString>, warning: bool) -> Self {
        self.plan = Some((plan.into(), warning));
        self
    }

    /// One quiet control at the right of the plan row.
    ///
    /// The identity row's width is already spoken for — the name, the account
    /// and whatever `trailing` holds — so a second control goes on the second
    /// line, where there is room for it and where it is next to the thing it
    /// is about.
    pub fn plan_trailing(mut self, el: impl IntoElement) -> Self {
        self.plan_trailing = Some(el.into_any_element());
        self
    }

    /// Replaces the meter + chevron with another element (the assistant's pill).
    pub fn trailing(mut self, el: impl IntoElement) -> Self {
        self.trailing = Some(el.into_any_element());
        self
    }

    /// Vertical padding: 10 in the shell, 8 in the sidebar cards.
    pub fn pad_y(mut self, pad: f32) -> Self {
        self.pad_y = pad;
        self
    }

    /// Click on the footer (account menu).
    pub fn on_click(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for SidebarFooter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let plan = self.plan;
        let plan_trailing = self.plan_trailing;
        let pad_y = self.pad_y;
        let mut row = h_flex()
            .id(id.clone())
            .w_full()
            .flex_none()
            .border_t_1()
            .border_color(p.line)
            .py(px(self.pad_y))
            .px(px(FOOTER_PAD_X))
            .gap(px(FOOTER_GAP))
            .ui(scale::FS_12)
            .text_color(p.ink_3)
            .child(avatar(self.initial))
            .child({
                // One line for the name; a second for the identity and a third
                // for the entitlement when the caller gave them. A bare name
                // keeps the quieter ink it always had.
                let stacked = self.detail.is_some();
                let mut stack = v_flex().flex_1().min_w(px(0.0)).child(
                    div()
                        .w_full()
                        .truncate()
                        .when(stacked, |d| d.text_color(p.ink_2))
                        .child(self.name),
                );
                if let Some(detail) = self.detail {
                    stack = stack.child(div().w_full().truncate().ui(scale::FS_11).text_color(p.ink_4).child(detail));
                }
                stack
            });
        if let Some((provider, fraction)) = self.meter {
            row = row.child(usage_meter(provider, fraction)).child(
                icon_button((id, "account"), IconName::ChevronDown).ghost().size(ButtonSize::Xs).icon_size(px(XS_GLYPH)),
            );
        }
        if let Some(trailing) = self.trailing {
            row = row.child(trailing);
        }
        if let Some(on_click) = self.on_click {
            row = row.on_click(move |e, w, cx| on_click(e, w, cx));
        }
        let Some((plan, warning)) = plan else {
            return row.into_any_element();
        };
        // The entitlement gets the footer's whole width, under the identity row
        // rather than inside its text column: the trailing control takes the
        // width the identity has to share, and a plan truncated where its
        // number lives says nothing at all.
        let ink = if warning { p.warning } else { p.ink_4 };
        v_flex()
            .w_full()
            .flex_none()
            .border_t_1()
            .border_color(p.line)
            .child(row.border_t_0().pb(px(0.0)))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(FOOTER_GAP))
                    .px(px(FOOTER_PAD_X))
                    .pb(px(pad_y))
                    .child(div().flex_1().min_w(px(0.0)).truncate().ui(scale::FS_11).text_color(ink).child(plan))
                    .children(plan_trailing.map(|el| div().flex_none().child(el))),
            )
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------
// The sidebar's search field
// ---------------------------------------------------------------------------

/// `.sfld{height:28px;padding:0 8px;gap:6px;margin:6px 8px}`, the search box at
/// the top of a sessions list.
const SEARCH_H: f32 = 28.0;
const SEARCH_PAD_X: f32 = 8.0;
const SEARCH_GAP: f32 = 6.0;
const SEARCH_MARGIN_X: f32 = 8.0;
const SEARCH_MARGIN_Y: f32 = 6.0;
/// The magnifier and the clear glyph.
const SEARCH_GLYPH: f32 = 12.0;

/// The sidebar's search row: a magnifier, a field the caller owns, and a clear
/// button that only exists while there is something to clear. Build with
/// [`sidebar_search`].
///
/// The field is a **slot**, the same pattern the composer's editor uses: text,
/// focus and what Escape means all belong to whoever passed the element in.
/// The library owns the frame and nothing else.
#[derive(IntoElement)]
pub struct SidebarSearch {
    id: ElementId,
    field: AnyElement,
    clearable: bool,
    on_clear: Option<ClickHandler>,
}

/// A search row wrapping `field`.
pub fn sidebar_search(id: impl Into<ElementId>, field: impl IntoElement) -> SidebarSearch {
    SidebarSearch { id: id.into(), field: field.into_any_element(), clearable: false, on_clear: None }
}

impl SidebarSearch {
    /// Whether the clear button is drawn: there is text to clear.
    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    /// The clear button was pressed.
    pub fn on_clear(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(Box::new(f));
        self
    }
}

impl RenderOnce for SidebarSearch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let mut row = h_flex()
            .id(id.clone())
            .w_full()
            .flex_none()
            .h(px(SEARCH_H))
            .items_center()
            .gap(px(SEARCH_GAP))
            .px(px(SEARCH_PAD_X))
            .mx(px(SEARCH_MARGIN_X))
            .my(px(SEARCH_MARGIN_Y))
            .rounded(px(scale::R_MD))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_2)
            .ui(scale::FS_12)
            .text_color(p.ink_2)
            .child(div().flex_none().child(icon(IconName::Search).size(px(SEARCH_GLYPH)).color(p.ink_4)))
            .child(div().flex_1().min_w(px(0.0)).overflow_hidden().child(self.field));
        if self.clearable {
            let mut clear =
                icon_button((id, "clear"), IconName::X).ghost().size(ButtonSize::Xs).icon_size(px(SEARCH_GLYPH));
            if let Some(on_clear) = self.on_clear {
                clear = clear.on_click(move |e, w, cx| on_clear(e, w, cx));
            }
            row = row.child(div().flex_none().child(clear));
        }
        row
    }
}
