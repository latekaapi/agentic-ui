//! The import preview dialog and the two-line add-skill menu row.

use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_motion::{tint_fade, EnterExit, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::pill;
use crate::data::PillVariant;
use crate::overlay::card::{modal_card, modal_presence, modal_scrim};
use crate::util::{interaction_flags, TrackInteraction};

use super::controls::{checkbox, CheckboxIntent, CheckboxState};
use super::row::SkillRowIntent;

/// Whether an import candidate is new, replaces a personal skill, or is
/// already installed (and therefore unchecked and dead).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportStatus {
    /// A skill Personal lacks: checkbox live, `New` chip.
    #[default]
    New,
    /// A skill Personal has under another source: `Replaces yours` chip.
    Replaces,
    /// Byte-identical with Personal: dimmed, checkbox disabled, no chip.
    Installed,
}

impl ImportStatus {
    /// The status chip label, if the row carries one.
    pub fn chip_label(&self) -> Option<&'static str> {
        match self {
            ImportStatus::New => Some("New"),
            ImportStatus::Replaces => Some("Replaces yours"),
            ImportStatus::Installed => None,
        }
    }

    /// Whether the row's checkbox is interactive.
    pub fn selectable(&self) -> bool {
        !matches!(self, ImportStatus::Installed)
    }
}

/// One import candidate: a checkbox, the name over its description, the
/// status chip and the mono tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRow {
    /// Stable identity, handed back by
    /// [`ImportPreviewIntent::ToggleRow`].
    pub id: SharedString,
    /// The skill name.
    pub name: SharedString,
    /// One ellipsised line under the name (or the already-installed note).
    pub description: SharedString,
    /// New / replaces / installed.
    pub status: ImportStatus,
    /// Startup tokens as drawn (`640`, `1.1k`).
    pub tokens: SharedString,
    /// Startup tokens as a number, for the summary math.
    pub weight: u32,
    /// Whether the candidate is picked for import.
    pub checked: bool,
}

impl ImportRow {
    /// A picked candidate.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        description: impl Into<SharedString>,
        status: ImportStatus,
        tokens: impl Into<SharedString>,
        weight: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            status,
            tokens: tokens.into(),
            weight,
            checked: true,
        }
    }

    /// Unchecks the row (installed rows stay unchecked).
    pub fn unchecked(mut self) -> Self {
        self.checked = false;
        self
    }

    /// The intent a checkbox press reports, or `None`: installed rows are
    /// already there, so their boxes are dead like dimmed skill rows.
    pub fn toggle_intent(&self) -> Option<SkillRowIntent> {
        if self.status.selectable() { Some(SkillRowIntent::Toggle) } else { None }
    }
}

/// The picked, importable rows: checked and not already installed.
pub fn selected_rows(rows: &[ImportRow]) -> Vec<&ImportRow> {
    rows.iter().filter(|r| r.checked && r.status.selectable()).collect()
}

/// What the picked rows add to every session's startup context.
pub fn added_tokens(rows: &[ImportRow]) -> u32 {
    selected_rows(rows).iter().map(|r| r.weight).sum()
}

/// Formats a token count the way rows draw it: `640`, `1.1k`, `6.9k`.
pub fn format_tokens(n: u32) -> String {
    if n < 1000 {
        n.to_string()
    } else {
        let tenths = (n as f32 / 100.0).round() / 10.0;
        if tenths >= 100.0 {
            format!("{}k", (tenths.round()) as u32)
        } else if tenths == tenths.trunc() {
            format!("{}k", tenths as u32)
        } else {
            format!("{tenths:.1}k")
        }
    }
}

/// The primary button label for `selected` picked rows.
pub fn default_primary_label(selected: usize) -> String {
    if selected == 0 {
        "Import".to_string()
    } else if selected == 1 {
        "Import 1 skill".to_string()
    } else {
        format!("Import {selected} skills")
    }
}

/// What the import preview asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportPreviewIntent {
    /// A row's checkbox was pressed; the argument is [`ImportRow::id`].
    ToggleRow(SharedString),
    /// The secondary button was pressed.
    Cancel,
    /// The primary button was pressed.
    Import,
}

