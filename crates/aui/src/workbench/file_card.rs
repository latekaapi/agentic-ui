//! The file card (`.fc`): a file the agent produced, with a tinted type
//! tile, its name and meta line, and host-defined action buttons.

use std::rc::Rc;

use aui_icons::icon;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::button;
use crate::workbench::docs::ArtifactKind;

/// `.fc{gap:12px;padding:10px 12px}` with a 36 px icon tile.
const CARD_GAP: f32 = 12.0;
const CARD_PAD_Y: f32 = 10.0;
const CARD_PAD_X: f32 = 12.0;
const TILE: f32 = 36.0;
const TILE_RADIUS: f32 = 8.0;
/// `.fc .ic svg{width:18px;height:18px}`.
const TILE_GLYPH: f32 = 18.0;
/// `.fc b{font-size:13px}` and `.fc span.d{font-size:12px}`.
const NAME_TEXT: f32 = scale::FS_13;
const META_TEXT: f32 = scale::FS_12;
/// The status word after the meta line: 11 px semibold.
const STATUS_TEXT: f32 = scale::FS_11;
/// `.fc .acts{gap:6px}`.
const ACTIONS_GAP: f32 = 6.0;

type ActionHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One host-defined action button on a [`FileCard`].
struct FileCardAction {
    id: SharedString,
    label: SharedString,
}

/// A file the agent produced (`.fc`). Build with [`file_card`].
#[derive(IntoElement)]
pub struct FileCard {
    id: ElementId,
    name: SharedString,
    kind: ArtifactKind,
    meta: Option<SharedString>,
    status: Option<SharedString>,
    actions: Vec<FileCardAction>,
    active: bool,
    on_action: Option<ActionHandler>,
    on_click: Option<ClickHandler>,
}

/// A file card for `name`: a 36 px tinted type tile, the bold single-line
/// name, and whatever [`FileCard::meta`], [`FileCard::status`] and
/// [`FileCard::action`] add.
pub fn file_card(id: impl Into<ElementId>, name: impl Into<SharedString>) -> FileCard {
    FileCard {
        id: id.into(),
        name: name.into(),
        kind: ArtifactKind::Other,
        meta: None,
        status: None,
        actions: Vec::new(),
        active: false,
        on_action: None,
        on_click: None,
    }
}

impl FileCard {
    /// The type tile's glyph and tint. Defaults to [`ArtifactKind::Other`].
    pub fn kind(mut self, kind: ArtifactKind) -> Self {
        self.kind = kind;
        self
    }

