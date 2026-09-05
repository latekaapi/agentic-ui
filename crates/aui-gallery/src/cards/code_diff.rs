//! Card 37 · Code and diff blocks. Reproduces
//! `design/src/cards/transcript/37-code-diff-blocks.html` at 760×560.

use aui::protocol::{Diff, DiffKind, DiffLine, Hunk};
use aui::transcript::{code_block, diff_block, DiffNote};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.cb{margin-bottom:14px}`.
const BLOCK_GAP: f32 = 14.0;
/// `.ds-note{max-width:80ch}` ≈ 640 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

const CODE: &str = concat!(
    "export function validateAddress(values: AddressValues) {\n",
    "  if (!values.country) return { ok: false, field: 'country' };\n",
    "  if (values.country === 'CA') return validateCanadianPostal(values);\n",
    "  // US ZIP or ZIP+4\n",
    "  return { ok: ZIP.test(values.postal), field: 'postal' };"
);

fn line(kind: DiffKind, old_no: Option<u32>, new_no: Option<u32>, text: &str) -> DiffLine {
    DiffLine { kind, old_no, new_no, text: text.into() }
}

/// The migration diff of the card.
pub fn sample_diff() -> Diff {
    Diff {
        path: "src/server/migrate.ts".into(),
        hunks: vec![Hunk {
            header: "@@ -42,5 +42,7 @@ export function applyMigration(db, version)".into(),
            lines: vec![
                line(DiffKind::Context, Some(42), Some(42), " const ctx = beginTx(db)"),
                line(DiffKind::Del, Some(43), None, "  runStep(ctx, \"schema\", version)"),
                line(DiffKind::Add, None, Some(43), "  await runStep(ctx, \"schema\", version)"),
                line(DiffKind::Add, None, Some(44), "  await runStep(ctx, \"backfill\", version)"),
                line(DiffKind::Context, Some(44), Some(45), " commit(ctx)"),
            ],
        }],
        added: 2,
        removed: 1,
    }
}

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(code_block("card37-code", "src/checkout/validators.ts", CODE).language("typescript").start_line(44).hidden_lines(22))
        .child(diff_block("card37-diff", sample_diff()).notes(vec![DiffNote {
            line: 44,
            text: "Backfill must run before commit if schema assumes new columns.".into(),
            pending: true,
        }]))
        .child(
            div()
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Code blocks sit on the terminal ground with a 30 px header: filename, language, then quiet actions that brighten on hover. Copy morphs to a check on the swap spring (click it). Long blocks fold after 12 lines. Diff blocks reveal a plus on line hover to add a note."),
        )
        .into_any_element()
}
