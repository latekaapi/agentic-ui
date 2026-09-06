#!/usr/bin/env python3
"""Generate a per-module public API overview from rustdoc's HTML output.

The nightly-only `--output-format json` is not available on this toolchain, so
this parses the HTML rustdoc already writes. Stdlib only.

Usage:
    cargo doc --workspace --no-deps
    python3 scripts/api-doc.py [--doc-dir target/doc] [--out docs/06-api.md]

The output is deterministic: crates in a fixed order, modules alphabetically,
items grouped by kind in a fixed kind order and alphabetical within a kind. No
paths, timestamps or rustdoc version strings are embedded, so re-running with an
unchanged tree produces a byte-identical file.
"""

import argparse
import html as html_mod
import json
import os
import re
import sys
import textwrap

# (rustdoc directory name, display name). Fixed order.
CRATES = [
    ("aui", "aui"),
    ("aui_motion", "aui-motion"),
    ("aui_tokens", "aui-tokens"),
    ("aui_icons", "aui-icons"),
    ("aui_protocol", "aui-protocol"),
]

# Fixed kind order within a module, and the label each kind prints under.
KIND_ORDER = ["fn", "struct", "enum", "trait", "constant", "type", "macro"]
KIND_LABEL = {
    "fn": "fn",
    "struct": "struct",
    "enum": "enum",
    "trait": "trait",
    "constant": "const",
    "type": "type",
    "macro": "macro",
}

# Signatures longer than this get their `where` clause elided.
WHERE_ELIDE_AT = 120
# Constant declarations longer than this get their value elided.
CONST_ELIDE_AT = 100
# Doc summaries longer than SUMMARY_MAX are cut back to their first sentence.
SUMMARY_MAX = 200
SUMMARY_MIN = 60
# Comma-separated variant / field lists wrap at this width.
WRAP_WIDTH = 96

TAG_RE = re.compile(r"<[^>]+>")
WS_RE = re.compile(r"\s+")


def text_of(fragment):
    """Strip every tag and entity from an HTML fragment; collapse whitespace."""
    if not fragment:
        return ""
    # <wbr> is rustdoc's soft word break inside identifiers: drop it with no space.
    fragment = fragment.replace("<wbr>", "")
    fragment = TAG_RE.sub("", fragment)
    fragment = html_mod.unescape(fragment)
    # A second unescape catches rustdoc's doubly-escaped code samples (&amp;lt;).
    if "&" in fragment:
        fragment = html_mod.unescape(fragment)
    return WS_RE.sub(" ", fragment).strip()


CODE_RE = re.compile(r"<code>(.*?)</code>", re.S)


def first_paragraph(fragment):
    """The first <p> of a docblock as plain Markdown, keeping inline code spans."""
    if not fragment:
        return ""
    m = re.search(r"<p>(.*?)</p>", fragment, re.S)
    para = m.group(1) if m else fragment
    # rustdoc renders Markdown `code` as <code>; put the backticks back before
    # the tags are stripped, unless the span itself contains a backtick.
    para = CODE_RE.sub(
        lambda mm: "`%s`" % mm.group(1) if "`" not in mm.group(1) else mm.group(1),
        para,
    )
    return shorten(text_of(para))


def shorten(summary):
    """Trim a long doc paragraph to its first sentence so entries stay one line."""
    if len(summary) <= SUMMARY_MAX:
        return summary
    for m in re.finditer(r"(?<=[.!?]) ", summary):
        if m.start() >= SUMMARY_MIN:
            return summary[: m.start()] + " [...]"
    return summary[:SUMMARY_MAX].rstrip() + " [...]"


def read(path):
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read()


def sidebar_items(module_dir):
    """The SIDEBAR_ITEMS map next to a module index page."""
    path = os.path.join(module_dir, "sidebar-items.js")
    if not os.path.exists(path):
        return {}
    raw = read(path)
    m = re.search(r"window\.SIDEBAR_ITEMS\s*=\s*(\{.*\});?\s*$", raw, re.S)
    if not m:
        return {}
    try:
        return json.loads(m.group(1))
    except ValueError:
        return {}


