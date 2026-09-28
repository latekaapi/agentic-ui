//! X6b narrow-pane cards. Each build mounts one workbench panel in a fluid
//! (`w_full`) container, so `--width <px>` captures it at a true narrow pane
//! width. The card frame keeps its 20 px padding, so `--width 320` mounts the
//! panel at 280 px, `--width 400` at 360 px and `--width 560` at 520 px.

use aui::protocol::{ChangeKind, Diff, DiffKind, DiffLine, FileChange, Hunk};
use aui::workbench::{
    annotations_panel, browser_nav, doc_pane, doc_tabs, doc_toolbar, diff_review, file_tree, git_changes, pr_check, pr_form, Annotation,
    DiffHighlight, DiffScope, DiffView, DocBlock, DocPage, FileNode, GitBadge, PrDescription, ReviewFile, ReviewNote,
};
use aui::shell::TabItem;
use aui_icons::{FileType, IconName};
use aui_webview::page::page;
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

// ── shared samples ──────────────────────────────────────────────────────────

fn line(kind: DiffKind, old_no: Option<u32>, new_no: Option<u32>, text: &str) -> DiffLine {
    DiffLine { kind, old_no, new_no, text: text.into() }
}

fn sample_diff() -> Diff {
    Diff {
        path: "src/checkout/validators.ts".into(),
        hunks: vec![Hunk {
            header: "@@ -42,7 +42,9 @@ export function validateAddress".into(),
            lines: vec![
                line(DiffKind::Context, Some(44), Some(44), "export function validateAddress(values: AddressValues) {"),
                line(DiffKind::Del, Some(45), None, "  if (!values.country) return true;"),
                line(DiffKind::Add, None, Some(45), "  if (!values.country) return { ok: false, field: 'country' };"),
                line(DiffKind::Add, None, Some(46), "  if (values.country === 'CA') return validateCanadianPostal(values);"),
                line(DiffKind::Context, Some(46), Some(47), "  // US ZIP or ZIP+4"),
            ],
        }],
        added: 8,
        removed: 3,
    }
}

fn sample_files() -> Vec<ReviewFile> {
    let file = |path: &str, change: ChangeKind, added: u32, removed: u32, notes: usize, selected: bool| ReviewFile {
        change: FileChange { path: path.into(), change, added, removed },
        notes,
        selected,
    };
    vec![
        file("src/checkout/validators.ts", ChangeKind::Modified, 8, 3, 2, true),
        file("src/checkout/validators.test.ts", ChangeKind::Added, 41, 0, 0, false),
        file("src/checkout/AddressForm.tsx", ChangeKind::Modified, 2, 1, 0, false),
    ]
}

fn sample_notes() -> Vec<ReviewNote> {
    vec![ReviewNote {
        file: "src/checkout/validators.ts".into(),
        line: 46,
        text: "Also handle 'GB' here, postcode format differs.".into(),
        summary: Some("also handle GB".into()),
        pending: false,
    }]
}

fn sample_highlights() -> Vec<DiffHighlight> {
    let prefix = "  if (!values.country) ".len();
    vec![DiffHighlight { hunk: 0, line: 2, start: prefix, end: "  if (!values.country) return { ok: false, field: 'country' };".len() }]
}

/// Diff review mounted fluidly: the frame follows the card width.
pub fn build_diff(_window: &mut Window, _cx: &mut App) -> AnyElement {
    v_flex()
        .w_full()
        .child(
            diff_review("x6b-diff", sample_files(), sample_diff(), sample_notes(), DiffScope::ThisTurn, DiffView::Unified)
                .summary("vs main · 3 files", 51, 7)
                .highlights(sample_highlights()),
        )
        .into_any_element()
}

fn change(path: &str, change: ChangeKind, added: u32, removed: u32) -> FileChange {
    FileChange { path: path.into(), change, added, removed }
}

