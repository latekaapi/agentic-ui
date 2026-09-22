//! Card 31 · User and assistant turns. Reproduces
//! `design/src/cards/transcript/31-turns.html` at 760×680.

use aui::protocol::{Attachment, AttachmentKind, TurnMeta, UploadState};
use aui::transcript::{assistant_turn, format_age, user_turn, AssistantTurnAction, UserTurnAction, COPY_HOLD, TextSelection};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::v_flex;

/// `.tr{gap:18px}`.
const TURN_GAP: f32 = 18.0;
/// `.ds-note{max-width:80ch}` ≈ 626 px of Geist 12.
const NOTE_MEASURE: f32 = 640.0;

/// A long, fictional real-world-shaped prompt (headings, lists, code spans)
/// of the same shape a coding-agent brief has. It must wrap inside the user
/// bubble, which never passes 78% of the column.
const LONG_TEXT: &str = r##"# Brief — checkout-flow-v2: address validation, coverage gaps, error copy

Repository `github.com/acme-web/checkout-flow-v2`, branch `main`, clean tree. **Do NOT commit
directly to `main`.** Do not touch the `orca` service. The validation library is vendored at
`vendor/address-kit` and already exports `validateCountry`, `postalPattern` and
`formatAddressLine` (read their doc comments; do not change the vendored copy). Prefix every
shell command with `pnpm install --frozen-lockfile && pnpm build --filter checkout`.
Spend rule: never hit the live payments sandbox, run `pnpm e2e:live`, or call the production
address-verification API. Everything here runs against the recorded fixtures. Read `AGENTS.md`,
`docs/checkout-notes.md`, `docs/validation.md` (§1 rules, "Coverage", the postal section) first.

## Diagnosis (done; do not redo)

Side-by-side of the recorded fixtures against `qa/reference/{addresses,receipts}` and the
suite's own `checkout/*` fixtures showed the validator matches the spec; what is off is the
form's wiring:

- **Blank country passes validation.** `validators.ts::validateAddress` returns `true` when
  `values.country` is empty; `AddressForm.tsx` renders the submit button as enabled the whole
  time. The spec (`docs/validation.md` `## 2 Required fields`, `checkout-baseline.png`) requires
  a filled country before submit unlocks.
- **Canadian codes fall into the US branch.** The `ZIP` regex matches `A1A 1A1` loosely enough
  that `validateCanadianPostal` never runs; `docs/validation.md`'s postal table lists CA and GB
  as their own branches.
- **No coverage.** `validators.test.ts` only exercises the US path; every reference fixture
  bounds coverage to at least one passing and one failing case per country.
- **Error copy is generic.** The form shows "Invalid input" for every field
  (`AddressForm.tsx` swallows the specific `ValidationError` variant), and **the postcode
  field keeps its red border after the value is corrected** (`onBlur` clears the message but
  not the `aria-invalid` flag).

Layout and spacing were checked and are not at fault (the form uses the same token set as the
rest of checkout; the two-column layout is decision 2026-08-30).

## What to build, in this order

1. **Country gate.** In `validateAddress`, an empty `country` returns
   `{ ok: false, field: 'country' }` instead of `true`; `AddressForm.tsx` disables submit while
   any required field is unset (keep the existing `isDirty` guard for untouched fields). The
   silent-fallback branch and the shipping-method side effect keep their own logic.
2. **Country-specific postal branches.** One `const POSTAL_RULES` next to `ZIP_PATTERN` in
   `validators.ts`, keyed by ISO country code, with a comment naming the source table in
   `docs/validation.md`. Every call site that today assumes the US pattern — the inline
   validator, the async duplicate-address check, the CSV importer, the admin override form —
   reads the country-specific rule instead (`resolvePostalRule(country)` returns the matcher and
   the placeholder text). The manual-override path stays as it is. Check every recorded fixture
   in both locales: nothing may accept a malformed code or reject a valid one; the
   `--fixtures ca,gb,us,empty` run still passes at both breakpoints.
