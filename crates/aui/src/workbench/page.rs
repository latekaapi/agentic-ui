//! D6's Page screen (docs/43 §2.7; canvas v49 `D6Page`, `D6StatesPage`):
//! the session's one page. Under the pane row a 36 px nav row (back, forward,
//! reload; the address on the Well; annotate, screenshot to the composer,
//! open in Browse), the page below. While the agent acts, one pill floats 16
//! above the page's foot; annotating outlines elements, pins numbered notes
//! and collects them in a panel 8 in from the bottom. The pill, the pins and
//! the panel stay dark in both themes. Over a native page every one of them
//! is an overlay window in the host; these draw the content.

use std::rc::Rc;

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_tokens::{dark, scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, relative, AnyElement, App, ClickEvent, ElementId, FontWeight, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, spinner};

/// The nav row.
pub const PAGE_NAV_H: f32 = 36.0;
/// Under this pane width back / forward leave the nav row.
pub const PAGE_NAV_NARROW: f32 = 300.0;
/// Muse blue: the annotation outline.
const ANNOTATE_BLUE: u32 = 0x3B6DF2;

/// What a nav-row control asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageNavAction {
    /// Back one page.
    Back,
    /// Forward one page.
    Forward,
    /// Reload.
    Reload,
    /// Edit the address.
    FocusAddress,
    /// Annotate on / off.
    Annotate,
    /// A screenshot of the page into the composer.
    Screenshot,
    /// Open the page in Browse.
    OpenInBrowse,
}

type NavHandler = Rc<dyn Fn(PageNavAction, &mut Window, &mut App) + 'static>;

/// `https://localhost:3000/pricing?x=1` → (`localhost:3000`, `/pricing?x=1`).
pub fn split_url(url: &str) -> (SharedString, SharedString) {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    match rest.find('/') {
        Some(i) => (rest[..i].to_string().into(), rest[i..].to_string().into()),
        None => (rest.to_string().into(), SharedString::default()),
    }
}

/// The address at rest (docs/43 §2.7): the Well, 28, r 6, the edge, padding
/// 0 8, gap 6 — lock 12 muted, the host 13.2 foreground, the path muted
/// (ellipsis). A click starts editing.
pub fn page_address(id: impl Into<ElementId>, url: &str, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let (host, path) = split_url(url);
    address_frame(id, cx)
        .role(gpui::Role::Button)
        .aria_label(format!("Address, {url}"))
        .cursor_pointer()
        .child(icon(IconName::Lock).size(px(12.0)).color(p.fg_muted))
        .child(div().flex_none().text_color(p.fg).child(host))
        .child(div().min_w(px(0.0)).overflow_hidden().text_ellipsis().text_color(p.fg_muted).child(path))
}

/// The address's frame, for the host's editing input to sit in.
pub fn address_frame(id: impl Into<ElementId>, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    h_flex()
        .id(id.into())
        .flex_1()
        .min_w(px(0.0))
        .h(px(28.0))
        .px(px(8.0))
        .gap(px(6.0))
        .items_center()
        .rounded(px(6.0))
        .bg(p.well)
        .shadow(p.edge())
        .whitespace_nowrap()
        .font_family(scale::FONT_UI)
        .text_px(13.2)
}

