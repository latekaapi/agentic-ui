//! D6 · Changes, commit and PR (docs/43 §2.6; canvas v49 `D6Changes`,
//! `D6StatesChanges`, `D6PRForm`). The first screen is live: click a row to
//! fold its diff open on D4's motion, click "24 unchanged lines" to expand it.

use std::sync::Arc;

use aui::interact::{fluid_list, Axis};
use aui::protocol::{ChangeKind, Diff, DiffKind, DiffLine, FileChange, Hunk};
use aui::workbench::{
    base_and_head, base_picker, branch_label, change_row, change_with_diff, changes_caption, commit_foot, commit_frame, commit_message_view,
    diff_body, draft_checkbox, form_field, hook_failed_note, pane_row, pane_row_fade, pr_card, primary_action, pushed_row, skeleton_bar,
    state_block, text_action, well_input, PrState,
};
use aui_icons::IconName;
use aui_tokens::{ActiveAui, AuiStyled};
use gpui::*;
use gpui::prelude::FluentBuilder as _;
use gpui_kit::base::{h_flex, v_flex};

const W: f32 = 400.0;
const H: f32 = 760.0;

fn dl(kind: DiffKind, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
    DiffLine { kind, old_no: old, new_no: new, text: text.into() }
}

fn right_diff() -> Arc<Diff> {
    use DiffKind::*;
    Arc::new(Diff {
        path: "crates/baaz/src/right.rs".into(),
        hunks: vec![
            Hunk {
                header: "@@ -2820,10 +2820,12 @@ fn changes_heading".into(),
                lines: vec![
                    dl(Context, Some(2820), Some(2820), "fn changes_heading(id: &'static str, label: SharedString)"),
                    dl(Context, Some(2821), Some(2821), "    div()"),
                    dl(Context, Some(2822), Some(2822), "        .id(id)"),
                    dl(Del, Some(2823), None, "        .px(px(16.0))"),
                    dl(Del, Some(2824), None, "        .pt(px(12.0))"),
                    dl(Add, None, Some(2823), "        .h(px(32.0))"),
                    dl(Add, None, Some(2824), "        .px(px(8.0))"),
                    dl(Add, None, Some(2825), "        .text_color(cx.theme().muted)"),
                    dl(Add, None, Some(2826), "        .text_size(rems(TEXT_CAPTION))"),
                    dl(Context, Some(2825), Some(2827), "        .child(label)"),
                ],
            },
            Hunk {
                header: "@@ -2853,7 +2879,9 @@ fn session_file_row".into(),
                lines: vec![
                    dl(Context, Some(2853), Some(2879), "fn session_file_row("),
                    dl(Context, Some(2854), Some(2880), "    change: &FileChange,"),
                    dl(Del, Some(2855), None, "    open: bool,"),
                    dl(Add, None, Some(2881), "    open: bool, hovered: bool,"),
                    dl(Add, None, Some(2882), "    theme: &Theme,"),
                ],
            },
        ],
        added: 148,
        removed: 12,
    })
}

fn fc(path: &str, change: ChangeKind, added: u32, removed: u32) -> FileChange {
    FileChange { path: path.into(), change, added, removed }
}

const MESSAGE: &str = "Merge Diff and Git into one Changes view\nSession files on top, the rest of the tree below; commit at the foot.";

#[derive(Clone, Copy)]
struct Live {
    open: Option<usize>,
    gap: bool,
}

fn screen(label: &str, body: AnyElement, foot: Option<AnyElement>, row: AnyElement, h: f32, cx: &App) -> Div {
    let p = cx.aui().colors;
    v_flex().gap(px(6.0)).child(div().text_px(12.0).text_color(p.fg_muted).child(label.to_string())).child(
        div()
            .relative()
            .w(px(W))
            .h(px(h))
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(p.surface_2)
            .shadow(p.edge())
            .flex()
            .flex_col()
            .child(row)
            .child(div().flex_1().min_h(px(0.0)).child(body))
            .child(pane_row_fade(cx))
            .when_some(foot, |d, f| d.child(commit_foot(f, cx))),
    )
}

fn changes_row(id: &str, cx: &App) -> AnyElement {
    let _ = cx;
    pane_row(SharedString::from(id.to_string()), IconName::Git, "Changes").on_close(|_, _, _| {}).into_any_element()
}