def module_doc(page):
    """The module's own first documentation paragraph."""
    m = re.search(
        r'<details class="toggle top-doc"[^>]*>.*?<div class="docblock">(.*?)</div>',
        page,
        re.S,
    )
    return first_paragraph(m.group(1)) if m else ""


def index_entries(page):
    """[(kind, name, href, summary)] from a module index's item tables."""
    out = []
    for table in re.findall(r'<dl class="item-table">(.*?)</dl>', page, re.S):
        for chunk in table.split("<dt>")[1:]:
            dt, _, rest = chunk.partition("</dt>")
            link = re.search(r'<a class="([a-z]+)" href="([^"]+)"', dt)
            if not link:
                continue
            kind, href = link.group(1), link.group(2)
            name = text_of(dt)
            dd = re.search(r"<dd>(.*?)</dd>", rest, re.S)
            out.append((kind, name, href, first_paragraph(dd.group(1) if dd else "")))
    return out


def item_decl(page):
    """The `pub fn ...` / `pub const ...` declaration block of an item page."""
    m = re.search(r'<pre class="rust item-decl"><code>(.*?)</code></pre>', page, re.S)
    if not m:
        return ""
    return tidy_signature(text_of(m.group(1)))


def tidy_signature(sig):
    """One line, tidy spacing, `where` elided when the line runs long."""
    sig = WS_RE.sub(" ", sig).strip()
    sig = sig.replace("( ", "(").replace(" )", ")").replace(", )", ")")
    sig = re.sub(r",\s*\)", ")", sig)
    if len(sig) > WHERE_ELIDE_AT and " where " in sig:
        sig = sig.split(" where ", 1)[0].rstrip() + " where ..."
    return sig


def elide_const(decl):
    """`pub const NAME: Type = <long literal>;` -> `... = ...;`"""
    if len(decl) <= CONST_ELIDE_AT or " = " not in decl:
        return decl
    return decl.split(" = ", 1)[0] + " = ...;"


def methods(page):
    """[(name, signature, summary)] for inherent and trait-declared methods."""
    sections = []
    for anchor in ("implementations", "required-methods", "provided-methods"):
        start = page.find('<h2 id="%s"' % anchor)
        if start == -1:
            continue
        end = page.find('<h2 id="', start + 1)
        sections.append(page[start : end if end != -1 else len(page)])

    found = {}
    pattern = re.compile(
        r'<section id="(?:method|tymethod)\.([A-Za-z0-9_]+)"[^>]*>'
        r'.*?<h4 class="code-header">(.*?)</h4>'
        r"(.*?)</details>",
        re.S,
    )
    for section in sections:
        for name, header, tail in pattern.findall(section):
            if name in found:
                continue
            doc = re.search(r'<div class="docblock">(.*?)</div>', tail, re.S)
            found[name] = (
                name,
                tidy_signature(text_of(header)),
                first_paragraph(doc.group(1) if doc else ""),
            )
    return sorted(found.values())


def variants(page):
    """Variant names of an enum, in declaration order."""
    start = page.find('<h2 id="variants"')
    if start == -1:
        return []
    end = page.find('<h2 id="', start + 1)
    block = page[start : end if end != -1 else len(page)]
    seen, out = set(), []
    for name in re.findall(r'<section id="variant\.([A-Za-z0-9_]+)"', block):
        if name not in seen:
            seen.add(name)
            out.append(name)
    return out


def fields(page):
    """Public field names of a struct, in declaration order."""
    start = page.find('<h2 id="fields"')
    if start == -1:
        return []
    end = page.find('<h2 id="', start + 1)
    block = page[start : end if end != -1 else len(page)]
    seen, out = set(), []
    for name in re.findall(r'id="structfield\.([A-Za-z0-9_]+)"', block):
        if name not in seen:
            seen.add(name)
            out.append(name)
    return out


def name_list(label, names):
    """`  - label: \\`a\\`, \\`b\\`` wrapped so no line runs away."""
    body = ", ".join("`%s`" % n for n in names)
    return textwrap.wrap(
        "%s: %s" % (label, body),
        width=WRAP_WIDTH,
        initial_indent="  - ",
        subsequent_indent="    ",
        break_long_words=False,
        break_on_hyphens=False,
    )


