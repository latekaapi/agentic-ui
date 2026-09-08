# Muse Code (MSP) research — building an `aui` chat client

Research for a gpui chat app over Meta's **Muse Code** agent, built on `crates/aui` and
adapting Muse's wire protocol onto `aui_protocol::{Session, Turn, Block, Delta, Intent}`.

Everything below is from **`muse` 1.0.3 (1.0.3-R2198.1)** on macOS: the schema bundle the
binary exports (`muse schema generate-json-schema/generate-ts`, fingerprint
`sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7`) plus live probes
against `muse serve`. Type and field names are quoted verbatim from
`msp.d.ts` (1892 lines, cited as `msp.d.ts:NNN`) and from captured wire frames.

Probe scripts and full transcripts live in the research scratchpad (`probe.py`,
`probe_real.py`, `probe_wire.py`, `probe_approve.py`, `transcript-*.jsonl`); the transcripts
quoted here are trimmed but otherwise verbatim.

---

## 1. MSP wire contract

### 1.1 Transport and framing

`muse serve` speaks **JSON-RPC 2.0 over stdio, newline-delimited JSON — one compact JSON
value per line, no `Content-Length` headers, no `\r\n`**. Confirmed live: a client that
writes `json.dumps(obj) + "\n"` and reads `for line in stdout` works end to end, and every
server frame arrives as exactly one line. The server writes compact JSON (no spaces after
`:` or `,`); `stderr` is separate and carries no protocol.

```
$ muse serve --help
muse serve — serve an MSP session host over stdio

The client owns this process's stdin and stdout and is its only
connection. Sandbox posture and session durability are constructed
here and apply to every session the host loads; neither is negotiable
over the wire. Approval mode is the other way round — it is selected
on the wire, so there is no approval flag here.

Options:
      --no-session-log        Use memory-only sessions
      --disable-sandbox       Disable shell filesystem/network sandboxing for this host
      --sandbox-network <MODE>  (default: proxy-only)
      --disable-write         Disable non-shell workspace filesystem writes
      --disable-shell         Disable workspace shell execution
      --trust-workspace       Load each session workspace's skills and rules
```

Key consequence for the app: **there is no `--provider` on `muse serve`.** The provider is
chosen **per session** in `session/start` params (`providerId`), which the probe confirmed —
see §2.2. `--echo-delay-ms` is a TUI-only flag and has no MSP equivalent.

Frame shapes (`msp.d.ts:822–853`, `1328–1336`, `735–745`):

- Request: `{"jsonrpc":"2.0","id":<number|string>,"method":"…","params":{…}}`, optional
  `trace: {traceparent, tracestate}` (requests only, SS1.8). `params` is omitted entirely
  when empty, never `null`. Ids: "each direction owns its own id space", and `1` ≠ `"1"`.
- Success: `{"jsonrpc":"2.0","id":…,"result":{…}}` — `result` is *always* an object,
  possibly `{}`, never a bare scalar, so every result can grow additive-optional members.
- Error: `{"jsonrpc":"2.0","id":…|null,"error":{code,message,data?}}`. `id` is `null` only
  for an unrecoverable parse error.
- Notification: no `id`; server→client notifications carry `emittedAtMs` (Unix ms).

**Both directions can send requests.** The server sends `userInput/request` and
`approval/request` as real JSON-RPC *requests* with the server's own `id` — see §1.6/§1.7.

Every server→client view event carries the SS4.2 base triple: `sessionId`, `viewCursor`
(opaque, strictly monotonic, observed live as `"v:<sessionId>:<n>"` — **do not parse it**),
and `sourceRange` (except `item/delta` and an ephemeral `item/started`, which have none).

### 1.2 Handshake

`initialize` params (`InitializeParams`, `msp.d.ts:424–430`):

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
  "clientInfo":{"name":"aui_probe","title":"AUI Probe","version":"0.1.0"},
  "capabilities":{"experimentalApi":false,
                  "requestedCapabilities":["userShell"],
                  "optOutNotificationMethods":[]}}}
```

- `ClientInfo` (`msp.d.ts:232–239`): `name` **must match `[a-z0-9_]+`**, `version` required,
  `title` optional. Diagnostics/telemetry attribution only — never an authority claim.
  `clientInfo.name` is recorded on approval-mode audit facts
  (`SessionApprovalModeChangedParams.clientName`) and in the durable `session_opened` record.
- `ClientCapabilities` (`msp.d.ts:222–229`): `experimentalApi` (default `false`),
  `requestedCapabilities` (free strings; unknown entries are silently not granted),
  `optOutNotificationMethods` (exact method names, no wildcards; the protected set cannot be
  opted out — `item/delta` explicitly *can* be, which is the lever for a "hide reasoning"
  low-bandwidth mode).

Live `InitializeResult` (verbatim, `transcript-echo.jsonl`):

```json
{"jsonrpc":"2.0","id":1,"result":{
  "serverInfo":{"name":"muse","version":"1.0.3"},
  "userAgent":"muse-build/1.0.3 (non-interactive; macos-aarch64; build 238bb03ff370497df820cffe572ee9b4f76ec5b1)",
  "museHome":"/Users/latekaapi/.local/share/muse",
  "platformFamily":"unix","platformOs":"macos",
  "schema":{"version":1,
            "fingerprint":"sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7"},
  "grantedCapabilities":["userShell"],
  "experimentalApi":false,
  "sessionDurability":"durable"}}
