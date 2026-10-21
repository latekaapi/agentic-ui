//! Markdown parse cost (docs/51 P2 in the harness repo: hold the line).
//!
//! A streaming answer is re-parsed whole on every chunk that changes it, so
//! the parse of a long answer is a per-chunk cost; the memo in
//! `parsed_markdown` only saves the frames between chunks. These benches pin
//! that cost so a parser change that makes it worse shows up as a regression
//! in `cargo bench -p aui --bench markdown`, not as a stutter weeks later.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// A realistic answer: headings, prose with inline code and links, a list, a
/// table and fenced code, repeated to `bytes`.
fn answer(bytes: usize) -> String {
    const SECTION: &str = "## Why the sidebar waited\n\n\
The list reply carried every row, so `merge_session_list` replaced the boot rows \
wholesale and the [label ladder](docs/51.md) disagreed with `rejoin`. A row whose \
title runs past eighty characters was cut on one path and **not** on the other.\n\n\
- the boot rows join the saved copy\n- the reply joins the wire rows\n- `rejoin` reads the index\n\n\
| Path | Label | Cut |\n|---|---|---|\n| boot | index | no |\n| reply | wire | yes |\n\n\
```rust\nfn label(meta: Option<&Meta>, wire: Option<&Session>) -> Option<&str> {\n    \
meta.and_then(|m| m.name.as_deref()).or_else(|| wire.and_then(|w| w.title.as_deref()))\n}\n```\n\n";
    let mut out = String::with_capacity(bytes + SECTION.len());
    while out.len() < bytes {
        out.push_str(SECTION);
    }
    out
}

fn parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_markdown");
    for bytes in [2 * 1024, 20 * 1024, 100 * 1024] {
        let source = answer(bytes);
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(bytes), &source, |b, source| {
            b.iter(|| aui::transcript::parse_markdown(black_box(source)))
        });
    }
    group.finish();
}

fn settled_json(c: &mut Criterion) {
    // A settled message that is one JSON document becomes a single code
    // block: the detection and pretty-print run on every uncached parse.
    let rows: Vec<String> = (0..400).map(|i| format!("{{\"id\":{i},\"name\":\"row {i}\",\"ok\":true}}")).collect();
    let source = format!("[{}]", rows.join(","));
    c.bench_function("parse_markdown_settled/json_400_rows", |b| {
        b.iter(|| aui::transcript::parse_markdown_settled(black_box(&source), true))
    });
}

criterion_group!(benches, parse, settled_json);
criterion_main!(benches);