type PreviewHandler = Rc<dyn Fn(ImportPreviewIntent, &mut Window, &mut App)>;
type MenuActivateHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// The import preview: title, subtitle, a checkbox row per candidate (each
/// with its status chip and tokens), the summary note and the action row —
/// hint left, spacer, Cancel, Import. Drawn through the shared modal chrome,
/// like [`crate::overlay::Dialog`]. Build with [`import_preview`].
#[derive(IntoElement)]
pub struct ImportPreview {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    rows: Vec<ImportRow>,
    summary: Option<SharedString>,
    hint: Option<SharedString>,
    cancel_label: SharedString,
    primary_label: Option<SharedString>,
    present: bool,
    timing: EnterExit,
    on_intent: Option<PreviewHandler>,
}

/// An import preview headed `title`, with an `Import` primary until one is
/// set.
pub fn import_preview(id: impl Into<ElementId>, title: impl Into<SharedString>) -> ImportPreview {
    ImportPreview {
        id: id.into(),
        title: title.into(),
        subtitle: None,
        rows: Vec::new(),
        summary: None,
        hint: None,
        cancel_label: "Cancel".into(),
        primary_label: None,
        present: true,
        timing: EnterExit::DEFAULT,
        on_intent: None,
    }
}

/// The dialog card width: room for name, chip and tokens.
const PREVIEW_W: f32 = 600.0;
/// `.ir{gap:12px;padding:10px 12px}` with the 48 px token column.
const IMPORT_GAP: f32 = 12.0;
const IMPORT_PAD_Y: f32 = 10.0;
const IMPORT_PAD_X: f32 = 12.0;
const IMPORT_TOK_W: f32 = 48.0;
/// Title 17.6 / 600, subtitle 14.3 ink-3, row name 14.3 / 500.
const TITLE_TEXT: f32 = 17.6;
const SUBTITLE_TEXT: f32 = 14.3;
const ROW_NAME: f32 = 14.3;
const ROW_DESC: f32 = 13.2;
const ROW_TOK: f32 = 12.1;
/// Installed rows sit below full ink, like dimmed skill rows.
const INSTALLED_OPACITY: f32 = 0.5;

impl ImportPreview {
    /// The subtitle under the title (what was found, and where it lands).
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A candidate row.
    pub fn row(mut self, row: ImportRow) -> Self {
        self.rows.push(row);
        self
    }

    /// The summary note above the action row (`Adds 1.8k tokens …`).
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// The action-row hint on the left (`Nothing is copied until you import`).
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Overrides the secondary label (`Cancel`).
    pub fn cancel_label(mut self, label: impl Into<SharedString>) -> Self {
        self.cancel_label = label.into();
        self
    }

    /// Overrides the primary label (by default
    /// [`default_primary_label`] for the picked count).
    pub fn primary_label(mut self, label: impl Into<SharedString>) -> Self {
        self.primary_label = Some(label.into());
        self
    }

    /// Whether the dialog is open; `false` plays the exit.
    pub fn present(mut self, present: bool) -> Self {
        self.present = present;
        self
    }

