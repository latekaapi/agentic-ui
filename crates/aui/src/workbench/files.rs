//! The worktree file tree of card 54 (`design/src/cards/workbench/54-files-docs.html`,
//! spec §5.5): a panel header with the repository name and the search /
//! refresh actions, a flat list of indented rows carrying a rotating chevron,
//! a muted file-type icon, the name and the git badge, and a footer that
//! states which worktree the tree belongs to.
//!
//! The tree takes a flat, pre-expanded list of [`FileNode`]s (the caller owns
//! the expansion state and the filtering) and reports intents through
//! [`FileTreeAction`]; it never touches a filesystem.

use std::rc::Rc;

use aui_icons::{icon, FileType, IconName};
use aui_motion::{tint_fade, Tween};
use aui_tokens::{scale, ActiveAui, AuiStyled, TextRole};
use gpui::{div, prelude::*, px, App, ElementId, Hsla, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{icon_button, ButtonSize};
use crate::util::{interaction_flags, TrackInteraction};

/// `.hd{height:34px;gap:6px;padding:0 8px 0 12px;font-weight:600;font-size:12.5px}`.
const HEADER_H: f32 = 34.0;
const HEADER_GAP: f32 = 6.0;
const HEADER_PAD_LEFT: f32 = 12.0;
const HEADER_PAD_RIGHT: f32 = 8.0;
const HEADER_TEXT: f32 = 12.5;
/// Glyphs inside the header's xs ghost buttons: `width:12px;height:12px`.
const HEADER_GLYPH: f32 = 12.0;
/// `.tree{padding:6px;font-size:12px}`.
const TREE_PAD: f32 = 6.0;
const TREE_TEXT: f32 = scale::FS_12;
/// `.n{height:24px;gap:6px;padding:0 6px}`.
const ROW_H: f32 = 24.0;
const ROW_GAP: f32 = 6.0;
const ROW_PAD: f32 = 6.0;
/// `.d1{padding-left:18px}.d2{padding-left:32px}.d3{padding-left:46px}` — 14 px
/// per level over a 4 px base.
const INDENT_BASE: f32 = 4.0;
const INDENT_STEP: f32 = 14.0;
/// `.n .chev{width:10px;height:10px}` — narrower than the shared 12 px
/// [`crate::nav::chevron`], so the row asks for [`crate::nav::chevron_sized`].
const CHEVRON: f32 = 10.0;
/// `.n .fic{width:14px;height:14px}`.
const FILE_ICON: f32 = 14.0;
/// `.n .b{font:600 10px var(--font-mono)}`.
const BADGE_TEXT: f32 = 10.0;
/// `.stat{height:28px;gap:10px;padding:0 12px;font:11px var(--font-ui)}`.
const FOOTER_H: f32 = 28.0;
const FOOTER_PAD: f32 = 12.0;
const FOOTER_TEXT: f32 = scale::FS_11;

/// The git status of a tree row, shown as a mono letter at the right of the
/// row (`.n .b`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitBadge {
    /// Modified — warning.
    M,
    /// Added — success.
    A,
    /// Deleted — danger.
    D,
    /// Untracked — ink-4.
    U,
}

impl GitBadge {
    /// The letter drawn in the badge.
    pub fn letter(self) -> &'static str {
        match self {
            GitBadge::M => "M",
            GitBadge::A => "A",
            GitBadge::D => "D",
            GitBadge::U => "U",
        }
    }

    /// The badge colour: `.b.m` warning, `.b.a` success, deleted danger,
    /// `.b.u` ink-4.
    pub fn color(self, colors: &aui_tokens::Palette) -> Hsla {
        match self {
            GitBadge::M => colors.warning,
            GitBadge::A => colors.success,
            GitBadge::D => colors.danger,
            GitBadge::U => colors.ink_4,
        }
    }
}

