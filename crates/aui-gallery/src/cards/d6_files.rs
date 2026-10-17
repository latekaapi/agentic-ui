//! D6 · Files and the file preview (docs/43 §2.4–2.5; canvas v49 `D6Files`,
//! `D6Preview`, `D6StatesFiles`): the tree (rows 28, fluid hover, selected,
//! the keyboard ring, ignored paths faint, git letters), the filter, a capped
//! folder, and the preview's code view with a revealed band, the Markdown
//! reading view, not shown, gone and loading.

use std::sync::Arc;

use aui::workbench::{
    code_skeleton, code_view, file_tree, filter_field, markdown_view, pane_action, pane_row, pane_row_fade, pushed_row, secondary_action,
    state_block, FileNode, FileTreeAction, GitBadge,
};
use aui_icons::{FileType, IconName};
use aui_tokens::{ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, input::InputState, v_flex};

const W: f32 = 300.0;
const H: f32 = 560.0;

const RUST: &str = r#"/// The merged Changes view (88b): this session's files first,
/// the working tree below, in one list.
fn changes_pane_from(pane_in: &ChangesPane) -> AnyElement {
    let status = pane_in.status;
    let (session_files, other_files) =
        partition_session_files(&status.files, pane_in.session_edits, pane_in.root);
    let diffs: HashMap<&str, &Diff> = pane_in
        .parsed
        .as_ref()
        .map(|parsed| {
            parsed
                .diffs
                .iter()
                .map(|diff| (diff.path.as_str(), diff))
                .collect()
        })
        .unwrap_or_default();
    let mut pane = v_flex()
        .id("right-changes-pane")
        .role(gpui::Role::Group)
        .aria_label("Changes")
        .size_full()
        .overflow_y_scroll();
    pane
}"#;

const MD: &str = "# baaz — notes for an agent working here\n\n## Visual verification is not optional here\n\nA green `cargo test` has never seen the window. Three apps' worth of cosmetic, flow-broken and jank defects reached the owner through a passing gate.\n\n```bash\ncargo build -p baaz\n```\n";

struct Tree {
    collapsed: std::collections::HashSet<SharedString>,
    selected: SharedString,
}

fn screen(label: &str, body: impl IntoElement, row: impl IntoElement, cx: &App) -> Div {
    let p = cx.aui().colors;
    v_flex()
        .gap(px(6.0))
        .child(div().text_px(12.0).text_color(p.fg_muted).child(label.to_string()))
        .child(
            div()
                .relative()
                .w(px(W))
                .h(px(H))
                .rounded(px(8.0))
                .overflow_hidden()
                .bg(p.surface_2)
                .shadow(p.edge())
                .flex()
                .flex_col()
                .child(row)
                .child(div().flex_1().min_h(px(0.0)).child(body))
                .child(pane_row_fade(cx)),
        )
}

pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let tree = window.use_keyed_state("d6-files-tree", cx, |_, _| Tree { collapsed: Default::default(), selected: "crates/baaz/src/right.rs".into() });
    let query = window.use_keyed_state("d6-files-filter", cx, |w, cx| {
        let mut s = InputState::new(w, cx);
        s.set_value("right", w, cx);
        s
    });
    let st = tree.read(cx);
    let open = |id: &str| !st.collapsed.contains(&SharedString::from(id.to_string()));
    let sel = |id: &str| st.selected.as_ref() == id;
    let dir = |id: &str, name: &str, depth: usize| FileNode::dir(id.to_string(), name.to_string(), open(id)).depth(depth);
    let file = |id: &str, name: &str, kind: FileType, depth: usize| FileNode::file(id.to_string(), name.to_string(), kind).depth(depth).selected(sel(id));
    let mut nodes = vec![
        dir(".cargo", ".cargo", 0).ignored(true),
        dir(".claude", ".claude", 0).ignored(true),
        file(".dev.env", ".dev.env", FileType::File, 0).ignored(true),
        dir(".github", ".github", 0),
        file("CLAUDE.md", "CLAUDE.md", FileType::Md, 0),
        file("Cargo.lock", "Cargo.lock", FileType::Lock, 0).badge(GitBadge::M),
        file("Cargo.toml", "Cargo.toml", FileType::File, 0),
        dir("crates", "crates", 0),
    ];
    if open("crates") {
        nodes.push(dir("crates/baaz", "baaz", 1));
        if open("crates/baaz") {
            nodes.push(dir("crates/baaz/src", "src", 2));
            if open("crates/baaz/src") {
                nodes.push(file("crates/baaz/src/layout.rs", "layout.rs", FileType::File, 3).badge(GitBadge::M));
                nodes.push(dir("crates/baaz/src/panes", "panes", 3).badge(GitBadge::A));
                nodes.push(file("crates/baaz/src/right.rs", "right.rs", FileType::File, 3).badge(GitBadge::M));
                nodes.push(file("crates/baaz/src/old.rs", "old.rs", FileType::File, 3).badge(GitBadge::D));
                nodes.push(file("crates/baaz/src/new.rs", "new.rs", FileType::File, 3).badge(GitBadge::U));
            }
        }
    }
    nodes.push(file("uiprobe.json", "uiprobe.json", FileType::Json, 0));
    let _ = st;
    let t = tree.clone();
    let files = screen(
        "Files · hover, selected, letters",
        file_tree("d6-tree", nodes).focused(Some("Cargo.toml".into())).on_action(move |a, _, cx| {
            t.update(cx, |s, cx| {
                match a {
                    FileTreeAction::Toggle(id) => {
                        if !s.collapsed.remove(id) {
                            s.collapsed.insert(id.clone());
                        }
                    }
                    FileTreeAction::Select(id) => s.selected = id.clone(),
                }
                cx.notify();
            })
        }),
        pane_row("d6-files-row", IconName::Folder, "Files").action(pane_action("d6-filter", IconName::Search, "Filter files").toggled(false)).on_close(|_, _, _| {}),
        cx,
    );

    let filtered = vec![
        FileNode::dir("crates", "crates", true),
        FileNode::dir("crates/baaz", "baaz", true).depth(1),
        FileNode::dir("crates/baaz/src", "src", true).depth(2),
        FileNode::file("crates/baaz/src/right.rs", "right.rs", FileType::File).depth(3).badge(GitBadge::M).selected(true),
        FileNode::file("x", "Showing the first 300 of 1 204 in this folder. Filter to find the rest.", FileType::File).depth(0),
    ];
    let mut filtered = filtered;
    filtered[4] = FileNode::note("note", "Showing the first 300 of 1 204 in this folder. Filter to find the rest.", 0);
    let filter = screen(
        "Filter · capped folder line",
        v_flex().size_full().child(filter_field("d6-filter-field", &query, Some("3 of 1,204".into()), window, cx)).child(file_tree("d6-tree-f", filtered)),
        pane_row("d6-files-row-f", IconName::Folder, "Files").action(pane_action("d6-filter-on", IconName::Search, "Filter files").toggled(true)),
        cx,
    );

    let lines: Arc<Vec<String>> = Arc::new(RUST.lines().map(str::to_string).collect());
    let code = screen(
        "Preview · code, revealed",
        code_view("d6-code", lines).language("rust").band(6..9, 1),
        pushed_row("d6-code-row", "right.rs").context("crates/baaz/src").back_label("Back to files").width(px(W)).action(pane_action(
            "d6-code-ext",
            IconName::External,
            "Open in default app",
        )),
        cx,
    );
    let md = screen(
        "Preview · Markdown",
        div().id("d6-md-scroll").size_full().overflow_y_scroll().child(markdown_view("d6-md", MD, cx)),
        pushed_row("d6-md-row", "CLAUDE.md").back_label("Back to files").width(px(W)).action(pane_action("d6-md-ext", IconName::External, "Open in default app")),
        cx,
    );
    let binary = screen(
        "Preview · not shown",
        state_block("d6-bin", IconName::File, "app.db")
            .line("SQLite database · 4.2 MB. Baaz shows text, Markdown and images here.")
            .action(secondary_action("d6-bin-open", "Open in default app", Some(IconName::External), cx)),
        pushed_row("d6-bin-row", "app.db").back_label("Back to files").width(px(W)),
        cx,
    );
    let gone = screen(
        "Preview · gone",
        state_block("d6-gone", IconName::File, "This file is gone").line_with(
            div()
                .child(div().font_family(aui_tokens::scale::FONT_MONO).child("crates/baaz/src/panes/old.rs"))
                .child("was moved or deleted since you opened it."),
        ),
        pushed_row("d6-gone-row", "old.rs").back_label("Back to files").width(px(W)),
        cx,
    );
    let loading = screen(
        "Preview · loading",
        code_skeleton(&[118.0, 92.0, 152.0, 132.0, 66.0, 112.0, 142.0, 74.0, 126.0, 100.0], cx),
        pushed_row("d6-load-row", "main.rs").back_label("Back to files").width(px(W)),
        cx,
    );
    let empty = screen(
        "Files · no project",
        state_block("d6-noproj", IconName::Folder, "No project").line("Open or add a project to see its files."),
        pane_row("d6-files-row-e", IconName::Folder, "Files"),
        cx,
    );

    v_flex()
        .gap(px(20.0))
        .child(h_flex().gap(px(16.0)).items_start().child(files).child(filter).child(code).child(md))
        .child(h_flex().gap(px(16.0)).items_start().child(binary).child(gone).child(loading).child(empty))
        .into_any_element()
}
