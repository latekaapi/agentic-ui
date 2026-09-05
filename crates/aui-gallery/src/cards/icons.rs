//! Card 05 · Icons and marks. The glyph set, the provider marks, the
//! file-type icons and the status glyphs. Reproduces
//! `design/src/cards/foundations/05-icons.html` at 760×380.

use aui_icons::{icon, provider_mark, FileType, IconName, Provider, RoleIcon};
use aui_tokens::{ActiveAui, scale, AuiStyled, Palette, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

/// `.caps{margin-bottom:10px}` above every section.
const SECTION_LABEL_GAP: f32 = 10.0;
/// `.ig`, `.marks`{margin-bottom:18px}.
const SECTION_GAP: f32 = 18.0;
/// `.ig{grid-template-columns:repeat(12,1fr)}`.
const GLYPH_COLUMNS: usize = 12;
/// `.ig{gap:10px 6px}` — row gap.
const GLYPH_ROW_GAP: f32 = 10.0;
/// `.ig{gap:10px 6px}` — column gap.
const GLYPH_COL_GAP: f32 = 6.0;
/// `.ig div{gap:5px}` between a glyph and its caption.
const GLYPH_LABEL_GAP: f32 = 5.0;
/// `.ig div{font:10px/1 var(--font-mono)}`.
const GLYPH_LABEL_SIZE: f32 = 10.0;
/// Captions are set solid (`/1`).
const LH_FLAT: f32 = 1.0;
/// `.ig svg{width:16px;height:16px}` — the glyph grid uses the header size.
const GLYPH_SIZE: f32 = 16.0;
/// `.marks{gap:14px}`.
const MARKS_GAP: f32 = 14.0;
/// `.marks span{gap:6px}`.
const MARK_LABEL_GAP: f32 = 6.0;
/// The extra `margin-left:12px` that starts a sub-group inside a `.marks` row.
const MARKS_GROUP_INDENT: f32 = 12.0;
/// `.i`, `.fic`{width:14px;height:14px}.
const ICON_SIZE: f32 = 14.0;
/// `.glyph{width:14px;height:14px;border-radius:50%}` and `.spinner` likewise.
const GLYPH_TILE: f32 = 14.0;
/// The check / x inside a status glyph: `width:9px;height:9px`.
const GLYPH_MARK: f32 = 9.0;
/// `.spinner{border:1.5px …}`.
const SPINNER_RING: f32 = 1.5;
/// Height of the accent segment of the spinner ring at rest: the `border-top`
/// of a 14 px circle covers the top 90° arc, i.e. `14 · (1 − cos45°) / 2`.
const SPINNER_CAP: f32 = 2.05;
/// `.dot{width:7px;height:7px}`.
const DOT_SIZE: f32 = 7.0;
/// `.caps` inherits the body line height (1.5) in this card.
const CAPS_LH: f32 = scale::LH_UI;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .child(section_label(p, "Glyphs"))
        .child(glyph_grid(p))
        .child(section_label(p, "Provider marks · 16 px, rounded 4"))
        .child(provider_row(p))
        .child(section_label(p, "File types · muted hues, no ground"))
        .child(file_type_row(p))
        .child(section_label(p, "Status glyphs and document kinds"))
        .child(status_row(p))
        .into_any_element()
}

/// A `.caps` section heading with its 10 px bottom margin.
fn section_label(p: Palette, label: &'static str) -> impl IntoElement {
    div()
        .mb(px(SECTION_LABEL_GAP))
        .text_role(TextRole::Caps)
        .line_height(relative(CAPS_LH))
        .text_color(p.ink_3)
        .child(label.to_uppercase())
}

/// `.ig`: the glyph set on a twelve-column grid, built from equal-width flex
/// cells because gpui has no CSS grid.
fn glyph_grid(p: Palette) -> impl IntoElement {
    let glyphs: [(IconName, &'static str); 24] = [
        (IconName::Terminal, "terminal"),
        (IconName::File, "file"),
        (IconName::Folder, "folder"),
        (IconName::Search, "search"),
        (IconName::Globe, "browser"),
        (IconName::Edit, "edit"),
        (IconName::Eye, "read"),
        (IconName::Git, "git"),
        (IconName::Brain, "thinking"),
        (IconName::Shield, "approval"),
        (IconName::Question, "question"),
        (IconName::List, "plan"),
        (IconName::Sparkle, "agent"),
        (IconName::Zap, "automation"),
        (IconName::Inbox, "inbox"),
        (IconName::Bell, "notify"),
        (IconName::Pin, "pin"),
        (IconName::Split, "split"),
        (IconName::Layout, "layout"),
        (IconName::Camera, "screenshot"),
        (IconName::Cursor, "pick"),
        (IconName::Paperclip, "attach"),
        (IconName::At, "mention"),
        (IconName::Slash, "command"),
    ];
    let mut grid = v_flex()
        .w_full()
        .mb(px(SECTION_GAP))
        .gap(px(GLYPH_ROW_GAP));
    for chunk in glyphs.chunks(GLYPH_COLUMNS) {
        let mut row = h_flex().w_full().items_start().gap(px(GLYPH_COL_GAP));
        for (name, label) in chunk {
            row = row.child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .items_center()
                    .gap(px(GLYPH_LABEL_GAP))
                    .child(icon(*name).size(px(GLYPH_SIZE)).color(p.ink))
                    .child(
                        div()
                            .font_family(scale::FONT_MONO)
                            .text_px(GLYPH_LABEL_SIZE)
                            .line_height(relative(LH_FLAT))
                            // The caption is a flex item in the CSS, so a long
                            // word overflows its cell instead of wrapping.
                            .whitespace_nowrap()
                            .text_color(p.ink_3)
                            .child(*label),
                    ),
            );
        }
        grid = grid.child(row);
    }
    grid
}

