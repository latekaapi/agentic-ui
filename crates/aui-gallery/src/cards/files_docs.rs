//! Card 54 · File tree and document panes. Reproduces
//! `design/src/cards/workbench/54-files-docs.html` at 980×600 (body padding 12):
//! the worktree explorer beside the assistant's document pane.

use aui::shell::TabItem;
use aui::workbench::{
    artifact_strip, doc_pane, doc_saved_hint, doc_tabs, doc_toolbar, file_tree, pane_status, pane_status_row, Artifact, ArtifactKind, DocBlock,
    DocCell, DocPage, DocRun, DocTable, FileNode, FileTreeAction, GitBadge,
};
use aui::data::{icon_button, ButtonSize};
use aui_icons::{FileType, IconName};
use aui_tokens::{scale, ActiveAui};
use gpui::*;
use std::collections::HashSet;
use gpui_kit::base::{h_flex, v_flex};

/// `body.ds{padding:12px}` — the gallery adds 20, the card pulls in by 8.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 980.0 - 24.0;
/// `.grid{grid-template-columns:240px 1fr;gap:14px;height:570px}`.
const FRAME_H: f32 = 570.0;
const TREE_W: f32 = 240.0;
const GRID_GAP: f32 = 14.0;
/// Glyphs in the tab band's xs ghost button: 12 px.
const SMALL_GLYPH: f32 = 12.0;

/// What the gallery remembers about the tree: which directories the reader
/// collapsed or re-expanded, and which row they picked.
#[derive(Clone, Default)]
struct TreeState {
    collapsed: HashSet<SharedString>,
    selected: Option<SharedString>,
}

/// The rows of the worktree tree.
fn sample_tree() -> Vec<FileNode> {
    vec![
        FileNode::dir("src", "src", true).badge(GitBadge::M),
        FileNode::dir("src/checkout", "checkout", true).depth(1).badge(GitBadge::M),
        FileNode::file("src/checkout/AddressForm.tsx", "AddressForm.tsx", FileType::Tsx).depth(2).badge(GitBadge::M),
        FileNode::file("src/checkout/validators.ts", "validators.ts", FileType::Ts).depth(2).badge(GitBadge::M).selected(true),
        FileNode::file("src/checkout/validators.test.ts", "validators.test.ts", FileType::Test).depth(2).badge(GitBadge::A),
        FileNode::file("src/checkout/index.ts", "index.ts", FileType::Ts).depth(2),
        FileNode::dir("src/components", "components", false).depth(1),
        FileNode::dir("src/lib", "lib", false).depth(1),
        FileNode::dir("tests", "tests", false),
        FileNode::file("package.json", "package.json", FileType::Json),
        FileNode::file("pnpm-lock.yaml", "pnpm-lock.yaml", FileType::Lock),
        FileNode::file("PLAN.md", "PLAN.md", FileType::Md).badge(GitBadge::U),
        FileNode::file("README.md", "README.md", FileType::Md),
    ]
}

/// The page the document pane shows.
fn sample_page() -> DocPage {
    DocPage::new(
        "Request for Proposal: Teacher Recruitment Services",
        "Directorate of Education · Draft v3 · 5 September 2026",
        vec![
            DocBlock::Paragraph(vec![
                DocRun::bold("1. Purpose."),
                DocRun::text(
                    " The Directorate invites proposals from qualified agencies for the recruitment of 240 secondary-school teachers across 38 institutions for the 2027 academic year.",
                ),
            ]),
            DocBlock::Paragraph(vec![
                DocRun::bold("2. Eligibility."),
                DocRun::text(" Bidders must hold a valid registration under the "),
                DocRun::changed("Procurement Rules 2019, Rule 14(2)"),
                DocRun::text(" and demonstrate three years of comparable placements."),
            ]),
            DocBlock::Paragraph(vec![DocRun::bold("3. Scope of services.")]),
            DocBlock::list([
                "Sourcing and screening against the qualification matrix in Annex A.",
                "Document verification in line with GO 2024-18.",
                "Onboarding support through the first term.",
            ]),
            DocBlock::heading(2, "4. Island-wise position"),
            DocBlock::Rule,
            DocBlock::Table(DocTable {
                widths: vec![0.658, 0.342],
                rows: vec![
                    vec![DocCell { col_span: 2, ..DocCell::header("ANDROTH Island at a glance") }],
                    vec![DocCell::text("Total geographical Area"), DocCell::text("4.90 sq.Kms")],
                    vec![DocCell::text("Maximum Length"), DocCell::text("4.66 km")],
                    vec![DocCell::text("Total No. of Schools"), DocCell::text("6")],
                ],
            }),
            DocBlock::Table(DocTable {
                widths: vec![],
                rows: vec![
                    vec![
                        DocCell::header("Sl No"),
                        DocCell::header("Name of School"),
                        DocCell::header("Class Range"),
                        DocCell::header("Rooms"),
                        DocCell { col_span: 2, ..DocCell::header("Students") },
                        DocCell::header("Ratio"),
                    ],
                    vec![
                        DocCell { header: true, ..DocCell::text("") },
                        DocCell { header: true, ..DocCell::text("") },
                        DocCell { header: true, ..DocCell::text("") },
                        DocCell { header: true, ..DocCell::text("") },
                        DocCell::header("Male"),
                        DocCell::header("Female"),
                        DocCell::header("Total"),
                        DocCell { header: true, ..DocCell::text("") },
                    ],
                    vec![
                        DocCell::text("1"),
                        DocCell::text("Govt. Model School, Androth"),
                        DocCell::text("VI–XII"),
                        DocCell::text("12"),
                        DocCell::text("85"),
                        DocCell::text("79"),
                        DocCell::text("164"),
                        DocCell::text("1:27"),
                    ],
                    vec![
                        DocCell::text("2"),
                        DocCell::text("Govt. High School, Kalpeni"),
                        DocCell::text("VI–X"),
                        DocCell::text("9"),
                        DocCell::text("62"),
                        DocCell::text("58"),
                        DocCell::text("120"),
                        DocCell::text("1:24"),
                    ],
                ],
            }),
        ],
    )
}

