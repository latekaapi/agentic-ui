//! Micro-measurements for the transcript's per-frame hot paths.
//!
//! These are `#[test]`s, not benchmarks: they assert the measured path still
//! produces what it produced before, and they *print* a per-call cost only
//! when `AUI_BENCH=1` is set, so `cargo test --workspace` stays fast. Run
//! them with
//!
//! ```sh
//! AUI_BENCH=1 cargo test -p aui --release hotpath_bench -- --nocapture --test-threads=1
//! ```
//!
//! The numbers behind the `library-hotpaths` audit's "measure before you
//! optimise" rule were taken this way; the row-cost proxy
//! ([`bench_row_cost_proxy`]) is the denominator the audit's 5 % rule uses.
//!
//! Where a hot path was changed, the shape it had **before** the change is
//! kept here beside the current one (the `_legacy_` measurements) and both are
//! timed in the same run: an absolute `ns/call` moves with the machine's
//! thermal state, so only a ratio measured back to back means anything.

use std::time::Instant;

use aui_tokens::scale;

use super::ansi::{ansi_runs, parse_ansi};
use super::markdown::{parse_markdown, parsed_markdown, span_runs, Block};
use super::prose::ProseStyle;
use super::syntax::syntax_runs_in;

/// Iterations per measurement. Enough to swamp `Instant`'s resolution on the
/// cheapest path measured here (a cache hit, tens of nanoseconds).
const ITERS: u32 = 20_000;

/// Whether to print timings. Off by default so the gate stays fast.
fn timing_on() -> bool {
    std::env::var("AUI_BENCH").is_ok_and(|v| v == "1")
}

