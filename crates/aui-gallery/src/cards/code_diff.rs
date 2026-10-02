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
/// The long-file preview pane below the transcript blocks.
const PREVIEW_H: f32 = 420.0;
/// The line the preview opens on.
const PREVIEW_LINE: u32 = 1000;

/// The head of the Rust sample: exactly 39 lines, so the highlighted band
/// below lands on lines 40–44 (`validate_postal`).
const RUST_HEAD: &str = concat!(
    "//! Address validators for the checkout flow.\n",
    "use std::collections::HashMap;\n",
    "\n",
    "const CA_LEN: usize = 7;\n",
    "const US_LEN: usize = 5;\n",
    "\n",
    "/// The outcome of a single field check.\n",
    "pub enum Outcome {\n",
    "    Ok,\n",
    "    Missing,\n",
    "    Invalid,\n",
    "}\n",
    "\n",
    "pub struct AddressValues {\n",
    "    pub country: String,\n",
    "    pub postal: String,\n",
    "    pub region: String,\n",
    "}\n",
    "\n",
    "impl AddressValues {\n",
    "    pub fn new(country: &str, postal: &str, region: &str) -> Self {\n",
    "        Self { country: country.into(), postal: postal.into(), region: region.into() }\n",
    "    }\n",
    "\n",
    "    pub fn is_empty(&self) -> bool {\n",
    "        self.country.is_empty() && self.postal.is_empty()\n",
    "    }\n",
    "}\n",
    "\n",
    "static KNOWN: &[&str] = &[\"CA\", \"US\", \"GB\"];\n",
    "\n",
    "pub fn validate_address(values: &AddressValues) -> Outcome {\n",
    "    if values.country.is_empty() {\n",
    "        return Outcome::Missing;\n",
    "    }\n",
    "    match values.country.as_str() {\n",
    "        \"CA\" => validate_postal(&values.postal, CA_LEN),\n",
    "        _ => Outcome::Ok,\n",
    "    }\n",
    "pub fn validate_postal(code: &str, len: usize) -> Outcome {\n",
    "    let mut upper = code.trim().to_uppercase();\n",
    "    upper.retain(|c| c.is_ascii_alphanumeric());\n",
    "    if upper.len() == len { Outcome::Ok } else { Outcome::Invalid }\n",
    "}\n",
);

/// A long Rust file: the 44-line head above plus generated checks, past the
/// 400-line virtualisation threshold, with lines 40–44 highlighted.
fn rust_sample() -> String {
    let mut code = String::from(RUST_HEAD);
    for i in 0..64usize {
        code.push_str(&format!(
            "\n/// Generated field check {i:02}.\n\
             pub fn check_field_{i:02}(value: &str) -> Outcome {{\n\
             \x20   let mut seen: HashMap<&str, usize> = HashMap::new();\n\
             \x20   seen.insert(value, {i});\n\
             \x20   match seen.get(value) {{\n\
             \x20       Some(_) => Outcome::Ok,\n\
             \x20       None => Outcome::Missing,\n\
             \x20   }}\n\
             }}\n"
        ));
    }
    code
}

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

/// A long file for the preview pane: the head above plus generated checks,
/// past the 400-line virtualisation threshold, so the filling block below
/// virtualises instead of building every row.
fn long_sample() -> String {
    let mut code = String::from(RUST_HEAD);
    for i in 0..140usize {
        code.push_str(&format!(
            "/// Generated field check {i:03}.\n\
             pub fn check_long_{i:03}(value: &str) -> Outcome {{\n\
             \x20   if value.len() == {i} {{ Outcome::Ok }} else {{ Outcome::Invalid }}\n\
             }}\n"
        ));
    }
    code
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    // Cached per card so the thousand-line sample is built once, not once
    // per frame.
    let long = window.use_keyed_state("card37-long", cx, |_, _| long_sample()).read(cx).clone();
    v_flex()
        .w_full()
        .gap(px(BLOCK_GAP))
        .child(
            code_block("card37-code", "src/checkout/validators.rs", rust_sample())
                .language("rust")
                .start_line(1)
                .highlight_lines(40..45),
        )
        .child(
            // The Files-style preview: a fixed-height pane whose filling
            // block owns the only scroll, opening at line 1000.
            v_flex().w_full().h(px(PREVIEW_H)).child(
                code_block("card37-preview", "src/checkout/generated.ts", long)
                    .language("typescript")
                    .fill(true)
                    .scroll_to_line(PREVIEW_LINE)
                    .scroll_token(1),
            ),
        )
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
                .child("Code blocks sit on the terminal ground with a 30 px header: filename, language, then quiet actions that brighten on hover. Copy morphs to a check on the swap spring (click it). Blocks past 400 lines virtualise, open scrolled to the highlighted lines 40–44. The preview pane below fills its height and owns the only scroll, opening at line 1000. Diff blocks reveal a plus on line hover to add a note."),
        )
        .into_any_element()
}
