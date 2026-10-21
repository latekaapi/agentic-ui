//! Card 31 · User and assistant turns. Reproduces
//! `design/src/cards/transcript/31-turns.html` at 760×680.

use aui::protocol::{Attachment, AttachmentKind, TurnMeta, UploadState};
use aui::transcript::{assistant_turn, format_age, user_turn, AnnotationBatch, AnnotationNote, AssistantTurnAction, UserTurnAction, COPY_HOLD, TextSelection};
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

/// A picture the repo carries, for the tiles and the annotation strip.
const PICTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/reference/cards/10-app-shell.png");

fn sent(name: &str, kind: AttachmentKind, size: Option<u64>, meta: Option<&str>, path: Option<&str>) -> Attachment {
    Attachment { name: name.into(), kind, size_bytes: size, meta: meta.map(Into::into), state: UploadState::Ready, path: path.map(Into::into), quote_of: None }
}

/// Builds the card content.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    // The card owns the selection, like an app would: one
    // `Option<TextSelection>` wired through every turn.
    let selection = window.use_keyed_state("card31-selection", cx, |_, _| None::<TextSelection>);
    let current = selection.read(cx).clone();
    // The card owns the copy confirmations too, one flag per demo turn: the
    // Copy intent sets it, a `COPY_HOLD` timer clears it.
    let user_copied = window.use_keyed_state("card31-user-copied", cx, |_, _| false);
    let assistant_copied = window.use_keyed_state("card31-assistant-copied", cx, |_, _| false);
    let hold = |flag: Entity<bool>| {
        move |cx: &mut App| {
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
    let arm_user = {
        let arm = hold(user_copied.clone());
        move |action: UserTurnAction, _: &mut Window, cx: &mut App| {
            if action == UserTurnAction::Copy {
                arm(cx)
            }
        }
    };
    let arm_assistant = {
        let arm = hold(assistant_copied.clone());
        move |_: AssistantTurnAction, _: &mut Window, cx: &mut App| arm(cx)
    };
    let picture = std::path::Path::new(PICTURE).is_file().then_some(PICTURE);
    let quote = Attachment {
        name: "the provider closed the stream before the reply finished".into(),
        kind: AttachmentKind::Quote,
        size_bytes: None,
        meta: None,
        state: UploadState::Ready,
        path: None,
        quote_of: Some("card31-assistant".into()),
    };
    let batch = AnnotationBatch {
        screenshot: picture.map(Into::into),
        size: (1520.0, 900.0),
        address: "localhost:3000/pricing".into(),
        notes: vec![
            AnnotationNote { index: 1, at: (300.0, 40.0), selector: "h1 · “Simple pricing”".into(), note: "Headline is fine; subtitle needs the free-trial length.".into() },
            AnnotationNote { index: 2, at: (700.0, 70.0), selector: "div.card.starter".into(), note: "Make the Starter card stand out; it is the plan most people pick.".into() },
            AnnotationNote { index: 3, at: (1180.0, 70.0), selector: "div.card.enterprise".into(), note: "Drop the “Contact us” button here; link the text instead.".into() },
        ],
    };
    let note = |text: &'static str| div().max_w(px(NOTE_MEASURE)).ui(scale::FS_12).text_color(p.ink_3).child(text);
    v_flex()
        .w_full()
        .gap(px(TURN_GAP))
        .child(
            user_turn("card31-user", USER_TEXT)
                .age(format_age(USER_SENT_MS, NOW_MS))
                .copied(*user_copied.read(cx))
                .on_action(arm_user)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(
            user_turn("card31-user-sent", "Here is the capture and the fixture.")
                .attachments(vec![
                    sent("transcript-real.png", AttachmentKind::Image, Some(240_000), None, picture),
                    sent("capture.mov", AttachmentKind::File, Some(3_400_000), Some("0:42"), None),
                    sent("synthetic-markdown.jsonl", AttachmentKind::File, Some(2_458), None, None),
                ])
                .on_attachment(|_, _, _| {}),
        )
        .child(user_turn("card31-user-quote", "Try that again.").attachments(vec![quote]).on_quote(|_, _, _, _| {}))
        .child(user_turn("card31-user-annotations", "").annotations(Some(batch)))
        .child(
            assistant_turn("card31-assistant", ASSISTANT_TEXT)
                .meta(TurnMeta { model: "opus 4.6".into(), duration_ms: 3100, tokens_in: 1800, tokens_out: 600, reasoning_tokens: 120, cost_usd: 0.04, ..TurnMeta::default() })
                .age(format_age(ASSISTANT_SENT_MS, NOW_MS))
                .copied(*assistant_copied.read(cx))
                .on_action(arm_assistant)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(
            assistant_turn("card31-streaming", "CA and GB now branch per country, with focused tests over both. The GB regex accepts the space form as of")
                .streaming(true)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(
            user_turn("card31-user-long", LONG_TEXT)
                .selection(current.as_ref())
                .on_selection_change(track_selection(selection.clone())),
        )
        .child(
            v_flex()
                .id("card31-width-compare")
                .role(Role::Group)
                .aria_label("User bubble width: default 78 percent stacked above full width")
                .w_full()
                .gap(px(TURN_GAP))
                .child(div().id("card31-width-default").role(Role::Group).aria_label("User turn at the default 78 percent width").w_full().child(user_turn("card31-user-default78", WIDTH_TEXT)))
                .child(div().id("card31-width-full").role(Role::Group).aria_label("User turn at full width through max width fraction").w_full().child(user_turn("card31-user-full", WIDTH_TEXT).max_width_fraction(1.0))),
        )
        .child(note("D4 (docs/41 §2.2): nothing shows under a turn at rest. Hover the person's turn for edit · copy · age in its 24 px rail; hover an answer for one copy button and the footer (model · time · tokens · reasoning · age). Both rails keep their height while hidden, so nothing shifts. Pictures and videos sent with a turn are 64 px tiles (a video adds the play glyph and its length), files 30 px chips; a quotation sits at the top of the bubble with \"from the reply above\"; a Browse annotation batch is one turn with the page strip, pins and notes. The streaming answer ends in a solid 2 px caret."))
        .into_any_element()
}