/// Times `f` and prints `ns/call` under `AUI_BENCH=1`.
fn bench(name: &str, iters: u32, mut f: impl FnMut()) {
    if !timing_on() {
        // Still exercise the path once, so the test has meaning in the gate.
        f();
        return;
    }
    for _ in 0..iters / 10 {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed();
    println!(
        "BENCH {name:<40} {:>10.1} ns/call  ({iters} iters, {:?} total)",
        elapsed.as_nanos() as f64 / f64::from(iters),
        elapsed
    );
}

/// A stress-shaped assistant turn: the mix of blocks a real answer has.
fn stress_markdown() -> String {
    let mut out = String::new();
    out.push_str("## What changed\n\n");
    for i in 0..6 {
        out.push_str(&format!(
            "Paragraph {i} with `inline code`, **bold lead** and a [link](https://example.com/{i}) \
             that wraps over more than one visual line because it is long enough to do so.\n\n"
        ));
        out.push_str("- first bullet with `code`\n- second bullet\n- third bullet **bold**\n\n");
    }
    out.push_str("| col | col2 |\n|---|---|\n| a | b |\n| c | d |\n\n");
    out.push_str("```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n");
    out.push_str("Closing paragraph that the streaming caret measures.\n");
    out
}

/// A stress-shaped code block: 60 lines of TypeScript-ish source.
fn stress_code() -> String {
    (0..60)
        .map(|i| {
            format!(
                "  const value{i} = compute{i}('literal {i}', {i}); // note {i}"
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Stress-shaped shell output: 40 lines with SGR colour.
fn stress_ansi() -> Vec<String> {
    (0..40)
        .map(|i| {
            format!("\u{1b}[32mok\u{1b}[0m  test case {i} \u{1b}[2m(0.0{i}s)\u{1b}[0m passed")
        })
        .collect()
}

fn style() -> ProseStyle {
    ProseStyle {
        ink: gpui::black(),
        code_ink: gpui::red(),
        code_bg: gpui::white(),
        size: scale::FS_13,
        line_height: scale::LH_BODY,
        paragraph_gap: 10.0,
    }
}

#[test]
fn bench_parsed_markdown_hit() {
    let source = stress_markdown();
    let style = style();
    let _warm = parsed_markdown(&source, &style);
    bench("parsed_markdown (cache hit)", ITERS, || {
        let blocks = parsed_markdown(&source, &style);
        assert!(!blocks.is_empty());
    });
}

#[test]
fn bench_parse_markdown_cold() {
    let source = stress_markdown();
    bench("parse_markdown (cold parse)", ITERS / 20, || {
        let blocks = parse_markdown(&source);
        assert!(!blocks.is_empty());
    });
}

#[test]
fn bench_prose_blocks_cached() {
    let source = stress_markdown();
    let _warm = super::prose::prose_blocks(&source);
    bench("prose_blocks (memo hit)", ITERS, || {
        let blocks = super::prose::prose_blocks(&source);
        assert!(!blocks.is_empty());
    });
}

#[test]
fn bench_prose_parse_cold() {
    let source = stress_markdown();
    bench("prose::parse (cold parse)", ITERS / 20, || {
        let blocks = super::prose::parse(&source);
        assert!(!blocks.is_empty());
    });
}

#[test]
fn bench_span_runs_all_blocks_legacy() {
    let source = stress_markdown();
    let style = style();
    let blocks = parsed_markdown(&source, &style);
    bench("span_runs LEGACY (all text blocks)", ITERS / 10, || {
        for block in blocks.iter() {
            if let Block::Paragraph(spans) | Block::Heading { spans, .. } = block {
                // The only difference from the current path: the two faces
                // are rebuilt per block instead of cloned from the statics.
                let ui = gpui::font(scale::FONT_UI);
                let mono = gpui::font(scale::FONT_MONO);
                assert_ne!(ui.family, mono.family);
                let (text, runs, _) = span_runs(spans, &style, style.ink);
                assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), text.len());
            }
        }
    });
}

#[test]
fn bench_span_runs_all_blocks() {
    let source = stress_markdown();
    let style = style();
    let blocks = parsed_markdown(&source, &style);
    bench("span_runs (all text blocks)", ITERS / 10, || {
        for block in blocks.iter() {
            if let Block::Paragraph(spans) | Block::Heading { spans, .. } = block {
                let (text, runs, _) = span_runs(spans, &style, style.ink);
                assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), text.len());
            }
        }
    });
}

#[test]
fn bench_font_lookup_pair_legacy() {
    bench("font(UI)+font(MONO) LEGACY", ITERS, || {
        let ui = gpui::font(scale::FONT_UI);
        let mono = gpui::font(scale::FONT_MONO);
        assert_ne!(ui.family, mono.family);
    });
}

#[test]
fn bench_font_lookup_pair() {
    bench("ui_font() + mono_font() (shared)", ITERS, || {
        let ui = super::markdown::ui_font();
        let mono = super::markdown::mono_font();
        assert_ne!(ui.family, mono.family);
    });
}

#[test]
fn bench_syntax_runs_block_legacy() {
    let code = stress_code();
    let palette = aui_tokens::Palette::for_kind(aui_tokens::ThemeKind::Dark);
    bench("syntax runs LEGACY (60 lines)", ITERS / 100, || {
        for line in code.lines() {
            let runs: Vec<gpui::TextRun> = super::syntax::tokenize_line_in(line, Some("ts"))
                .into_iter()
                .map(|(range, kind)| {
                    let mut f = gpui::font(scale::FONT_MONO);
                    if kind == super::syntax::TokenKind::Comment {
                        f.style = gpui::FontStyle::Italic;
                    }
                    gpui::TextRun {
                        len: range.len(),
                        font: f,
                        color: super::syntax::token_color(kind, &palette),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }
                })
                .collect();
            assert!(!runs.is_empty());
        }
    });
}

#[test]
fn bench_syntax_runs_block() {
    let code = stress_code();
    let palette = aui_tokens::Palette::for_kind(aui_tokens::ThemeKind::Dark);
    for line in code.lines() {
        let _ = syntax_runs_in(line, Some("ts"), &palette, scale::FONT_MONO);
    }
    bench("syntax_runs_in (60-line block)", ITERS / 100, || {
        for line in code.lines() {
            let runs = syntax_runs_in(line, Some("ts"), &palette, scale::FONT_MONO);
            assert!(!runs.is_empty());
        }
    });
}

#[test]
fn bench_ansi_block_legacy() {
    let lines = stress_ansi();
    let palette = aui_tokens::Palette::for_kind(aui_tokens::ThemeKind::Dark);
    bench("ansi LEGACY parse+runs (40 lines)", ITERS / 100, || {
        for line in &lines {
            let (text, runs) = ansi_runs(&parse_ansi(line), &palette, scale::FONT_MONO);
            assert!(!text.is_empty());
            assert!(!runs.is_empty());
        }
    });
}

#[test]
fn bench_ansi_parse_only_cold() {
    let lines = stress_ansi();
    bench("parse_ansi (40 lines, uncached)", ITERS / 100, || {
        for line in &lines {
            assert!(!parse_ansi(line).is_empty());
        }
    });
}

#[test]
fn bench_ansi_block() {
    let lines = stress_ansi();
    let palette = aui_tokens::Palette::for_kind(aui_tokens::ThemeKind::Dark);
    for line in &lines {
        let _ = ansi_runs(&super::ansi::ansi_spans(line), &palette, scale::FONT_MONO);
    }
    bench("ansi_spans + ansi_runs (40 lines)", ITERS / 100, || {
        for line in &lines {
            let (text, runs) = ansi_runs(&super::ansi::ansi_spans(line), &palette, scale::FONT_MONO);
            assert!(!text.is_empty());
            assert!(!runs.is_empty());
        }
    });
}

/// `block_id` as it was: a `format!` per block, plus a second `format!` for
/// the per-item and per-cell names.
fn legacy_block_id(id: &gpui::ElementId, index: usize, name: &str) -> gpui::ElementId {
    (
        id.clone(),
        gpui::SharedString::from(format!("md-{index}-{name}")),
    )
        .into()
}

#[test]
fn bench_block_ids_legacy() {
    let source = stress_markdown();
    let style = style();
    let blocks = parsed_markdown(&source, &style);
    let id: gpui::ElementId = "turn".into();
    let count = blocks.len();
    bench("block_id LEGACY (whole turn)", ITERS / 10, || {
        for index in 0..count {
            let child = legacy_block_id(&id, index, "p");
            assert_ne!(child, id);
        }
    });
}

#[test]
fn bench_block_item_ids_legacy() {
    let id: gpui::ElementId = "turn".into();
    bench("list-item id LEGACY (12 items)", ITERS / 10, || {
        for n in 0..12usize {
            let child = legacy_block_id(&id, 3, &format!("li{n}"));
            assert_ne!(child, id);
        }
    });
}

#[test]
fn bench_block_item_ids() {
    let id: gpui::ElementId = "turn".into();
    bench("list-item id (12 items)", ITERS / 10, || {
        for n in 0..12usize {
            let child = super::markdown::block_item_id_for_bench(&id, 3, "li", n);
            assert_ne!(child, id);
        }
    });
}

#[test]
fn bench_block_ids() {
    let source = stress_markdown();
    let style = style();
    let blocks = parsed_markdown(&source, &style);
    let id: gpui::ElementId = "turn".into();
    let count = blocks.len();
    bench("block_id (per block, whole turn)", ITERS / 10, || {
        for index in 0..count {
            let child = super::markdown::block_id(&id, index, "p");
            assert_ne!(child, id);
        }
    });
}

#[test]
fn bench_markdown_selected_text_legacy() {
    let source = stress_markdown();
    let selection = super::selectable::TextSelection {
        cell: super::selectable::SelectionKey::paragraph("", 1),
        range: 0..4,
    };
    bench("markdown_selected_text LEGACY", ITERS / 20, || {
        let _ = super::markdown::selected_text_uncached_for_bench(&source, &selection);
    });
}

#[test]
fn bench_selectable_runs_clone() {
    let style = style();
    let blocks = parsed_markdown(&stress_markdown(), &style);
    let spans = blocks
        .iter()
        .find_map(|b| match b {
            Block::Paragraph(spans) => Some(spans.clone()),
            _ => None,
        })
        .expect("the stress source has a paragraph");
    let (text, runs, _) = span_runs(&spans, &style, style.ink);
    bench("SelectableText runs: clone LEGACY", ITERS, || {
        let cloned = runs.clone();
        assert_eq!(cloned.len(), runs.len());
    });
    let _ = &text;
    bench("SelectableText runs: move (current)", ITERS, || {
        let mut owned = runs.clone();
        let moved = std::mem::take(&mut owned);
        assert_eq!(moved.len(), runs.len());
    });
}

#[test]
fn bench_markdown_selected_text() {
    let source = stress_markdown();
    let selection = super::selectable::TextSelection {
        cell: super::selectable::SelectionKey::paragraph("", 1),
        range: 0..4,
    };
    bench("markdown_selected_text", ITERS / 20, || {
        let _ = super::markdown::markdown_selected_text(&source, &selection);
    });
}

#[test]
fn bench_footer_items() {
    let meta = aui_protocol::TurnMeta {
        model: "claude".into(),
        duration_ms: 3100,
        tokens_in: 1200,
        tokens_out: 1200,
        reasoning_tokens: 40,
        cost_usd: 0.04,
    };
    bench("footer_items + format_duration", ITERS, || {
        let items = super::turns::footer_items(&meta);
        let d = super::tool_card::format_duration(12_400);
        assert!(!items.is_empty());
        assert!(!d.is_empty());
    });
}

/// The denominator for the audit's "under 5 % of the row cost" rule: the
/// per-frame work one settled assistant turn does outside gpui — the memo
/// lookup, the run build for every text block and the element id per block.
#[test]
fn bench_row_cost_proxy() {
    let source = stress_markdown();
    let style = style();
    let id: gpui::ElementId = "turn".into();
    let _warm = parsed_markdown(&source, &style);
    bench("ROW COST PROXY (one assistant turn)", ITERS / 10, || {
        let blocks = parsed_markdown(&source, &style);
        for (index, block) in blocks.iter().enumerate() {
            let _child = super::markdown::block_id(&id, index, "p");
            if let Block::Paragraph(spans) | Block::Heading { spans, .. } = block {
                let _ = span_runs(spans, &style, style.ink);
            }
        }
    });
}