3. **Field-level error copy.** `validators.ts`: each failure returns a `ValidationError` with a
   `code` (`required`, `format`, `unsupported-country`). `AddressForm.tsx`:
   `errorCopy(code, field)` instead of the generic string, and the `aria-invalid` flag clears
   in the same `onBlur` that clears the message. The address field no longer needs the shared
   red-border class when only the email field is invalid; drop that rule if it only existed for
   the shared-invalid state. Screenshots cannot show the live focus ring; say so in the report
   and leave the on-screen check to the reviewer.
4. **Submit and retry.** In `AddressForm.tsx`: the submit handler keeps
   `analytics.trackAttempt()`, then disables the button and shows the inline spinner — the form
   and the in-flight request survive a re-render, so a second click cannot double-submit.
   Register `onRetry` once: if the last submit failed with a network error (not a validation
   error), the button reads "Try again"; otherwise "Continue to payment". `--fixtures` runs and
   `--bench` are untouched (they mock the network). Escape still cancels the sheet through
   `onDismiss`. Document in `docs/checkout-notes.md` (the retry row) and `docs/validation.md`.
5. **CHK-142 is verified live.** Delete `AddressForm.legacyFallback` and its
   `// TODO: remove after CHK-142` comment in `AddressForm.tsx`, the sentence in that file's
   header comment that keeps it, and the "Still open … legacyFallback" line in
   `docs/CHANGELOG.md`; note in `docs/diagnosis/checkout.md` (CHK-142) that the new validator
   was verified live against the sandbox with no fallback.

## Regression proof

