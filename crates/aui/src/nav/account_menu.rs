//! The sidebar-footer account menu: one sectioned panel with a header, a
//! usage section with a compact row per provider, then the menu rows.
//!
//! Hosts used to stack a bare "Usage" label, one [`usage_card`](crate::screens::usage_card)
//! per provider and a separate [`view_menu`](super::view_menu) in a column;
//! the column ran wider than the sidebar and the anchored popover flipped
//! over the transcript. This panel draws all three blocks inside the same
//! chrome as `view_menu` at one width (260 px by default).
//!
//! Every interactive item carries `role=MenuItem` with its label as the
//! accessible name; the header and the usage rows are not interactive.

use std::rc::Rc;

use aui_icons::provider_mark;
use aui_motion::{presence, EnterExit, PresenceStyle};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use super::view_menu::{menu_row, menu_surface};
use crate::composer::provider_display_name;
use crate::data::avatar;
use crate::screens::{usage_warns, UsageRowData, UsageRowState};

/// The account menu's default width: room for a usage bar beside its percent.
const MENU_W: f32 = 260.0;
/// The menu's inner side padding, matching [`view_menu`](super::view_menu):
/// the section label sits at this measure, never flush at the panel edge.
const MENU_PAD_X: f32 = 8.0;
/// Compact usage rows sit 8 px apart.
const USAGE_ROW_GAP: f32 = 8.0;
/// The gap inside one usage row: headline to windows, window to window.
const USAGE_INNER_GAP: f32 = 4.0;
/// The header avatar's diameter.
const AVATAR: f32 = 24.0;
/// The provider mark on a usage headline.
const MARK: f32 = 14.0;
/// The provider name on a usage headline.
const NAME_TEXT: f32 = scale::FS_13;
/// Window labels, percents, reasons and the header detail line.
const SMALL_TEXT: f32 = scale::FS_11;
/// The thin usage bar filling the middle of a window line.
const BAR_H: f32 = 4.0;
const BAR_R: f32 = 2.0;
/// The bar's minimum measure so the fill reads at narrow widths.
const BAR_MIN_W: f32 = 24.0;
/// The menu enters with a fade and a 6 px rise, like `view_menu`.
const MENU_RISE: f32 = 6.0;

type ActivateHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// One row of the account menu's item block: an interactive item or a
/// hairline between two blocks of items.
#[derive(Debug, Clone, PartialEq)]
pub enum AccountMenuItem {
    /// An interactive row: the label, an optional muted detail at the right,
    /// and whether the label draws in danger ink.
    Item {
        /// The row label, also its accessible name.
        label: SharedString,
        /// A muted suffix at the row's right, if any.
        detail: Option<SharedString>,
        /// Draws the label in danger ink (sign-out).
        destructive: bool,
    },
    /// A hairline between two blocks of items.
    Separator,
}

impl AccountMenuItem {
    /// An interactive row with `label` and no detail, not destructive.
    pub fn new(label: impl Into<SharedString>) -> Self {
        AccountMenuItem::Item { label: label.into(), detail: None, destructive: false }
    }

    /// A muted suffix at the row's right; ignored by separators.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        if let AccountMenuItem::Item { detail: slot, .. } = &mut self {
            *slot = Some(detail.into());
        }
        self
    }

    /// Draws the label in danger ink; ignored by separators.
    pub fn destructive(mut self) -> Self {
        if let AccountMenuItem::Item { destructive, .. } = &mut self {
            *destructive = true;
        }
        self
    }

    /// Whether this row is a hairline rather than an interactive item.
    pub fn is_separator(&self) -> bool {
        matches!(self, AccountMenuItem::Separator)
    }

    /// The interactive label, or `None` for a separator.
    pub fn label(&self) -> Option<&SharedString> {
        match self {
            AccountMenuItem::Item { label, .. } => Some(label),
            AccountMenuItem::Separator => None,
        }
    }
}

/// The account menu: header, usage section, then menu rows in one panel.
/// Build with [`account_menu`].
#[derive(IntoElement)]
pub struct AccountMenu {
    id: ElementId,
    header: Option<(SharedString, Option<SharedString>)>,
    usage: Option<(SharedString, Vec<UsageRowData>)>,
    items: Vec<AccountMenuItem>,
    width: f32,
    present: bool,
    timing: EnterExit,
    on_activate: Option<ActivateHandler>,
}

/// An account menu over the blocks its builders add: header, usage, items.
pub fn account_menu(id: impl Into<ElementId>) -> AccountMenu {
    AccountMenu {
        id: id.into(),
        header: None,
        usage: None,
        items: Vec::new(),
        width: MENU_W,
        present: true,
        timing: EnterExit::DEFAULT,
        on_activate: None,
    }
}