    /// The meta line under the name (`Section 2 rewritten · v3`).
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// The status word after the meta line (`Created`, `Updated`).
    pub fn status(mut self, status: impl Into<SharedString>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Adds an action button with a stable `action_id`, reported through
    /// [`FileCard::on_action`]. Repeatable; every button but the last is
    /// ghost, the last is the primary call to action.
    pub fn action(mut self, action_id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.actions.push(FileCardAction { id: action_id.into(), label: label.into() });
        self
    }

    /// Draws the selected state (the file currently open in the pane).
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Called with the action id when an action button is pressed.
    pub fn on_action(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    /// Called when the card's text body (tile and labels) is pressed; the
    /// action buttons report through [`FileCard::on_action`] instead.
    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for FileCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let tint = self.kind.tint(&p).unwrap_or(p.ink_2);
        let (border, bg) = if self.active { (p.accent_ring, p.accent_soft) } else { (p.line, p.surface_1) };
        let mut body = h_flex()
            .id((id.clone(), "body"))
            .flex_1()
            .min_w(px(0.0))
            .gap(px(CARD_GAP))
            .items_center()
            .child(
                div()
                    .flex_none()
                    .size(px(TILE))
                    .rounded(px(TILE_RADIUS))
                    .bg(p.surface_2)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(self.kind.icon()).size(px(TILE_GLYPH)).color(tint)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().truncate().ui(NAME_TEXT).semibold().text_color(p.ink).child(self.name.clone()))
                    .child({
                        let mut row = h_flex().w_full().gap(px(CARD_GAP)).items_center();
                        match self.meta.clone() {
                            Some(meta) => row = row.child(div().flex_1().min_w(px(0.0)).truncate().ui(META_TEXT).text_color(p.ink_3).child(meta)),
                            None => row = row.child(div().flex_1().min_w(px(0.0))),
                        }
                        if let Some(status) = self.status.clone() {
                            row = row.child(div().flex_none().ui(STATUS_TEXT).semibold().text_color(p.success).child(status));
                        }
                        row
                    }),
            );
        if let Some(f) = self.on_click.clone() {
            body = body.cursor_pointer().on_click(move |_, w, cx| f(w, cx));
        }
        let mut card = h_flex()
            .id(id.clone())
            .w_full()
            .gap(px(CARD_GAP))
            .py(px(CARD_PAD_Y))
            .px(px(CARD_PAD_X))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(border)
            .bg(bg)
            .child(body);
        if !self.actions.is_empty() {
            let last = self.actions.len() - 1;
            let mut acts = h_flex().flex_none().gap(px(ACTIONS_GAP)).items_center();
            for (index, action) in self.actions.iter().enumerate() {
                let press = {
                    let handler = self.on_action.clone();
                    let action_id = action.id.clone();
                    move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                        if let Some(h) = &handler {
                            h(&action_id, w, cx);
                        }
                    }
                };
                let mut el = button((id.clone(), action.id.clone()), action.label.clone()).sm();
                if index != last {
                    el = el.ghost();
                }
                acts = acts.child(el.on_click(press));
            }
            card = card.child(acts);
        }
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The builder carries the name and defaults to the generic kind with
    /// no meta, status or actions.
    #[test]
    fn a_new_card_carries_its_name_and_defaults() {
        let card = file_card("fc", "RFP-draft-v3.docx");
        assert_eq!(card.name.to_string(), "RFP-draft-v3.docx", "the builder keeps the name");
        assert_eq!(card.kind, ArtifactKind::Other, "the tile defaults to the generic kind");
        assert!(card.meta.is_none(), "no meta line by default");
        assert!(card.status.is_none(), "no status word by default");
        assert!(card.actions.is_empty(), "no actions by default");
        assert!(!card.active, "not selected by default");
    }

    /// Meta, status, kind and the selected state round-trip through the
    /// builder, and actions keep their ids in order.
    #[test]
    fn builder_setters_round_trip() {
        let card = file_card("fc", "vendor-scoring.xlsx")
            .kind(ArtifactKind::Sheet)
            .meta("Scores, Matrix and Notes sheets")
            .status("Updated")
            .action("download", "Download")
            .action("open", "Open in pane")
            .active(true);
        assert_eq!(card.kind, ArtifactKind::Sheet, "the tile follows the kind");
        assert_eq!(card.meta.as_deref(), Some("Scores, Matrix and Notes sheets"), "the builder keeps the meta line");
        assert_eq!(card.status.as_deref(), Some("Updated"), "the builder keeps the status word");
        assert!(card.active, "the selected state sticks");
        let ids: Vec<&str> = card.actions.iter().map(|a| a.id.as_ref()).collect();
        assert_eq!(ids, vec!["download", "open"], "action ids stay in order, got {ids:?}");
        let labels: Vec<&str> = card.actions.iter().map(|a| a.label.as_ref()).collect();
        assert_eq!(labels, vec!["Download", "Open in pane"], "action labels stay with their ids, got {labels:?}");
    }

    /// The handlers are stored when set, so presses have somewhere to go.
    #[test]
    fn handlers_are_stored_when_set() {
        let card = file_card("fc", "notes.md").on_action(|_, _, _| {}).on_click(|_, _| {});
        assert!(card.on_action.is_some(), "the action handler is stored");
        assert!(card.on_click.is_some(), "the body handler is stored");
    }
}
