# aui design system (HTML source of truth)

This folder is the design-system bundle that gets pushed to Claude Design and later ported to gpui.

- `tokens/tokens.css` — every colour, space, radius, type and motion token, light and dark. The gpui theme JSON is generated from this file.
- `src/base.css` — shared primitives used by the cards (buttons, chips, pills, dots, spinners, shimmer, code/diff rows).
- `src/sprite.svg` — the icon glyphs.
- `src/cards/<group>/<name>.html` — one card per component family. The first line is a `<!-- ds: … -->` comment with group, name, subtitle, size and theme (`dark`, `light`, or `both`).
- `build.py` — inlines tokens, base CSS and the sprite into each card, adds the `@dsCard` marker Claude Design reads, and writes `dist/` plus `dist/cards.json` and `dist/index.html`.

Build:

```bash
python3 design/build.py
```

Render a card to PNG for review (headless Chrome):

```bash
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars \
  --window-size=1300,830 --screenshot=/tmp/shell.png "file://$PWD/design/dist/components/shell/10-app-shell.html"
```

Workflow: edit a card → build → review PNG → push `dist/` to the Claude Design design-system project (DesignSync) → compose screens in Claude Design → implement in `crates/aui`.
