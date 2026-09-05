#!/usr/bin/env python3
"""Build the design-system bundle: one self-contained HTML page per card.
Each src card starts with a metadata comment:
<!-- ds: group="Transcript" name="Tool call cards" subtitle="pending / running / done" width=760 height=520 theme=dark|light|both -->
The rest of the file is body HTML (may include <style> and <script>)."""
import re, sys, json, pathlib, shutil
ROOT = pathlib.Path(__file__).parent
SRC, DIST = ROOT/"src"/"cards", ROOT/"dist"
TOKENS = (ROOT/"tokens"/"tokens.css").read_text()
BASE = (ROOT/"src"/"base.css").read_text()
SPRITE = (ROOT/"src"/"sprite.svg").read_text()
FONTS = '<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600;700&family=Geist+Mono:wght@400;500;600&display=swap">'
META_RE = re.compile(r'<!--\s*ds:\s*(.*?)-->', re.S)
ATTR_RE = re.compile(r'(\w+)=("([^"]*)"|(\S+))')

def parse(src):
    m = META_RE.search(src)
    if not m: raise SystemExit(f"missing ds meta in {src[:60]}")
    attrs = {k: (v3 if raw.startswith('"') else v4) for k, raw, v3, v4 in ATTR_RE.findall(m.group(1))}
    body = src[m.end():].strip()
    return attrs, body

def page(attrs, body, theme):
    name, group = attrs["name"], attrs["group"]
    sub = attrs.get("subtitle", "")
    w, h = attrs.get("width", "720"), attrs.get("height", "480")
    marker = f'<!-- @dsCard group="{group}" name="{name}" subtitle="{sub}" width="{w}" height="{h}" -->'
    return (f'{marker}\n<!doctype html>\n<html lang="en" data-theme="{theme}"><head><meta charset="utf-8">'
            f'<meta name="viewport" content="width=device-width,initial-scale=1"><title>{name}</title>{FONTS}'
            f'<style>{TOKENS}\n{BASE}</style></head><body class="ds">\n{SPRITE}\n{body}\n</body></html>\n')

def main():
    if DIST.exists(): shutil.rmtree(DIST)
    (DIST/"components").mkdir(parents=True)
    index = []
    for f in sorted(SRC.rglob("*.html")):
        attrs, body = parse(f.read_text())
        themes = ["dark", "light"] if attrs.get("theme", "dark") == "both" else [attrs.get("theme", "dark")]
        for t in themes:
            rel = f.relative_to(SRC).with_suffix("")
            out = DIST/"components"/(f"{rel}{'' if len(themes)==1 else '-'+t}.html")
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(page(attrs, body, t))
            index.append({"path": str(out.relative_to(DIST)), "group": attrs["group"], "name": attrs["name"] + ("" if len(themes)==1 else f" ({t})"),
                          "subtitle": attrs.get("subtitle",""), "width": int(attrs.get("width",720)), "height": int(attrs.get("height",480))})
    shutil.copy(ROOT/"tokens"/"tokens.css", DIST/"tokens.css")
    (DIST/"cards.json").write_text(json.dumps(index, indent=2))
    groups = {}
    for c in index: groups.setdefault(c["group"], []).append(c)
    items = "".join(f'<h2>{g}</h2><ul>' + "".join(f'<li><a href="{c["path"]}">{c["name"]}</a> <span>{c["subtitle"]}</span></li>' for c in cs) + '</ul>' for g, cs in groups.items())
    (DIST/"index.html").write_text(f'<!doctype html><html data-theme="dark"><head><meta charset="utf-8"><title>aui design system</title>{FONTS}<style>{TOKENS}{BASE} body{{padding:32px;max-width:720px}} h2{{font-size:13px;margin:24px 0 8px;color:var(--ink-3);text-transform:uppercase;letter-spacing:.08em}} ul{{list-style:none;padding:0;margin:0}} li{{padding:6px 0;border-bottom:1px solid var(--line)}} li span{{color:var(--ink-3);font-size:12px;margin-left:8px}}</style></head><body><h1 style="font-size:20px;margin:0 0 4px">aui design system</h1><p class="subtle">{len(index)} cards</p>{items}</body></html>')
    print(f"built {len(index)} cards -> {DIST}")

if __name__ == "__main__": main()
