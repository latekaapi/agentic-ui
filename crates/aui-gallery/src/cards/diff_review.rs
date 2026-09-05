//! Card 52 · Diff review. Reproduces
//! `design/src/cards/workbench/52-diff-review.html` at 900×640 (body padding 12).

use aui::protocol::{ChangeKind, Diff, DiffKind, DiffLine, FileChange, Hunk};
use aui::workbench::{diff_review, DiffHighlight, DiffScope, DiffView, ReviewFile, ReviewNote};
use gpui::*;
use gpui_kit::base::v_flex;

/// `body.ds{padding:12px}` — the gallery frame adds 20, so the card pulls in by 8
/// and the pane spans the card width less the 24 px of design padding.
const FRAME_INSET: f32 = -8.0;
const FRAME_W: f32 = 900.0 - 24.0;

/// The reviewed file.
const PATH: &str = "src/checkout/validators.ts";

fn line(kind: DiffKind, old_no: Option<u32>, new_no: Option<u32>, text: &str) -> DiffLine {
    DiffLine { kind, old_no, new_no, text: text.into() }
}

fn file(path: &str, change: ChangeKind, added: u32, removed: u32, notes: usize, selected: bool) -> ReviewFile {
    ReviewFile { change: FileChange { path: path.into(), change, added, removed }, notes, selected }
}

/// The two hunks of `validators.ts` the card shows.
fn sample_diff() -> Diff {
    Diff {
        path: PATH.into(),
        hunks: vec![
            Hunk {
                header: "@@ -42,7 +42,9 @@ export function validateAddress".into(),
                lines: vec![
                    line(DiffKind::Context, Some(44), Some(44), "export function validateAddress(values: AddressValues) {"),
                    line(DiffKind::Del, Some(45), None, "  if (!values.country) return true;"),
                    line(DiffKind::Add, None, Some(45), "  if (!values.country) return { ok: false, field: 'country' };"),
                    line(DiffKind::Add, None, Some(46), "  if (values.country === 'CA') return validateCanadianPostal(values);"),
                    line(DiffKind::Context, Some(46), Some(47), "  // US ZIP or ZIP+4"),
                    line(DiffKind::Add, None, Some(48), "  return { ok: ZIP.test(values.postal), field: 'postal' };"),
                ],
            },
            Hunk {
                header: "@@ -88,3 +90,4 @@ function backfillUsers(rows)".into(),
                lines: vec![
                    line(DiffKind::Context, Some(88), Some(90), "  for (const row of rows) {"),
                    line(DiffKind::Del, Some(89), None, "    db.exec(sql, row)"),
                    line(DiffKind::Add, None, Some(91), "    if (!row.tier) continue"),
                ],
            },
        ],
        added: 8,
        removed: 3,
    }
}

/// `.hd2` on the deleted tail and `.hl` on the added one, both after the
/// unchanged `  if (!values.country) `.
fn sample_highlights() -> Vec<DiffHighlight> {
    let prefix = "  if (!values.country) ".len();
    vec![
        DiffHighlight { hunk: 0, line: 1, start: prefix, end: "  if (!values.country) return true;".len() },
        DiffHighlight { hunk: 0, line: 2, start: prefix, end: "  if (!values.country) return { ok: false, field: 'country' };".len() },
    ]
}

/// Builds the card content.
pub fn build(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let files = vec![
        file(PATH, ChangeKind::Modified, 8, 3, 2, true),
        file("src/checkout/validators.test.ts", ChangeKind::Added, 41, 0, 0, false),
        file("src/checkout/AddressForm.tsx", ChangeKind::Modified, 2, 1, 0, false),
        file("src/checkout/legacy-zip.ts", ChangeKind::Deleted, 0, 3, 0, false),
    ];
    let notes = vec![
        ReviewNote {
            file: PATH.into(),
            line: 46,
            text: "Also handle 'GB' here, postcode format differs.".into(),
            summary: Some("also handle GB".into()),
            pending: false,
        },
        ReviewNote {
            file: PATH.into(),
            line: 91,
            text: "Silently skipping rows — log the count or surface it.".into(),
            summary: Some("log skipped rows".into()),
            pending: true,
        },
    ];
    v_flex()
        .w(px(FRAME_W))
        .m(px(FRAME_INSET))
        .child(
            diff_review("card52", files, sample_diff(), notes, DiffScope::ThisTurn, DiffView::Unified)
                .summary("vs main · 3 files", 51, 7)
                .highlights(sample_highlights()),
        )
        .into_any_element()
}