impl AccountMenu {
    /// The non-interactive top block: a 24 px avatar with the name's initial,
    /// the name (email) in ink and an optional detail line in ink-3.
    pub fn header(mut self, name: impl Into<SharedString>, detail: Option<SharedString>) -> Self {
        self.header = Some((name.into(), detail));
        self
    }

    /// The usage section: a caps `title` and one compact row per entry, with
    /// no per-row card borders.
    pub fn usage(mut self, title: impl Into<SharedString>, rows: Vec<UsageRowData>) -> Self {
        self.usage = Some((title.into(), rows));
        self
    }

    /// The menu rows under the usage section, in order.
    pub fn rows(mut self, rows: Vec<AccountMenuItem>) -> Self {
        self.items = rows;
        self
    }

    /// Overrides the 260 px default width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Whether the menu is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the menu is drawn at rest on its first frame. For a
    /// menu that is part of a static composition (the design card, a restored
    /// panel) rather than one the person just opened.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// An item was clicked; the argument is the item index with separators
    /// skipped (see [`AccountMenu::item_index_at_row`]).
    pub fn on_activate(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }

    /// Maps a position in the `rows` block (separators included) to the
    /// activation index [`on_activate`](AccountMenu::on_activate) reports:
    /// separators map to `None`, items to their order among items.
    pub fn item_index_at_row(&self, row: usize) -> Option<usize> {
        let item = self.items.get(row)?;
        if item.is_separator() {
            return None;
        }
        Some(self.items.iter().take(row).filter(|item| !item.is_separator()).count())
    }

    /// The number of interactive items (separators excluded).
    pub fn item_count(&self) -> usize {
        self.items.iter().filter(|item| !item.is_separator()).count()
    }
}

/// A hairline spanning the panel between two blocks.
fn hairline(p: &aui_tokens::Palette) -> gpui::Div {
    div().flex_none().h(px(1.0)).my(px(6.0)).mx(px(4.0)).bg(p.line)
}

/// The non-interactive header block: avatar, name, optional detail.
fn header_block(name: SharedString, detail: Option<SharedString>, p: &aui_tokens::Palette) -> gpui::Div {
    let initial: SharedString = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default().into();
    let mut text = v_flex().min_w(px(0.0)).flex_1().child(
        div()
            .truncate()
            .text_color(p.ink)
            .medium()
            .child(name),
    );
    if let Some(detail) = detail {
        text = text.child(
            div()
                .truncate()
                .text_px(SMALL_TEXT)
                .line_height(gpui::relative(scale::LH_UI))
                .text_color(p.ink_3)
                .child(detail),
        );
    }
    h_flex().w_full().items_center().gap(px(8.0)).px(px(MENU_PAD_X)).py(px(2.0)).child(
        avatar(initial).size(px(AVATAR)),
    ).child(text)
}

/// One compact usage row: provider headline, then one line per window or the
/// unavailable reason. Never interactive.
fn usage_row(row: &UsageRowData, p: &aui_tokens::Palette) -> gpui::Div {
    let name = provider_display_name(row.provider);
    let mut head = h_flex()
        .w_full()
        .items_center()
        .gap(px(6.0))
        .child(provider_mark(row.provider).size(px(MARK)))
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_px(NAME_TEXT)
                .line_height(gpui::relative(scale::LH_UI))
                .text_color(p.ink)
                .medium()
                .child(name),
        );
    if let Some(plan) = row.plan.clone() {
        head = head.child(
            div()
                .flex_none()
                .text_px(SMALL_TEXT)
                .line_height(gpui::relative(scale::LH_UI))
                .text_color(p.ink_3)
                .child(plan),
        );
    }

    let mut body = v_flex().w_full().gap(px(USAGE_INNER_GAP)).child(head);
    match &row.state {
        UsageRowState::Windows(windows, as_of) => {
            for window in windows.iter() {
                let used = window.used_fraction.clamp(0.0, 1.0);
                let fill = if usage_warns(window.used_fraction) { p.warning } else { p.ink_3 };
                // The resets note shares the window line when it fits and
                // wraps under it when it does not — never clipped.
                let line = h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex_none()
                            .text_px(SMALL_TEXT)
                            .line_height(gpui::relative(scale::LH_UI))
                            .text_color(p.ink_2)
                            .child(window.label.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(BAR_MIN_W))
                            .h(px(BAR_H))
                            .rounded(px(BAR_R))
                            .bg(p.line)
                            .overflow_hidden()
                            .child(div().h_full().rounded(px(BAR_R)).bg(fill).w(gpui::relative(used))),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(scale::FONT_MONO)
                            .text_px(SMALL_TEXT)
                            .line_height(gpui::relative(scale::LH_UI))
                            .text_color(fill)
                            .child(format!("{}%", (used * 100.0).round())),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_px(SMALL_TEXT)
                            .line_height(gpui::relative(scale::LH_UI))
                            .text_color(p.ink_3)
                            .child(format!("resets in {}", window.resets_at_text)),
                    );
                body = body.child(line);
            }
            if let Some(as_of) = as_of {
                body = body.child(
                    div()
                        .text_px(SMALL_TEXT)
                        .line_height(gpui::relative(scale::LH_UI))
                        .text_color(p.ink_3)
                        .child(format!("as of {as_of}")),
                );
            }
        }
        UsageRowState::Unavailable(reason) => {
            body = body.child(
                div()
                    .w_full()
                    .text_px(SMALL_TEXT)
                    .line_height(gpui::relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(reason.clone()),
            );
        }
    }
    body
}