/// One row of the tree. Directories carry `open`; files leave it `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct FileNode {
    /// Stable id, reported by [`FileTreeAction::Select`] and
    /// [`FileTreeAction::Toggle`] (usually the path).
    pub id: SharedString,
    /// The name shown on the row (the last path segment).
    pub name: SharedString,
    /// The file-type icon and hue.
    pub kind: FileType,
    /// Nesting level; level 0 sits at the tree's own padding.
    pub depth: usize,
    /// `Some(open)` marks the row as a directory and draws the chevron.
    pub open: Option<bool>,
    /// The git status letter at the right of the row.
    pub badge: Option<GitBadge>,
    /// The selected row (`.n.on`).
    pub selected: bool,
}

impl FileNode {
    /// A file row.
    pub fn file(id: impl Into<SharedString>, name: impl Into<SharedString>, kind: FileType) -> Self {
        Self { id: id.into(), name: name.into(), kind, depth: 0, open: None, badge: None, selected: false }
    }

    /// A directory row; `open` picks the open / closed folder icon and the
    /// chevron's rotation.
    pub fn dir(id: impl Into<SharedString>, name: impl Into<SharedString>, open: bool) -> Self {
        let kind = if open { FileType::FolderOpen } else { FileType::Folder };
        Self { id: id.into(), name: name.into(), kind, depth: 0, open: Some(open), badge: None, selected: false }
    }

    /// Sets the nesting level.
    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Sets the git badge.
    pub fn badge(mut self, badge: GitBadge) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Marks the row selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Whether the row is a directory.
    pub fn is_dir(&self) -> bool {
        self.open.is_some()
    }
}

/// What a tree row or header control asks the application to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTreeAction {
    /// Open the node with this id.
    Select(SharedString),
    /// Expand or collapse the directory with this id.
    Toggle(SharedString),
    /// The header's search button.
    Search,
    /// The header's refresh button.
    Refresh,
}

type ActionHandler = Rc<dyn Fn(&FileTreeAction, &mut Window, &mut App)>;

/// The file tree. Build with [`file_tree`].
#[derive(IntoElement)]
pub struct FileTree {
    id: ElementId,
    nodes: Vec<FileNode>,
    header: Option<SharedString>,
    footer: Option<SharedString>,
    flush: bool,
    on_action: Option<ActionHandler>,
}

/// A file tree over a flat list of pre-expanded rows.
pub fn file_tree(id: impl Into<ElementId>, nodes: Vec<FileNode>) -> FileTree {
    FileTree { id: id.into(), nodes, header: None, footer: None, flush: false, on_action: None }
}

impl FileTree {
    /// The header's repository or root name; without it the header is hidden.
    pub fn header(mut self, name: impl Into<SharedString>) -> Self {
        self.header = Some(name.into());
        self
    }

    /// The footer status line ("worktree checkout-flow-v2 · 2 modified · 1 added").
    pub fn footer(mut self, text: impl Into<SharedString>) -> Self {
        self.footer = Some(text.into());
        self
    }

    /// Drops the outer card chrome for a tree mounted as a full-height pane
    /// or stacked with other panels, where the column already owns the
    /// edges. The tree draws no outer radius or border today, so this is
    /// already flush; the builder exists so all four pane components share
    /// the same opt-in and a future border does not reappear unasked.
    /// Keeps every internal divider, the header's bottom hairline and all
    /// padding. Off by default.
    pub fn flush(mut self) -> Self {
        self.flush = true;
        self
    }