/// A nav button that cannot act: the glyph faint at 60 %.
fn dead_button(id: ElementId, glyph: IconName, label: &'static str, cx: &App) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    crate::data::record_ax_label(label);
    div()
        .id(id)
        .size(px(28.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .opacity(0.6)
        .role(gpui::Role::Button)
        .aria_label(label)
        .child(icon(glyph).size(px(14.0)).color(p.ink_4))
}

/// The 36 px nav row (padding 0 6, gap 2). `address` is the host's slot —
/// [`page_address`] at rest, the editing input in an [`address_frame`].
#[allow(clippy::too_many_arguments)]
pub fn page_nav_row(
    id: impl Into<ElementId>,
    address: AnyElement,
    can_back: bool,
    can_forward: bool,
    annotating: bool,
    width: f32,
    loading: Option<f32>,
    on_action: impl Fn(PageNavAction, &mut Window, &mut App) + 'static,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let on: NavHandler = Rc::new(on_action);
    let button = |key: &'static str, glyph: IconName, label: &'static str, action: PageNavAction| {
        let on = on.clone();
        icon_button((id.clone(), key), glyph).ghost().accessibility_label(label).on_click(move |_, w, cx| on(action, w, cx))
    };
    let narrow = width > 0.0 && width < PAGE_NAV_NARROW;
    let mut row = h_flex().id(id.clone()).relative().w_full().h(px(PAGE_NAV_H)).flex_none().px(px(6.0)).gap(px(2.0)).items_center().bg(p.surface_2);
    if !narrow {
        row = row
            .child(if can_back { button("back", IconName::ArrowLeft, "Back", PageNavAction::Back).into_any_element() } else { dead_button((id.clone(), "back").into(), IconName::ArrowLeft, "Back", cx).into_any_element() })
            .child(if can_forward {
                button("forward", IconName::ArrowRight, "Forward", PageNavAction::Forward).into_any_element()
            } else {
                dead_button((id.clone(), "forward").into(), IconName::ArrowRight, "Forward", cx).into_any_element()
            });
    }
    row = row
        .child(button("reload", IconName::Refresh, "Reload", PageNavAction::Reload))
        .child(address)
        .child({
            let on = on.clone();
            icon_button((id.clone(), "annotate"), IconName::MapPin).ghost().toggled(annotating).accessibility_label("Annotate").on_click(move |_, w, cx| on(PageNavAction::Annotate, w, cx))
        })
        .child(button("shot", IconName::Camera, "Screenshot to the composer", PageNavAction::Screenshot))
        .child(button("browse", IconName::External, "Open in Browse", PageNavAction::OpenInBrowse));
    if let Some(progress) = loading {
        // A 2 px muted bar grows under the address while the page loads.
        row = row.child(div().absolute().left(px(0.0)).bottom(px(0.0)).h(px(2.0)).w(relative(progress.clamp(0.0, 1.0))).bg(p.fg_muted));
    }
    row
}

// ─────────────────────────────────────────────────────────────── over the page

/// The agent pill (docs/43 §2.7): Floating dark (#2C2C32, the edge + float
/// drop, full radius), 32 tall, padding 0 14 / 0 10, gap 8 — the provider
/// mark 16, the action 13.2, a 12 px spinner. Dark in both themes; centre it
/// 16 above the page's foot.
pub fn agent_pill(id: impl Into<ElementId>, provider: Provider, text: impl Into<SharedString>, cx: &App) -> gpui::Stateful<gpui::Div> {
    let d = dark();
    let id: ElementId = id.into();
    let text: SharedString = text.into();
    let ring = (d.fg_muted.alpha(0.35), d.fg_muted);
    h_flex()
        .id(id.clone())
        .role(gpui::Role::Status)
        .aria_label(text.clone())
        .h(px(32.0))
        .flex_none()
        .pl(px(10.0))
        .pr(px(14.0))
        .gap(px(8.0))
        .items_center()
        .rounded_full()
        .bg(d.overlay)
        .shadow(d.floating_shadow())
        .font_family(scale::FONT_UI)
        .text_px(13.2)
        .text_color(d.fg)
        .whitespace_nowrap()
        .child(provider_mark(provider).size(px(16.0)))
        .child(text)
        .child(dark_spinner((id, "spin"), ring, cx))
}

fn dark_spinner(id: impl Into<ElementId>, (track, arc): (Hsla, Hsla), cx: &App) -> impl IntoElement {
    let _ = cx;
    spinner(id).size(px(12.0)).colors(track, arc)
}

/// The hovered element while annotating: 1.5 px Muse blue, r 4, a 6 % fill.
pub fn annotate_outline(width: f32, height: f32) -> gpui::Div {
    let blue = Hsla::from(gpui::rgb(ANNOTATE_BLUE));
    div().w(px(width)).h(px(height)).rounded(px(4.0)).border(px(1.5)).border_color(blue).bg(blue.alpha(0.06))
}

/// A pinned note's marker: a 20 px dark circle, 11 / 600 white, a 2 px white
/// ring.
pub fn annotate_pin(index: usize) -> gpui::Div {
    div()
        .size(px(24.0))
        .flex_none()
        .rounded_full()
        .bg(gpui::white())
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(20.0))
                .rounded_full()
                .bg(Hsla::from(gpui::rgb(0x18181B)))
                .flex()
                .items_center()
                .justify_center()
                .font_family(scale::FONT_UI)
                .text_px(11.0)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(gpui::white())
                .child(index.to_string()),
        )
}

