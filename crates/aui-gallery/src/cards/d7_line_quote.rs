//! Quote lines from the pane (docs/82 7.1): line selection in the Changes
//! diff and the file preview, the selection bar hung over the selected
//! lines, the composer's quote carrying the path and line range, and the
//! sent bubble naming where the lines came from. Live: press a line, drag
//! or shift-click to extend; Quote clears the selection here.

use std::ops::Range;
use std::sync::Arc;

use aui::composer::{composer, composer_state_rows, ComposerQuote};
use aui::protocol::{Attachment, AttachmentKind, Diff, DiffKind, DiffLine, Hunk, UploadState};
use aui::transcript::{quote_meta, user_turn};
use aui::util::ElementExt as _;
use aui::workbench::{code_view, diff_body, line_quote_bar, LineSelectGeometry};
use aui_icons::Provider;
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::{h_flex, v_flex};

const RUST: &str = "pub fn total(a: u32, b: u32) -> u32 {\n    a.saturating_add(b)\n}\n\n/// Cents as `d.cc`.\npub fn cents(v: u32) -> String {\n    format!(\"{}.{:02}\", v / 100, v % 100)\n}\n";

fn dl(kind: DiffKind, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
    DiffLine { kind, old_no: old, new_no: new, text: text.into() }
}

fn lib_diff() -> Arc<Diff> {
    use DiffKind::*;
    Arc::new(Diff {
        path: "src/lib.rs".into(),
        hunks: vec![Hunk {
            header: "@@ -1,3 +1,8 @@".into(),
            lines: vec![
                dl(Context, Some(1), Some(1), "pub fn total(a: u32, b: u32) -> u32 {"),
                dl(Del, Some(2), None, "    a + b"),
                dl(Add, None, Some(2), "    a.saturating_add(b)"),
                dl(Context, Some(3), Some(3), "}"),
                dl(Add, None, Some(4), ""),
                dl(Add, None, Some(5), "/// Cents as `d.cc`."),
                dl(Add, None, Some(6), "pub fn cents(v: u32) -> String {"),
                dl(Add, None, Some(7), "    format!(\"{}.{:02}\", v / 100, v % 100)"),
                dl(Add, None, Some(8), "}"),
            ],
        }],
        added: 6,
        removed: 1,
    })
}

/// One live sample: the view, its selection, the body's bounds and the bar.
#[derive(Default)]
struct Sample {
    selected: Option<Range<usize>>,
    body: Option<Bounds<Pixels>>,
}

fn panel(label: &'static str, view: AnyElement, bar: Option<AnyElement>, state: Entity<Sample>, h: f32, cx: &App) -> Div {
    let p = cx.aui().colors;
    v_flex()
        .gap(px(6.0))
        .child(div().role_size(TextRole::Caption).text_color(p.fg_muted).child(label))
        .child(
            div()
                .relative()
                .w(px(400.0))
                .h(px(h))
                .bg(p.surface_2)
                .border_1()
                .border_color(p.line)
                .rounded(px(8.0))
                .overflow_hidden()
                .on_prepaint(move |bounds, _, cx| {
                    state.update(cx, |s, _| s.body = Some(bounds));
                })
                .child(view)
                .children(bar),
        )
}

pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;

    // The diff, two lines held (the removed line and its replacement).
    let diff_state = window.use_keyed_state("d7-diff-sel", cx, |_, _| Sample { selected: Some(1..3), body: None });
    let diff_geom = window.use_keyed_state("d7-diff-geom", cx, |_, _| LineSelectGeometry::default()).read(cx).clone();
    let (sel, body) = { let s = diff_state.read(cx); (s.selected.clone(), s.body) };
    let set = diff_state.clone();
    let diff = diff_body("d7-diff", lib_diff()).selected(sel.clone()).selection_geometry(&diff_geom).on_select_lines(move |r, _, cx| {
        set.update(cx, |s, cx| {
            s.selected = Some(r);
            cx.notify();
        })
    });
    let clear = diff_state.clone();
    let diff_bar = line_quote_bar("d7-diff-bar", sel.is_some(), &diff_geom, body, move |_, _, cx| clear.update(cx, |s, cx| {
        s.selected = None;
        cx.notify();
    }), window, cx);
    let diff_view = div().pt(px(48.0)).px(px(12.0)).child(diff).into_any_element();

    // The preview, lines 6–8 held.
    let code_state = window.use_keyed_state("d7-code-sel", cx, |_, _| Sample { selected: Some(5..8), body: None });
    let code_geom = window.use_keyed_state("d7-code-geom", cx, |_, _| LineSelectGeometry::default()).read(cx).clone();
    let (sel, body) = { let s = code_state.read(cx); (s.selected.clone(), s.body) };
    let lines: Arc<Vec<String>> = Arc::new(RUST.lines().map(str::to_string).collect());
    let set = code_state.clone();
    let code = code_view("d7-code", lines).language("rust").selected(sel.clone()).selection_geometry(&code_geom).on_select_lines(move |r, _, cx| {
        set.update(cx, |s, cx| {
            s.selected = Some(r);
            cx.notify();
        })
    });
    let clear = code_state.clone();
    let code_bar = line_quote_bar("d7-code-bar", sel.is_some(), &code_geom, body, move |_, _, cx| clear.update(cx, |s, cx| {
        s.selected = None;
        cx.notify();
    }), window, cx);

    // The composer holding a quote from each, and the sent bubble.
    let field = window.use_keyed_state("d7-q-field", cx, |w, cx| {
        let mut s = composer_state_rows("Ask Muse, or type / for commands", 1, 8, w, cx);
        s.set_value("Does cents need the same guard?", w, cx);
        s
    });
    let composer = composer("d7-q-composer", &field, Provider::Muse, "Muse Spark 1.3")
        .docked(true)
        .raised(true)
        .mode("Auto")
        .effort("Default")
        .can_send(true)
        .quotes(vec![
            ComposerQuote {
                id: "q1".into(),
                text: "pub fn cents(v: u32) -> String {\n    format!(\"{}.{:02}\", v / 100, v % 100)\n}".into(),
                code: Some(("rust".into(), 3)),
                location: Some("src/lib.rs:6–8".into()),
            },
            ComposerQuote { id: "q2".into(), text: "-    a + b\n+    a.saturating_add(b)".into(), code: Some(("diff".into(), 2)), location: Some("src/lib.rs:2".into()) },
        ]);
    let sent = user_turn("d7-q-sent", "Does cents need the same guard?").attachments(vec![Attachment {
        name: "pub fn cents(v: u32) -> String {\n    format!(\"{}.{:02}\", v / 100, v % 100)\n}".into(),
        kind: AttachmentKind::Quote,
        size_bytes: None,
        meta: Some(quote_meta("rust", Some("src/lib.rs:6–8"))),
        state: UploadState::Ready,
        path: None,
        quote_of: None,
    }]);

    let label = |t: &'static str| div().role_size(TextRole::Caption).text_color(p.fg_muted).child(t);
    v_flex()
        .gap(px(20.0))
        .child(
            h_flex()
                .gap(px(20.0))
                .items_start()
                .child(panel("Changes · two lines selected", diff_view, diff_bar, diff_state, 280.0, cx))
                .child(panel("Preview · lines 6–8 selected", code.into_any_element(), code_bar, code_state, 280.0, cx)),
        )
        .child(label("Quotes in the composer"))
        .child(div().w(px(820.0)).child(composer))
        .child(label("Sent"))
        .child(h_flex().child(div().w(px(560.0)).child(sent)))
        .into_any_element()
}
