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

Learned 2026-09-05 (phase 3, shell + sidebar):
- Stateless `RenderOnce` components keep hover/press state with `window.use_keyed_state((id, "…"), cx, init)` → `Entity<S>`; updating it with `cx.notify()` re-renders the owning view. `aui::util::{interaction_flags, TrackInteraction}` wraps this (on_hover / on_mouse_down / on_mouse_up / on_mouse_up_out).
- CSS collapses vertical margins between siblings; taffy/gpui does not — port `margin:2px 0` as `mt` only or rows drift 2 px per row.
- Element bounds are only known after layout: `gpui_kit::base::ElementExt::on_prepaint(|bounds, window, cx| …)` (a canvas child) stores them; the tab-strip indicator reads last frame's bounds from an `Rc<RefCell<…>>` held in keyed state and calls `window.request_animation_frame()` on the first frame.
- An off-screen `--screenshot` window still receives pointer hover; parity renders are placed at the display's top-left (`Bounds::new(point(0,0), size)`) so hover styles do not leak.
- `Icon::rotate` rotates SVGs only (no div transform); the spinner's accent arc and the stroke-2 chevron are sprite symbols (`spinner-ring`, `spinner-arc`, `chev`, `check-bold`, `x-bold`) with per-path `stroke-width` overriding the generated root attribute.
- `StyledText::new(text).with_highlights([(range, HighlightStyle{..})])` gives mixed-weight inline text; `gpui-base` `Lerp` covers f32 / Pixels / Hsla so `tween` can animate colours.
- CSS margin collapsing only happens in block flow: rows inside a flex column keep both margins (4 px apart), rows in a plain block list collapse to 2 px. `session_row` keeps both margins and offers `collapse_margins()` for block lists; the same trap shows up wherever a CSS `margin` ports to `mt`/`mb`.
- CSS grid rows stretch their cells; port two-column card grids as `h_flex().items_stretch()` rows, not two independent columns.
- gpui-kit's multi-line `Textarea` (component) always sets `editor_paddings` 8/10 on its state in render and a 1.25 rem line box; wrap it, subtract the padding, and set `text_size`/`line_height` on the element (`Textarea` is `Styled`).
- `StyledText::with_runs(Vec<TextRun>)` is the way to mix fonts inline (mono inline code, bold leads, ANSI colours); `HighlightStyle` cannot change the font family, and a run's background hugs the glyphs.
- gpui-kit markdown (`TextView::markdown`) renders inline code without the mono face — `aui::transcript::prose` is the run-based replacement for the transcript.
- Concurrent subagents in one working tree break the shared `cargo build`; give yourself a `git worktree` with its own `CARGO_TARGET_DIR` for parity runs while they work.

**How to apply:** reuse these before re-reading the registry sources. See [[gpui-ecosystem-versions]] and [[project-decisions-2026-09-05]].

**Paint-order gotchas (found 2026-09-06 while fixing Phase 3 bugs):** gpui paints background → children → border, so an absolutely positioned child at a negative inset is covered by the parent's own border; draw accent rails as a wrapper's border instead. Anything that overflows its box (popovers, fanned toast stacks) must go through `aui::overlay::popover_layer` (a `gpui::deferred` wrapper) or later siblings paint over it. Collapsed shell rail is 72 px when traffic lights are present; `AUI_GALLERY_COLLAPSED=1` renders the shell/assistant screens collapsed.