/// One pinned note in the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct PageNote {
    /// The pin's number.
    pub index: usize,
    /// The element's selector (`button.choose`).
    pub selector: SharedString,
    /// What the person wrote.
    pub note: SharedString,
}

type Click = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The notes panel (docs/43 §2.7): Floating r 10, padding 6, 8 in from the
/// page's bottom and sides; rows 13.2 (number cap 18 at 9 %, selector mono
/// 12.1 muted, the note); a footer over a rule: "Click an element to pin a
/// note" faint, Clear, **Send N to <agent>**. Dark in both themes.
pub fn notes_panel(id: impl Into<ElementId>, notes: &[PageNote], agent: &str, on_clear: Option<Click>, on_send: Option<Click>, cx: &App) -> gpui::Stateful<gpui::Div> {
    let d: Palette = dark();
    let _ = cx;
    let id: ElementId = id.into();
    let rows = notes.iter().map(|n| {
        h_flex()
            .gap(px(8.0))
            .px(px(6.0))
            .py(px(6.0))
            .items_start()
            .rounded(px(6.0))
            .text_px(13.2)
            .line_height(px(18.0))
            .child(
                div()
                    .size(px(18.0))
                    .flex_none()
                    .rounded_full()
                    .bg(d.selected_overlay())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_px(11.0)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(n.index.to_string()),
            )
            .child(
                v_flex()
                    .min_w(px(0.0))
                    .child(div().font_family(scale::FONT_MONO).text_px(12.1).text_color(d.fg_muted).overflow_hidden().whitespace_nowrap().text_ellipsis().child(n.selector.clone()))
                    .child(div().text_color(d.fg).child(n.note.clone())),
            )
    });
    let clear = h_flex()
        .id((id.clone(), "clear"))
        .role(gpui::Role::Button)
        .aria_label("Clear")
        .cursor_pointer()
        .h(px(28.0))
        .px(px(10.0))
        .items_center()
        .rounded(px(6.0))
        .text_color(d.fg_muted)
        .hover(move |s| s.bg(d.hover_overlay()).text_color(d.fg))
        .child("Clear")
        .when_some(on_clear, |b, f| b.on_click(move |e, w, cx| f(e, w, cx)));
    let label = format!("Send {} to {agent}", notes.len());
    let send = h_flex()
        .id((id.clone(), "send"))
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .cursor_pointer()
        .h(px(28.0))
        .px(px(12.0))
        .items_center()
        .rounded(px(6.0))
        .bg(d.accent)
        .text_color(d.bg)
        .font_weight(FontWeight::SEMIBOLD)
        .when(notes.is_empty(), |b| b.opacity(0.45))
        .child(label)
        .when_some(on_send.filter(|_| !notes.is_empty()), |b, f| b.on_click(move |e, w, cx| f(e, w, cx)));
    v_flex()
        .id(id)
        .role(gpui::Role::Dialog)
        .aria_label("Notes on the page")
        .w_full()
        .p(px(6.0))
        .rounded(px(10.0))
        .bg(d.overlay)
        .shadow(d.floating_shadow())
        .font_family(scale::FONT_UI)
        .text_px(13.2)
        .text_color(d.fg)
        .children(rows)
        .child(
            h_flex()
                .mt(px(4.0))
                .pt(px(6.0))
                .px(px(4.0))
                .pb(px(2.0))
                .gap(px(6.0))
                .items_center()
                .border_t_1()
                .border_color(d.line)
                .child(div().flex_1().min_w(px(0.0)).overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(d.ink_4).child("Click an element to pin a note"))
                .child(clear)
                .child(send),
        )
}

// ─────────────────────────────────────────────────────────────── states