/// The artifacts the chat created.
fn sample_artifacts() -> Vec<Artifact> {
    vec![
        Artifact::new("RFP-draft-v3.docx", ArtifactKind::Doc).version("v3").active(true),
        Artifact::new("vendor-scoring.xlsx", ArtifactKind::Sheet).version("v1"),
        Artifact::new("eligibility-notes.md", ArtifactKind::Note),
        Artifact::new("org-chart.png", ArtifactKind::Image),
    ]
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let active = window.use_keyed_state("card54-tab", cx, |_, _| 0usize);
    let current = *active.read(cx);
    let tabs = vec![
        TabItem::new("docx", "RFP-draft-v3.docx", IconName::Doc).closable(false),
        TabItem::new("xlsx", "vendor-scoring.xlsx", IconName::Sheet).closable(false),
        TabItem::new("pdf", "GO-2024-18.pdf", IconName::Pdf).closable(false),
    ];
    let ids = ["docx", "xlsx", "pdf"];
    let select = active.clone();

    // `.pan{border:1px solid var(--line-strong);border-radius:12px;background:var(--surface-1)}`.
    let panel = |el: Div| {
        el.rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.surface_1)
            .overflow_hidden()
    };

    // The tree's expansion and selection live in the gallery so the chevron
    // spring and the selected row can be exercised by hand.
    let tree_state = window.use_keyed_state("card54-tree-state", cx, |_, _| TreeState::default());
    let state = tree_state.read(cx).clone();
    let mut nodes = sample_tree();
    for node in &mut nodes {
        if let (true, Some(open)) = (state.collapsed.contains(&node.id), node.open) {
            node.open = Some(!open);
            node.kind = if !open { FileType::FolderOpen } else { FileType::Folder };
        }
        node.selected = state.selected.as_ref() == Some(&node.id) || (state.selected.is_none() && node.selected);
    }
    // A collapsed directory hides the rows beneath it (ids are paths).
    let closed: Vec<SharedString> = nodes.iter().filter(|n| n.open == Some(false)).map(|n| n.id.clone()).collect();
    nodes.retain(|n| !closed.iter().any(|dir| n.id.starts_with(&format!("{dir}/"))));
    let tree = file_tree("card54-tree", nodes)
        .header("acme-web")
        .footer("worktree checkout-flow-v2 · 2 modified · 1 added")
        .on_action(move |action, _, cx| {
            let action = action.clone();
            tree_state.update(cx, |s, cx| {
                match action {
                    FileTreeAction::Select(id) => s.selected = Some(id),
                    FileTreeAction::Toggle(id) => {
                        if !s.collapsed.remove(&id) {
                            s.collapsed.insert(id);
                        }
                    }
                    FileTreeAction::Search | FileTreeAction::Refresh => {}
                }
                cx.notify();
            })
        });

    let band = doc_tabs("card54-tabs", tabs, current)
        .on_select(move |id, _, cx| {
            let i = ids.iter().position(|t| *t == id.as_ref()).unwrap_or(0);
            select.update(cx, |v, cx| {
                *v = i;
                cx.notify();
            })
        })
        .trailing(doc_saved_hint("Saved", cx))
        .trailing(icon_button("card54-more", IconName::Dots).ghost().size(ButtonSize::Xs).icon_size(px(SMALL_GLYPH)));

    div()
        .w(px(FRAME_W))
        .h(px(FRAME_H))
        .flex_none()
        .m(px(FRAME_INSET))
        .child(
            h_flex()
                .size_full()
                .gap(px(GRID_GAP))
                .items_stretch()
                .child(panel(div().flex_none().w(px(TREE_W)).h_full()).child(tree))
                .child(
                    panel(v_flex().flex_1().min_w(px(0.0)).h_full())
                        .child(band)
                        .child(doc_toolbar("card54-toolbar", "Body text", "Georgia · 11"))
                        .child(doc_pane("card54-doc", sample_page()))
                        .child(artifact_strip("card54-arts", sample_artifacts()))
                        .child(pane_status_row(
                            "card54-status",
                            vec![pane_status("Page 1 of 4"), pane_status("·"), pane_status("1,214 words")],
                            vec![pane_status("1 change from chat highlighted").accent()],
                        )),
                ),
        )
        .into_any_element()
}