fn commit_dirty(id: &str, focused: bool, busy: bool, cx: &App) -> AnyElement {
    commit_frame(focused, cx)
        .child(commit_message_view(SharedString::from(format!("{id}-msg")), MESSAGE, cx))
        .child(
            h_flex()
                .h(px(28.0))
                .gap(px(6.0))
                .items_center()
                .child(branch_label("right-pane", Some("↑2".into()), cx))
                .child(div().flex_1())
                .child(primary_action(SharedString::from(format!("{id}-commit")), if busy { "Committing…" } else { "Commit 5 files" }, None, busy, false, cx)),
        )
        .into_any_element()
}

pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let live = window.use_keyed_state("d6-changes-live", cx, |_, _| Live { open: Some(0), gap: false });
    let st = *live.read(cx);
    let p = cx.aui().colors;
    let session = [fc("crates/baaz/src/right.rs", ChangeKind::Modified, 148, 12), fc("crates/baaz/src/layout.rs", ChangeKind::Modified, 31, 4), fc("crates/baaz/src/panes/mod.rs", ChangeKind::Added, 64, 0)];
    let other = [fc("docs/02-app.md", ChangeKind::Modified, 22, 9), fc("scripts/old_probe.py", ChangeKind::Deleted, 0, 37)];

    // The live screen: captions + rows on one fluid list, the diff under the open row.
    let mut list = fluid_list("d6-chg-list", Axis::Y).gap(px(0.0)).item_static(0, changes_caption("d6-cap-s", "This session", Some(3), true, cx));
    let mut ix = 1;
    for (i, change) in session.iter().chain(other.iter()).enumerate() {
        if i == 3 {
            list = list.item_static(ix, changes_caption("d6-cap-o", "Other changes", Some(2), false, cx));
            ix += 1;
        }
        let open = st.open == Some(i);
        let l = live.clone();
        let row = change_row(("d6-chg", i), change.clone(), open).width(px(W)).on_toggle(move |_, _, cx| {
            l.update(cx, |s, cx| {
                s.open = if s.open == Some(i) { None } else { Some(i) };
                cx.notify();
            })
        });
        let row = row.on_open_file(|_, _, _| {});
        let l2 = live.clone();
        let gap_open = st.gap;
        let el = change_with_diff(
            ("d6-chg-fold", i),
            row,
            open,
            move || {
                let mut body = diff_body(("d6-diff", i), right_diff()).on_gap(move |_, _, cx| {
                    l2.update(cx, |s, cx| {
                        s.gap = !s.gap;
                        cx.notify();
                    })
                });
                if gap_open {
                    body = body.open_gap(0, Arc::new((2828..2879).map(|n| format!("        // line {n}")).collect()));
                }
                body.into_any_element()
            },
            window,
            cx,
        );
        list = list.item(ix, el);
        ix += 1;
    }
    let live_body = div().id("d6-chg-scroll").size_full().overflow_y_scroll().pt(px(4.0)).px(px(6.0)).pb(px(160.0)).child(list).into_any_element();
    let s1 = screen("Changes · live (click rows)", live_body, Some(commit_dirty("d6-c1", false, false, cx)), changes_row("d6-r1", cx), H, cx);

    // Foot states.
    let rows_only = |id: &str| {
        let mut col = v_flex().pt(px(4.0)).px(px(6.0)).child(changes_caption(SharedString::from(format!("{id}-cap")), "This session", Some(3), true, cx));
        for (i, c) in session.iter().enumerate() {
            col = col.child(change_row((SharedString::from(id.to_string()), i), c.clone(), false).width(px(W)));
        }
        col.into_any_element()
    };
    let failed = v_flex()
        .child(hook_failed_note(
            "d6-hook",
            "pre-commit hook failed",
            &["cargo clippy --all-targets -- -D warnings".into(), "error: unused variable: `hovered` (right.rs:2881)".into()],
            vec![text_action("d6-hook-out", "Show full output", None, 24.0, cx).into_any_element(), text_action("d6-hook-ask", "Ask Muse to fix it", None, 24.0, cx).into_any_element()],
            cx,
        ))
        .child(commit_dirty("d6-c3", false, false, cx))
        .into_any_element();
    let s2 = screen("A hook failed", rows_only("d6-r2rows"), Some(failed), changes_row("d6-r2", cx), 420.0, cx);
    let empty_msg = commit_frame(true, cx)
        .child(commit_message_view("d6-c4-msg", "", cx))
        .child(h_flex().h(px(28.0)).items_center().child(branch_label("right-pane", None, cx)).child(div().flex_1()).child(primary_action("d6-c4-commit", "Commit 5 files", None, false, true, cx)))
        .into_any_element();
    let s3 = screen("Committing · empty message", rows_only("d6-r3rows"), Some(v_flex().gap(px(8.0)).child(commit_dirty("d6-c5", false, true, cx)).child(empty_msg).into_any_element()), changes_row("d6-r3", cx), 420.0, cx);

    let clean = |id: &str| {
        div().size_full().pb(px(60.0)).child(state_block(SharedString::from(format!("{id}-st")), IconName::Check, "No changes").line("The working tree is clean.")).into_any_element()
    };
    let ahead = commit_frame(false, cx)
        .child(h_flex().h(px(28.0)).items_center().child(branch_label("right-pane", Some("· 2 commits not pushed".into()), cx)).child(div().flex_1()).child(primary_action("d6-push", "Push", None, false, false, cx)))
        .into_any_element();
    let s4 = screen("Clean, not pushed", clean("d6-clean1"), Some(ahead), changes_row("d6-r4", cx), 300.0, cx);
    let pushed = commit_frame(false, cx)
        .child(h_flex().h(px(28.0)).items_center().child(branch_label("right-pane", Some("· up to date".into()), cx)).child(div().flex_1()).child(primary_action("d6-cpr", "Create PR", Some(IconName::PullRequest), false, false, cx)))
        .into_any_element();
    let s5 = screen("Pushed, no PR", clean("d6-clean2"), Some(pushed), changes_row("d6-r5", cx), 300.0, cx);
    let pr = pr_card("d6-pr", "Merge Diff and Git into one Changes view", 128, PrState::Open, Some((3, 4, true)), text_action("d6-pr-view", "View", Some(IconName::External), 24.0, cx), cx).into_any_element();
    let s6 = screen("PR open", clean("d6-clean3"), Some(pr), changes_row("d6-r6", cx), 300.0, cx);

    let form = v_flex()
        .p(px(12.0))
        .gap(px(12.0))
        .child(form_field("Into", base_and_head(base_picker("d6-base", "main", cx), "right-pane", cx), cx))
        .child(form_field("Title", well_input("Merge Diff and Git into one Changes view", 32.0, true, cx), cx))
        .child(form_field(
            "Description",
            well_input(
                v_flex()
                    .child("Changes is one view now: this session’s files on top, the rest of the working tree below, and commit, push and PR at its foot.")
                    .child(div().mt(px(8.0)).text_color(p.fg_muted).child("Drafted from this session · edit freely")),
                150.0,
                false,
                cx,
            ),
            cx,
        ))
        .child(draft_checkbox("d6-draft", false, cx))
        .into_any_element();
    let form_foot = h_flex()
        .justify_end()
        .gap(px(6.0))
        .px(px(4.0))
        .child(text_action("d6-cancel", "Cancel", None, 28.0, cx))
        .child(primary_action("d6-create", "Create PR", Some(IconName::PullRequest), false, false, cx))
        .into_any_element();
    let s7 = screen(
        "Create PR",
        form,
        Some(form_foot),
        pushed_row("d6-pr-row", "Create pull request").back_label("Back to changes").width(px(W)).on_close(|_, _, _| {}).into_any_element(),
        H,
        cx,
    );
    let loading = v_flex()
        .pt(px(4.0))
        .px(px(6.0))
        .child(changes_caption("d6-load-cap", "This session", None, true, cx))
        .children([90.0, 120.0, 70.0, 110.0, 84.0].iter().map(|w| {
            h_flex().h(px(32.0)).gap(px(10.0)).pl(px(24.0)).pr(px(8.0)).items_center().child(skeleton_bar(px(10.0), cx)).child(skeleton_bar(px(*w), cx)).child(div().flex_1()).child(skeleton_bar(px(44.0), cx))
        }))
        .into_any_element();
    let s8 = screen("Loading", loading, None, changes_row("d6-r8", cx), 300.0, cx);
    let s9 = screen(
        "Not a git repository",
        state_block("d6-norepo", IconName::Git, "Not a git repository").line("Changes needs git. Files still shows the project.").into_any_element(),
        None,
        changes_row("d6-r9", cx),
        300.0,
        cx,
    );

    v_flex()
        .gap(px(20.0))
        .child(h_flex().gap(px(16.0)).items_start().child(s1).child(s7).child(v_flex().gap(px(20.0)).child(s2).child(s3)))
        .child(h_flex().gap(px(16.0)).items_start().child(s4).child(s5).child(s6))
        .child(h_flex().gap(px(16.0)).items_start().child(s8).child(s9))
        .into_any_element()
}
