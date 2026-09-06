#!/usr/bin/env python3
"""Rebuild the screen artboards and emit standalone HTML for headless rendering.

The builders emit `.dc.html` artboards for the design-canvas runtime: the CSS is
already inlined, but the markup is wrapped in `<x-dc>` / `<helmet>` and the theme
is a `{{theme}}` placeholder filled in by `support.js`. Chrome cannot render that
directly (`<x-dc>` is an unknown inline element, so the grid never forms), so we
strip the runtime wrappers and pin the theme, writing one standalone file per
screen into `design/screens/dist/<group>-<Name>.html`.

    python3 design/screens/render.py          # build + emit standalone HTML
"""
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
DIST = HERE / "dist"

GROUPS = {"assistant": "light", "harness": "dark"}


def standalone(text: str, theme: str) -> str:
    text = re.sub(r'<script src="\./support\.js"></script>\s*', "", text)
    text = re.sub(r"<script data-dc-script.*?</script>\s*", "", text, flags=re.S)
    for tag in ("x-dc", "helmet"):
        text = text.replace(f"<{tag}>", "").replace(f"</{tag}>", "")
    text = text.replace("{{theme}}", theme)
    return text


def main() -> int:
    DIST.mkdir(exist_ok=True)
    written = []
    for group, theme in GROUPS.items():
        gdir = HERE / group
        subprocess.run(
            [sys.executable, "build_screens.py"], cwd=gdir, check=True,
            stdout=subprocess.DEVNULL,
        )
        for src in sorted(gdir.glob("*.dc.html")):
            name = src.name[: -len(".dc.html")]
            out = DIST / f"{group}-{name}.html"
            out.write_text(standalone(src.read_text(), theme))
            written.append(out)
    for path in written:
        print(path.relative_to(HERE.parents[1]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