    /// Called with every intent the tree emits.
    pub fn on_action(mut self, f: impl Fn(&FileTreeAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for FileTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // The tree draws no outer radius or border, so `flush` changes
        // nothing visually; the read keeps the opt-in part of the build.
        let _ = self.flush;
        let mut panel = v_flex().id(id.clone()).size_full().overflow_hidden().bg(p.surface_1);

        if let Some(name) = self.header {
            let search = self.on_action.clone();
            let refresh = self.on_action.clone();
            panel = panel.child(
                h_flex()
                    .w_full()
                    .flex_none()
                    .h(px(HEADER_H))
                    .gap(px(HEADER_GAP))
                    .pl(px(HEADER_PAD_LEFT))
                    .pr(px(HEADER_PAD_RIGHT))
                    .border_b_1()
                    .border_color(p.line)
                    .text_color(p.ink)
                    .ui(HEADER_TEXT)
                    .semibold()
                    .child(icon(IconName::Folder).color(p.ink))
                    .child(div().flex_1().min_w(px(0.0)).truncate().child(name))
                    .child(
                        icon_button((id.clone(), "search"), IconName::Search)
                            .ghost()
                            .size(ButtonSize::Xs)
                            .icon_size(px(HEADER_GLYPH))
                            .on_click(move |_, w, cx| {
                                if let Some(f) = &search {
                                    f(&FileTreeAction::Search, w, cx)
                                }
                            }),
                    )
                    .child(
                        icon_button((id.clone(), "refresh"), IconName::Refresh)
                            .ghost()
                            .size(ButtonSize::Xs)
                            .icon_size(px(HEADER_GLYPH))
                            .on_click(move |_, w, cx| {
                                if let Some(f) = &refresh {
                                    f(&FileTreeAction::Refresh, w, cx)
                                }
                            }),
                    ),
            );
        }

        let mut tree = v_flex().w_full().flex_none().p(px(TREE_PAD)).ui(TREE_TEXT);
        for node in self.nodes {
            tree = tree.child(tree_row((id.clone(), node.id.clone()), node, self.on_action.clone(), window, cx));
        }
        panel = panel.child(tree);

        if let Some(text) = self.footer {
            panel = panel.child(
                h_flex()
                    .w_full()
                    .flex_none()
                    .h(px(FOOTER_H))
                    .px(px(FOOTER_PAD))
                    .border_t_1()
                    .border_color(p.line)
                    .ui(FOOTER_TEXT)
                    .text_color(p.ink_3)
                    // The status wraps inside the panel's width the way the
                    // CSS one does, overflowing the 28 px band rather than
                    // truncating.
                    .child(div().min_w(px(0.0)).child(text)),
            );
        }
        panel
    }
}

/// One `.n` row: chevron (directories only), file-type icon, name, git badge.
fn tree_row(id: impl Into<ElementId>, node: FileNode, on_action: Option<ActionHandler>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let (state, flags) = interaction_flags(id.clone(), window, cx);
    // `.n:hover{background:var(--surface-2)}`, `.n.on{background:var(--accent-soft);color:var(--ink)}`.
    let tint = if node.selected { p.accent_soft } else { p.surface_2 };
    let bg = tint_fade((id.clone(), "bg"), node.selected || flags.hovered, tint, Tween::FAST, window, cx);
    // `.n{color:var(--ink-2)}`, `.n.dir{color:var(--ink)}`.
    let text = if node.selected || node.is_dir() { p.ink } else { p.ink_2 };
    let icon_color = node.kind.hue().map(Into::into).unwrap_or(p.ink_3);
    let indent = if node.depth == 0 { ROW_PAD } else { INDENT_BASE + INDENT_STEP * node.depth as f32 };

    let mut row = h_flex()
        .id(id.clone())
        .w_full()
        .flex_none()
        .h(px(ROW_H))
        .gap(px(ROW_GAP))
        .pl(px(indent))
        .pr(px(ROW_PAD))
        .rounded(px(scale::R_SM))
        .bg(bg)
        .text_color(text)
        .cursor_pointer()
        .track_interaction(&state);

    if let Some(open) = node.open {
        row = row.child(crate::nav::chevron_sized(id.clone(), open, p.ink_3, CHEVRON, window, cx));
    }
    row = row
        .child(icon(node.kind.icon()).size(px(FILE_ICON)).color(icon_color))
        .child(div().min_w(px(0.0)).truncate().child(node.name.clone()));

    if let Some(badge) = node.badge {
        row = row.child(div().flex_1().min_w(px(0.0))).child(
            div()
                .flex_none()
                .text_role(TextRole::MonoSmall)
                .text_px(BADGE_TEXT)
                .semibold()
                .text_color(badge.color(&p))
                .child(badge.letter()),
        );
    }

    if let Some(f) = on_action {
        let node_id = node.id.clone();
        let is_dir = node.is_dir();
        row = row.on_click(move |_, w, cx| {
            let action = if is_dir { FileTreeAction::Toggle(node_id.clone()) } else { FileTreeAction::Select(node_id.clone()) };
            f(&action, w, cx)
        });
    }
    row
}