impl RenderOnce for AccountMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let sample = presence((id.clone(), "presence"), self.present, self.timing, window, cx);
        let style = PresenceStyle::fade_rise(sample, MENU_RISE);
        let row_h = cx.aui().metrics.row;

        let mut menu = menu_surface(&p, self.width).relative().top(style.offset_y).opacity(style.opacity);
        let has_header = self.header.is_some();
        let has_usage = self.usage.is_some();
        if let Some((name, detail)) = self.header {
            menu = menu.child(header_block(name, detail, &p));
        }
        if let Some((title, rows)) = self.usage {
            if has_header {
                menu = menu.child(hairline(&p));
            }
            let mut section = v_flex().w_full().gap(px(USAGE_ROW_GAP)).child(
                div().px(px(MENU_PAD_X)).text_role(TextRole::Caps).text_color(p.ink_3).child(title),
            );
            for row in rows.iter() {
                section = section.child(usage_row(row, &p));
            }
            menu = menu.child(section);
        }
        if !self.items.is_empty() && (has_header || has_usage) {
            menu = menu.child(hairline(&p));
        }
        // The activation index counts items only: separators hold a row
        // position but never report. Count eagerly so each closure captures
        // the same index [`AccountMenu::item_index_at_row`] maps to.
        let mut item_index = 0usize;
        for (row_pos, item) in self.items.into_iter().enumerate() {
            match item {
                AccountMenuItem::Separator => menu = menu.child(hairline(&p)),
                AccountMenuItem::Item { label, detail, destructive } => {
                    let activation = item_index;
                    item_index += 1;
                    let key: ElementId = (id.clone(), SharedString::from(format!("row-{row_pos}"))).into();
                    let ink = if destructive { p.danger } else { p.ink };
                    let mut el = menu_row(key, row_h, gpui::transparent_black(), window, cx)
                        .role(gpui::Role::MenuItem)
                        .aria_label(label.clone())
                        .text_color(ink)
                        .child(div().min_w(px(0.0)).truncate().child(label.clone()));
                    if let Some(detail) = detail {
                        el = el.child(div().flex_1()).child(
                            div()
                                .flex_none()
                                .text_color(p.ink_3)
                                .child(detail),
                        );
                    }
                    if let Some(handler) = self.on_activate.clone() {
                        el = el.on_click(move |_, w, cx| {
                            handler(activation, w, cx);
                        });
                    }
                    menu = menu.child(el);
                }
            }
        }
        menu
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aui_icons::Provider;

    fn menu() -> AccountMenu {
        account_menu("menu").rows(vec![
            AccountMenuItem::new("Settings…"),
            AccountMenuItem::Separator,
            AccountMenuItem::new("Providers…"),
            AccountMenuItem::new("Sign out").destructive(),
        ])
    }

    #[test]
    fn rows_keep_order_and_width_overrides() {
        let menu = menu().width(220.0);
        let labels: Vec<Option<SharedString>> =
            menu.items.iter().map(AccountMenuItem::label).map(|label| label.cloned()).collect();
        assert_eq!(labels, vec![Some("Settings…".into()), None, Some("Providers…".into()), Some("Sign out".into())]);
        assert!((menu.width - 220.0).abs() < f32::EPSILON);
        assert!((account_menu("bare").width - MENU_W).abs() < f32::EPSILON);
    }

    #[test]
    fn activation_index_skips_separators() {
        let menu = menu();
        assert_eq!(menu.item_index_at_row(0), Some(0));
        assert_eq!(menu.item_index_at_row(1), None);
        assert_eq!(menu.item_index_at_row(2), Some(1));
        assert_eq!(menu.item_index_at_row(3), Some(2));
        assert_eq!(menu.item_index_at_row(4), None);
        assert_eq!(menu.item_count(), 3);
    }

    #[test]
    fn builders_keep_header_and_usage() {
        let menu = account_menu("menu")
            .header("latekaapi@gmail.com", None)
            .usage("Usage", vec![UsageRowData::new(Provider::Codex, UsageRowState::Unavailable("No reading yet".into()))]);
        assert_eq!(menu.header, Some(("latekaapi@gmail.com".into(), None)));
        let (_, rows) = menu.usage.as_ref().expect("usage section");
        assert_eq!(rows.len(), 1);
    }
}
