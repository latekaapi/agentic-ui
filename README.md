# agentic-ui

A Rust component library for agent tooling, built on [gpui](https://www.gpui.rs)
(`gpui-pre`) and `gpui-kit`. It provides the shell, transcript, composer and
workbench that an Orca-like agent harness needs, plus the day-job assistant app
built from the same parts. Everything is designed first in `design/`, rendered to
reference screenshots, and then matched pixel for pixel in Rust.

## Crates

| Crate | What it is |
| --- | --- |
| `aui-tokens` | Colour, space, radius, type, shadow and motion tokens; light and dark themes for the `gpui-kit` theme registry. |
| `aui-motion` | Springs and animation primitives: `Transition`, `Presence`, `Collapse`, `Stagger`, `Shimmer`, `IconMorph`, `StreamReveal` and friends. |
| `aui-icons` | Lucide-style icon set plus provider marks, file-type icons and status glyphs. |
| `aui` | The component library itself: shell, nav, transcript, composer, workbench, data, feedback and overlay modules. |
| `aui-protocol` | Transport-agnostic session model — `Session`, `Turn`, `Block`, streaming `Delta`s and the `Intent`s the UI emits back. No I/O, no gpui. |
| `aui-webview` | The embedded browser pane: wry/WKWebView, tabs, nav, JS bridge, element annotator and screenshot-to-chat. |
| `aui-terminal` | PTY-backed block terminal and the agent's TUI mode, rendered as a gpui element. |
| `aui-gallery` | Storybook app: every component in every state, a motion playground and a theme switcher; also the source of the design-system previews. |

## Prerequisites

- macOS (the shell, webview and terminal panes use AppKit and WKWebView).
- Xcode command line tools: `xcode-select --install`.
- Rust stable via [rustup](https://rustup.rs): `rustup update stable`.

## Build and run

```sh
# the storybook — every component, every state
cargo run -p aui-gallery

# render one gallery entry at its card size, for the parity loop
cargo run -p aui-gallery -- --screenshot <entry> <out.png>

# other gallery flags
cargo run -p aui-gallery -- --list                          # entry ids and sizes
cargo run -p aui-gallery -- --theme light --entry <id>      # open in a theme / on an entry
cargo run -p aui-gallery -- --screenshot-window <out.png>   # whole window, chrome included
cargo run -p aui-gallery -- --screenshot <entry> <out.png> --screenshot-delay 900   # sample motion later

# render, diff against the reference and print the pixel distance
scripts/parity.sh <entry> <reference-stem>     # e.g. scripts/parity.sh foundations/colour 01-color

# regenerate the gpui-kit theme JSON (crates/aui-tokens/themes/agentic-ui.json)
cargo run -p aui-tokens --example dump-theme

# tests across the workspace
cargo test --workspace

# API docs for every crate
cargo doc --workspace --no-deps --open
```

## Design is the source of truth

- `design/` holds the design system: `design/src/cards/**` are the authored
  cards, `design/dist/` the built HTML, `design/tokens/` the token JSON, and
  `design/reference/` the rendered PNGs that Rust output is compared against.
- `docs/02-component-spec.md` is the component contract — every card, every
  state, every measurement.
- `docs/03-parity-process.md` is the render-and-compare loop.
- `docs/04-design-rules.md` is the taste guide the specs are written against.

When the design and the code disagree, the design wins; change the card first,
re-render the reference, then change the Rust.

## The parity loop

For each component: read its section of the spec, build it in `aui`, add a
gallery entry covering every state, render it with
`cargo run -p aui-gallery -- --screenshot <entry> <out.png>`, and compare the
result against the matching PNG in `design/reference/`. Differences in spacing,
weight, colour or motion are bugs in the Rust until the spec says otherwise. The
full procedure, including how to add a new reference render and what counts as a
pass, is in [`docs/03-parity-process.md`](docs/03-parity-process.md).
