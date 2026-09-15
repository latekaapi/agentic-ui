# Third-party notices

`aui` is MIT-licensed (see [`LICENSE`](LICENSE)). It bundles or adapts the
following third-party assets.

## Geist / Geist Mono fonts

`crates/aui-tokens/fonts/*.ttf` bundle the Geist and Geist Mono typefaces.
Licensed under the SIL Open Font License, Version 1.1 — see
[`crates/aui-tokens/fonts/OFL.txt`](crates/aui-tokens/fonts/OFL.txt).

```
Copyright 2024 The Geist Project Authors (https://github.com/vercel/geist-font)
```

## Icon glyphs (Lucide / Feather)

`design/src/sprite.svg`, the source sprite sheet `aui-icons` generates its
`IconName` glyph set from, adapts stroke-icon paths from the
[Lucide](https://lucide.dev) icon set. Lucide is ISC-licensed; the subset of
its icons it inherited from the [Feather](https://feathericons.com) project
(which includes several glyphs this sprite uses, e.g. the chevrons) carries
Feather's own MIT notice.

```
ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

---

The following Lucide icons are derived from the Feather project — this
sprite's chevrons and several other glyphs are among them:

airplay, alert-circle, alert-octagon, alert-triangle, aperture, arrow-down-circle,
arrow-down-left, arrow-down-right, arrow-down, arrow-left-circle, arrow-left,
arrow-right-circle, arrow-right, arrow-up-circle, arrow-up-left, arrow-up-right,
arrow-up, at-sign, calendar, cast, check, chevron-down, chevron-left,
chevron-right, chevron-up, chevrons-down, chevrons-left, chevrons-right,
chevrons-up, circle, clipboard, clock, code, columns, command, compass,
corner-down-left, corner-down-right, corner-left-down, corner-left-up,
corner-right-down, corner-right-up, corner-up-left, corner-up-right, crosshair,
database, divide-circle, divide-square, dollar-sign, download, external-link,
feather, frown, hash, headphones, help-circle, info, italic, key, layout,
life-buoy, link-2, link, loader, lock, log-in, log-out, maximize, meh,
minimize, minimize-2, minus-circle, minus-square, minus, monitor, moon,
more-horizontal, more-vertical, move, music, navigation-2, navigation,
octagon, pause-circle, percent, plus-circle, plus-square, plus, power, radio,
rss, search, server, share, shopping-bag, sidebar, smartphone, smile, square,
table-2, tablet, target, terminal, trash-2, trash, triangle, tv, type,
upload, x-circle, x-octagon, x-square, x, zoom-in, zoom-out

The MIT License (MIT) (for the icons listed above)

Copyright (c) 2013-present Cole Bemis

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## `gpui-kit`'s own bundled assets

`aui` layers its icon set over `gpui-kit`'s (see `crates/aui/src/assets.rs`)
and depends on `gpui-kit`'s own Lucide-based icon set and theme assets at
build time. Those ship under `gpui-kit`'s own license (Apache-2.0) in its own
distribution and are not vendored into this repository.