    /// Skips the enter: the dialog is drawn at rest on its first frame, for
    /// a static composition rather than one the person just opened.
    pub fn at_rest(mut self) -> Self {
        self.timing.enter = std::time::Duration::ZERO;
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(ImportPreviewIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ImportPreview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // The shared modal chrome both modals draw through: flat scrim, the
        // card shell, the fade-rise-scale presence.
        let style = modal_presence(&id, self.present, self.timing, window, cx);
        let selected = selected_rows(&self.rows).len();
        let primary: SharedString =
            self.primary_label.clone().unwrap_or_else(|| default_primary_label(selected).into());

        let mut list = v_flex().flex_none().w_full().px(px(12.0)).gap(px(2.0));
        for row_model in &self.rows {
            let installed = !row_model.status.selectable();
            let row_id: ElementId = (id.clone(), row_model.id.clone()).into();
            let state = if row_model.checked { CheckboxState::On } else { CheckboxState::Off };
            let mut box_el = checkbox((row_id.clone(), "box"), state, installed)
                .accessibility_label(SharedString::from(format!("Include {}", row_model.name)));
            if row_model.toggle_intent().is_some() {
                if let Some(handler) = self.on_intent.clone() {
                    let row_key = row_model.id.clone();
                    box_el =
                        box_el.on_intent(move |_: CheckboxIntent, w, cx| handler(ImportPreviewIntent::ToggleRow(row_key.clone()), w, cx));
                }
            }
            let mut item = h_flex()
                .id(row_id)
                .w_full()
                .items_center()
                .gap(px(IMPORT_GAP))
                .px(px(IMPORT_PAD_X))
                .py(px(IMPORT_PAD_Y))
                .rounded(px(scale::R_SM))
                .child(box_el)
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .justify_center()
                        .child(
                            div()
                                .flex_none()
                                .ui(ROW_NAME)
                                .line_height(relative(1.0))
                                .medium()
                                .text_color(p.ink)
                                .child(row_model.name.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w_full()
                                .truncate()
                                .ui(ROW_DESC)
                                .line_height(relative(scale::LH_UI))
                                .text_color(p.ink_3)
                                .child(row_model.description.clone()),
                        ),
                );
            if let Some(chip_label) = row_model.status.chip_label() {
                let variant =
                    if row_model.status == ImportStatus::Replaces { PillVariant::Warning } else { PillVariant::Line };
                item = item.child(pill(chip_label).variant(variant));
            }
            item = item.child(
                div()
                    .flex_none()
                    .w(px(IMPORT_TOK_W))
                    .flex()
                    .justify_end()
                    .mono(ROW_TOK)
                    .line_height(relative(1.0))
                    .medium()
                    .text_color(p.ink_4)
                    .child(row_model.tokens.clone()),
            );
            if installed {
                item = item.opacity(INSTALLED_OPACITY);
            }
            list = list.child(item);
        }

        let mut card = v_flex().flex_none().w(px(PREVIEW_W));
        card = card.child(
            v_flex().flex_none().w_full().px(px(24.0)).pt(px(20.0)).pb(px(12.0)).gap(px(4.0)).child(
                div()
                    .flex_none()
                    .ui(TITLE_TEXT)
                    .line_height(relative(1.0))
                    .semibold()
                    .text_color(p.ink)
                    .child(self.title.clone()),
            ),
        );
        if let Some(subtitle) = self.subtitle {
            card = card.child(
                div()
                    .flex_none()
                    .w_full()
                    .px(px(24.0))
                    .ui(SUBTITLE_TEXT)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_3)
                    .child(subtitle),
            );
        }
        card = card.child(div().flex_none().h(px(12.0))).child(list);
        if let Some(summary) = self.summary {
            card = card.child(
                div()
                    .flex_none()
                    .mx(px(24.0))
                    .mt(px(12.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .rounded(px(scale::R_SM))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.surface_1)
                    .ui(SUBTITLE_TEXT)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_2)
                    .child(summary),
            );
        }

        // The action row: hint left, spacer, Cancel, Import.
        let mut foot = h_flex()
            .flex_none()
            .w_full()
            .items_center()
            .gap(px(8.0))
            .px(px(24.0))
            .py(px(16.0))
            .mt(px(16.0))
            .border_t_1()
            .border_color(p.line);
        if let Some(hint) = self.hint {
            foot = foot.child(div().flex_none().ui(ROW_TOK).text_color(p.ink_3).child(hint));
        }
        foot = foot.child(div().flex_1());
        let mut cancel = crate::data::button((id.clone(), "cancel"), self.cancel_label.clone());
        cancel = cancel.accessibility_label(self.cancel_label.clone());
        if let Some(handler) = self.on_intent.clone() {
            cancel = cancel.on_click(move |_, w, cx| handler(ImportPreviewIntent::Cancel, w, cx));
        }
        let mut primary_btn = crate::data::button((id.clone(), "import"), primary.clone()).primary();
        primary_btn = primary_btn.accessibility_label(primary.clone());
        if selected == 0 {
            primary_btn = primary_btn.disabled(true);
        } else if let Some(handler) = self.on_intent.clone() {
            primary_btn = primary_btn.on_click(move |_, w, cx| handler(ImportPreviewIntent::Import, w, cx));
        }
        foot = foot.child(cancel).child(primary_btn);
        card = card.child(foot);

        let shell = modal_card(&p, PREVIEW_W, &style).child(card);
        let dismiss = self.on_intent.clone().map(|h| {
            Rc::new(move |w: &mut Window, cx: &mut App| h(ImportPreviewIntent::Cancel, w, cx))
                as Rc<dyn Fn(&mut Window, &mut App)>
        });
        modal_scrim(id, style.opacity, dismiss, shell).into_any_element()
    }
}

