//! Card 31 · User and assistant turns. Reproduces
//! `design/src/cards/transcript/31-turns.html` at 760×680.

use aui::protocol::{Attachment, AttachmentKind, TurnMeta, UploadState};
use aui::transcript::{assistant_turn, user_turn, AssistantTurnAction, TextSelection};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.tr{gap:18px}`.
const TURN_GAP: f32 = 18.0;
/// `.ds-note{max-width:80ch}` ≈ 626 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

/// A long real-world prompt: the first ~80 lines of the harness's
/// transcript-design brief (headings, lists, code spans). It must wrap
/// inside the user bubble, which never passes 78% of the column.
const LONG_TEXT: &str = r##"# Brief — H1: transcript design, native lights, header collapse, close/reopen, D25 cleanup

Repository `/Users/latekaapi/Projects/harness`, branch `main`, clean tree. **Do NOT commit.**
Do not touch `~/Projects/cockpit`. The library at `/Users/latekaapi/Projects/agentic-ui` is on
branch `transcript-2026-09-12` and already carries `SidebarHeader::native_lights`,
`aui::shell::traffic_light_position` and `AppShell::header_follows_sidebar` (read their
rustdoc; do not change the library). Prefix every shell command with
`export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH"`.
Spend rule: never `turn/start`, `--send`, `send:`/`steer:`, live tests, `muse logout`,
`account/logout`. Everything here runs on `--replay` and `--no-connect`. Read `CLAUDE.md`,
`docs/05-handoff.md`, `docs/02-app.md` (§1 flags, "Measuring", the transcript section) first.

## Diagnosis (done; do not redo)

Side-by-side of `--replay` captures against `agentic-ui/design/reference/{screens,cards}` and
the gallery's own `transcript/*` cards showed the library cards match the design; what is off
is the harness's composition:

- **Blocks inside an assistant turn touch.** `session/render.rs::transcript_list` builds one
  `v_flex` row per turn with no gap; `transcript::turn` returns the blocks as bare children. The
  design (`design/src/cards/shell/10-app-shell.html` `.grp2{gap:8px}`, `harness-Main.png`)
  puts 8 px between stacked cards and between prose and a card.
- **Turn rhythm.** The row's `pb(SP_5)` is the only spacing between turns; the design's
  transcript column is `.tr{gap:16px}`.
- **No measure.** The column stretches to the pane (1112 px at 1440 wide); every reference
  screen bounds the transcript to ~760–800 px and centres it, composer included.
- **Native traffic lights overlap the header** (the window keeps macOS's lights,
  `app.rs` passes `.traffic_lights(false)` to both the shell and the sidebar header, and
  `main.rs` never sets `traffic_light_position`), and **collapsing the sidebar collapses the
  header cell** (`sidebar_header(..).collapsed(!self.sidebar_open)`).
- **Closing the window strands the app.** `main.rs` `on_window_should_close` returns `true`,
  the window is removed, and nothing handles `on_reopen`, so the Dock icon and ⌘-Tab find no
  window to show.

Colour and type were checked and are not at fault (tokens are generated from the same
`tokens.css`; the 1.1 text scale is decision 2026-09-05).

## What to build, in this order

1. **Gaps.** In `transcript_list`, the per-turn row gets `.gap(px(scale::SP_2))` (8) between
   its blocks, and the between-turn step becomes 16 px (the `scale` step that equals 16; keep
   `TRANSCRIPT_PAD_TOP` for the first row and the tail padding as they are). The silent-footer
   row and the user turn's in-flow action row keep their own spacing.