```

`InitializeResult` (`msp.d.ts:433–452`): `experimentalApi` echoes the negotiated setting;
`grantedCapabilities` is fixed for the connection lifetime (INV-009); `schema.fingerprint` is
the content hash of this binary's stable-surface bundle — **a mismatch against the bundle the
app was generated from is a warning condition, not an error** (SS1.4.1). `sessionDurability`
is `"durable" | "ephemeral"`; absent reads as `durable`.

Close the handshake with the client→server notification `initialized` (no params). Calling
anything before it yields `-32600` / `notInitialized`; calling `initialize` twice yields
`-32600` / `alreadyInitialized`.

The **only capability in the v1 registry is `userShell`**; `rawLog` is reserved
(`msp.schema.json` `capabilities`: `{"grantable":["userShell"],"reserved":[{"name":"rawLog","reference":"#13929"}]}`).

**Stable vs experimental surface: identical.** `muse schema generate-ts --out msp-exp
--experimental` produced a file byte-for-byte identical to the stable export
(`diff` reports no differences; both 1892 lines, 103265 bytes). So in 1.0.3 there is nothing
extra behind `experimentalApi: true` on the published surface — do not set it.

### 1.3 Method and notification index

`MspMethod` (`msp.d.ts:1888`) — 31 methods:

```
initialize
session/start  session/resume  session/fork  session/list  session/read
session/compact  session/setModel  session/setApprovalMode  session/userShell
turn/start  turn/steer  turn/interrupt  turn/cancel  turn/unqueue
model/list
view/page  view/unsubscribe
approval/decide  approval/listPending
userInput/answer  userInput/cancel  userInput/clarify
subagent/sendMessage  subagent/followupTask  subagent/interrupt  subagent/stop
subagent/resume  subagent/reopen  subagent/close  subagent/readResult
```

`MspNotification` (`msp.d.ts:1890`) — 23:

```
initialized (client→server)
turn/started  turn/completed  turn/retracted  turn/retryScheduled  turn/unqueued
item/started  item/updated  item/delta  item/completed
approval/requested  approval/updated  approval/resolved
userInput/requested  userInput/settled
session/modelChanged  session/goalChanged  session/todoListChanged  session/branchChanged
session/tokenUsage  session/contextUsage  session/approvalModeChanged
view/gap
```

**Two things the binary emits that the published index does not list** (found live — handle
them, don't assume the index is exhaustive):

- `session/started` — a notification whose params are `{"session": Session}`, emitted
  *before* the `session/start` response. Also fired on `session/fork`.
- `userInput/request` and `approval/request` — server→client **requests** (with an `id`),
  carrying the same params as the `…/requested` notifications. Captured live, see §1.6.

Also absent from the index but referenced throughout the doc comments: `item/readOutput`
(the `OutputRef` fetch path, "served by #208") and `workflow/*` (spec 14410) — `muse schema
--help` says outright that `workflow/*` "carries no row yet". Treat both as **not available
in 1.0.3**; a `toolCall`'s bytes are only reachable through `visibleOutput` (bounded) and the
durable log on disk.

### 1.4 Session lifecycle

Every command carries `commandId`: a **client-minted UUIDv7**. This is enforced — a v4 UUID
is rejected:

```
--> {"method":"session/start","params":{"commandId":"<uuid4>",…}}
<-- {"error":{"code":-32602,"message":"invalid session/start commandId: expected UUIDv7",
              "data":{"kind":"invalidParams"}}}
```

`commandId` is the idempotency handle: a dedup'd retry returns the same result. For
`turn/start` the fresh turn's `turnId` **equals** its `commandId` (SS3.1.4) — confirmed live.

**`session/start`** (`SessionStartParams`, `msp.d.ts:1131–1146`):
`commandId` (req), `workspaceRoot` (absolute path), `providerId` (`"meta"` default,
`"echo"` works), `modelId`, `approvalMode` (`ApprovalMode | null`), `sessionId` (mint your
own; a retained id is `commandRejected` reason `session_id_conflict`), `config`
(`SessionConfig` — **no members in v1**, and unknown keys are rejected, not ignored).
Result: `{session: Session, viewCursor}`. The connection is subscribed at that cursor.

**`Session`** (`msp.d.ts:856–881`) — the object every lifecycle method returns:

```json
{"sessionId":"01a081e5-3361-7952-94ce-456eda0dd590",
 "path":"/Users/…/.local/share/muse/sessions/2026/09/08/01a081e5-…/session.jsonl",
 "status":"idle","activeTurnId":null,
 "createdAt":"2026-09-08T16:41:16.912641Z","updatedAt":"2026-09-08T16:41:16.912645Z",
 "workspaceRoot":"/…/ws","providerId":"meta","modelId":"muse-spark-1.3-contributor",
 "turnCount":0,"forkedFrom":null,
 "approvalMode":{"mode":"onRequest","source":"startup","lastCommandId":null}}
```

`activeTurnId` is **required-nullable** (always on the wire, `null` when idle).
`status: SessionStatus = "notLoaded" | "idle" | "running"` — `session/list` reports
`notLoaded` for sessions loaded by *other* hosts. `path` is the **empty string** under the
ephemeral profile ("the one value a client must not hand to a filesystem call").
`approvalMode` is additive-optional and legitimately missing on index-derived
`session/list` rows.

**Durability** is a property of the *host process*, fixed at construction
(`muse serve --no-session-log` → `sessionDurability: "ephemeral"`), never negotiated per
session. So an app that wants scratch sessions must spawn a second `muse serve`.

**`session/resume`** (`msp.d.ts:1064–1088`): `commandId`, `sessionId`, `cursor?` (a view
cursor *this client observed*, or an observed `summarizedThrough` compaction anchor),
`excludeItems?` (default `false`), `history?: HistoryPreference`.
Result `{session, history, pendingRequests, viewCursor}`; the connection is subscribed after
`viewCursor`. A resume that loads a session writes a durable `SessionResumed` record.

`HistoryPreference = "auto" | "inline" | "snapshot" | "anchored"` (`msp.d.ts:419`) is a
**preference, not a budget override**. What was actually served is `history.mode`:
`HistoryMode = "anchoredSnapshot" | "inline" | "snapshot" | "none"` (`msp.d.ts:413`), and
under `auto` the rung order is anchoredSnapshot → inline → snapshot → elided snapshot → none.
When `mode` is `"none"`, `noneReason: HistoryNoneReason = "excluded" | "cursorSuffix" |
"historyBudget" | "projectionUnavailable" | "projectionReadLimit"` says why. Observed live:
`excludeItems: true` → `{"mode":"none","noneReason":"excluded"}`; default → `{"mode":"inline",
"items":[…]}`.

`SessionHistory` (`msp.d.ts:997–1006`): `items: Item[] | null` (populated only for `inline`),
`snapshot: ViewSnapshot | null` (only for `snapshot`/`anchoredSnapshot`), `mode`, `noneReason?`.

**`ViewSnapshot`** (`msp.d.ts:1844–1853`): `schemaVersion`, `viewCursor`, `state: SnapshotState`,
`anchor?: {boundaryCursor, summarizedThrough}` (present exactly for `anchoredSnapshot`).
**`SnapshotState`** (`msp.d.ts:1228–1255`) is the whole folded view and is exactly what a
client needs to render a cold session in one shot: `items: Item[]` (first-opened order, each
at its latest revision), `activeTurn: TurnRef | null`, `queuedTurns: TurnRef[]`,
`pendingApprovals: PendingApprovalPointer[]`, `pendingUserInputs: PendingUserInputPointer[]`,
`approvalMode`, `branch: BranchState | null`, `contextUsage?`, `effectiveModel`, `goal`,
`todoList`, `tokenUsage: CumulativeTokenUsage`, `turnCount`.
⚠️ The d.ts warns (#22785 E8) that the *genesis* snapshot rung today serves
`state: {"items": […]}` only — a schema-validating client must tolerate the other members
being absent.

**`session/fork`** (`msp.d.ts:961–982`): `commandId`, `sessionId`, `cutPoint?: {lastTurnId}`
(inclusive; **turn ids, not counts**, because ids survive compaction), `excludeItems?`.
Returns the `session/resume` envelope for the *new* session. Live:

```json
"forkedFrom":{"sessionId":"01a081ee-3995-…","cutCursor":"session:01a081ee-3995-…:turn:0",
              "cutExplicit":false,"commandId":"01a081ee-…"}
```

`ForkProvenance.cutCursor` is an **opaque, display-only** provenance string — not one of the
cursor families, no method accepts it. Naming an in-progress or unknown turn →
`-32023 forkBoundaryInvalid` with `data.lastTurnId`.

**`session/list`** (`msp.d.ts:1009–1026`): `cursor?`, `limit?` (default 50, **max 200**),
`updatedAfter?` (RFC3339), `workspaceRoot?` (exact path equality). Read-only, never touches
leases. Result `{sessions: Session[], nextCursor: string|null}`, ordered `updatedAt` DESC.
Page cursors are a **distinct opaque family** from view cursors.
⚠️ Observed quirk: the echo session listed with `"providerId":"meta"` even though its durable
metadata record says `"provider_id":"echo"` — the list row comes from the index projection.
Do not trust `session/list.providerId` for a session started with a non-default provider.

**`session/read`** (`msp.d.ts:1044–1062`): read one stored session **without attaching** — no
writer lease, no load, no subscription, no `SessionResumed` record, and it **never re-issues
pending requests**. `excludeItems` defaults to **`true`** here (opposite of resume/fork).
This is the method for a session-list preview pane.

**`view/page`** (`msp.d.ts:1813–1841`) — the backfill path for old sessions:
`sessionId`, `limit` (1–1000), `cursor?` (exclusive anchor), `direction?: "forward"|"backward"`
(default forward), `anchor?: "latestCompaction"` (mutually exclusive with `cursor`; the one
exception to the observed-only cursor rule — the cold-client entry point). Result
`{events: UnframedViewNotification[], nextCursor: string|null, resolvedAnchor?}`.
Each event is `{method, params}` — the live notification minus `jsonrpc`/`emittedAtMs`,
nothing lifted or spliced. Pages are **always ascending by `viewCursor` in both directions**,
durable-sourced only, contiguous, and never skip a cursor. `item/delta` is never replayed —
so a backfilled `agentMessage` arrives whole via its `item/completed`.

Live `view/page` element (trimmed):

```json
{"method":"turn/unqueued",
 "params":{"sessionId":"01a081ee-3995-…","viewCursor":"v:01a081ee-3995-…:13",
           "sourceRange":{"stream":{"kind":"session","id":"01a081ee-3995-…"},
                          "first":{"id":"1c379a89-…","sequence":45},
                          "last":{"id":"1c379a89-…","sequence":45}},
           "turnId":"01a081ee-70cd-…","commandId":"01a081ee-70cd-…"}}
```

**`view/unsubscribe`** (`msp.d.ts:1856–1863`): `{sessionId}` → `{}`, idempotent. Stops
following; does not unload the session (unloading is the host's idle policy).

**`view/gap`** (`msp.d.ts:1798–1805`): `{sessionId, after, next}` brackets a dropped range and
delivery continues at `next`. Two sanctioned recoveries: splice-fill (buffer live events at
cursors ≥ `next`, `view/page` the `(after, next)` range forward, discard overlap, splice) or
re-anchor through the anchored read surface. **The UI must handle this** — it is the one
event that says "your transcript has a hole".

### 1.5 Turns

**`turn/start`** (`TurnStartParams`, `msp.d.ts:1531–1544`):

```json
{"commandId":"<uuidv7>","sessionId":"…",
 "input":[{"type":"text","text":"hello from the probe"}],
 "displayText":"…", "ifBusy":"queue", "reasoningEffort":"low"}
```

- `input: TurnInputPart[]` — required, non-empty. `TurnInputPart` (`msp.d.ts:1443–1456`) is a
  **flat discriminated object**: `type: "text" | "image"` (**closed** — an unknown type is
  `invalidParams`), `text?` for text, `base64Data?` + `mediaType?` **required** for image,
  `width?`/`height?` which must be given **together or not at all**. Multiple text parts are
  joined in order. **File mentions are text, not a part type: write `@relative/path` inside a
  text part.** A structured `mention` part is reserved and currently rejected.
- `displayText?` — presentation form for transcripts; durable, carried on the resulting
  `userMessage` item, **never model-visible**. This is where `@`-mention chip markup goes.
- `ifBusy: IfBusy = "queue" | "steer" | "replace"` (`msp.d.ts:422`), **wire default `queue`**.
- `reasoningEffort: ReasoningEffort = "none" | "minimal" | "low" | "medium" | "high" |
  "xhigh" | "ultra"` (`msp.d.ts:812`) — **closed**, sampled at submission.

⚠️ **Effort mismatch, verified live.** The CLI advertises `none|minimal|low|medium|high|xhigh|max|ultra`
and the on-disk model catalog lists tiers `minimal, low, medium, high, xhigh, **max**` — but
MSP's enum has `ultra` and **no `max`**:

```
--> turn/start … "reasoningEffort":"max"
<-- {"code":-32602,"message":"Invalid params: invalid turn/start params: unknown variant `max`,
     expected one of `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `ultra`",
     "data":{"kind":"invalidParams","retryable":false}}
```

The effort picker must be driven by the MSP enum, not by the catalog's
`reasoning_effort_variants`, and must map/hide `max`.

`TurnStartResult` (`msp.d.ts:1547–1558`): `{commandId, status, turnId, startedNewTurn,
disposition}` where `disposition: TurnStartDisposition = "started" | "queued" | "steered"`.
`turnId` from the ack is **authoritative** — never derive it. Live:

```json
{"commandId":"01a081ee-70cd-7e64-b362-5922d6da6b51","status":"accepted",
 "turnId":"01a081ee-70cd-7e64-b362-5922d6da6b51","startedNewTurn":false,"disposition":"queued"}
```

**`turn/steer`** (`msp.d.ts:1575–1586`): `commandId`, `sessionId`, `expectedTurnId`
(**required** — closes the race where the turn completes or is replaced between your read and
your steer; input meant for turn A can never leak into turn B), `input`, `reasoningEffort?`
(applies forward, never backward). The steered `userMessage` item carries `steered: true`.
No `turn/started` is emitted for a steer.

**`turn/interrupt`** (`msp.d.ts:1462–1481`) — the "user pressed stop" gesture on the runtime's
**priority lane**: `commandId`, `sessionId`, `turnId?` (omit → current foreground turn),
`retract?` (default `false`). With `retract: true`, if the turn is cancelled before any
assistant output committed, the submission is durably retracted → `turn/retracted`, and the
client may **restore the prompt text**. Acceptance ≠ stopped: the turn is over when you fold
its `turn/completed` with `terminal: "cancelled"`.

**`turn/cancel`** (`msp.d.ts:1386–1403`): same shape *without* `retract`, on the normal command
lane. Pairing a retract with a plain cancel is durably rejected `not_paired_interrupt`.
Practical guidance: **bind the stop button to `turn/interrupt`**, and use `turn/cancel` only
for a background/non-urgent cancel.

**`turn/unqueue`** (`msp.d.ts:1602–1619`): `commandId`, `sessionId`, `turnId` (**required**,
exactly as the queueing ack minted it; never "whichever is newest"). "It is **not** a stop" —
a reclaim that arrives after its target launched is durably rejected. Authoritative removal is
the `turn/unqueued` view event, which carries the queueing `commandId` so the client can
**restore the submission's text** exactly as for `turn/retracted`. No `turn/started` or
`turn/completed` is ever emitted for that `turnId`.

Turn view events:

| Notification | Params (`msp.d.ts`) | Notes |
|---|---|---|
| `turn/started` | `commandId, turnId, sessionId, sourceRange, viewCursor` (1561–1572) | fresh submits immediately, queued at their launch boundary, **never** steered |
| `turn/completed` | `turnId, terminal, error?, reason?, durationMs?, timeToFirstTokenMs?, usage?` (1406–1427) | the terminal |
| `turn/retracted` | `commandId, turnId` (1492–1503) | interrupt-paired retract won; the `userMessage` is re-emitted with `retracted: true` |
| `turn/retryScheduled` | `attempt, nextAttempt, maxAttempts, reason, retryDelayMs, turnId` (1506–1525) | **non-terminal**; render "attempt N/M · retrying in Ss · reason" instead of dead air. Derive the countdown locally — `retryDelayMs` is a backoff, not a fire time |
| `turn/unqueued` | `turnId, commandId` (1622–1633) | see above |

`TurnTerminal = "completed" | "failed" | "cancelled"` (open, `msp.d.ts:1599`).
`TurnError = {kind, message, retryable}` (`msp.d.ts:1430–1437`), present **iff** `terminal ==
"failed"` — "mid-turn failures reach the client here — never as a JSON-RPC error".
`TurnErrorKind` (`msp.d.ts:1440`): `stepLimit | configError | projectionError | logError |
workflowLaunchError | environmentError | modelError | launchError` (open).

Live `turn/completed`:

```json
{"turnId":"01a081ed-2ed2-72d3-b87e-c73e0780e714","terminal":"completed",
 "durationMs":17339,"timeToFirstTokenMs":16862,
 "sourceRange":{…},"viewCursor":"v:01a081ed-2e95-…:26"}
```

### 1.6 Items — the transcript

`ItemKind` (`msp.d.ts:601`), nine v1 kinds, **open**:
`userMessage | agentMessage | reasoning | toolCall | userShell | subagent | workflow |
reminderChild | compaction`. Clients **MUST** render an unknown kind generically: kind name +
`status` + `fallbackText`.

`ItemStatus` (`msp.d.ts:616`), open: `inProgress | completed | failed | cancelled | rejected |
timedOut`. **Terminal = anything other than `"inProgress"`**; unknown values are
terminal-unknown.

`Item` (`msp.d.ts:455–572`) is one flat struct with per-kind optional fields. Common:

- `itemId` — bare UUIDv7, identity for the whole lifecycle. Identity rule: the **task id** for
  `toolCall`/`subagent`, the **pre-minted commit `message_id`** for `agentMessage`/`reasoning`,
  the opening durable record's event id for everything else.
- `revision` — integer ≥ 1, strictly monotonic per item. **Apply rule: replace iff higher.**
  `item/delta` never bumps it.
- `kind`, `status`, `turnId` (`null` **only** for `userShell` — the one kind outside a turn),
  `recordedAt` (RFC3339; absent on ephemeral-opened items), `fallbackText` (server one-liner
  for generic rendering), `truncated` (a streamed surface saturated the server's text budget).

Per kind (field → owning kind, verbatim from the doc comments):

| Kind | Fields |
|---|---|
| `userMessage` | `text` (prompt as submitted), `displayText`, `attachments: MessageAttachment[]` (**metadata only** — `{mediaType, type:"image", width?, height?}`; base64 is never echoed back), `commandId` (multi-client UIs de-dup their local echo on it), `steered`, `retracted` |
| `agentMessage` | `text` — the accumulated reply, streamed via `item/delta` field `"text"` |
| `reasoning` | `summary: string[]` — one entry per summary part, part *n* streams via `item/delta` field `"summary.n"` (part boundary = index change); `text` is raw committed reasoning where the provider exposes it and is **never streamed in v1**; `providerItemId` (`rs_…`) |
| `toolCall` | `tool` (name), `callId` (`call_…`), `args` (**model-authored argument JSON, verbatim — clients parse**), `visibleOutput` (bounded, streams via `item/delta` field `"output"`), `outputRef: OutputRef`, `modelVisibleContent: ModelVisibleContent[]`, `approvalId` (the approval that gated it — the join key), `failureKind` (`TaskFailureKind`, snake_case verbatim), `failureReason`, `background`, `backgroundInitiator` |
| `userShell` | `commandText`, `visibleOutput`, `outputRef`, `exitCode`, `exitSignal` (the **number**, e.g. 9 — nothing maps it to a name), `durationMs`, `commandId`, `turnId: null` |
| `subagent` | `subagentId`, `childSessionId` (readable via `session/read`/`view/page` — child drill-down with no second protocol), `agentPath`, `role`, `objective`, `depth`, `controlStatus: SubagentControlStatus`, `result: SubagentResult`, `usage` (**transitive** — child + descendants, never folded into `session/tokenUsage.cumulative`), `workflowRunId`, `durationMs` |
| `workflow` | `entryId`, `scriptId`, `runId` via `workflowRunId`, `resumeFromRunId`, `triggerSource` (e.g. `"modelProposal"`), `children: WorkflowChild[]` (keyed by `(childId, attempt)`, **re-emitted whole** on every change; `revision` orders), `message` |
| `reminderChild` | `reminderAgentId`, `generationId`, `taskId`, `childSessionId`, `childSessionLogPath` |
| `compaction` | `trigger: CompactionTrigger` (`manual`/`auto`), `outcome: CompactionOutcome`, `strategyId`, `reason` (snake_case, e.g. `"no_compactable_history"`), `tokensBefore`, `tokensAfter`, `summarizedThrough` (**opaque compaction anchor** — accepted by `session/resume.cursor` and `view/page.anchor`; relay, never parse) |

`OutputRef` (`msp.d.ts:748–765`): `{id, uri, kind, byteLen, availability, digest?, mediaType?,
path?}` where `availability: "available" | "missing" | "unsupported" | "accessFailed"`.
**The fetch path `item/readOutput` is not on the 1.0.3 method index** — so treat `outputRef`
as a "N KB stored, open externally" affordance, not something the app can page. In every live
capture `outputRef` was `null` and the whole output fitted in `visibleOutput`.

The four item notifications:

- **`item/started`** (`msp.d.ts:604–613`) — `{item, sessionId, viewCursor, sourceRange?}`; the
  full item at revision 1. `sourceRange` is **absent** on an ephemeral-sourced open (the
  delta-streamed kinds).
- **`item/delta`** (`msp.d.ts:587–598`) — `{itemId, delta, field?, sessionId, viewCursor}`.
  **No `sourceRange` at all.** `field` is a dotted path; **absent means `"text"`**.
  Concatenating a field's deltas in cursor order equals that field's value on
  `item/completed`, *unless the surface saturated* (then `truncated: true`).
  **Deltas never bump `revision`.**
- **`item/updated`** (`msp.d.ts:619–628`) — full item at a higher revision, for non-terminal
  changes deltas cannot express (a tool call being backgrounded, a retracted user message).
- **`item/completed`** (`msp.d.ts:575–584`) — the authoritative terminal object, always with a
  durable `sourceRange`. **Clients MUST accept `item/completed` for an `itemId` they never
  saw `item/started` for** (single-shot kinds, after a gap fill).

Streaming, exactly as captured (echo provider, `transcript-echo-try.jsonl`):

```json
{"method":"item/started","params":{"sessionId":"01a081e5-dbbf-…","viewCursor":"…:4",
  "item":{"itemId":"9e7bcf36-b550-407c-87db-d784f7ba7015","kind":"agentMessage",
          "turnId":"01a081e5-dbfb-…","revision":1,"status":"inProgress","text":""}}}
{"method":"item/delta","params":{"sessionId":"…","viewCursor":"…:5",
  "itemId":"9e7bcf36-…","field":"text","delta":"Hi."}}
{"method":"item/completed","params":{…,"item":{"itemId":"9e7bcf36-…","kind":"agentMessage",
  "revision":2,"status":"completed","text":"Hi."}}}
```

and against the real provider the deltas were word-sized:

```
"delta":"Hello — Muse"   "delta":" Code here"   "delta":"."
"delta":" What"          "delta":" do"          "delta":" you need"   "delta":"?"
```

A `toolCall` (real provider): `args` arrived **whole on `item/started`** — it was *not*
streamed — and the output arrived whole on `item/completed`:

```json
{"method":"item/started","params":{…,"item":{"itemId":"…","kind":"toolCall","tool":"bash",
  "revision":1,"status":"inProgress",
  "args":"{\"command\":\"ls\",\"description\":\"List workspace files\"}"}}}
{"method":"item/completed","params":{…,"item":{…,"revision":2,"status":"completed",
  "visibleOutput":"README.md\nnotes.txt\n"}}}
```

Design note for the adapter: **`args` is a verbatim string, not an object** — parse it
per-tool and fall back to raw display when it is not valid JSON ("survives model-emitted
almost-JSON"). The observed tool names are `bash` and `request_user_input`; there is no tool
*kind* taxonomy on the wire — the app must own the `tool` name → card mapping.

### 1.7 Approvals

Two delivery paths, both observed:

- **`approval/requested`** notification — "the fold of its durable Requested record".
- **`approval/request`** server→client **request** (carries an `id`). The d.ts describes these
  as "re-issued server-to-client requests right after a `session/resume` response"
  (`msp.d.ts:783`), but the sibling `userInput/request` was observed arriving on a *fresh*
  turn as well (§1.8). Handle both; **always answer via `approval/decide`**, never by
  responding to the request frame.

`ApprovalRequestParams` (`msp.d.ts:98–114`) — full live capture (`transcript-wire.jsonl`,
`session/userShell "echo hi && ls"` under `denyUnmatched`):

```json
{"sessionId":"01a081ee-3995-…",
 "approvalId":"c44e8673-7ddd-46a9-934a-d9d50fc449f4",
 "turnId":"01a081ee-39d1-…","taskId":"c44e8673-…","itemId":"c44e8673-…",
 "toolCallId":"user_shell_01a081ee-39d1-…","toolName":"shell",
 "rawArgs":"{\"command\":\"echo hi && ls\"}",
 "viewCursor":"v:01a081ee-3995-…:3","sourceRange":{…},
 "subject":{"kind":"shell","command":"echo hi && ls","workspaceRoot":"/…/ws",
   "stages":[
     {"requirementId":{"approvalId":"c44e8673-…","sourceIndex":0},
      "position":1,"totalStages":2,"argv":["echo","hi"],"argvComplete":true,
      "resolution":{"kind":"unresolved"},
      "suggestedPrefix":{"argvPrefix":["echo"],
                         "label":"Always allow in this workspace: echo ..."}},
     {"requirementId":{"approvalId":"c44e8673-…","sourceIndex":1},
      "position":2,"totalStages":2,"argv":["ls"],"argvComplete":true,
      "resolution":{"kind":"unresolved"},
      "suggestedPrefix":{"argvPrefix":["ls"],
                         "label":"Always allow in this workspace: ls ..."}}]},
 "currentRequirementId":{"approvalId":"c44e8673-…","sourceIndex":0},
 "availableChoices":[
   {"choiceId":"allow_once","label":"Allow once","decision":"approved","scope":"once"},
   {"choiceId":"allow_local_prefix","label":"Always allow in this workspace: echo ...",
    "decision":"approvedPolicyAmendment","scope":"localPersistent",
    "rulePreview":"Always allow in this workspace: echo ..."},
   {"choiceId":"abort","label":"Reject","decision":"abort","scope":"once",
    "acceptsFeedback":true}],
 "protectedWrite":false,"judgeEscalated":false}
```

- `ApprovalSubject` (`msp.d.ts:172–186`) is an **open** flat union:
  `kind: shell | fileAccess | network | process | tool` plus `command?`, `path?`, `host?`,
  `port?`, `protocol?`, `access?`, `target?`, `toolName?`, `workspaceRoot?`,
  `origin?: {kind, command?, url?}`, `stages?`. Unknown kinds "are rendered generically and
  **never auto-approved by clients**".
- `ApprovalStage` (`msp.d.ts:147–155`): `argv`, `argvComplete`, `position`, `totalStages`,
  `requirementId`, `resolution`, `suggestedPrefix`. **A pipeline is staged**: `echo hi && ls`
  is two stages, decided one at a time.
- `ApprovalChoice` (`msp.d.ts:23–30`): `choiceId`, `label`, `decision: ApprovalDecision`,
  `scope: "once" | "session" | "localPersistent"`, `rulePreview?`, `acceptsFeedback?`.
  **The choices are server-minted; the UI must render what it is given, not a fixed
  allow/always/deny triad.** Only a choice with `acceptsFeedback` may carry `feedback`.
- `ApprovalDecision` (`msp.d.ts:62`): `approved | approvedForSession |
  approvedPolicyAmendment | denied | deniedPolicyAmendment | timedOut | abort`.
- `protectedWrite`, `judgeEscalated` — badge these (a judge-escalated request means the LLM
  approval judge kicked it up to the human).

**`approval/decide`** (`msp.d.ts:35–48`): `commandId`, `sessionId`, `approvalId`, `choiceId`
(one of the *current* `availableChoices`, else `-32052`), `requirementId` (**must equal
`currentRequirementId`** — the multi-stage race guard; stale → `-32053`), `feedback?`
(free-text guidance delivered to the model with a denial; **never persisted**, so it never
appears in `approval/resolved`).
Result (`msp.d.ts:51–60`): `{approvalId, commandId, status, terminal}` where `terminal` is
`false` when the choice satisfied a stage but further requirements remain.

The multi-stage flow, captured live (`transcript-approve.jsonl`, `promptUnmatched`):

```
approval/requested  currentRequirementId.sourceIndex = 0   (stage 1/2, argv ["echo","hi"])
→ approval/decide  choiceId "allow_once"  → {"terminal": false}
approval/updated    currentRequirementId.sourceIndex = 1   (stage 2/2, argv ["ls"])
                    change: {"kind":"stageResolved",
                             "requirementId":{…,"sourceIndex":0},
                             "choiceId":"allow_once","decision":"approved"}
→ approval/decide  choiceId "allow_once"  → {"terminal": true}
approval/updated    change: {"kind":"stageResolved", …"sourceIndex":1…}
approval/resolved   decision "approved"  resolvedBy "user"
```

**`approval/updated`** (`msp.d.ts:194–203`): the refreshed pending view plus one `change:
ApprovalChange` (`msp.d.ts:12–21`: `kind`, `choiceId?`, `decision?`, `requirementId?`,
`reason?`, `executable?`, `reparsed?`, `status?: "succeeded"|"failed"`). Note the choices
**change between stages** (the `rulePreview` went from `echo ...` to `ls ...`) — re-render
from the update, do not cache.

**`approval/resolved`** (`msp.d.ts:132–145`): `{approvalId, itemId, turnId, decision,
policyResult: "allow"|"deny", resolvedBy: "user"|"policy"|"llmJudge", stageEvidence[],
amendment?: {durability, rulePreview}, decidedByCommandId?}`. A **policy** resolution arrives
with no user interaction at all:

```json
{"approvalId":"de37d859-…","decision":"denied","policyResult":"deny","resolvedBy":"policy",
 "stageEvidence":[{"requirementId":{…},"position":1,"totalStages":1,
                   "argv":["curl","https://example.com"],"resolution":{"kind":"unresolved"}}]}
```

…and the gated item then completes as `status: "rejected"`:

```json
{"itemId":"e9a98861-…","kind":"userShell","revision":2,"status":"rejected",
 "visibleOutput":"tool denied: deny_unmatched: no policy rule allows this action",
 "commandText":"curl https://example.com","durationMs":15}
```

So the UI must be able to render an approval card that was **never actionable** — it opened
and resolved by policy in the same breath. `resolvedBy: "llmJudge"` is the third case
(`--approval-judge on` is the default).

**`approval/listPending`** (`msp.d.ts:65–76`): `{sessionId}` → `{approvals:
ApprovalRequestParams[], userInputs: UserInputRequestParams[]}` — the pull dual of the
re-issued requests, lease-free, works on unloaded sessions, never subscribes. Ordering is by
opening `viewCursor`. Live on an idle session: `{"approvals":[],"userInputs":[]}`.

**Approval modes.** `ApprovalMode` (`msp.d.ts:79`) is **closed**:
`allowAll | promptUnmatched | onRequest | denyUnmatched`. "Select, never create" — a client
selects a preconfigured mode and can never construct one, supply inline rules, or describe a
policy on the wire. Set it at `session/start.approvalMode` or mid-session with
**`session/setApprovalMode`** (`msp.d.ts:1091–1110`): `{commandId, sessionId, mode}` →
`{applyOutcome: "completed"|"noop", effectiveMode: EffectiveApprovalModeState, commandId,
status}`. **Applies next-action** — an in-flight pending approval is not retroactively decided.
`EffectiveApprovalModeState = {mode, source: "startup"|"replay"|"approvalReconfigure",
lastCommandId}`. The `session/approvalModeChanged` notification adds `clientName`.

Observed behaviour: under `onRequest` a plain `ls` from the model ran with **no approval at
all**; under `promptUnmatched` and `denyUnmatched` the same command raised one. The CLI's
`--approval-mode untrusted|on-request|never` maps onto (roughly) `denyUnmatched`/
`promptUnmatched` · `onRequest` · `allowAll`.

### 1.8 User input (the agent asks the person)

Same dual delivery. Captured live on the real provider — **as a server→client request**:

```json
{"jsonrpc":"2.0","id":1,"method":"userInput/request","params":{
  "sessionId":"01a081ed-2e95-…","userInputId":"28da2a48-266e-497f-8c9e-6120288496c9",
  "turnId":"01a081ed-2ed2-…","itemId":"28da2a48-…",
  "toolCallId":"call_01a081ed5b1576b19c1f3970681062a5","toolName":"request_user_input",
  "viewCursor":"v:01a081ed-2e95-…:13","sourceRange":{…},
  "questions":[{"id":"file_choice","header":"File",
                "question":"Which of the listed files do you want described?",
                "selection":{"mode":"single"},
                "options":[{"label":"README.md"},{"label":"notes.txt"}]}]}}
```

…and the identical payload again as the `userInput/requested` notification.

`UserInputRequestParams` (`msp.d.ts:1754–1765`): `userInputId`, `itemId`, `turnId`,
`toolCallId`, `toolName`, `questions: UserInputQuestion[]`, `autoResolutionMs?`
(**a timeout the UI should show as a countdown**), `sourceRange?`, `viewCursor`.

`UserInputQuestion` (`msp.d.ts:1745–1751`): `id`, `header` (a short label — `"File"`),
`question` (the prompt), `options: UserInputOption[]`, `selection: UserInputSelection`.
`UserInputOption` (`msp.d.ts:1732–1736`): `label`, `description?`,
`preview?: {content, format}` — **an option can carry a rendered preview** (e.g. a diff or a
snippet) that the card should be able to expand.
`UserInputSelection` (`msp.d.ts:1767–1771`): `mode: "single" | "multiple"` (closed),
`minSelections?`, `maxSelections?`.

Three settlement methods, all `{commandId, sessionId, userInputId, …}`:

- **`userInput/answer`** (`msp.d.ts:1662–1671`): `answers: UserInputAnswer[]`, one per
  question. Each carries `questionId` then **exactly one of** `selectedLabel` (single),
  `selectedLabels` (multiple, within min/max) or `freeText` (≤ 500 chars), plus optional
  `note` (≤ 500). Mismatches → `-32057 userInputAnswerInvalid`. **Image `attachments` are a
  reserved field, rejected if sent.** Note the answer keys on the option **label**, not an id.
- **`userInput/cancel`** (`msp.d.ts:1684–1693`): `reason?`. The tool call resolves with a
  cancelled result the model sees.
- **`userInput/clarify`** (`msp.d.ts:1711–1720`): `clarification: {content (≤500), format
  ("text" in v1)}` — the "let me explain instead of picking" path; the model receives the
  text and **re-decides**. This is a first-class UI affordance the aui question card does not
  have today.

**`userInput/settled`** (`msp.d.ts:1776–1786`): `{userInputId, outcome, answers[],
clarification|null, reason|null, decidedByCommandId|null, sourceRange, viewCursor}`.
`UserInputOutcome` (`msp.d.ts:1743`): `answered | cancelled | interrupted | clarified |
timedOut | aborted`.

The gating `toolCall` item then completes with the answer echoed into `visibleOutput`:

```json
{"kind":"toolCall","tool":"request_user_input","revision":2,"status":"completed",
 "visibleOutput":"{\"status\":\"answered\",\"answers\":[{\"id\":\"file_choice\",\"selected_label\":\"README.md\"}]}"}
```

### 1.9 Models

**`model/list`** (`msp.d.ts:690–705`) — "a query, not a command": no `commandId`, no durable
record, no view event. Params `{sessionId?}` — with a session, the row matching its effective
model is flagged `isActive`. Result `{models: ModelCatalogEntry[], providerId, profileId,
source: ModelCatalogSource}` where source is `providerCatalog | fakeCatalog |
unresolvedCatalog | bundledCatalog | configCatalog` — "how a client tells a live provider
catalog from a test fake and labels it honestly". `models` MAY be empty. A snapshot at call
time; **v1 has no catalog subscription**.

`ModelCatalogEntry` (`msp.d.ts:646–669`): `modelId`, `displayLabel`, `providerId`,
`profileId | null`, `releaseDate | null`, `description | null`, `contextLimit | null`,
`outputLimit | null`, `cost: ModelCost | null`, `isActive`, `isDefault`. Rows are ordered
**newest `releaseDate` first**, hidden rows never reach the wire, and a client
**MUST NOT assume exactly one `isActive` or `isDefault`**.

`ModelCost` (`msp.d.ts:678–687`): `{input, output, cached, currency|null}` — **decimal
strings, per 1M tokens, verbatim**; "cost arithmetic stays client-local view math".

Live (`model/list` with no session):

```json
{"providerId":"meta","profileId":"tbh","source":"providerCatalog","models":[
 {"modelId":"muse-spark-1.3","displayLabel":"muse-spark-1.3","providerId":"meta",
  "profileId":"tbh","releaseDate":"2026-09-02","description":null,
  "contextLimit":1007997,"outputLimit":128000,"cost":null,"isActive":false,"isDefault":false},
 {"modelId":"muse-spark-1.3-contributor","displayLabel":"muse-spark-1.3-contributor",…,
  "description":"Your content, including inter-session messages, may be used for product improvement.",
  "isActive":false,"isDefault":true},
 {"modelId":"muse-spark-1.2",…,"releaseDate":"2026-08-05",…},
 {"modelId":"muse-spark-1.2-contributor",…}]}
```

⚠️ **`cost` is `null` for every row** on this subscription, and `model/list` **ignores
`providerId`** — an echo session still gets the meta catalog. So a cost footer must degrade
gracefully, and the picker must not promise per-model pricing.

⚠️ **Supported reasoning efforts are NOT on the wire.** `ModelCatalogEntry` has no effort
field. They exist only in the on-disk catalog cache
(`~/.local/share/muse/model-catalog/6d657461__p746268.json`), as
`rows[].reasoning_effort_variants: [{tier, description}]` — for `muse-spark-1.3`:
`minimal, low, medium, high, xhigh, max`. Since `max` is not an MSP `ReasoningEffort`, the
effort picker should simply offer the MSP enum minus what the model rejects.

**`session/setModel`** (`msp.d.ts:1113–1128`): `{commandId, sessionId, model: ModelSelection}`.
`ModelSelection` (`msp.d.ts:708–717`): `modelId` (**required**), `providerId?`,
`profileId? (null == absent)`, `displayLabel?`. Empty strings normalize to absent. The
selection is durable and applies to subsequent model calls; if a turn is running it is
"admitted now and applied at the next model-call boundary". Result is admission only.
Live on an echo session it is refused, which is worth handling:

```json
{"code":-32030,"message":"session/setModel command 01a081ee-… rejected: unsupported_route",
 "data":{"kind":"commandRejected","retryable":false,"commandId":"…","reason":"unsupported_route"}}
```

**`session/modelChanged`** (`msp.d.ts:1029–1042`): `{modelId, providerId?, source:
ModelChangeSource ("user"|"default"|"policy")}`.

### 1.10 Context, usage, compaction

**`session/tokenUsage`** (`msp.d.ts:1176–1199`) — one per model completion that reports usage:

```json
{"turnId":"01a081e5-3401-…","modelId":"muse-spark-1.3-contributor",
 "usage":{"inputTokens":19213,"outputTokens":115,"cachedTokens":0,
          "cacheWriteTokens":0,"cacheReadTokens":0,"reasoningTokens":94},
 "promptTokens":19213,"totalTokens":19328,"durationMs":5027,
 "cumulative":{"promptTokens":19213,"outputTokens":115,"totalTokens":19328}}
```

`usage: TokenUsage` is the raw provider counters, **not summable across providers**;
`promptTokens` is the server-derived counted-once value ("clients display and sum these and
never re-derive the provider's cache convention"); `cumulative: CumulativeTokenUsage` is the
session running total and **never goes backward**. `finishReason?` is verbatim and open.
Subagent/workflow-child usage is **never** folded into `cumulative` — it rides the owning item's
`usage`, so a "total spend" figure must add those explicitly.

**`session/contextUsage`** (`msp.d.ts:942–955`) — replace wholesale, emitted only when the
`(windowTokens, usedTokens, pressure)` triple **changes value**:

```json
{"windowTokens":1007997,"usedTokens":19328,"pressure":"normal"}
```

`ContextPressureLevel` (`msp.d.ts:265`): `normal | warning | blocked` — "hard threshold first,
both inclusive `>=`". `windowTokens` is **absent when the basis has no limit** ("the limit
part is omitted, never invented"), so the meter must handle "used, no denominator".
`blocked` is the state where the composer should refuse to send and offer compaction.

**`session/compact`** (`msp.d.ts:918–935`): `{commandId, sessionId, turnId?}` — the `/compact`
gesture. Omit `turnId` and the server resolves the current/latest run; no resolvable run →
`commandRejected` reason `missing_run`. It is **the one method that may ack `"noop"`**
(`CompactStatus = "accepted" | "noop"`), with `reason` (e.g. `no_compactable_history`). "A noop
is a success, not an error." Failures *after* admission (`summarizer_failed`,
`install_rejected`, `cancelled`) are **not wire errors** — they arrive as the compaction item's
terminal view event.

Compaction is observed as a **`compaction` item** (§1.6), not a bespoke notification:
`trigger: "manual"|"auto"`, `outcome: CompactionOutcome = compacted | noop | failed |
cancelled`, `strategyId`, `reason`, `tokensBefore`/`tokensAfter`, `summarizedThrough`.
That is exactly the "context compacted — 128k → 22k" marker the transcript should draw, and
`summarizedThrough` is the anchor a later cold resume passes to `session/resume.cursor` or
`view/page.anchor: "latestCompaction"`.

### 1.11 Todo, goal, branch

**`session/todoListChanged`** (`msp.d.ts:1160–1173`): `{items: TodoItem[], revision,
sourceTool, sessionId, sourceRange, viewCursor}`. **Replace the whole list every event; an
empty `items` array is a cleared list, not a no-op.** `revision` is diagnostics only —
`viewCursor` orders. `TodoItem` (`msp.d.ts:1339–1346`): `{text, status, activeForm?}`,
`TodoStatus = pending | inProgress | completed | cancelled`.

**`session/goalChanged`** (`msp.d.ts:985–994`): `{goal?: Goal}` — replace wholesale, an
explicit `null` **clears**, `null` never means unchanged. `Goal` (`msp.d.ts:399–410`):
`{objective, status (free string, verbatim), percentComplete (verbatim, >100 passes through —
display clamping is the renderer's job), currentWork?, nextWork?}`. This is the TUI's `/goal`
feature; it has **no counterpart in `aui-protocol`** today.

**`session/branchChanged`** (`msp.d.ts:902–915`): `{workspaceRoot, branch? (null on a
detached-HEAD observation — a fact, not "unchanged"), vcs?: "git"|"sapling"}`. Fired on the
first turn even in a non-repo workspace. Maps onto `Session::branch`.

### 1.12 User shell

**`session/userShell`** (`msp.d.ts:1202–1217`): `{commandId, sessionId, commandText}` — the
TUI's `!` escape hatch. **Capability-gated**: the connection must have negotiated `userShell`
at `initialize`, else `-32010 capabilityRequired` with `data.capability`. The result is
immediate admission; output arrives as the `userShell` item's terminal view event. It is
**outside any turn** (`turnId: null`) and still goes through the approval policy — the free
approval captures in §1.7 came from exactly this path.

### 1.13 Subagents and workflows

`subagent/*` (8 methods, `msp.d.ts:1279–1326`) all take `{commandId, sessionId, subagentId}`;
`sendMessage`/`followupTask` add `body` (trimmed, rejected when empty);
`interrupt`/`stop`/`close` add `reason?`. `readResult`, `resume`, `reopen` take the bare target.
What a client renders: the `subagent` item's `role`, `objective`, `depth`, `controlStatus`
(`accepted | starting | running | resultReady | closing | closed | recoveryPending |
manualReconciliation`), transitive `usage`, and `result: SubagentResult` (`msp.d.ts:1303–1316`:
`summary` ≤512 chars, `text?` ≤32 KiB, `artifactRefs[]`, `evidenceRefs[]`, `structuredData?`,
`errorKind?`). `childSessionId` lets a card drill into the child transcript with
`session/read` / `view/page` — a natural "open subagent" affordance.

`workflow` items carry `children: WorkflowChild[]` (`msp.d.ts:1866–1885`: `childId`, `attempt`,
`status`, `phase?`, `label?`, `terminal?`, `durationMs?`, `usage?`, `resultRef?`) re-emitted
whole on each change — render as a small table of child rows. **There are no `workflow/*`
methods in 1.0.3**, so a client can only observe workflows, never drive them.

### 1.14 Error table

Codes come from `msp.schema.json`'s `errors` array; `error.data.kind` (`ErrorKind`,
`msp.d.ts:358`) is the stable camelCase category to branch on — **`message` is never a branch
point**. `error.data.retryable`, when present, **overrides** the table default.

| Code | `kind` | Retryable | Override kinds | Extra `data` | UI treatment |
|---|---|---|---|---|---|
| -32700 | `parseError` | no | | | **dialog** — the connection is broken; restart the child |
| -32600 | `invalidRequest` | no | `notInitialized`, `alreadyInitialized` | | **dialog** (client bug / handshake) |
| -32601 | `methodNotFound` | no | `experimentalRequired` | `descriptor` | **dialog** — server older than the app |
| -32602 | `invalidParams` | no | `experimentalRequired` | `alignedNextOffset` | **dialog** in dev, **inline** for a user-facing bad value (bad effort tier) |
| -32603 | `internal` | no | `pageEventTooLarge`, `outputResultTooLarge` | `viewCursor` | **dialog** |
| -32001 | `overloaded` | **yes** | | `retryable` | **inline** banner with auto-retry |
| -32002 | `inputTooLarge` | no | | `limitBytes` | **inline** on the composer |
| -32010 | `capabilityRequired` | no | | `capability` | **inline** (disable the `!` shell affordance) |
| -32011 | `notFound` | no | | `anchor`, `reason` (e.g. `missingAnchor`) | **inline** |
| -32013 | `interrupted` | — | | | **inline** / silent |
| -32014 | `cancelled` | — | | | silent |
| -32020 | `sessionNotFound` | no | | `sessionId` | **dialog** on open, **inline** in a list |
| -32021 | `sessionInUse` | no | | `sessionId` | **dialog** — "open in another window" |
| -32022 | `sessionAmbiguous` | no | | `paths` | **dialog** with a chooser |
| -32023 | `forkBoundaryInvalid` | no | | `lastTurnId` | **inline** on the fork affordance |
| -32024 | `sessionNotLoaded` | no | | `sessionId` | **inline**; resume first |
| -32025 | `sessionStreamMismatch` | no | | | **dialog** |
| -32030 | `commandRejected` | no | | `commandId`, `reason` | **inline** on the control that sent it (`unsupported_route`, `missing_run`, `session_id_conflict`, `not_paired_interrupt`) |
| -32031 | `backpressured` | **yes** | | `capacity` | silent, retry with backoff |
| -32040 | `viewTruncated` | no | | `earliestCursor` (nullable, always present) | silent → re-anchor |
| -32042 | `boundaryPruned` | no | `boundaryUnusable`, `noBoundary` | `anchor`, `latestBoundaryCursor` | silent → fall back to a plain forward page |
| -32050 | `approvalNotFound` | no | | `approvalId` | **inline** — dismiss the card |
| -32051 | `approvalAlreadyResolved` | no | | `resolution: {decision, resolvedBy, viewCursor}` | **inline** — re-render the card as resolved |
| -32052 | `approvalChoiceInvalid` | no | | `choiceId` | **inline** — re-render choices |
| -32053 | `approvalRequirementStale` | no | | `currentRequirementId` | silent — re-render at the new stage |
| -32054 | `approvalReviewerUnavailable` | no | | | **inline** banner |
| -32055 | `userInputNotFound` | no | | `userInputId` | **inline** |
| -32056 | `userInputAlreadySettled` | no | | `settlement: {outcome, viewCursor}` | **inline** |
| -32057 | `userInputAnswerInvalid` | no | | `userInputId` | **inline** on the question card |

Reserved and never emitted by a v1 host: `-32060..-32069` (SS6 raw log, deferred) and `-32012`
(deliberately unassigned; a command-id conflict is `commandRejected`).

Rule of thumb: **dialog** for anything that means the connection or the app is wrong (`-32700`,
`-32600`, `-32601`, `-32603`, `-3202x` session identity); **inline** for anything a person can
act on where it happened; **silent + recover** for the cursor/backpressure family.
Live samples:

```json
{"code":-32020,"message":"session … was not found","data":{"kind":"sessionNotFound","retryable":false,"sessionId":"…"}}
{"code":-32601,"message":"method not found","data":{"kind":"methodNotFound"}}
{"code":-32051,"message":"approval … is already resolved",
 "data":{"kind":"approvalAlreadyResolved","retryable":false,"approvalId":"…",
         "resolution":{"decision":"approved","resolvedBy":"user","viewCursor":"…:5"}}}
```

---

## 2. Live probe — captured transcripts

Client: `scratchpad/probe.py` (≈120 lines of Python; spawns `muse serve`, newline framing,
UUIDv7 minting, request/notification demux, logs every raw line prefixed `-->` / `<--`).

### 2.1 Framing and handshake (verbatim, `transcript-echo-try.jsonl` lines 1–4)

```
--> {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientInfo": {"name": "aui_probe", "version": "0.1.0"}}}
<-- {"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"muse","version":"1.0.3"},"userAgent":"muse-build/1.0.3 (non-interactive; macos-aarch64; build 238bb03ff370497df820cffe572ee9b4f76ec5b1)","museHome":"/Users/…/.local/share/muse","platformFamily":"unix","platformOs":"macos","schema":{"version":1,"fingerprint":"sha256:03312c…758b7"},"grantedCapabilities":["userShell"],"experimentalApi":false,"sessionDurability":"durable"}}
--> {"jsonrpc": "2.0", "method": "initialized"}
```

### 2.2 Provider is per session

```
--> {"jsonrpc": "2.0", "id": 2, "method": "session/start", "params": {"providerId": "echo", "commandId": "01a081e5-dbbf-791b-b454-730b7c6c835c", "workspaceRoot": "/…/scratchpad/ws"}}
<-- {"jsonrpc":"2.0","method":"session/started","params":{"session":{"sessionId":"01a081e5-dbbf-7201-9320-1a3448bf5caa",…}}}
<-- {"jsonrpc":"2.0","id":2,"result":{"session":{"sessionId":"01a081e5-dbbf-7201-9320-1a3448bf5caa","path":"/Users/…/sessions/2026/09/08/01a081e5-…/session.jsonl","status":"idle","activeTurnId":null,…},"viewCursor":"v:01a081e5-dbbf-7201-9320-1a3448bf5caa:1"}}
```

The durable log confirms it took effect:
`{"payload_type":"runtime.session.metadata","payload":{"kind":"metadata","record":{"workspace_root":"/…/ws","provider_id":"echo"}}}`.
`--echo-delay-ms` has no MSP equivalent, and `model/list` still returns the meta catalog for an
echo session.

### 2.3 A whole echo turn

```
--> {"jsonrpc": "2.0", "id": 4, "method": "turn/start", "params": {"commandId": "01a081e5-dbfb-744c-820f-82e3a51f5353", "sessionId": "01a081e5-dbbf-…", "input": [{"type": "text", "text": "say hi"}]}}
<-- {"jsonrpc":"2.0","method":"session/branchChanged","params":{…,"viewCursor":"…:1","workspaceRoot":"/…/ws"}}
<-- {"jsonrpc":"2.0","id":4,"result":{"commandId":"01a081e5-dbfb-…","status":"accepted","turnId":"01a081e5-dbfb-…","startedNewTurn":true,"disposition":"started"}}
<-- {"jsonrpc":"2.0","method":"turn/started","params":{…,"viewCursor":"…:2","turnId":"01a081e5-dbfb-…"}}
<-- {"jsonrpc":"2.0","method":"item/completed","params":{…,"viewCursor":"…:3","item":{"kind":"userMessage",…}}}
<-- {"jsonrpc":"2.0","method":"item/started","params":{…,"viewCursor":"…:4","item":{"itemId":"9e7bcf36-b550-407c-87db-d784f7ba7015","kind":"agentMessage","turnId":"01a081e5-dbfb-…","revision":1,"status":"inProgress","text":""}}}
<-- {"jsonrpc":"2.0","method":"item/delta","params":{…,"viewCursor":"…:5","itemId":"9e7bcf36-…","field":"text","delta":"Hi."}}
<-- {"jsonrpc":"2.0","method":"item/completed","params":{…,"item":{"itemId":"9e7bcf36-…","revision":2,"status":"completed","text":"Hi."}}}
<-- {"jsonrpc":"2.0","method":"turn/completed","params":{…,"turnId":"01a081e5-dbfb-…","terminal":"completed"}}
```

Note the ordering: **the `userMessage` `item/completed` arrives after `turn/started`**, and the
`turn/start` response can arrive *after* view events for the same turn. Do not assume ack
precedes stream.

### 2.4 What echo cannot exercise

The echo provider emits **one canned `agentMessage` and nothing else** — no `reasoning`, no
`toolCall`, no `approval/*`, no `userInput/*`, no `session/tokenUsage`, no
`session/contextUsage`. It is fine for framing, ids, delta shape, queueing and interrupts, and
useless for cards.

**Approvals and user shell can still be exercised for free**, because `session/userShell` is a
user-initiated command that goes through the same approval policy without any model call. That
is how the multi-stage approval capture in §1.7 was obtained. Approvals against a *model* tool
call, and every `reasoning` item, require the real provider.

### 2.5 The rest of the command plane (echo session, `transcript-wire.jsonl`)

```
--- userShell:        {"commandId":"…","status":"accepted"}
--- turn1:            {"status":"accepted","turnId":"01a081ee-70bb-…","startedNewTurn":true,"disposition":"started"}
--- turn2 (ifBusy=queue): {"status":"accepted","turnId":"01a081ee-70cd-…","startedNewTurn":false,"disposition":"queued"}
--- unqueue t2:       {"status":"accepted","turnId":"01a081ee-70cd-…"}   → notification turn/unqueued
--- interrupt retract:{"status":"accepted","turnId":"01a081ee-70bb-…"}   → notification turn/retracted
--- compact:          {"commandId":"…","status":"accepted"}
--- fork:             {"session":{…,"forkedFrom":{"sessionId":"01a081ee-3995-…","cutCursor":"session:01a081ee-3995-…:turn:0","cutExplicit":false,"commandId":"…"}},…}
--- listPending:      {"approvals":[],"userInputs":[]}
--- unsubscribe:      {}
--- read (excludeItems):  history {"mode":"none","noneReason":"excluded"}
--- resume (history auto): history {"mode":"inline","items":[…]}
```

Notification kinds seen on that one connection: `approval/requested`, `approval/resolved`,
`item/started`, `item/updated`, `item/completed`, `session/approvalModeChanged`,
`session/branchChanged`, `session/started`, `turn/started`, `turn/completed`,
`turn/retracted`, `turn/unqueued`.

### 2.6 The real-provider turn

Budget honoured: **two turns against `providerId: "meta"`** (one warm-up "hello", one scripted
tool-call + question turn). Prompt: *"Run the shell command `ls` in the workspace, then use
your question tool to ask me which of the listed files I want described."*, `approvalMode:
"onRequest"`, `reasoningEffort: "low"`, in a scratch git repo containing `README.md` and
`notes.txt`.

Sequence (`transcript-real.jsonl`, 29 notifications):

```
session/started
session/approvalModeChanged  mode "onRequest" source "approvalReconfigure"
turn/started
item/completed  userMessage
item/started    toolCall  tool "bash"  args {"command":"ls","description":"List workspace files"}
item/completed  toolCall  status "completed"  visibleOutput "README.md\nnotes.txt\n"
item/started    toolCall  tool "request_user_input"  args {"questions":[…]}
userInput/request   (server→client REQUEST, id 1)
userInput/requested (notification, identical params)
→ userInput/answer  {"answers":[{"questionId":"file_choice","selectedLabel":"README.md"}]}
                    → {"status":"accepted","userInputId":"28da2a48-…"}
userInput/settled
item/completed  toolCall request_user_input  visibleOutput {"status":"answered","answers":[{"id":"file_choice","selected_label":"README.md"}]}
item/started    agentMessage  → item/delta ×N → item/completed
session/tokenUsage  {"inputTokens":19213,"outputTokens":115,"reasoningTokens":94,…}
session/contextUsage {"windowTokens":1007997,"usedTokens":19328,"pressure":"normal"}
turn/completed  terminal "completed" durationMs 17339 timeToFirstTokenMs 16862
```

**No `approval/*` fired for `ls`** — under `onRequest` the policy allowed it outright. That is
itself the finding: `onRequest` is not "ask for everything". The approval captures in §1.7 came
from `promptUnmatched`/`denyUnmatched` via `session/userShell`, which cost nothing.

Also observed: **no `reasoning` item at all** on this turn despite `reasoningTokens: 94` — the
provider billed reasoning but exposed no summary parts. A reasoning-block UI must tolerate
"thought for N tokens, no text".

---

## 3. Auth

### 3.1 Two distinct credentials

The `muse` on `PATH` is a **33 KB bash launcher** (`~/.local/bin/muse`) that downloads and
execs the real binary (`~/.local/bin/muse-bin-1.0.3-R2198.1`, 242 MB). There are two
independent credentials, and confusing them is easy:

1. **The launcher's download token** — an OIDC device-code login against
   `https://auth.meta.com` (`MUSE_AUTH_URL`), `client_id` `1031625952748946`
   (`MUSE_CLIENT_ID`), endpoints `/oidc/device/authorization/` and `/oidc/device/token/`,
   grant `urn:ietf:params:oauth:grant-type:device_code`. Used only to fetch binaries from
   `lookaside.facebook.com`.
2. **The provider credential** — what `muse login` / `muse auth set` stores and what the agent
   uses to call the model.

Both are keyed out of `~/.config/muse/auth.json` (`MUSE_AUTH_PATH` overrides;
`$XDG_CONFIG_HOME/muse/auth.json` when set).

### 3.2 `muse login` — device code flow

```
$ muse login --help
usage: muse login

Log in with your Meta account: approve a code in your browser.
META_API_KEY always takes priority over the account login.
```

The launcher's `device_login()` prints **to stderr**, in this exact shape:

```
Open this page to sign in:
  <verification_uri_complete or verification_uri>
Confirm this code matches:            # or "Enter this code:" when there is no _complete URI
  <user_code>                         # bold via tput when stderr is a tty and NO_COLOR is unset

Waiting for approval (link expires in N minutes)...
Signed in.
```

It then polls `token_endpoint` every `interval` seconds (default 5, `+5` on `slow_down`),
handling `authorization_pending`, `slow_down`, `access_denied`
("muse: the sign-in request was denied"), `expired_token` and a hard 1800 s ceiling
(`((lifetime <= 1800)) || lifetime=1800`). It also offers to open the verification page.

**Can a client drive it non-interactively?** Not as an API — there is no MSP auth method and
no JSON output. A GUI must **spawn `muse login` as a child, parse stderr** for the two lines
after `Open this page to sign in:` and `Enter this code:` / `Confirm this code matches:`, and
watch for `Signed in.` / the `muse: …` failure lines. Two gates matter:
`login_after_denial()` refuses to prompt when `MUSE_LOGIN=0`, and when `MUSE_LOGIN != 1` **it
also refuses if stderr is not a tty** — so a GUI spawning `muse login` **must set
`MUSE_LOGIN=1`** or give it a pty. The values are opaque strings; do not log them.

### 3.3 `muse logout`, `muse auth set`, `META_API_KEY`

```
$ muse logout --help
Remove the saved Meta credential (API key or Meta-account login).
META_API_KEY in the environment is not touched.

$ muse auth --help
Usage: muse auth set [--provider <PROVIDER>] --api-key-stdin
The API key is read from stdin (never taken as a command-line argument, so it
never lands in shell history).
```

**Precedence: `META_API_KEY` (environment) > stored credential (API key or account login).**
Both `muse login` and `muse logout` say so explicitly. `muse exec` additionally accepts
`--api-key-stdin` per invocation. `muse serve` has **no** auth flag — it inherits the ambient
credential, so the app's login screen is "run `muse login`, then respawn/retry".

### 3.4 What is in `auth.json` (key names only)

```
schema_version = 2
providers.meta.mechanism    (string)
providers.meta.storage      (string)
providers.meta.obtained_via (string)
providers.meta.api_base_url (string)
providers.meta.user_full_name (string)
providers.meta.user_email   (string)
```

No token material is in the file on this machine — the durable `session_opened` record reports
`"credential_backend":"keychain","keychain_fallback_reason":"none"`, i.e. the secret lives in
the macOS keychain and `auth.json` is only the pointer plus display identity. `providers.meta.
user_full_name` / `user_email` are exactly what a "signed in as …" chip should show.
File mode is `0600`; siblings are `settings.json` (0644), `trust.json` (0600) and `.lock` files.

### 3.5 Detecting logged-out / expired

There is **no MSP auth error kind** — `ErrorKind` has no `unauthenticated`. So:

- `initialize` **succeeds while logged out** (it does no provider I/O), and so does
  `session/start`. Auth only fails when a model call happens.
- The failure therefore surfaces as **`turn/completed` with `terminal: "failed"` and
  `error.kind: "modelError"` (or `configError`)** — a view event, not a JSON-RPC error. This is
  the string the client must classify.
- Cheap pre-flight: `model/list` returns `source: "providerCatalog"` when the catalog was
  fetched with a live credential, versus `bundledCatalog` / `unresolvedCatalog` /
  `fakeCatalog` otherwise. Combined with the presence of `providers.meta` in `auth.json`, that
  is a good enough "are we signed in" probe before the first turn.
- The binary carries the string `meta is not authenticated`, which the TUI shows; the MSP host
  has no equivalent structured signal.

---

## 4. Outside MSP — what the TUI does that the wire does not

### 4.1 There is no plan mode

`/plan` **exists but is a skill, not a mode**: the palette lists it as
`/plan · built-in · default skill — "Create a grounded, decision-complete plan, then stop for
approval. Use ONLY when the user explicitly as…"`. There is no `PermissionMode::Plan`
equivalent on the wire, no read-only turn flag, and `ApprovalMode` is closed at four values.

The closest thing is a **permission profile**. `--permission-profile <ID>` selects a *named*
profile; there is no list subcommand, but the binary carries the built-in ids and labels:

| id | label | description (verbatim from the binary) |
|---|---|---|
| *(read-only profile)* | — | `Reads files only.` |
| `ask-me` | Ask me | `Edits workspace and temporary files; requires approval for protected writes, new network targets, and other restricted actions.` |
| `auto-review` | Auto-review | `Same access as Ask me; AI reviews eligible actions; trusted permission hooks may decide approval requests first.` |
| `unrestricted` | Unrestricted | `No filesystem sandbox or approval prompts; local commands have direct network access; project trust is unchanged.` |

(The `Reads files only.` profile's id sits immediately before the `ask-me` label block in the
string table but is not separately recoverable from `strings`; enumerate it at runtime.)
Other profile machinery in the binary: `permission_profile.resolve`, sources
`fallback | user_named | managed_named | unavailable`, error strings
`Named permission profiles are unavailable: no permission profile is available.`,
`--permission-profile cannot be used with …`, `escalated execution requires an unrestricted
permission profile`, and enterprise config keys `execution.permission_profiles`,
`execution.approval_modes`, `execution.forbid_approval_bypass`, `execution.forbid_sandbox_bypass`,
`model_egress.allowed_models`, `model_egress.allowed_providers`, `model_egress.web_search`.

**`muse serve` exposes none of this.** A profile cannot be selected over MSP; only
`ApprovalMode` can. If the app wants "plan mode", it must implement it itself — e.g. a
`denyUnmatched` session plus a system-prompt convention, or by sending the `/plan` skill's
prompt.

### 4.2 Slash commands (TUI, driven live under a pty)

39 entries. Captured by running `muse --provider echo --trust-workspace` in a pty (answering
its DSR/OSC queries) and paging the `/` palette:

| Command | Description |
|---|---|
| `/compact` | Summarize the conversation to free up context |
| `/copy` | Copy the last response to the clipboard |
| `/deep-research` | Research a question across sources with cross-checking and citations |
| `/effort` | Set the model's effort level |
| `/export` | Save the conversation, or the full session log |
| `/feedback` | Send quick feedback to the team |
| `/fork` | Branch this session from the latest message |
| `/goal` | Start or manage continuous work toward a goal |
| `/help` | Show help |
| `/init` | Explore the workspace and create or improve AGENTS.md |
| `/keymap` | Show keyboard shortcuts |
| `/logout` | Log out, forget the saved login, and exit |
| `/model` | Choose model |
| `/name` | Show or rename this session |
| `/quit` | Quit when idle |
| `/recap` | Show a recap of recent session activity |
| `/resume` | Resume an earlier Muse Code session |
| `/rules` | Show which md files govern this session |
| `/settings` | Open local settings |
| `/side` | Start a side conversation |
| `/skills` | Browse and use skills |
| `/status` | Show current session status |
| `/stop` | Stop all background tasks |
| `/subagents` | View running and past subagents |
| `/tasks` | View and manage background tasks and terminals |
| `/theme` | Choose color theme |
| `/upgrade` | Show your subscription plan |
| `/usage` | Show session usage |
| `/voice` | Manage voice input |
| `/workflows` | Browse workflow runs |
| `/loop` | Schedule a recurring prompt |
| `/create-skill`, `/decoction`, `/doctor`, `/grill`, `/grill-and-record`, `/import`, `/manage-settings`, `/plan` | skill entries (`· built-in \| user · default skill`) |

Two structural facts for the app's own `/` menu: **skills appear in the same list as commands,
tagged with their source**, which is exactly `aui::composer::CommandItem::source_tag`; and
`/effort` is provider-gated (`◆ /effort is not available for the echo provider`).

TUI keymap (37 rows, from `/keymap`) worth mirroring:

| Action | Keys |
|---|---|
| Commands | `/` |
| Shell command | `!` |
| File paths | `@` |
| Shortcut overlay | `?` (composer empty) |
| Submit | `enter` |
| New line | `shift+enter` / `ctrl+j` / `ctrl+m` |
| **Queue while running** | **`alt+enter`** ("Queue or steer input while a run is active") |
| Interrupt or exit | `ctrl+c` |
| Expand tool output | `ctrl+o` |
| Process background | `ctrl+b` |
| Paste images | `ctrl+v` / `super+v` |
| External editor | `ctrl+g` (`$VISUAL`/`$EDITOR`) |
| History search / prev / next | `ctrl+r` / `ctrl+p` / `ctrl+n` |
| Cancel or close | `esc` |
| Clear terminal | `ctrl+l` |

### 4.3 Session naming, status, export, worktrees, trust, skills

- **Session naming is not on the wire.** `/name` ("Show or rename this session") writes a
  durable `SessionNameChanged` record; the index has `session_name` and
  `session_name_revision`. Names are auto-generated two-word slugs (`dew-sidereal` in
  `/status`). MSP has **no rename method and no name field on `Session`** — an app that wants
  names must read `session-index.db` or keep its own.
- **`/status`** shows: version + session name + `IDLE`, MODEL (`echo · native-basic`),
  WORKSPACE + `trusted · not found`, **ACCESS `Ask · me`** (the permission profile), USAGE
  (`0 tokens · 0 turns · 0 subagents`), CONTEXT (`not projected`), SESSION uuid, ACTIVITY
  (`no tasks`, `0 terminals · inbox clear`). A good spec for the app's session header.
- **Workspace trust is a first-run gate**: launching in an untrusted workspace shows
  *"Do you trust this workspace? … Trusting allows project-local skills, rules, hooks, and
  plugin config to load before the model runs."* with `1 Trust and continue` / `2 Quit`.
  `muse serve --trust-workspace` opts in for the host; there is **no per-session trust on the
  wire**. Trust is stored in `~/.config/muse/trust.json`.
- **`muse export`** — `[--session <id|path>] [--last] [--out <file>] [--redacted]`, writes a
  single self-contained JSON document (`export_schema_version 1`) with timestamps, messages,
  verbatim encrypted reasoning, tool calls/results, approvals, question outcomes, model ids,
  `ses_`/`trajectory_` ids and fork/subagent lineage. Offline. Prints exactly one line: the
  absolute path. Not on the wire.
- **Worktrees** — `-w/--worktree off|create|existing`, `--worktree-base <REF>` (default HEAD),
  `--worktree-existing <PATH>`. Session-level and **CLI-only**; `session/start` has no worktree
  parameter. The durable log has `SessionWorktreeOperationIntent`, `SessionWorktreePrepared`,
  `SessionWorktreeSetupFailed`, `WorktreeCleanupOutcome` records, so worktrees exist in the
  runtime but are unreachable from MSP.
- **`--yolo`** = "Disable approval and sandboxing and trust this workspace for this run" —
  the composite of `--disable-approval`, `--disable-sandbox` and `--trust-workspace`. On the
  wire the nearest thing is `approvalMode: "allowAll"`, which does **not** disable the sandbox;
  sandbox posture is a `muse serve` flag.
- **`--approval-judge <off|on>`** (default **on**) — "LLM approval judge for Prompt-bound
  calls". Its effect is visible on the wire as `judgeEscalated: true` on an approval and
  `resolvedBy: "llmJudge"` on a resolution. Not selectable over MSP.
- **Skills** — `muse skills list|inspect|enable|disable|user-only|validate|install|import|
  update|uninstall`, scopes `user|project|built-in|plugin`, `--json` on every subcommand, and
  `muse skills import --from claude|codex`. Bundled skills live at
  `~/.local/share/muse/skills/bundled/muse-core/skills`. Skills surface in MSP only as ordinary
  `toolCall` items; the app should shell out to `muse skills list --json` to populate a
  `/`-menu "Skills" section.
- **`muse init [--dry-run] [--force]`** — "Scaffold agent config in this workspace"
  (`AGENTS.md`); `/init` is the in-TUI equivalent.
- Other CLI-only knobs the app may want to expose by respawning the host:
  `--preset native-basic|miniswe`, `--agents <JSON>` (ephemeral agent-definition overlay),
  `--parallel-tool-calls` / `--no-parallel-tool-calls`, `--enable-shell-tool`,
  `--context-compaction-strategy summary-preserved-suffix/v1 | prefix-extension-summary/v1 |
  prefix-extension-inventory-summary/v1` with soft/hard thresholds (on `muse exec`), and
  `muse session-message list|send --target <session-uuid-or-name>` for cross-session messages.

### 4.4 `~/.local/share/muse` on disk

```
~/.local/share/muse/
├── session-index.db            SQLite; the session list the TUI picker uses
├── sessions/
│   ├── 2026/09/08/<session-uuid>/session.jsonl      the durable log (append-only records)
│   └── .msp-view-v1/<session-uuid>/                 the MSP view fold cache
│       ├── HEAD.json  index-00000000.bin  journal-00000000.bin
│       └── snapshot-<uuid>.json                     folded snapshots
├── model-catalog/6d657461__p746268.json             hex(provider)__p+hex(profile)
├── skills/bundled/muse-core/skills/
├── plugins/cache/builtin/muse-core/
├── feature-config/7075626c6963.json                 hex("public")
├── runtime/muse/{sessions, .session-registry.mutation.lock}
└── tui-history.jsonl (+ .lock)                      composer history
```

`session.jsonl` records are envelopes:
`{schema_version, id, stream:{kind,id}, sequence, recorded_at (µs), record_type:"event",
durability:"durable", causation_id, payload_type, payload_schema_version, payload}` —
`payload_type` values seen include `runtime.command_intake.received`,
`runtime.session.metadata`, `session.opened.observed`. The `sequence` and record `id` are
exactly what a `sourceRange` points at.

**`session-index.db` → `sessions` table** (the schema an app could read directly for its
session list, instead of paging `session/list`):

```sql
CREATE TABLE sessions (
  session_id TEXT PRIMARY KEY, session_stream_id TEXT NOT NULL,
  session_dir TEXT NOT NULL, session_log_path TEXT NOT NULL UNIQUE,
  layout TEXT NOT NULL, workspace_root TEXT, workspace_key TEXT,
  provider_id TEXT, model_id TEXT, git_branch TEXT,
  title TEXT NOT NULL, first_user_prompt TEXT, search_text TEXT NOT NULL,
  created_at_us INTEGER, updated_at_us INTEGER,
  prompt_count INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL, status_rank INTEGER NOT NULL,
  source_fingerprint TEXT, indexed_at_us INTEGER NOT NULL,
  latest_segment_terminated INTEGER NOT NULL DEFAULT 0,
  session_name TEXT, session_name_revision INTEGER CHECK(session_name_revision >= 0),
  msp_created_at_us INTEGER, msp_updated_at_us INTEGER, msp_turn_count INTEGER,
  msp_fork_source_session_id TEXT, msp_fork_cut_cursor TEXT,
  msp_fork_cut_explicit INTEGER, msp_fork_command_id TEXT,
  msp_provider_id TEXT, msp_model_id TEXT, msp_source_fingerprint TEXT,
  CHECK ((session_name IS NULL) = (session_name_revision IS NULL)));
```

Indexes exist on `(status_rank, updated_at_us DESC, …)`, `(workspace_key, …)`,
`(created_at_us DESC, …)`, `provider_id` and `status` — i.e. the DB is already shaped for
"group by workspace, sort by recency". It gives the app four things MSP does not:
**`session_name`, `title`, `first_user_prompt`, `search_text`** (search!) and `git_branch`.
Recommendation: use `session/list` as the source of truth for identity and status, and read
`session-index.db` **read-only** to enrich rows with name/title/search. Never write to it.

`model-catalog/*.json` is the only place `reasoning_effort_variants` exists:

```json
{"schema_version":1,"provider_id":"meta","profile_id":"tbh","source":"provider_catalog",
 "rows":[{"model_id":"muse-spark-1.3","display_label":"muse-spark-1.3","visibility":"visible",
   "release_date":"2026-09-02","display_order":null,"is_current":false,"is_default":false,
   "roles":[],"context_limit":1007997,"output_limit":128000,"description":null,"cost":null,
   "reasoning_effort_variants":[{"tier":"minimal","description":null},{"tier":"low",…},
     {"tier":"medium",…},{"tier":"high",…},{"tier":"xhigh",…},{"tier":"max",…}]}]}
```

---

## 5. Reference feature inventory — t3code and synara

> ⚠️ **Correction to the premise: neither repo is a Muse Code / MSP client.** Exhaustive greps
> for `muse`, `\bMSP\b` and `Content-Length` across both trees found only false positives
> (`COMSPEC`, `mspaint.exe`, HTTP headers). Both are **multi-provider** harnesses for Claude
> Code / Codex / Cursor / Grok / OpenCode / Antigravity (synara adds Droid, Pi, Devin). Their
> value here is as *feature references* for a coding-agent chat UI, and their protocol layers
> (ACP + the Codex app-server) are close cousins of MSP — **both frame with newline-delimited
> JSON-RPC 2.0, neither uses `Content-Length`**, which independently corroborates §1.1.
>
> Synara is architecturally a derivative of t3code: same Effect-TS + React + TanStack Router +
> Lexical + `@pierre/diffs` + `@legendapp/list` + zustand stack, same `apps/{web,desktop,server}`
> + `packages/{contracts,shared}` layout, same `orchestration.dispatchCommand` bus, same
> `thread.*` command literals. Most differences below are synara's *additions*.
>
> Path prefixes: `T3` = `…/scratchpad/t3code`, `SY` = `…/scratchpad/synara`.

### 5.1 t3code (pingdotgg/t3code)

**Stack.** pnpm monorepo, Effect-TS (effect-smol) throughout. `apps/web` React 19 + React
Compiler + TanStack Router + Tailwind 4 + `@base-ui/react` + Lexical composer + zustand;
`apps/desktop` Electron (Ghostty terminal, `@clerk/electron`); `apps/mobile` Expo with Swift /
Kotlin native modules; `apps/server` Node Effect HTTP/WS. `packages/effect-acp` and
`packages/effect-codex-app-server` are hand-written Effect RPC clients; `native/resource-monitor`
is Rust.

**Protocol.** NDJSON JSON-RPC 2.0: ACP `RpcSerialization.ndJsonRpc()`
`T3/packages/effect-acp/src/protocol.ts:88`, `encodeJsonl` appends `\n`
`_internal/shared.ts:105-108`; Codex `encodeWireMessage`
`T3/packages/effect-codex-app-server/src/protocol.ts:100-104` with a line-split decoder
`:399-419`. ACP calls: `initialize, authenticate, logout, session/new, session/load,
session/list, session/fork, session/resume, session/close, session/prompt, session/cancel,
session/set_config_option, session/set_mode, session/set_model`; handles `session/update,
session/request_permission, session/elicitation(+/complete), fs/read_text_file,
fs/write_text_file, terminal/create|output|wait_for_exit|kill|release`
(`client.ts:148-222`). Codex app-server calls `initialize, thread/start, thread/resume,
thread/compact/start, turn/start, turn/interrupt, thread/read, thread/rollback,
account/rateLimits/read, skills/list` and handles requests
`item/commandExecution/requestApproval, item/fileChange/requestApproval,
mcpServer/elicitation/request, item/tool/requestUserInput`
(`CodexSessionRuntime.ts:722-2442`).

**Composer.** Send: `submitComposerDraft`
`T3/apps/web/src/components/chat/composerSubmission.ts:33`, `submitComposer(event, intent:
"foreground"|"background")` `ChatComposer.tsx:2996`, button `ComposerPrimaryActions.tsx:222-259`,
120k-char cap `PROVIDER_SEND_TURN_MAX_INPUT_CHARS` `T3/packages/contracts/src/orchestration.ts:158`.
**Queue-while-busy: absent on desktop/web** (only a prompt `stashQueue` and an
`attachmentUploadQueue`); a real outbox exists only in mobile
(`T3/apps/mobile/src/state/thread-outbox-manager.ts`). **Steer is implicit** — Enter while
running sends into the live turn, same `turnId`
(`T3/apps/server/src/provider/Layers/ClaudeAdapter.ts:4892-4899`). Attachments:
`classifyComposerAttachmentFile` `composerAttachmentFiles.ts:65` (image / file /
unsupported-image, HEIC→JPEG), paste `ChatComposer.tsx:4516`, drag-drop
`composerMentionDrag.ts:54` (MIME `application/x-t3code-composer-mention`), limits 8 files /
10 MB image / 50 MB file. Slash commands: a closed built-in set
`ComposerSlashCommand = "model"|"plan"|"default"` `composer-logic.ts:13` plus `/usage-limits`,
with everything else **provider-supplied at runtime**
(`resolveProviderSlashCommandsForCwd` `ChatComposer.tsx:1758`); skills appear as
`/skill:<name>` (`:2077-2085`); menu item kinds
`"path"|"slash-command"|"provider-slash-command"|"skill"` `ComposerCommandMenu.tsx:61`.
Mentions: `@` files/dirs, `$` skills, **no agent mentions**
(`ComposerTriggerKind` `T3/packages/shared/src/composerTrigger.ts:1`). Model picker
`ProviderModelPicker.tsx:29` → `ModelPickerContent.tsx:122` (virtualized, search, favorites)
with `mod+shift+m` and `mod+1..9` (`keybindings.ts:46-60`). Effort: `TraitsPicker.tsx`,
provider-driven descriptors labelled `effort/reasoningEffort/variant/agent` (`:46-51`);
Claude "ultrathink" is a prompt-prefix injection (`:344-350`). Permission mode: **four**
`RuntimeMode`s `orchestration.ts:120-127` — `approval-required` "Supervised",
`auto-accept-edits`, `auto`, `full-access`. Plan mode: `ProviderInteractionMode =
"default"|"plan"` (`:128-130`), Plan/Build toggle `ChatComposer.tsx:1006-1046`, **Shift+Tab**
`:3216-3219`, plan-ready banner `ComposerPlanFollowUpBanner.tsx:4` →
Refine / Implement / Implement-in-new-thread. Context meter `ContextWindowMeter.tsx:18`: ring +
popover with `pct · used/max`, "Total processed", auto-compaction line and a **Compact button**
(`:142-151`) that injects `/compact` (`ChatComposer.tsx:3057-3083`), gated on the provider
advertising a `compact` command. Stop: `ComposerPrimaryActions.tsx:89-108` →
`interruptThreadTurn` `ChatView.tsx:3552`.

**Transcript.** `react-markdown` + remarkGfm/alerts/directives, `ChatMarkdown.tsx:3116`;
streaming is only a highlight-cache gate (`:1015`) — no smoothing. Shiki via `@pierre/diffs`
`preferredHighlighter: "shiki-wasm"` `lib/syntaxHighlighting.ts:16`, wrap + copy toolbar
(`:886-993`). **Reasoning is dropped server-side**
(`ProviderRuntimeIngestion.ts:1479-1481`) — only a "Thinking" shimmer row
(`MessagesTimeline.tsx:1836-1846`). **No tool-name registry**: one generic `PlainWorkEntryRow`
(`:3265-3459`), classification `toolGroupAction →
"read"|"edit"|"command"|"browser"|"code-search"|"search"|"other"|"update"`
(`T3/packages/client-runtime/src/work-log/presentation.ts:405-433`), icons switched on
`itemType` (`:3151-3158`), shell labels from a full command parser (`commandLabel.ts:1367`),
own MCP tools in a label table `T3_MCP_TOOL_LABELS` (`presentation.ts:57-102`). Expanded body
is a `<pre>`; MCP is a raw JSON dump (`:3097`). Only bespoke card is the subagent spawn CTA
`AgentSpawnCtaRow` (`:3173-3240`). **No diffs in the transcript** — assistant edits render a
`ChangedFilesCard` tree (`ChangedFilesTree.tsx:25`) opening `DiffPanel.tsx` (split/unified at
`:1018`). Approvals live **in the composer dock**, not the transcript:
`ComposerPendingApprovalPanel.tsx:11` (kinds `mcp-elicitation|command|file-read|file-change`),
options `cancel / decline / acceptForSession "Always allow this session" / accept`
(`ComposerPendingApprovalActions.tsx:22-27`) — **no keyboard shortcuts**. Questions:
`ComposerPendingUserInputPanel.tsx:23`, single (auto-advance 200 ms) vs `multiSelect`,
**digits 1–9** (`:141-166`), `n/N` counter, **answers can carry attachments**; answered
questions replay as `QuestionAnswerHistory` (`:3465-3526`). Plan card
`ProposedPlanCard.tsx:35`; todos in the composer `ComposerTasksBadge.tsx:102`. Errors: a
dismissible non-modal `ThreadErrorBanner.tsx:36-71`, inline error rows detected by regex
(`presentation.ts:340-357`), **no retry**. Interrupted marker: "You stopped after {duration}"
(`MessagesTimeline.logic.ts:706-712`). Compaction marker: `ContextCompactionTimelineRow`
separator "Compacted context X → Y tokens" (`MessagesTimeline.tsx:1322-1341`). **No per-message
token/cost** — cost lives in `components/usage/*`.

**Session list.** `Sidebar.tsx` (~4.1k lines), sections
`"pinned"|"active"|"snoozed"|"settled"` (`Sidebar.logic.ts:88`) then by project, drag-reorder.
Menu (`threadActionMenu.logic.ts:9-27`): pin, settle, snooze, rename, regenerate-title,
mark-unread, copy, archive, delete → `thread.delete` (`orchestration.ts:938`),
`thread.archive` (:944), `thread.meta.update` (:1035). **Fork is not surfaced** although the
protocol supports `session/fork`. Search: in-memory title filter (`Sidebar.logic.ts:882`) +
server `orchestration.searchThreads` + a content-search dialog.

**Auth.** Provider auth is out-of-band CLI except **Antigravity**, which has in-app OAuth:
`ANTIGRAVITY_AUTH_METHODS` (`oauth-personal`, `oauth-business`, `gemini-api-key`,
`agent-platform`) `T3/packages/contracts/src/settings.ts:709-723`, UI
`ProviderSetupSection.tsx` with `AUTH_PHASE_LABELS` (`:36-43`), RPCs
`provider.auth.start/complete/cancel/logout/subscribe` (`rpc.ts:261-271`). For Claude/Codex a
welcome wizard **embeds a terminal** running `claude auth login` / `codex login`
(`providerReadiness.logic.ts:104-129`). Failures surface as an inline
`ProviderStatusBanner.tsx` ("X is unauthenticated") plus toasts — **no modal**. Cloud account
auth is Clerk.

### 5.2 synara (Emanuele-web04/synara)

**Stack.** Bun workspaces + turbo, Effect-TS. `apps/web` React + TanStack Router/Query/Virtual
+ Tailwind + `@base-ui/react` + shadcn + Lexical + **xterm.js** + pdfjs + katex; `apps/desktop`
thin Electron with a native Swift screen-capture helper; `apps/server` Bun Effect + SQLite.
ACP via the external `@agentclientprotocol/sdk`; Codex via a hand-rolled JSONL transport.

**Protocol.** NDJSON again: `acpSdk.ndJsonStream(output, input)`
`SY/apps/server/src/provider/acp/AcpSessionRuntime.ts:780-781` with a byte-level `0x0a`
frame-size guard (`:261-287`); `CodexJsonlFramer.push` splits on `0x0a`, CRLF-tolerant, fatal
UTF-8 decoder, 16 MB max frame `SY/apps/server/src/codexAppServerTransport.ts:35-129`.
Codex calls include `turn/steer`, `thread/fork`, `thread/compact/start`, `review/start`,
`skills/list`, `plugin/list`, `item/requestApproval/decision`,
`item/tool/requestUserInput/answered` (`codexAppServerManager.ts:3708-3720`). Grok ACP
extensions `x.ai/ask_user_question`, `x.ai/exit_plan_mode` (`GrokAcpExtension.ts:5-6`).

**Composer.** `onSend(e, requestedDispatchMode?: "queue"|"steer", queuedTurn?)`
`SY/apps/web/src/components/ChatView.tsx:7604`. **Queue-while-busy is first class**:
`FollowUpBehavior = "queue"|"steer"` (default queue) `appSettings.ts:110-112`, Cmd/Ctrl+Enter
flips, `enqueueQueuedComposerTurn` (`ChatView.tsx:8094`) snapshots prompt + attachments +
model + modes, rows in `ComposerQueuedHeader.tsx:68` with **Steer / Edit / Delete**
(`QueuedComposerActions.tsx:32-59`), auto-drain `lib/queuedComposerDrain.ts:150`.
**Explicit steer** `onSteerQueuedComposerTurn` (`ChatView.tsx:9649`); providers without native
steering get interrupt+re-dispatch behind a "steer gate"
(`providerSupportsNativeTurnSteering` `SY/packages/shared/src/providerMetadata.ts:132`), and
steered turns get a "Steering conversation" marker (`MessagesTimeline.tsx:248-254`).
Attachments ordered assistant-selections → browser-annotations → file-comments → pasted-text →
files → images (`ComposerReferenceAttachments.tsx:40`); long paste auto-collapses to a chip at
≥25 lines / 4000 chars (`composerPastedText.ts:34-35`). **17 built-in slash commands**
`BUILT_IN_COMPOSER_SLASH_COMMANDS` `SY/packages/shared/src/composerSlashCommands.ts:7-25`:
`clear, compact, model, plan, debug, default, review, fork, side, status, subagents, fast,
export, goal, rename, feedback, automation` — a near-superset of Muse's own list, merged with
provider-native commands via a Claude alias table (`:43-52`); `/status` opens
`ComposerSlashStatusDialog.tsx:46`. Menu item kinds:
`path|local-root|slash-command|provider-native-command|fork-target|review-target|model|plugin|
thread|skill|agent` (`ComposerCommandMenu.tsx:146-224`). Mentions cover files, an `@local`
folder browser, plugins, threads, skills (`$`) **and agents** (`@alias()`,
`ChatView.tsx:10769-10850`) with Lexical nodes
`ComposerMentionNode/SkillNode/SlashCommandNode/AgentMentionNode/TerminalContextNode`.
**Combined model+effort picker** `ComposerModelEffortPicker.tsx`. Permission mode: **three**
(`RUNTIME_MODE_PRESENTATION` `lib/runtimeMode.ts:28-44`) — `approval-required` "Ask for
approval", `auto` "Approve for me", `full-access`. Modes `"default"|"plan"|"debug"`
(`ComposerExtrasMenu.tsx:84-101`), Shift+Tab (`ChatView.tsx:11031-11034`). Context meter
`ContextWindowMeter.tsx:9`: pct · used/max, "Model window", "Next turn", "Total processed",
auto-compact note and **"Session cost: $"** (`:114`); **no compact button** — `/compact` only
(`useComposerSlashCommands.ts:169-188`). Stop `ChatView.tsx:12168-12186` plus separate
`thread.task.stop` for workflows and per-subagent stops. **Voice input**
(`ComposerVoiceButton.tsx:9`, waveform bar, `.transcribeVoice` RPC). A whole stack of panels
sits above the input (`ChatView.tsx:11753-11874`): live-changes header with Review,
active-task card, `WorkflowRunCard` (stop/pause/resume), subagent strip, queued header,
**persistent goal header with timer**, branch-mismatch banner, approval panel, user-input panel.

**Transcript.** `react-markdown` v10 + gfm/breaks/math + KaTeX (`ChatMarkdown.tsx:1485`) with a
**real streaming pipeline**: `useSmoothStreamedText` rAF reveal (40 ms / 2000 cps),
`useDeferredValue` re-parse coalescing (`:1542`), throttled highlight 160 ms → 1 s
(`:1150-1155`), table-delimiter repair (`:40`). Shiki `shiki-js` with an LRU of 500 entries /
50 MB (`lib/syntaxHighlighting.ts:67`); code headers show file + line range (`:1031`).
Reasoning: no collapsible block — consecutive updates compact into one "Reasoning trace" row
(`agentActivity.logic.ts:137-138`) whose expansion is a **full-pane**
`AgentActivityDetailView.tsx:39`. Tool cards dispatch on `itemType`/`requestKind`/
`activityKind` (`TimelineWorkEntryRow.tsx:234-273`) with real bespoke work: a **web-fetch
favicon chip** (`:485`), a GitHub MCP icon (`:293`), a **24-entry browser-tool title table**
(`SY/packages/shared/src/browserAutomationPresentation.ts:3-28`), ~28 running/completed/failed
sentences for its own MCP tools (`lib/toolCallLabel.ts:133-278`), `AutomationCreatedCard`,
`SynaraThreadCreationCard`. Tool details (`ToolCallDetailsDialog.tsx:28`) show an Activity
definition list (Status / Started / Elapsed), the command fence, Files, a **Diff**, an
**Edits Before/After grid** (`:95`), written content, output and exit-code chips. Panels use
`@pierre/diffs` `FileDiffCard` with a **Stacked/Split** toggle (`DiffPanel.tsx:242-259`) and
per-line comments (`FileLineCommentBox.tsx:24`, Cmd+Enter). Approvals above the composer with
**digit shortcuts 1–4** (`ComposerPendingApprovalPanel.tsx:45-70,94-110`): `accept` "Approve
once", `acceptForSession` "Always allow this session", `decline`, `cancel` "Cancel turn";
prompts per kind `command|file-read|file-change|permissions`, and a `permissionProfile` JSON
block. Questions `ComposerPendingUserInputPanel.tsx:32` with label+description options,
**1–9 shortcuts**, single auto-advance, Previous/Next chevrons and an "i of N" counter — **no
free text, no previews**. Plan card + `ProposedPlanActions.tsx:24` (download to `.plan`,
export markdown, copy). Errors are **persistent toasts**, not banners
(`useThreadErrorToast.ts:20`, with a design note at `:56-59`) plus inline
`ProviderHealthBanner` / `RateLimitBanner`; no retry, but **"Edit and resend"**
(`MessagesTimeline.tsx:1829`). No interrupted divider (only a "Cancelled" lifecycle row), but
there **is** a `ForkSourceDivider.tsx:19` "Continued from chat". Compaction is a normal work row
with a `ContextCompactionIcon`. Message footer: Copy → **Fork thread from this turn** → Pin to
panel → time → goal chip (`:2500-2559`); a magnified `MessageTrail` rail; right-dock panes for
browser / iOS simulator / diff / explorer / file / terminal / sidechats / git / PR
(`rightDockPaneMeta.tsx:36-49`).

**Session list.** `Sidebar.tsx` (~7k lines): **Spaces → Pinned → Projects → a parent/child
thread tree** built from `parentThreadId` and `forkSourceThreadId`
(`buildProjectThreadTree` `Sidebar.logic.ts:875`), plus a **Kanban** board
(`components/kanban/`). Menu (`Sidebar.tsx:3016-3040`): rename, pin, clear notification, mark
unread, **Handoff to <provider>**, copy path, open in terminal, copy id, archive (8 s undo
toast), delete → `thread.fork.create` (`orchestration.ts:1212`), `thread.handoff.create`
(:1186), `thread.meta.update` (:1257). Search via `SidebarSearchPalette.tsx` and
`WorkspaceSearchPalette.tsx`.

**Auth.** **No in-app provider login at all.** Auth is probed by shelling out to the CLIs
(`ProviderHealth.ts:118` `CODEX_AUTH_STATUS_ARGS`, `parseAuthStatusFromOutput` `:501`,
`claudeAuthStatus.ts`), state `"authenticated"|"unauthenticated"|"unknown"`
(`SY/packages/contracts/src/server.ts:42-47`); the model-picker row is disabled with an inert
"Sign in" label (`ProviderModelPicker.tsx:73-80`) and `providerUnavailableReason` reads
"X is not authenticated yet." (`providerAvailability.ts:99`). **No error dialogs anywhere.**

### 5.3 Checklist — feature × (t3code, synara, MSP supports it?, `aui` component)

Legend: ✅ yes · ~ partial · — no.

| Feature | t3code | synara | MSP supports it? | `aui` component |
|---|---|---|---|---|
| Send a turn | ✅ | ✅ | ✅ `turn/start` | `composer::composer` + `ComposerIntent::Send` |
| Queue while busy | — (mobile only) | ✅ editable queue | ✅ `ifBusy:"queue"` → `disposition:"queued"` | ~ `composer::queue_row` (single row, no strip) |
| Unqueue a queued turn | — | ✅ Delete | ✅ `turn/unqueue` + `turn/unqueued` | ~ `QueueIntent::Remove` |
| Edit a queued turn | — | ✅ | ✅ unqueue + restore (`turn/unqueued.commandId`) | ~ `QueueIntent::Edit` |
| Explicit steer / interject | ~ (implicit) | ✅ Steer button | ✅ `turn/steer` (needs `expectedTurnId`) | **missing** |
| Image attachments | ✅ | ✅ | ✅ `TurnInputPart{type:"image",base64Data,mediaType}` | `composer::attachment_row`, `UserTurn::attachments` |
| Generic file attachments | ✅ 50 MB | ✅ | — (only text + image parts) | `AttachmentKind::File` |
| Long-paste → chip | — | ✅ | n/a (client-side) | ~ `ComposerChipKind::File` |
| Built-in slash commands | 3 | 17 | — (client-side; Muse TUI has 39) | `composer::command_menu` |
| Provider-native slash commands | ✅ | ✅ | — (no MSP command list) | ~ `CommandItem::source_tag` |
| `@file` mentions | ✅ | ✅ | ✅ as `@path` **text**, chips via `displayText` | `composer::mention_picker` |
| `@agent` mentions | — | ✅ | — | ~ `MentionKind` has no Agent |
| `$skill` mentions | ✅ | ✅ | — (skills are just tool calls) | `MentionKind::Skill` |
| Model picker | ✅ | ✅ | ✅ `model/list` + `session/setModel` | ~ chip only, no menu |
| Reasoning/effort picker | ✅ | ✅ | ✅ `reasoningEffort` (7 tiers, closed) | ~ chip only, no menu |
| Permission/approval-mode picker | ✅ 4 modes | ✅ 3 modes | ✅ `session/setApprovalMode` (4 closed values) | ~ chip only; `PermissionMode` mismatches |
| Plan-mode toggle (Shift+Tab) | ✅ | ✅ (+Debug) | **— no plan mode** (`/plan` is a skill) | `PlanState`, `plan_card` |
| Context-usage meter | ✅ ring+popover | ✅ +session cost | ✅ `session/contextUsage` (+`pressure`) | ~ `Composer::context_percent` only |
| Manual compaction button | ✅ in the meter | — (`/compact`) | ✅ `session/compact` (may ack `noop`) | **missing** |
| Auto-compaction display | ✅ | ✅ | ✅ `compaction` item `trigger:"auto"` | ~ `MarkerKind::ContextCompacted` |
| Stop / interrupt | ✅ | ✅ +per-subagent | ✅ `turn/interrupt` (`retract`), `turn/cancel` | `ComposerIntent::Stop`, `StatusRow::key_hint` |
| User shell (`!`) | ✅ (Ghostty/xterm) | ✅ xterm | ✅ `session/userShell` (capability-gated) | `workbench::terminal` |
| Voice input | — | ✅ | — | **missing** |
| Prompt history ↑/↓ | ✅ | ✅ | n/a | **missing** |
| Persistent goal header | — | ✅ | ✅ `session/goalChanged` | **missing** (also missing from `aui-protocol`) |
| Streaming markdown | ✅ basic | ✅ smoothed | ✅ `item/delta` field `"text"` | `assistant_turn().streaming()` + `caret_visible` |
| Code blocks + highlighting | ✅ Shiki wasm | ✅ Shiki js | n/a (markdown in `agentMessage`) | `code_block`, `syntax_runs`, `ts_language` |
| Thinking / reasoning block | — dropped | ~ full-pane detail | ✅ `reasoning` item, `summary.N` deltas | ✅ `thinking_block` (best-in-class of the three) |
| Collapsible reasoning | — | ~ | n/a | ✅ `ThinkingBlock::expanded` |
| Tool cards per tool kind | — generic | ~ icon-by-itemType | — (only a `tool` **name** string) | ✅ `tool_card` + `ToolKind`/`ToolBody` |
| Shell tool output + exit code | ✅ label | ✅ exit-code chips | ✅ `visibleOutput`, `userShell.exitCode` | ✅ `ToolBody::Shell` (`SHELL_FOLD`) |
| Diff in transcript | — | ~ in details | — (no structured diff on the wire) | ✅ `diff_block` |
| Side-by-side diff panel | ✅ | ✅ | — | ✅ `workbench::diff_review` |
| Approval card | ✅ 4 options | ✅ 4 options | ✅ `approval/requested` + `approval/decide` | ~ `approval_card` (fixed triad, no choice list) |
| Approval keyboard shortcuts | — | ✅ 1–4 | n/a | ✅ (`tests/keyboard.rs` drives the card) |
| Multi-stage approval | — | — | ✅ `stages`, `currentRequirementId`, `approval/updated` | **missing** |
| Denial feedback text | — | — | ✅ `feedback` on `acceptsFeedback` choices | **missing** |
| Policy / judge auto-resolution badge | — | ~ `permissionProfile` | ✅ `resolvedBy`, `judgeEscalated`, `protectedWrite` | ~ `ApprovalState::AutoAllowed` only |
| Question card, single/multi | ✅ | ✅ | ✅ `selection.mode` + min/max | ✅ `question_card().multi()` |
| Question digit shortcuts | ✅ 1–9 | ✅ 1–9 | n/a | ~ `QuestionOption::key` |
| Question option previews | — | — | ✅ `UserInputOption.preview` | **missing** |
| Question free text | ✅ attachments | — | ✅ `freeText` / `userInput/clarify` | ~ `allow_other` / `on_other` |
| Question timeout | — | — | ✅ `autoResolutionMs` | **missing** |
| Proposed plan card | ✅ | ✅ | — | ✅ `plan_card` |
| Todo / task list | ✅ composer badge | ✅ card + sidebar | ✅ `session/todoListChanged` | ✅ `todo_list` |
| Subagent rendering | ✅ CTA → panel | ✅ strip + child threads | ✅ `subagent` item + `subagent/*` | ~ `ToolBody::SubAgent { turns }` only |
| Workflow card | ~ CTA | ✅ `WorkflowRunCard` | ~ observe-only (`workflow` item; no methods) | **missing** |
| Error banner (inline) | ✅ | — (toast) | ✅ `turn/completed` failed + inline error kinds | ✅ `feedback::banner`, `error_card` |
| Error **dialog** (modal) | — | — | ✅ needed for `-32700/-3260x/-3202x` | **missing** |
| Retry a failed turn | — | ~ edit-and-resend | ✅ `TurnError.retryable` | ✅ `ErrorCard::on_retry`, `AssistantTurnAction::Retry` |
| Retry-scheduled countdown | — | — | ✅ `turn/retryScheduled` | **missing** |
| Interrupted / cancelled marker | ✅ "You stopped…" | — | ✅ `terminal:"cancelled"`, `turn/retracted` | ~ `marker_row` (no `MarkerKind` variant) |
| Fork-source divider | — | ✅ | ✅ `Session.forkedFrom` | ~ `marker_row` |
| Compaction marker with counts | ✅ separator | ~ icon row | ✅ `compaction` item (`tokensBefore/After`) | ~ `MarkerKind::ContextCompacted` |
| View-gap notice | — | — | ✅ `view/gap` | **missing** |
| Per-message token/cost footer | — | — | ✅ `session/tokenUsage` per completion | ✅ `AssistantTurn::meta(TurnMeta)` |
| Session cost display | — (Usage page) | ✅ in meter | ~ `ModelCost` strings, `cost: null` live | ~ `ComposerMeta::cost` |
| Session list grouping | ✅ status × project | ✅ spaces × project × tree | ~ `session/list` + `workspaceRoot` filter | ✅ `nav::sidebar_view(Grouping)` |
| Session rename | ✅ | ✅ + regenerate | **—** (index-only, `/name`) | **missing** |
| Session fork | — | ✅ | ✅ `session/fork` (+`cutPoint`) | ~ `AssistantTurnAction::Fork` |
| Session resume | implicit | implicit | ✅ `session/resume` (+`cursor`) | ✅ row click |
| Session delete / archive | ✅ | ✅ undo toast | **—** (no MSP method) | **missing** |
| Session search | ✅ | ✅ | **—** (index `search_text` only) | ~ `overlay::command_palette` |
| Backfill an old session | ✅ `thread/read` | ✅ `thread/read` | ✅ `view/page` (+`anchor`) | n/a (data layer) |
| In-app provider login | ✅ Antigravity only | — | **— no MSP auth surface** | **missing** |
| Embedded-terminal CLI login | ✅ wizard | — | n/a (`muse login` device code) | ~ `workbench::terminal` |
| Auth error dialog | — banner/toast | — disabled row | n/a (no `unauthenticated` kind) | **missing** |
| Workspace-trust gate | — | — | — (`muse serve --trust-workspace`) | **missing** |
| Wire framing | NDJSON | NDJSON | **NDJSON** ✔ corroborated | n/a |

### 5.4 Deeper protocol notes from the reference clients

The nearest analogue to MSP in either repo is **OpenAI's `codex app-server`** protocol —
JSON-RPC over a `<binary> app-server` stdio child, method names `thread/start`, `turn/start`,
`item/agentMessage/delta`, `item/commandExecution/requestApproval`. Both repos speak it, and
both frame it the way MSP does.

**t3code's framer** (`T3/packages/effect-codex-app-server/src/protocol.ts:100`, `:398`):

```ts
const encodeWireMessage = (message) =>
  encodeJsonString(message).pipe(Effect.map((encoded) => `${encoded}\n`))
…
Stream.decodeText(),
Stream.runForEach((chunk) => Effect.sync(() => {
  for (let newline = chunk.indexOf("\n"); newline !== -1; newline = chunk.indexOf("\n", start)) {
    remainder.push(chunk.slice(start, newline));
    lines.push(remainder.join("").replace(/\r$/, ""));
```

**synara's framer** is the more defensive of the two and is a good template for `MuseClient`
(`SY/apps/server/src/codexAppServerTransport.ts:34`):

```ts
/** Raw-byte JSONL framing so split UTF-8 sequences never decode prematurely. */
export class CodexJsonlFramer {
  private readonly decoder = new TextDecoder("utf-8", { fatal: true });
  … const newline = bytes.indexOf(0x0a, start); … // takeFrame() strips a trailing 0x0d
```

with `CODEX_APP_SERVER_MAX_FRAME_BYTES = 16MB`, `MAX_QUEUED_STDIN_BYTES = 32MB`, typed failure
reasons `"frame-too-large" | "invalid-utf8" | "unterminated-frame" | "read-closed" |
"write-overloaded" | "write-closed"`, and `handleStdoutLine` (`:3006-3030`) that **logs and
drops** unparseable lines and valid-JSON-without-an-envelope rather than killing the session
(`logIgnoredCodexStdout`). Lift all four of those into `MuseClient`: byte-level newline split,
fatal UTF-8, a frame cap, and tolerance of non-protocol stdout noise.

(One difference from MSP: the Codex dialect's envelope is `{id, method, params}` with **no
`jsonrpc` field**; MSP always carries `"jsonrpc":"2.0"`. Both repos' *ACP* clients do emit it —
`T3/packages/effect-acp/src/protocol.ts:94,570`.)

Findings worth copying, with citations:

- **t3code generates its protocol types** from the schema
  (`T3/packages/effect-codex-app-server/src/_generated/meta.gen.ts`, ~160 methods with
  `ClientRequestParamsByMethod` / `ClientRequestResponsesByMethod` maps). The equivalent for
  this app is generating Rust types from `muse schema generate-json-schema`, and checking
  `InitializeResult.schema.fingerprint` at runtime.
- **t3code discards reasoning at ingestion** —
  `T3/apps/server/src/orchestration/Layers/ProviderRuntimeIngestion.ts:1479`:
  `if (event.type === "content.delta" && event.payload.streamKind !== "assistant_text") { return; }`
  — even though its adapters emit `reasoning_text` / `reasoning_summary_text`. That is why its
  thinking UI is only a shimmer row. **Do not repeat this**: MSP's `reasoning.summary.N` deltas
  are the input to `aui::transcript::thinking_block`, which is already better than either
  reference.
- **synara buffers reasoning summaries and re-emits them as items placed next to the tool rows
  that interrupted them** (`SY/…/ProviderRuntimeIngestion.ts:346, 1188, 1283, 2361, 2806`).
  MSP gives this for free — `reasoning` items are ordered by `viewCursor` alongside `toolCall`
  items, so a cursor-ordered fold reproduces the interleaving with no extra work.
- **synara models streamed assistant text as segments** so text and tool rows interleave in
  execution order rather than one block above every tool call
  (`SY/packages/contracts/src/orchestration.ts:514-527`, `OrchestrationMessageTextSegment` with
  `sequence`/`startedAt`/`endedAt`). MSP does this natively: each `agentMessage` is its own
  item with its own cursor, so a turn with two tool calls yields three `agentMessage` items,
  not one. **The adapter must therefore append a new `Block::Text` per `agentMessage` item, not
  merge them.**
- **synara's approval digit shortcuts carry the right safety comment**
  (`SY/apps/web/src/components/chat/ComposerPendingApprovalPanel.tsx:92-110`):
  *"Digit shortcuts bubble from focused controls inside this card only; a bare number key
  elsewhere in the app must never approve a tool request."* — guarded against
  `HTMLInputElement`/`HTMLTextAreaElement`/`[contenteditable]` targets and modifier keys. Copy
  that rule verbatim into the aui approval card's key handling.
- **t3code's approval option `warning`** renders a triangle + tooltip described as *"a provider
  caution, such as a prompt injection warning on 'allow always'"*
  (`T3/…/ComposerPendingApprovalActions.tsx:58-60`). MSP's analogue is `protectedWrite` and
  `judgeEscalated`.
- **Both put approvals and questions above the composer, not in the transcript.** That is a
  deliberate, twice-independently-arrived-at choice; `aui::transcript::needs_you_banner` plus a
  docked card is the matching shape.
- **t3code's error-banner dismissal is per-`(thread, messageId)` and held module-level so it
  survives remounts** (`T3/…/ThreadErrorBanner.tsx:19-33`). **synara instead uses one sticky
  toast per thread** (`timeout: 0`, `priority: "high"`) with an "Unblock thread" action
  (`SY/…/useThreadErrorToast.ts:26-44`) and a `closeSilently` flag to tell self-close from user
  dismissal.
- **Neither repo has a modal error dialog for *thread* errors** — but both have a general
  confirm/alert dialog host (`T3/apps/web/src/components/ConfirmDialogHost.tsx` with a
  `queuedConfirmations` queue; `SY/apps/web/src/components/ui/alert-dialog.tsx`) and a
  **quit guard** while turns are running (`T3/…/QuitHoldOverlay.tsx`;
  `SY/…/RunningChatsQuitDialog.tsx` + `RunningChatsQuitCoordinator.tsx`). The gpui app needs
  the same two: a queued dialog host, and a "turns are running" quit guard.
- **synara normalizes cancellation vocabulary** in one place
  (`SY/apps/web/src/workLog.ts:905`):
  `["cancelled","canceled","declined","interrupted","killed","stopped","aborted"]`, and
  collapses compaction progress into its terminal row rather than leaving a stale spinner
  (`:1152-1172`). MSP's `ItemStatus` is already normalized, but the compaction
  progress→terminal collapse is a real UI lesson.
- **Neither repo surfaces a per-message token/cost footer.** t3code puts cost behind
  user-editable per-model price tables (`usagePriceTable.ts`, `UsagePriceOverrides.tsx`)
  because providers do not report it; synara shows only a session cost. Given MSP's
  `ModelCost` is `null` on this subscription, `TurnMeta::cost_usd` should be optional in the UI.
- **Session-list ordering**: synara ranks by *attention* before time —
  `threadSortAttentionRank` (`SY/apps/web/src/components/Sidebar.logic.ts:1241-1250`):
  live-working/connecting = 2, unseen-finished = 1, else 0, then timestamp, then id. That maps
  cleanly onto `Session.status` (`running`/`idle`/`notLoaded`) plus a local unread flag.

---

## 6. Gap list for the `aui` library

Read against `docs/06-api.md` (`aui::transcript`, `aui::composer`, `aui::overlay`,
`aui::feedback`, `aui::nav`, `aui::data`).

### 6.1 Missing outright

| Need (MSP / TUI) | Status in `aui` |
|---|---|
| **Modal dialog** (session-identity errors, `sessionInUse`, `parseError`, confirm-destructive) | **missing.** `aui::overlay` has only `command_palette` + `palette_scrim` + `popover_layer`; the module doc says "and later menus, popovers and **dialogs**". `feedback` has `banner` and `toast` only. |
| **Login / auth screen** (device-code URL + user code + "waiting for approval" + signed-in-as) | **missing.** Nothing in `aui` renders an auth state; `nav::sidebar_footer` shows a name + provider usage meter but has no signed-out state. |
| **Reasoning-effort picker** | **partial.** `Composer::effort(label)` renders a chip and `ComposerIntent::Effort` fires — but there is **no menu component** for the seven `ReasoningEffort` tiers. `composer::plus_menu` is the closest primitive. |
| **Model picker** | **partial.** Same shape: `Composer::model` + `ComposerIntent::Model`, no picker. A `ModelCatalogEntry` row wants label + context limit + release date + description + default/active badges; `overlay::PaletteItem` (`context`, `badge`, `keys`) could carry it. |
| **Approval-mode picker** | **partial.** `Composer::mode` + `ComposerIntent::Mode` chip; no menu, and `aui_protocol::PermissionMode` (`ask/plan/auto/bypass`) does not line up with `ApprovalMode` (`allowAll/promptUnmatched/onRequest/denyUnmatched`). |
| **Context-window meter** | **partial.** `Composer::context_percent(u8)` prints `context N%` and `ComposerMeta::context` is a string; `data::usage_meter(provider, fraction)` is the **provider quota** meter, not a context meter. Nothing renders `ContextPressureLevel` (`warning` / `blocked`) as a state, and nothing handles "used tokens, no `windowTokens`". |
| **Compaction marker** | **partial.** `MarkerKind::ContextCompacted` exists in `aui-protocol` and `transcript::marker_row` can draw it, but there is no builder that carries `tokensBefore → tokensAfter`, `trigger`, `outcome` or a "compact now" action. |
| **Queued-messages strip** | **partial.** `composer::queue_row(id, text)` draws **one** queued message with `QueueIntent::{Edit, Remove}`. There is **no container/strip**, no ordering/`+N` overflow, and `QueueIntent` has no `Promote`/`SendNow`. `transcript::StatusRow::note` can say "1 queued message". Mapping: `Remove` → `turn/unqueue`; `Edit` → unqueue + restore text (which is exactly what `turn/unqueued.commandId` is for). |
| **Retry-scheduled row** (`turn/retryScheduled`) | **missing.** `StatusRow` can shimmer a label but nothing renders "attempt 2/5 · retrying in 4 s · reason" with a live countdown. |
| **Multi-stage approval UI** | **missing.** `approval_card` is single-decision: `on_decide(ApprovalDecision::{Once, Always, Deny})`, plus `scope`, `rule`, `cwd`, `capabilities`, `reason`. MSP needs **server-minted choice lists** (`availableChoices[].{choiceId,label,scope,rulePreview,acceptsFeedback}`), a **stage indicator** (`position/totalStages`, `argv`), and a **denial-feedback text field**. |
| **`judgeEscalated` / `protectedWrite` badges** | **missing** on `ApprovalCard`. |
| **Policy-resolved approval** (opened and resolved by policy, never actionable) | **missing state.** `ApprovalState` has `AutoAllowed { rule }` but no auto-**denied** variant. |
| **`userInput/clarify`** ("let me explain instead") | **missing.** `QuestionCard` has `on_answer`, `on_select`, `on_other`, `on_skip` — `on_other` is the closest but it means free-text *as an option*, not a clarification that makes the model re-decide. |
| **Question option previews** (`UserInputOption.preview {content, format}`) | **missing.** `QuestionOption` is `{label, description, key}` — no expandable preview body. |
| **Question timeout** (`autoResolutionMs`) | **missing** — no countdown affordance. |
| **Question `header`** (short label above the prompt) | **partial** — `QuestionCard::subtitle` is *below*; `header` is a different slot. |
| **Goal block** (`session/goalChanged`) | **missing** in both `aui` and `aui-protocol`: objective + status + `percentComplete` + current/next work. |
| **Subagent card** | **partial.** `ToolKind::SubAgent` + `ToolBody::SubAgent { turns }` exist, but no rendering of `role`, `objective`, `depth`, `controlStatus`, transitive `usage`, `SubagentResult.{summary,text,artifactRefs}`, or a "open child session" affordance. |
| **Workflow card** | **missing** entirely (`WorkflowChild` rows). |
| **`view/gap` notice** | **missing** — a "some events were dropped, backfilling…" inline marker. |
| **Session rename / search / delete** | **missing.** `nav::session_row`/`compact_session_row` render `SessionSummary`; there is no rename affordance, no search field, no delete. (`fork` exists as `AssistantTurnAction::Fork`.) |
| **Workspace-trust gate** | **missing** — needed if the app ever launches `muse serve` without `--trust-workspace`. |

### 6.2 Things that already fit well

- `transcript::assistant_turn(...).streaming(true)` + `caret_visible` map straight onto
  `item/delta` on `agentMessage`.
- `thinking_block(text, elapsed, state)` + `summary` maps onto `reasoning` (`summary[n]`
  streamed via field `summary.n`).
- `tool_card(verb, target, status, body)` with `ToolBody::{Shell, Read, Edit, Search, Web,
  Browser, SubAgent, Mcp, None}` and `SHELL_FOLD = 6` covers `toolCall` rendering once the app
  maps `tool` names to kinds.
- `todo_list(items)` maps 1:1 onto `session/todoListChanged` (replace wholesale).
- `error_card(title, detail).on_retry(...)` maps onto `turn/completed` `terminal: "failed"` with
  `TurnError.retryable`.
- `status_row("Working…").elapsed(...).key_hint("esc", "to interrupt").note("1 queued message")`
  is the running-turn affordance.
- `needs_you_banner` is exactly the "blocked on approval / question" banner.
- `feedback::banner(kind, runs)` with `BannerKind::{Waiting, Info, Error, Success}` and
  `BannerRun::{Text, Bold, Mono}` covers the inline half of the error table.
- `composer::command_menu` (with `CommandItem::source_tag`) matches Muse's `/` palette
  including the `· built-in · default skill` tags; `mention_picker` matches `@`.
- `overlay::command_palette` covers `/resume`-style session switching.
- `nav::sidebar_view(Grouping)` + `date_group_header` covers the session list's grouping.

---

## 7. Recommended adapter design

### 7.1 `MuseClient` — child process + JSON-RPC over stdio

```rust
/// One `muse serve` child. Owns the pipes; all I/O on a background thread.
pub struct MuseClient {
    child: std::process::Child,
    next_id: AtomicI64,                                    // client id space
    pending: Mutex<HashMap<i64, oneshot::Sender<Result<Value, MuseError>>>>,
    tx: Sender<String>,                                    // writer thread
    events: Sender<MuseEvent>,                             // to the UI
    granted: Vec<String>,                                  // from InitializeResult
    schema_fingerprint: String,
}

pub enum MuseEvent {
    Notification { method: String, params: Value, cursor: Option<String> },
    ServerRequest { id: Value, method: String, params: Value }, // approval/request, userInput/request
    Closed(Option<i32>),
}
```

Rules the transport must enforce, all learned from the probes:

1. **Framing**: write `serde_json::to_string(&frame)` + `'\n'`; read with `BufReader::lines()`.
   Never buffer more than one line; never emit pretty JSON.
2. **Ids**: `next_id.fetch_add(1)` for client requests. The server has its **own** id space —
   never match a server request's id against `pending`. Dispatch on
   `msg.get("id").is_some() && msg.get("method").is_some()` → `ServerRequest`;
   `id` + (`result`|`error`) → response; `method` only → notification.
3. **`commandId` is a UUIDv7**, minted per command (`uuid::Uuid::now_v7()`); keep it, because
   `turnId == commandId` for fresh turns and `turn/unqueued`/`turn/retracted` echo it back so
   the composer can restore text.
4. **Never respond to `approval/request` / `userInput/request`** with a JSON-RPC result —
   settle them with `approval/decide` / `userInput/answer|cancel|clarify`. Dedupe against the
   matching `…/requested` notification on `approvalId` / `userInputId`.
5. **Ack ≠ outcome.** Every `status: "accepted"` means admitted. The authority is always the
   view event (`turn/completed`, `approval/resolved`, `userInput/settled`, `turn/unqueued`).
6. **Ordering**: view events for a command can arrive *before* its response (observed for
   `turn/start`). Fold by `viewCursor`, not by arrival relative to acks.
7. **`view/gap`** → buffer live events at cursors ≥ `next`, `view/page` the `(after, next)`
   range forward with `limit: 1000`, discard the overlap, splice.
8. **Reconnect**: on child exit, respawn, `initialize`, then `session/resume` with the **last
   observed `viewCursor`** → `history.mode: "none"` and only the suffix streams.
9. Compare `InitializeResult.schema.fingerprint` against the bundle the types were generated
   from; **warn, do not fail**.
10. Set `MUSE_LOGIN=1` (or give a pty) if the app ever spawns `muse login`.

### 7.2 Mapping MSP → `aui_protocol`

Session identity:

| MSP | `aui_protocol` |
|---|---|
| `Session.sessionId` | `Session::id` |
| `Session.providerId` (`"meta"`) | `Session::agent` — **no `Provider::Muse` variant; `Provider` is a closed enum of `Claude/Codex/Grok/Gemini/Pi/Cursor`.** Needs a new variant (plus a mark in `aui-icons`). |
| `Session.modelId` | `Session::model` |
| `Session.workspaceRoot` | `Session::cwd` |
| `session/branchChanged.branch` | `Session::branch` (`None` on detached HEAD) |
| `Session.approvalMode.mode` | `Session::mode` — **lossy**: `allowAll→Bypass`, `onRequest→Auto`, `promptUnmatched→Ask`, `denyUnmatched→` *(no variant)*; `PermissionMode::Plan` has no MSP counterpart. Should become its own enum. |
| — | `Session::environment` — always `Environment::Local` for a local `muse serve`. |
| `Session.forkedFrom`, `turnCount`, `createdAt/updatedAt`, `status`, `path` | **no home** in `aui_protocol::Session`. |

Turns and items:

| MSP notification / item | `Delta` / `Block` |
|---|---|
| `turn/started` | `Delta::TurnStarted { turn: Turn::Assistant { id: turnId, blocks: [], meta: default } }` |
| `item/completed` `userMessage` | `Delta::TurnStarted { turn: Turn::User { id, text, attachments, mentions } }` — note MSP opens the *user* message as an item inside the turn, while `aui_protocol` models it as a separate `Turn`. Use `displayText` when present, else `text`; map `MessageAttachment` → `Attachment { kind: Image, state: Ready }`. |
| `item/started` `agentMessage` | `Delta::BlockAdded { block: Block::Text { text: "", streaming: true } }`; remember `itemId → block_index`. |
| `item/delta` field `"text"` | `Delta::TextDelta { turn_id, block_index, text: delta }` ✅ exact fit |
| `item/completed` `agentMessage` | `Delta::BlockUpdated { .. Block::Text { text, streaming: false } }` (also handles `truncated`) |
| `item/started` `reasoning` | `Delta::BlockAdded { Block::Thinking { text: "", elapsed_ms: 0, summary: None, state: Thinking } }` |
| `item/delta` field `"summary.N"` | **no fit.** `Delta::TextDelta` only appends to `Block::Text`. Either join parts with `\n\n` into `Block::Thinking::text` via `BlockUpdated`, or add a `Delta::ThinkingDelta`. |
| `item/completed` `reasoning` | `Block::Thinking { state: Done, summary: Some(first part) }` |
| `item/started`/`updated`/`completed` `toolCall` | `Block::ToolCall { id: itemId, kind, verb, target, status, duration_ms, body }`. `ItemStatus → ToolStatus`: `inProgress→Running`, `completed→Success`, `failed→Error`, `cancelled→Cancelled`, `rejected→Error`, `timedOut→Error`, unknown→`Error`. `tool` name → `ToolKind` is **app-owned** (`bash→Shell`, `request_user_input→` *(none — render as a Question)*, else `ToolKind::Mcp{server:"muse",tool}`). `args` is a **string**: parse per tool for `verb`/`target`. |
| `item/delta` field `"output"` | append to `ToolBody::Shell::output_lines` via `BlockUpdated` — **no delta variant fits** |
| `toolCall.outputRef` | **no field** on `ToolBody`; drop or surface as a `ToolBody::Shell` footer line |
| `item/*` `userShell` | `Block::ToolCall { kind: Shell, verb: "$", target: commandText, body: ToolBody::Shell { exit_code, .. } }`; `exitSignal` has no home |
| `item/*` `subagent` | `Block::ToolCall { kind: SubAgent, body: ToolBody::SubAgent { turns: [] } }` — `role`/`objective`/`controlStatus`/`result`/`childSessionId` are **all lost** |
| `item/*` `workflow` | **no variant.** Closest is `Block::Activity { steps }` mapping `WorkflowChild → Step` |
| `item/*` `reminderChild` | **no variant** — render generically |
| `item/*` `compaction` | `Delta::BlockAdded { Block::Marker { kind: MarkerKind::ContextCompacted, text: "128k → 22k · auto" } }` — `outcome`/`trigger`/`summarizedThrough` lost |
| `approval/requested` | `Delta::BlockAdded { Block::Approval { id: approvalId, tool: toolName, command: subject.command, reason: "", cwd: subject.workspaceRoot, capabilities: [], scope, state: Pending, rule } }` — **`availableChoices` and `stages` have no home**; see §6.1 |
| `approval/updated` | `Delta::BlockUpdated` on that block (new stage, new choices) |
| `approval/resolved` | `ApprovalState::{AllowedOnce, Denied, AutoAllowed{rule}}` — a policy **denial** has no variant |
| `userInput/requested` | `Delta::BlockAdded { Block::Question { id: userInputId, prompt: q.question, subtitle: q.header, options, multi: selection.mode=="multiple", allow_other: false, answer: None } }` — **one MSP prompt may carry N questions**; `aui_protocol::Block::Question` is one question. Emit N blocks and answer them together. `preview` and `autoResolutionMs` are lost. |
| `userInput/settled` | `Block::Question::answer = Some(Answer { selected, other })` |
| `session/todoListChanged` | `Delta::BlockUpdated { Block::Todo { items } }` (upsert one todo block per session) |
| `session/tokenUsage` + `turn/completed` | `Delta::TurnFinished { meta: TurnMeta { model, duration_ms, tokens_in: promptTokens, tokens_out: outputTokens, cost_usd } }` — **`cost_usd` must be computed client-side** from `ModelCost` decimal strings, and every live row had `cost: null`. |
| `session/contextUsage` | **no `Delta` variant** — route to app state for `Composer::context_percent` |
| `session/goalChanged` | **no variant** at all |
| `session/modelChanged` | `Session::model` (app state); could be a `Block::Marker` |
| `session/approvalModeChanged` | `Block::Marker { kind: MarkerKind::PermissionModeChanged { mode } }` (after the enum widening above) |
| `turn/completed` `terminal:"failed"` | `Delta::BlockAdded { Block::Error { title: error.kind, detail: error.message, retryable: error.retryable } }` then `TurnFinished` |
| `turn/completed` `terminal:"cancelled"` | `Block::Marker` — **`MarkerKind` has no `TurnCancelled`** |
| `turn/retracted` / `turn/unqueued` | remove the optimistic turn, restore composer text — **no `Delta` variant removes a turn** |
| `turn/retryScheduled` | **no variant** |
| `view/gap` | **no variant** |

Intents → MSP:

| `Intent` | MSP call |
|---|---|
| `Send { text, attachments, mentions }` | `turn/start` — mentions rendered as `@path` inside the text part, chips into `displayText`; images as `{type:"image",base64Data,mediaType}` parts |
| `Queue { .. }` | `turn/start` with `ifBusy: "queue"` (or `"steer"` for interject) |
| `Stop` | `turn/interrupt { retract: true }` |
| `Approve { id, decision }` | `approval/decide { approvalId: id, choiceId, requirementId }` — **`ApprovalDecision::{Once,Always,Deny}` must become a `choiceId`**, so the block has to carry the choice list |
| `Answer { id, answer }` | `userInput/answer` — needs option **labels**, not indices |
| `AcceptPlan`/`RejectPlan`/`EditPlan` | **no MSP counterpart** (no plan mode) |
| `SendNotes`, `OpenFile`, `OpenDiff`, `ChangeView`, `ToggleRightPane` | app-local |
| *(missing)* | `turn/unqueue`, `turn/steer`, `session/compact`, `session/setModel`, `session/setApprovalMode`, `session/fork`, `session/userShell`, `userInput/cancel`, `userInput/clarify` all need new `Intent` variants |

### 7.3 Suggested shape

Put the transport in a new `crates/muse-client` (no gpui), and the fold in
`crates/muse-adapter`: `MuseFold { session: aui_protocol::Session, item_index: HashMap<ItemId,
(TurnId, usize)>, last_cursor: String, pending_approvals, pending_inputs, context: Option<ContextUsage>,
todo, goal }`, exposing `fn apply(&mut self, ev: MuseEvent) -> Vec<Delta>` plus side-channel
state the `Delta` enum cannot carry (context usage, goal, queued turns, retry schedule).
Extend `aui-protocol` before the UI work: `Provider::Muse`, an `ApprovalMode`-shaped permission
enum, `Delta::{ThinkingDelta, ToolOutputDelta, TurnRemoved}`, `Block::{Goal, Workflow}`,
`MarkerKind::{TurnCancelled, RetryScheduled, ViewGap}`, and choice lists on `Block::Approval`.