/// Git changes over the Create PR form, both fluid.
pub fn build_git(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let files = vec![
        (change("validators.ts", ChangeKind::Modified, 8, 3), true),
        (change("validators.test.ts", ChangeKind::Added, 41, 0), true),
        (change("AddressForm.tsx", ChangeKind::Modified, 2, 1), false),
    ];
    let message = "Validate addresses per country and add CA/GB coverage";
    let description = vec![PrDescription::Text("Adds regression tests for US ZIP+4 and Canadian postal codes.".into())];
    let checks = vec![pr_check("ci / unit", true).detail("2 m 10 s"), pr_check("lint", true)];
    // Each panel sits in a bounded box taller than its content, so the
    // capture proves the root fills the pane (panel ground, pinned
    // footers) instead of sizing to its content.
    v_flex()
        .w_full()
        .gap(px(16.0))
        .child(
            v_flex().w_full().h(px(500.0)).child(
                git_changes("x6b-changes", files, message, 3, 0).branch("feature/checkout-flow-v2").on_action(|_, _, _| {}),
            ),
        )
        .child(
            v_flex().w_full().h(px(540.0)).child(
                pr_form("x6b-pr", "main", "Validate addresses per country and add CA/GB coverage", description, checks)
                    .provider("GitHub")
                    .on_action(|_, _, _| {}),
            ),
        )
        .into_any_element()
}

/// File tree in a bounded-height fluid container so its root must fill it.
pub fn build_files(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let nodes = vec![
        FileNode::dir("src", "src", true).badge(GitBadge::M),
        FileNode::dir("src/checkout", "checkout", true).depth(1).badge(GitBadge::M),
        FileNode::file("src/checkout/validators.ts", "validators.ts", FileType::Ts).depth(2).badge(GitBadge::M).selected(true),
        FileNode::file("src/checkout/validators.test.ts", "validators.test.ts", FileType::Test).depth(2).badge(GitBadge::A),
        FileNode::file("package.json", "package.json", FileType::Json),
    ];
    v_flex()
        .w_full()
        .h(px(560.0))
        .child(file_tree("x6b-tree", nodes).header("acme-web").footer("worktree checkout-flow-v2 · 2 modified · 1 added").on_action(|_, _, _| {}))
        .into_any_element()
}

/// Browser nav over the annotations panel in a bounded-height fluid row.
pub fn build_browser(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let annotations = vec![
        Annotation::new(1, "h1 · “Simple pricing”", "Headline is fine; subtitle needs the free-trial length."),
        Annotation::new(2, "div.card.starter", "Make the Starter card stand out; it is the plan most people pick."),
    ];
    v_flex()
        .w_full()
        .child(browser_nav("x6b-nav", "localhost:3000/pricing").annotating(true).on_action(|_, _, _| {}))
        // The real browser pane composition: the page takes the free space
        // beside the fixed panel, so no pane ground ever shows through.
        .child(
            h_flex()
                .w_full()
                .h(px(560.0))
                .child(page())
                .child(annotations_panel("x6b-annots", annotations).selected(Some(0)).on_action(|_, _, _| {})),
        )
        .into_any_element()
}

fn sample_page() -> DocPage {
    DocPage::new(
        "Request for Proposal: Teacher Recruitment Services",
        "Directorate of Education · Draft v3",
        vec![
            DocBlock::heading(2, "1. Purpose"),
            DocBlock::text("The Directorate invites proposals from qualified agencies for the recruitment of 240 secondary-school teachers."),
            DocBlock::heading(2, "2. Eligibility"),
            DocBlock::text("Bidders must hold a valid registration and demonstrate three years of comparable placements."),
        ],
    )
}

/// Document pane (tabs, toolbar, page) in a bounded-height fluid column.
pub fn build_docs(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let tabs = vec![
        TabItem::new("docx", "RFP-draft-v3.docx", IconName::Doc).closable(false),
        TabItem::new("xlsx", "vendor-scoring.xlsx", IconName::Sheet).closable(false),
    ];
    v_flex()
        .w_full()
        .h(px(600.0))
        .child(doc_tabs("x6b-tabs", tabs, 0))
        // The full toolbar: it wraps below ~460 px so every control stays
        // visible (callers that must hold one line use without_font_select
        // / without_export instead).
        .child(doc_toolbar("x6b-toolbar", "Body text", "Georgia · 11"))
        .child(doc_pane("x6b-doc", sample_page()))
        .into_any_element()
}