/// One row of the Add skill menu: the leading icon, the title over its
/// subtitle, and muted trailing text (`3 new`, `none`, `uses a turn`). The
/// view menu has no two-line row, so this is the menu's own row for the Add
/// skill menu. Build with [`menu_row_two_line`].
#[derive(IntoElement)]
pub struct MenuTwoLineRow {
    id: ElementId,
    key: Option<SharedString>,
    icon: IconName,
    title: SharedString,
    subtitle: SharedString,
    subtitle_mono: bool,
    trailing: Option<SharedString>,
    accessibility_label: Option<SharedString>,
    on_activate: Option<MenuActivateHandler>,
}

/// A two-line menu row with `icon`, `title` and `subtitle`.
pub fn menu_row_two_line(
    id: impl Into<ElementId>,
    icon: IconName,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> MenuTwoLineRow {
    MenuTwoLineRow {
        id: id.into(),
        key: None,
        icon,
        title: title.into(),
        subtitle: subtitle.into(),
        subtitle_mono: false,
        trailing: None,
        accessibility_label: None,
        on_activate: None,
    }
}

/// `.mi{gap:10px;padding:8px 10px}` with a 16 px leading glyph and 12.1 px
/// trailing text.
const MENU_ITEM_GAP: f32 = 10.0;
const MENU_ITEM_PAD_Y: f32 = 8.0;
const MENU_ITEM_PAD_X: f32 = 10.0;
const MENU_ITEM_GLYPH: f32 = 16.0;
const MENU_ITEM_TITLE: f32 = 14.3;
const MENU_ITEM_SUB: f32 = 12.1;
const MENU_ITEM_TRAIL: f32 = 12.1;

impl MenuTwoLineRow {
    /// The identity handed back by `on_activate` (by default the title).
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Draws the subtitle in mono (a source path like `~/.claude/skills`).
    pub fn subtitle_mono(mut self) -> Self {
        self.subtitle_mono = true;
        self
    }

    /// Muted text at the row's right edge.
    pub fn trailing(mut self, trailing: impl Into<SharedString>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    /// Overrides the accessible name (by default `title, subtitle`).
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// The row was activated; the argument is the row's id.
    pub fn on_activate(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for MenuTwoLineRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (istate, flags) = interaction_flags(id.clone(), window, cx);
        let ground = tint_fade((id.clone(), "bg"), flags.hovered, p.surface_2, Tween::FAST, window, cx);

        let mut sub = div().flex_none().w_full();
        if self.subtitle_mono {
            sub = sub.mono(MENU_ITEM_SUB).text_color(p.ink_3).child(self.subtitle.clone());
        } else {
            sub = sub.ui(MENU_ITEM_SUB).line_height(relative(1.0)).text_color(p.ink_3).child(self.subtitle.clone());
        }
        let aria = self
            .accessibility_label
            .unwrap_or_else(|| SharedString::from(format!("{}, {}", self.title, self.subtitle)));

        let mut row = h_flex()
            .id(id)
            .w_full()
            .items_start()
            .gap(px(MENU_ITEM_GAP))
            .px(px(MENU_ITEM_PAD_X))
            .py(px(MENU_ITEM_PAD_Y))
            .rounded(px(scale::R_SM))
            .bg(ground)
            .cursor_pointer()
            .track_interaction(&istate)
            .child(div().flex_none().mt(px(2.0)).child(icon(self.icon).size(px(MENU_ITEM_GLYPH)).color(p.ink_3)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(
                        div()
                            .flex_none()
                            .ui(MENU_ITEM_TITLE)
                            .line_height(relative(1.0))
                            .text_color(p.ink)
                            .child(self.title.clone()),
                    )
                    .child(sub),
            )
            .role(gpui::Role::MenuItem)
            .aria_label(aria);
        if let Some(trailing) = self.trailing {
            row = row.child(div().flex_none().mt(px(2.0)).ui(MENU_ITEM_TRAIL).text_color(p.ink_3).child(trailing));
        }
        if let Some(handler) = self.on_activate {
            let key = self.key.unwrap_or_else(|| self.title.clone());
            row = row.on_click(move |_, w, cx| handler(&key, w, cx));
        }
        row
    }
}
