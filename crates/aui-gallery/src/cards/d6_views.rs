//! D6 · the views the transcript opens (docs/43 §2.9–2.10; canvas v49
//! `D6Output`, `D6Image`, `D6StatesViews`): the full output (done with a
//! non-zero exit and the cap footer, still running, fetching) and the image
//! view (fitted, not available). Click the image to toggle fit ↔ 100 %.

use std::sync::Arc;

use aui::workbench::{image_missing, image_view, output_fetching, output_title, output_view, pane_action, pushed_row, pushed_row_with};
use aui_icons::IconName;
use aui_tokens::{ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

const W: f32 = 400.0;
const H: f32 = 600.0;

const OUTPUT: &str = "$ cargo test --workspace\n   Compiling baaz v0.1.0 (crates/baaz)\n    Finished `test` profile [unoptimized + debuginfo] target(s) in 41.2s\n     Running unittests src/main.rs\n\nrunning 1 204 tests\ntest right::tests::a_binary_file_diff_parses_without_hunks ... \u{1b}[32mok\u{1b}[0m\ntest right::tests::session_files_come_first_in_edit_order ... \u{1b}[32mok\u{1b}[0m\ntest right::tests::the_changes_headings_name_their_counts ... \u{1b}[31mFAILED\u{1b}[0m\ntest layout::tests::legacy_kinds_normalise_to_changes ... \u{1b}[32mok\u{1b}[0m\n\nfailures:\n\n---- right::tests::the_changes_headings_name_their_counts stdout ----\nthread 'main' panicked at crates/baaz/src/right.rs:2931:9:\nassertion `left == right` failed\n  left: \"Other changes · 5\"\n right: \"Other changes\"\n\n\u{1b}[2mnote: run with `RUST_BACKTRACE=1` for a backtrace\u{1b}[0m\n\ntest result: \u{1b}[31mFAILED\u{1b}[0m. 1 202 passed; 1 failed; 1 ignored";

fn frame(label: &str, row: AnyElement, body: AnyElement, cx: &App) -> Div {
    let p = cx.aui().colors;
    v_flex().gap(px(6.0)).child(div().text_px(12.0).text_color(p.fg_muted).child(label.to_string())).child(
        div().w(px(W)).h(px(H)).rounded(px(8.0)).overflow_hidden().bg(p.surface_2).shadow(p.edge()).flex().flex_col().child(row).child(div().flex_1().min_h(px(0.0)).child(body)),
    )
}

pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let actual = window.use_keyed_state("d6-image-actual", cx, |_, _| false);
    let lines: Arc<Vec<String>> = Arc::new(OUTPUT.lines().map(str::to_string).collect());
    let copy = || pane_action("d6-out-copy", IconName::Copy, "Copy the output");
    let done = frame(
        "Full output",
        pushed_row_with("d6-out-row", output_title("cargo test --workspace", Some(101), Some("43.8 s".into()), None, cx))
            .back_label("Back")
            .width(px(W))
            .action(copy())
            .on_close(|_, _, _| {})
            .into_any_element(),
        output_view("d6-out", lines.clone()).capped(Some(1204)).into_any_element(),
        cx,
    );
    let running = frame(
        "Still running",
        pushed_row_with("d6-run-row", output_title("cargo test --workspace", None, None, Some(12), cx)).width(px(W)).action(copy()).into_any_element(),
        output_view("d6-run", Arc::new(lines[..8].to_vec())).follow(true).into_any_element(),
        cx,
    );
    let fetching = frame(
        "Fetching",
        pushed_row_with("d6-fetch-row", output_title("cargo build --workspace", None, None, None, cx)).width(px(W)).action(pane_action("d6-fetch-copy", IconName::Copy, "Copy the output").disabled(true)).into_any_element(),
        output_fetching(cx).into_any_element(),
        cx,
    );

    let path = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/images/minimal.png"));
    let is_actual = *actual.read(cx);
    let a = actual.clone();
    let image = frame(
        "Image (click: fit ↔ 100 %)",
        pushed_row("d6-img-row", "minimal.png").back_label("Back").width(px(W)).action(pane_action("d6-img-ext", IconName::External, "Open in default app")).into_any_element(),
        image_view("d6-img", path, "1440 × 900 · PNG · 312 KB", Some((1440.0, 900.0)), is_actual, window, cx)
            .on_click(move |_, _, cx| a.update(cx, |v, cx| {
                *v = !*v;
                cx.notify();
            }))
            .into_any_element(),
        cx,
    );
    let missing = frame(
        "Image not available",
        pushed_row("d6-miss-row", "capture-peek.png").back_label("Back").width(px(W)).into_any_element(),
        image_missing("d6-miss", "capture-peek.png", cx).into_any_element(),
        cx,
    );

    v_flex()
        .gap(px(20.0))
        .child(h_flex().gap(px(16.0)).items_start().child(done).child(running).child(fetching))
        .child(h_flex().gap(px(16.0)).items_start().child(image).child(missing))
        .into_any_element()
}