/// No page yet (docs/43 §2.7): the state block over a 32 px address field
/// "Search or enter an address" (the host's input in a Well frame).
pub fn no_page_state(id: impl Into<ElementId>, field: impl IntoElement, agent: &str, cx: &App) -> gpui::Div {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(14.0))
        .px(px(24.0))
        .child(
            div().h(px(180.0)).w_full().child(
                super::pane::state_block(id, IconName::Globe, "No page yet")
                    .line(format!("When {agent} opens a page for this session, it shows here. Or type an address.")),
            ),
        )
        .child(div().w_full().max_w(px(320.0)).child(field))
        .when(cx.aui().colors.is_dark(), |d| d)
}

/// The address field of [`no_page_state`]: 32 tall on the Well with the edge.
pub fn address_field_frame(focused: bool, cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    h_flex()
        .w_full()
        .h(px(32.0))
        .px(px(10.0))
        .gap(px(8.0))
        .items_center()
        .rounded(px(6.0))
        .bg(p.well)
        .shadow(if focused { super::pane::focus_edge(cx) } else { p.edge() })
        .font_family(scale::FONT_UI)
        .text_px(14.3)
        .child(icon(IconName::Search).size(px(14.0)).color(p.fg_muted))
}

/// Couldn't load (docs/43 §2.7): Baaz's own page on the sheet — "This page
/// couldn’t load", the reason with the host in mono, a secondary Try again.
pub fn page_error_state(id: impl Into<ElementId>, host: &str, reason: &str, on_retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static, cx: &App) -> super::pane::StateBlock {
    let id: ElementId = id.into();
    let p = cx.aui().colors;
    super::pane::state_block(id.clone(), IconName::Globe, "This page couldn’t load")
        .line_with(
            div()
                .child(div().font_family(scale::FONT_MONO).text_color(p.fg).child(host.to_string()))
                .child(reason.to_string()),
        )
        .action(super::pane::secondary_action((id, "retry"), "Try again", Some(IconName::Refresh), cx).on_click(on_retry))
}

/// What a Recent pages row does when picked: it gets the row's index.
pub type RecentSelect = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// One row of the Recent pages menu.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentRow {
    /// What the row says: the page's title, or its host and path when it had none.
    pub title: SharedString,
    /// The host, muted at the right (hidden under [`PAGE_NAV_NARROW`]).
    pub host: SharedString,
    /// The page on screen: checked.
    pub current: bool,
}

impl RecentRow {
    /// A row for `url` titled `title`, current when it is the page on screen.
    pub fn new(url: &str, title: &str, current: bool) -> Self {
        let (host, path) = split_url(url);
        let title = if title.trim().is_empty() { format!("{host}{path}") } else { title.trim().to_string() };
        RecentRow { title: title.into(), host: host.to_string().into(), current }
    }
}

/// The Recent pages menu under the Page row's title (owner 2026-10-18,
/// canvas v50 `D6PageRecent`): the app's one dropdown, a "Recent pages"
/// label, then one row per address, newest first; the page on screen is
/// checked. Width = pane − 12, at most 320. `on_select` gets the row index.
pub fn recent_pages_menu(
    id: impl Into<ElementId>,
    rows: &[RecentRow],
    pane_width: f32,
    on_select: RecentSelect,
) -> crate::interact::Dropdown {
    use crate::interact::{dropdown, DropdownEntry, DropdownItem, DropdownTrailing};
    let narrow = pane_width < PAGE_NAV_NARROW;
    let mut menu = dropdown(id, "Recent pages").width(px(recent_menu_width(pane_width))).entry(DropdownEntry::Label("Recent pages".into()));
    for (i, row) in rows.iter().enumerate() {
        let pick = on_select.clone();
        let mut item = DropdownItem::new(row.title.clone()).checked(row.current).on_select(move |w, cx| pick(i, w, cx));
        if !narrow {
            item = item.trailing(DropdownTrailing::Value(row.host.clone()));
        }
        menu = menu.entry(item);
    }
    menu
}

/// The Recent pages menu's width for a pane `pane_width` wide.
pub fn recent_menu_width(pane_width: f32) -> f32 {
    (pane_width - 12.0).min(320.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_split_into_host_and_path() {
        assert_eq!(split_url("http://localhost:3000/pricing"), ("localhost:3000".into(), "/pricing".into()));
        assert_eq!(split_url("https://example.com"), ("example.com".into(), "".into()));
        assert_eq!(split_url("example.com/a?b=1"), ("example.com".into(), "/a?b=1".into()));
    }
}