def walk_modules(doc_dir, crate_dir, crate_display):
    """(module path, directory) for every public module, root first then sorted."""
    root = os.path.join(doc_dir, crate_dir)
    found = {}
    queue = [(crate_display, root)]
    while queue:
        path, directory = queue.pop()
        if path in found:
            continue
        found[path] = directory
        for child in sidebar_items(directory).get("mod", []):
            child_dir = os.path.join(directory, child)
            if os.path.exists(os.path.join(child_dir, "index.html")):
                queue.append((path + "::" + child, child_dir))
    rest = sorted(p for p in found if p != crate_display)
    return [(crate_display, root)] + [(p, found[p]) for p in rest]


def render_module(path, directory, lines, counts):
    index = os.path.join(directory, "index.html")
    if not os.path.exists(index):
        return
    page = read(index)

    lines.append("### `%s`" % path)
    lines.append("")
    doc = module_doc(page)
    if doc:
        lines.append(doc)
        lines.append("")

    by_kind = {}
    for kind, name, href, summary in index_entries(page):
        if kind == "mod":
            continue  # modules get their own section
        by_kind.setdefault(kind, []).append((name, href, summary))

    known = [k for k in KIND_ORDER if k in by_kind]
    extra = sorted(k for k in by_kind if k not in KIND_ORDER)
    if not known and not extra:
        lines.append("_No public items._")
        lines.append("")
        return

    for kind in known + extra:
        for name, href, summary in sorted(by_kind[kind]):
            counts[0] += 1
            label = KIND_LABEL.get(kind, kind)
            head = "- **%s** `%s`" % (label, name)
            if summary:
                head += " — " + summary
            lines.append(head)

            item_path = os.path.join(directory, href)
            if not os.path.exists(item_path):
                continue
            item = read(item_path)

            if kind in ("fn", "type"):
                decl = item_decl(item)
                if decl:
                    lines.append("  - `%s`" % decl)
            elif kind == "constant":
                decl = item_decl(item)
                if decl:
                    lines.append("  - `%s`" % elide_const(decl))
            elif kind == "enum":
                names = variants(item)
                if names:
                    lines.extend(name_list("variants", names))
            elif kind == "struct":
                names = fields(item)
                if names:
                    lines.extend(name_list("fields", names))

            if kind in ("struct", "enum", "trait"):
                for mname, sig, msummary in methods(item):
                    entry = "  - `%s`" % sig
                    if msummary:
                        entry += " — " + msummary
                    lines.append(entry)
        lines.append("")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--doc-dir", default="target/doc", help="rustdoc output directory")
    ap.add_argument("--out", default="docs/06-api.md", help="markdown file to write")
    args = ap.parse_args()

    doc_dir = args.doc_dir
    if not os.path.isdir(doc_dir):
        sys.exit(
            "no rustdoc output at %s — run `cargo doc --workspace --no-deps` first"
            % doc_dir
        )

    lines = [
        "# API overview",
        "",
        "The public surface of every library crate, one section per crate and one",
        "subsection per module. Generated from rustdoc's HTML by `scripts/api-doc.py`;",
        "do not edit by hand.",
        "",
        "Regenerate with:",
        "",
        "```",
        "cargo doc --workspace --no-deps && python3 scripts/api-doc.py",
        "```",
        "",
        "`aui-gallery` is a binary demo and `aui-webview` / `aui-terminal` expose only a",
        "`PHASE` marker constant, so none of the three appear here.",
        "",
    ]

    counts = [0]
    modules = 0
    for crate_dir, crate_display in CRATES:
        if not os.path.isdir(os.path.join(doc_dir, crate_dir)):
            continue
        lines.append("## `%s`" % crate_display)
        lines.append("")
        for path, directory in walk_modules(doc_dir, crate_dir, crate_display):
            modules += 1
            render_module(path, directory, lines, counts)

    while lines and lines[-1] == "":
        lines.pop()
    lines.append("")

    out = args.out
    parent = os.path.dirname(out)
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(out, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))

    print("%s: %d items across %d modules" % (out, counts[0], modules))


if __name__ == "__main__":
    main()
