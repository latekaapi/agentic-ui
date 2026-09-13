//! `folder_drop_card`: the full-width dashed card that accepts a folder —
//! by click (to choose one) or by dropping one onto it.

use std::path::PathBuf;
use std::rc::Rc;

use aui_icons::{icon, IconName};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, ExternalPaths, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::kbd;
use crate::util::{interaction_flags, TrackInteraction};

/// The folder glyph: 20 px, ink-2.
const GLYPH: f32 = 20.0;
/// The centred stack breathes as one: glyph, title, subtitle.
const STACK_GAP: f32 = 4.0;

type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type DropHandler = Rc<dyn Fn(Vec<PathBuf>, &mut Window, &mut App)>;

/// A full-width card that takes a folder. Build with [`folder_drop_card`].
///
/// The whole card is the click target; a drag-over of [`ExternalPaths`]
/// lights the border in accent and the ground in accent-soft, and a drop
/// hands the drop handler every dropped path that is a directory while
/// files are ignored.
#[derive(IntoElement)]
pub struct FolderDropCard {
    id: ElementId,
    title: SharedString,
    subtitle: SharedString,
    key: Option<SharedString>,
    dragging: bool,
    on_click: Option<ClickHandler>,
    on_drop: Option<DropHandler>,
}

/// A card reading "Drop a folder here" / "or click to choose one".
pub fn folder_drop_card(id: impl Into<ElementId>) -> FolderDropCard {
    FolderDropCard {
        id: id.into(),
        title: "Drop a folder here".into(),
        subtitle: "or click to choose one".into(),
        key: None,
        dragging: false,
        on_click: None,
        on_drop: None,
    }
}

/// Every path in `paths` that is a directory, in order; files are ignored.
fn dirs_only(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths.iter().filter(|path| path.is_dir()).cloned().collect()
}

impl FolderDropCard {
    /// Overrides the title (`Drop a folder here`).
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Overrides the subtitle (`or click to choose one`).
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// A keycap at the card's right (`⌘⇧O`): the shortcut that chooses.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Draws the drag-over state statically: accent border on accent-soft.
    ///
    /// Gallery-only — static captures cannot hold a real platform drag, so
    /// the card fakes the lit state for them. The live card never sets this;
    /// it lights through `drag_over` while [`ExternalPaths`] hover instead.
    pub fn dragging(mut self, dragging: bool) -> Self {
        self.dragging = dragging;
        self
    }

    /// Click on the card (choose a folder).
    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    /// A drop landed: every dropped path that is a directory.
    pub fn on_drop(mut self, f: impl Fn(Vec<PathBuf>, &mut Window, &mut App) + 'static) -> Self {
        self.on_drop = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for FolderDropCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let (state, _) = interaction_flags(id.clone(), window, cx);
        let clickable = self.on_click.is_some();
        // The keycap sits at the right without moving the centred stack:
        // equal spacers on both sides keep the middle truly centred.
        let mut card = h_flex()
            .id(id)
            .relative()
            .w_full()
            .items_center()
            .p(px(scale::SP_5))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_dashed()
            .border_color(if self.dragging { p.accent } else { p.line_strong })
            .bg(if self.dragging { p.accent_soft } else { p.surface_2 })
            .track_interaction(&state)
            .when(clickable, |d| d.cursor_pointer())
            .child(div().flex_1())
            .child(
                v_flex()
                    .flex_none()
                    .items_center()
                    .gap(px(STACK_GAP))
                    .child(icon(IconName::Folder).size(px(GLYPH)).color(p.ink_2))
                    .child(div().ui(scale::FS_13).semibold().text_color(p.ink).child(self.title))
                    .child(div().ui(scale::FS_12).text_color(p.ink_3).child(self.subtitle)),
            )
            .child(div().flex_1().flex().justify_end().children(self.key.map(kbd)));
        if let Some(on_click) = self.on_click {
            card = card.on_click(move |_, w, cx| on_click(w, cx));
        }
        // A platform file-drag over the card lights it the way `dragging`
        // draws it; the drop hands over the directories and ignores files.
        card = card.drag_over::<ExternalPaths>(|style, _, _, cx| {
            let p = cx.aui().colors;
            style.bg(p.accent_soft).border_color(p.accent)
        });
        if let Some(on_drop) = self.on_drop {
            card = card.on_drop(move |paths: &ExternalPaths, w, cx| {
                on_drop(dirs_only(paths.paths()), w, cx);
            });
        }
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory passes the drop filter; a file does not.
    #[test]
    fn drop_hands_over_directories_and_ignores_files() {
        let root = std::env::temp_dir().join("aui-folder-drop-filter");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("work")).unwrap();
        std::fs::write(root.join("notes.md"), "hi").unwrap();
        let paths = vec![root.join("work"), root.join("notes.md"), root.join("missing")];
        assert_eq!(dirs_only(&paths), vec![root.join("work")]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