2. **Measure.** One `const TRANSCRIPT_MEASURE: f32` next to `TRANSCRIPT_PAD_X` in
   `session.rs`, 880 px, with a comment naming the design's 760 px card at the 1.1 text scale.
   Every centre-pane row that today uses `TRANSCRIPT_PAD_X` — the list rows, the loading row,
   the status row, banners, caret menus, the queue strip — and the composer's content are
   bounded to that width and centred (`max_w` + `mx_auto` on an inner `w_full` wrapper; the
   composer's docked border keeps spanning the pane, only its inner content is bounded). The
   empty state stays as it is. Check every replay capture at both themes: nothing may overflow,
   clip or jump; the `--steps sidebar-width:<px>` captures at min and max widths still fit.
3. **Native lights and header.** `main.rs` (both `WindowOptions`): `traffic_light_position:
   Some(aui::shell::traffic_light_position(cx))`. `app.rs`: `sidebar_header(..)
   .native_lights(true)` instead of `.traffic_lights(false)`, drop `.collapsed(..)`, and
   `app_shell(..).header_follows_sidebar(false)`. The centre header no longer needs the
   expand-sidebar button when collapsed (the toggle stays in the sidebar header); remove that
   branch if it only existed for the collapsed header. The `--steps sidebar` capture must show
   the full header row over a rail-width body. Screenshots cannot show the OS lights; say so in
   the report and leave the on-screen check to the owner.
4. **Close and reopen.** In `main.rs`: the window's `on_window_should_close` keeps
   `tier::cleanup_probes()`, then hides the app (`cx.hide()`) and returns `false` — the window
   and the `muse serve` child survive, so ⌘-Tab and the Dock bring the same session back.
   Register `cx.on_reopen` once: if `cx.windows()` is empty (the window was removed some other
   way), rebuild the shell window through one shared `open_shell_window(..)` function factored
   out of `main`; otherwise `cx.activate(true)`. `--screenshot` runs and `--bench` are untouched
   (they quit themselves). ⌘Q still quits through `on_app_quit`. Document in `docs/08-keymap.md`
   (the ⌘W row) and `docs/02-app.md`.
5. **D25 is verified live.** Delete `Harness::reconnect_after_login` and its
   `#[allow(dead_code)]` in `login.rs`, the sentence in that file's module doc that keeps it,
   and the "Still open … `reconnect_after_login`" clause in `docs/CHANGELOG.md`; note in
   `docs/diagnosis/login.md` (D25) that the Meta-account login was verified live 2026-09-12
   with no reconnect.

## Regression proof

`scripts/captures.sh <dir>` takes the deterministic set (53 PNGs). The reference set from
before this brief is at `/private/tmp/claude-501/-Users-latekaapi-Projects-harness/f7a85ce4-a78a-46db-b3ec-e131d3878ca7/scratchpad/set-before`.
Take the set after your change, `cmp` file by file, and list in the report which captures
changed. Items 1–3 change every session capture (intended: gaps, measure, header); the five
`login-*` captures must be byte-identical. Adapter snapshots (`UPDATE_SNAPSHOTS=1 cargo test
"##;

const USER_TEXT: &str = "Tighten address validation in `@src/checkout` and add coverage for CA and GB postcodes. Keep the existing copy.";
const ASSISTANT_TEXT: &str = "I'm checking the existing form flow, then I'll patch the validator and run the focused tests.\n\n\
Found the country-specific branch in `validateAddress`. Two things stand out:\n\n\
- An empty country currently returns `true`, which lets a blank address through.\n\
- Canadian postal codes fall into the US ZIP branch.\n\n\
I'll make the ZIP/postal path explicit and keep the checkout copy unchanged, then run the two focused test files";

/// Stores a card-level selection intent: drags, word and paragraph picks
/// replace the stored selection, plain clicks clear it.
fn track_selection(
    setter: Entity<Option<TextSelection>>,
) -> impl Fn(Option<TextSelection>, &mut Window, &mut App) + 'static {
    move |next, _, cx| {
        setter.update(cx, |held, cx| {
            *held = next;
            cx.notify();
        });
    }
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let attachments = vec![
        Attachment { name: "validators.ts".into(), kind: AttachmentKind::File, size_bytes: None, meta: Some("180 lines".into()), state: UploadState::Ready },
        Attachment { name: "checkout-form.png".into(), kind: AttachmentKind::Image, size_bytes: None, meta: None, state: UploadState::Ready },
    ];
    // The card owns the selection, like an app would: one
    // `Option<TextSelection>` wired through every turn.
    let selection = window.use_keyed_state("card31-selection", cx, |_, _| None::<TextSelection>);
    let current = selection.read(cx).clone();
    v_flex()
        .w_full()
        .gap(px(TURN_GAP))
        .child(div().w_full().flex().justify_end().child(
            user_turn("card31-user", USER_TEXT)
                .attachments(attachments)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        ))
        .child(
            assistant_turn("card31-assistant", ASSISTANT_TEXT)
                .streaming(true)
                .meta(TurnMeta { model: "opus 4.6".into(), duration_ms: 3100, tokens_in: 1800, tokens_out: 600, reasoning_tokens: 0, cost_usd: 0.04 })
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(div().w_full().flex().justify_end().child(
            user_turn("card31-user-bottom", USER_TEXT)
                .actions_bottom(true)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        ))
        .child(
            assistant_turn("card31-assistant-bottom", ASSISTANT_TEXT)
                .meta(TurnMeta { model: "opus 4.6".into(), duration_ms: 3100, tokens_in: 1800, tokens_out: 600, reasoning_tokens: 0, cost_usd: 0.04 })
                .actions_bottom(true)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(
            assistant_turn("card31-assistant-reduced", "Validator patched — running the focused tests now.")
                .actions(&[AssistantTurnAction::Copy, AssistantTurnAction::Retry])
                .actions_bottom(true)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(div().w_full().flex().justify_end().child(
            user_turn("card31-user-long", LONG_TEXT)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        ))
        .child(
            div()
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Long prompt above: eighty lines of markdown wrap inside the bubble, which never passes 78% of the column. The short prompts sit in short bubbles hugging the right."),
        )
        .child(
            div()
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Reduced action set: this turn passes .actions(&[Copy, Retry]), so Fork and Pin stay hidden — the same filter drives the hover toolbar and the bottom row."),
        )
        .child(
            div()
                .mt(px(scale::SP_4) - px(TURN_GAP))
                .max_w(px(NOTE_MEASURE))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("User turns sit right in a soft bubble; assistant turns are full-width text with no bubble, so the transcript reads like a document. Streamed chunks fade and rise 3 px; the caret blinks on the accent. The toolbar appears on hover above the turn and never shifts layout. The bottom-row variant pins the same actions in-flow under the prose, muted until hover. Drag inside any turn to select (double-click a word, triple-click a paragraph): the card holds one Option<TextSelection> wired through each turn's selection/on_selection_change, and copies it out with turn_selected_text."),
        )
        .into_any_element()
}