/// `.marks`: the six provider marks with their names.
fn provider_row(p: Palette) -> impl IntoElement {
    let names = [
        (Provider::Claude, "Claude Code"),
        (Provider::Codex, "Codex"),
        (Provider::Grok, "Grok"),
        (Provider::Gemini, "Gemini"),
        (Provider::Pi, "Pi"),
        (Provider::Cursor, "Cursor"),
    ];
    let mut row = marks_row(p);
    for (provider, label) in names {
        row = row.child(mark_item(provider_mark(provider), label, 0.0));
    }
    row
}

/// `.marks`: the file-type icons, then the role icons and the pane toggle.
fn file_type_row(p: Palette) -> impl IntoElement {
    let files = [
        (FileType::Folder, "folder"),
        (FileType::FolderOpen, "open"),
        (FileType::Ts, "ts"),
        (FileType::Tsx, "tsx"),
        (FileType::Json, "json"),
        (FileType::Md, "md"),
        (FileType::Test, "test"),
        (FileType::Css, "css"),
        (FileType::Lock, "lock"),
    ];
    let mut row = marks_row(p);
    for (file_type, label) in files {
        // `.fic` defaults to ink-3; `.fic.lock` overrides to ink-4.
        let color = match file_type {
            FileType::Lock => p.ink_4,
            other => other.hue().map(Hsla::from).unwrap_or(p.ink_3),
        };
        row = row.child(mark_item(
            icon(file_type.icon()).size(px(ICON_SIZE)).color(color),
            label,
            0.0,
        ));
    }
    for (i, (role, label)) in [
        (RoleIcon::GradCap, "education"),
        (RoleIcon::Scale, "law"),
        (RoleIcon::Gear, "operations"),
    ]
    .into_iter()
    .enumerate()
    {
        let indent = if i == 0 { MARKS_GROUP_INDENT } else { 0.0 };
        row = row.child(mark_item(
            icon(role.icon()).size(px(ICON_SIZE)).color(p.ink_3),
            label,
            indent,
        ));
    }
    row.child(mark_item(
        icon(IconName::PanelRight).size(px(ICON_SIZE)).color(p.ink_3),
        "toggle pane",
        0.0,
    ))
}

/// `.marks`: the status glyphs and the document-kind icons.
fn status_row(p: Palette) -> impl IntoElement {
    let kinds = [
        (IconName::Doc, "docx", p.info),
        (IconName::Sheet, "xlsx", p.success),
        (IconName::Pdf, "pdf", p.danger),
        (IconName::Image, "image", p.ink_2),
        (IconName::Play, "video", p.accent),
    ];
    let mut row = marks_row(p)
        .child(mark_item(spinner(p), "running", 0.0))
        .child(mark_item(
            status_glyph(p.success_soft, icon(IconName::Check).size(px(GLYPH_MARK)).color(p.success)),
            "success",
            0.0,
        ))
        .child(mark_item(
            status_glyph(p.danger_soft, icon(IconName::X).size(px(GLYPH_MARK)).color(p.danger)),
            "error",
            0.0,
        ))
        .child(mark_item(
            div().size(px(DOT_SIZE)).flex_none().rounded_full().bg(p.warning),
            "needs you",
            0.0,
        ));
    for (i, (name, label, color)) in kinds.into_iter().enumerate() {
        let indent = if i == 0 { MARKS_GROUP_INDENT } else { 0.0 };
        row = row.child(mark_item(icon(name).size(px(ICON_SIZE)).color(color), label, indent));
    }
    row
}

/// The shared `.marks` container: a wrapping row of icon + label pairs.
fn marks_row(p: Palette) -> Div {
    h_flex()
        .w_full()
        .flex_wrap()
        .items_center()
        .mb(px(SECTION_GAP))
        .gap(px(MARKS_GAP))
        .ui(scale::FS_12)
        .text_color(p.ink_2)
}

/// One `.marks span`: a 14–16 px glyph and its label.
fn mark_item(glyph: impl IntoElement, label: &'static str, indent: f32) -> impl IntoElement {
    h_flex()
        .flex_none()
        .items_center()
        .gap(px(MARK_LABEL_GAP))
        .ml(px(indent))
        .child(glyph)
        .child(div().whitespace_nowrap().child(label))
}

/// `.glyph.ok` / `.glyph.err`: a 14 px tinted disc around a 9 px mark.
fn status_glyph(ground: Hsla, mark: impl IntoElement) -> impl IntoElement {
    div()
        .size(px(GLYPH_TILE))
        .flex_none()
        .rounded_full()
        .bg(ground)
        .flex()
        .items_center()
        .justify_center()
        .child(mark)
}

/// `.spinner` at its resting frame: a line-strong ring whose top 90° arc is
/// accent. gpui has no per-side border colour, so the accent arc is a clipped
/// overlay of the same ring.
fn spinner(p: Palette) -> impl IntoElement {
    div()
        .relative()
        .size(px(GLYPH_TILE))
        .flex_none()
        .rounded_full()
        .border(px(SPINNER_RING))
        .border_color(p.line_strong)
        .child(
            div()
                .absolute()
                .top(-px(SPINNER_RING))
                .left(-px(SPINNER_RING))
                .w(px(GLYPH_TILE))
                .h(px(SPINNER_CAP))
                .overflow_hidden()
                .child(
                    div()
                        .size(px(GLYPH_TILE))
                        .rounded_full()
                        .border(px(SPINNER_RING))
                        .border_color(p.accent),
                ),
        )
}
