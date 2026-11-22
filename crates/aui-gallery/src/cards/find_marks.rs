//! Card · Find marks (docs/82 7.5). A message with its matches for "patch"
//! painted the way a transcript find paints them: every match in the find tint
//! (`warning-soft`), the current one ("2 of N") stronger, in prose, in a list
//! and inside a fenced block. The marks come from the library's own
//! `markdown_find`, as a host would hand them back to `Markdown::find_marks`.

use std::rc::Rc;

use aui::transcript::{markdown, markdown_find, FindMark, ProseStyle};
use aui_tokens::{ActiveAui, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::v_flex;

const SAMPLE: &str = "## Validator patch\n\nThe patch tightens `validateAddress`: an empty country no longer passes, and the Canadian patch skips the US branch.\n\n- Patch the ZIP path\n- Run the focused tests\n\n```rust\n// patch: reject empty countries\nif country.is_empty() {\n    return false;\n}\n```";

/// Which of the matches is the current one (0-based): "2 of N".
const CURRENT: usize = 1;

/// Builds the card content.
pub fn build(_window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let style = ProseStyle { ink: p.ink, code_ink: p.ink, code_bg: p.surface_2, role: TextRole::Read, paragraph_gap: 10.0 };
    let matches = markdown_find(SAMPLE, true, "patch", false);
    let marks: Vec<FindMark> = matches
        .into_iter()
        .enumerate()
        .map(|(i, m)| FindMark { cell: m.cell, range: m.range, current: i == CURRENT })
        .collect();
    v_flex()
        .w_full()
        .gap(px(12.0))
        .child(div().role_size(TextRole::Caption).text_color(p.fg_muted).child(format!("2 of {} · every match tinted, the current one stronger", marks.len())))
        .child(markdown("card-find-marks", SAMPLE, style).find_marks(Some(Rc::new(marks))))
        .into_any_element()
}