`scripts/fixtures.sh <dir>` takes the deterministic set (41 recorded requests). The reference
set from before this brief is at `qa/reference/set-before`. Take the set after your change,
`diff` request by request, and list in the report which fixtures changed. Items 1–3 change
every address fixture (intended: gate, branches, copy); the six `payment-*` fixtures must be
byte-identical. Snapshot tests (`pnpm test -u --filter checkout` regenerates them, then
"##;

const USER_TEXT: &str = "Tighten address validation in `@src/checkout` and add coverage for CA and GB postcodes. Keep the existing copy.";

/// The card's clock: every age caption on this card formats against it once,
/// the way an app reads its own clock once per frame.
const NOW_MS: u64 = 1_786_320_000_000;
/// The user prompt went out five minutes before the card's clock.
const USER_SENT_MS: u64 = NOW_MS - 5 * 60_000;
/// The assistant reply started two and a half minutes before the clock.
const ASSISTANT_SENT_MS: u64 = NOW_MS - 150_000;
const ASSISTANT_TEXT: &str = "I'm checking the existing form flow, then I'll patch the validator and run the focused tests.\n\n\
Found the country-specific branch in `validateAddress`. Two things stand out:\n\n\
- An empty country currently returns `true`, which lets a blank address through.\n\
- Canadian postal codes fall into the US ZIP branch.\n\n\
I'll make the ZIP/postal path explicit and keep the checkout copy unchanged, then run the two focused test files";

/// The same paste in both width rows, long enough to pass 78% of the card:
/// the default bubble stops at the cap while the full-width one uses the
/// whole column.
const WIDTH_TEXT: &str = "Pasting circular 12/2026 into the pane to read it here: at the default cap this bubble stops at 78% of the column, while the full-width turn below uses the whole column.";

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
    // The card owns the copy confirmations too, one flag per demo turn: the
    // Copy intent sets it, a `COPY_HOLD` timer clears it. The library never
    // timers anything itself, and the same hold drives the code block header.
    let user_copied = window.use_keyed_state("card31-user-copied", cx, |_, _| false);
    let assistant_copied = window.use_keyed_state("card31-assistant-copied", cx, |_, _| false);
    let arm_user = {
        let flag = user_copied.clone();
        move |action: UserTurnAction, _: &mut Window, cx: &mut App| {
            if action != UserTurnAction::Copy {
                return;
            }
            flag.update(cx, |c, cx| {
                *c = true;
                cx.notify();
            });
            let flag = flag.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(COPY_HOLD).await;
                flag.update(cx, |c, cx| {
                    *c = false;
                    cx.notify();
                });
            })
            .detach();
        }
    };
    let arm_assistant = {
        let flag = assistant_copied.clone();
        move |action: AssistantTurnAction, _: &mut Window, cx: &mut App| {
            if action != AssistantTurnAction::Copy {
                return;
            }
            flag.update(cx, |c, cx| {
                *c = true;
                cx.notify();
            });
            let flag = flag.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(COPY_HOLD).await;
                flag.update(cx, |c, cx| {
                    *c = false;
                    cx.notify();
                });
            })
            .detach();
        }
    };
    v_flex()
        .w_full()
        .gap(px(TURN_GAP))
        .child(div().w_full().flex().justify_end().child(
            user_turn("card31-user", USER_TEXT)
                .attachments(attachments)
                .age(format_age(USER_SENT_MS, NOW_MS))
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        ))
        .child(
            assistant_turn("card31-assistant", ASSISTANT_TEXT)
                .streaming(true)
                .meta(TurnMeta { model: "opus 4.6".into(), duration_ms: 3100, tokens_in: 1800, tokens_out: 600, reasoning_tokens: 0, cost_usd: 0.04 })
                .age(format_age(ASSISTANT_SENT_MS, NOW_MS))
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(div().w_full().flex().justify_end().child(
            user_turn("card31-user-bottom", USER_TEXT)
                .actions_bottom(true)
                .age(format_age(NOW_MS, NOW_MS))
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
            user_turn("card31-user-copied", "Copy this prompt with the row below.")
                .actions_bottom(true)
                .copied(*user_copied.read(cx))
                .on_action(arm_user)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        ))
        .child(
            assistant_turn("card31-assistant-copied", "Copy this reply with the row below — the check holds 1.2 s, the code block's hold.")
                .actions_bottom(true)
                .copied(*assistant_copied.read(cx))
                .on_action(arm_assistant)
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
            v_flex()
                .id("card31-width-compare")
                .role(Role::Group)
                .aria_label("User bubble width: default 78 percent stacked above full width")
                .w_full()
                .gap(px(TURN_GAP))
                .child(
                    div()
                        .id("card31-width-default")
                        .role(Role::Group)
                        .aria_label("User turn at the default 78 percent width")
                        .w_full()
                        .child(
                            user_turn("card31-user-default78", WIDTH_TEXT)
                                .selection(current.as_ref())
                                .on_selection_change(track_selection(selection.clone())),
                        ),
                )
                .child(
                    div()
                        .id("card31-width-full")
                        .role(Role::Group)
                        .aria_label("User turn at full width through max width fraction")
                        .w_full()
                        .child(
                            user_turn("card31-user-full", WIDTH_TEXT)
                                .max_width_fraction(1.0)
                                .selection(current.as_ref())
                                .on_selection_change(track_selection(selection.clone())),
                        ),
                ),
        )
        .child(
            div()
                .id("card31-width-caption")
                .role(Role::Paragraph)
                .aria_label("Caption: the user bubble width cap is the caller's choice")
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Reading width above: the same paste twice, stacked — the default bubble stops at 78% of the column while the full-width turn reaches the right edge through max_width_fraction(1.0). The width cap is the caller's choice, default 78%. Fenced code and tables keep their own scrolling frames."),
        )
        .child(
            div()
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Reduced action set: this turn passes .actions(&[Copy, Retry]), so Fork and Pin stay hidden — the same filter drives the hover toolbar and the bottom row."),
        )
        .child(
            div()
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("How-long-ago captions: the user bubble carries one underneath (5m ago, now), the assistant footer one as its last cell (2m ago). Both format once per card with format_age against the card's clock; a turn with no reported timestamp draws no caption at all."),
        )
        .child(
            div()
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child("Copy confirmations: the two bottom rows above are live — press copy and the rail's copy glyph morphs to a success check for 1.2 s (COPY_HOLD, the code block's hold and colour), then returns. The card owns the flag and the timer per turn; the hover rails run the same morph but stay hidden until hover, so the bottom rows carry the demo."),
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
