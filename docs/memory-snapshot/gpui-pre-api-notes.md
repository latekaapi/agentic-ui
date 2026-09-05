---
name: gpui-pre-api-notes
description: "Hard-won gpui-pre 0.3.3 / gpui-kit 0.6 API facts that shaped the aui workspace (screenshot gating, native springs, theme JSON, asset masks, missing letter-spacing)"
metadata: 
  node_type: memory
  type: reference
  originSessionId: 933ab298-7c5f-45b4-ba22-81c59e419380
  modified: 2026-09-05T14:46:36.558Z
---

Verified 2026-09-05 while bootstrapping crates/ (registry sources under ~/.cargo/registry/src/index.crates.io-*/):
- `Window::render_to_image()` exists but is gated by `test-support` on BOTH `gpui-pre` and `gpui-pre-platform` (the latter forwards to `gpui-pre-macos`). aui-gallery enables both; without the platform feature it returns "render_to_image not implemented for this platform". Output is at device pixels (2× on retina) — the gallery downsamples to logical size because the Chrome references are 1×.
- gpui-pre 0.3.3 ships springs natively: `SpringConfig::new(k, c, m)` (const fn), `SpringAnimation`, `AnimationExt::with_spring`, `sampled_easing(config, eps)`. gpui-base 0.6 has a `motion` module (Presence, Transition, Easing with cubic_bezier/linear_stops/steps, Stagger, Timing, Keyframes, MotionTransform) and `cx.reduce_motion()`. Build aui-motion on top of these, don't reimplement solvers.
- gpui-kit theme = `ThemeSet` JSON (`themes[]` of `ThemeConfig`, dotted colour keys like "primary.background", hex with alpha "#rrggbbaa", plus "highlight" with editor.* and "syntax" map). Register with `ThemeRegistry::global_mut(cx).load_themes_from_str`, then set `Theme::global_mut(cx).light_theme/dark_theme` and call `Theme::change(mode, window, cx)`. Must run after `gpui_kit::init`.
- `gpui_kit::*` re-exports gpui; `gpui_kit::base::{h_flex, v_flex}`; `gpui_kit::component::{Root, TitleBar, Theme, ThemeRegistry}`. TitleBar height 34, macOS traffic lights at (9,9) with 80 px left padding.
- SVG icons render as alpha masks tinted by text colour; the file needs explicit stroke/fill attributes, CSS classes are ignored, `<text>` is not rasterised. `Svg` paints nothing when no text colour is set — aui-icons falls back to `window.text_style().color`.
- gpui has no letter-spacing style: caps labels (.08em tracking) can't match the design exactly — a recorded parity gap.
- `serde_json::json!` with ~40 keys hits the default recursion limit; aui-tokens sets `#![recursion_limit = "256"]`.
- Fonts: `cx.text_system().add_fonts(Vec<Cow<[u8]>>)`; Geist statics bundled in crates/aui-tokens/fonts (OFL).

**How to apply:** reuse these before re-reading the registry sources. See [[gpui-ecosystem-versions]] and [[project-decisions-2026-09-05]].
